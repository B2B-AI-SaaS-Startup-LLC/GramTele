//! Port of the generated `scheme-dump_to_text.cpp` (`addTextSerialize`,
//! `addTextSerializeInit` and `DumpToTextType` in `generate_tl.py`).
//!
//! The original generates one `Serialize_<name>` function per constructor and
//! method, all following the same stage machine; the port runs that machine
//! once over [`crate::meta`]. Output text is the same, including its quirks:
//!
//! * a trailing `",\n"` before the closing `}` of every object;
//! * absent conditional fields are skipped, the separator depends on whether
//!   this is the first stage, not on what was printed;
//! * 64-bit flags are combined as `int64(low) + (int64(high) << 32)` with the
//!   low word sign-extended, so with bit 31 of `flags` set the `flags2` bits
//!   used to skip absent fields are off by one in the original; this is
//!   reproduced;
//! * `Vector<Vector<T>>` items are dumped as boxed values.
//!
//! Primitives, vectors, `gzip_packed` and layer wrappers are printed by
//! `DumpToTextCore`, which lives in the `mtproto` port
//! (`mtproto/details/mtproto_dump_to_text.cpp`) and is passed in as
//! [`DumpCore`].

use std::collections::HashMap;
use std::sync::OnceLock;

use tg_tl::core_types::{ID_CORE_MESSAGE, ID_MSG_CONTAINER, ID_RPC_RESULT};
use tg_tl::{
    Reader, TypeId, ID_DOUBLE, ID_FLAGS, ID_FLAGS64, ID_INT, ID_INT128, ID_INT256, ID_LONG,
    ID_STRING, ID_VECTOR,
};

use crate::meta::{self, Kind, Ty};

/// `DumpToTextCore`: prints a primitive / vector / special value of type
/// `cons` and returns `false` if it can't.
pub trait DumpCore {
    /// `DumpToTextCore(to, from, end, cons, level, vcons)`.
    fn dump_core(
        &self,
        to: &mut String,
        from: &mut Reader<'_>,
        cons: TypeId,
        level: u32,
        vcons: TypeId,
    ) -> bool;
}

/// `DumpToTextBuffer::addSpaces`.
pub fn add_spaces(to: &mut String, level: i32) {
    for _ in 0..level.max(0) * 2 {
        to.push(' ');
    }
}

/// `DumpToTextBuffer::error`.
pub fn add_error(to: &mut String, problem: &str) {
    to.push_str("[ERROR] (");
    to.push_str(problem);
    to.push(')');
}

#[derive(Clone, Copy)]
enum Step {
    Flags { wide: bool },
    True { bit: u8, field: &'static str },
    Push { ty: TypeId, vty: TypeId },
}

struct Serializer {
    name: &'static str,
    steps: Vec<(&'static str, Step, Option<u8>)>,
}

fn prim_id(ty: &Ty) -> Option<TypeId> {
    Some(match ty {
        Ty::Int => ID_INT,
        Ty::Long => ID_LONG,
        Ty::Double => ID_DOUBLE,
        Ty::String | Ty::Bytes => ID_STRING,
        Ty::Int128 => ID_INT128,
        Ty::Int256 => ID_INT256,
        _ => return None,
    })
}

/// The `types.push_back(...); vtypes.push_back(...)` pair the generator
/// computes for a parameter type.
fn pushes(ty: &Ty) -> (TypeId, TypeId) {
    match ty {
        Ty::Vector {
            item: Ty::Vector { .. },
            ..
        } => (0, 0),
        Ty::Vector { boxed, item } => {
            let vty = match item {
                Ty::Bare(c) => meta::CONSTRUCTORS[usize::from(*c)].id,
                other => prim_id(other).unwrap_or(0),
            };
            (if *boxed { 0 } else { ID_VECTOR }, vty)
        }
        Ty::Bare(c) => (meta::CONSTRUCTORS[usize::from(*c)].id, 0),
        other => (prim_id(other).unwrap_or(0), 0),
    }
}

fn serializer(def: &'static meta::Def) -> Serializer {
    let flags_name = def
        .params
        .iter()
        .find(|p| matches!(p.kind, Kind::Flags { .. }))
        .map_or("flags", |p| p.name);
    let flags2_name = def.flags2.unwrap_or("flags2");
    let steps = def
        .params
        .iter()
        .map(|p| match p.kind {
            Kind::Flags { wide } => (p.name, Step::Flags { wide }, None),
            Kind::True { bit } => (
                p.name,
                Step::True {
                    bit,
                    field: if bit >= 32 { flags2_name } else { flags_name },
                },
                Some(bit),
            ),
            Kind::Value { ty, bit } => {
                let (ty, vty) = pushes(&ty);
                (p.name, Step::Push { ty, vty }, bit)
            }
        })
        .collect();
    Serializer {
        name: def.cpp_name,
        steps,
    }
}

fn manual(name: &'static str, steps: &[(&'static str, TypeId, TypeId)]) -> Serializer {
    Serializer {
        name,
        steps: steps
            .iter()
            .map(|&(n, ty, vty)| (n, Step::Push { ty, vty }, None))
            .collect(),
    }
}

fn serializers() -> &'static HashMap<TypeId, Serializer> {
    static RESULT: OnceLock<HashMap<TypeId, Serializer>> = OnceLock::new();
    RESULT.get_or_init(|| {
        let mut map = HashMap::new();
        for def in meta::CONSTRUCTORS.iter().chain(meta::FUNCTIONS) {
            map.entry(def.id).or_insert_with(|| serializer(def));
        }
        map.entry(ID_RPC_RESULT).or_insert_with(|| {
            manual(
                "rpc_result",
                &[("req_msg_id", ID_LONG, 0), ("result", 0, 0)],
            )
        });
        map.entry(ID_MSG_CONTAINER).or_insert_with(|| {
            manual("msg_container", &[("messages", ID_VECTOR, ID_CORE_MESSAGE)])
        });
        map.entry(ID_CORE_MESSAGE).or_insert_with(|| {
            manual(
                "core_message",
                &[
                    ("msg_id", ID_LONG, 0),
                    ("seq_no", ID_INT, 0),
                    ("bytes", ID_INT, 0),
                    ("body", 0, 0),
                ],
            )
        });
        map
    })
}

struct Frames {
    types: Vec<TypeId>,
    vtypes: Vec<TypeId>,
    stages: Vec<i32>,
    flags: Vec<i64>,
}

impl Frames {
    fn push(&mut self, ty: TypeId, vty: TypeId) {
        self.types.push(ty);
        self.vtypes.push(vty);
        self.stages.push(0);
        self.flags.push(0);
    }
    fn pop(&mut self) {
        self.types.pop();
        self.vtypes.pop();
        self.stages.pop();
        self.flags.pop();
    }
    fn advance(&mut self) {
        if let Some(stage) = self.stages.last_mut() {
            *stage += 1;
        }
    }
}

/// One `Serialize_<name>(to, stage, lev, ...)` call.
fn run(
    s: &Serializer,
    to: &mut String,
    stage: i32,
    lev: i32,
    frames: &mut Frames,
    from: &Reader<'_>,
    iflag: u64,
) -> bool {
    let wide = s
        .steps
        .iter()
        .any(|(_, step, _)| matches!(step, Step::Flags { wide: true }));
    let flag = if wide { iflag } else { u64::from(iflag as u32) };
    if s.steps.is_empty() {
        to.push_str("{ ");
        to.push_str(s.name);
        to.push_str(" }");
        frames.pop();
        return true;
    }
    let has_conditions = s.steps.iter().any(|(_, _, bit)| bit.is_some());
    let first = stage == 0;
    let mut stage = stage;
    if has_conditions {
        let absent = |index: i32| -> bool {
            usize::try_from(index)
                .ok()
                .and_then(|i| s.steps.get(i))
                .and_then(|(_, _, bit)| *bit)
                .is_some_and(|bit| flag & (1u64 << bit) == 0)
        };
        while absent(stage) {
            stage += 1;
            frames.advance();
        }
    }
    let not_first = if has_conditions { !first } else { stage != 0 };
    if not_first {
        to.push_str(",\n");
        add_spaces(to, lev);
    } else {
        to.push_str("{ ");
        to.push_str(s.name);
        to.push('\n');
        add_spaces(to, lev);
    }
    let step = usize::try_from(stage).ok().and_then(|i| s.steps.get(i));
    match step {
        Some(&(name, step, _)) => {
            to.push_str("  ");
            to.push_str(name);
            to.push_str(": ");
            frames.advance();
            match step {
                Step::Flags { wide } => {
                    let rest = from.rest();
                    let value = if wide {
                        if rest.len() < 2 {
                            return false;
                        }
                        i64::from(rest[0]).wrapping_add(i64::from(rest[1]) << 32)
                    } else {
                        match rest.first() {
                            Some(&low) => i64::from(low),
                            None => return false,
                        }
                    };
                    if let Some(last) = frames.flags.last_mut() {
                        *last = value;
                    }
                    frames.push(if wide { ID_FLAGS64 } else { ID_FLAGS }, 0);
                }
                Step::True { bit, field } => {
                    let logged = if bit >= 32 { bit - 32 } else { bit };
                    to.push_str(&format!("YES [ BY BIT {logged} IN FIELD {field} ]"));
                }
                Step::Push { ty, vty } => frames.push(ty, vty),
            }
        }
        None => {
            to.push('}');
            frames.pop();
        }
    }
    true
}

/// `DumpToTextType(to, from, end, cons, level, vcons)`.
pub fn dump_to_text_type(
    to: &mut String,
    from: &mut Reader<'_>,
    cons: TypeId,
    level: u32,
    vcons: TypeId,
    core: &dyn DumpCore,
) -> bool {
    let serializers = serializers();
    let mut frames = Frames {
        types: Vec::with_capacity(20),
        vtypes: Vec::with_capacity(20),
        stages: Vec::with_capacity(20),
        flags: Vec::with_capacity(20),
    };
    frames.push(cons, vcons);
    while let Some(&top) = frames.types.last() {
        let mut ty = top;
        let vty = frames.vtypes.last().copied().unwrap_or(0);
        let stage = frames.stages.last().copied().unwrap_or(0);
        let flag = frames.flags.last().copied().unwrap_or(0);
        if ty == 0 {
            if from.is_empty() {
                add_error(to, "insufficient data");
                return false;
            } else if stage != 0 {
                add_error(to, "unknown type on stage > 0");
                return false;
            }
            ty = match from.get() {
                Ok(v) => v,
                Err(_) => return false,
            };
            if let Some(last) = frames.types.last_mut() {
                *last = ty;
            }
        }
        let lev = level as i32 + frames.types.len() as i32 - 1;
        if let Some(s) = serializers.get(&ty) {
            if !run(s, to, stage, lev, &mut frames, from, flag as u64) {
                add_error(to, "could not decode type");
                return false;
            }
        } else if core.dump_core(to, from, ty, lev.max(0) as u32, vty) {
            frames.pop();
        } else {
            add_error(to, "could not decode type");
            to.push_str(&format!("(ERROR_SCHEME_BAD_CONS:0x{ty:x})"));
            return false;
        }
    }
    true
}
