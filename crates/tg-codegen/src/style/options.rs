use crate::common::{Error, Result};
use std::path::PathBuf;
#[derive(Default, Debug, Clone)]
pub struct Options {
    pub inputs: Vec<PathBuf>,
    pub includes: Vec<PathBuf>,
    pub output: PathBuf,
    pub timestamp: PathBuf,
    pub render_svg: Option<(PathBuf, PathBuf, Option<u32>)>,
}
pub fn parse(args: &[String]) -> Result<Options> {
    let mut out = Options {
        output: ".".into(),
        includes: vec![".".into()],
        ..Default::default()
    };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--render-svg" {
            if i + 2 >= args.len() {
                return Err(Error::new(
                    "Command Line",
                    0,
                    903,
                    "expected: --render-svg input.svg output.png [size]",
                ));
            }
            let input = args[i + 1].clone().into();
            let output = args[i + 2].clone().into();
            i += 2;
            let size = args
                .get(i + 1)
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|&n| n > 0);
            if size.is_some() {
                i += 1;
            }
            out.render_svg = Some((input, output, size));
        } else if ["-I", "-o", "-t", "-w"].iter().any(|p| a.starts_with(p)) {
            let p = &a[..2];
            let v = if a.len() > 2 {
                a[2..].to_owned()
            } else {
                i += 1;
                args.get(i).cloned().ok_or_else(|| {
                    Error::new(
                        "Command Line",
                        0,
                        match p {
                            "-I" => 901,
                            "-w" => 905,
                            _ => 902,
                        },
                        format!(
                            "{} path expected after {p}",
                            match p {
                                "-I" => "include",
                                "-w" => "working",
                                "-t" => "timestamp",
                                _ => "output",
                            }
                        ),
                    )
                })?
            };
            match p {
                "-I" => out.includes.push(v.into()),
                "-o" => out.output = v.into(),
                "-t" => out.timestamp = v.into(),
                _ => {}
            }
        } else {
            out.inputs.push(a.into());
        }
        i += 1;
    }
    if out.render_svg.is_none() {
        if out.timestamp.as_os_str().is_empty() {
            return Err(Error::new(
                "Command Line",
                0,
                903,
                "timestamp path expected",
            ));
        }
        if out.inputs.is_empty() {
            return Err(Error::new("Command Line", 0, 903, "input path expected"));
        }
    }
    Ok(out)
}
