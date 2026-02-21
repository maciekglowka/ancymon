use std::collections::HashMap;

use crate::{errors::AncymonError, values::Value};

#[derive(Clone, Debug)]
pub struct Event {
    pub(crate) meta: EventMeta,
    pub(crate) name: String,
    pub(crate) value: Result<Value, Value>,
}
impl Event {
    /// Spawn an event with fresh meta, timestamped at current time.
    ///
    /// Meant for use on flow start inside of triggers.
    pub fn initial(name: String, value: Result<Value, Value>) -> Self {
        let mut meta = EventMeta(HashMap::new());
        let ts = chrono::Utc::now().timestamp();
        meta.0.insert("timestamp".to_string(), Value::Integer(ts));
        Self { name, value, meta }
    }
    /// Spawn an event with inherited meta.
    ///
    /// Meant for use as a subsequent eent in the handlers.
    pub fn with_meta(name: String, value: Result<Value, Value>, meta: EventMeta) -> Self {
        Self { name, value, meta }
    }
}

#[derive(Clone, Debug)]
pub struct EventMeta(HashMap<String, Value>);
impl EventMeta {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }
    pub fn insert(&mut self, key: String, value: Value) {
        self.0.insert(key, value);
    }
    #[cfg(test)]
    /// Empty meta, used as a test fixture only
    pub fn dummy() -> Self {
        Self(HashMap::new())
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
pub fn unpack_error(value: &Value) -> Option<(&Value, &Value)> {
    let map = value.as_map()?;
    Some((map.get("value")?, map.get("error")?))
}
