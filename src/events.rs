use std::collections::HashMap;

use crate::{errors::AncymonError, values::Value};

#[derive(Clone, Debug)]
pub struct Event {
    pub(crate) name: String,
    pub(crate) value: Result<Value, Value>,
}
impl Event {
    pub fn new(name: String, value: Result<Value, Value>) -> Self {
        Self { name, value }
    }
}

pub fn pack_error(value: Value, error: AncymonError) -> Value {
    Value::Map(HashMap::from_iter(vec![
        ("value".to_string(), value),
        ("error".to_string(), Value::String(format!("{error}"))),
    ]))
}

/// Unpack event error value
///
/// If successful returns (original_value, error_message_str)
pub fn unpack_error<'a>(value: &'a Value) -> Option<(&'a Value, &'a Value)> {
    let map = value.as_map()?;
    Some((map.get("value")?, map.get("error")?))
}
