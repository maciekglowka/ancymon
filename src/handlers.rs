#[derive(Clone, Debug)]
pub(crate) struct Handler {
    pub(crate) event: String,
    pub(crate) function: mlua::Function,
    // TODO on error
    // pub(crate) max_retries: usize,
    // pub(crate) retry_delay: u64,
}
impl mlua::FromLua for Handler {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(t) => Ok(Self {
                event: t.get("event")?,
                function: t.get("function")?,
                // max_retries: t.get("max_retries").unwrap_or_default(),
                // retry_delay: t.get("retry_delay").unwrap_or_default(),
            }),
            v => Err(mlua::Error::FromLuaConversionError {
                // FIXME
                from: std::any::type_name_of_val(&v),
                to: "Handler".to_string(),
                message: Some("Expected table".to_string()),
            }),
        }
    }
}
