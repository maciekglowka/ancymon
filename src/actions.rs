use serde::Deserialize;

use crate::values::Value;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Action {
    pub(crate) handler: String,
    pub(crate) event: String,
    pub(crate) emit: String,
    pub(crate) arguments: Value,
    #[serde(default)]
    #[serde(rename = "accepted-input")]
    pub(crate) accepted_input: AcceptedInput,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub enum AcceptedInput {
    #[default]
    NotNull,
    Null,
    Ok,
    Err,
}
