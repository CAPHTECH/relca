//! Strict configuration and dedicated estimate output.
use crate::{
    Verdict,
    conformance::{LogDiagnostic, identity, input},
    diag,
    probability::Rational,
    report::ModelMetadata,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, io::Read};

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Selection {
    All(String),
    Explicit(Vec<String>),
}
impl Selection {
    pub(super) fn includes(&self, id: &str) -> bool {
        match self {
            Self::All(_) => true,
            Self::Explicit(xs) => xs.iter().any(|x| x == id),
        }
    }
    pub(super) fn known(&self, has: impl Fn(&str) -> bool) -> bool {
        match self {
            Self::All(_) => true,
            Self::Explicit(xs) => xs.iter().all(|x| has(x)),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Spec {
    schema: String,
    pub(super) model_sha256: String,
    pub(super) log_sha256: String,
    pub(super) confidence: Rational,
    pub(super) min_samples: u64,
    pub(super) max_support: u64,
    pub(super) decimals: u32,
    pub(super) slots: Selection,
}
impl Spec {
    pub(super) fn read(reader: impl Read) -> Result<Self, &'static str> {
        const LIMIT: u64 = 1024 * 1024;
        let mut bytes = Vec::new();
        reader
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "spec_input")?;
        if bytes.len() as u64 > LIMIT {
            return Err("spec_bad_field");
        }
        let s: Self = serde_json::from_slice(&bytes).map_err(|_| "spec_bad_field")?;
        let sha = |v: &str| {
            v.len() == 64
                && v.bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        };
        if s.schema != "relca-estimate-spec/v1"
            || !sha(&s.model_sha256)
            || !sha(&s.log_sha256)
            || s.confidence.is_zero()
            || s.confidence == Rational::one()
            || !s.confidence.within_bits(256)
            || s.min_samples == 0
            || s.max_support == 0
            || s.decimals > 63
            || !match &s.slots {
                Selection::All(s) => s == "all",
                Selection::Explicit(xs) => {
                    !xs.is_empty()
                        && xs.iter().all(|x| !x.is_empty())
                        && xs.iter().collect::<BTreeSet<_>>().len() == xs.len()
                }
            }
        {
            return Err("spec_bad_field");
        }
        Ok(s)
    }
}

#[derive(Serialize)]
pub struct EstimateReport {
    schema: &'static str,
    pub verdict: Verdict,
    pub(super) reason: Option<&'static str>,
    pub(super) model: ModelMetadata,
    pub(super) configuration: Option<String>,
    pub(super) log: LogMetadata,
    pub(super) multiplicity: Option<Value>,
    pub(super) slots: Vec<Row>,
    pub(super) unmapped_actor: Vec<identity::UnmappedActor>,
    pub(super) unmapped_operation: Vec<identity::UnmappedOperation>,
    pub(super) truncated: bool,
    pub(super) assumption_records: Vec<Value>,
    pub diagnostics: Vec<LogDiagnostic>,
    scope_notice: &'static [&'static str],
}
impl Default for EstimateReport {
    fn default() -> Self {
        Self {
            schema: "relca-estimate/v1",
            verdict: Verdict::Inconclusive,
            reason: None,
            model: ModelMetadata {
                name: None,
                sha256: None,
            },
            configuration: None,
            log: LogMetadata::default(),
            multiplicity: None,
            slots: Vec::new(),
            unmapped_actor: Vec::new(),
            unmapped_operation: Vec::new(),
            truncated: false,
            assumption_records: Vec::new(),
            diagnostics: Vec::new(),
            scope_notice: &[
                "この窓の観測であり、別の期間・To-Be・将来の確率ではない",
                "ログの真正性と代表性は人が未確認。区間は各回の案件を標本とする",
                "開始されなかった案件の数は、このログから数えられない",
            ],
        }
    }
}
impl EstimateReport {
    pub fn invalid(reason: &'static str) -> Self {
        let mut out = Self::default();
        out.fail(Verdict::Invalid, "E-LOG-0", reason);
        out
    }
    pub(super) fn fail(&mut self, verdict: Verdict, code: &str, reason: &'static str) {
        self.verdict = verdict;
        self.reason = Some(reason);
        self.diagnostics.push(LogDiagnostic {
            diagnostic: diag::Diagnostic {
                code: code.into(),
                severity: diag::Severity::Error,
                message: reason.into(),
                span: None,
                presentation: diag::Presentation::default(),
            },
            reason: Some(reason.into()),
        });
    }
    pub(super) fn log_failure(&mut self, f: input::Failure) {
        self.fail(
            if f.code.starts_with('U') {
                Verdict::Unsupported
            } else {
                Verdict::Invalid
            },
            f.code,
            f.reason,
        );
    }
}

#[derive(Default, Serialize)]
pub(super) struct LogMetadata {
    pub(super) sha256: Option<String>,
    lines: Option<usize>,
    processed_lines: usize,
    events: usize,
    clock: Option<String>,
    time_unit: Option<String>,
    pub(super) window: Option<input::Window>,
    pub(super) case_count: Option<usize>,
    endpoints: Option<Vec<input::CaseScope>>,
    limit: Option<&'static str>,
}
impl From<&input::Log> for LogMetadata {
    fn from(log: &input::Log) -> Self {
        let h = log.header.as_ref();
        Self {
            sha256: log.sha256.clone(),
            lines: log.lines,
            processed_lines: log.processed_lines,
            events: log.events.len(),
            clock: h.map(|h| h.clock.clone()),
            time_unit: h.map(|h| h.time_unit.clone()),
            window: h.map(|h| h.window),
            case_count: h.map(|h| h.cases.len()),
            endpoints: h.map(|h| h.cases.clone()),
            limit: log.limit,
        }
    }
}

#[derive(Clone, Serialize)]
pub(super) struct Slot {
    pub(super) id: String,
    pub(super) kind: &'static str,
    pub(super) target: String,
    pub(super) span: diag::Span,
    pub(super) location: Option<diag::Location>,
}
#[derive(Serialize)]
pub(super) struct Row {
    pub(super) slot: Slot,
    #[serde(flatten)]
    pub(super) stats: Stats,
    pub(super) rounds: Option<Vec<Round>>,
    pub(super) occurrences_per_case: Option<Value>,
}
impl Row {
    pub(super) fn new(slot: Slot, stats: Stats) -> Self {
        Self {
            slot,
            stats,
            rounds: None,
            occurrences_per_case: None,
        }
    }
}
#[derive(Serialize)]
pub(super) struct Round {
    pub(super) round: usize,
    #[serde(flatten)]
    pub(super) stats: Stats,
}
#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Status {
    Derived,
    #[default]
    Withheld,
    NotDerivable,
    NotRequested,
}
#[derive(Clone, Default, Serialize)]
pub(super) struct Stats {
    pub(super) status: Status,
    pub(super) reason: Option<&'static str>,
    pub(super) coverage: Option<&'static str>,
    pub(super) n: Option<u32>,
    pub(super) unknown: Option<u32>,
    pub(super) censored: Option<u32>,
    pub(super) counts: Value,
    pub(super) interval: Value,
    pub(super) candidate: Value,
    pub(super) rounding_error_bound: Option<Rational>,
    pub(super) duration_summary: Value,
    pub(super) marginal_counts: Value,
    pub(super) provenance: Value,
    pub(super) sensitivity_points: Value,
    pub(super) sensitivity_reason: Option<&'static str>,
    pub(super) declared_vs_candidate: Value,
}
impl Stats {
    pub(super) fn blank(status: Status, reason: &'static str) -> Self {
        Self {
            status,
            reason: Some(reason),
            ..Self::default()
        }
    }
    pub(super) fn incomplete(&self) -> bool {
        self.status == Status::Withheld || self.coverage == Some("partial")
    }
}
