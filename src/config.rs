use mlua::Lua;
use regex::Regex;
use std::collections::HashMap;

use crate::{
    actions::Action,
    errors::{AncymonError, ConfigError},
    triggers::Trigger,
    values::Value,
};

const ENV_REGEX_STR: &str = r#"\$\{([[:word:]]+)\}"#;
static ENV_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(ENV_REGEX_STR).unwrap());

#[derive(Debug)]
pub struct Config {
    pub(crate) sources: HashMap<String, Value>,
    pub(crate) handlers: HashMap<String, Value>,
    pub(crate) actions: Vec<Action>,
    pub(crate) triggers: Vec<Trigger>,
}
impl Config {
    pub fn new(s: &str) -> Result<Self, AncymonError> {
        let s = Self::expand_env_variables(s)?;

        let lua = Lua::new();
        lua.load(s)
            .exec()
            .map_err(|e| ConfigError::ParsingError(format!("Lua file parse error: {e}")))?;

        let globals = lua.globals();

        let sources = globals
            .get::<HashMap<String, Value>>("sources")
            .map_err(|e| ConfigError::ParsingError(format!("Sources map parse error: {e}")))?;

        let handlers = globals
            .get::<HashMap<String, Value>>("handlers")
            .map_err(|e| ConfigError::ParsingError(format!("Handlers map parse error: {e}")))?;

        let triggers = globals
            .get::<Vec<Trigger>>("triggers")
            .map_err(|e| ConfigError::ParsingError(format!("Triggers array parse error: {e}")))?;

        let actions = globals
            .get::<Vec<Action>>("actions")
            .map_err(|e| ConfigError::ParsingError(format!("Actions array parse error: {e}")))?;

        Ok(Self {
            sources,
            handlers,
            actions,
            triggers,
        })
    }
    fn expand_env_variables(s: &str) -> Result<String, AncymonError> {
        let mut s = s.to_string();

        while let Some(capture) = ENV_REGEX.captures_iter(&s).next() {
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

        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let config_str = r#"
            sources = {}

            sources["cron"] = {["type"]="cron"}
            sources["discord-command"] = {
                ["type"]="discord-command",
                ["bot-token"]="1234"
            }

            handlers = {}            

            handlers["debug"] = {["type"]="debug"}
            handlers["discord-dm"] = {
                ["type"]="discord-dm",
                ["user-id"]=4321,
                ["bot-token"]="1234"
            }

            triggers = {
                {
                    ["source"]="startup",
                    ["emit"]="startup-trigger"
                },
                {
                    ["source"]="cron",
                    ["emit"]="cron-trigger",
                    ["arguments"]="*/5 * * * * *"
                }
            }

            actions = {
                {
                    ["handler"]="discord-dm",
                    ["event"]="cron-trigger",
                    ["emit"]="discord-dm-event",
                    ["arguments"]={
                        ["message"]="Notification",
                        
                    }
                }                
            }
        "#;

        let config = Config::new(config_str).unwrap();

        assert_eq!(2, config.sources.len());
        assert_eq!(
            config.sources["cron"],
            Value::Map(HashMap::from_iter(vec![(
                "type".to_string(),
                Value::String("cron".to_string())
            )]))
        );
        assert_eq!(
            config.sources["discord-command"],
            Value::Map(HashMap::from_iter(vec![
                (
                    "type".to_string(),
                    Value::String("discord-command".to_string())
                ),
                ("bot-token".to_string(), Value::String("1234".to_string())),
            ]))
        );

        assert_eq!(2, config.handlers.len());
        assert_eq!(
            config.handlers["debug"],
            Value::Map(HashMap::from_iter(vec![(
                "type".to_string(),
                Value::String("debug".to_string())
            )]))
        );
        assert_eq!(
            config.handlers["discord-dm"],
            Value::Map(HashMap::from_iter(vec![
                ("type".to_string(), Value::String("discord-dm".to_string())),
                ("bot-token".to_string(), Value::String("1234".to_string())),
                ("user-id".to_string(), Value::Integer(4321)),
            ]))
        );

        assert_eq!(2, config.triggers.len());
        assert_eq!(config.triggers[0].source, "startup".to_string());
        assert_eq!(config.triggers[0].emit, "startup-trigger".to_string());
        assert_eq!(config.triggers[0].arguments, Value::Null);
        assert_eq!(config.triggers[1].source, "cron".to_string());
        assert_eq!(config.triggers[1].emit, "cron-trigger".to_string());
        assert_eq!(
            config.triggers[1].arguments,
            Value::String("*/5 * * * * *".to_string())
        );

        assert_eq!(1, config.actions.len());
        assert_eq!(config.actions[0].handler, "discord-dm".to_string());
        assert_eq!(config.actions[0].event, "cron-trigger".to_string());
        assert_eq!(config.actions[0].emit, "discord-dm-event".to_string());
        assert_eq!(
            config.actions[0].arguments,
            Value::Map(HashMap::from_iter(vec![(
                "message".to_string(),
                Value::String("Notification".to_string())
            )]))
        );
    }

    #[test]
    fn expand_variables() {
        unsafe {
            std::env::set_var("ANCYMON_TEST_SECRET", "$%^#");
        }
        unsafe {
            std::env::set_var("ANCYMON_TEST_ID", "997");
        }
        let config_str = r#"
            sources = {}
            sources["startup"] = {["type"]="startup"}

            triggers = {
                {
                    ["source"]="startup",
                    ["emit"]="start"
                },
            }

            handlers = {}            

            handlers["msg"] = {
                ["type"]="mail",
                ["arguments"] = {
                    ["secret"]="${ANCYMON_TEST_SECRET}",
                    ["id"]="${ANCYMON_TEST_ID}"
                }
            }

            actions = {
                {
                    ["handler"]="msg",
                    ["event"]="start",
                    ["emit"]="",
                }                
            }
        "#;

        let config = Config::new(config_str).unwrap();

        assert_eq!(
            config.handlers["msg"]
                .as_map()
                .unwrap()
                .get("arguments")
                .unwrap()
                .as_map()
                .unwrap()
                .get("secret")
                .unwrap()
                .as_str()
                .unwrap(),
            "$%^#"
        );
        assert_eq!(
            config.handlers["msg"]
                .as_map()
                .unwrap()
                .get("arguments")
                .unwrap()
                .as_map()
                .unwrap()
                .get("id")
                .unwrap()
                .as_str()
                .unwrap(),
            "997"
        );
    }
}
