//! Connect as a cfg(test) child of report. Normalization preserves None as Ok;
//! it must not invent an error that production record did not return.
mod private_types {
    pub use crate::ir::{ActorId, Model, PersonId};
}
#[path = "q6c_record.rs"]
mod fixed;
use super::work_profiles::{self, WorkProfiles};
trait Normalize {
    fn normalize(self) -> Result<Option<serde_json::Value>, String>;
}
impl Normalize for Option<WorkProfiles> {
    fn normalize(self) -> Result<Option<serde_json::Value>, String> {
        Ok(self.map(|v| serde_json::to_value(v).unwrap()))
    }
}
impl<E: std::fmt::Debug> Normalize for Result<Option<WorkProfiles>, E> {
    fn normalize(self) -> Result<Option<serde_json::Value>, String> {
        self.map_err(|e| format!("{e:?}"))
            .and_then(Normalize::normalize)
    }
}
const SOURCE: &str = r#"model Private version "0.2" {
    type R = enum { ok }
    people [p,q]
    kernel K { * -> { 1:ok } }
    performance P basis "hand assumption" { load none; executor q { kernel K } }
    actor Desk { executors [p,q]; capability run() -> R {
        qualified_executors [q]; duration fixed 2min; outcomes performance P
    } }
    process Flow { r = Desk.run(); end done }
    cut C { include [Desk] }
    bounds { cases 1; live_tasks 2; clock 10min }
    verify reachable end.done
}"#;
#[test]
fn m3_q6c_record_never_silently_omits_a_broken_declaration() {
    let m = crate::lower::lower(&crate::syntax::parse(SOURCE).unwrap()).unwrap();
    fixed::check_record(|m, s| work_profiles::record(m, s).normalize(), SOURCE, &m);
}
#[test]
fn m3_q6c_record_absence_is_only_for_an_old_model() {
    let src = SOURCE
        .replace("qualified_executors [q];", "")
        .replace("outcomes performance P", "result nondet { ok }")
        .replace(
            "performance P basis \"hand assumption\" { load none; executor q { kernel K } }",
            "",
        );
    let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
    assert!(
        work_profiles::record(&m, &src)
            .normalize()
            .unwrap()
            .is_none()
    );
}
