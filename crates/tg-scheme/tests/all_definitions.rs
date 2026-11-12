//! Build wire samples from schema metadata, independently of generated writers.
//! Exercise every constructor and method, with conditional fields absent/present.
use tg_scheme::meta::{self, Def, Kind, Ty};
use tg_scheme::tl::{self, Reader};

fn value(ty: Ty, depth: usize) -> Option<Vec<i32>> {
    if depth > 24 {
        return None;
    }
    Some(match ty {
        Ty::Int => vec![17],
        Ty::Long | Ty::Double => vec![17, 0],
        Ty::Int128 => vec![1, 2, 3, 4],
        Ty::Int256 => vec![1, 2, 3, 4, 5, 6, 7, 8],
        Ty::String | Ty::Bytes => vec![0x0062_6102], // two bytes "ab", padding
        Ty::Template => vec![tl::ID_BOOL_TRUE as i32],
        Ty::Vector { boxed, .. } => {
            if boxed {
                vec![tl::ID_VECTOR as i32, 0]
            } else {
                vec![0]
            }
        }
        Ty::Bare(index) => body(&meta::CONSTRUCTORS[usize::from(index)], false, depth + 1)?,
        Ty::Boxed(index) => {
            let mut sample = None;
            for &constructor in meta::TYPES[usize::from(index)].constructors {
                let def = &meta::CONSTRUCTORS[usize::from(constructor)];
                if let Some(fields) = body(def, false, depth + 1) {
                    let mut words = vec![def.id as i32];
                    words.extend(fields);
                    sample = Some(words);
                    break;
                }
            }
            sample?
        }
    })
}

fn body(def: &Def, present: bool, depth: usize) -> Option<Vec<i32>> {
    let flags = if present {
        def.params.iter().fold(0u64, |bits, p| {
            let bit = match p.kind {
                Kind::True { bit } | Kind::Value { bit: Some(bit), .. } => Some(bit),
                _ => None,
            };
            bits | bit.map_or(0, |b| 1u64 << b)
        })
    } else {
        0
    };
    let mut words = Vec::new();
    for p in def.params {
        match p.kind {
            Kind::Flags { wide } => {
                words.push(flags as i32);
                if wide {
                    words.push((flags >> 32) as i32);
                }
            }
            Kind::True { .. } => {}
            Kind::Value { ty, bit } => {
                if bit.is_none() || present {
                    words.extend(value(ty, depth + 1)?);
                }
            }
        }
    }
    Some(words)
}

#[test]
fn every_constructor_and_method_reads_reference_fields() {
    for present in [false, true] {
        for (method, definitions) in [(false, meta::CONSTRUCTORS), (true, meta::FUNCTIONS)] {
            for (index, def) in definitions.iter().enumerate() {
                let mut words = vec![def.id as i32];
                words.extend(
                    body(def, present, 0).unwrap_or_else(|| panic!("sample for {}", def.name)),
                );
                let mut reader = Reader::new(&words);
                let result = if method {
                    tg_scheme::__reencode_function(index, &mut reader)
                } else {
                    let Ty::Boxed(t) = def.result else {
                        panic!("constructor result")
                    };
                    tg_scheme::__reencode_type(usize::from(t), &mut reader)
                };
                let encoded = result
                    .unwrap()
                    .unwrap_or_else(|e| panic!("{}: {e:?}", def.name));
                assert_eq!(encoded, words, "{} (conditionals {present})", def.name);
                assert!(reader.is_empty(), "{}", def.name);
            }
        }
    }
}
