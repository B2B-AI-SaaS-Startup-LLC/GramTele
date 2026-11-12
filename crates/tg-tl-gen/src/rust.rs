//! Rust code emission: the port of the output half of `generate_tl.py`
//! (`typesText`, `dataTexts`, `funcsText`, `methods`, `factories`,
//! `flagOperators`) and of the `dumpToText` tables.
//!
//! Shape of the output (see the crate docs of `tg-scheme` and its README for
//! the full C++ → Rust table):
//!
//! * a boxed type → `enum`, one variant per constructor; a constructor with
//!   parameters is held as `Arc<XData>` (the original's refcounted
//!   `type_data`), one without parameters inline as a unit struct;
//! * a constructor → `XData` struct with public fields, `ID`, `F_*` flag
//!   constants, `v*()` / `is_*()` getters with the original's semantics;
//! * a method → struct with public fields, `new(...)`, `ID`, `F_*`, and a
//!   [`tg_tl::Function`] impl naming the response type;
//! * `MTP_x(...)` factories → `mtp_x(...)` functions returning the enum;
//! * `Bool` is Rust `bool` (no generated enum);
//! * metadata tables (`meta.rs` in `tg-scheme`) for text dumps and tests.
//!
//! The code is plain functions and `match`es without macros, and only
//! `Clone` + `Default` are derived (`Debug` behind the `debug` feature), to
//! keep the compile time of the ~3000 generated items down.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use crate::generate_tl::{
    function_boxed_cpp_name, Definition, ParamKind, Primitive, Schema, TypeRef,
};
use crate::names;

macro_rules! w {
    ($o:expr) => {{
        let _ = writeln!($o);
    }};
    ($o:expr, $($t:tt)*) => {{
        let _ = writeln!($o, $($t)*);
    }};
}

/// One C++ name and the Rust item it maps to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameMapping {
    /// The C++ name (`MTPDuser`, `mtpc_user`, `MTP_user`, ...).
    pub cpp: String,
    /// The Rust path relative to the crate root (`UserData`,
    /// `UserData::ID`, `mtp_user`, ...).
    pub rust: String,
}

/// Rust names computed for a schema.
pub struct Names<'a> {
    schema: &'a Schema,
    bool_type: Option<usize>,
    type_names: HashSet<String>,
}

impl<'a> Names<'a> {
    /// Names for `schema`.
    pub fn new(schema: &'a Schema) -> Self {
        Self {
            schema,
            bool_type: schema.type_index("Bool"),
            type_names: schema
                .types
                .iter()
                .map(|t| names::type_name(&t.cpp_name()))
                .collect(),
        }
    }

    fn is_bool(&self, t: usize) -> bool {
        Some(t) == self.bool_type
    }

    /// `true` if constructor `c` belongs to `Bool` (not generated).
    pub fn is_bool_constructor(&self, c: usize) -> bool {
        matches!(self.schema.constructors[c].result, TypeRef::Boxed(t) if self.is_bool(t))
    }

    /// Rust enum of boxed type `t`.
    pub fn type_name(&self, t: usize) -> String {
        names::type_name(&self.schema.types[t].cpp_name())
    }

    /// Rust data struct of constructor `c`.
    pub fn data_name(&self, c: usize) -> String {
        names::data_name(&self.schema.constructors[c].cpp_name())
    }

    /// Rust method struct of function `f`. The only exception to the naming
    /// rule: a method whose name equals a type name gets the suffix `Method`
    /// (`rpc_drop_answer` vs `RpcDropAnswer` → `RpcDropAnswerMethod`).
    pub fn function_name(&self, f: usize) -> String {
        let name = names::function_name(&self.schema.functions[f].cpp_name());
        if self.type_names.contains(&name) {
            format!("{name}Method")
        } else {
            name
        }
    }

    /// Rust type of a field / result.
    pub fn rust_type(&self, ty: &TypeRef) -> String {
        match ty {
            TypeRef::Primitive(p) => match p {
                Primitive::Int => "i32".into(),
                Primitive::Long => "u64".into(),
                Primitive::Double => "f64".into(),
                Primitive::String | Primitive::Bytes => "Vec<u8>".into(),
                Primitive::Int128 => "tl::Int128".into(),
                Primitive::Int256 => "tl::Int256".into(),
            },
            TypeRef::Boxed(t) if self.is_bool(*t) => "bool".into(),
            TypeRef::Boxed(t) => self.type_name(*t),
            TypeRef::Bare(c) => self.data_name(*c),
            TypeRef::Vector { boxed: true, item } => format!("Vec<{}>", self.rust_type(item)),
            TypeRef::Vector { boxed: false, item } => {
                format!("tl::BareVec<{}>", self.rust_type(item))
            }
            TypeRef::Template => "X".into(),
        }
    }

    fn is_copy(&self, ty: &TypeRef) -> bool {
        match ty {
            TypeRef::Primitive(p) => !matches!(p, Primitive::String | Primitive::Bytes),
            TypeRef::Boxed(t) => self.is_bool(*t),
            _ => false,
        }
    }

    /// Every C++ name of the schema with its Rust counterpart.
    pub fn mappings(&self) -> Vec<NameMapping> {
        let mut result = Vec::new();
        let mut push = |cpp: String, rust: String| result.push(NameMapping { cpp, rust });
        for (t, info) in self.schema.types.iter().enumerate() {
            let rust = if self.is_bool(t) {
                "bool".to_string()
            } else {
                self.type_name(t)
            };
            push(format!("MTP{}", info.cpp_name()), rust.clone());
            if info.cpp_bare_name() != info.cpp_name() {
                push(format!("MTP{}", info.cpp_bare_name()), rust);
            }
        }
        for (c, def) in self.schema.constructors.iter().enumerate() {
            let s = def.cpp_name();
            if self.is_bool_constructor(c) {
                let (id, value) = if def.name == "boolTrue" {
                    ("tl::ID_BOOL_TRUE", "true")
                } else {
                    ("tl::ID_BOOL_FALSE", "false")
                };
                push(format!("mtpc_{s}"), id.to_string());
                push(format!("MTP_{s}"), value.to_string());
                continue;
            }
            let data = self.data_name(c);
            let TypeRef::Boxed(t) = def.result else {
                continue;
            };
            push(format!("MTPD{s}"), data.clone());
            push(format!("mtpc_{s}"), format!("{data}::ID"));
            push(format!("MTP_{s}"), names::factory_name(&s));
            push(
                format!("MTP{}::c_{s}", self.schema.types[t].cpp_name()),
                format!("{}::{}", self.type_name(t), names::c_getter(&s)),
            );
        }
        for (f, def) in self.schema.functions.iter().enumerate() {
            let s = def.cpp_name();
            let rust = self.function_name(f);
            push(format!("MTP{s}"), rust.clone());
            let boxed = function_boxed_cpp_name(&def.name);
            if boxed != s {
                push(format!("MTP{boxed}"), rust.clone());
            }
            push(format!("mtpc_{s}"), format!("{rust}::ID"));
        }
        result
    }
}

/// Checks that the Rust names are unique where they must be.
pub fn check_names(schema: &Schema) -> Result<(), String> {
    let n = Names::new(schema);
    let mut types: HashMap<String, String> = HashMap::new();
    let mut add = |rust: String, what: String| -> Result<(), String> {
        if names::RESERVED_TYPE_NAMES.contains(&rust.as_str()) {
            return Err(format!("Rust name {rust} of {what} shadows the prelude"));
        }
        if let Some(other) = types.insert(rust.clone(), what.clone()) {
            return Err(format!("Rust name {rust} used by both {other} and {what}"));
        }
        Ok(())
    };
    for t in 0..schema.types.len() {
        if !n.is_bool(t) {
            add(n.type_name(t), format!("type {}", schema.types[t].name))?;
        }
    }
    for (c, def) in schema.constructors.iter().enumerate() {
        if !n.is_bool_constructor(c) {
            add(n.data_name(c), format!("constructor {}", def.name))?;
        }
    }
    for (f, def) in schema.functions.iter().enumerate() {
        add(n.function_name(f), format!("method {}", def.name))?;
        if def.template.is_some() && def.flags.is_some() {
            add(
                format!("{}Flags", n.function_name(f)),
                format!("flags of {}", def.name),
            )?;
        }
    }
    let mut values = HashSet::new();
    for (c, def) in schema.constructors.iter().enumerate() {
        if !n.is_bool_constructor(c) && !values.insert(names::factory_name(&def.cpp_name())) {
            return Err(format!("Factory name of {} is not unique", def.name));
        }
    }
    for t in &schema.types {
        let mut getters = HashSet::new();
        let mut variants = HashSet::new();
        for &c in &t.constructors {
            let s = schema.constructors[c].cpp_name();
            if !getters.insert(names::c_getter(&s)) || !variants.insert(names::variant_name(&s)) {
                return Err(format!(
                    "Getter / variant of {s} is not unique in {}",
                    t.name
                ));
            }
        }
    }
    for def in schema.constructors.iter().chain(&schema.functions) {
        let mut seen = HashSet::new();
        for p in &def.params {
            for name in [
                names::field(&p.name),
                names::getter(&p.name),
                names::flag_const(&p.name),
            ] {
                if !seen.insert(name.clone()) {
                    return Err(format!("Name {name} is not unique in {}", def.name));
                }
            }
        }
    }
    Ok(())
}

/// The default value of an enum is its first constructor with default
/// fields; make sure this does not recurse forever.
fn check_default_cycles(schema: &Schema) -> Result<(), String> {
    fn deps(schema: &Schema, c: usize, out: &mut Vec<usize>) {
        for p in &schema.constructors[c].params {
            if let ParamKind::Value { ty, bit: None } = &p.kind {
                match ty {
                    TypeRef::Boxed(t) => out.push(schema.types[*t].constructors[0]),
                    TypeRef::Bare(c) => out.push(*c),
                    _ => {}
                }
            }
        }
    }
    fn visit(schema: &Schema, c: usize, state: &mut [u8]) -> Result<(), String> {
        match state[c] {
            1 => {
                return Err(format!(
                    "Default value of {} recurses into itself",
                    schema.constructors[c].name
                ))
            }
            2 => return Ok(()),
            _ => {}
        }
        state[c] = 1;
        let mut next = Vec::new();
        deps(schema, c, &mut next);
        for d in next {
            visit(schema, d, state)?;
        }
        state[c] = 2;
        Ok(())
    }
    let mut state = vec![0u8; schema.constructors.len()];
    for c in 0..schema.constructors.len() {
        visit(schema, c, &mut state)?;
    }
    Ok(())
}

struct Emitter<'a> {
    schema: &'a Schema,
    n: Names<'a>,
    out: String,
}

/// Generated sources.
pub struct Output {
    /// Types, methods, factories (`scheme.rs`).
    pub scheme: String,
    /// Metadata tables (`scheme_meta.rs`).
    pub meta: String,
}

/// Generates Rust code for `schema`.
pub fn generate(schema: &Schema) -> Result<Output, String> {
    check_names(schema)?;
    check_default_cycles(schema)?;
    let mut e = Emitter {
        schema,
        n: Names::new(schema),
        out: String::with_capacity(8 << 20),
    };
    e.header();
    for t in 0..schema.types.len() {
        if !e.n.is_bool(t) {
            e.boxed_type(t);
        }
    }
    for c in 0..schema.constructors.len() {
        if !e.n.is_bool_constructor(c) {
            e.constructor(c);
        }
    }
    for f in 0..schema.functions.len() {
        e.function(f);
    }
    e.flag_casts();
    e.test_support();
    let scheme = std::mem::take(&mut e.out);
    e.meta();
    Ok(Output {
        scheme,
        meta: e.out,
    })
}

impl Emitter<'_> {
    fn header(&mut self) {
        let o = &mut self.out;
        w!(o, "// WARNING! All changes made in this file will be lost!");
        w!(
            o,
            "// Created from '{}' by tg-tl-gen (port of generate_tl.py).",
            self.schema.input_names.join("', '")
        );
        w!(o);
        w!(o, "/// `kCurrentLayer`.");
        w!(o, "pub const CURRENT_LAYER: i32 = {};", self.schema.layer);
        w!(o);
    }

    fn flags_tag(&self, def: &Definition, rust: &str) -> String {
        if def.template.is_some() {
            format!("{rust}Flags")
        } else {
            rust.to_string()
        }
    }

    fn field_decl(&self, def: &Definition, tag: &str) -> Vec<(String, String)> {
        let mut result = Vec::new();
        for p in &def.params {
            match &p.kind {
                ParamKind::Flags { .. } => {
                    result.push((names::field(&p.name), format!("tl::Flags<{tag}>")))
                }
                ParamKind::True { .. } => {}
                ParamKind::Value { ty, bit } => {
                    let ty = self.n.rust_type(ty);
                    let ty = if bit.is_some() {
                        format!("Option<{ty}>")
                    } else {
                        ty
                    };
                    result.push((names::field(&p.name), ty));
                }
            }
        }
        result
    }

    fn flags_field(def: &Definition) -> String {
        names::field(def.flags.as_deref().unwrap_or("flags"))
    }

    fn flag_consts(&mut self, def: &Definition, tag: &str) {
        if def.flags.is_none() {
            return;
        }
        let mut max = 0;
        for p in &def.params {
            if let ParamKind::True { bit } | ParamKind::Value { bit: Some(bit), .. } = p.kind {
                w!(
                    self.out,
                    "    /// `Flag::f_{}`.\n    pub const {}: tl::Flags<{tag}> = tl::Flags::from_raw(1 << {bit});",
                    p.name,
                    names::flag_const(&p.name)
                );
                max = max.max(bit);
            }
        }
        w!(
            self.out,
            "    /// `Flag::MAX_FIELD`.\n    pub const MAX_FIELD: tl::Flags<{tag}> = tl::Flags::from_raw(1 << {max});"
        );
    }

    fn write_body(&mut self, def: &Definition) {
        let flags = Self::flags_field(def);
        for p in &def.params {
            let f = names::field(&p.name);
            match &p.kind {
                ParamKind::Flags { wide } => {
                    w!(self.out, "        tl::put(__w, self.{f}.value() as u32);");
                    if *wide {
                        w!(
                            self.out,
                            "        tl::put(__w, (self.{f}.value() >> 32) as u32);"
                        );
                    }
                }
                ParamKind::True { .. } => {}
                ParamKind::Value { bit: None, .. } => {
                    w!(self.out, "        tl::Serialize::write(&self.{f}, __w);")
                }
                ParamKind::Value { bit: Some(bit), .. } => w!(
                    self.out,
                    "        tl::write_conditional(__w, self.{flags}.bit({bit}), &self.{f});"
                ),
            }
        }
    }

    fn read_body(&mut self, def: &Definition) {
        let flags = Self::flags_field(def);
        let mut fields = Vec::new();
        for p in &def.params {
            let f = names::field(&p.name);
            match &p.kind {
                ParamKind::Flags { wide } => {
                    if *wide {
                        w!(self.out, "        let __low = u64::from(__r.get()?);");
                        w!(
                            self.out,
                            "        let {f} = tl::Flags::from_raw(__low | (u64::from(__r.get()?) << 32));"
                        );
                    } else {
                        w!(
                            self.out,
                            "        let {f} = tl::Flags::from_raw(u64::from(__r.get()?));"
                        );
                    }
                }
                ParamKind::True { .. } => continue,
                ParamKind::Value { bit: None, .. } => {
                    w!(self.out, "        let {f} = tl::Deserialize::read(__r)?;")
                }
                ParamKind::Value { bit: Some(bit), .. } => w!(
                    self.out,
                    "        let {f} = tl::read_conditional(__r, {flags}.bit({bit}))?;"
                ),
            }
            fields.push(f);
        }
        w!(self.out, "        Ok(Self {{ {} }})", fields.join(", "));
    }

    fn boxed_type(&mut self, t: usize) {
        let schema = self.schema;
        let info = &schema.types[t];
        let name = self.n.type_name(t);
        let ctors = info.constructors.clone();
        let single = ctors.len() == 1;
        let variant = |c: usize| names::variant_name(&self.schema.constructors[c].cpp_name());
        let o = &mut self.out;
        w!(
            o,
            "/// `{}` — C++ `MTP{}` (bare `MTP{}`).",
            info.name,
            info.cpp_name(),
            info.cpp_bare_name()
        );
        w!(o, "#[derive(Clone)]");
        w!(o, "#[cfg_attr(feature = \"debug\", derive(Debug))]");
        w!(o, "pub enum {name} {{");
        for &c in &ctors {
            let def = &self.schema.constructors[c];
            let data = self.n.data_name(c);
            let held = if def.params.is_empty() {
                data
            } else {
                format!("Arc<{data}>")
            };
            w!(o, "    /// `{}`.\n    {}({held}),", def.name, variant(c));
        }
        w!(o, "}}");
        w!(o);
        let first = ctors[0];
        w!(o, "impl Default for {name} {{");
        w!(
            o,
            "    fn default() -> Self {{\n        Self::{}(Default::default())\n    }}",
            variant(first)
        );
        w!(o, "}}");
        w!(o);
        w!(o, "impl {name} {{");
        w!(o, "    /// The constructor id (`type()`).");
        w!(o, "    pub fn type_id(&self) -> u32 {{");
        w!(o, "        match self {{");
        for &c in &ctors {
            w!(
                o,
                "            Self::{}(_) => {}::ID,",
                variant(c),
                self.n.data_name(c)
            );
        }
        w!(o, "        }}\n    }}");
        for &c in &ctors {
            let def = &self.schema.constructors[c];
            let data = self.n.data_name(c);
            let getter = names::c_getter(&def.cpp_name());
            w!(
                o,
                "    /// `c_{}()`. Panics on another constructor (`Expects`).",
                def.cpp_name()
            );
            w!(o, "    pub fn {getter}(&self) -> &{data} {{");
            if single {
                w!(
                    o,
                    "        let Self::{}(data) = self;\n        data",
                    variant(c)
                );
            } else {
                w!(o, "        match self {{");
                w!(o, "            Self::{}(data) => data,", variant(c));
                w!(
                    o,
                    "            _ => panic!(\"Type in {name}::{getter}: 0x{{:08x}}\", self.type_id()),"
                );
                w!(o, "        }}");
            }
            w!(o, "    }}");
            if single && !def.params.is_empty() {
                w!(o, "    /// `data()` of a single-constructor type.");
                w!(
                    o,
                    "    pub fn data(&self) -> &{data} {{\n        self.{getter}()\n    }}"
                );
            }
        }
        w!(o, "}}");
        w!(o);
        w!(o, "impl tl::Serialize for {name} {{");
        w!(o, "    fn write(&self, __w: &mut tl::Buffer) {{");
        w!(o, "        match self {{");
        for &c in &ctors {
            let body = if schema.constructors[c].params.is_empty() {
                "data"
            } else {
                "&**data"
            };
            w!(
                o,
                "            Self::{}(data) => {{\n                tl::put(__w, {}::ID);\n                tl::Serialize::write({body}, __w);\n            }}",
                variant(c),
                self.n.data_name(c)
            );
        }
        w!(o, "        }}\n    }}\n}}");
        w!(o);
        w!(o, "impl tl::Boxed for {name} {{");
        w!(
            o,
            "    fn type_id(&self) -> u32 {{\n        {name}::type_id(self)\n    }}"
        );
        w!(
            o,
            "    fn read_bare(__r: &mut tl::Reader<'_>, __cons: u32) -> tl::Result<Self> {{"
        );
        let first_def = &self.schema.constructors[first];
        if single && first_def.params.is_empty() {
            // The original's bare read of a single constructor without data
            // has no `cons` check at all.
            w!(o, "        let _ = (__r, __cons);");
            w!(
                o,
                "        Ok(Self::{}({}))",
                variant(first),
                self.n.data_name(first)
            );
        } else {
            w!(o, "        match __cons {{");
            for &c in &ctors {
                let def = &self.schema.constructors[c];
                let data = self.n.data_name(c);
                if def.params.is_empty() {
                    w!(
                        o,
                        "            {data}::ID => Ok(Self::{}({data})),",
                        variant(c)
                    );
                } else {
                    w!(
                        o,
                        "            {data}::ID => Ok(Self::{}(Arc::new(tl::Deserialize::read(__r)?))),",
                        variant(c)
                    );
                }
            }
            w!(
                o,
                "            id => Err(tl::Error::UnexpectedConstructor {{ id }}),"
            );
            w!(o, "        }}");
        }
        w!(o, "    }}\n}}");
        w!(o);
        w!(o, "impl tl::Deserialize for {name} {{");
        w!(
            o,
            "    fn read(__r: &mut tl::Reader<'_>) -> tl::Result<Self> {{\n        let __cons = __r.get()?;\n        <Self as tl::Boxed>::read_bare(__r, __cons)\n    }}\n}}"
        );
        w!(o);
        for &c in &ctors {
            let def = &self.schema.constructors[c];
            let data = self.n.data_name(c);
            let wrap = if def.params.is_empty() {
                "data".to_string()
            } else {
                "Arc::new(data)".to_string()
            };
            w!(
                o,
                "impl From<{data}> for {name} {{\n    fn from(data: {data}) -> Self {{\n        Self::{}({wrap})\n    }}\n}}\n",
                variant(c)
            );
        }
    }

    fn constructor(&mut self, c: usize) {
        let schema = self.schema;
        let def = &schema.constructors[c];
        let data = self.n.data_name(c);
        let TypeRef::Boxed(t) = def.result else {
            unreachable!("constructor result is boxed")
        };
        let type_name = self.n.type_name(t);
        let s = def.cpp_name();
        let fields = self.field_decl(def, &data);
        w!(
            self.out,
            "/// `{}#{:08x}` — C++ `MTPD{s}`, `mtpc_{s}`, `MTP_{s}`.",
            def.name,
            def.id
        );
        if def.params.is_empty() {
            w!(self.out, "#[derive(Clone, Copy, Default)]");
            w!(self.out, "#[cfg_attr(feature = \"debug\", derive(Debug))]");
            w!(self.out, "pub struct {data};");
        } else {
            w!(self.out, "#[derive(Clone, Default)]");
            w!(self.out, "#[cfg_attr(feature = \"debug\", derive(Debug))]");
            w!(self.out, "pub struct {data} {{");
            for (f, ty) in &fields {
                w!(self.out, "    pub {f}: {ty},");
            }
            w!(self.out, "}}");
        }
        w!(self.out);
        w!(self.out, "impl {data} {{");
        w!(
            self.out,
            "    /// `mtpc_{s}`.\n    pub const ID: u32 = 0x{:08x};",
            def.id
        );
        self.flag_consts(def, &data);
        self.getters(def, &data);
        w!(self.out, "}}");
        w!(self.out);
        w!(
            self.out,
            "impl tl::Identifiable for {data} {{\n    const ID: u32 = {data}::ID;\n}}\n"
        );
        w!(self.out, "impl tl::Serialize for {data} {{");
        if def.params.is_empty() {
            w!(self.out, "    fn write(&self, _: &mut tl::Buffer) {{}}");
        } else {
            w!(self.out, "    fn write(&self, __w: &mut tl::Buffer) {{");
            self.write_body(def);
            w!(self.out, "    }}");
        }
        w!(self.out, "}}");
        w!(self.out);
        w!(self.out, "impl tl::Deserialize for {data} {{");
        if def.params.is_empty() {
            w!(
                self.out,
                "    fn read(_: &mut tl::Reader<'_>) -> tl::Result<Self> {{\n        Ok(Self)\n    }}"
            );
        } else {
            w!(
                self.out,
                "    fn read(__r: &mut tl::Reader<'_>) -> tl::Result<Self> {{"
            );
            self.read_body(def);
            w!(self.out, "    }}");
        }
        w!(self.out, "}}");
        w!(self.out);
        // Factory (MTP_x).
        let args: Vec<String> = fields.iter().map(|(f, ty)| format!("{f}: {ty}")).collect();
        let names_list: Vec<&str> = fields.iter().map(|(f, _)| f.as_str()).collect();
        let variant = names::variant_name(&s);
        w!(self.out, "/// `MTP_{s}`.");
        w!(
            self.out,
            "pub fn {}({}) -> {type_name} {{",
            names::factory_name(&s),
            args.join(", ")
        );
        if def.params.is_empty() {
            w!(self.out, "    {type_name}::{variant}({data})");
        } else {
            w!(
                self.out,
                "    {type_name}::{variant}(Arc::new({data} {{ {} }}))",
                names_list.join(", ")
            );
        }
        w!(self.out, "}}");
        w!(self.out);
    }

    fn getters(&mut self, def: &Definition, tag: &str) {
        let flags = Self::flags_field(def);
        for p in &def.params {
            let f = names::field(&p.name);
            match &p.kind {
                ParamKind::Flags { .. } => {
                    w!(
                        self.out,
                        "    /// `v{}()`.\n    pub fn {}(&self) -> tl::Flags<{tag}> {{\n        self.{f}\n    }}",
                        p.name,
                        names::getter(&p.name)
                    );
                }
                ParamKind::True { bit } => {
                    let getter = names::is_getter(&p.name);
                    // AyuGram: generate_tl.py hook, `is_noforwards()` and
                    // `is_restricted()` always report false, the real flag is
                    // `is_ayuNoforwards()` / `is_ayuRestricted()`.
                    let hooked = def.flags.as_deref() == Some("flags")
                        && (p.name == "noforwards" || p.name == "restricted");
                    if hooked {
                        w!(
                            self.out,
                            "    /// `is_{}()` (AyuGram: always `false`).\n    pub fn {getter}(&self) -> bool {{\n        false\n    }}",
                            p.name
                        );
                        w!(
                            self.out,
                            "    /// `is_ayu{}()` (AyuGram: the real flag).\n    pub fn is_ayu_{}(&self) -> bool {{\n        self.{flags}.bit({bit})\n    }}",
                            upper_first(&p.name),
                            p.name.to_lowercase()
                        );
                    } else {
                        w!(
                            self.out,
                            "    /// `is_{}()`.\n    pub fn {getter}(&self) -> bool {{\n        self.{flags}.bit({bit})\n    }}",
                            p.name
                        );
                    }
                }
                ParamKind::Value { ty, bit } => {
                    let rust = self.n.rust_type(ty);
                    let copy = self.n.is_copy(ty);
                    let getter = names::getter(&p.name);
                    match (bit, copy) {
                        (None, true) => w!(
                            self.out,
                            "    /// `v{}()`.\n    pub fn {getter}(&self) -> {rust} {{\n        self.{f}\n    }}",
                            p.name
                        ),
                        (None, false) => w!(
                            self.out,
                            "    /// `v{}()`.\n    pub fn {getter}(&self) -> &{rust} {{\n        &self.{f}\n    }}",
                            p.name
                        ),
                        (Some(bit), true) => w!(
                            self.out,
                            "    /// `v{}()`: present iff flag bit {bit} is set.\n    pub fn {getter}(&self) -> Option<{rust}> {{\n        if self.{flags}.bit({bit}) {{ self.{f} }} else {{ None }}\n    }}",
                            p.name
                        ),
                        (Some(bit), false) => w!(
                            self.out,
                            "    /// `v{}()`: present iff flag bit {bit} is set.\n    pub fn {getter}(&self) -> Option<&{rust}> {{\n        if self.{flags}.bit({bit}) {{ self.{f}.as_ref() }} else {{ None }}\n    }}",
                            p.name
                        ),
                    }
                }
            }
        }
    }

    fn function(&mut self, f: usize) {
        let schema = self.schema;
        let def = &schema.functions[f];
        let rust = self.n.function_name(f);
        let s = def.cpp_name();
        let generic = def.template.is_some();
        let tag = self.flags_tag(def, &rust);
        let fields = self.field_decl(def, &tag);
        let (gen, gen_use) = if generic { ("<X>", "<X>") } else { ("", "") };
        let response = match &def.result {
            TypeRef::Template => "X::Response".to_string(),
            other => self.n.rust_type(other),
        };
        w!(
            self.out,
            "/// `{}#{:08x}` — C++ `MTP{}` (bare `MTP{s}`), `mtpc_{s}`. Returns `{response}`.",
            def.name,
            def.id,
            function_boxed_cpp_name(&def.name)
        );
        w!(self.out, "#[derive(Clone, Default)]");
        w!(self.out, "#[cfg_attr(feature = \"debug\", derive(Debug))]");
        if fields.is_empty() {
            w!(self.out, "pub struct {rust};");
        } else {
            w!(self.out, "pub struct {rust}{gen} {{");
            for (f, ty) in &fields {
                w!(self.out, "    pub {f}: {ty},");
            }
            w!(self.out, "}}");
        }
        w!(self.out);
        if generic && def.flags.is_some() {
            w!(
                self.out,
                "/// Flags tag of the generic method `{}`.",
                def.name
            );
            w!(self.out, "pub struct {tag};");
            w!(self.out);
            w!(self.out, "impl {tag} {{");
            self.flag_consts(def, &tag);
            w!(self.out, "}}");
            w!(self.out);
        }
        let args: Vec<String> = fields.iter().map(|(f, ty)| format!("{f}: {ty}")).collect();
        let names_list: Vec<&str> = fields.iter().map(|(f, _)| f.as_str()).collect();
        w!(self.out, "impl{gen} {rust}{gen_use} {{");
        w!(
            self.out,
            "    /// `mtpc_{s}`.\n    pub const ID: u32 = 0x{:08x};",
            def.id
        );
        if !generic {
            self.flag_consts(def, &tag);
        }
        if fields.is_empty() {
            w!(
                self.out,
                "    /// `MTP{}()`.\n    pub fn new() -> Self {{\n        Self\n    }}",
                function_boxed_cpp_name(&def.name)
            );
        } else {
            w!(
                self.out,
                "    /// `MTP{}(...)`.\n    pub fn new({}) -> Self {{\n        Self {{ {} }}\n    }}",
                function_boxed_cpp_name(&def.name),
                args.join(", "),
                names_list.join(", ")
            );
        }
        w!(self.out, "}}");
        w!(self.out);
        let ser_bound = if generic { "<X: tl::Serialize>" } else { "" };
        let de_bound = if generic { "<X: tl::Deserialize>" } else { "" };
        w!(
            self.out,
            "impl{gen} tl::Identifiable for {rust}{gen_use} {{\n    const ID: u32 = 0x{:08x};\n}}\n",
            def.id
        );
        w!(
            self.out,
            "impl{ser_bound} tl::Serialize for {rust}{gen_use} {{"
        );
        w!(self.out, "    fn write(&self, __w: &mut tl::Buffer) {{");
        w!(self.out, "        tl::put(__w, 0x{:08x});", def.id);
        self.write_body(def);
        w!(self.out, "    }}\n}}");
        w!(self.out);
        w!(
            self.out,
            "impl{de_bound} tl::Deserialize for {rust}{gen_use} {{"
        );
        w!(
            self.out,
            "    fn read(__r: &mut tl::Reader<'_>) -> tl::Result<Self> {{"
        );
        w!(self.out, "        let __cons = __r.get()?;");
        w!(
            self.out,
            "        if __cons != 0x{:08x} {{\n            return Err(tl::Error::UnexpectedConstructor {{ id: __cons }});\n        }}",
            def.id
        );
        if fields.is_empty() {
            w!(self.out, "        Ok(Self)");
        } else {
            self.read_body(def);
        }
        w!(self.out, "    }}\n}}");
        w!(self.out);
        let fn_bound = if generic { "<X: tl::Function>" } else { "" };
        w!(
            self.out,
            "impl{fn_bound} tl::Function for {rust}{gen_use} {{\n    type Response = {response};\n}}\n"
        );
    }

    fn flag_casts(&mut self) {
        for &(child, parent) in &self.schema.flag_casts {
            let c = self.n.data_name(child);
            let p = self.n.data_name(parent);
            w!(
                self.out,
                "/// `mtpCastFlags(MTPD{}::Flags)`.",
                self.schema.constructors[child].cpp_name()
            );
            w!(self.out, "impl tl::FlagsInherit<{p}> for {c} {{}}");
        }
        w!(self.out);
    }

    /// Dispatchers over all types and methods used by the tests of
    /// `tg-scheme` (read a value of a given type, write it back).
    fn test_support(&mut self) {
        let o = &mut self.out;
        w!(
            o,
            "/// Reads a boxed value of type number `index` (see `meta::TYPES`) and"
        );
        w!(o, "/// writes it back. Test support.");
        w!(o, "#[doc(hidden)]");
        w!(
            o,
            "pub fn __reencode_type(index: usize, __r: &mut tl::Reader<'_>) -> Option<tl::Result<tl::Buffer>> {{"
        );
        w!(o, "    Some(match index {{");
        for t in 0..self.schema.types.len() {
            let ty = if self.n.is_bool(t) {
                "bool".to_string()
            } else {
                self.n.type_name(t)
            };
            w!(
                o,
                "        {t} => <{ty} as tl::Deserialize>::read(__r).map(|v| tl::serialize(&v)),"
            );
        }
        w!(o, "        _ => return None,\n    }})\n}}");
        w!(o);
        w!(
            o,
            "/// Serializes the default value of type number `index`. Test support."
        );
        w!(o, "#[doc(hidden)]");
        w!(
            o,
            "pub fn __default_type(index: usize) -> Option<tl::Buffer> {{"
        );
        w!(o, "    Some(match index {{");
        for t in 0..self.schema.types.len() {
            let ty = if self.n.is_bool(t) {
                "bool".to_string()
            } else {
                self.n.type_name(t)
            };
            w!(
                o,
                "        {t} => tl::serialize(&<{ty} as Default>::default()),"
            );
        }
        w!(o, "        _ => return None,\n    }})\n}}");
        w!(o);
        w!(
            o,
            "/// Reads method number `index` (see `meta::FUNCTIONS`; generic `X` is"
        );
        w!(o, "/// `bool`) and writes it back. Test support.");
        w!(o, "#[doc(hidden)]");
        w!(
            o,
            "pub fn __reencode_function(index: usize, __r: &mut tl::Reader<'_>) -> Option<tl::Result<tl::Buffer>> {{"
        );
        w!(o, "    Some(match index {{");
        for (f, def) in self.schema.functions.iter().enumerate() {
            let rust = self.n.function_name(f);
            let ty = if def.template.is_some() {
                format!("{rust}<bool>")
            } else {
                rust
            };
            w!(
                o,
                "        {f} => <{ty} as tl::Deserialize>::read(__r).map(|v| tl::serialize(&v)),"
            );
        }
        w!(o, "        _ => return None,\n    }})\n}}");
    }

    fn meta_type(&self, ty: &TypeRef) -> String {
        match ty {
            TypeRef::Primitive(p) => format!("Ty::{p:?}"),
            TypeRef::Boxed(t) => format!("Ty::Boxed({t})"),
            TypeRef::Bare(c) => format!("Ty::Bare({c})"),
            TypeRef::Vector { boxed, item } => {
                format!(
                    "Ty::Vector {{ boxed: {boxed}, item: &{} }}",
                    self.meta_type(item)
                )
            }
            TypeRef::Template => "Ty::Template".into(),
        }
    }

    fn meta_def(&mut self, def: &Definition) {
        let line = self.meta_def_line(def);
        w!(self.out, "{line}");
    }

    fn meta_def_line(&self, def: &Definition) -> String {
        let mut params = Vec::new();
        for p in &def.params {
            let kind = match &p.kind {
                ParamKind::Flags { wide } => format!("Kind::Flags {{ wide: {wide} }}"),
                ParamKind::True { bit } => format!("Kind::True {{ bit: {bit} }}"),
                ParamKind::Value { ty, bit } => format!(
                    "Kind::Value {{ ty: {}, bit: {} }}",
                    self.meta_type(ty),
                    match bit {
                        Some(b) => format!("Some({b})"),
                        None => "None".into(),
                    }
                ),
            };
            params.push(format!("Param {{ name: \"{}\", kind: {kind} }}", p.name));
        }
        format!(
            "    Def {{ name: \"{}\", cpp_name: \"{}\", id: 0x{:08x}, params: &[{}], result: {}, flags2: {} }},",
            def.name,
            def.cpp_name(),
            def.id,
            params.join(", "),
            self.meta_type(&def.result),
            match &def.flags64 {
                Some(f) => format!("Some(\"{f}\")"),
                None => "None".into(),
            }
        )
    }

    fn meta(&mut self) {
        w!(
            self.out,
            "// WARNING! All changes made in this file will be lost!"
        );
        w!(self.out, "// Created by tg-tl-gen.");
        w!(self.out);
        w!(self.out, "/// All constructors in scheme order.");
        w!(self.out, "pub static CONSTRUCTORS: &[Def] = &[");
        let schema = self.schema;
        for def in &schema.constructors {
            self.meta_def(def);
        }
        w!(self.out, "];");
        w!(self.out);
        w!(self.out, "/// All methods in scheme order.");
        w!(self.out, "pub static FUNCTIONS: &[Def] = &[");
        for def in &schema.functions {
            self.meta_def(def);
        }
        w!(self.out, "];");
        w!(self.out);
        w!(
            self.out,
            "/// All boxed types in order of their first constructor."
        );
        w!(self.out, "pub static TYPES: &[Type] = &[");
        for t in &self.schema.types {
            let ctors: Vec<String> = t.constructors.iter().map(|c| c.to_string()).collect();
            w!(
                self.out,
                "    Type {{ name: \"{}\", constructors: &[{}] }},",
                t.name,
                ctors.join(", ")
            );
        }
        w!(self.out, "];");
    }
}

fn upper_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
