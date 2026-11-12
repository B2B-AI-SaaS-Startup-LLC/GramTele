use crate::common::{Error, Result};
#[derive(Default, Debug)]
pub struct Options {
    pub input: String,
    pub output: String,
    pub sources: String,
    pub subsets_only: bool,
}
pub fn parse(args: &[String]) -> Result<Options> {
    let mut out = Options {
        output: ".".into(),
        ..Default::default()
    };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--subsets-only" {
            out.subsets_only = true;
        } else if ["-o", "-w", "-s"].iter().any(|p| a.starts_with(p)) {
            let option = &a[..2];
            let value = if a.len() > 2 {
                a[2..].to_owned()
            } else {
                i += 1;
                args.get(i).cloned().ok_or_else(|| {
                    Error::new(
                        "Command Line",
                        0,
                        if option == "-s" {
                            906
                        } else if option == "-w" {
                            905
                        } else {
                            902
                        },
                        format!(
                            "{} path expected after {option}",
                            if option == "-s" {
                                "sources"
                            } else if option == "-w" {
                                "working"
                            } else {
                                "output"
                            }
                        ),
                    )
                })?
            };
            match option {
                "-o" => out.output = value,
                "-s" => out.sources = value,
                _ => {}
            }
        } else if out.input.is_empty() {
            out.input = a.clone();
        } else {
            return Err(Error::new(
                "Command Line",
                0,
                904,
                "only one input path expected",
            ));
        }
        i += 1;
    }
    if out.input.is_empty() {
        return Err(Error::new("Command Line", 0, 903, "input path expected"));
    }
    Ok(out)
}
