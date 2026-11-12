//! ADR 0032 boundary-review fixes, checked in the core: the bare-name
//! requires that ADR 0032 accepts, the alias duplicates of finish/start, and
//! the range stop rule shared by the engine and the reference interpreter.
//! The byte comparisons against the pre-change binary live in
//! `crates/relca-cli/tests/actor_observed_review_fixes.rs`.
#[path = "support/actor_observed_acceptance.rs"]
mod h;
use relca_core::{ast, diag, engine, ir, lower, reference, syntax};

const OBSERVED: &str = "model M version \"0.2\" {\n  type T = enum { ok, no }\n  actor A { capacity 1; state n: 0..1 = 0; capability run() -> T { requires n == 0 else no; writes n; finish { n = 1 }; duration fixed 1min; result nondet { ok } } }\n  process P { a = A.run(); end done }\n  bounds { cases 1; live_tasks 1; clock 5min }\n  verify safety no_resource_overcommit\n}\n";

fn errors(s: &str) -> Vec<diag::Diagnostic> {
    syntax::parse(s)
        .and_then(|a| lower::lower(&a))
        .expect_err("must be statically refused")
        .into_iter()
        .filter(|d| d.severity == diag::Severity::Error)
        .collect()
}

fn actor_observed(m: &ir::Model, id: ir::StateVarId) -> bool {
    matches!(
        m.state_variables[id.0].kind,
        ir::StateKind::ActorObserved(_)
    )
}

/// Fix 1: the owner's observed state is still read by a bare name.
#[test]
fn fix1_bare_requires_on_actor_observed_state_is_accepted() {
    let eq = h::model(OBSERVED);
    let lt = h::model(&h::change(OBSERVED, "requires n == 0", "requires n < 1"));
    for (m, operator) in [(&eq, ast::IntComparison::Eq), (&lt, ast::IntComparison::Lt)] {
        let requires = m
            .capabilities
            .iter()
            .find_map(|c| c.requires.as_ref())
            .expect("requires lowered");
        match &requires.condition {
            ir::Cond::IntCompare {
                variable,
                operator: o,
                ..
            } => {
                assert_eq!(*o, operator);
                assert!(actor_observed(m, *variable));
            }
            c => panic!("unexpected requires condition {c:?}"),
        }
    }
}

/// Fix 3: a bare name and its self-qualified alias are one target.
#[test]
fn fix3_bare_and_self_qualified_alias_duplicates_stay_refused() {
    let finish = h::change(OBSERVED, "finish { n = 1 }", "finish { n = 1; A.n = 1 }");
    let start = h::change(
        &h::change(OBSERVED, "requires n == 0 else no; ", ""),
        "finish { n = 1 }",
        "start { n = 1; A.n = 1 }",
    );
    for (s, message) in [
        (finish, "duplicate finish target"),
        (start, "duplicate start target"),
    ] {
        let ds = errors(&s);
        let at = s.find("A.n = 1").unwrap();
        assert!(
            ds.iter().any(|d| d.code == diag::U_SEM_0
                && d.message == message
                && d.span.is_some_and(|sp| sp.start == at && sp.end == at + 3)),
            "{ds:?}\n{s}"
        );
    }
}

/// Fix 4: every range stop leaves `unreachable` unchecked in both
/// interpreters, also in the ADR 0029 case-integer models without Actor state.
#[test]
fn fix4_case_integer_range_stop_unchecked_agrees_between_engine_and_reference() {
    for name in [
        "int-arrivals.broken-overflow",
        "int-set-stop.broken-outward",
        "int-underflow.broken-underflow",
    ] {
        let s = h::source(name);
        let m = h::model(&s);
        assert!(
            !m.state_variables
                .iter()
                .any(|v| matches!(v.kind, ir::StateKind::ActorObserved(_))),
            "{name} must be a model without Actor observed state"
        );
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert!(
            !e.state_range_violations.is_empty() && !r.outcome.state_range_violations.is_empty(),
            "{name} must stop on a range violation"
        );
        assert_eq!(e.unchecked, vec!["unreachable".to_string()], "{name}");
        assert_eq!(r.outcome.unchecked, e.unchecked, "{name}");
    }
}
