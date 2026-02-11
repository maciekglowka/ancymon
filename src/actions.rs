use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Action {
    pub(crate) handler: String,
    pub(crate) event: String,
    pub(crate) emit: String,
    pub(crate) arguments: toml::Value,
}
