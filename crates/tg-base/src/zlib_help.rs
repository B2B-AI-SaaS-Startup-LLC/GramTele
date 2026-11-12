//! Port of `lib_base/base/zlib_help.h`.
//!
//! The original wraps minizip (`unzOpen2` / `zipOpen2` over an in-memory
//! `zlib_filefunc_def`) for theme files, cloud blobs (emoji sets,
//! dictionaries) and the statistics `.xlsx` export. The port reimplements
//! the subset of minizip those wrappers reach, on top of `flate2` (raw
//! deflate through `miniz_oxide`), keeping the C-style result codes so the
//! callers translate one to one.
//!
//! Byte compatibility of the written archives: headers are laid out exactly
//! as minizip writes them through `zipOpenNewFileInZip` (version made by 0,
//! version needed 20, the dos date of an all-zero `tm_zip` = 0x00200000,
//! sizes patched into the local header, `internal_fa` = 1 for text detected
//! by zlib's heuristic). Stored entries give byte-identical archives;
//! deflated entries differ only in the compressed bytes, because
//! `miniz_oxide` and zlib choose different (equally valid) encodings.
//!
//! ZIP64 local/central sizes and end records are supported for reading.
//! Not supported: archives split over several disks, encrypted entries (as in
//! the original without a password, their data is fed to inflate as is and
//! fails), and more than 65534 entries or 4 GiB when writing.

use flate2::{Compress, Compression, Crc, Decompress, FlushCompress, FlushDecompress, Status};

pub const UNZ_OK: i32 = 0;
pub const UNZ_END_OF_LIST_OF_FILE: i32 = -100;
pub const UNZ_ERRNO: i32 = -1;
pub const UNZ_EOF: i32 = 0;
pub const UNZ_PARAMERROR: i32 = -102;
pub const UNZ_BADZIPFILE: i32 = -103;
pub const UNZ_INTERNALERROR: i32 = -104;
pub const UNZ_CRCERROR: i32 = -105;

pub const ZIP_OK: i32 = 0;
pub const ZIP_ERRNO: i32 = -1;
pub const ZIP_PARAMERROR: i32 = -102;
pub const ZIP_BADZIPFILE: i32 = -103;
pub const ZIP_INTERNALERROR: i32 = -104;

/// zlib `Z_DEFLATED`.
pub const Z_DEFLATED: i32 = 8;
/// zlib `Z_DEFAULT_COMPRESSION`.
pub const Z_DEFAULT_COMPRESSION: i32 = -1;
/// zlib `Z_STREAM_ERROR`.
pub const Z_STREAM_ERROR: i32 = -2;
/// zlib `Z_DATA_ERROR`.
pub const Z_DATA_ERROR: i32 = -3;

/// `zlib::kCaseSensitive`.
pub const CASE_SENSITIVE: i32 = 1;
/// `zlib::kCaseInsensitive`.
pub const CASE_INSENSITIVE: i32 = 2;

/// minizip `CASESENSITIVITYDEFAULTVALUE`, used for case sensitivity 0.
#[cfg(windows)]
const CASE_SENSITIVITY_DEFAULT: i32 = 2;
#[cfg(not(windows))]
const CASE_SENSITIVITY_DEFAULT: i32 = 1;

/// minizip `UNZ_MAXFILENAMEINZIP`.
const MAX_FILE_NAME_IN_ZIP: usize = 256;

const LOCAL_HEADER_MAGIC: u32 = 0x0403_4b50;
const CENTRAL_HEADER_MAGIC: u32 = 0x0201_4b50;
const END_HEADER_MAGIC: u32 = 0x0605_4b50;
const SIZE_CENTRAL_HEADER: usize = 46;
const SIZE_LOCAL_HEADER: usize = 30;
const SIZE_END_HEADER: usize = 22;

/// minizip `tm_unz` / `tm_zip`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TmZip {
    pub tm_sec: u32,
    pub tm_min: u32,
    pub tm_hour: u32,
    pub tm_mday: u32,
    pub tm_mon: u32,
    pub tm_year: u32,
}

/// minizip `unz_global_info`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnzGlobalInfo {
    pub number_entry: u64,
    pub size_comment: u64,
}

/// minizip `unz_file_info`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnzFileInfo {
    pub version: u64,
    pub version_needed: u64,
    pub flag: u64,
    pub compression_method: u64,
    pub dos_date: u64,
    pub crc: u64,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub size_filename: u64,
    pub size_file_extra: u64,
    pub size_file_comment: u64,
    pub disk_num_start: u64,
    pub internal_fa: u64,
    pub external_fa: u64,
    pub tmu_date: TmZip,
}

/// minizip `zip_fileinfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ZipFileInfo {
    pub tmz_date: TmZip,
    pub dos_date: u64,
    pub internal_fa: u64,
    pub external_fa: u64,
}

fn read_u16(data: &[u8], at: usize) -> Option<u64> {
    data.get(at..at.checked_add(2)?)
        .map(|b| u64::from(u16::from_le_bytes([b[0], b[1]])))
}

fn read_u32(data: &[u8], at: usize) -> Option<u64> {
    data.get(at..at.checked_add(4)?)
        .map(|b| u64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])))
}

fn read_u64(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        data.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

fn zip64_extra(extra: &[u8]) -> Option<&[u8]> {
    let mut at = 0;
    while at + 4 <= extra.len() {
        let kind = read_u16(extra, at)?;
        let length = read_u16(extra, at + 2)? as usize;
        let value = extra.get(at + 4..at + 4 + length)?;
        if kind == 1 {
            return Some(value);
        }
        at += 4 + length;
    }
    None
}

fn dos_date_to_tmu_date(dos_date: u64) -> TmZip {
    let date = dos_date >> 16;
    TmZip {
        tm_mday: (date & 0x1f) as u32,
        tm_mon: (((date & 0x1E0) / 0x20) as u32).wrapping_sub(1),
        tm_year: (((date & 0x0FE00) / 0x0200) + 1980) as u32,
        tm_hour: ((dos_date & 0xF800) / 0x800) as u32,
        tm_min: ((dos_date & 0x7E0) / 0x20) as u32,
        tm_sec: (2 * (dos_date & 0x1f)) as u32,
    }
}

fn tmz_date_to_dos_date(tm: &TmZip) -> u64 {
    let mut year = u64::from(tm.tm_year);
    if year >= 1980 {
        year -= 1980;
    } else if year >= 80 {
        year -= 80;
    }
    let date = u64::from(tm.tm_mday) + 32 * (u64::from(tm.tm_mon) + 1) + 512 * year;
    let time = u64::from(tm.tm_sec) / 2 + 32 * u64::from(tm.tm_min) + 2048 * u64::from(tm.tm_hour);
    ((date << 16) | time) & 0xFFFF_FFFF
}

/// Bytes of a C string: everything before the first NUL.
fn c_str(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|&b| b == 0) {
        Some(end) => &bytes[..end],
        None => bytes,
    }
}

/// minizip `unzStringFileNameCompare`.
fn file_names_equal(a: &[u8], b: &[u8], case_sensitivity: i32) -> bool {
    let case_sensitivity = if case_sensitivity == 0 {
        CASE_SENSITIVITY_DEFAULT
    } else {
        case_sensitivity
    };
    let (a, b) = (c_str(a), c_str(b));
    if case_sensitivity == 1 {
        a == b
    } else {
        // `strcmpcasenosensitive_internal`: only ASCII a-z are folded.
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
    }
}

#[derive(Clone, Debug, Default)]
struct CurrentFile {
    info: UnzFileInfo,
    offset_curfile: u64,
    name: Vec<u8>,
    extra: Vec<u8>,
    comment: Vec<u8>,
}

struct OpenedFile {
    data_start: usize,
    rest_read_compressed: u64,
    rest_read_uncompressed: u64,
    deflated: bool,
    inflate: Decompress,
    crc: Crc,
    crc_wait: u32,
}

/// The `unzFile` handle.
struct Unz {
    global: UnzGlobalInfo,
    byte_before_the_zipfile: u64,
    offset_central_dir: u64,
    num_file: u64,
    pos_in_central_dir: u64,
    current_file_ok: bool,
    current: CurrentFile,
    opened: Option<OpenedFile>,
}

impl Unz {
    /// `unzOpen2`: finds the end of central directory record.
    fn open(data: &[u8]) -> Option<Self> {
        // `unz64local_SearchCentralDir`: the signature closest to the end
        // within the last 0xffff + record bytes.
        if data.len() < 4 {
            return None;
        }
        let lowest = data.len().saturating_sub(0xffff + SIZE_END_HEADER);
        let central_pos = (lowest..=data.len() - 4)
            .rev()
            .find(|&p| read_u32(data, p) == Some(u64::from(END_HEADER_MAGIC)))?;
        if central_pos + SIZE_END_HEADER > data.len() {
            return None;
        }
        let at = |offset| read_u16(data, central_pos + offset).unwrap_or(0);
        let number_disk = at(4);
        let number_disk_with_cd = at(6);
        let mut number_entry = at(8);
        let mut number_entry_cd = at(10);
        let mut size_central_dir = read_u32(data, central_pos + 12).unwrap_or(0);
        let mut offset_central_dir = read_u32(data, central_pos + 16).unwrap_or(0);
        let size_comment = at(20);
        let mut directory_end = central_pos as u64;
        if central_pos >= 20 && read_u32(data, central_pos - 20) == Some(0x0706_4b50) {
            let locator = central_pos - 20;
            if read_u32(data, locator + 4)? != 0 || read_u32(data, locator + 16)? != 1 {
                return None;
            }
            let location = usize::try_from(read_u64(data, locator + 8)?).ok()?;
            let position = if read_u32(data, location) == Some(0x0606_4b50) {
                location
            } else {
                // A self-extracting prefix is not included in ZIP offsets.
                (0..locator)
                    .rev()
                    .find(|&p| read_u32(data, p) == Some(0x0606_4b50))?
            };
            let record = data.get(position..position.checked_add(56)?)?;
            if read_u32(record, 16)? != 0 || read_u32(record, 20)? != 0 {
                return None;
            }
            number_entry = read_u64(record, 24)?;
            number_entry_cd = read_u64(record, 32)?;
            size_central_dir = read_u64(record, 40)?;
            offset_central_dir = read_u64(record, 48)?;
            directory_end = position as u64;
        }
        if number_entry_cd != number_entry || number_disk_with_cd != 0 || number_disk != 0 {
            return None;
        }
        let directory_limit = offset_central_dir.checked_add(size_central_dir)?;
        if directory_end < directory_limit {
            return None;
        }
        let mut result = Self {
            global: UnzGlobalInfo {
                number_entry,
                size_comment,
            },
            byte_before_the_zipfile: directory_end - directory_limit,
            offset_central_dir,
            num_file: 0,
            pos_in_central_dir: 0,
            current_file_ok: false,
            current: CurrentFile::default(),
            opened: None,
        };
        result.go_to_first_file(data);
        Some(result)
    }

    /// `unz64local_GetCurrentFileInfoInternal`.
    fn read_current_info(&mut self, data: &[u8]) -> i32 {
        let Some(base) = self
            .pos_in_central_dir
            .checked_add(self.byte_before_the_zipfile)
            .and_then(|v| usize::try_from(v).ok())
        else {
            return UNZ_BADZIPFILE;
        };
        if base > data.len() {
            return UNZ_ERRNO;
        }
        let Some(header) = data.get(base..base + SIZE_CENTRAL_HEADER) else {
            return UNZ_BADZIPFILE;
        };
        if read_u32(header, 0) != Some(u64::from(CENTRAL_HEADER_MAGIC)) {
            return UNZ_BADZIPFILE;
        }
        let u16_at = |offset| read_u16(header, offset).expect("inside the header");
        let u32_at = |offset| read_u32(header, offset).expect("inside the header");
        let mut info = UnzFileInfo {
            version: u16_at(4),
            version_needed: u16_at(6),
            flag: u16_at(8),
            compression_method: u16_at(10),
            dos_date: u32_at(12),
            crc: u32_at(16),
            compressed_size: u32_at(20),
            uncompressed_size: u32_at(24),
            size_filename: u16_at(28),
            size_file_extra: u16_at(30),
            size_file_comment: u16_at(32),
            disk_num_start: u16_at(34),
            internal_fa: u16_at(36),
            external_fa: u32_at(38),
            tmu_date: TmZip::default(),
        };
        info.tmu_date = dos_date_to_tmu_date(info.dos_date);
        let mut offset_curfile = u32_at(42);
        let name_start = base + SIZE_CENTRAL_HEADER;
        let extra_start = name_start + info.size_filename as usize;
        let comment_start = extra_start + info.size_file_extra as usize;
        let end = comment_start + info.size_file_comment as usize;
        if end > data.len() {
            return UNZ_ERRNO;
        }
        if info.uncompressed_size == 0xffff_ffff
            || info.compressed_size == 0xffff_ffff
            || offset_curfile == 0xffff_ffff
        {
            let Some(extra) = zip64_extra(&data[extra_start..comment_start]) else {
                return UNZ_BADZIPFILE;
            };
            let mut at = 0;
            for field in [
                &mut info.uncompressed_size,
                &mut info.compressed_size,
                &mut offset_curfile,
            ] {
                if *field == 0xffff_ffff {
                    let Some(value) = read_u64(extra, at) else {
                        return UNZ_BADZIPFILE;
                    };
                    *field = value;
                    at += 8;
                }
            }
        }
        self.current = CurrentFile {
            info,
            offset_curfile,
            name: data[name_start..extra_start].to_vec(),
            extra: data[extra_start..comment_start].to_vec(),
            comment: data[comment_start..end].to_vec(),
        };
        UNZ_OK
    }

    /// `unzGoToFirstFile`.
    fn go_to_first_file(&mut self, data: &[u8]) -> i32 {
        self.pos_in_central_dir = self.offset_central_dir;
        self.num_file = 0;
        let error = self.read_current_info(data);
        self.current_file_ok = error == UNZ_OK;
        error
    }

    /// `unzGoToNextFile`.
    fn go_to_next_file(&mut self, data: &[u8]) -> i32 {
        if !self.current_file_ok {
            return UNZ_END_OF_LIST_OF_FILE;
        }
        // 0xffff entries may mean "more than fits": then walk until the
        // signature check fails.
        if self.global.number_entry != 0xffff && self.num_file + 1 == self.global.number_entry {
            return UNZ_END_OF_LIST_OF_FILE;
        }
        let info = &self.current.info;
        self.pos_in_central_dir += SIZE_CENTRAL_HEADER as u64
            + info.size_filename
            + info.size_file_extra
            + info.size_file_comment;
        self.num_file += 1;
        let error = self.read_current_info(data);
        self.current_file_ok = error == UNZ_OK;
        error
    }

    /// `unzLocateFile`.
    fn locate_file(&mut self, data: &[u8], name: &[u8], case_sensitivity: i32) -> i32 {
        if name.len() >= MAX_FILE_NAME_IN_ZIP {
            return UNZ_PARAMERROR;
        }
        if !self.current_file_ok {
            return UNZ_END_OF_LIST_OF_FILE;
        }
        let saved = (self.num_file, self.pos_in_central_dir, self.current.clone());
        let mut error = self.go_to_first_file(data);
        while error == UNZ_OK {
            // The name is read into a buffer of `UNZ_MAXFILENAMEINZIP` bytes.
            let current = &self.current.name;
            let current = &current[..current.len().min(MAX_FILE_NAME_IN_ZIP)];
            if file_names_equal(current, name, case_sensitivity) {
                return UNZ_OK;
            }
            error = self.go_to_next_file(data);
        }
        // Not found: restore the current file.
        self.num_file = saved.0;
        self.pos_in_central_dir = saved.1;
        self.current = saved.2;
        self.current_file_ok = true;
        error
    }

    /// `unzOpenCurrentFile` (no password, not raw).
    fn open_current_file(&mut self, data: &[u8]) -> i32 {
        if !self.current_file_ok {
            return UNZ_PARAMERROR;
        }
        if self.opened.is_some() {
            let _ = self.close_current_file();
        }
        let info = self.current.info;
        // `unz64local_CheckCurrentFileCoherencyHeader`.
        let Some(base) = self
            .current
            .offset_curfile
            .checked_add(self.byte_before_the_zipfile)
            .and_then(|v| usize::try_from(v).ok())
            .filter(|&v| v <= data.len())
        else {
            return UNZ_BADZIPFILE;
        };
        let Some(header) = data.get(base..base + SIZE_LOCAL_HEADER) else {
            return UNZ_BADZIPFILE;
        };
        let u16_at = |offset| read_u16(header, offset).expect("inside the header");
        let u32_at = |offset| read_u32(header, offset).expect("inside the header");
        let flags = u16_at(6);
        let sizes_deferred = flags & 8 != 0;
        let mut compressed = u32_at(18);
        let mut uncompressed = u32_at(22);
        if compressed == 0xffff_ffff || uncompressed == 0xffff_ffff {
            let start = base + SIZE_LOCAL_HEADER + u16_at(26) as usize;
            let Some(extra) = data
                .get(start..start + u16_at(28) as usize)
                .and_then(zip64_extra)
            else {
                return UNZ_BADZIPFILE;
            };
            let mut at = 0;
            for field in [&mut uncompressed, &mut compressed] {
                if *field == 0xffff_ffff {
                    let Some(value) = read_u64(extra, at) else {
                        return UNZ_BADZIPFILE;
                    };
                    *field = value;
                    at += 8;
                }
            }
        }
        let coherent = read_u32(header, 0) == Some(u64::from(LOCAL_HEADER_MAGIC))
            && u16_at(8) == info.compression_method
            && (info.compression_method == 0 || info.compression_method == Z_DEFLATED as u64)
            && (sizes_deferred || u32_at(14) == info.crc)
            && (sizes_deferred || compressed == info.compressed_size)
            && (sizes_deferred || uncompressed == info.uncompressed_size)
            && u16_at(26) == info.size_filename;
        if !coherent {
            return UNZ_BADZIPFILE;
        }
        let data_start = base + SIZE_LOCAL_HEADER + u16_at(26) as usize + u16_at(28) as usize;
        self.opened = Some(OpenedFile {
            data_start,
            rest_read_compressed: info.compressed_size,
            rest_read_uncompressed: info.uncompressed_size,
            deflated: info.compression_method == Z_DEFLATED as u64,
            inflate: Decompress::new(false),
            crc: Crc::new(),
            crc_wait: info.crc as u32,
        });
        UNZ_OK
    }

    /// `unzReadCurrentFile`: bytes read, 0 at the end, or a negative error.
    fn read_current_file(&mut self, data: &[u8], buffer: &mut [u8]) -> i32 {
        let Some(opened) = self.opened.as_mut() else {
            return UNZ_PARAMERROR;
        };
        if buffer.is_empty() {
            return 0;
        }
        let wanted = (buffer.len() as u64).min(opened.rest_read_uncompressed) as usize;
        let buffer = &mut buffer[..wanted];
        let mut read = 0usize;
        while read < buffer.len() {
            let consumed = opened.inflate.total_in() as usize;
            let input_start = opened.data_start + if opened.deflated { consumed } else { 0 };
            let Ok(available) = usize::try_from(opened.rest_read_compressed) else {
                return UNZ_ERRNO;
            };
            if available == 0 && !opened.deflated {
                break;
            }
            let Some(input) = input_start
                .checked_add(available)
                .and_then(|end| data.get(input_start..end))
            else {
                return UNZ_ERRNO;
            };
            if !opened.deflated {
                let count = (buffer.len() - read).min(input.len());
                buffer[read..read + count].copy_from_slice(&input[..count]);
                opened.data_start += count;
                opened.rest_read_compressed -= count as u64;
                opened.crc.update(&buffer[read..read + count]);
                opened.rest_read_uncompressed -= count as u64;
                read += count;
                continue;
            }
            let before_in = opened.inflate.total_in();
            let before_out = opened.inflate.total_out();
            let status =
                opened
                    .inflate
                    .decompress(input, &mut buffer[read..], FlushDecompress::Sync);
            let used_in = opened.inflate.total_in() - before_in;
            let produced = (opened.inflate.total_out() - before_out) as usize;
            opened.rest_read_compressed -= used_in;
            opened.crc.update(&buffer[read..read + produced]);
            opened.rest_read_uncompressed -= produced as u64;
            read += produced;
            match status {
                Ok(Status::StreamEnd) => break,
                Ok(_) if used_in == 0 && produced == 0 => break,
                Ok(_) => {}
                Err(_) => return Z_DATA_ERROR,
            }
        }
        read as i32
    }

    /// `unzCloseCurrentFile`.
    fn close_current_file(&mut self) -> i32 {
        let Some(opened) = self.opened.take() else {
            return UNZ_PARAMERROR;
        };
        if opened.rest_read_uncompressed == 0 && opened.crc.sum() != opened.crc_wait {
            return UNZ_CRCERROR;
        }
        UNZ_OK
    }
}

/// `zlib::FileToRead`: a zip archive in memory.
///
/// Every method first checks [`error`](Self::error); after the first failure
/// all calls return that error until [`clear_error`](Self::clear_error).
pub struct FileToRead {
    data: Vec<u8>,
    handle: Option<Unz>,
    error: i32,
}

impl FileToRead {
    pub fn new(content: impl Into<Vec<u8>>) -> Self {
        let data = content.into();
        let handle = Unz::open(&data);
        let error = if handle.is_some() { UNZ_OK } else { -1 };
        Self {
            data,
            handle,
            error,
        }
    }

    fn call(&mut self, f: impl FnOnce(&mut Unz, &[u8]) -> i32) -> i32 {
        if self.error() == UNZ_OK {
            self.error = match self.handle.as_mut() {
                Some(handle) => f(handle, &self.data),
                None => -1,
            };
        }
        self.error()
    }

    /// `getGlobalInfo`.
    pub fn get_global_info(&mut self, info: &mut UnzGlobalInfo) -> i32 {
        self.call(|handle, _| {
            *info = handle.global;
            UNZ_OK
        })
    }

    /// `locateFile`.
    pub fn locate_file(&mut self, file_name: &str, case_sensitivity: i32) -> i32 {
        self.call(|handle, data| handle.locate_file(data, file_name.as_bytes(), case_sensitivity))
    }

    /// `goToFirstFile`.
    pub fn go_to_first_file(&mut self) -> i32 {
        self.call(|handle, data| handle.go_to_first_file(data))
    }

    /// `goToNextFile`; `UNZ_END_OF_LIST_OF_FILE` after the last entry.
    pub fn go_to_next_file(&mut self) -> i32 {
        self.call(|handle, data| handle.go_to_next_file(data))
    }

    /// `getCurrentFileInfo`: fills `info` and copies as much of the name,
    /// extra field and comment as fits into the buffers; a NUL is appended
    /// when the name is shorter than its buffer (as minizip does).
    pub fn get_current_file_info(
        &mut self,
        info: Option<&mut UnzFileInfo>,
        file_name: Option<&mut [u8]>,
        extra_field: Option<&mut [u8]>,
        comment: Option<&mut [u8]>,
    ) -> i32 {
        self.call(|handle, _| {
            if !handle.current_file_ok {
                return UNZ_PARAMERROR;
            }
            let current = &handle.current;
            if let Some(info) = info {
                *info = current.info;
            }
            if let Some(buffer) = file_name {
                let count = current.name.len().min(buffer.len());
                buffer[..count].copy_from_slice(&current.name[..count]);
                if current.name.len() < buffer.len() {
                    buffer[current.name.len()] = 0;
                }
            }
            for (source, target) in [(&current.extra, extra_field), (&current.comment, comment)] {
                if let Some(buffer) = target {
                    let count = source.len().min(buffer.len());
                    buffer[..count].copy_from_slice(&source[..count]);
                }
            }
            UNZ_OK
        })
    }

    /// `getCurrentFileName`: at most 128 bytes of the name, up to the first
    /// NUL, decoded as UTF-8 (lossy); empty on error.
    pub fn get_current_file_name(&mut self) -> String {
        const MAX_NAME: usize = 128;
        let mut name = [0u8; MAX_NAME + 1];
        let result = self.get_current_file_info(None, Some(&mut name[..MAX_NAME]), None, None);
        if result == UNZ_OK {
            String::from_utf8_lossy(c_str(&name)).into_owned()
        } else {
            String::new()
        }
    }

    /// `openCurrentFile`.
    pub fn open_current_file(&mut self) -> i32 {
        self.call(|handle, data| handle.open_current_file(data))
    }

    /// `readCurrentFile`: number of bytes read, or the (negative) error.
    pub fn read_current_file(&mut self, buffer: &mut [u8]) -> i32 {
        if self.error() == UNZ_OK {
            let result = match self.handle.as_mut() {
                Some(handle) => handle.read_current_file(&self.data, buffer),
                None => -1,
            };
            if result >= 0 {
                return result;
            }
            self.error = result;
        }
        self.error()
    }

    /// `closeCurrentFile`: `UNZ_CRCERROR` if the whole file was read and the
    /// checksum differs.
    pub fn close_current_file(&mut self) -> i32 {
        self.call(|handle, _| handle.close_current_file())
    }

    /// `readCurrentFileContent(fileSizeLimit)`: the whole current file, or
    /// empty on any error (the original also logs the reason).
    pub fn read_current_file_content(&mut self, file_size_limit: i32) -> Vec<u8> {
        let mut info = UnzFileInfo::default();
        if self.get_current_file_info(Some(&mut info), None, None, None) != UNZ_OK {
            return Vec::new();
        }
        let size = info.uncompressed_size;
        if size > u64::from(file_size_limit as u32) {
            if self.error == UNZ_OK {
                self.error = -1;
            }
            return Vec::new();
        }
        if self.open_current_file() != UNZ_OK {
            return Vec::new();
        }
        let mut result = vec![0u8; size as usize];
        let could_read = self.read_current_file(&mut result);
        if could_read != size as i32 {
            return Vec::new();
        }
        if self.close_current_file() != UNZ_OK {
            return Vec::new();
        }
        result
    }

    /// `readFileContent(name, caseSensitivity, fileSizeLimit)`.
    pub fn read_file_content(
        &mut self,
        file_name: &str,
        case_sensitivity: i32,
        file_size_limit: i32,
    ) -> Vec<u8> {
        if self.locate_file(file_name, case_sensitivity) != UNZ_OK {
            return Vec::new();
        }
        self.read_current_file_content(file_size_limit)
    }

    /// `close`.
    pub fn close(&mut self) {
        if let Some(mut handle) = self.handle.take() {
            if handle.opened.is_some() {
                let _ = handle.close_current_file();
            }
        }
    }

    /// `error`.
    #[must_use]
    pub fn error(&self) -> i32 {
        self.error
    }

    /// `clearError`.
    pub fn clear_error(&mut self) {
        self.error = UNZ_OK;
    }
}

/// zlib `detect_data_type` approximated over the whole input instead of the
/// literals of the first deflate block: binary if any "block-listed"
/// control byte occurs, text if a tab, line feed, carriage return or a byte
/// >= 32 occurs.
fn is_text(statistics: &[bool; 256]) -> bool {
    let mut block_mask = 0xf3ff_c07fu32;
    for &seen in &statistics[..32] {
        if block_mask & 1 != 0 && seen {
            return false;
        }
        block_mask >>= 1;
    }
    statistics[9] || statistics[10] || statistics[13] || statistics[32..].iter().any(|&s| s)
}

struct OpenedEntry {
    central_header: Vec<u8>,
    pos_local_header: usize,
    deflate: Option<Compress>,
    crc: Crc,
    total_uncompressed: u64,
    total_compressed: u64,
    byte_seen: [bool; 256],
}

/// The `zipFile` handle.
#[derive(Default)]
struct Zip {
    central_dir: Vec<u8>,
    number_entry: u64,
    opened: Option<OpenedEntry>,
}

impl Zip {
    #[allow(clippy::too_many_arguments)]
    fn open_new_file(
        &mut self,
        data: &mut Vec<u8>,
        filename: &str,
        zipfi: Option<&ZipFileInfo>,
        extrafield_local: &[u8],
        extrafield_global: &[u8],
        comment: Option<&str>,
        method: i32,
        level: i32,
    ) -> i32 {
        if method != 0 && method != Z_DEFLATED {
            return ZIP_PARAMERROR;
        }
        if self.opened.is_some() {
            let error = self.close_file_in_zip(data);
            if error != ZIP_OK {
                return error;
            }
        }
        let filename = c_str(filename.as_bytes());
        let filename: &[u8] = if filename.is_empty() { b"-" } else { filename };
        let comment = comment.map_or(&[][..], |c| c_str(c.as_bytes()));
        if filename.len() > 0xffff
            || extrafield_local.len() > 0xffff
            || extrafield_global.len() > 0xffff
            || comment.len() > 0xffff
        {
            return ZIP_PARAMERROR;
        }
        let dos_date = match zipfi {
            None => 0,
            Some(zipfi) if zipfi.dos_date != 0 => zipfi.dos_date,
            Some(zipfi) => tmz_date_to_dos_date(&zipfi.tmz_date),
        } as u32;
        let mut flag = 0u16;
        match level {
            8 | 9 => flag |= 2,
            2 => flag |= 4,
            1 => flag |= 6,
            _ => {}
        }
        let deflate = if method == Z_DEFLATED {
            let compression = match level {
                Z_DEFAULT_COMPRESSION => Compression::default(),
                0..=9 => Compression::new(level as u32),
                _ => return Z_STREAM_ERROR,
            };
            Some(Compress::new(compression, false))
        } else {
            None
        };
        let pos_local_header = data.len();
        if pos_local_header >= 0xffff_ffff {
            return ZIP_INTERNALERROR;
        }

        let mut central = Vec::with_capacity(SIZE_CENTRAL_HEADER + filename.len());
        central.extend_from_slice(&CENTRAL_HEADER_MAGIC.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&flag.to_le_bytes());
        central.extend_from_slice(&(method as u16).to_le_bytes());
        central.extend_from_slice(&dos_date.to_le_bytes());
        central.extend_from_slice(&[0u8; 12]); // crc, compressed, uncompressed
        central.extend_from_slice(&(filename.len() as u16).to_le_bytes());
        central.extend_from_slice(&(extrafield_global.len() as u16).to_le_bytes());
        central.extend_from_slice(&(comment.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // disk number start
        let (internal_fa, external_fa) = zipfi.map_or((0, 0), |z| (z.internal_fa, z.external_fa));
        central.extend_from_slice(&(internal_fa as u16).to_le_bytes());
        central.extend_from_slice(&(external_fa as u32).to_le_bytes());
        central.extend_from_slice(&(pos_local_header as u32).to_le_bytes());
        central.extend_from_slice(filename);
        central.extend_from_slice(extrafield_global);
        central.extend_from_slice(comment);

        // `Write_LocalFileHeader`.
        data.extend_from_slice(&LOCAL_HEADER_MAGIC.to_le_bytes());
        data.extend_from_slice(&20u16.to_le_bytes());
        data.extend_from_slice(&flag.to_le_bytes());
        data.extend_from_slice(&(method as u16).to_le_bytes());
        data.extend_from_slice(&dos_date.to_le_bytes());
        data.extend_from_slice(&[0u8; 12]); // filled in by close
        data.extend_from_slice(&(filename.len() as u16).to_le_bytes());
        data.extend_from_slice(&(extrafield_local.len() as u16).to_le_bytes());
        data.extend_from_slice(filename);
        data.extend_from_slice(extrafield_local);

        self.opened = Some(OpenedEntry {
            central_header: central,
            pos_local_header,
            deflate,
            crc: Crc::new(),
            total_uncompressed: 0,
            total_compressed: 0,
            byte_seen: [false; 256],
        });
        ZIP_OK
    }

    fn run_deflate(
        deflate: &mut Compress,
        data: &mut Vec<u8>,
        mut input: &[u8],
        flush: FlushCompress,
    ) -> Result<u64, ()> {
        let mut produced = 0u64;
        loop {
            data.reserve(input.len() / 2 + 4096);
            let before_in = deflate.total_in();
            let before_out = deflate.total_out();
            let status = deflate.compress_vec(input, data, flush).map_err(|_| ())?;
            let used = (deflate.total_in() - before_in) as usize;
            produced += deflate.total_out() - before_out;
            input = &input[used..];
            let finished = match flush {
                FlushCompress::Finish => status == Status::StreamEnd,
                _ => input.is_empty() && data.len() < data.capacity(),
            };
            if finished {
                return Ok(produced);
            }
        }
    }

    fn write_in_file(&mut self, data: &mut Vec<u8>, buffer: &[u8]) -> i32 {
        let Some(opened) = self.opened.as_mut() else {
            return ZIP_PARAMERROR;
        };
        opened.crc.update(buffer);
        opened.total_uncompressed += buffer.len() as u64;
        for &byte in buffer {
            opened.byte_seen[usize::from(byte)] = true;
        }
        match opened.deflate.as_mut() {
            Some(deflate) => match Self::run_deflate(deflate, data, buffer, FlushCompress::None) {
                Ok(produced) => opened.total_compressed += produced,
                Err(()) => return ZIP_ERRNO,
            },
            None => {
                data.extend_from_slice(buffer);
                opened.total_compressed += buffer.len() as u64;
            }
        }
        ZIP_OK
    }

    /// `zipCloseFileInZip`.
    fn close_file_in_zip(&mut self, data: &mut Vec<u8>) -> i32 {
        let Some(mut opened) = self.opened.take() else {
            return ZIP_PARAMERROR;
        };
        let mut text = false;
        if let Some(deflate) = opened.deflate.as_mut() {
            match Self::run_deflate(deflate, data, &[], FlushCompress::Finish) {
                Ok(produced) => opened.total_compressed += produced,
                Err(()) => return ZIP_ERRNO,
            }
            // Level 0 emits stored blocks, which never set `data_type`.
            text = deflate.total_in() > 0 && is_text(&opened.byte_seen);
            text = text && opened.central_header[10] == Z_DEFLATED as u8 && {
                // A stored-only deflate stream starts with a stored block
                // header byte (BFINAL + BTYPE 00).
                let first = data.get(
                    opened.pos_local_header + SIZE_LOCAL_HEADER + {
                        let header = &opened.central_header;
                        usize::from(u16::from_le_bytes([header[28], header[29]]))
                            + local_extra_len(data, opened.pos_local_header)
                    },
                );
                !matches!(first, Some(b) if b & 0b110 == 0)
            };
        }
        if opened.total_compressed >= 0xffff_ffff || opened.total_uncompressed >= 0xffff_ffff {
            return ZIP_INTERNALERROR;
        }
        let crc = opened.crc.sum();
        let compressed = opened.total_compressed as u32;
        let uncompressed = opened.total_uncompressed as u32;
        let header = &mut opened.central_header;
        header[16..20].copy_from_slice(&crc.to_le_bytes());
        header[20..24].copy_from_slice(&compressed.to_le_bytes());
        if text {
            header[36..38].copy_from_slice(&1u16.to_le_bytes()); // Z_ASCII
        }
        header[24..28].copy_from_slice(&uncompressed.to_le_bytes());
        self.central_dir.extend_from_slice(header);

        let local = opened.pos_local_header + 14;
        data[local..local + 4].copy_from_slice(&crc.to_le_bytes());
        data[local + 4..local + 8].copy_from_slice(&compressed.to_le_bytes());
        data[local + 8..local + 12].copy_from_slice(&uncompressed.to_le_bytes());

        self.number_entry += 1;
        ZIP_OK
    }

    /// `zipClose(file, NULL)`.
    fn close(&mut self, data: &mut Vec<u8>) -> i32 {
        let mut error = ZIP_OK;
        if self.opened.is_some() {
            error = self.close_file_in_zip(data);
        }
        let centraldir_pos = data.len();
        if self.number_entry >= 0xffff || centraldir_pos >= 0xffff_ffff {
            // minizip switches to ZIP64 here, which the port does not write.
            return ZIP_INTERNALERROR;
        }
        if error == ZIP_OK {
            data.extend_from_slice(&self.central_dir);
        }
        let size_centraldir = self.central_dir.len() as u32;
        if error == ZIP_OK {
            data.extend_from_slice(&END_HEADER_MAGIC.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&(self.number_entry as u16).to_le_bytes());
            data.extend_from_slice(&(self.number_entry as u16).to_le_bytes());
            data.extend_from_slice(&size_centraldir.to_le_bytes());
            data.extend_from_slice(&(centraldir_pos as u32).to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes()); // no global comment
        }
        error
    }
}

fn local_extra_len(data: &[u8], pos_local_header: usize) -> usize {
    read_u16(data, pos_local_header + 28).unwrap_or(0) as usize
}

/// `zlib::FileToWrite`: builds a zip archive in memory.
pub struct FileToWrite {
    data: Vec<u8>,
    handle: Option<Zip>,
    error: i32,
}

impl Default for FileToWrite {
    fn default() -> Self {
        Self::new()
    }
}

impl FileToWrite {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            handle: Some(Zip::default()),
            error: ZIP_OK,
        }
    }

    /// `openNewFile(filename, zipfi, extrafield_local, ..., method, level)`.
    #[allow(clippy::too_many_arguments)]
    pub fn open_new_file(
        &mut self,
        filename: &str,
        zipfi: Option<&ZipFileInfo>,
        extrafield_local: &[u8],
        extrafield_global: &[u8],
        comment: Option<&str>,
        method: i32,
        level: i32,
    ) -> i32 {
        if self.error() == ZIP_OK {
            self.error = match self.handle.as_mut() {
                Some(handle) => handle.open_new_file(
                    &mut self.data,
                    filename,
                    zipfi,
                    extrafield_local,
                    extrafield_global,
                    comment,
                    method,
                    level,
                ),
                None => -1,
            };
        }
        self.error()
    }

    /// `writeInFile`.
    pub fn write_in_file(&mut self, buffer: &[u8]) -> i32 {
        if self.error() == ZIP_OK {
            self.error = match self.handle.as_mut() {
                Some(handle) => handle.write_in_file(&mut self.data, buffer),
                None => -1,
            };
        }
        self.error()
    }

    /// `closeFile`.
    pub fn close_file(&mut self) -> i32 {
        if self.error() == ZIP_OK {
            self.error = match self.handle.as_mut() {
                Some(handle) => handle.close_file_in_zip(&mut self.data),
                None => -1,
            };
        }
        self.error()
    }

    /// `close`: writes the central directory.
    pub fn close(&mut self) {
        if let Some(mut handle) = self.handle.take() {
            if handle.close(&mut self.data) != ZIP_OK && self.error == ZIP_OK {
                self.error = -1;
            }
        }
    }

    /// `error`.
    #[must_use]
    pub fn error(&self) -> i32 {
        self.error
    }

    /// `result`: the archive bytes (complete after [`close`](Self::close)).
    #[must_use]
    pub fn result(&self) -> Vec<u8> {
        self.data.clone()
    }
}

impl Drop for FileToWrite {
    fn drop(&mut self) {
        self.close();
    }
}

/// Gzip stream used by MTProto `gzip_packed`, with a caller-provided expansion
/// budget. Uses the same already-selected miniz_oxide backend as ZIP entries.
pub fn inflate_gzip(input: &[u8], limit: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut decoder = flate2::read::GzDecoder::new(input);
    let mut output = Vec::new();
    decoder
        .by_ref()
        .take((limit as u64).saturating_add(1))
        .read_to_end(&mut output)?;
    if output.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "gzip expansion limit exceeded",
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_dates() {
        assert_eq!(tmz_date_to_dos_date(&TmZip::default()), 0x0020_0000);
        let tm = TmZip {
            tm_sec: 30,
            tm_min: 45,
            tm_hour: 13,
            tm_mday: 11,
            tm_mon: 10,
            tm_year: 2026,
        };
        let dos = tmz_date_to_dos_date(&tm);
        assert_eq!(
            dos,
            ((11 + 32 * 11 + 512 * 46) << 16) | (15 + 32 * 45 + 2048 * 13)
        );
        assert_eq!(dos_date_to_tmu_date(dos), tm);
    }

    #[test]
    fn name_comparison() {
        assert!(file_names_equal(
            b"Colors.TDesktop-Theme",
            b"colors.tdesktop-theme",
            2
        ));
        assert!(!file_names_equal(
            b"Colors.TDesktop-Theme",
            b"colors.tdesktop-theme",
            1
        ));
        // Only ASCII is folded.
        assert!(!file_names_equal("Ж".as_bytes(), "ж".as_bytes(), 2));
        assert!(file_names_equal(b"a\0b", b"a", 1));
    }

    #[test]
    fn text_detection() {
        let mut seen = [false; 256];
        seen[usize::from(b'a')] = true;
        assert!(is_text(&seen));
        seen[0] = true;
        assert!(!is_text(&seen));
        let mut gray = [false; 256];
        gray[27] = true; // ESC is "gray-listed"
        assert!(!is_text(&gray));
    }
}
