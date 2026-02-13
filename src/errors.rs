#[derive(Clone, Debug)]
pub enum AncymonError {
    BuildError(BuildError),
    ConfigError(ConfigError),
    RuntimeError(RuntimeError),
    ConversionError(String),
}

impl std::fmt::Display for AncymonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TODO")
    }
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub enum BuildError {
    Handler(String),
    Source(String),
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

#[derive(Clone, Debug)]
pub enum RuntimeError {
    InvalidArguments(String),
    InvalidArgumentType(String),
    Bot(String),
    Handler(String),
    Source(String),
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
