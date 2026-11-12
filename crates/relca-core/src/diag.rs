//! Shared diagnostic vocabulary. Unsupported is an error severity, not a warning.

use serde::{Deserialize, Serialize};

pub const E_SYN_0: &str = "E-SYN-0";
pub const E_ARR_0: &str = "E-ARR-0";
pub const E_SIM_0: &str = "E-SIM-0";
pub const U_SYN_0: &str = "U-SYN-0";
pub const U_SEM_0: &str = "U-SEM-0";
pub const U_LEDGER_0: &str = "U-LEDGER-0";
pub const E_NAME_0: &str = "E-NAME-0";
pub const E_TYPE_0: &str = "E-TYPE-0";
pub const E_PROB_0: &str = "E-PROB-0";
pub const E_POL_LATENT: &str = "E-POL-LATENT";
pub const E_CAP_0: &str = "E-CAP-0";
pub const E_CANCEL_0: &str = "E-CANCEL-0";
pub const E_CON_0: &str = "E-CON-0";
pub const E_CUT_1: &str = "E-CUT-1";
pub const W_CUT_0: &str = "W-CUT-0";
pub const W_CUT_1: &str = "W-CUT-1";
pub const W_DEADLOCK_0: &str = "W-DEADLOCK-0";
pub const W_BUDGET_0: &str = "W-BUDGET-0";
pub const E_CONF_0: &str = "E-CONF-0";
pub const W_CONF_0: &str = "W-CONF-0";
pub const E_DIFF_0: &str = "E-DIFF-0";
pub const W_DIFF_1: &str = "W-DIFF-1";
pub const W_DIFF_2: &str = "W-DIFF-2";
pub const W_DIFF_3: &str = "W-DIFF-3";
pub const E_CMP_0: &str = "E-CMP-0";
pub const W_AGY_0: &str = "W-AGY-0";
pub const W_AGY_1: &str = "W-AGY-1";
pub const W_AGY_2: &str = "W-AGY-2";
pub const W_AGY_3: &str = "W-AGY-3";
pub const W_AGY_4: &str = "W-AGY-4";
pub const W_LIN_2: &str = "W-LIN-2";
pub const W_RESP_0: &str = "W-RESP-0";
pub const W_RESP_1: &str = "W-RESP-1";
pub const W_RESP_2: &str = "W-RESP-2";
pub const W_CONTEST_0: &str = "W-CONTEST-0";
pub const W_AUTH_0: &str = "W-AUTH-0";
pub const W_UNREACH_0: &str = "W-UNREACH-0";
pub const W_UNREACH_1: &str = "W-UNREACH-1";
pub const W_UNREACH_2: &str = "W-UNREACH-2";
pub const W_UNREACH_3: &str = "W-UNREACH-3";
pub const E_LOG_0: &str = "E-LOG-0";
pub const E_LOG_1: &str = "E-LOG-1";
pub const E_LOG_2: &str = "E-LOG-2";
pub const U_LOG_0: &str = "U-LOG-0";
pub const U_LOG_1: &str = "U-LOG-1";
pub const W_LOG_0: &str = "W-LOG-0";
pub const W_LOG_1: &str = "W-LOG-1";
pub const W_LOG_2: &str = "W-LOG-2";
pub const W_LOG_3: &str = "W-LOG-3";
pub const W_LOG_4: &str = "W-LOG-4";

/// The whole vocabulary. A code outside it is not a Relca diagnostic.
pub const CODES: &[&str] = &[
    U_LEDGER_0,
    E_LOG_0,
    E_LOG_1,
    E_LOG_2,
    U_LOG_0,
    U_LOG_1,
    W_LOG_0,
    W_LOG_1,
    W_LOG_2,
    W_LOG_3,
    W_LOG_4,
    W_UNREACH_0,
    W_UNREACH_1,
    W_UNREACH_2,
    W_UNREACH_3,
    E_SYN_0,
    E_ARR_0,
    E_SIM_0,
    U_SYN_0,
    U_SEM_0,
    E_NAME_0,
    E_TYPE_0,
    E_PROB_0,
    E_POL_LATENT,
    E_CAP_0,
    E_CANCEL_0,
    E_CON_0,
    E_CUT_1,
    W_CUT_0,
    W_CUT_1,
    W_DEADLOCK_0,
    W_BUDGET_0,
    E_CONF_0,
    W_CONF_0,
    E_DIFF_0,
    W_DIFF_1,
    W_DIFF_2,
    W_DIFF_3,
    E_CMP_0,
    W_AGY_0,
    W_AGY_1,
    W_AGY_2,
    W_AGY_3,
    W_AGY_4,
    W_LIN_2,
    W_RESP_0,
    W_RESP_1,
    W_RESP_2,
    W_CONTEST_0,
    W_AUTH_0,
];

/// The severity a vocabulary code must carry, or None outside the vocabulary.
pub fn severity_of(code: &str) -> Option<Severity> {
    if !CODES.contains(&code) {
        return None;
    }
    Some(if code.starts_with("W-") {
        Severity::Warning
    } else {
        Severity::Error
    })
}

/// Half-open UTF-8 byte offsets into the original source: start <= end <= len.
/// Offsets must lie on character boundaries; no line/column conversion is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// E-* and U-* use Error; W-* use Warning. The code distinguishes invalid
/// from unsupported, so callers must not classify every Error as invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// Codes are the constants above. A missing span is serialized as null, never
/// omitted. Diagnostic messages are explanatory, not stable machine identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
}
