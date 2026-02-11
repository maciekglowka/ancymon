use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct Event {
    #[serde(rename = "query-source")]
    pub(crate) query_source: String,
    pub(crate) arguments: toml::Value,
}
