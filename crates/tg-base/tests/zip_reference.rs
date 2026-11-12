use std::io::Read;
use tg_base::zlib_help::*;

const TEXT: &[u8] = include_bytes!("fixtures/theme.txt");

#[test]
fn inflate_reference_zlib_stream() {
    let mut output = Vec::new();
    flate2::read::ZlibDecoder::new(&include_bytes!("fixtures/theme.zlib")[..])
        .read_to_end(&mut output)
        .unwrap();
    assert_eq!(output, TEXT);
}

#[test]
fn read_reference_zip_stored_and_deflated() {
    for fixture in [
        &include_bytes!("fixtures/stored.zip")[..],
        &include_bytes!("fixtures/deflated.zip")[..],
        &include_bytes!("fixtures/zip64.zip")[..],
    ] {
        let mut archive = FileToRead::new(fixture);
        assert_eq!(archive.error(), UNZ_OK);
        let mut info = UnzGlobalInfo::default();
        assert_eq!(archive.get_global_info(&mut info), UNZ_OK);
        assert_eq!(info.number_entry, 2);
        assert_eq!(archive.get_current_file_name(), "colors.tdesktop-theme");
        assert_eq!(
            archive.read_file_content("COLORS.TDESKTOP-THEME", CASE_INSENSITIVE, 4096),
            TEXT
        );
        assert_eq!(archive.go_to_next_file(), UNZ_OK);
        assert_eq!(archive.get_current_file_name(), "empty");
        assert_eq!(archive.read_current_file_content(1), b"");
        assert_eq!(archive.error(), UNZ_OK);
        assert_eq!(archive.go_to_next_file(), UNZ_END_OF_LIST_OF_FILE);
        // Errors are sticky, including end-of-directory.
        assert_eq!(archive.go_to_first_file(), UNZ_END_OF_LIST_OF_FILE);
        archive.clear_error();
        assert_eq!(archive.go_to_first_file(), UNZ_OK);
    }
}

#[test]
fn limits_crc_and_invalid_archives() {
    let fixture = include_bytes!("fixtures/stored.zip");
    let mut archive = FileToRead::new(&fixture[..]);
    assert!(archive.read_current_file_content(1).is_empty());
    assert_ne!(archive.error(), UNZ_OK);
    let mut corrupt = fixture.to_vec();
    let data_start = 30 + "colors.tdesktop-theme".len();
    corrupt[data_start] ^= 1;
    let mut archive = FileToRead::new(corrupt);
    assert!(archive.read_current_file_content(4096).is_empty());
    assert_eq!(archive.error(), UNZ_CRCERROR);
    assert_ne!(FileToRead::new(Vec::new()).error(), UNZ_OK);
    assert_ne!(FileToRead::new(b"invalid".to_vec()).error(), UNZ_OK);
}

#[test]
fn write_zip_headers_and_content() {
    for method in [0, Z_DEFLATED] {
        let mut archive = FileToWrite::new();
        assert_eq!(
            archive.open_new_file("a", None, &[], &[], None, method, 6),
            ZIP_OK
        );
        assert_eq!(archive.write_in_file(b"hello"), ZIP_OK);
        assert_eq!(archive.close_file(), ZIP_OK);
        archive.close();
        assert_eq!(archive.error(), ZIP_OK);
        let bytes = archive.result();
        // Local header: fixed signature/version/flags/date and independent CRC32.
        assert_eq!(&bytes[..8], &[0x50, 0x4b, 3, 4, 20, 0, 0, 0]);
        assert_eq!(&bytes[14..18], &[0x86, 0xa6, 0x10, 0x36]);
        assert_eq!(&bytes[22..26], &[5, 0, 0, 0]);
        if method == 0 {
            assert_eq!(&bytes[31..36], b"hello");
            assert_eq!(bytes, include_bytes!("fixtures/minizip-stored.zip"));
        }
        let mut reader = FileToRead::new(bytes);
        assert_eq!(reader.read_file_content("a", CASE_SENSITIVE, 5), b"hello");
        assert_eq!(reader.error(), UNZ_OK);
    }
}

#[test]
fn read_minizip_reference() {
    for fixture in [
        &include_bytes!("fixtures/minizip-stored.zip")[..],
        &include_bytes!("fixtures/minizip-deflated.zip")[..],
    ] {
        let mut reader = FileToRead::new(fixture);
        assert_eq!(reader.read_file_content("a", CASE_SENSITIVE, 5), b"hello");
        assert_eq!(reader.error(), UNZ_OK);
    }
}
