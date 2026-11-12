//! Completes tg-scheme DumpCore for the original primitives, gzip and layers.
use crate::core_types::LAYERS;
use tg_scheme::dump_to_text::{add_spaces, dump_to_text_type, DumpCore};
use tg_tl::{Deserialize, Reader};
pub struct Core;
fn text<T: Deserialize>(from: &mut Reader<'_>, write: impl FnOnce(T) -> String) -> Option<String> {
    T::read(from).ok().map(write)
}
impl DumpCore for Core {
    fn dump_core(
        &self,
        to: &mut String,
        from: &mut Reader<'_>,
        cons: u32,
        level: u32,
        vcons: u32,
    ) -> bool {
        use tg_tl::*;
        let result = match cons {
            ID_INT => text::<i32>(from, |v| format!("{v} [INT]")),
            ID_LONG => text::<u64>(from, |v| format!("{v} [LONG]")),
            ID_INT128 => text::<Int128>(from, |v| format!("{} * 2^64 + {} [INT128]", v.h, v.l)),
            ID_INT256 => text::<Int256>(from, |v| {
                format!(
                    "{} * 2^192 + {} * 2^128 + {} * 2 ^ 64 + {} [INT256]",
                    v.h.h, v.h.l, v.l.h, v.l.l
                )
            }),
            ID_DOUBLE => text::<f64>(from, qt_double),
            ID_STRING => text::<Vec<u8>>(from, |v| match String::from_utf8(v) {
                Ok(s) => format!(
                    "\"{}\" [STRING]",
                    s.replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                ),
                Err(e) => {
                    let bytes = e.into_bytes();
                    let limit = if bytes.len() < 64 { bytes.len() } else { 16 };
                    let hex = bytes[..limit]
                        .iter()
                        .map(|v| format!("{v:02X}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!(
                        "{hex}{} [{} BYTES]",
                        if limit < bytes.len() { "..." } else { "" },
                        bytes.len()
                    )
                }
            }),
            ID_VECTOR => {
                let Ok(count) = i32::read(from) else {
                    return false;
                };
                to.push_str(&format!("[ vector<0x{vcons:x}> ({count})"));
                if count != 0 {
                    to.push('\n');
                    add_spaces(to, level as i32);
                    for _ in 0..count {
                        to.push_str("  ");
                        if !dump_to_text_type(to, from, vcons, level + 1, 0, self) {
                            return false;
                        }
                        to.push_str(",\n");
                        add_spaces(to, level as i32);
                    }
                } else {
                    to.push(' ');
                }
                to.push(']');
                return true;
            }
            crate::core_types::GZIP_PACKED => {
                if level >= 64 {
                    return false;
                }
                let Ok(bytes) = Vec::<u8>::read(from) else {
                    return false;
                };
                let Ok(bytes) = tg_base::zlib_help::inflate_gzip(&bytes, 32 * 1024 * 1024) else {
                    return false;
                };
                if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
                    return false;
                }
                let words = bytes_to_primes(&bytes);
                to.push_str("[GZIPPED] ");
                return dump_to_text_type(to, &mut Reader::new(&words), 0, level + 1, 0, self);
            }
            _ => {
                if let Some(i) = LAYERS
                    .iter()
                    .enumerate()
                    .skip(1)
                    .find_map(|(i, &id)| (id == cons).then_some(i))
                {
                    to.push_str(&format!("[LAYER{}] ", i + 1));
                    return dump_to_text_type(to, from, 0, level, 0, self);
                }
                if cons == 0xda9b0d0d {
                    let Ok(layer) = i32::read(from) else {
                        return false;
                    };
                    to.push_str(&format!("[LAYER{layer}] "));
                    return dump_to_text_type(to, from, 0, level, 0, self);
                }
                None
            }
        };
        if let Some(text) = result {
            to.push_str(&text);
            true
        } else {
            false
        }
    }
}
// QString::number(double) defaults to general format with six significant digits.
fn qt_double(value: f64) -> String {
    if value.is_nan() {
        return "nan [DOUBLE]".into();
    }
    if value.is_infinite() {
        return format!(
            "{}inf [DOUBLE]",
            if value.is_sign_negative() { "-" } else { "" }
        );
    }
    if value == 0.0 {
        return "0 [DOUBLE]".into();
    }
    let exponent = value.abs().log10().floor() as i32;
    let rendered = if !(-4..6).contains(&exponent) {
        let s = format!("{value:.5e}");
        let (m, e) = s.split_once('e').unwrap();
        let e: i32 = e.parse().unwrap();
        format!(
            "{}e{}{:02}",
            m.trim_end_matches('0').trim_end_matches('.'),
            if e < 0 { "-" } else { "+" },
            e.abs()
        )
    } else {
        let decimals = (5 - exponent).max(0) as usize;
        let s = format!("{value:.decimals$}");
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').into()
        } else {
            s
        }
    };
    format!("{rendered} [DOUBLE]")
}
pub fn dump_to_text_type_from(words: &[i32], cons: u32) -> (String, bool) {
    let mut out = String::new();
    let result = dump_to_text_type(&mut out, &mut Reader::new(words), cons, 0, 0, &Core);
    (out, result)
}
pub fn dump_to_text(words: &[i32]) -> String {
    dump_to_text_type_from(words, crate::core_types::CORE_MESSAGE).0
}
