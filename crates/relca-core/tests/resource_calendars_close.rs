#[path = "support/q4_close.rs"]
mod close;
#[path = "support/q4_review.rs"]
mod review;
use relca_core::{diff, engine, ir, lower, reference, report, syntax};
use serde_json::json;

#[test]
fn mixed_if_wait_withdraws_all_candidate_effects_not_only_the_suffix() {
    for shape in 0..3 {
        for condition in [false, true] {
            close::mixed_assert(
                &json!(report::check(&close::mixed(shape, condition))),
                shape,
                condition,
            );
        }
    }
}

#[test]
fn diff_condition_wait_keeps_refusal_diagnostic_and_confirmed_child_violation() {
    let control = diff::diff(&close::diff(false, false), "From", "To");
    assert_eq!(control.verdict, relca_core::Verdict::Satisfied);
    let unselected = diff::diff(&close::diff(false, true), "From", "From");
    assert_eq!(unselected.verdict, relca_core::Verdict::Satisfied);
    for (from, to) in [(true, false), (false, true), (true, true)] {
        let value = json!(diff::diff(&close::diff(from, to), "From", "To"));
        assert_eq!(value["verdict"], "violated");
        assert!(value["diagnostics"].as_array().unwrap().iter().any(|d| {
            d["code"] == "U-SEM-0"
                && d["message"]
                    .as_str()
                    .unwrap()
                    .contains("condition_wait_diff_unsupported")
        }));
        for (owner, waiting) in [("from", from), ("to", to)] {
            let child = &value[owner]["report"];
            assert_eq!(
                child["verdict"],
                if waiting { "violated" } else { "satisfied" }
            );
            assert_eq!(child["bounds"]["truncated"], false);
            if waiting {
                assert!(
                    child["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["code"] == "W-DEADLOCK-0")
                );
            }
        }
        assert_eq!(value["differences"], json!([]));
    }
}

#[test]
fn public_ir_contest_condition_wait_is_rejected_before_static_certificates_are_used() {
    let mut m =
        lower::lower(&syntax::parse(&close::contest(false, false, 1, false)).unwrap()).unwrap();
    m.process.body.insert(
        1,
        ir::Statement {
            kind: ir::Stmt::WaitUntil(ir::Cond::Eq(ir::ResultMatch {
                binding: ir::BindingId(0),
                value: ir::ValueId(0),
            })),
            span: m.process.body[0].span,
        },
    );
    let outcome = engine::explore(&m);
    assert!(
        outcome.diagnostics.iter().any(
            |d| d.code == "U-SEM-0" && d.message.contains("condition_wait_contest_unsupported")
        )
    );
    assert!(outcome.terminals.is_empty());
    assert_eq!(
        reference::explore(&m).stop,
        reference::StopReason::SemanticAbort
    );
}

#[test]
fn contest_condition_wait_never_certifies_reversal_or_hides_restricted_authority() {
    for effect in [false, true] {
        let control = json!(report::check(&close::contest(false, false, 1, effect)));
        assert_eq!(control["verdict"], "satisfied", "{control}");
        for restricted in [false, true] {
            let value = json!(report::check(&close::contest(true, restricted, 1, effect)));
            close::refused(&value, "condition_wait_contest_unsupported");
            assert!(value.get("contest_paths").is_none_or(|v| v.is_null()));
        }
    }
}
