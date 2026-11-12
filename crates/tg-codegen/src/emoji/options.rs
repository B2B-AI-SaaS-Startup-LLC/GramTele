use crate::common::{Error, Result};
#[derive(Default, Debug)]
pub struct Options {
    pub output: String,
    pub data: String,
    pub replaces: String,
    pub old: Vec<String>,
    pub images: Option<String>,
}
pub fn parse(args: &[String]) -> Result<Options> {
    let mut o = Options {
        output: ".".into(),
        ..Options::default()
    };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--images" {
            i += 1;
            o.images = Some(args.get(i).cloned().ok_or_else(|| {
                Error::new(
                    "Command Line",
                    0,
                    902,
                    "images argument expected after --images",
                )
            })?);
        } else if let Some(rest) = a.strip_prefix("-o") {
            o.output = if rest.is_empty() {
                i += 1;
                args.get(i).cloned().ok_or_else(|| {
                    Error::new("Command Line", 0, 902, "output path expected after -o")
                })?
            } else {
                rest.into()
            };
        } else if o.data.is_empty() {
            o.data = a.clone();
        } else if o.replaces.is_empty() {
            o.replaces = a.clone();
        } else {
            o.old.push(a.clone());
        }
        i += 1;
    }
    if o.output.is_empty() {
        return Err(Error::new("Command Line", 0, 902, "output path expected"));
    }
    if o.replaces.is_empty() {
        return Err(Error::new("Command Line", 0, 903, "replaces path expected"));
    }
    Ok(o)
}
