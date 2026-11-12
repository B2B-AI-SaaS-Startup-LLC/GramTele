use super::{generator, Parser};
use crate::common::Result;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
pub fn process(inputs: &[PathBuf], includes: Vec<PathBuf>) -> Result<BTreeMap<String, String>> {
    let mut parser = Parser::new(includes);
    for input in inputs {
        parser.parse(Path::new(input))?;
    }
    let mut result = BTreeMap::new();
    for module in parser.cache.values() {
        result.insert(
            format!("{}.rs", generator::module_name(module)),
            generator::generate(module)?,
        );
    }
    Ok(result)
}
