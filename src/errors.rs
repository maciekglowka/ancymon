#[derive(Debug)]
pub enum AncymonError {
    ConfigError,
    QuerySourceError,
}

impl std::fmt::Display for AncymonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TODO")
    }
}
