use super::{data, data_read, generator, replaces};
use crate::common::Result;
pub fn process(
    map: &[u8],
    suggestions: &[u8],
    file: &str,
    old: &[data_read::InputData],
) -> Result<String> {
    let data = data::prepare(&data_read::read_data(map)?, old)?;
    let replaces = replaces::check_and_convert(replaces::prepare(suggestions, file)?, &data, file)?;
    generator::generate(&data, &replaces)
}
