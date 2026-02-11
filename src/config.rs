use serde::Deserialize;
use std::collections::HashMap;
use toml::Table;

use crate::{errors::AncymonError, event::Event};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    #[serde(rename = "query-sources")]
    pub(crate) query_sources: HashMap<String, Table>,
    #[serde(default)]
    pub(crate) events: Vec<Event>,
}
impl Config {
    pub fn new(s: &str) -> Result<Self, AncymonError> {
        toml::from_str(s).map_err(|_| AncymonError::ConfigError)
    }
}
