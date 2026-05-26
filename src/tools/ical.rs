use std::{collections::HashMap, str::FromStr};

use async_trait::async_trait;
use chrono::{DateTime, Local, TimeDelta};
use icalendar::{Calendar, Component, DatePerhapsTime};
use serde::Deserialize;

use crate::{
    errors::{AncymonError, RuntimeError},
    events::EventMeta,
    tools::Tool,
    values::Value,
};

/// An event handler that retrieves and formats events from an iCalendar (iCal)
/// calendar.
///
/// This handler fetches events from a remote iCalendar URL and returns them
/// either as structured data or formatted text, depending on the provided
/// arguments.
///
/// Configuration
///
/// The handler requires a `url` field in its configuration:
///
/// ```toml
/// [handlers.ical]
/// type = "ical"
/// url = "https://example.com/calendar.ics"
/// ```
///
/// Usage
///
/// The handler can be triggered by an event containing a Unix timestamp and
/// will search for events within a specified time window relative to the
/// timestamp.
///
/// For example you can run the below configuration at 20:00 daily (providing
/// current timestamp as an input to collect events for the next day:
///
/// ```toml
/// [[actions]]
/// handler = "ical"
/// event = "some-trigger-event"
/// emit = "ical-events"
/// arguments.start-offset-hours = 4      # Set the window start 4 hours after the provided timestamp
/// arguments.range-hours = 24            # Set the window end 24 hours after the start
/// arguments.text-output = true          # Return formatted text instead of structured data
/// ```
///
/// Return Value
///
/// The handler returns a `Value::Array` containing either:
///
/// 1. **Structured events** (`text-output = false`): Each element is a
///    `Value::Map` with the following fields:
///    - `start` (i64): Unix timestamp (seconds since epoch) of event start
///    - `end` (i64): Unix timestamp of event end
///    - `summary` (Optional String): Event summary/title
///    - `description` (Optional String): Event description
///
/// 2. **Formatted text** (`text-output = true`): Each element is a
///    `Value::String` containing a formatted event summary in Markdown format:
///    - Event title in a heading
///    - Time range (formatted as local datetime)
///    - Description (if available)
///
/// The event are sorted by start time.
/// Handles recurring events defined with `RRULE`
///
/// If no events are found within the specified time window, the handler returns
/// `Value::Null`.
#[derive(Clone, Default)]
pub struct ICalReader {
    url: String,
}
impl ICalReader {
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
impl Tool for ICalReader {
    async fn init(&mut self, config: &Value) -> Result<(), AncymonError> {
        let config: ICalConfig = config.clone().try_into()?;
        self.url = config.url;
        Ok(())
    }

    /// Queries an iCalendar (iCal) calendar for events within a specified time
    /// window.
    ///
    /// # Arguments
    ///
    /// * `event` - A `&Value` expected to contain the base event's Unix
    ///   timestamp (integer seconds). This timestamp is used to establish the
    ///   base date for the search window.
    /// * `arguments` - A `&Value` expected to contain `ICalArguments`,
    ///   specifying:
    ///   - `start-offset-hours` (i64): Hours to add to the base timestamp for
    ///     the query window start
    ///   - `range-hours` (i64): Duration of the query window in hours
    ///   - `text-output` (bool, default: false): If true, outputs formatted
    ///     text instead of structured data
    /// * `_` - A mutable reference to `EventMeta`, which is currently unused.
    ///
    /// # Returns
    ///
    /// * `Result<Value, AncymonError>`:
    ///   - If no events are found in the specified range, it returns
    ///     `Ok(Value::Null)`.
    ///   - If events are found:
    ///     - If `arguments.text_output` is `true`, it returns
    ///       `Ok(Value::Array)` containing strings, where each string is a
    ///       formatted, human-readable summary of an event.
    ///     - Otherwise, it returns `Ok(Value::Array)` where each element is a
    ///       `Value::Map` containing structured event data: `{"start":
    ///       timestamp, "end": timestamp, "summary": string, ...}`.
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

        let mut events = calendar
            .events()
            .flat_map(|ev| {
                get_in_range(ev, &start, &end)
                    .into_iter()
                    .map(|(s, e)| map_event(ev, &s, &e))
            })
            .collect::<Vec<_>>();

        if events.is_empty() {
            return Ok(Value::Null);
        }

        events.sort_by_key(|e| {
            e.as_map()
                .unwrap()
                .get("start")
                .map(|v| v.as_int().unwrap_or(i64::MAX))
                .unwrap_or(i64::MAX)
        });

        let events = events
            .into_iter()
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
    start: i64,
    end: i64,
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
/// For recurring events a list of all matching dates is returned.
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
