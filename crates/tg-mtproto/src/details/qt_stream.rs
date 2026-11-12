//! QDataStream Qt_5_1 persistence: BE integers, byte-counted UTF-16 BE QString.
#[derive(Default)]
pub struct Writer(pub Vec<u8>);
impl Writer {
    pub fn i32(&mut self, v: i32) {
        self.0.extend(v.to_be_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.0.extend(v.to_be_bytes());
    }
    pub fn bytes(&mut self, v: &[u8]) {
        self.i32(v.len() as i32);
        self.0.extend_from_slice(v);
    }
    pub fn string(&mut self, v: &str) {
        self.i32((v.encode_utf16().count() * 2) as i32);
        for c in v.encode_utf16() {
            self.0.extend(c.to_be_bytes());
        }
    }
}
pub struct Reader<'a> {
    pub data: &'a [u8],
    pub position: usize,
}
impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }
    pub fn raw(&mut self, size: usize) -> Option<&'a [u8]> {
        let end = self.position.checked_add(size)?;
        let value = self.data.get(self.position..end)?;
        self.position = end;
        Some(value)
    }
    pub fn i32(&mut self) -> Option<i32> {
        Some(i32::from_be_bytes(self.raw(4)?.try_into().ok()?))
    }
    pub fn u64(&mut self) -> Option<u64> {
        Some(u64::from_be_bytes(self.raw(8)?.try_into().ok()?))
    }
    pub fn bytes(&mut self) -> Option<Vec<u8>> {
        let len = self.i32()?;
        if len == -1 {
            return Some(Vec::new());
        }
        self.raw(usize::try_from(len).ok()?).map(<[u8]>::to_vec)
    }
    pub fn string(&mut self) -> Option<String> {
        let bytes = self.bytes()?;
        if !bytes.len().is_multiple_of(2) {
            return None;
        }
        let utf16: Vec<_> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect();
        String::from_utf16(&utf16).ok()
    }
    pub fn at_end(&self) -> bool {
        self.position == self.data.len()
    }
}
