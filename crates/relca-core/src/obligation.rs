//! ADR 0022 obligation records. This module contains data, not monitoring semantics.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Fulfilled,
    Waived,
    Violated,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Complete,
    Truncated,
    Aborted,
    /// ADR 0029 U2: a branch stopped by an out-of-range write, and no
    /// computational cut. Only bounded-integer models can produce it.
    RangeStopped,
}

impl Coverage {
    /// Abort wins, then a computational cut, then a range stop (ADR 0029 U2).
    pub fn of(aborted: bool, truncated: bool, range_stopped: bool) -> Self {
        if aborted {
            Self::Aborted
        } else if truncated {
            Self::Truncated
        } else if range_stopped {
            Self::RangeStopped
        } else {
            Self::Complete
        }
    }

    /// The wire word, as in `capability_refusals[].coverage`.
    pub fn word(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Truncated => "truncated",
            Self::Aborted => "aborted",
            Self::RangeStopped => "range_stopped",
        }
    }

    /// The null reason of a withdrawn exact range; `None` when complete.
    pub fn reason(self) -> Option<crate::metrics::Reason> {
        use crate::metrics::Reason;
        match self {
            Self::Complete => None,
            Self::Truncated => Some(Reason::Truncated),
            Self::Aborted => Some(Reason::Aborted),
            Self::RangeStopped => Some(Reason::RangeStopped),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    pub status: Status,
    pub triggered_at: u64,
    pub deadline: u64,
    pub resolved_at: Option<u64>,
    pub reason: Option<String>,
    pub by_clause: Option<String>,
    pub executor: Option<String>,
    pub authority: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obligation {
    pub id: String,
    pub response_id: String,
    pub response_label: Option<String>,
    pub case_id: u64,
    pub trigger_site: String,
    pub trigger: String,
    pub targets: Vec<String>,
    pub debtor: Option<String>,
    pub debtor_status: DebtorStatus,
    pub non_waivable: bool,
    pub coverage: Coverage,
    pub outcomes: Vec<Resolution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebtorStatus {
    Declared,
    NotConfigured,
}

/// Declaration projection only: it does not verify who fulfilled the duty.
pub(crate) fn debtor(
    c: &crate::ir::ContractClause,
    m: &crate::ir::Model,
) -> (Option<String>, DebtorStatus) {
    use crate::ir::{ContractKind, Debtor};
    match c.kind {
        ContractKind::NamedResponse {
            debtor: Some(d), ..
        } => (
            Some(match d {
                Debtor::Person(p) => m.people[p.0].name.clone(),
                Debtor::Actor(a) => m.actors[a.0].name.clone(),
            }),
            DebtorStatus::Declared,
        ),
        _ => (None, DebtorStatus::NotConfigured),
    }
}
