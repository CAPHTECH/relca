use relca_core::{Verdict, engine, ir, lower, reference, report, syntax};
use serde_json::{Value, json};

const TIE: &str = include_str!("../../../fixtures/mean-repeat-policy/model.relca");
const EARLIER: &str =
    include_str!("../../../fixtures/mean-repeat-policy.broken-earlier-arrival/model.relca");
const FORK: &str = include_str!("support/repeat_policy_fork.relca");

fn once(s: &str, from: &str, to: &str) -> String {
    assert_eq!(s.matches(from).count(), 1, "{from}");
    s.replacen(from, to, 1)
}

fn model(s: &str, plan: &str) -> ir::Model {
    lower::lower_with_selection(&syntax::parse(s).unwrap(), Some(plan), None).unwrap()
}

fn check(s: &str, plan: &str) -> report::Report {
    let result = report::check_with_selection(s, Some(plan), None);
    assert_eq!(
        result.verdict,
        Verdict::Satisfied,
        "{plan}: {:?}",
        result.diagnostics
    );
    result
}

fn means(o: &relca_core::outcome::Outcome) -> Vec<Value> {
    assert!(
        !o.diagnostics
            .iter()
            .any(|d| d.severity == relca_core::diag::Severity::Error),
        "{:?}",
        o.diagnostics
    );
    o.quality.as_ref().unwrap().means.as_ref().unwrap().clone()
}

fn both_exhaustive(m: &ir::Model) -> relca_core::outcome::Outcome {
    let e = engine::explore(m);
    let r = reference::explore(m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(means(&e), means(&r.outcome));
    assert_eq!(e.properties, r.outcome.properties);
    e
}

#[test]
fn second_round_and_new_case_follow_ready_time_then_case_id() {
    for (src, lead, ends, wait) in [(TIE, "15/2", [7, 10], "5"), (EARLIER, "9", [9, 10], "8")] {
        for plan in ["Loop", "Hand"] {
            let m = model(src, plan);
            let out = both_exhaustive(&m);
            assert_eq!(
                means(&out)[0]["value"],
                json!({"lower":lead,"upper":lead,"basis":"exact","reason":null})
            );
            let report = serde_json::to_value(check(src, plan)).unwrap();
            for (i, end) in ends.into_iter().enumerate() {
                assert_eq!(
                    report["case_outcomes"][i]["terminals"][0]["completed_at"],
                    json!({"min":end,"max":end})
                );
            }
            assert_eq!(
                report["metrics"]["actors"][0]["queue"]["wait_total"]["stats"]["min"],
                wait
            );
        }
    }
}

#[test]
fn first_idle_and_sampled_load_match_the_hand_written_process_and_reference() {
    let mut normalized = Vec::new();
    let src = once(
        TIE,
        "actor A { capacity 1",
        "resource Slot capacity 1\n  people [p1,p2]\n  actor A { executors [p2,p1]",
    );
    let src = once(&src, "work() -> T {", "work() -> T { uses Slot: 1;");
    let src = once(&src, "handover() -> T {", "handover() -> T { uses Slot: 1;");
    let src = once(&src, "call_order\n", "call_order assign first_idle\n");
    for plan in ["Loop", "Hand"] {
        let m = model(&src, plan);
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for out in [&e, &r.outcome] {
            assert_eq!(
                out.completed_replications, 2,
                "{plan}: {:?}",
                out.diagnostics
            );
            let rows = out.quality.as_ref().unwrap().means.as_ref().unwrap();
            assert_eq!(
                rows[0]["value"],
                json!({"point":"15/2","observations":4,"censored":0,"reason":null})
            );
            for rep in &out.replications {
                assert!(rep.cut.is_none() && rep.draws.is_empty());
                let v = serde_json::to_value(rep.metrics.as_ref().unwrap()).unwrap();
                let a = &v["actors"][0];
                assert_eq!(v["resources"][0]["utilization"]["min"], "5/6");
                assert_eq!(a["queue"]["wait_total"]["stats"]["min"], "5");
                let people = v["people"].as_array().unwrap();
                let p2 = people.iter().find(|p| p["person"] == "p2").unwrap();
                assert_eq!(p2["work"]["min"], "10");
                normalized.push(json!([
                    rows[0]["value"],
                    a["utilization"],
                    a["queue"]["wait_total"],
                    p2["work"]
                ]));
            }
        }
        let report = report::simulate_with_selection(&src, Some(plan), None);
        assert_eq!(report.verdict, Verdict::Satisfied);
    }
    assert!(normalized.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn simultaneous_fork_branches_use_their_own_lexical_order() {
    for (src, first) in [
        (FORK.to_owned(), "1"),
        (
            once(FORK, "a=A.short(); b=A.long()", "b=A.long(); a=A.short()"),
            "4",
        ),
    ] {
        for plan in ["Loop", "Hand"] {
            let expected = if plan == "Hand" { "1" } else { first };
            let out = both_exhaustive(&model(&src, plan));
            assert_eq!(means(&out)[0]["value"]["lower"], expected);
            assert_eq!(means(&out)[0]["value"]["upper"], expected);
            assert_eq!(means(&out)[1]["value"]["lower"], "10");
        }
    }
}

#[test]
fn sampled_contention_and_fork_use_the_same_keys_as_exhaustive() {
    for (src, lead, observations, wait) in [
        (TIE, "15/2", 4, Some("5")),
        (EARLIER, "9", 4, Some("8")),
        (FORK, "10", 2, None),
    ] {
        for plan in ["Loop", "Hand"] {
            let m = model(src, plan);
            let ro = reference::simulate(&m);
            assert_eq!(ro.stop, reference::StopReason::Complete);
            for o in [engine::simulate(&m), ro.outcome] {
                assert_eq!(o.completed_replications, 2, "{:?}", o.diagnostics);
                let rows = o.quality.unwrap().means.unwrap();
                assert_eq!(
                    rows.last().unwrap()["value"],
                    json!({"point":lead,"observations":observations,"censored":0,"reason":null})
                );
                if let Some(wait) = wait {
                    for rep in o.replications {
                        let m = serde_json::to_value(rep.metrics.unwrap()).unwrap();
                        assert_eq!(m["actors"][0]["utilization"]["min"], "5/6");
                        assert_eq!(m["actors"][0]["queue"]["wait_total"]["stats"]["min"], wait);
                    }
                } else {
                    assert_eq!(rows[0]["value"]["point"], "1");
                }
            }
            assert_eq!(
                report::simulate_with_selection(src, Some(plan), None).verdict,
                Verdict::Satisfied
            );
        }
    }
}

#[test]
fn dispatch_after_finishes_preserves_the_two_case_hand_schedule() {
    for (source, lead, ends, wait) in [(TIE, "15/2", [7, 10], "5"), (EARLIER, "9", [9, 10], "8")] {
        let source = once(
            source,
            "then call_order\n",
            "then call_order dispatch after_finishes\n",
        );
        for plan in ["Loop", "Hand"] {
            let m = model(&source, plan);
            let e = both_exhaustive(&m);
            assert_eq!(
                means(&e)[0]["value"],
                json!({"lower":lead,"upper":lead,"basis":"exact","reason":null})
            );
            let v = serde_json::to_value(check(&source, plan)).unwrap();
            for (i, end) in ends.into_iter().enumerate() {
                assert_eq!(
                    v["case_outcomes"][i]["terminals"][0]["completed_at"],
                    json!({"min":end,"max":end})
                );
            }
            let r = reference::simulate(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            for sample in [engine::simulate(&m), r.outcome] {
                assert_eq!(sample.completed_replications, 2, "{:?}", sample.diagnostics);
                assert_eq!(
                    sample.quality.unwrap().means.unwrap()[0]["value"],
                    json!({"point":lead,"observations":4,"censored":0,"reason":null})
                );
                for rep in sample.replications {
                    let metrics = serde_json::to_value(rep.metrics.unwrap()).unwrap();
                    assert_eq!(metrics["actors"][0]["utilization"]["min"], "5/6");
                    assert_eq!(
                        metrics["actors"][0]["queue"]["wait_total"]["stats"]["min"],
                        wait
                    );
                }
            }
            assert_eq!(
                report::simulate_with_selection(&source, Some(plan), None).verdict,
                Verdict::Satisfied
            );
        }
    }
}

#[test]
fn singleton_serial_repeat_matches_no_policy_and_the_written_process() {
    let single = once(&once(TIE, "cases 2;", "cases 1;"), "[0min,2min]", "[0min]");
    let no_policy = once(
        &single,
        "  policy fifo tie_by case_id then call_order\n",
        "",
    );
    let scheduled = check(&single, "Loop");
    let unscheduled = check(&no_policy, "Loop");
    let written = check(&single, "Hand");
    for r in [&scheduled, &unscheduled, &written] {
        let v = serde_json::to_value(r).unwrap();
        assert_eq!(
            v["properties"][0]["cases"][0]["probability"],
            json!({"min":"1","max":"1"})
        );
        assert_eq!(
            v["case_outcomes"][0]["terminals"][0]["completed_at"],
            json!({"min":5,"max":5})
        );
    }
    assert_eq!(
        serde_json::to_value(scheduled.repetition).unwrap(),
        serde_json::to_value(unscheduled.repetition).unwrap()
    );
    for r in [scheduled.quality, unscheduled.quality, written.quality] {
        assert_eq!(
            r.unwrap().means.unwrap()[0]["value"],
            json!({"lower":"5","upper":"5","basis":"exact","reason":null})
        );
    }
}

#[test]
fn compare_repeat_and_written_agree_with_policy() {
    let r = relca_core::compare::compare(
        TIE,
        &["Loop", "Hand"],
        relca_core::compare::Analysis::Exhaustive,
    );
    assert_eq!(r.verdict, Verdict::Satisfied, "{:?}", r.diagnostics);
    let v = serde_json::to_value(r).unwrap();
    for p in v["plans"].as_array().unwrap() {
        assert_eq!(p["report"]["quality"]["means"][0]["value"]["lower"], "15/2");
    }
}

#[test]
fn priority_admission_and_repeat_use_the_same_unrolled_waiting_behavior() {
    let s = once(
        TIE,
        "  policy fifo",
        "  case_priority fixed [1,2]\n  concurrent cases 1 on_overflow wait_external priority descending tie fifo tie_by case_id\n  policy fifo",
    );
    // Case 2 waits outside until case 1 closes at 5. Both take five minutes
    // of service, giving elapsed 5 and 8 minutes from their arrivals.
    for plan in ["Loop", "Hand"] {
        let out = both_exhaustive(&model(&s, plan));
        assert_eq!(
            means(&out)[0]["value"],
            json!({"lower":"13/2","upper":"13/2","basis":"exact","reason":null})
        );
        let report = serde_json::to_value(check(&s, plan)).unwrap();
        assert_eq!(
            report["case_outcomes"][0]["terminals"][0]["completed_at"],
            json!({"min":5,"max":5})
        );
        assert_eq!(
            report["case_outcomes"][1]["terminals"][0]["completed_at"],
            json!({"min":10,"max":10})
        );
    }
}

#[test]
fn repeat_with_contest_is_still_named_as_an_unsupported_combination() {
    let s = once(
        FORK,
        "  actor A",
        "  people [p]\n  actor Party { control party; standing over [end.done] basis \"claim\" }\n  actor Route { executors [p]; capability respond() -> T { duration fixed 1min; result nondet { ok } } }\n  actor A",
    );
    let s = once(&s, "include [A]", "include [A,Party,Route]");
    let s = once(
        &s,
        "  verify probability",
        "  contract Review { contest Party on end.done route Route.respond reversible_by [p] respond_by 1min }\n  verify probability",
    );
    let r = report::check_with_selection(&s, Some("Loop"), None);
    assert_eq!(r.verdict, Verdict::Unsupported, "{:?}", r.diagnostics);
    assert!(
        r.diagnostics.iter().any(|d| d.code == "U-SEM-0"
            && d.message.contains("repeat")
            && d.message.contains("contest")),
        "{:?}",
        r.diagnostics
    );
}

fn compare_both_plans(s: &str, lead: &str, ends: &[u64]) {
    for plan in ["Loop", "Hand"] {
        let m = model(s, plan);
        let out = both_exhaustive(&m);
        let cases = serde_json::to_value(check(s, plan)).unwrap();
        assert_eq!(
            means(&out).last().unwrap()["value"],
            json!({"lower":lead,"upper":lead,"basis":"exact","reason":null})
        );
        for (i, end) in ends.iter().enumerate() {
            assert_eq!(
                cases["case_outcomes"][i]["terminals"][0]["completed_at"],
                json!({"min":end,"max":end})
            );
        }
    }
}

#[test]
fn observable_and_priority_start_policies_match_written_schedules() {
    let observable = once(TIE, "policy fifo", "policy observable fifo");
    compare_both_plans(&observable, "15/2", &[7, 10]);
    let priority = once(
        TIE,
        "policy fifo tie_by case_id",
        "case_priority fixed [1,2]\n  policy observable priority descending tie fifo tie_by case_id",
    );
    compare_both_plans(&priority, "15/2", &[10, 7]);
}

#[test]
fn calendar_and_deterministic_work_profile_match_written_schedules() {
    let closed = once(
        TIE,
        "actor A { capacity 1",
        "calendar Day through 13min on_close continue { [0min,1min) capacity 0; [1min,13min) capacity 1 }\n  actor A { capacity 1; calendar Day;",
    );
    compare_both_plans(&closed, "19/2", &[10, 11]);
    let profile = once(
        TIE,
        "actor A {",
        "kernel K { * -> { 1: ok } }\n  performance P basis \"deterministic control\" { load none; executor anonymous { kernel K } }\n  actor A {",
    );
    let profile = once(
        &profile,
        "duration fixed 2min; result nondet { ok }",
        "duration fixed 2min; outcomes performance P",
    );
    let profile = once(&profile, "policy fifo", "policy observable fifo");
    compare_both_plans(&profile, "15/2", &[7, 10]);
}

#[test]
fn repeat_with_gap_if_and_wait_until_matches_the_written_schedule() {
    let s = once(&once(TIE, "cases 2;", "cases 1;"), "[0min,2min]", "[0min]");
    let s = once(
        &s,
        "  process Repeated",
        "  observed case { go: bool = true }\n  process Repeated",
    );
    let s = once(
        &s,
        "at_most 2 { a=A.work(); again }",
        "at_most 2 gap fixed 3min { wait until go == true; if go == true { a=A.work(); again } else { leave } }",
    );
    let s = once(
        &s,
        "a1=A.work(); a2=A.work();",
        "wait until go == true; if go == true { a1=A.work(); wait 3min; wait until go == true; if go == true { a2=A.work() } else {} } else {};",
    );
    compare_both_plans(&s, "8", &[8]);
}

#[test]
fn simultaneous_cases_each_with_a_repeat_fork_match_the_written_schedule() {
    let s = once(
        &once(
            &once(FORK, "[0min]", "[0min,0min]"),
            "cases 1; live_tasks 2; clock 12min",
            "cases 2; live_tasks 4; clock 25min",
        ),
        "capacity 1; state ready: bool = false",
        "capacity 1",
    );
    let s = once(&s, "; writes ready; finish { ready=true }", "");
    let s = once(
        &s,
        "  property case.Started = reached(A.ready == true)\n",
        "",
    );
    let s = once(
        &s,
        "  metric mean.First = mean(elapsed, case.Started)\n",
        "",
    );
    compare_both_plans(&s, "39/2", &[19, 20]);
}

#[test]
fn policy_nonqueued_site_feature_diagnostics() {
    for (name, base, marker) in [
        (
            "response_loss",
            include_str!("../../../fixtures/agy-vp-response-control/model.relca"),
            "bounds { cases",
        ),
        (
            "merge",
            include_str!("merge_if/models/M1.relca"),
            "  bounds",
        ),
        (
            "receipt query",
            include_str!("../../relca-cli/tests/snapshots/inputs/r2-recovery.relca"),
            "  scenario Full",
        ),
    ] {
        let src = once(
            base,
            marker,
            &format!("  policy fifo tie_by case_id then call_order\n{marker}"),
        );
        let r =
            report::check_with_selection(&src, None, (name == "receipt query").then_some("Full"));
        assert_eq!(
            r.verdict,
            Verdict::Unsupported,
            "{name}: {:?}",
            r.diagnostics
        );
        assert!(
            r.diagnostics.iter().any(|d| d.code == "U-SEM-0"
                && d.message.contains(name)
                && d.message.contains("policy")),
            "{name}: {:?}",
            r.diagnostics
        );
    }
}
