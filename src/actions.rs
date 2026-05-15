use serde::Deserialize;

use crate::values::Value;

#[derive(Clone, Debug)]
pub(crate) struct Action {
    pub(crate) handler: String,
    pub(crate) event: String,
    pub(crate) emit: String,
    pub(crate) arguments: Value,
    pub(crate) accepted_input: AcceptedInput,
    pub(crate) max_retries: usize,
    pub(crate) retry_delay: u64,
}
impl mlua::FromLua for Action {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(t) => Ok(Action {
                handler: t.get("handler")?,
                event: t.get("event")?,
                emit: t.get("emit")?,
                arguments: t.get("arguments").unwrap_or_default(),
                accepted_input: t.get("accepted_input").unwrap_or_default(),
                max_retries: t.get("max_retries").unwrap_or_default(),
                retry_delay: t.get("retry_delay").unwrap_or_default(),
            }),
            v => Err(mlua::Error::FromLuaConversionError {
                // FIXME
                from: std::any::type_name_of_val(&v),
                to: "Action".to_string(),
                message: Some("Expected table".to_string()),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub enum AcceptedInput {
    #[default]
    NotNull,
    Null,
    Ok,
    Err,
}
impl TryFrom<&str> for AcceptedInput {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "NotNull" => Ok(Self::NotNull),
            "Null" => Ok(Self::Null),
            "Ok" => Ok(Self::Ok),
            "Err" => Ok(Self::Err),
            v => Err(format!("Invalid str value for AcceptedInput variant {v}")),
        }
    }
}
impl mlua::FromLua for AcceptedInput {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::String(s) => s.to_string_lossy().as_str().try_into().map_err(|e| {
                mlua::Error::FromLuaConversionError {
                    from: "Value::String",
                    to: "AcceptedInput".to_string(),
                    message: Some(e),
                }
            }),
            v => Err(mlua::Error::FromLuaConversionError {
                // FIXME
                from: std::any::type_name_of_val(&v),
                to: "AcceptedInput".to_string(),
                message: Some("Expected string".to_string()),
            }),
        }
    }
}
