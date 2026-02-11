#[derive(Clone, Debug)]
pub(crate) struct Event {
    name: String,
    value: Option<toml::Value>,
}
impl Event {
    pub(crate) fn new(name: &str, value: Option<toml::Value>) -> Self {
        Self {
            name: name.to_string(),
            value,
        }
    }
}
