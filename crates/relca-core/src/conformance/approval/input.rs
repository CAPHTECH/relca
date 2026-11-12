//! A separate bounded sidecar parser; relca-log/v1 is deliberately unchanged.
use crate::{conformance::input::Failure, ir};
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt, io::Read};

pub(super) const MAX_BYTES: usize = 1_048_576;
pub(super) const MAX_EPISODES: usize = 10_000;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Profile {
    pub approval_operation: String,
    pub proposal_operation: String,
    pub proposal_approve_results: Vec<String>,
    pub approval_decision_results: Vec<String>,
    pub approval_approve_results: Vec<String>,
    pub reinvestigation_operations: Vec<String>,
}
fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Episode {
    pub case_id: String,
    pub approval_task_id: String,
    #[serde(deserialize_with = "nullable")]
    pub proposal_task_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub reinvestigation_task_ids: Option<Vec<String>>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Spec {
    #[serde(rename = "schema")]
    _schema: String,
    pub model_sha256: String,
    pub log_sha256: String,
    pub profile: Profile,
    pub episodes: Vec<Episode>,
}
#[derive(Debug)]
pub(super) struct Parsed {
    pub spec: Option<Spec>,
    pub sha256: Option<String>,
    pub limit: Option<&'static str>,
}
pub(super) fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
// Reject duplicates recursively before inspecting the schema envelope.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON with unique keys")
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
            fn visit_str<E: de::Error>(self, s: &str) -> Result<Unique, E> {
                Ok(Unique(s.into()))
            }
            fn visit_bool<E: de::Error>(self, b: bool) -> Result<Unique, E> {
                Ok(Unique(b.into()))
            }
            fn visit_u64<E: de::Error>(self, n: u64) -> Result<Unique, E> {
                Ok(Unique(n.into()))
            }
            fn visit_i64<E: de::Error>(self, n: i64) -> Result<Unique, E> {
                Ok(Unique(n.into()))
            }
            fn visit_f64<E: de::Error>(self, n: f64) -> Result<Unique, E> {
                Ok(Unique(serde_json::json!(n)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
        }
        d.deserialize_any(Visitor)
    }
}
pub(super) fn read(reader: impl Read) -> Result<Parsed, Failure> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::invalid("approval_spec_input"))?;
    if bytes.len() > MAX_BYTES {
        return Ok(Parsed {
            spec: None,
            sha256: None,
            limit: Some("approval_spec_limit"),
        });
    }
    let v = serde_json::from_slice::<Unique>(&bytes)
        .map_err(|e| {
            Failure::invalid(if e.to_string().starts_with("duplicate_key") {
                "approval_spec_duplicate_key"
            } else {
                "approval_spec_bad_field"
            })
        })?
        .0;
    match v.get("schema").and_then(Value::as_str) {
        Some("relca-approval-spec/v1") => (),
        Some(s) if !s.is_empty() => return Err(Failure::unsupported("approval_spec_schema")),
        _ => return Err(Failure::invalid("approval_spec_bad_field")),
    }
    if v["profile"]
        .as_array()
        .is_some_and(|a| a.len() > 1 && a.iter().all(Value::is_object))
    {
        return Err(Failure {
            code: "U-LOG-1",
            reason: "unsupported_approval_profiles",
        });
    }
    let spec: Spec =
        serde_json::from_value(v).map_err(|_| Failure::invalid("approval_spec_bad_field"))?;
    let valid_hash = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if !valid_hash(&spec.model_sha256) || !valid_hash(&spec.log_sha256) {
        return Err(Failure::invalid("approval_spec_bad_field"));
    }
    let limit = (spec.episodes.len() > MAX_EPISODES).then_some("approval_episode_limit");
    Ok(Parsed {
        spec: Some(spec),
        sha256: Some(hash(&bytes)),
        limit,
    })
}
fn unique(ids: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    ids.iter().all(|s| !s.is_empty() && seen.insert(s))
}
pub(super) fn capability(m: &ir::Model, op: &str) -> Option<ir::CapabilityId> {
    m.capabilities.iter().enumerate().find_map(|(i, c)| {
        (format!("{}.{}", m.actors[c.actor.0].name, c.name) == op).then_some(ir::CapabilityId(i))
    })
}
impl Spec {
    pub fn validate(&self, m: &ir::Model) -> Result<(), Failure> {
        let bad = || Failure::invalid("approval_spec_bad_profile");
        let p = &self.profile;
        let a = capability(m, &p.approval_operation).ok_or_else(bad)?;
        let b = capability(m, &p.proposal_operation).ok_or_else(bad)?;
        // A scalar result vocabulary cannot configure a record return. This is
        // an unsupported model capability, not a misspelled profile member.
        if [a, b]
            .iter()
            .any(|id| m.types[m.capabilities[id.0].return_type.0].record.is_some())
        {
            return Err(Failure {
                code: "U-LOG-1",
                reason: "unsupported_approval_semantics",
            });
        }
        if a == b
            || !unique(&p.reinvestigation_operations)
            || p.reinvestigation_operations.is_empty()
        {
            return Err(bad());
        }
        for op in &p.reinvestigation_operations {
            let id = capability(m, op).ok_or_else(bad)?;
            if id == a || id == b {
                return Err(bad());
            }
        }
        for (id, results) in [
            (a, &p.approval_decision_results),
            (a, &p.approval_approve_results),
            (b, &p.proposal_approve_results),
        ] {
            let c = &m.capabilities[id.0];
            if results.is_empty()
                || !unique(results)
                || results
                    .iter()
                    .any(|s| !m.types[c.return_type.0].members.contains(s))
            {
                return Err(bad());
            }
        }
        if p.approval_approve_results
            .iter()
            .any(|s| !p.approval_decision_results.contains(s))
        {
            return Err(bad());
        }
        let mut keys = BTreeSet::new();
        let mut assigned = BTreeSet::new();
        for e in &self.episodes {
            if e.case_id.is_empty()
                || e.approval_task_id.is_empty()
                || e.proposal_task_id
                    .as_ref()
                    .is_some_and(|s| s.is_empty() || s == &e.approval_task_id)
            {
                return Err(Failure::invalid("approval_spec_bad_field"));
            }
            if !keys.insert((&e.case_id, &e.approval_task_id)) {
                return Err(Failure::invalid("approval_spec_duplicate_assignment"));
            }
            if let Some(ids) = &e.reinvestigation_task_ids {
                if ids.iter().any(|s| s.is_empty()) {
                    return Err(Failure::invalid("approval_spec_bad_field"));
                }
                for id in ids {
                    if id == &e.approval_task_id
                        || e.proposal_task_id.as_ref() == Some(id)
                        || !assigned.insert((&e.case_id, id))
                    {
                        return Err(Failure::invalid("approval_spec_duplicate_assignment"));
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn record_returns_are_unsupported_rather_than_bad_result_names() {
        let (_, mut m, _, text) = super::super::prepared("a01_baseline");
        let id = capability(&m, "P.propose").unwrap();
        let ty = m.capabilities[id.0].return_type;
        m.types[ty.0].record = Some(ir::RecordShape { fields: Vec::new() });
        m.types[ty.0].members.clear();
        let s = read(text.as_bytes()).unwrap().spec.unwrap();
        let failure = s.validate(&m).unwrap_err();
        assert_eq!(
            (failure.code, failure.reason),
            ("U-LOG-1", "unsupported_approval_semantics")
        );
    }
    #[test]
    fn i1_frozen_spec_rejections_and_inclusive_limits() {
        for (name, reason) in [
            ("a16_duplicate_key", "approval_spec_duplicate_key"),
            ("a16_unknown_field", "approval_spec_bad_field"),
            ("a16_missing_field", "approval_spec_bad_field"),
            (
                "a16_duplicate_episode",
                "approval_spec_duplicate_assignment",
            ),
            (
                "a16_double_assignment",
                "approval_spec_duplicate_assignment",
            ),
            ("a16_future_schema", "approval_spec_schema"),
            ("a16_multiple_profiles", "unsupported_approval_profiles"),
            ("a16_bad_result", "approval_spec_bad_profile"),
            ("a16_bad_operation", "approval_spec_bad_profile"),
        ] {
            let (_, m, _, text) = super::super::prepared(name);
            let result = read(text.as_bytes()).and_then(|p| p.spec.unwrap().validate(&m));
            assert_eq!(result.unwrap_err().reason, reason, "{name}");
        }
        for name in [
            "a17_spec_boundary",
            "a17_spec_limit",
            "a17_episode_boundary",
            "a17_episode_limit",
        ] {
            let (_, m, _, text) = super::super::prepared(name);
            let p = read(text.as_bytes()).unwrap();
            assert_eq!(p.limit.is_some(), name.ends_with("limit"));
            if let Some(s) = p.spec {
                s.validate(&m).unwrap();
            }
            if name == "a17_spec_limit" {
                assert!(p.sha256.is_none());
            }
        }
    }
}
