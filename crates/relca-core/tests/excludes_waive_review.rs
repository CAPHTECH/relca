const MULTIPLE: &str = r#"
model M version "0.2" {
 type T = enum { ok, bad }
 people [p,q,issuer]
 actor Gate { capacity 2; capability run() -> T { duration fixed 1min; result nondet { ok } } }
 actor Officer { executors [p]; capability release() -> T { commits; duration fixed 1min; result nondet { ok } } }
 actor B { capacity 1; capability run() -> T { duration fixed 3min; result nondet { ok } } }
 actor Wait { capacity 1; capability run() -> T { duration fixed 6min; result nondet { ok } } }
 permit [p,q] to Officer.release
 authority G { basis "grant"; grants [p,q] to Officer.release; issued_by issuer }
 process Flow { g1 = Gate.run(); g2 = Gate.run(); o = Officer.release(); g3 = Gate.run(); w = Wait.run(); end done }
 bounds { cases 1; live_tasks 3; clock 30min }
 verify safety no_resource_overcommit
 contract Controls { response delivery: Gate.run.finish -> one_of(B.run.finish) by 5min after Gate.run.finish otherwise unresolved; waive delivery on Officer.release.finish reason "exception" authorized_by G }
}
"#;

#[test]
fn response_witness_selects_unwaived_occurrence() {
    let report = relca_core::explain::explain(MULTIPLE);
    let value = serde_json::to_value(report).unwrap();
    let witnesses = value["explanation"]["counterexamples"].as_array().unwrap();
    let steps = witnesses[0]["steps"].as_array().unwrap();
    let violation = &steps.last().unwrap()["violation"];
    assert_eq!(violation["deadline"], 9);
    let trigger = violation["trigger_step"].as_u64().unwrap() as usize;
    assert_eq!(trigger, 19);
    assert_eq!(steps[trigger]["binding"], "g3");
    assert_eq!(steps[trigger]["kind"], "finish");
}

fn queued_cancel_model(arrivals: bool) -> String {
    format!(
        r#"model M version "0.2" {{
      type T = enum {{ pass, ok }}
      people [p]
      actor Fast {{ executors [p]; capability win() -> T {{ duration fixed 1min; result nondet {{ pass }}; cancel forbidden_after_start }} }}
      actor Slow {{ executors [p]; capability work() -> T {{ duration fixed 3min; result nondet {{ ok }}; cancel cooperative ack_within 1min }} }}
      actor After {{ capacity 1; capability run() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }}
      permit [p] to Fast.win
      permit [p] to Slow.work
      process Flow {{
        fork f {{ fast = Fast.win(); slow = Slow.work() }}
        join any f where result == pass tie by [fast,slow] remaining cancel_then_drain if none -> rejected
        a = After.run(); end done
        end rejected
      }}
      bounds {{ cases 1; live_tasks 2; clock 10min }}
      {}
      verify safety no_resource_overcommit
      contract C {{ excludes Slow.work.cancel_ack, After.run.start }}
    }}"#,
        if arrivals {
            "arrivals fixed [0min]"
        } else {
            ""
        }
    )
}

fn queued_cancel_is_not_ack(arrivals: bool, reference_route: bool) {
    // Slow is queued whenever Fast wins before it starts; the other schedules
    // may legitimately acknowledge Slow. Capacity zero makes that impossible,
    // retaining the shared-person model but isolating queued cancellation.
    let source = queued_cancel_model(arrivals)
        .replace("actor Slow { executors [p];", "actor Slow { capacity 0;");
    let source = source.replace("      permit [p] to Slow.work", "");
    let model = relca_core::lower::lower(&relca_core::syntax::parse(&source).unwrap()).unwrap();
    let engine = relca_core::engine::explore(&model);
    let reference = relca_core::reference::explore(&model);
    assert_eq!(reference.stop, relca_core::reference::StopReason::Complete);
    let outcome = if reference_route {
        &reference.outcome
    } else {
        &engine
    };
    assert_eq!(
        outcome.properties.last().unwrap().verdict,
        relca_core::Verdict::Vacuous,
        "arrivals={arrivals}: {outcome:?}"
    );
    assert!(!outcome.truncated);
    if arrivals {
        let terminals = &outcome.case_outcomes.as_ref().unwrap()[0].terminals;
        assert_eq!(terminals.len(), 1);
        let relca_core::outcome::CaseTerminalKind::End(end) = terminals[0].kind else {
            panic!("queued cancellation must drain to done")
        };
        assert_eq!(model.ends[end.0], "done");
        let time = terminals[0].completed_at.unwrap();
        assert_eq!((time.min, time.max), (2, 2));
    } else {
        assert!(
            outcome
                .terminals
                .iter()
                .any(|t| model.ends[t.end.0] == "done")
        );
    }
}

#[test]
fn queued_cancel_engine() {
    queued_cancel_is_not_ack(false, false);
}
#[test]
fn queued_cancel_engine_arrivals() {
    queued_cancel_is_not_ack(true, false);
}
#[test]
fn queued_cancel_reference() {
    queued_cancel_is_not_ack(false, true);
}
#[test]
fn queued_cancel_reference_arrivals() {
    queued_cancel_is_not_ack(true, true);
}

const DEADLOCK: &str = r#"model M version "0.2" {
 type T = enum { ok }
 actor A { capacity 0; capability run() -> T { duration fixed 1min; result nondet { ok } } }
 process P { a = A.run(); end done }
 bounds { cases 1; live_tasks 2; clock 10min }
 verify safety no_resource_overcommit
 contract C { response delivery: case.received -> one_of(end.done) by 5min after case.received otherwise unresolved }
}"#;

#[test]
fn ledger_deadlock_explanations_are_verified() {
    for arrivals in [false, true] {
        let source = if arrivals {
            DEADLOCK.replace("verify safety", "arrivals fixed [0min]\n verify safety")
        } else {
            DEADLOCK.into()
        };
        let value = serde_json::to_value(relca_core::explain::explain(&source)).unwrap();
        assert_eq!(value["explanation"]["status"], "complete", "{value:#}");
        let examples = value["explanation"]["counterexamples"].as_array().unwrap();
        assert_eq!(examples.len(), 2);
        for example in examples {
            assert_eq!(example["replay"]["status"], "verified", "{example:#}");
            let detail = &example["steps"].as_array().unwrap().last().unwrap()["deadlock_detail"];
            assert_eq!(
                detail["obligations"].as_array().unwrap().len(),
                1,
                "arrivals={arrivals} {example:#}"
            );
            assert_eq!(detail["obligations"][0]["deadline"], 5);
        }
    }
}

#[test]
fn arrivals_clock_expiry_explanation_is_verified() {
    let source = DEADLOCK
        .replace("capacity 0", "capacity 1")
        .replace("fixed 1min", "fixed 20min")
        .replace("clock 10min", "clock 2min")
        .replace("by 5min", "by 1min")
        .replace("verify safety", "arrivals fixed [0min]\n verify safety");
    let report = relca_core::explain::explain(&source);
    assert_eq!(
        report.explanation.status,
        relca_core::explain::Status::Complete,
        "{report:?}"
    );
    let witness = &report.explanation.counterexamples[0];
    assert_eq!(witness.status, relca_core::explain::ReplayStatus::Verified);
    assert_eq!(
        witness
            .steps
            .last()
            .unwrap()
            .violation
            .as_ref()
            .unwrap()
            .deadline,
        Some(1)
    );
}

#[test]
fn simulation_marks_unreported_obligation_table_unchecked() {
    let source = include_str!("../../../fixtures/sim-contract/model.relca");
    let source = source.replace("response case.received", "response delivery: case.received");
    let value = serde_json::to_value(relca_core::report::simulate(&source)).unwrap();
    assert!(
        value["unchecked"]
            .as_array()
            .is_some_and(|xs| xs.iter().any(|x| x == "obligation_outcomes")),
        "{value:#}"
    );
}

fn model(source: &str) -> relca_core::ir::Model {
    relca_core::lower::lower(&relca_core::syntax::parse(source).unwrap()).unwrap()
}

fn assert_unresolved(outcome: &relca_core::outcome::Outcome, expected: &str) {
    let table = outcome.obligations.as_ref().unwrap();
    assert!(!table.is_empty(), "{outcome:?}");
    let row = &table[0];
    assert_eq!(row.trigger_site, "received");
    assert_eq!(row.outcomes.len(), 1);
    let resolution = &row.outcomes[0];
    assert_eq!(
        resolution.status,
        relca_core::obligation::Status::Unresolved
    );
    assert_eq!(resolution.reason.as_deref(), Some(expected));
    assert_eq!(resolution.triggered_at, 0);
    assert_eq!(resolution.deadline, 5);
    assert_eq!(resolution.resolved_at, None);
}

#[test]
fn unresolved_cut_and_abort_reasons_have_hand_values() {
    for arrivals in [false, true] {
        let source = if arrivals {
            DEADLOCK.replace("verify safety", "arrivals fixed [0min]\n verify safety")
        } else {
            DEADLOCK.into()
        };
        let live = model(&source.replace("live_tasks 2", "live_tasks 0"));
        assert_unresolved(&relca_core::engine::explore(&live), "live_tasks");
        assert_unresolved(&relca_core::reference::explore(&live).outcome, "live_tasks");
        assert_unresolved(
            &relca_core::engine::explore_with_budget(&model(&source), if arrivals { 6 } else { 1 }),
            "state_budget",
        );
        let abort = model(&source.replace("capacity 0", "capacity 1").replace("otherwise unresolved }", "otherwise unresolved; response A.run.finish -> one_of(end.done) by 18446744073709551615min after A.run.finish otherwise unresolved }"));
        assert_unresolved(&relca_core::engine::explore(&abort), "aborted");
        assert_unresolved(&relca_core::reference::explore(&abort).outcome, "aborted");
    }
}
