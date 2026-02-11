use serde::Deserialize;
use std::collections::HashMap;
use toml::Table;

use crate::{
    actions::Action,
    errors::{AncymonError, ConfigError},
    triggers::Trigger,
};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub(crate) sources: HashMap<String, Table>,
    pub(crate) handlers: HashMap<String, Table>,
    pub(crate) actions: Vec<Action>,
    pub(crate) triggers: Vec<Trigger>,
}
impl Config {
    pub fn new(s: &str) -> Result<Self, AncymonError> {
        toml::from_str(s).map_err(|_| ConfigError::ParsingError.into())
    }
}
