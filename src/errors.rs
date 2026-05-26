#[derive(Clone, Debug)]
pub enum AncymonError {
    BuildError(BuildError),
    ConfigError(ConfigError),
    RuntimeError(RuntimeError),
    ConversionError(String),
}

impl std::fmt::Display for AncymonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuildError(e) => write!(f, "{e}"),
            Self::ConfigError(e) => write!(f, "{e}"),
            Self::RuntimeError(e) => write!(f, "{e}"),
            Self::ConversionError(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for AncymonError {}
impl serde::de::Error for AncymonError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        AncymonError::ConversionError(msg.to_string())
    }
}

#[derive(Clone, Debug)]
pub enum ConfigError {
    ReadError(String),
    ParsingError(String),
    MissingValue(String),
    InvalidValue(String),
    InvalidValueType(String),
    MissingConfig(String),
    InvalidSource(String),
    InvalidToolType(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TODO")
    }
}

impl std::error::Error for ConfigError {}

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

impl std::error::Error for BuildError {}

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
    Tool(String),
    Source(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArguments(s) => write!(f, "Invalid arguments: {s}"),
            Self::InvalidArgumentType(s) => write!(f, "Invalid argument type: {s}"),
            Self::Bot(s) => write!(f, "Bot error: {s}"),
            Self::Handler(s) => write!(f, "Handler error: {s}"),
            Self::Source(s) => write!(f, "Source error: {s}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<RuntimeError> for AncymonError {
    fn from(value: RuntimeError) -> Self {
        Self::RuntimeError(value)
    }
}
