use std::{collections::HashMap, str::FromStr};

use async_trait::async_trait;
use chrono::{DateTime, Local, TimeDelta};
use icalendar::{Calendar, Component, DatePerhapsTime};
use serde::Deserialize;

use crate::{
    errors::{AncymonError, RuntimeError},
    events::EventMeta,
    handlers::EventHandler,
    values::Value,
};

#[derive(Clone, Default)]
pub struct ICalHandler {
    url: String,
}
impl ICalHandler {
    async fn fetch(&self) -> Result<Calendar, AncymonError> {
        let body = reqwest::get(&self.url)
            .await
            .map_err(|e| RuntimeError::Handler(format!("Can't fetch the calendar: {e}")))?
            .text()
            .await
            .map_err(|e| RuntimeError::Handler(format!("Can't read calendar response: {e}")))?;
        body.parse()
            .map_err(|e| RuntimeError::Handler(format!("Calendar parsing failed: {e}")).into())
    }
}

#[async_trait]
impl EventHandler for ICalHandler {
    async fn init(&mut self, config: &Value) -> Result<(), AncymonError> {
        let config: ICalConfig = config.clone().try_into()?;
        self.url = config.url;
        Ok(())
    }

    /// Expects timestamp as event input
    async fn execute(
        &self,
        event: &Value,
        arguments: &Value,
        _: &mut EventMeta,
    ) -> Result<Value, AncymonError> {
        let arguments: ICalArguments = arguments.clone().try_into()?;

        let ts = event
            .as_int()
            .ok_or(RuntimeError::InvalidArgumentType(format!(
                "Expected ts int, got {event:?}"
            )))?;
        let ts_day = DateTime::from_timestamp_secs(ts)
            .ok_or(RuntimeError::Handler(format!("Invalid timestamp: {ts}")))?
            .with_timezone(&Local);

        let start = ts_day + TimeDelta::hours(arguments.start_offset_hours);
        let end = start + TimeDelta::hours(arguments.range_hours);

        let calendar = self.fetch().await?;

        let events = calendar
            .events()
            .flat_map(|ev| {
                get_in_range(ev, &start, &end)
                    .into_iter()
                    .map(|(s, e)| map_event(ev, &s, &e))
            })
            .map(|e| {
                if arguments.text_output {
                    Value::String(format_text(&e))
                } else {
                    e
                }
            })
            .collect::<Vec<_>>();

        Ok(Value::Array(events))
    }
}

#[derive(Deserialize)]
struct ICalArguments {
    #[serde(rename = "start-offset-hours")]
    start_offset_hours: i64,
    #[serde(rename = "range-hours")]
    range_hours: i64,
    #[serde(default)]
    #[serde(rename = "text-output")]
    text_output: bool,
}

#[derive(Deserialize)]
struct ICalConfig {
    url: String,
}

/// Find event dates in a range given.
///
/// For a single event it simply returns a 1-element vec,
/// if the event is within the range or an empty vec otherwise.
/// For reccurring events a list of all matching dates is returned.
fn get_in_range(
    event: &icalendar::Event,
    start: &DateTime<Local>,
    end: &DateTime<Local>,
) -> Vec<(DateTime<Local>, DateTime<Local>)> {
    if let Some(rule) = event.property_value("RRULE") {
        let Some(s) = event.get_start().and_then(map_date) else {
            return vec![];
        };
        let Some(e) = event.get_end().and_then(map_date) else {
            return vec![];
        };
        let dt_start = s.with_timezone(&rrule::Tz::Local(Local));
        let Ok(rule_set) = rrule::RRule::from_str(rule).and_then(|r| r.build(dt_start)) else {
            return vec![];
        };
        let duration = e.signed_duration_since(s);
        let rule_set = rule_set
            .after(start.with_timezone(&rrule::Tz::Local(Local)))
            .before(end.with_timezone(&rrule::Tz::Local(Local)));
        rule_set
            .all(100)
            .dates
            .into_iter()
            .map(|s| {
                (
                    s.with_timezone(&Local),
                    (s + duration).with_timezone(&Local),
                )
            })
            .collect::<Vec<_>>()
    } else {
        let ev_start = event.get_start().and_then(map_date);
        let ev_end = event.get_end().and_then(map_date);
        match (ev_start, ev_end) {
            (Some(s), Some(e)) => {
                if s < *end && (s >= *start || e > *start) {
                    vec![(s, e)]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }
}

fn map_date(cal_date: DatePerhapsTime) -> Option<DateTime<Local>> {
    match cal_date {
        DatePerhapsTime::DateTime(dt) => Some(dt.try_into_utc()?.with_timezone(&Local)),
        // For naive date assume local
        DatePerhapsTime::Date(d) => d
            .and_hms_opt(00, 00, 00)?
            .and_local_timezone(Local)
            .single(),
    }
}

fn map_event(event: &icalendar::Event, start: &DateTime<Local>, end: &DateTime<Local>) -> Value {
    let mut map = HashMap::new();
    map.insert("start".to_string(), Value::Integer(start.timestamp()));
    map.insert("end".to_string(), Value::Integer(end.timestamp()));

    if let Some(summary) = event.get_summary() {
        map.insert("summary".to_string(), Value::String(summary.to_string()));
    }
    if let Some(description) = event.get_description() {
        map.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }

    Value::Map(map)
}

fn ts_to_local_str(ts: i64) -> String {
    let Some(utc) = DateTime::<chrono::Utc>::from_timestamp_secs(ts) else {
        return String::new();
    };
    utc.with_timezone(&Local).to_string()
}

fn format_text(event: &Value) -> String {
    let mut text = String::new();

    let Some(map) = event.as_map() else {
        // Should never happen so we do not return Option<_>.
        return String::new();
    };
    if let Some(Value::String(summary)) = map.get("summary") {
        text += &format!("## {summary}\n");
    } else {
        text += "## <Unnamed event>\n";
    }
    match (map.get("start"), map.get("end")) {
        (Some(Value::Integer(s)), Some(Value::Integer(e))) => {
            text += &format!("{} - {}\n", ts_to_local_str(*s), ts_to_local_str(*e));
        }
        (_, Some(Value::Integer(e))) => {
            text += &format!("<start missing> - {}\n", ts_to_local_str(*e));
        }
        (Some(Value::Integer(s)), _) => {
            text += &format!("{} - <end missing>-\n", ts_to_local_str(*s));
        }
        _ => (),
    }
    if let Some(Value::String(description)) = map.get("description") {
        text += &format!("{description}\n\n");
    } else {
        text += "\n\n";
    }

    text
}
