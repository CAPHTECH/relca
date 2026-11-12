use super::input::{Event, Time};
use crate::diag::Span;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tri {
    Conformant,
    Violating,
    Unknown,
}
impl Tri {
    pub fn fold(values: impl IntoIterator<Item = Self>) -> Self {
        let mut any = false;
        let mut unknown = false;
        for v in values {
            any = true;
            match v {
                Self::Violating => return Self::Violating,
                Self::Unknown => unknown = true,
                Self::Conformant => (),
            }
        }
        if !any || unknown {
            Self::Unknown
        } else {
            Self::Conformant
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub event_id: String,
    pub line: usize,
    pub case_id: String,
    pub task_id: String,
    pub time: Time,
    pub after: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub id: String,
    pub case_id: Option<String>,
    pub conformance: Tri,
    pub reason: Option<String>,
    pub evidence: Vec<Evidence>,
    pub model_span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_phase: Option<String>,
}
impl Check {
    pub fn new(
        id: impl Into<String>,
        case: Option<&str>,
        conformance: Tri,
        reason: Option<&str>,
        indices: &[usize],
        events: &[Event],
        model_span: Option<Span>,
    ) -> Self {
        let mut indices = indices.to_vec();
        indices.sort_by(|a, b| events[*a].event_id.cmp(&events[*b].event_id));
        indices.dedup();
        Self {
            id: id.into(),
            case_id: case.map(String::from),
            conformance,
            reason: reason.map(String::from),
            evidence: indices
                .into_iter()
                .map(|i| {
                    let e = &events[i];
                    Evidence {
                        event_id: e.event_id.clone(),
                        line: e.line,
                        case_id: e.case_id.clone(),
                        task_id: e.task_id.clone(),
                        time: e.time,
                        after: e.after.clone(),
                    }
                })
                .collect(),
            model_span,
            missing_phase: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Tri::*;
    #[test]
    fn fold_preserves_independent_violations_but_never_empty_success() {
        assert_eq!(super::Tri::fold([]), Unknown);
        assert_eq!(super::Tri::fold([Conformant, Unknown]), Unknown);
        assert_eq!(super::Tri::fold([Unknown, Violating]), Violating);
        assert_eq!(super::Tri::fold([Conformant, Conformant]), Conformant);
    }
}
