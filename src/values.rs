use serde::{de, Deserialize};
use std::collections::HashMap;

use crate::errors::AncymonError;

#[derive(Clone, Default, Debug, PartialEq)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
}
impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Bool(b) = self {
            return Some(*b);
        }
        None
    }
    pub fn as_int(&self) -> Option<i64> {
        if let Value::Integer(i) = self {
            return Some(*i);
        }
        None
    }
    pub fn as_float(&self) -> Option<f64> {
        if let Value::Float(f) = self {
            return Some(*f);
        }
        None
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Value::String(s) = self {
            return Some(s);
        }
        None
    }
    pub fn as_array(&self) -> Option<&Vec<Self>> {
        if let Value::Array(a) = self {
            return Some(a);
        }
        None
    }
    pub fn as_map(&self) -> Option<&HashMap<String, Self>> {
        if let Value::Map(m) = self {
            return Some(m);
        }
        None
    }
}

struct ValueVisitor;
impl<'de> de::Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an Ancymon value")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        de::Deserialize::deserialize(deserializer)
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(Value::Integer(v))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        if let Ok(v) = i64::try_from(v) {
            Ok(Value::Integer(v))
        } else {
            Err(de::Error::custom("u64 value too large to fit in i64"))
        }
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
        Ok(Value::Float(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where
        V: de::SeqAccess<'de>,
    {
        let mut vec = Vec::new();
        while let Some(value) = visitor.next_element()? {
            vec.push(value);
        }
        Ok(Value::Array(vec))
    }

    fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where
        V: de::MapAccess<'de>,
    {
        let mut map = HashMap::new();
        while let Some((key, value)) = visitor.next_entry()? {
            map.insert(key, value);
        }
        Ok(Value::Map(map))
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_int() {
        let toml_value = "key = 3";
        let value = toml::from_str::<Value>(toml_value).unwrap();
        assert_eq!(value.as_map().unwrap()["key"], Value::Integer(3));
    }
    #[test]
    fn deserialize_float() {
        let toml_value = "key = 3.14";
        let value = toml::from_str::<Value>(toml_value).unwrap();
        assert_eq!(value.as_map().unwrap()["key"], Value::Float(3.14));
    }
    #[test]
    fn deserialize_bool() {
        let toml_value = "key = true";
        let value = toml::from_str::<Value>(toml_value).unwrap();
        assert_eq!(value.as_map().unwrap()["key"], Value::Bool(true));
    }
    #[test]
    fn deserialize_str() {
        let toml_value = r#"key = "hello world""#;
        let value = toml::from_str::<Value>(toml_value).unwrap();
        assert_eq!(
            value.as_map().unwrap()["key"],
            Value::String("hello world".to_string())
        );
    }
    #[test]
    fn deserialize_array() {
        let toml_value = "key = [1, 2, 3]";
        let value = toml::from_str::<Value>(toml_value).unwrap();
        assert_eq!(
            value.as_map().unwrap()["key"],
            Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3)
            ])
        );
    }
    #[test]
    fn deserialize_map() {
        let toml_value = r#"key = { a = 1, b = "hello" }"#;
        let value = toml::from_str::<Value>(toml_value).unwrap();
        assert_eq!(
            value.as_map().unwrap()["key"],
            Value::Map(HashMap::from_iter(vec![
                ("a".to_string(), Value::Integer(1)),
                ("b".to_string(), Value::String("hello".to_string()))
            ]))
        );
    }
    #[test]
    fn deserialize_nested() {
        let toml_value = r#"key = { a = [1, 2, 3], b = { c = "hello" } }"#;
        let value = toml::from_str::<Value>(toml_value).unwrap();
        let map = value.as_map().unwrap()["key"].as_map().unwrap();
        assert_eq!(
            map["a"],
            Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3)
            ])
        );
        assert_eq!(
            map["b"],
            Value::Map(HashMap::from_iter(vec![(
                "c".to_string(),
                Value::String("hello".to_string())
            )]))
        )
    }
}
