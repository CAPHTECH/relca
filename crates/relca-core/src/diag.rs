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
pub const W_CUT_2: &str = "W-CUT-2";
pub const W_DEADLOCK_0: &str = "W-DEADLOCK-0";
pub const W_BUDGET_0: &str = "W-BUDGET-0";
pub const E_CONF_0: &str = "E-CONF-0";
pub const W_CONF_0: &str = "W-CONF-0";
pub const W_CONF_1: &str = "W-CONF-1";
pub const W_CONF_2: &str = "W-CONF-2";
pub const E_DIFF_0: &str = "E-DIFF-0";
pub const W_DIFF_1: &str = "W-DIFF-1";
pub const W_DIFF_2: &str = "W-DIFF-2";
pub const W_DIFF_3: &str = "W-DIFF-3";
pub const E_CMP_0: &str = "E-CMP-0";
pub const W_CMP_1: &str = "W-CMP-1";
pub const W_AGY_0: &str = "W-AGY-0";
pub const W_AGY_1: &str = "W-AGY-1";
pub const W_AGY_2: &str = "W-AGY-2";
pub const W_AGY_3: &str = "W-AGY-3";
pub const W_AGY_4: &str = "W-AGY-4";
pub const W_AGY_5: &str = "W-AGY-5";
pub const W_LIN_2: &str = "W-LIN-2";
pub const W_RESP_0: &str = "W-RESP-0";
pub const W_RESP_1: &str = "W-RESP-1";
pub const W_RESP_2: &str = "W-RESP-2";
pub const W_RESP_3: &str = "W-RESP-3";
pub const W_CONTEST_0: &str = "W-CONTEST-0";
pub const W_CONTEST_1: &str = "W-CONTEST-1";
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
    W_CUT_2,
    W_DEADLOCK_0,
    W_BUDGET_0,
    E_CONF_0,
    W_CONF_0,
    W_CONF_1,
    W_CONF_2,
    E_DIFF_0,
    W_DIFF_1,
    W_DIFF_2,
    W_DIFF_3,
    E_CMP_0,
    W_CMP_1,
    W_AGY_0,
    W_AGY_1,
    W_AGY_2,
    W_AGY_3,
    W_AGY_4,
    W_AGY_5,
    W_LIN_2,
    W_RESP_0,
    W_RESP_1,
    W_RESP_2,
    W_RESP_3,
    W_CONTEST_0,
    W_CONTEST_1,
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
/// omitted. A documented reason token or a fixed identifier supplied by the
/// detector is stable; explanatory message text is not a machine identifier.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    #[serde(flatten)]
    pub presentation: Presentation,
}

impl std::fmt::Debug for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Internal exploration fingerprints describe the diagnostic facts,
        // not their source-dependent presentation in a report.
        f.debug_struct("Diagnostic")
            .field("code", &self.code)
            .field("severity", &self.severity)
            .field("message", &self.message)
            .field("span", &self.span)
            .finish()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presentation {
    pub location: Option<Box<Location>>,
    pub detail: Option<Box<Detail>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

pub(crate) fn line_starts(src: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            src.bytes()
                .enumerate()
                .filter_map(|(i, b)| (b == b'\n').then_some(i + 1)),
        )
        .collect()
}

impl Location {
    pub(crate) fn indexed(src: &str, starts: &[usize], span: Span) -> Option<Self> {
        if span.start > span.end
            || !src.is_char_boundary(span.start)
            || !src.is_char_boundary(span.end)
        {
            return None;
        }
        let at = |byte| {
            let line = starts.partition_point(|start| *start <= byte);
            (line, src[starts[line - 1]..byte].chars().count() + 1)
        };
        let (line, column) = at(span.start);
        let (end_line, end_column) = at(span.end);
        Some(Self {
            line,
            column,
            end_line,
            end_column,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailKind {
    Rewrite,
    AnalysisNotConnected,
    PropertyUndecided,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detail {
    pub reason: String,
    pub kind: DetailKind,
    pub cause: Option<String>,
    pub blockers: Vec<Blocker>,
    pub see: String,
}

impl Diagnostic {
    pub(crate) fn unconnected(self, reason: &str, cause: &str, blockers: Vec<Blocker>) -> Self {
        self.described(
            reason,
            cause,
            DetailKind::AnalysisNotConnected,
            blockers,
            "docs/guide/dsl.md#解析が未接続のとき",
        )
    }

    /// Attach report-only information without altering the original refusal.
    pub(crate) fn described(
        mut self,
        reason: &str,
        cause: &str,
        kind: DetailKind,
        blockers: Vec<Blocker>,
        see: &str,
    ) -> Self {
        self.presentation.detail = Some(Box::new(Detail {
            reason: reason.into(),
            kind,
            cause: Some(cause.into()),
            blockers,
            see: see.into(),
        }));
        self
    }

    pub(crate) fn loop_detail(
        self,
        cause: &str,
        kind: DetailKind,
        repeat: Option<Span>,
        feature: Option<Span>,
    ) -> Self {
        let mut blockers = Vec::new();
        if let Some(span) = repeat {
            blockers.push(Blocker::at("repeat", None, Some(span)));
        }
        if let Some(span) = feature {
            blockers.push(Blocker::at("feature", None, Some(span)));
        }
        self.described(
            "loop_feature_combination",
            cause,
            kind,
            blockers,
            "docs/guide/dsl.md#繰り返し",
        )
    }

    pub(crate) fn merge_detail(self, cause: &str, blockers: Vec<Blocker>) -> Self {
        self.described(
            "merge_feature_combination",
            cause,
            DetailKind::AnalysisNotConnected,
            blockers,
            "docs/guide/dsl.md#分岐した値を先へ渡す",
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blocker {
    pub role: String,
    pub name: Option<String>,
    pub location: Option<Location>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exits: Vec<EvidenceExit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rounds: Vec<u64>,
    // Internal provenance, resolved when a report is projected onto its source.
    #[serde(skip)]
    pub(crate) span: Option<Span>,
    #[serde(skip)]
    repeat_name_span: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceExit {
    pub loop_name: String,
    pub round: u64,
    pub phase: crate::ir::LoopPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leave_site: Option<u64>,
}

impl Blocker {
    pub(crate) fn at(role: &str, name: Option<String>, span: Option<Span>) -> Self {
        Self {
            role: role.into(),
            name,
            location: None,
            exits: Vec::new(),
            rounds: Vec::new(),
            span,
            repeat_name_span: false,
        }
    }

    pub(crate) fn repeat_from_name(span: Span) -> Self {
        let mut row = Self::at("repeat", None, Some(span));
        row.repeat_name_span = true;
        row
    }

    pub(crate) fn for_call(
        role: &str,
        model: &crate::ir::Model,
        call: &crate::ir::Call,
        public: crate::ir::BindingId,
    ) -> Self {
        if let Some(site) = model.loop_site(call.binding) {
            let mut row = Self::at(role, Some(site.call_site.clone()), Some(call.span));
            row.rounds.extend(site.round);
            row
        } else {
            Self::at(
                role,
                model.process.bindings.get(public.0).map(|b| b.name.clone()),
                Some(call.span),
            )
        }
    }
}

impl DetailKind {
    pub(crate) fn loop_cause(cause: &str) -> Self {
        if cause == "value_returning_if" {
            Self::Rewrite
        } else {
            Self::AnalysisNotConnected
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rewrite => "rewrite",
            Self::AnalysisNotConnected => "analysis_not_connected",
            Self::PropertyUndecided => "property_undecided",
        }
    }
}

pub(crate) fn locate<'a>(src: &str, diagnostics: impl IntoIterator<Item = &'a mut Diagnostic>) {
    // Lower retains the loop name's existing span, not another semantic
    // span. For report-only positions, accept only an adjacent whole token.
    fn repeat_token(src: &str, span: Span, from_name: bool) -> Option<Span> {
        let is_word_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let end = if from_name {
            let before = src
                .get(..span.start)?
                .trim_end_matches([' ', '\t', '\r', '\n']);
            before.strip_suffix("repeat")?;
            before.len()
        } else if src.get(span.start..)?.starts_with("repeat")
            && !src
                .as_bytes()
                .get(span.start + 6)
                .is_some_and(|b| is_word_byte(*b))
        {
            span.start + 6
        } else {
            return None;
        };
        let start = end - 6;
        if start > 0 && is_word_byte(src.as_bytes()[start - 1]) {
            return None;
        }
        Some(Span { start, end })
    }
    let mut starts = None;
    for d in diagnostics {
        if d.span.is_none() && d.presentation.detail.is_none() {
            continue;
        }
        let starts = starts.get_or_insert_with(|| line_starts(src));
        let at = |span| Location::indexed(src, starts, span);
        d.presentation.location = d.span.and_then(at).map(Box::new);
        if let Some(detail) = &mut d.presentation.detail {
            for b in &mut detail.blockers {
                if let Some(span) = b.span.take() {
                    b.location = if b.role == "repeat" {
                        repeat_token(src, span, b.repeat_name_span).and_then(at)
                    } else {
                        at(span)
                    };
                }
            }
            detail.blockers.sort_by(|a, b| {
                (a.location.is_none(), a.location, &a.role, &a.name, &a.exits).cmp(&(
                    b.location.is_none(),
                    b.location,
                    &b.role,
                    &b.name,
                    &b.exits,
                ))
            });
            detail.blockers.dedup_by(|a, b| {
                if (a.role.as_str(), &a.name, a.location, &a.exits)
                    != (b.role.as_str(), &b.name, b.location, &b.exits)
                {
                    return false;
                }
                b.rounds.extend(&a.rounds);
                b.rounds.sort_unstable();
                b.rounds.dedup();
                true
            });
        }
    }
}
