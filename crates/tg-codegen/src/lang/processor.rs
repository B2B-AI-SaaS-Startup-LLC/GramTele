use super::{generate, parse, Generated};
use crate::common::Result;
pub fn process(input: &[u8], file: &str, known_tags: &str, indices: &str) -> Result<Generated> {
    let tags: Vec<_> = known_tags
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    generate(&parse(input, file, &tags)?, indices, file)
}
