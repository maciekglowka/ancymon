use serde::Deserialize;

use crate::values::Value;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Action {
    pub(crate) handler: String,
    pub(crate) event: String,
    pub(crate) emit: String,
    #[serde(default)]
    pub(crate) arguments: Value,

    #[serde(default)]
    #[serde(rename = "accepted-input")]
    pub(crate) accepted_input: AcceptedInput,

    #[serde(default)]
    #[serde(rename = "max-retries")]
    pub(crate) max_retries: usize,
    #[serde(default)]
    #[serde(rename = "retry-delay")]
    pub(crate) retry_delay: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub enum AcceptedInput {
    #[default]
    NotNull,
    Null,
    Ok,
    Err,
}
