use std::error::Error;

#[derive(Debug)]
pub enum AncymonError {
    BuildError(BuildError),
    ConfigError(ConfigError),
    RuntimeError(RuntimeError),
}

impl std::fmt::Display for AncymonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TODO")
    }
}

#[derive(Debug)]
pub enum ConfigError {
    ParsingError,
    MissingValue(String),
    InvalidValue(String),
    InvalidValueType(String),
    MissingConfig(String),
    InvalidSource(String),
    InvalidHandlerType(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TODO")
    }
}

impl From<ConfigError> for AncymonError {
    fn from(value: ConfigError) -> Self {
        Self::ConfigError(value)
    }
}

#[derive(Debug)]
pub enum BuildError {
    Handler(Box<dyn Error>),
    Source(Box<dyn Error>),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TODO")
    }
}

impl From<BuildError> for AncymonError {
    fn from(value: BuildError) -> Self {
        Self::BuildError(value)
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    InvalidArgumentType(String),
    Handler(Box<dyn Error>),
    Source(Box<dyn Error>),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TODO")
    }
}

impl From<RuntimeError> for AncymonError {
    fn from(value: RuntimeError) -> Self {
        Self::RuntimeError(value)
    }
}
