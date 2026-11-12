use super::structure_types::*;
use std::sync::Arc;
#[derive(Clone, Debug, Default)]
pub struct Module {
    pub path: String,
    pub includes: Vec<Arc<Module>>,
    pub structs: Vec<Struct>,
    pub variables: Vec<Variable>,
    pub palette: bool,
}
impl Module {
    pub fn find_struct(&self, name: &str) -> Option<&Struct> {
        self.structs
            .iter()
            .find(|s| s.name == name)
            .or_else(|| self.includes.iter().find_map(|m| m.find_struct(name)))
    }
    pub fn find_variable(&self, name: &str) -> Option<(&Variable, bool)> {
        self.variables
            .iter()
            .find(|v| v.name == name)
            .map(|v| (v, true))
            .or_else(|| {
                self.includes
                    .iter()
                    .find_map(|m| m.find_variable(name).map(|(v, _)| (v, false)))
            })
    }
}
