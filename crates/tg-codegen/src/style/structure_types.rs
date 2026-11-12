#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Int,
    Bool,
    Double,
    Pixels,
    String,
    Color,
    Point,
    Size,
    Align,
    Margins,
    Font,
    Icon,
    Struct(String),
}
impl Type {
    pub fn builtin(s: &str) -> Option<Self> {
        Some(match s {
            "int" => Self::Int,
            "bool" => Self::Bool,
            "double" => Self::Double,
            "pixels" => Self::Pixels,
            "string" => Self::String,
            "color" => Self::Color,
            "point" => Self::Point,
            "size" => Self::Size,
            "align" => Self::Align,
            "margins" => Self::Margins,
            "font" => Self::Font,
            "icon" => Self::Icon,
            _ => return None,
        })
    }
    pub fn label(&self) -> String {
        match self {
            Self::Struct(n) => format!("struct {n}"),
            _ => format!("{self:?}").to_lowercase(),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    pub ty: Type,
    pub data: Data,
    pub copy_of: String,
}
impl Value {
    pub fn new(ty: Type, data: Data) -> Self {
        Self {
            ty,
            data,
            copy_of: String::new(),
        }
    }
    /// Default values of Value(Type, Qt::Uninitialized), not Rust UI defaults.
    pub fn uninitialized(ty: Type) -> Self {
        let data = match &ty {
            Type::Int | Type::Double | Type::Pixels => Data::Number(0.0),
            Type::Bool => Data::Bool(false),
            Type::String => Data::Text(String::new()),
            Type::Color => Data::Color {
                rgba: [0, 0, 0, 255],
                fallback: String::new(),
            },
            Type::Point | Type::Size => Data::Tuple(vec![0, 0]),
            Type::Align => Data::Text("topleft".into()),
            Type::Margins => Data::Tuple(vec![0, 0, 0, 0]),
            Type::Font => Data::Font {
                family: String::new(),
                size: 13,
                flags: 0,
            },
            Type::Icon => Data::Icon(Vec::new()),
            Type::Struct(_) => Data::Struct(Vec::new()),
        };
        Self::new(ty, data)
    }
    pub fn number(&self) -> f64 {
        match self.data {
            Data::Number(n) => n,
            _ => 0.0,
        }
    }
    pub fn text(&self) -> &str {
        match &self.data {
            Data::Text(s) => s,
            _ => "",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    Default,
    Number(f64),
    Bool(bool),
    Text(String),
    Color {
        rgba: [u8; 4],
        fallback: String,
    },
    Tuple(Vec<i32>),
    Font {
        family: String,
        size: i32,
        flags: u8,
    },
    Icon(Vec<MonoIcon>),
    Struct(Vec<Field>),
}
#[derive(Clone, Debug, PartialEq)]
pub struct MonoIcon {
    pub filename: String,
    pub color: Value,
    pub padding: [i32; 4],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Uninitialized,
    Implicit,
    ImplicitOtherModule,
    Explicit,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub value: Value,
    pub status: Status,
}
#[derive(Clone, Debug)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<(String, Type)>,
}
#[derive(Clone, Debug)]
pub struct Variable {
    pub name: String,
    pub value: Value,
    pub description: String,
}
