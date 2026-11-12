//! Safe, owned formatting part of crash_report_header.{h,cpp}.
//! This buffer is NOT an async-signal-safe replacement for the original global
//! ReportHeaderBytes pointer. Installing fatal signal handlers remains deferred.
pub const REPORT_HEADER_SIZE_LIMIT: usize = 64 * 1024;
pub struct ReportHeader {
    bytes: Vec<u8>,
}
impl Default for ReportHeader {
    fn default() -> Self {
        Self {
            bytes: Vec::with_capacity(REPORT_HEADER_SIZE_LIMIT),
        }
    }
}
impl ReportHeader {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn length(&self) -> usize {
        self.bytes.len()
    }
    pub fn write_bytes(&mut self, bytes: &[u8]) -> &mut Self {
        let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
        let count = end.min(REPORT_HEADER_SIZE_LIMIT - self.bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        self
    }
    pub fn write_wide(&mut self, text: &[i32]) -> &mut Self {
        for &c in text.iter().take_while(|&&c| c != 0) {
            self.write_bytes(&[if (0..128).contains(&c) { c as u8 } else { b'?' }]);
        }
        self
    }
    pub fn write_signed(&mut self, value: i64) -> &mut Self {
        self.write_bytes(value.to_string().as_bytes())
    }
    pub fn write_unsigned(&mut self, value: u64) -> &mut Self {
        self.write_bytes(value.to_string().as_bytes())
    }
    pub fn write_double(&mut self, mut value: f64) -> &mut Self {
        assert!(
            value.is_finite() && value.abs() < u64::MAX as f64,
            "nonrepresentable report number"
        );
        if value < 0.0 {
            self.write_bytes(b"-");
            value = -value;
        }
        self.write_unsigned(value.floor() as u64).write_bytes(b".");
        value -= value.floor();
        for _ in 0..4 {
            value *= 10.0;
            let digit = value.floor();
            self.write_bytes(&[b'0' + digit as u8]);
            value -= digit;
        }
        self
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_sample() {
        let mut header = ReportHeader::new();
        header
            .write_bytes(b"v=")
            .write_signed(-42)
            .write_bytes(b" ")
            .write_unsigned(u64::MAX)
            .write_bytes(b" ")
            .write_double(12.125)
            .write_bytes(b" ")
            .write_wide(&[65, 0x416, -1, 0]);
        assert_eq!(header.bytes(), b"v=-42 18446744073709551615 12.1250 A??");
    }
    #[test]
    fn nul_and_size_limit() {
        let mut header = ReportHeader::new();
        header.write_bytes(b"a\0ignored");
        header.write_bytes(&vec![b'x'; REPORT_HEADER_SIZE_LIMIT]);
        header.write_bytes(b"ignored");
        assert_eq!(header.length(), REPORT_HEADER_SIZE_LIMIT);
        assert_eq!(header.bytes()[0], b'a');
    }
}
