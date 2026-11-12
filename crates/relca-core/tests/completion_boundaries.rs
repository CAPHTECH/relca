//! Supplemental hand oracles; these are not extra fixture-directory pairs.
use relca_core::{Verdict, engine, lower, report, syntax};
use serde_json::{Value, json};
fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
fn preflight(s: &str) {
    let prefix = s.split("verify ").next().unwrap();
    let old = format!("{prefix} verify safety no_resource_overcommit }}");
    let a = syntax::parse(&old).expect("old syntax residual must parse before implementation");
    if s.contains("configuration Selected") {
        lower::lower_configuration(&a, "Selected").expect("old selected skeleton must lower");
    } else {
        lower::lower(&a).expect("old skeleton must lower");
    }
}
fn valid_check(s: &str) -> Value {
    preflight(s);
    check(s)
}
fn p<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing {id}: {v}"))
}
fn ids(v: &Value, code: &str) -> Vec<Value> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == code)
        .cloned()
        .collect()
}
fn reference_hand(src: &str, truncated: bool, hand: &[(&str, Option<(&str, &str)>)]) {
    let ast = syntax::parse(src).unwrap();
    let model = if src.contains("configuration Selected") {
        lower::lower_configuration(&ast, "Selected").unwrap().model
    } else {
        lower::lower(&ast).unwrap()
    };
    let mut r = relca_core::reference::explore(&model);
    assert_eq!(r.stop, relca_core::reference::StopReason::Complete);
    assert_eq!(r.outcome.truncated, truncated);
    assert_eq!(r.outcome.properties.len(), hand.len());
    for (p, (verdict, probability)) in r.outcome.properties.iter().zip(hand) {
        assert_eq!(
            serde_json::to_value(p.verdict).unwrap(),
            *verdict,
            "independent reference hand verdict"
        );
        let check = |v: Verdict, q: &Option<relca_core::probability::ProbabilityRange>| {
            assert_eq!(serde_json::to_value(v).unwrap(), *verdict);
            assert_eq!(
                q.as_ref().map(|q| (q.min.to_string(), q.max.to_string())),
                probability.map(|(a, b)| (a.to_owned(), b.to_owned())),
                "independent reference hand endpoints"
            );
        };
        if let Some(cs) = &p.cases {
            for c in cs {
                check(c.verdict, &c.probability);
            }
        } else {
            check(p.verdict, &p.probability);
        }
    }
    // Only after the reference has independently met the hand property oracle.
    assert!(r.outcome.diagnostics.starts_with(&model.diagnostics));
    r.outcome.diagnostics.drain(..model.diagnostics.len());
    assert_eq!(
        engine::explore(&model),
        r.outcome,
        "supplemental all-field differential"
    );
}
const MIXED: &str = r#"model M version "0.2" {
 type T=enum {good,bad,slow,ok}
 kernel K { * -> {0.5:good;0.25:bad;0.25:slow} }
 actor Judge {capacity 1;capability run()->T {duration fixed 1min;outcomes kernel K}}
 actor Blocked {capacity 0;capability run()->T {duration fixed 1min;result nondet {ok}}}
 actor Slow {capacity 1;capability run()->T {duration fixed 5min;result nondet {ok}}}
 process Flow {j=Judge.run();if j==good {end done} else {if j==bad {b=Blocked.run();end done} else {s=Slow.run();end done}}}
 bounds {cases 1;live_tasks 2;clock 2min}
 verify recoverable end verify terminating end verify almost_sure end
 verify probability reached end.done within 10min >= 1
}"#;

#[test]
fn every_boundary_old_syntax_template_and_single_edit_lowers_before_new_runtime() {
    // Separate preflight prevents a RED assertion in the first variant from
    // hiding an accidental typo in a later variant of the same test.
    let file = include_str!("completion_boundaries.rs");
    let mut count = 0;
    for (start, _) in file.match_indices("r#\"model ") {
        let source = &file[start + 3..];
        let end = source.find("\"#;").unwrap();
        let source = &source[..end];
        preflight(source);
        count += 1;
        for (from, to) in [
            ("actor Slow {capacity 0", "actor Slow {capacity 1"),
            ("queue capacity 0", "queue capacity 1"),
            ("b=Blocked.run();", ""),
            ("if a==yes {if", "if (a==yes && a==yes) {if"),
        ] {
            if source.contains(from) {
                preflight(&source.replace(from, to));
            }
        }
    }
    assert_eq!(count, 8, "explicit raw boundary-template census");
    let old = include_str!("../../../fixtures/any-drain/model.relca");
    lower::lower(&syntax::parse(old).unwrap()).unwrap();
}

#[test]
fn known_deadlock_wins_over_cut_but_cut_only_failure_is_inconclusive() {
    // 1/2 end + 1/4 deadlock + 1/4 unknown. min-upper=3/4<1.
    for (src, prob, verdict) in [
        (
            MIXED.to_string(),
            json!({"min":"1/2","max":"3/4"}),
            "violated",
        ),
        (
            MIXED.replace("b=Blocked.run();", ""),
            json!({"min":"3/4","max":"1"}),
            "inconclusive",
        ),
    ] {
        let v = valid_check(&src);
        for id in ["recoverable end", "terminating end", "almost_sure end"] {
            assert_eq!(p(&v, id)["verdict"], verdict, "{id}");
        }
        assert_eq!(p(&v, "almost_sure end")["probability"], prob);
        assert_eq!(v["unchecked"], json!(["unreachable"]));
        for c in 0..4 {
            assert!(ids(&v, &format!("W-UNREACH-{c}")).is_empty());
        }
        reference_hand(
            &src,
            true,
            &[
                (verdict, None),
                (verdict, None),
                (
                    verdict,
                    Some((prob["min"].as_str().unwrap(), prob["max"].as_str().unwrap())),
                ),
                (
                    verdict,
                    Some((prob["min"].as_str().unwrap(), prob["max"].as_str().unwrap())),
                ),
            ],
        );
    }
}

#[test]
fn empty_arms_nested_dead_if_short_circuit_and_duplicate_labels_are_lexical() {
    let src = r#"model M version "0.2" {type T=enum {yes,no}
 actor A {capacity 1;capability run()->T {duration fixed 1min;result nondet {yes}}}
 process Flow {a=A.run();if a==yes {if a==yes {} else {}} else {if a==yes {} else {}};end done;end done}
 bounds {cases 1;live_tasks 1;clock 3min} verify recoverable end verify terminating end verify almost_sure end}"#;
    let v = valid_check(src);
    let arms = ids(&v, "W-UNREACH-1");
    assert_eq!(
        arms.len(),
        4,
        "outer else + reached inner else + both arms of dead inner"
    );
    assert!(
        ids(&v, "W-UNREACH-2").is_empty(),
        "same label reached once is sufficient"
    );
    for id in ["recoverable end", "terminating end", "almost_sure end"] {
        assert_eq!(p(&v, id)["verdict"], "satisfied");
    }
    reference_hand(
        src,
        false,
        &[
            ("satisfied", None),
            ("satisfied", None),
            ("satisfied", Some(("1", "1"))),
        ],
    );
    let grouped = src.replace("if a==yes {if", "if (a==yes && a==yes) {if");
    let v = check(&grouped);
    assert_eq!(
        ids(&v, "W-UNREACH-1").len(),
        4,
        "boolean operands are not additional arms"
    );
}

#[test]
fn rejection_kind_is_not_a_normal_end_named_rejected() {
    let src = r#"model M version "0.2" {type T=enum {ok} actor A {capacity 1;capability run()->T {duration fixed 1min;result nondet {ok}}}
 process Flow {a=A.run();end rejected} arrivals fixed [0min,3min] policy fifo tie_by case_id queue capacity 0 on_overflow reject
 bounds {cases 2;live_tasks 1;clock 5min} verify recoverable end verify terminating end verify almost_sure end verify probability reached end.rejected within 1min >= 1}"#;
    for (capacity, verdict, prob) in [(0, "violated", "0"), (1, "satisfied", "1")] {
        let src = src.replace("queue capacity 0", &format!("queue capacity {capacity}"));
        let v = valid_check(&src);
        for id in [
            "recoverable end",
            "terminating end",
            "almost_sure end",
            "probability reached #1",
        ] {
            let pp = p(&v, id);
            assert_eq!(pp["verdict"], verdict);
            for c in pp["cases"].as_array().unwrap() {
                assert_eq!(c["verdict"], verdict);
                if id == "almost_sure end" || id.starts_with("probability") {
                    assert_eq!(c["probability"], json!({"min":prob,"max":prob}));
                }
            }
        }
        reference_hand(
            &src,
            false,
            &[
                (verdict, None),
                (verdict, None),
                (verdict, Some((prob, prob))),
                (verdict, Some((prob, prob))),
            ],
        );
    }
}

#[test]
fn queued_cancellation_is_not_start_but_running_cancellation_is_start() {
    let base = r#"model M version "0.2" {type T=enum {ok}
 actor Quick {capacity 1;capability run()->T {duration fixed 1min;result nondet {ok};cancel forbidden_after_start}}
 actor Slow {capacity 0;capability run()->T {duration fixed 5min;result nondet {ok};cancel cooperative ack_within 1min}}
 process Flow {fork f {a=Quick.run();b=Slow.run()};join any f where result==ok tie by [a,b] remaining cancel_then_drain if none -> no_match;end done;end no_match}
 bounds {cases 1;live_tasks 2;clock 10min} verify recoverable end verify terminating end verify almost_sure end}"#;
    for cap in [0, 1] {
        let s = base.replace(
            "actor Slow {capacity 0",
            &format!("actor Slow {{capacity {cap}"),
        );
        let v = valid_check(&s);
        assert_eq!(
            p(&v, "almost_sure end")["probability"],
            json!({"min":"1","max":"1"})
        );
        let cs = ids(&v, "W-UNREACH-0");
        assert_eq!(cs.len(), if cap == 0 { 1 } else { 0 });
        if cap == 0 {
            assert!(cs[0]["message"].as_str().unwrap().contains("never_started"));
        }
        let es = ids(&v, "W-UNREACH-2");
        assert_eq!(es.len(), 1);
        assert!(es[0]["message"].as_str().unwrap().contains("no_match"));
        let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let span = m.process.body[0].span;
        assert_eq!(
            es[0]["span"],
            json!({"start":span.start,"end":span.end}),
            "if_none uses containing fork/join provenance"
        );
        reference_hand(
            &s,
            false,
            &[
                ("satisfied", None),
                ("satisfied", None),
                ("satisfied", Some(("1", "1"))),
            ],
        );
    }
}

#[test]
fn cut_exclusion_unselected_process_shared_person_and_utf8_span_are_scope_bound() {
    let src = r#"model M version "0.2" {type T=enum {ok}
 cut C {question "日本語";include [A,B];exclude Outside reason "external" loses "execution"}
 people [p,q,r]
 actor A {executors [p];capability run()->T {duration fixed 1min;result nondet {ok}}}
 actor B {executors [p,q];capability unused()->T {duration fixed 1min;result nondet {ok}}}
 actor Outside {executors [r];capability external()->T {duration fixed 1min;result nondet {ok}}}
 process Flow {a=A.run();end done}
 process Other {b=B.unused();end other}
 configuration Selected {cut C;process Flow}
 bounds {cases 1;live_tasks 1;clock 3min} verify recoverable end verify terminating end verify almost_sure end}"#;
    preflight(src);
    let v = serde_json::to_value(report::check_configuration(src, "Selected")).unwrap();
    let caps = ids(&v, "W-UNREACH-0");
    assert_eq!(caps.len(), 1);
    let msg = caps[0]["message"].as_str().unwrap();
    assert!(msg.contains("B.unused") && msg.contains("not_called") && msg.contains("Flow"));
    assert!(
        ids(&v, "W-UNREACH-2").is_empty(),
        "Other.other is outside selected process inventory"
    );
    let people = ids(&v, "W-UNREACH-3");
    assert_eq!(
        people.len(),
        1,
        "shared p starts through A, outside-only r excluded"
    );
    let from = src.find("p,q,r").unwrap() + 2;
    assert_eq!(
        people[0]["span"],
        json!({"start":from,"end":from+1}),
        "UTF8 byte identifier span, not actor/model span"
    );
    reference_hand(
        src,
        false,
        &[
            ("satisfied", None),
            ("satisfied", None),
            ("satisfied", Some(("1", "1"))),
        ],
    );
}

#[test]
fn state_budget_zero_one_and_unknown_frontier_are_never_fabricated_deadlocks() {
    let end = r#"model M version "0.2" { process Flow {end done} bounds {cases 1;live_tasks 1;clock 1min} verify recoverable end verify terminating end verify almost_sure end}"#;
    preflight(end);
    let m = lower::lower(&syntax::parse(end).unwrap()).unwrap();
    for (budget, verdict, truncated) in [
        (0, Verdict::Inconclusive, true),
        (1, Verdict::Satisfied, false),
    ] {
        let o = engine::explore_with_budget(&m, budget);
        assert_eq!(o.truncated, truncated);
        assert_eq!(o.properties.len(), 3);
        for p in &o.properties {
            assert_eq!(p.verdict, verdict);
        }
        assert!(!o.diagnostics.iter().any(|d| d.code == "W-DEADLOCK-0"));
    }
}

#[test]
fn end_conditions_do_not_leak_into_policy_and_normalization_is_closed() {
    let base = r#"model M version "0.2" { type T=enum {ok} actor A {capacity 1;capability run()->T {duration fixed 1min;result nondet {ok}}} process Flow {a=A.run();end done} bounds {cases 1;live_tasks 1;clock 2min} verify probability reached ((end.done)) within 1min >= 1 }"#;
    preflight(base);
    assert_eq!(check(base)["verdict"], "satisfied");
    for target in [
        "end.done && end.done",
        "end.done || end.done",
        "end.done == true",
    ] {
        let v = check(&base.replace("((end.done))", target));
        assert_eq!(v["verdict"], "unsupported");
        assert!(!ids(&v, "U-SYN-0").is_empty());
    }
    let unknown = check(&base.replace("end.done))", "end.missing))"));
    assert_eq!(unknown["verdict"], "invalid");
    assert!(!ids(&unknown, "E-NAME-0").is_empty());
    let policy = check(&base.replace(
        "a=A.run();end done",
        "a=A.run();if end.done {end done} else {end other}",
    ));
    assert_eq!(policy["verdict"], "unsupported");
    for property in ["recoverable", "terminating", "almost_sure"] {
        let src = base.replace(
            "verify probability reached ((end.done)) within 1min >= 1",
            &format!("verify {property} end verify {property} end"),
        );
        let v = check(&src);
        assert_eq!(v["verdict"], "unsupported");
        assert!(!ids(&v, "U-SEM-0").is_empty());
    }
}

#[test]
fn any_adoption_and_finish_are_not_end_hits_until_drain_completes() {
    // Existing fixed-time input: short finishes/adopted at 3, uncancellable
    // long drains until 5, then end.done. Work=3+5=8, result_at=3.
    let old = include_str!("../../../fixtures/any-drain/model.relca");
    let src=old.replace("verify reachable end.done", "verify reachable end.done\n verify almost_sure end\n verify probability reached end.done within 3min >= 1\n verify probability reached end.done within 5min >= 1");
    let v = check(&src);
    assert_eq!(
        p(&v, "probability reached #1")["probability"],
        json!({"min":"0","max":"0"})
    );
    assert_eq!(
        p(&v, "probability reached #2")["probability"],
        json!({"min":"1","max":"1"})
    );
    assert_eq!(p(&v, "almost_sure end")["verdict"], "satisfied");
    assert_eq!(v["terminals"][0]["elapsed"], json!({"min":5,"max":5}));
    assert_eq!(v["terminals"][0]["work"], json!({"min":8,"max":8}));
    assert_eq!(v["terminals"][0]["result_at"], json!({"min":3,"max":3}));
    let m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
    let r = relca_core::reference::explore(&m);
    assert_eq!(r.stop, relca_core::reference::StopReason::Complete);
    for (i, hand) in [(1, "1"), (2, "0"), (3, "1")] {
        let q = r.outcome.properties[i].probability.as_ref().unwrap();
        assert_eq!(q.min.to_string(), hand);
        assert_eq!(q.max.to_string(), hand);
    }
}

#[test]
fn end_free_deadlock_is_a_valid_completion_question_not_a_fake_terminal() {
    let src = r#"model M version "0.2" {process Flow {} bounds {cases 1;live_tasks 1;clock 1min} verify recoverable end verify terminating end verify almost_sure end}"#;
    let v = valid_check(src);
    assert_eq!(v["verdict"], "violated");
    assert_eq!(v["terminals"], json!([]));
    for id in ["recoverable end", "terminating end", "almost_sure end"] {
        assert_eq!(p(&v, id)["verdict"], "violated");
    }
    assert_eq!(
        p(&v, "almost_sure end")["probability"],
        json!({"min":"0","max":"0"})
    );
}

#[test]
fn four_unreachable_codes_are_registered_warnings_and_never_deadlock_errors() {
    for code in ["W-UNREACH-0", "W-UNREACH-1", "W-UNREACH-2", "W-UNREACH-3"] {
        assert!(relca_core::diag::CODES.contains(&code));
        assert_eq!(
            relca_core::diag::severity_of(code),
            Some(relca_core::diag::Severity::Warning)
        );
    }
}

#[test]
fn real_runtime_abort_marks_unreachable_unchecked_but_static_failure_does_not() {
    // First one-minute operation precedes this maximal duration; its end time
    // overflows u64. This is the established public arithmetic-abort route,
    // not an invented graph-limit error or a malformed new property.
    let src = include_str!("../../../fixtures/agency-origin/model.relca")
        .replace(
            "commits; duration fixed 1min",
            "commits; duration fixed 18446744073709551615min",
        )
        .replace("clock 32min", "clock 18446744073709551615min");
    lower::lower(&syntax::parse(&src).unwrap()).unwrap();
    let v = check(&src);
    assert!(!ids(&v, "U-SEM-0").is_empty());
    assert!(v["bounds"]["cases"].is_u64());
    assert!(v["bounds"]["truncated"].is_null());
    assert_eq!(v["unchecked"], json!(["unreachable"]));
    for i in 0..4 {
        assert!(ids(&v, &format!("W-UNREACH-{i}")).is_empty());
    }
    assert!(check("not a model").get("unchecked").is_none());
}
