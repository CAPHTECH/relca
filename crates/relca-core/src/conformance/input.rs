//! Strict, bounded JSONL ingestion. Error messages never reproduce input payloads.
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fmt,
    io::{BufRead, BufReader, Read},
};

pub const MAX_EVENTS: usize = 10000;
pub const MAX_LINE_BYTES: usize = 65536;
pub const MAX_ORDER_STATES: usize = 100000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: &'static str,
    pub reason: &'static str,
}
impl Failure {
    pub fn invalid(reason: &'static str) -> Self {
        Self {
            code: "E-LOG-0",
            reason,
        }
    }
    pub fn unsupported(reason: &'static str) -> Self {
        Self {
            code: "U-LOG-0",
            reason,
        }
    }
}

// deserialize_with makes nullable fields required, unlike serde's implicit
// missing Option default. Unknown fields are rejected at every nesting level.
fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub kind: String,
    pub schema: String,
    pub clock: String,
    pub time_unit: String,
    pub window: Window,
    pub cases: Vec<CaseScope>,
    pub actor_map: Vec<ActorMap>,
    pub executor_map: Vec<ExecutorMap>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub from: u64,
    pub to: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseScope {
    pub case_id: String,
    #[serde(deserialize_with = "nullable")]
    pub from_case_start: Option<bool>,
    #[serde(deserialize_with = "nullable")]
    pub through_case_end: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorMap {
    pub source_id: String,
    pub actor: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutorMap {
    pub source_id: String,
    pub person: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Start,
    Finish,
    Cancel,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelPhase {
    Request,
    Ack,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Time {
    pub earliest: u64,
    pub latest: u64,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub kind: String,
    pub event_id: String,
    pub case_id: String,
    pub task_id: String,
    pub operation: String,
    pub lifecycle: Lifecycle,
    #[serde(deserialize_with = "nullable")]
    pub cancel_phase: Option<CancelPhase>,
    pub time: Time,
    pub after: Vec<String>,
    pub actor_id: String,
    #[serde(deserialize_with = "nullable")]
    pub executor_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub input_version: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub output_version: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub result: Option<String>,
    #[serde(skip)]
    pub line: usize,
}
#[derive(Debug, Default)]
pub struct Log {
    pub header: Option<Header>,
    pub events: Vec<Event>,
    pub sha256: Option<String>,
    pub lines: Option<usize>,
    pub processed_lines: usize,
    pub limit: Option<&'static str>,
}

/// Recursively reject duplicate object keys before constructing typed records.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON value with unique keys")
            }
            fn visit_map<M: de::MapAccess<'de>>(self, mut map: M) -> Result<Unique, M::Error> {
                let mut out = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if out.contains_key(&key) {
                        return Err(de::Error::custom("duplicate_key"));
                    }
                    out.insert(key, map.next_value::<Unique>()?.0);
                }
                Ok(Unique(Value::Object(out)))
            }
            fn visit_seq<S: de::SeqAccess<'de>>(self, mut seq: S) -> Result<Unique, S::Error> {
                let mut out = Vec::new();
                while let Some(v) = seq.next_element::<Unique>()? {
                    out.push(v.0);
                }
                Ok(Unique(Value::Array(out)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
                Ok(Unique(Value::String(v.into())))
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
                Ok(Unique(Value::Bool(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Unique, E> {
                Ok(Unique(serde_json::json!(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
        }
        d.deserialize_any(Visitor)
    }
}
fn unique_ids<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    ids.into_iter().all(|s| !s.is_empty() && seen.insert(s))
}
fn header(v: Value) -> Result<Header, Failure> {
    // Read the envelope before applying this version's required/known fields.
    // Future schemas need not retain v1's clock or extension vocabulary.
    match v.get("schema").and_then(Value::as_str) {
        Some("relca-log/v1") => (),
        Some(s) if !s.is_empty() => return Err(Failure::unsupported("schema")),
        _ => return Err(Failure::invalid("bad_field")),
    }
    let h: Header = serde_json::from_value(v).map_err(|_| Failure::invalid("bad_field"))?;
    if h.kind != "header"
        || h.schema.is_empty()
        || h.time_unit.is_empty()
        || h.clock.is_empty()
        || h.window.from > h.window.to
        || !unique_ids(h.cases.iter().map(|c| c.case_id.as_str()))
        || !unique_ids(h.actor_map.iter().map(|m| m.source_id.as_str()))
        || !unique_ids(h.executor_map.iter().map(|m| m.source_id.as_str()))
        || h.actor_map.iter().any(|m| m.actor.is_empty())
        || h.executor_map.iter().any(|m| m.person.is_empty())
    {
        return Err(Failure::invalid("bad_field"));
    }
    if h.time_unit != "min" {
        return Err(Failure::unsupported("time_unit"));
    }
    Ok(h)
}

pub fn read(reader: impl Read) -> Result<Log, Failure> {
    let mut log = Log::default();
    read_into(reader, &mut log)?;
    Ok(log)
}
/// Preserve known metadata on errors without publishing prefix conclusions.
/// The caller supplies a fresh Log; unconsumed bytes never acquire a full hash.
pub fn read_into(reader: impl Read, log: &mut Log) -> Result<(), Failure> {
    let mut reader = BufReader::new(reader);
    let mut hash = Sha256::new();
    let mut ids = BTreeSet::new();
    loop {
        let mut line = Vec::new();
        reader
            .by_ref()
            .take((MAX_LINE_BYTES + 2) as u64)
            .read_until(b'\n', &mut line)
            .map_err(|_| Failure::invalid("input"))?;
        if line.is_empty() {
            break;
        }
        let bytes = line.len() - usize::from(line.last() == Some(&b'\n'));
        if bytes > MAX_LINE_BYTES && log.header.is_none() {
            return Err(Failure::invalid("header_line_limit"));
        }
        let limit = if bytes > MAX_LINE_BYTES {
            Some("line_limit")
        } else if log.header.is_some() && log.events.len() == MAX_EVENTS {
            Some("event_limit")
        } else {
            None
        };
        if limit.is_some() {
            log.limit = limit;
            return Ok(());
        }
        hash.update(&line);
        log.processed_lines += 1;
        let v = serde_json::from_slice::<Unique>(&line)
            .map_err(|e| {
                Failure::invalid(if e.to_string().starts_with("duplicate_key") {
                    "duplicate_key"
                } else {
                    "malformed_jsonl"
                })
            })?
            .0;
        if log.header.is_none() {
            log.header = Some(header(v)?);
            continue;
        }
        let h = log.header.as_ref().expect("header read");
        let mut e: Event = serde_json::from_value(v).map_err(|_| Failure::invalid("bad_field"))?;
        if e.kind != "event"
            || [
                &e.event_id,
                &e.case_id,
                &e.task_id,
                &e.operation,
                &e.actor_id,
            ]
            .iter()
            .any(|s| s.is_empty())
            || [
                &e.executor_id,
                &e.input_version,
                &e.output_version,
                &e.result,
            ]
            .iter()
            .any(|s| s.as_ref().is_some_and(|s| s.is_empty()))
            || !h.cases.iter().any(|c| c.case_id == e.case_id)
            || e.time.earliest > e.time.latest
            || e.time.earliest < h.window.from
            || e.time.latest > h.window.to
            || (e.lifecycle == Lifecycle::Cancel) != e.cancel_phase.is_some()
            || (e.lifecycle != Lifecycle::Finish && e.result.is_some())
            || !unique_ids(e.after.iter().map(String::as_str))
            || e.after.contains(&e.event_id)
        {
            return Err(Failure::invalid("bad_field"));
        }
        if !ids.insert(e.event_id.clone()) {
            return Err(Failure::invalid("duplicate_event_id"));
        }
        e.line = log.processed_lines;
        log.events.push(e);
    }
    if log.header.is_none() {
        return Err(Failure::invalid("malformed_jsonl"));
    }
    log.sha256 = Some(hash.finalize().iter().map(|b| format!("{b:02x}")).collect());
    log.lines = Some(log.processed_lines);
    if log
        .events
        .iter()
        .any(|e| e.after.iter().any(|id| !ids.contains(id)))
    {
        return Err(Failure::invalid("unknown_after"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_keys_are_checked_recursively_without_payload_echo() {
        for s in [r#"{"a":1,"a":2}"#, r#"{"a":[{"x":null,"x":true}]}"#] {
            assert!(serde_json::from_str::<Unique>(s).is_err());
        }
        assert!(serde_json::from_str::<Unique>(r#"{"a":null,"b":[1,true,"ok"]}"#).is_ok());
    }
    #[test]
    fn nullable_fields_are_required_and_not_defaulted() {
        let missing = r#"{"case_id":"c","from_case_start":true}"#;
        assert!(serde_json::from_str::<CaseScope>(missing).is_err());
        assert!(
            serde_json::from_str::<CaseScope>(
                &missing.replace("true}", "true,\"through_case_end\":null}")
            )
            .is_ok()
        );
    }
}
