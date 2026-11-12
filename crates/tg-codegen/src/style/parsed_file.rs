use super::{module::Module, structure_types::*};
use crate::common::{Error, Kind, Result, TokenizedFile};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
#[derive(Default)]
pub struct Parser {
    pub includes: Vec<PathBuf>,
    pub cache: BTreeMap<PathBuf, Arc<Module>>,
}
impl Parser {
    pub fn new(includes: Vec<PathBuf>) -> Self {
        Self {
            includes,
            ..Default::default()
        }
    }
    pub fn parse(&mut self, path: &Path) -> Result<Arc<Module>> {
        self.read(path, &mut Vec::new())
    }
    fn read(&mut self, path: &Path, stack: &mut Vec<PathBuf>) -> Result<Arc<Module>> {
        let found = if path.is_file() {
            path.to_path_buf()
        } else {
            self.includes
                .iter()
                .map(|p| p.join(path))
                .find(|p| p.is_file())
                .unwrap_or_else(|| path.to_path_buf())
        };
        let path = std::fs::canonicalize(&found).map_err(|_| {
            Error::new(
                found.display().to_string(),
                0,
                101,
                ": error: file does not exist.",
            )
        })?;
        if stack.contains(&path) {
            return Err(Error::new(
                path.display().to_string(),
                1,
                809,
                "include cycle detected.",
            ));
        }
        if let Some(m) = self.cache.get(&path) {
            return Ok(m.clone());
        }
        let bytes = std::fs::read(&path)
            .map_err(|_| Error::new(path.display().to_string(), 0, 103, "' for read."))?;
        stack.push(path.clone());
        let mut f = TokenizedFile::parse_lazy(&bytes, &path.display().to_string())?;
        let mut m = Module {
            path: path.display().to_string(),
            palette: path.extension().is_some_and(|x| x == "palette"),
            ..Default::default()
        };
        while f.peek().kind != Kind::End {
            let name = f.expect(Kind::Name)?.value;
            if name == "using" {
                let include = f.expect(Kind::String)?.value;
                f.expect(Kind::Symbol(';'))?;
                let local = path.parent().unwrap().join(&include);
                let target = if local.is_file() {
                    local
                } else {
                    PathBuf::from(&include)
                };
                m.includes.push(self.read(&target, stack)?);
                continue;
            }
            if f.eat(Kind::Symbol('{')) {
                if m.palette {
                    return Err(f.unexpected("unique color variable for the palette"));
                }
                if m.find_struct(&name).is_some() {
                    return Err(f.error(805, format!("struct '{name}' already defined")));
                }
                let mut fields = Vec::new();
                while !f.eat(Kind::Symbol('}')) {
                    let field = f.expect(Kind::Name)?.value;
                    f.expect(Kind::Symbol(':'))?;
                    let ty = f.expect(Kind::Name)?.value;
                    let ty = if let Some(ty) = Type::builtin(&ty) {
                        ty
                    } else if m.find_struct(&ty).is_some() {
                        Type::Struct(ty)
                    } else {
                        return Err(f.error(804, format!("type name '{ty}' not found")));
                    };
                    f.expect(Kind::Symbol(';'))?;
                    fields.push((field, ty));
                }
                if fields.is_empty() {
                    return Err(f.unexpected("at least one field in struct"));
                }
                m.structs.push(Struct { name, fields });
            } else if f.eat(Kind::Symbol(':')) {
                if m.find_variable(&name).is_some() {
                    return Err(f.error(805, format!("variable '{name}' already defined")));
                }
                let variable = self.variable(&mut f, &m, name)?;
                if m.palette && variable.value.ty != Type::Color {
                    return Err(f.unexpected("unique color variable for the palette"));
                }
                m.variables.push(variable);
            } else {
                return Err(
                    f.unexpected("using keyword, or struct definition, or variable definition")
                );
            }
        }
        f.finish()?;
        stack.pop();
        let result = Arc::new(m);
        self.cache.insert(path, result.clone());
        Ok(result)
    }
    fn variable(&self, f: &mut TokenizedFile, m: &Module, name: String) -> Result<Variable> {
        let value = self.value(f, m)?;
        let mut description = String::new();
        if !matches!(value.ty, Type::Struct(_)) || !value.copy_of.is_empty() {
            f.expect(Kind::Symbol(';'))?;
            let line = f.tokens[f.position.saturating_sub(1)].line;
            if let Some(comment) = f.comments.get(line.saturating_sub(1)) {
                let bytes = comment.get(2..).unwrap_or_default();
                description = crate::common::checked_utf8_string::checked(bytes)
                    .ok_or_else(|| f.error(202, "incorrect UTF-8 string in the comment."))?
                    .trim()
                    .to_owned();
            }
        }
        Ok(Variable {
            name,
            value,
            description,
        })
    }
    fn value(&self, f: &mut TokenizedFile, m: &Module) -> Result<Value> {
        let mut v = self.atom(f, m)?;
        if matches!(v.ty, Type::Int | Type::Pixels | Type::Double) {
            while matches!(f.peek().kind, Kind::Symbol('+') | Kind::Symbol('-')) {
                let plus = f.take().value == "+";
                let right = self.atom(f, m)?;
                if right.ty != v.ty {
                    return Err(f.error(
                        802,
                        format!(
                            "type mismatch: cannot {} different types",
                            if plus { "add" } else { "subtract" }
                        ),
                    ));
                }
                let n = if plus {
                    v.number() + right.number()
                } else {
                    v.number() - right.number()
                };
                v = Value::new(v.ty, Data::Number(n));
            }
        }
        Ok(v)
    }
    fn atom(&self, f: &mut TokenizedFile, m: &Module) -> Result<Value> {
        let token = f.take();
        let text = token.value;
        match token.kind {
            Kind::Symbol('-') => {
                let v = self.value(f, m)?; // Original uses Int() even for double, retaining its zero conversion.
                let n = if v.ty == Type::Double {
                    0.0
                } else {
                    -v.number()
                };
                Ok(Value::new(v.ty, Data::Number(n)))
            }
            Kind::Int | Kind::Double => Ok(Value::new(
                if token.kind == Kind::Int {
                    Type::Int
                } else {
                    Type::Double
                },
                Data::Number(if token.kind == Kind::Int {
                    text.parse::<i32>().unwrap_or(0) as f64
                } else {
                    text.parse().unwrap_or(0.0)
                }),
            )),
            Kind::String => {
                if !text.is_ascii() {
                    return Err(f.error(806, "unicode symbols are not supported"));
                }
                Ok(Value::new(Type::String, Data::Text(text)))
            }
            Kind::Symbol('#') => {
                if !m.palette {
                    return Err(f.unexpected(
                        "color value alias, unique color values are only allowed in palette module",
                    ));
                }
                let hex = f.take().value.to_ascii_lowercase();
                if ![6, 8].contains(&hex.len()) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(
                        f.unexpected("color value in #ccc, #ccca, #cccccc or #ccccccaa format")
                    );
                }
                let mut rgba = [0, 0, 0, 255];
                for (i, c) in hex.as_bytes().chunks_exact(2).enumerate() {
                    rgba[i] = u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap();
                }
                let fallback = if f.eat(Kind::Symbol('|')) {
                    let name = f.expect(Kind::Name)?.value;
                    if !m.variables.iter().any(|v| v.name == name) {
                        return Err(f.error(804, "fallback color name"));
                    }
                    name
                } else {
                    String::new()
                };
                Ok(Value::new(Type::Color, Data::Color { rgba, fallback }))
            }
            Kind::Name => {
                if let Some(n) = text
                    .strip_suffix("px")
                    .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                {
                    return Ok(Value::new(
                        Type::Pixels,
                        Data::Number(n.parse::<i32>().unwrap_or(0) as f64),
                    ));
                }
                match text.as_str() {
                    "true" | "false" => {
                        return Ok(Value::new(Type::Bool, Data::Bool(text == "true")))
                    }
                    "transparent" => {
                        return Ok(Value::new(
                            Type::Color,
                            Data::Color {
                                rgba: [255, 255, 255, 0],
                                fallback: String::new(),
                            },
                        ))
                    }
                    "point" | "size" | "margins" => {
                        f.expect(Kind::Symbol('('))?;
                        let count = if text == "margins" { 4 } else { 2 };
                        let mut parts = Vec::new();
                        for i in 0..count {
                            let v = self.value(f, m)?;
                            if v.ty != Type::Pixels {
                                return Err(f.error(
                                    802,
                                    format!(
                                        "type mismatch: expected {} px values for the {text}",
                                        if count == 4 { "four" } else { "two" }
                                    ),
                                ));
                            }
                            parts.push(v.number() as i32);
                            if i + 1 < count {
                                f.expect(Kind::Symbol(','))?;
                            }
                        }
                        f.expect(Kind::Symbol(')'))?;
                        return Ok(Value::new(
                            Type::builtin(&text).unwrap(),
                            Data::Tuple(parts),
                        ));
                    }
                    "align" => {
                        f.expect(Kind::Symbol('('))?;
                        let name = f.expect(Kind::Name)?.value;
                        f.expect(Kind::Symbol(')'))?;
                        if name.is_empty()
                            || !name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
                        {
                            return Err(f.error(806, "bad align string"));
                        }
                        return Ok(Value::new(Type::Align, Data::Text(name)));
                    }
                    "font" => {
                        f.expect(Kind::Symbol('('))?;
                        let (mut flags, mut family, mut size) = (0, String::new(), None);
                        while !f.eat(Kind::Symbol(')')) {
                            match f.peek().value.as_str() {
                                "bold" | "semibold" => {
                                    flags |= 1;
                                    f.take();
                                }
                                "italic" => {
                                    flags |= 2;
                                    f.take();
                                }
                                "underline" => {
                                    flags |= 4;
                                    f.take();
                                }
                                _ => {
                                    let v = self.value(f, m)?;
                                    match v.ty {
                                        Type::String => family = v.text().into(),
                                        Type::Pixels => size = Some(v.number() as i32),
                                        _ => return Err(f.error(
                                            802,
                                            "type mismatch: px value for the font size expected",
                                        )),
                                    }
                                }
                            }
                        }
                        return Ok(Value::new(
                            Type::Font,
                            Data::Font {
                                family,
                                size: size.ok_or_else(|| {
                                    f.error(
                                        802,
                                        "type mismatch: px value for the font size expected",
                                    )
                                })?,
                                flags,
                            },
                        ));
                    }
                    "icon" => {
                        let mut parts = Vec::new();
                        if f.eat(Kind::Symbol('(')) {
                            parts.push(self.icon_part(f, m)?);
                            f.expect(Kind::Symbol(')'))?;
                        } else if f.eat(Kind::Symbol('{')) {
                            while !f.eat(Kind::Symbol('}')) {
                                f.expect(Kind::Symbol('{'))?;
                                parts.push(self.icon_part(f, m)?);
                                f.expect(Kind::Symbol('}'))?;
                                f.eat(Kind::Symbol(','));
                            }
                        }
                        return Ok(Value::new(Type::Icon, Data::Icon(parts)));
                    }
                    _ => {}
                }
                if let Some(pattern) = m.find_struct(&text) {
                    let mut fields: Vec<_> = pattern
                        .fields
                        .iter()
                        .map(|(n, ty)| Field {
                            name: n.clone(),
                            value: Value::uninitialized(ty.clone()),
                            status: Status::Uninitialized,
                        })
                        .collect();
                    if f.eat(Kind::Symbol('(')) {
                        loop {
                            let parent = f.expect(Kind::Name)?.value;
                            let (p, local) = m.find_variable(&parent).ok_or_else(|| {
                                f.error(804, format!("parent '{parent}' not found"))
                            })?;
                            if p.value.ty != Type::Struct(text.clone()) {
                                return Err(f.error(802,format!("type mismatch: parent '{parent}' has type '{}' while child value has type struct {text}",p.value.ty.label())));
                            }
                            if let Data::Struct(source) = &p.value.data {
                                for (dst, src) in fields.iter_mut().zip(source) {
                                    if src.status == Status::Explicit
                                        || dst.status == Status::Uninitialized
                                    {
                                        dst.value = src.value.clone();
                                        dst.status = if local {
                                            Status::Implicit
                                        } else {
                                            Status::ImplicitOtherModule
                                        };
                                    }
                                }
                            }
                            if f.eat(Kind::Symbol(')')) {
                                break;
                            }
                            f.expect(Kind::Symbol(','))?;
                        }
                    }
                    f.expect(Kind::Symbol('{'))?;
                    while !f.eat(Kind::Symbol('}')) {
                        let name = f.expect(Kind::Name)?.value;
                        f.expect(Kind::Symbol(':'))?;
                        let v = self.variable(f, m, name.clone())?;
                        let field=fields.iter_mut().find(|x|x.name==name).ok_or_else(||f.error(803,format!("field '{name}' was not found in struct of type 'struct {text}'")))?;
                        if field.value.ty != v.value.ty {
                            return Err(f.error(802,format!("type mismatch: field '{name}' has type '{}' while value has type '{}'",field.value.ty.label(),v.value.ty.label())));
                        }
                        field.value = v.value;
                        field.status = Status::Explicit;
                    }
                    for field in &fields {
                        if field.status == Status::ImplicitOtherModule
                            && field.value.ty == Type::Icon
                            && field.value.copy_of.is_empty()
                            && matches!(&field.value.data,Data::Icon(p)if !p.is_empty())
                        {
                            return Err(f.error(
                                807,
                                format!(
                                    "an unnamed icon field '{}' is inherited from parent.",
                                    field.name
                                ),
                            ));
                        }
                    }
                    return Ok(Value::new(Type::Struct(text), Data::Struct(fields)));
                }
                let (variable, _) = m
                    .find_variable(&text)
                    .ok_or_else(|| f.unexpected("variable value"))?;
                let mut v = variable.value.clone();
                let mut copy = text.clone();
                while f.eat(Kind::Symbol('.')) {
                    let field = f.expect(Kind::Name)?.value;
                    match &v.data {
                        Data::Tuple(parts) => {
                            let index = match (&v.ty, field.as_str()) {
                                (Type::Point, "x")
                                | (Type::Size, "width")
                                | (Type::Margins, "left") => 0,
                                (Type::Point, "y")
                                | (Type::Size, "height")
                                | (Type::Margins, "top") => 1,
                                (Type::Margins, "right") => 2,
                                (Type::Margins, "bottom") => 3,
                                (Type::Margins, "leftRight") => 4,
                                (Type::Margins, "topBottom") => 5,
                                _ => {
                                    return Err(f.error(
                                        803,
                                        format!(
                                            "{} has only {} fields",
                                            v.ty.label(),
                                            match v.ty {
                                                Type::Size => "'width' and 'height'",
                                                Type::Point => "'x' and 'y'",
                                                _ => "'left', 'top', 'right' and 'bottom'",
                                            }
                                        ),
                                    ))
                                }
                            };
                            let n = if index < 4 {
                                parts[index]
                            } else if index == 4 {
                                parts[0] + parts[2]
                            } else {
                                parts[1] + parts[3]
                            };
                            return Ok(Value::new(Type::Pixels, Data::Number(n as f64)));
                        }
                        Data::Struct(fields) => {
                            v = fields
                                .iter()
                                .find(|x| x.name == field)
                                .ok_or_else(|| f.error(803, format!("field '{field}' not found")))?
                                .value
                                .clone();
                            copy.push('.');
                            copy.push_str(&field);
                        }
                        _ => return Err(f.error(802, format!("'{text}' is not a struct"))),
                    }
                }
                v.copy_of = copy;
                Ok(v)
            }
            _ => Err(f.unexpected("variable value")),
        }
    }
    fn icon_part(&self, f: &mut TokenizedFile, m: &Module) -> Result<MonoIcon> {
        let v = self.value(f, m)?;
        let filename = if v.ty == Type::String {
            let full = v.text();
            let mut pieces = full.split('-');
            let name = pieces.next().unwrap();
            for modifier in pieces {
                if !matches!(
                    modifier,
                    "invert" | "flip_horizontal" | "flip_vertical" | "rotate_cw" | "rotate_ccw"
                ) && !modifier.split_once('x').is_some_and(|(w, h)| {
                    w.parse::<u32>().is_ok_and(|x| x > 0) && h.parse::<u32>().is_ok_and(|x| x > 0)
                }) {
                    return Err(f.error(808, format!("unknown modifier: {modifier}")));
                }
            }
            let mut found = None;
            for sub in ["", "icons"] {
                for p in &self.includes {
                    let p = p.join(sub).join(name.trim_start_matches('/'));
                    if PathBuf::from(format!("{}.png", p.display())).is_file()
                        || PathBuf::from(format!("{}.svg", p.display())).is_file()
                    {
                        found = Some(
                            p.parent()
                                .unwrap()
                                .join(Path::new(full).file_name().unwrap())
                                .display()
                                .to_string(),
                        );
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            found.ok_or_else(|| f.error(101, format!("could not open icon file '{full}'")))?
        } else if let Data::Tuple(parts) = &v.data {
            if v.ty != Type::Size {
                return Err(f.unexpected("icon filename or rect size"));
            }
            format!("size://{},{}", parts[0], parts[1])
        } else {
            return Err(f.unexpected("icon filename or rect size"));
        };
        let mut color = Value::new(Type::Color, Data::Default);
        let mut padding = [0; 4];
        if f.eat(Kind::Symbol(',')) {
            color = self.value(f, m)?;
            if color.ty != Type::Color {
                return Err(f.unexpected("icon color"));
            }
            if f.eat(Kind::Symbol(',')) {
                let v = self.value(f, m)?;
                if let Data::Tuple(parts) = v.data {
                    if v.ty != Type::Margins && v.ty != Type::Point {
                        return Err(f.unexpected("icon offset"));
                    }
                    padding[..parts.len()].copy_from_slice(&parts);
                } else {
                    return Err(f.unexpected("icon offset"));
                }
            }
        }
        Ok(MonoIcon {
            filename,
            color,
            padding,
        })
    }
}
