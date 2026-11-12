//! Computation limits are distinct from malformed static IR (ADR 0067).
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationLimit {
    pub counter: String,
    pub limit: usize,
    pub requested: Option<usize>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub requested_overflow: bool,
    pub phase: String,
    pub process: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub effect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub capability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub judgement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub call: Option<String>,
    pub span: Option<diag::Span>,
}

#[derive(Debug)]
pub(super) enum Error {
    Limit(Box<DerivationLimit>),
    InvalidIr(String),
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl Error {
    pub(super) fn diagnostic(self) -> diag::Diagnostic {
        let (message, span) = match self {
            Self::Limit(l) => (l.to_string(), l.span),
            Self::InvalidIr(reason) => (format!("invalid static IR for Agency: {reason}"), None),
        };
        diag::Diagnostic {
            code: diag::U_SEM_0.into(),
            severity: diag::Severity::Error,
            message,
            span,
        }
    }
}

impl std::fmt::Display for DerivationLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Agency derivation limit exceeded: {} {} (requested ",
            self.counter, self.limit
        )?;
        match self.requested {
            Some(n) => write!(f, "{n}")?,
            None => write!(f, "unrepresentable")?,
        }
        write!(f, ") in {}; process {}", self.phase, self.process)?;
        if let Some(effect) = &self.effect {
            write!(f, ", effect {effect}")?;
        }
        if let Some(cap) = &self.capability {
            write!(f, " ({cap})")?;
        }
        if let Some(j) = &self.judgement {
            write!(f, ", judgement {j}")?;
        }
        if let Some(call) = &self.call {
            write!(f, ", call {call}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub(super) struct Location {
    pub phase: String,
    pub process: String,
    pub effect: Option<String>,
    pub capability: Option<String>,
    pub judgement: Option<String>,
    pub call: Option<String>,
    pub span: Option<diag::Span>,
}

impl Location {
    pub fn model(m: &ir::Model, phase: &str) -> Self {
        Self {
            phase: phase.into(),
            process: m.process.name.clone(),
            span: Some(m.process.span),
            ..Self::default()
        }
    }
    pub fn limit(&self, counter: &str, limit: usize, requested: usize) -> Error {
        Error::Limit(Box::new(self.record(counter, limit, Some(requested))))
    }
    pub fn unrepresentable(&self, counter: &str, limit: usize) -> Error {
        Error::Limit(Box::new(self.record(counter, limit, None)))
    }
    fn record(&self, counter: &str, limit: usize, requested: Option<usize>) -> DerivationLimit {
        DerivationLimit {
            counter: counter.into(),
            limit,
            requested,
            requested_overflow: requested.is_none(),
            phase: self.phase.clone(),
            process: self.process.clone(),
            effect: self.effect.clone(),
            capability: self.capability.clone(),
            judgement: self.judgement.clone(),
            call: self.call.clone(),
            span: self.span,
        }
    }
}
