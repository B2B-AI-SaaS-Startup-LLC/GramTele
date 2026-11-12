// Minimal compile contracts, ONLY for generated-code tests. Not a UI/runtime.
extern crate self as tg_ui;
extern crate self as tg_core;
pub mod style {
 pub fn convert_scale(value:i32)->i32{value}
 pub fn wide_multiplied(value:i32,_:f64)->i32{value}
 #[derive(Clone,Default)] pub struct Color;
 impl Color {pub fn named(_:&str)->Self{Self}pub fn palette(_:usize)->Self{Self}pub fn rgba(_:[u8;4])->Self{Self}}
 pub struct PaletteColor {pub name:&'static str,pub rgba:[u8;4],pub fallback:i32}
 #[derive(Clone,Default)] pub struct Point; impl Point{pub fn new(_:i32,_:i32)->Self{Self}}
 #[derive(Clone,Default)] pub struct Size; impl Size{pub fn new(_:i32,_:i32)->Self{Self}}
 #[derive(Clone,Default)] pub struct Margins; impl Margins{pub fn new(_:i32,_:i32,_:i32,_:i32)->Self{Self}}
 #[derive(Clone,Default)] pub struct Align; impl Align{pub fn named(_:&str)->Self{Self}}
 #[derive(Clone,Default)] pub struct Font; impl Font{pub fn new(_:&str,_:i32,_:u8)->Self{Self}}
 #[derive(Clone,Default)] pub struct Icon; impl Icon{pub fn from_sources(_:Vec<IconSource>)->Self{Self}}
 #[derive(Clone)] pub struct IconSource{pub bytes:&'static[u8],pub bytes_2x:&'static[u8],pub bytes_3x:&'static[u8],pub format:&'static str,pub modifiers:Vec<&'static str>,pub color:Color,pub padding:[i32;4],pub size:[i32;2]}
}
pub mod lang {
 pub struct Entry{pub key:&'static str,pub index:u16,pub value:&'static[u16],pub tags:&'static[u16],pub plural_base:&'static str}
 pub struct Phrase;impl Phrase{pub const fn new(_:u16,_:&'static[u16],_:bool)->Self{Self}}
}
pub mod emoji {
 pub struct Emoji{pub id:&'static[u16],pub postfixed:bool,pub variated:bool,pub colored:bool}
 pub struct Replacement{pub id:&'static[u16],pub replacement:&'static str,pub words:&'static[&'static str],pub checksum:u32}
}
