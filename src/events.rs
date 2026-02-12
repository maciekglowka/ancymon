use crate::errors::AncymonError;

pub(crate) type EventValue = Result<Option<toml::Value>, AncymonError>;

#[derive(Clone, Debug)]
pub(crate) struct Event {
    pub(crate) name: String,
    pub(crate) value: Result<Option<toml::Value>, AncymonError>,
}
impl Event {
    pub(crate) fn new(name: String, value: EventValue) -> Self {
        Self { name, value }
    }
}
