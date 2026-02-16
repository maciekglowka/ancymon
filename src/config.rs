use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use toml::Table;

use crate::{
    actions::Action,
    errors::{AncymonError, ConfigError},
    triggers::Trigger,
};

const ENV_REGEX_STR: &str = r#"\$\{([[:word:]]+)\}"#;
static ENV_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(ENV_REGEX_STR).unwrap());

#[derive(Debug, Deserialize)]
pub struct Config {
    pub(crate) sources: HashMap<String, Table>,
    pub(crate) handlers: HashMap<String, Table>,
    pub(crate) actions: Vec<Action>,
    pub(crate) triggers: Vec<Trigger>,
}
impl Config {
    pub fn new(s: &str) -> Result<Self, AncymonError> {
        let mut value: toml::Value =
            toml::from_str(s).map_err(|e| ConfigError::ParsingError(format!("{e}")))?;
        Self::expand_env_variables(&mut value)?;
        Ok(value.try_into().unwrap())
    }
    fn expand_env_variables(value: &mut toml::Value) -> Result<(), AncymonError> {
        match value {
            toml::Value::String(s) => {
                while let Some(capture) = ENV_REGEX.captures_iter(s).next() {
                    let outer = capture.get(0).ok_or(ConfigError::ParsingError(format!(
                        "Invalid env variable placeholder at: {s}"
                    )))?;
                    let inner = capture.get(1).ok_or(ConfigError::ParsingError(format!(
                        "Invalid env variable placeholder at: {s}"
                    )))?;
                    let var = inner.as_str();
                    let val = std::env::var(var).map_err(|_| {
                        ConfigError::MissingValue(format!("Env variable not defined: {var}"))
                    })?;
                    let range = outer.range();
                    s.replace_range(range, &val);
                }
            }
            toml::Value::Array(a) => {
                for v in a.iter_mut() {
                    Self::expand_env_variables(v)?;
                }
            }
            toml::Value::Table(t) => {
                for (_, v) in t.iter_mut() {
                    Self::expand_env_variables(v)?;
                }
            }
            _ => (),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_str() {
        unsafe {
            std::env::set_var("ANCYMON_TEST_FIRST", "1234");
        }
        unsafe {
            std::env::set_var("ANCYMON_TEST_SECOND", "5678");
        }
        let mut val = toml::Value::String(
            "my value is: ${ANCYMON_TEST_FIRST}:${ANCYMON_TEST_SECOND}".to_string(),
        );
        Config::expand_env_variables(&mut val).unwrap();
        assert_eq!("my value is: 1234:5678", val.as_str().unwrap());
    }

    #[test]
    fn expand_str_twice() {
        unsafe {
            std::env::set_var("ANCYMON_TEST_ONLY", "987");
        }
        let mut val = toml::Value::String(
            "my value is: ${ANCYMON_TEST_ONLY}:${ANCYMON_TEST_ONLY}:".to_string(),
        );
        Config::expand_env_variables(&mut val).unwrap();
        assert_eq!("my value is: 987:987:", val.as_str().unwrap());
    }

    #[test]
    fn expand_table() {
        unsafe {
            std::env::set_var("ANCYMON_TEST_MAP", "SECRET1234");
        }
        let mut val = toml::Value::Table(toml::Table::from_iter(vec![
            (
                "public".to_string(),
                toml::Value::String("NOT_SECRET".to_string()),
            ),
            (
                "private".to_string(),
                toml::Value::String(":${ANCYMON_TEST_MAP}:".to_string()),
            ),
        ]));

        Config::expand_env_variables(&mut val).unwrap();
        assert_eq!(
            ":SECRET1234:",
            val.as_table()
                .unwrap()
                .get("private")
                .unwrap()
                .as_str()
                .unwrap()
        );
        assert_eq!(
            "NOT_SECRET",
            val.as_table()
                .unwrap()
                .get("public")
                .unwrap()
                .as_str()
                .unwrap()
        );
    }

    #[test]
    fn expand_array() {
        unsafe {
            std::env::set_var("ANCYMON_TEST_ARR", "SECRET987");
        }

        let mut val = toml::Value::Array(vec![
            toml::Value::String("NOT_SECRET".to_string()),
            toml::Value::String(":${ANCYMON_TEST_ARR}:".to_string()),
        ]);

        Config::expand_env_variables(&mut val).unwrap();
        assert_eq!("NOT_SECRET", val.as_array().unwrap()[0].as_str().unwrap());
        assert_eq!(":SECRET987:", val.as_array().unwrap()[1].as_str().unwrap());
    }

    #[test]
    fn expand_config() {
        unsafe {
            std::env::set_var("ANCYMON_TEST_SECRET", "$%^#");
        }
        unsafe {
            std::env::set_var("ANCYMON_TEST_ID", "997");
        }
        let config_str = r#"
          [sources.startup]  
          type = "startup"

          [[triggers]]
          source = "startup"
          emit = "start"

          [handlers.msg]
          type = "mail"
          arguments.secret = "${ANCYMON_TEST_SECRET}"
          arguments.id = "${ANCYMON_TEST_ID}"

          [[actions]]
          handler = "msg"
          event = "start"
          emit = ""
        "#;

        let config = Config::new(config_str).unwrap();

        assert_eq!(
            config.handlers["msg"]
                .get("arguments")
                .unwrap()
                .as_table()
                .unwrap()
                .get("secret")
                .unwrap()
                .as_str()
                .unwrap(),
            "$%^#"
        );
        assert_eq!(
            config.handlers["msg"]
                .get("arguments")
                .unwrap()
                .as_table()
                .unwrap()
                .get("id")
                .unwrap()
                .as_str()
                .unwrap(),
            "997"
        );
    }
}
