//! Implementation-owned artificial-state tests: these are not the independent
//! frozen review oracle. Normal DSL could not reach the missing-note states.
use super::*;

fn setup() -> (Lower, ast::Process, ir::Block) {
    let src = r#"model Annotation version "0.2" {
        type Status = enum { ok, fail }
        actor Probe { capacity 1; capability check() -> Status { duration fixed 1min; result nondet { ok, fail } } }
        process Flow { status = Probe.check(); degrade Manual when status == fail { end done } else { end done } }
        bounds { cases 1; live_tasks 1; clock 2min } verify reachable end.done }"#;
    let ast = crate::syntax::parse(src).unwrap();
    let m = crate::lower::lower(&ast).unwrap();
    let p = ast
        .items
        .into_iter()
        .find_map(|item| match item {
            ast::Item::Process(p) => Some(p),
            _ => None,
        })
        .unwrap();
    let ir::Stmt::Call(call) = &m.process.body[0].kind else {
        panic!("producer")
    };
    let mut lower = Lower::default();
    lower.calls.insert("status".into(), call.clone());
    (lower, p, m.process.body)
}

fn fault(name: &str, prior_error: bool) {
    let (mut lower, p, mut body) = setup();
    let span = p.degradations[0].span;
    match name {
        "missing_statement" => body.retain(|s| s.span != span),
        "not_if" => {
            body.iter_mut().find(|s| s.span == span).unwrap().kind = ir::Stmt::End(ir::EndId(0))
        }
        "missing_producer" => lower.calls.clear(),
        _ => unreachable!(),
    }
    if prior_error {
        lower.error(diag::E_NAME_0, "existing error", p.span);
    }
    assert!(lower.degradations(&p, &body).is_empty());
    if prior_error {
        assert_eq!(lower.errors.len(), 1);
        assert_eq!(lower.errors[0].message, "existing error");
    } else {
        assert!(
            lower
                .errors
                .iter()
                .any(|d| d.code == diag::U_SEM_0 && d.message.contains(name)),
            "{name}: {:?}",
            lower.errors
        );
    }
}

#[test]
fn complete_annotation_remains_connected() {
    let (mut lower, p, body) = setup();
    assert_eq!(lower.degradations(&p, &body).len(), 1);
    assert!(lower.errors.is_empty());
}
#[test]
fn missing_statement_annotation_is_not_silently_dropped() {
    fault("missing_statement", false);
}
#[test]
fn non_if_annotation_is_not_silently_dropped() {
    fault("not_if", false);
}
#[test]
fn missing_producer_annotation_is_not_silently_dropped() {
    fault("missing_producer", false);
}
#[test]
fn prior_error_is_preserved_without_cascading_annotation_diagnostics() {
    for name in ["missing_statement", "not_if", "missing_producer"] {
        fault(name, true);
    }
}
