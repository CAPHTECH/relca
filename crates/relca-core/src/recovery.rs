//! Closed recovery observations. These DTOs contain no interpreter transitions.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
include!("recovery/round2_tests.rs");
pub const RULES: &str = "response-recovery-p3a-1";
pub const SCOPE: &str = "joint modeled recovery paths; query evidence is not a business decision or a received original acknowledgement";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Complete,
    Partial,
    Sampled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub start: usize,
    pub end: usize,
}
impl From<crate::diag::Span> for Site {
    fn from(s: crate::diag::Span) -> Self {
        Self {
            start: s.start,
            end: s.end,
        }
    }
}
impl From<(usize, usize)> for Site {
    fn from(s: (usize, usize)) -> Self {
        Self {
            start: s.0,
            end: s.1,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    CommittedOnce,
    NotCommitted,
    StillUnknown,
}
impl Status {
    pub fn from_label(s: &str) -> Option<Self> {
        match s {
            "committed_once" => Some(Self::CommittedOnce),
            "not_committed" => Some(Self::NotCommitted),
            "still_unknown" => Some(Self::StillUnknown),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::CommittedOnce => "committed_once",
            Self::NotCommitted => "not_committed",
            Self::StillUnknown => "still_unknown",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stop {
    End,
    ReceiptUnknown,
    Truncated,
    CutOff,
    StateRange,
    SemanticAbort,
    Deadlock,
}
fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recovery {
    pub rules: String,
    pub coverage: Coverage,
    pub scope: String,
    pub paths: Vec<Path>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Path {
    pub id: String,
    pub commands: Vec<Command>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub case_id: u64,
    pub command: String,
    pub provider: String,
    pub capability: String,
    pub saved_key: u64,
    pub original_site: Site,
    pub queries: Vec<Query>,
    pub resolutions: Vec<Resolution>,
    pub tasks: Vec<Task>,
    pub unresolved: bool,
    pub stop_reason: Stop,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub id: String,
    pub binding: String,
    pub site: Site,
    pub at: u64,
    pub status: Status,
    #[serde(deserialize_with = "nullable")]
    pub evidence: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    pub query_ref: String,
    pub site: Site,
    pub at: u64,
    pub fact: Status,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub binding: String,
    pub capability: String,
    #[serde(deserialize_with = "nullable")]
    pub executor: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub result: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub started_at: Option<u64>,
    #[serde(deserialize_with = "nullable")]
    pub finished_at: Option<u64>,
    pub site: Site,
}
impl Recovery {
    pub fn new(mut paths: Vec<Path>, coverage: Coverage) -> Self {
        for path in &mut paths {
            path.id.clear();
            path.commands.sort_by_key(|c| (c.case_id, c.original_site));
            for c in &mut path.commands {
                c.queries.sort_by_key(|q| (q.at, q.site));
                c.resolutions.sort_by_key(|r| (r.at, r.site));
                c.tasks
                    .sort_by_key(|t| (t.started_at.or(t.finished_at).unwrap_or(0), t.site));
            }
        }
        let mut unique = BTreeMap::new();
        for p in paths {
            unique
                .entry(serde_json::to_string(&p.commands).expect("recovery DTO"))
                .or_insert(p);
        }
        let mut paths: Vec<_> = unique.into_values().collect();
        for (i, p) in paths.iter_mut().enumerate() {
            p.id = format!("path{}", i + 1);
            for c in &mut p.commands {
                let mut names = BTreeMap::new();
                for q in &mut c.queries {
                    let id = format!("{}:{}", p.id, q.id);
                    names.insert(q.id.clone(), id.clone());
                    q.id = id;
                    if q.evidence.is_some() {
                        q.evidence = Some(q.id.clone());
                    }
                }
                for r in &mut c.resolutions {
                    if let Some(id) = names.get(&r.query_ref) {
                        r.query_ref = id.clone();
                    }
                }
            }
        }
        Self {
            rules: RULES.into(),
            scope: SCOPE.into(),
            coverage,
            paths,
        }
    }
    pub fn unsettled(&self) -> bool {
        self.coverage == Coverage::Partial
            || self
                .paths
                .iter()
                .flat_map(|p| &p.commands)
                .any(|c| c.unresolved || c.stop_reason == Stop::CutOff)
    }
    pub fn validate_host(&self, host: &Value) -> Result<(), &'static str> {
        let bad = "invalid receipt recovery observations";
        if self.rules != RULES
            || self.scope != SCOPE
            || (self.paths.is_empty() && self.coverage != Coverage::Partial)
        {
            return Err(bad);
        }
        let sampled = host["mode"] == "simulation";
        if (self.coverage == Coverage::Sampled && !sampled)
            || (self.coverage == Coverage::Complete
                && (sampled || host["bounds"]["truncated"] == true))
        {
            return Err(bad);
        }
        let clock = host["bounds"]["clock"].as_u64().unwrap_or(u64::MAX);
        let mut paths = BTreeSet::new();
        let mut query_ids = BTreeSet::new();
        for p in &self.paths {
            if p.id.is_empty() || !paths.insert(&p.id) {
                return Err(bad);
            }
            let mut commands = BTreeSet::new();
            for c in &p.commands {
                if c.case_id == 0
                    || c.saved_key != c.case_id
                    || c.command.is_empty()
                    || c.provider.is_empty()
                    || !c.capability.starts_with(&format!("{}.", c.provider))
                    || c.original_site.start > c.original_site.end
                    || !commands.insert((c.case_id, &c.command, c.original_site))
                {
                    return Err(bad);
                }
                let case = host["case_outcomes"]
                    .as_array()
                    .and_then(|cs| cs.iter().find(|r| r["case_id"] == c.case_id))
                    .ok_or(bad)?;
                // Case terminals are aggregate observations; a non-End joint
                // path cannot supply their End unless another joint path does.
                if c.stop_reason != Stop::End
                    && case["terminals"].as_array().is_some_and(|ts| {
                        ts.iter()
                            .any(|t| t["kind"] == "end" || t["kind"] == "overflow")
                    })
                    && !self.paths.iter().any(|path| {
                        let rows: Vec<_> = path
                            .commands
                            .iter()
                            .filter(|other| other.case_id == c.case_id)
                            .collect();
                        rows.is_empty() || rows.iter().any(|other| other.stop_reason == Stop::End)
                    })
                {
                    return Err(bad);
                }
                if (!c.resolutions.is_empty() && c.unresolved)
                    || (!c.unresolved
                        && c.resolutions.is_empty()
                        && host["response_receipts"].as_array().is_some_and(|rs| {
                            !rs.iter().any(|r| {
                                r["case_id"] == c.case_id
                                    && r["command"] == c.command
                                    && r["resolved_effect"] == true
                            })
                        }))
                {
                    return Err(bad);
                }
                if let Some(receipts) = host["response_receipts"].as_array()
                    && matches!(c.stop_reason, Stop::End | Stop::ReceiptUnknown)
                    && !receipts.iter().any(|r| {
                        r["case_id"] == c.case_id
                            && r["command"] == c.command
                            && r["capability"] == c.capability
                    })
                {
                    return Err(bad);
                }
                let mut tasks = BTreeMap::new();
                for t in &c.tasks {
                    if t.binding.is_empty()
                        || t.capability.is_empty()
                        || t.site.start > t.site.end
                        || tasks.insert(&t.binding, t).is_some()
                        || t.started_at.is_some_and(|t| t > clock)
                        || t.finished_at.is_some_and(|end| {
                            end > clock || t.started_at.is_none_or(|start| start > end)
                        })
                        || t.executor.as_ref().is_some_and(|s| s.is_empty())
                        || t.result.as_ref().is_some_and(|s| s.is_empty())
                    {
                        return Err(bad);
                    }
                }
                let mut queries = BTreeMap::new();
                for q in &c.queries {
                    if q.id.is_empty()
                        || !query_ids.insert(&q.id)
                        || queries.insert(&q.id, q).is_some()
                        || q.at > clock
                        || q.site.start > q.site.end
                        || (q.status != Status::StillUnknown
                            && q.evidence.as_deref() != Some(q.id.as_str()))
                        || (q.status == Status::StillUnknown && q.evidence.is_some())
                    {
                        return Err(bad);
                    }
                    let task = tasks.get(&q.binding).ok_or(bad)?;
                    if task.finished_at != Some(q.at)
                        || task.result.as_deref() != Some(q.status.label())
                        || task.site != q.site
                        || !task.capability.starts_with(&format!("{}.", c.provider))
                    {
                        return Err(bad);
                    }
                }
                if c.resolutions.len() > 1 {
                    return Err(bad);
                }
                for r in &c.resolutions {
                    let q = queries.get(&r.query_ref).ok_or(bad)?;
                    if r.fact == Status::StillUnknown
                        || q.evidence.is_none()
                        || r.fact != q.status
                        || r.at < q.at
                        || r.at > clock
                        || r.site.start > r.site.end
                    {
                        return Err(bad);
                    }
                }
                match c.stop_reason {
                    Stop::End => {
                        if c.unresolved
                            || (!c.tasks.is_empty() && c.resolutions.len() != 1)
                            || !case["terminals"].as_array().is_some_and(|ts| {
                                ts.iter()
                                    .any(|t| t["kind"] == "end" || t["kind"] == "overflow")
                            })
                        {
                            return Err(bad);
                        }
                    }
                    Stop::ReceiptUnknown => {
                        if !c.unresolved
                            || !case["terminals"]
                                .as_array()
                                .is_some_and(|ts| ts.iter().any(|t| t["kind"] == "receipt_unknown"))
                        {
                            return Err(bad);
                        }
                    }
                    Stop::Truncated => {
                        if host["bounds"]["truncated"] != true {
                            return Err(bad);
                        }
                    }
                    Stop::CutOff => {
                        if !case["terminals"]
                            .as_array()
                            .is_some_and(|ts| ts.iter().any(|t| t["kind"] == "cut_off"))
                            && case["cut_off_count"].as_u64().unwrap_or(0) == 0
                        {
                            return Err(bad);
                        }
                    }
                    Stop::StateRange => {
                        if !host["state_range_violations"]
                            .as_array()
                            .is_some_and(|v| !v.is_empty())
                            || sampled
                                && !case["range_stopped_count"].as_u64().is_some_and(|n| n > 0)
                        {
                            return Err(bad);
                        }
                    }
                    Stop::SemanticAbort => {
                        if !host["diagnostics"]
                            .as_array()
                            .is_some_and(|ds| ds.iter().any(|d| d["severity"] == "error"))
                        {
                            return Err(bad);
                        }
                    }
                    Stop::Deadlock => {
                        if !host["diagnostics"]
                            .as_array()
                            .is_some_and(|ds| ds.iter().any(|d| d["code"] == "W-DEADLOCK-0"))
                            || sampled && !case["deadlocked_count"].as_u64().is_some_and(|n| n > 0)
                        {
                            return Err(bad);
                        }
                    }
                }
            }
        }
        let automatic = host["properties"]
            .as_array()
            .and_then(|ps| ps.iter().find(|p| p["id"] == "receipt_resolution"))
            .ok_or(bad)?;
        if (sampled || self.unsettled()) && automatic["verdict"] != "inconclusive" {
            return Err(bad);
        }
        Ok(())
    }
}
