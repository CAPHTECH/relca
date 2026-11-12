//! ADR0039 acceptance: precomputed oracles, independent interpreters, RED checkpoint.
#[path = "support/limits_acceptance.rs"]
mod h;
use relca_core::{engine, explain, reference};
use serde_json::{Value, json};

#[test]
fn closed_population_exact_edits_and_no_language_comments() {
    let mut actual: Vec<_> = std::fs::read_dir(h::root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("lim-"))
        .collect();
    actual.sort();
    assert_eq!(actual, h::names());
    for p in h::pairs() {
        let a = h::source(p["good"].as_str().unwrap());
        let b = h::source(p["bad"].as_str().unwrap());
        assert!(!a.contains("//") && !a.contains('#') && !b.contains("//") && !b.contains('#'));
        if p["label"] != true {
            assert_eq!(
                h::edit(&a, p["old"].as_str().unwrap(), p["new"].as_str().unwrap()),
                b
            );
        }
    }
}
#[test]
fn old_grammar_preflight_rules_out_unrelated_input_defects() {
    for p in h::pairs() {
        for n in [p["good"].as_str().unwrap(), p["bad"].as_str().unwrap()] {
            let good = if h::expected(n)["verdict"] == "invalid"
                || h::expected(n)["verdict"] == "unsupported"
            {
                p["good"].as_str().unwrap()
            } else {
                n
            };
            let skeleton = h::skeleton(&h::source(good));
            h::selected(&skeleton);
        }
    }
    // Keep ordinary contracts intact in a second, narrower projection. A bare
    // skeleton must not hide an unrelated malformed excludes/contest clause.
    let mut ordinary = 0;
    for n in h::names() {
        let expected = h::expected(&n);
        let s = h::source(&n);
        if expected["verdict"] == "invalid"
            || expected["verdict"] == "unsupported"
            || ["case.admitted", "end.overflowed", "end.spill"]
                .iter()
                .any(|t| s.contains(t))
        {
            continue;
        }
        let s = s
            .lines()
            .filter(|l| {
                !l.trim_start().starts_with("queue capacity")
                    && !l.trim_start().starts_with("concurrent cases")
            })
            .collect::<Vec<_>>()
            .join("\n");
        h::selected(&s);
        ordinary += 1;
    }
    assert!(
        ordinary > 10,
        "ordinary event and contest scaffolding controls must run"
    );
}

#[test]
fn population_75_exhaustive50_static23_sample2_reference_complete50_no_exclusions() {
    let (mut runtime, mut refused, mut samples, mut complete) = (0, 0, 0, 0);
    for n in h::names() {
        let s = h::source(&n);
        let expected = h::expected(&n);
        if expected["mode"] == "simulation" {
            samples += 1;
            continue;
        }
        if expected["verdict"] == "invalid" || expected["verdict"] == "unsupported" {
            h::subset(&expected, &h::checked(&s));
            refused += 1;
            continue;
        }
        runtime += 1;
        h::both(&s, &expected);
        complete += 1;
    }
    assert_eq!((runtime, refused, samples, complete), (50, 23, 2, 50));
}

#[test]
fn mass_keeps_absolute_completion_relative_elapsed_and_admission_ranges() {
    let v = h::checked(&h::source("lim-mass"));
    assert_eq!(v["verdict"], "satisfied");
    let c = &v["case_outcomes"][2];
    assert_eq!(c["admitted_at"], json!({"min":2,"max":3}));
    for (key, lo, hi) in [("completed_at", 3, 9), ("elapsed", 1, 7), ("work", 1, 3)] {
        assert_eq!(c["terminals"][0][key], json!({"min":lo,"max":hi}));
    }
    assert_eq!(
        v["admission"]["external_wait_max"],
        json!({"min":0,"max":1})
    );
}
macro_rules! accepts {($($test:ident => $name:literal),+ $(,)?)=>{$(#[test] fn $test(){h::accept($name);})+};}
accepts! {
    p01_queue_wait => "lim-queue-wait", p02_readmit_same_tick => "lim-readmit",
    p03_overflow_receipt => "lim-overflow", p04_concurrent_wait => "lim-concurrent-wait",
    p05_running_cases_count => "lim-count", p06_within_arrival_origin => "lim-within",
    p07_clock_not_rejection => "lim-clock", p08_probability_mass => "lim-mass",
    p09_label_independence => "lim-label", p10_legacy_route => "lim-legacy-route",
    p11_limits_agree => "lim-both", p12_undeclared_target => "lim-counts",
    p12b_separate_counts => "lim-counts-bare", p12c_rejected_vacuity => "lim-counts-obligations",
    p13_simulation_does_not_randomize => "lim-sim", p15_admitted_origin => "lim-admitted-deadline",
    p16_zero_deadline => "lim-zero-deadline-overflow", p19_deadlock_held => "lim-deadlock-held",
    p20_arrival_before_finish => "lim-same-time-boundary",
    p22_relative_times => "lim-delayed-arrival", admitted_reject_context => "lim-admitted-reject",
    admitted_after_matches => "lim-admitted-after", admitted_not_invariant => "lim-admitted-invariant",
    admitted_not_excludes => "lim-admitted-excludes", zero_limit_legal => "lim-zero-limit",
    overflow_not_admitted => "lim-overflow-admitted", instant_readmission_clock_zero => "lim-instant",
    standing_overflow_refusal => "lim-standing", contest_overflow_refusal => "lim-contest-overflow"
}
#[test]
fn p21_end_identity() {
    h::accept("lim-both-end-id");
    let m = h::selected(&h::source("lim-both-end-id"));
    assert_eq!(
        m.ends,
        vec!["done".to_string(), "spill".to_string()],
        "both limits share exactly one overflow EndId after process ends"
    );
}
#[test]
fn p14_every_static_classification_and_positive_control() {
    for n in h::names()
        .into_iter()
        .filter(|n| n.starts_with("lim-static"))
    {
        let v = h::checked(&h::source(&n));
        h::subset(&h::expected(&n), &v);
        if n != "lim-static" {
            assert_eq!(v["bounds"]["truncated"], Value::Null);
            assert!(v.get("case_outcomes").is_none());
            assert!(v.get("admission").is_none());
        }
    }
}
#[test]
fn clock_check_withdraws_every_scalar_but_simulation_preserves_observed_counts() {
    let s = h::source("lim-clock.broken-short-clock");
    let v = h::checked(&s);
    assert_eq!(v["verdict"], "inconclusive");
    fn scalars(v: &Value, seen: &mut usize) {
        match v {
            Value::Object(o) => {
                if o.contains_key("min") && o.contains_key("reason") {
                    *seen += 1;
                    assert_eq!(v["min"], Value::Null);
                    assert_eq!(v["max"], Value::Null);
                    assert_eq!(v["reason"], "truncated");
                } else {
                    for x in o.values() {
                        scalars(x, seen);
                    }
                }
            }
            Value::Array(a) => {
                for x in a {
                    scalars(x, seen)
                }
            }
            _ => {}
        }
    }
    let mut seen = 0;
    scalars(&v["metrics"], &mut seen);
    assert!(seen > 30, "all Scalar inventory must be inspected");
    let s = h::edit(
        &s,
        "  metrics {",
        "  simulation replications 10 seed 1 confidence 0.95\n  metrics {",
    );
    let sample = h::sampled(&s);
    assert_eq!(sample["verdict"], "inconclusive");
    assert_eq!(sample["simulation"]["truncated_replications"], 10);
    for (k, n) in [
        ("received", 3),
        ("admitted", 2),
        ("completed", 1),
        ("unfinished", 2),
        ("rejected", 0),
    ] {
        assert_eq!(
            sample["metrics"]["overall"]["counts"][k]["stats"],
            json!({"mean":n.to_string(),"min":n.to_string(),"max":n.to_string(),"samples":10,"reason":null})
        );
        assert_eq!(
            sample["metrics"]["overall"]["counts"][k]["observed_total"],
            n * 10
        );
    }
    let e = engine::simulate(&h::selected(&s));
    let r = reference::simulate(&h::selected(&s));
    assert_eq!(
        e, r.outcome,
        "same replication identities, keys, cuts and metrics"
    );
    assert_eq!(sample["metrics"]["cases"][2]["external_wait"]["samples"], 0);
    assert_eq!(
        sample["metrics"]["cases"][2]["external_wait"]["reason"],
        "unfinished"
    );
}
#[test]
fn readmission_has_no_random_draw_site_and_init_stays_once_at_arrival() {
    let s = h::source("lim-sim");
    let m = h::selected(&s);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    assert_eq!(e, r.outcome);
    assert_eq!(e.completed_replications, 10);
    assert!(e.replications.iter().all(|r| r.draws.is_empty()));
    let latent = h::edit(
        &s,
        "  observed case",
        "  latent case { flag: bool ~ { true: 0.5, false: 0.5 } }\n  observed case",
    );
    let m = h::selected(&latent);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    assert_eq!(e, r.outcome);
    for rep in e.replications {
        assert_eq!(rep.draws.len(), 3);
        for (i, d) in rep.draws.iter().enumerate() {
            assert!(d.key.contains(&format!("case={}", i + 1)), "{}", d.key);
            assert!(d.key.contains("n=1"), "{}", d.key);
            assert!(!d.key.contains("admission"));
        }
    }
}
#[test]
fn explain_receipt_and_admission_are_distinct_verified_events() {
    let s = h::source("lim-admitted-deadline.broken-receipt");
    assert!(!s.contains("configuration"));
    let v = serde_json::to_value(explain::explain(&s)).unwrap();
    assert_eq!(v["verdict"], "violated");
    assert_eq!(v["explanation"]["status"], "complete");
    let cs = v["explanation"]["counterexamples"].as_array().unwrap();
    assert!(!cs.is_empty());
    let mut receipt = false;
    let mut admission = false;
    for c in cs {
        assert_eq!(c["replay"]["status"], "verified");
        for stage in c["steps"].as_array().unwrap() {
            for event in stage["events"].as_array().unwrap() {
                if event["kind"] == "received" {
                    receipt = true;
                }
                if event["kind"] == "admitted" {
                    admission = true;
                }
            }
        }
    }
    assert!(
        receipt && admission,
        "trace cannot collapse receipt into admission: {v}"
    );
}
#[test]
fn overflow_reachable_name_rejected_is_not_rejection_and_f_excludes_it() {
    let s = h::source("lim-overflow");
    let s = h::edit(&s, "overflowed", "rejected");
    let s = h::edit(
        &s,
        "  verify safety",
        "  verify reachable end.rejected\n  verify safety",
    );
    let v = h::checked(&s);
    assert_eq!(v["verdict"], "violated");
    let reach = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "reachable end.rejected")
        .unwrap();
    assert_eq!(
        reach["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["verdict"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["violated", "satisfied", "satisfied"]
    );
    assert_eq!(v["admission"]["overflow_count"], json!({"min":2,"max":2}));
    for c in &v["case_outcomes"].as_array().unwrap()[1..] {
        assert_eq!(c["terminals"][0]["kind"], "overflow");
        assert_eq!(c["terminals"][0]["end"], "rejected");
        assert!(c.get("admitted_at").is_none());
    }
    let f = &v["agency"]["viewpoints"]["framing"]["ends"];
    assert!(!f.is_null(), "F oracle must inspect actual framing table");
    assert!(
        !f.to_string().contains("rejected"),
        "overflow must not add a framing terminal: {f}"
    );
}

#[test]
fn compare_includes_overflow_without_hiding_regular_terminal_mismatch() {
    use relca_core::{compare, lower, report, syntax};
    let s = h::compare_source();
    for selection in ["Left", "Right"] {
        let v = serde_json::to_value(report::check_configuration(&s, selection)).unwrap();
        assert_eq!(v["verdict"], "satisfied");
        assert_eq!(v["admission"]["overflow_count"], json!({"min":2,"max":2}));
        let selected = lower::lower_configuration(&syntax::parse(&s).unwrap(), selection).unwrap();
        assert_eq!(
            selected.terminal_names,
            ["done".to_string(), "overflowed".to_string()]
                .into_iter()
                .collect()
        );
        let e = engine::explore(&selected.model);
        let mut r = reference::explore(&selected.model);
        assert_eq!(r.stop, reference::StopReason::Complete);
        r.outcome
            .diagnostics
            .drain(..selected.model.diagnostics.len());
        assert_eq!(e, r.outcome);
    }
    let v = serde_json::to_value(compare::compare(
        &s,
        &["Left", "Right"],
        compare::Analysis::Exhaustive,
    ))
    .unwrap();
    assert_eq!(v["verdict"], "satisfied");
    let overflow: Vec<_> = v["tables"]["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "overflow")
        .collect();
    assert_eq!(
        overflow.len(),
        3,
        "one declared-overflow column per case, including not_reported case1"
    );
    assert!(overflow.iter().all(|r| r["end"] == "overflowed"));
    for row in overflow {
        for cell in row["values"].as_array().unwrap() {
            if row["case_id"] == 1 {
                assert_eq!(cell["status"], "not_reported");
                assert!(cell["value"].is_null());
            } else {
                assert_eq!(cell["status"], "reported");
                assert_eq!(cell["value"]["completed_at"], json!({"min":0,"max":0}));
            }
        }
    }
    let changed = h::edit(
        &s,
        "process Other { r = A.run(); end done }",
        "process Other { r = A.run(); end other }",
    );
    let v = serde_json::to_value(compare::compare(
        &changed,
        &["Left", "Right"],
        compare::Analysis::Exhaustive,
    ))
    .unwrap();
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CMP-0")
    );
}
#[test]
fn concurrent_slot_returns_at_business_end_not_contest_closure() {
    let s = h::contest_source();
    let skeleton = h::skeleton(&s);
    h::selected(&skeleton);
    let v = h::checked(&s);
    assert!(
        v["case_outcomes"].is_array(),
        "contest lower and execution required, not root-only verdict: {v}"
    );
    assert_eq!(
        v["case_outcomes"][1]["admitted_at"],
        json!({"min":2,"max":2})
    );
    assert_eq!(
        v["case_outcomes"][1]["terminals"][0]["completed_at"],
        json!({"min":4,"max":4})
    );
    assert_eq!(
        v["metrics"]["cases"][1]["external_wait"],
        json!({"min":"2","max":"2","reason":null})
    );
    // The independent reference must retain all filing/decline branches too.
    let m = h::selected(&s);
    let e = engine::explore(&m);
    let mut r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    assert_eq!(e, r.outcome);
}
#[test]
fn overflow_task_inventory_and_cost_are_never_created_even_with_rates() {
    let s = h::source("lim-counts-bare");
    let rated = h::edit(
        &s,
        "currency \"JPY\"",
        "currency \"JPY\"; rate A per_minute 10; rate A.run per_start 3",
    );
    for (s, total) in [(&s, None), (&rated, Some("23"))] {
        let v = h::checked(s);
        assert_eq!(v["verdict"], "satisfied");
        for c in &v["metrics"]["cases"].as_array().unwrap()[1..] {
            assert_eq!(
                c["observed_cost_total"],
                json!({"min":null,"max":null,"reason":"never_created"})
            );
            let terminal = c["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["kind"] == "overflow")
                .unwrap();
            assert_eq!(
                terminal["cost_per_case"],
                json!({"min":null,"max":null,"reason":"never_created"})
            );
            assert_eq!(
                terminal["lead_time"],
                json!({"min":"0","max":"0","reason":null})
            );
            assert_eq!(c["tasks"][0]["never_created_count"]["stats"]["min"], "1");
        }
        match total {
            Some(n) => assert_eq!(v["metrics"]["overall"]["observed_cost_total"]["min"], n),
            None => assert_eq!(
                v["metrics"]["overall"]["observed_cost_total"]["reason"],
                "not_configured"
            ),
        }
    }
    let simulation = h::edit(
        &s,
        "  bounds {",
        "  simulation replications 10 seed 1 confidence 0.95\n  bounds {",
    );
    let m = h::selected(&simulation);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    assert_eq!(e, r.outcome);
    let v = h::sampled(&simulation);
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["simulation"]["completed_replications"], 10);
    for (key, n) in [
        ("received", 3),
        ("admitted", 1),
        ("completed", 3),
        ("overflowed", 2),
        ("rejected", 0),
    ] {
        assert_eq!(
            v["metrics"]["overall"]["counts"][key]["stats"]["mean"],
            n.to_string()
        );
        assert_eq!(
            v["metrics"]["overall"]["counts"][key]["observed_total"],
            n * 10
        );
    }
    for c in &v["case_outcomes"].as_array().unwrap()[1..] {
        assert_eq!(
            c["terminals"][0],
            json!({"kind":"overflow","end":"overflowed","count":10})
        );
    }
}

#[test]
fn public_ir_capacity_zero_and_missing_end_fail_closed_independently() {
    use relca_core::reference::StopReason;
    let mut wait = h::selected(&h::source("lim-readmit"));
    wait.admission_queue.as_mut().unwrap().capacity = 0;
    let mut overflow = h::selected(&h::source("lim-overflow"));
    overflow.ends.clear();
    for m in [wait, overflow] {
        let e = std::panic::catch_unwind(|| engine::explore(&m))
            .expect("public malformed IR must not panic");
        assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"), "{e:?}");
        let r = std::panic::catch_unwind(|| reference::explore(&m))
            .expect("reference must defend independently");
        assert_eq!(r.stop, StopReason::SemanticAbort);
    }
}
#[test]
fn contextual_words_and_three_component_case_actor_event_keep_old_meaning() {
    let s = r#"model M version "0.2" {
 type wait_external=enum {fifo,concurrent}
 actor case {capacity 1;capability admitted()->wait_external {duration fixed 1min;result nondet{fifo}}}
 process P {r=case.admitted();end done}
 bounds {cases 1;live_tasks 1;clock 2min}
 contract C {invariant case.admitted.finish -> r == fifo}
 verify safety no_resource_overcommit
}"#;
    let m = h::selected(s);
    assert!(m.legacy_route());
    let v = h::checked(s);
    assert_eq!(v["verdict"], "satisfied");
    assert!(v.get("case_outcomes").is_none());
    assert!(v.get("admission").is_none());
    h::both(s, &json!({"bounds":{"truncated":false}}));
}
#[test]
fn zero_concurrent_reject_or_end_is_legal_but_zero_wait_is_invalid() {
    let s = h::source("lim-zero-limit");
    let s = h::edit(&s, "queue capacity 0", "concurrent cases 0");
    let v = h::checked(&s);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(v["admission"]["rejected_count"], json!({"min":3,"max":3}));
    let end = h::edit(&s, "on_overflow reject", "on_overflow end spill");
    let v = h::checked(&end);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(v["admission"]["overflow_count"], json!({"min":3,"max":3}));
    let wait = h::edit(&s, "on_overflow reject", "on_overflow wait_external fifo");
    let v = h::checked(&wait);
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CON-0")
    );
    let oversized = h::edit(
        &h::source("lim-count"),
        "concurrent cases 2",
        "concurrent cases 10",
    );
    let v = h::checked(&oversized);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(v["admission"]["rejected_count"], json!({"min":0,"max":0}));
}
#[test]
fn assumptions_keep_limit_order_arrival_time_and_model_rule_boundary_explicit() {
    let s = h::source("lim-concurrent-wait");
    let v = h::checked(&s);
    assert_eq!(
        h::row(&v, "concurrent.cases")["value"],
        "1 on_overflow wait_external fifo"
    );
    let text = h::row(&v, "concurrent.cases")["affects"].as_str().unwrap();
    for sentence in [
        "Admission limits and overflow handling are model rules, not an Actor.",
        "Counts are tested before same-time starts and finishes.",
        "Concurrent cases stop counting at the business end, not contest closure.",
        "Receipt and initial values occur at arrival; admission occurs when both limits have room.",
        "External FIFO may reuse slots released at the same time after immediate events settle; new arrivals do not.",
        "External wait is included in elapsed, within and receipt-based response deadlines.",
        "first_start_wait remains arrival-based and overlaps external_wait; do not add them.",
    ] {
        assert!(
            text.contains(sentence),
            "missing frozen semantic assumption {sentence}: {text}"
        );
    }
    let initial = h::edit(
        &s,
        "  observed case",
        "  latent case { flag:bool ~ {true:0.5,false:0.5} }\n  observed case",
    );
    let v = h::checked(&initial);
    assert_eq!(
        h::row(&v, "initial.case_independence")["value"],
        "independent-per-arrival"
    );
    let v = h::checked(&h::source("lim-both"));
    let fields: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["field"].as_str().unwrap())
        .collect();
    let i = fields.iter().position(|f| *f == "queue.capacity").unwrap();
    assert_eq!(fields[i + 1], "concurrent.cases");
    assert_eq!(
        h::row(&v, "queue.capacity")["affects"],
        "admitted nonterminal cases with no task ever started are counted before same-time starts; rejected cases retain their probability mass"
    );
}

#[test]
fn p09_full_report_projection_changes_only_labels_grain_source_hash_and_spans() {
    fn project(mut v: Value) -> Value {
        // Explicit metadata positions. No numerical or verdict field is erased.
        v["model"]["sha256"] = json!("source-hash");
        v["actors"][0]["grain"] = json!({"value":null,"status":"default"});
        fn walk(v: &mut Value) {
            match v {
                Value::Object(o) => {
                    for (k, x) in o {
                        if k == "span" {
                            *x = Value::Null;
                        } else {
                            walk(x);
                        }
                    }
                }
                Value::Array(a) => {
                    for x in a {
                        walk(x);
                    }
                }
                Value::String(s) => {
                    // Closed spellings from the input transformation, not values
                    // discovered by comparing changed implementation output.
                    *s = match s.as_str() {
                        "Desk" => "A".into(),
                        "spill" => "overflowed".into(),
                        _ => s
                            .replace("Desk.", "A.")
                            .replace("end.spill", "end.overflowed")
                            .replace("on_overflow end spill", "on_overflow end overflowed"),
                    };
                }
                _ => {}
            }
        }
        walk(&mut v);
        v
    }
    let a = h::checked(&h::source("lim-label"));
    let b = h::checked(&h::source("lim-label.broken-relabel"));
    assert_eq!(a["verdict"], "satisfied");
    assert_eq!(b["verdict"], "satisfied");
    assert_eq!(
        b["actors"][0]["grain"],
        json!({"value":"team","status":"declared"})
    );
    let a = project(a);
    let b = project(b);
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap(),
        "all remaining bytes, including numerical, property and Agency payloads"
    );
}

#[test]
fn admitted_response_may_precede_limit_and_after_must_match_trigger() {
    let s = h::source("lim-admitted-deadline");
    let clause = "  contract C { response case.admitted -> one_of(end.done) by 2min after case.admitted otherwise unresolved }";
    let before = h::edit(
        &h::edit(&s, clause, ""),
        "  concurrent cases",
        &format!("{clause}\n  concurrent cases"),
    );
    let a = h::checked(&s);
    let b = h::checked(&before);
    assert_eq!(a["verdict"], "satisfied");
    assert_eq!(b["verdict"], "satisfied");
    let by_id = |v: &serde_json::Value| {
        let mut p = v["properties"].as_array().unwrap().clone();
        p.sort_by_key(|x| x["id"].as_str().unwrap().to_owned());
        p
    };
    assert_eq!(
        by_id(&a),
        by_id(&b),
        "declaration order is not admission semantics"
    );
    for key in ["case_outcomes", "admission"] {
        assert_eq!(
            a[key], b[key],
            "declaration order is not admission semantics"
        );
    }
}
#[test]
fn static_limit_messages_keep_the_specific_reason_not_generic_refusal() {
    for (n, code, message) in [
        (
            "lim-static.broken-missing-fifo",
            "U-SYN-0",
            "external wait requires fifo order",
        ),
        (
            "lim-static.broken-zero-queue",
            "E-CON-0",
            "external wait requires a positive limit",
        ),
        (
            "lim-static.broken-zero-concurrent",
            "E-CON-0",
            "external wait requires a positive limit",
        ),
        (
            "lim-static.broken-end-conflict",
            "E-CON-0",
            "overflow terminal conflicts with a process terminal",
        ),
        (
            "lim-both.broken-disagree",
            "U-SEM-0",
            "queue and concurrent limits disagree on overflow",
        ),
        (
            "lim-standing.broken-overflow",
            "U-SEM-0",
            "Standing on an overflow terminal is unsupported",
        ),
    ] {
        let v = h::checked(&h::source(n));
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code && d["message"] == message),
            "{n}: {v}"
        );
    }
}

// Boundary review m1: a range stop leaves only unfinished cases unknown. The
// overflowed cases ended at 0 without Task and keep their per-case rows.
// Hand order (lim-overflow calc): at 0 case1 is admitted, cases2,3 overflow and
// end overflowed at 0; case1 runs A on [0,2) and its finish hit += 1 tries 1 > 0
// at 2. Case1 is unknown (deadline 4 not reached, no finish); cases2,3 are not.
#[test]
fn range_stop_keeps_finished_overflow_cases_out_of_the_unknown_rows() {
    let s = h::edit(
        &h::source("lim-overflow"),
        "observed case { hit: bool = false }",
        "observed case { hit: 0..0 = 0 }",
    );
    let s = h::edit(&s, "finish { hit = true }", "finish { hit += 1 }");
    let overflowed = json!({"kind":"overflow","end":"overflowed","completed_at":{"min":0,"max":0},
        "elapsed":{"min":0,"max":0},"work":{"min":0,"max":0},"result_at":null});
    let rows = |first: &str| {
        json!([{"case_id":1,"verdict":first},{"case_id":2,"verdict":"satisfied"},
            {"case_id":3,"verdict":"satisfied"}])
    };
    let hand = json!({
        "bounds": {"truncated": false},
        "properties": [
            {"verdict": "inconclusive"},
            {"verdict": "inconclusive", "cases": rows("inconclusive")},
            {"verdict": "violated", "cases": rows("violated")}
        ],
        "case_outcomes": [
            {"case_id":1,"arrival":0,"terminals":[]},
            {"case_id":2,"arrival":0,"terminals":[overflowed]},
            {"case_id":3,"arrival":0,"terminals":[overflowed]}
        ]
    });
    h::both(&s, &hand);
    h::subset(
        &json!({
            "verdict": "violated",
            "bounds": {"truncated": false, "range_violation_stops": true},
            "properties": [
                {"id": "safety no_resource_overcommit", "verdict": "inconclusive"},
                {"id": "contract C #1 response", "verdict": "inconclusive", "cases": rows("inconclusive")},
                {"id": "state_in_range", "verdict": "violated", "cases": rows("violated")}
            ],
            "admission": {"overflow_count": {"min": 2, "max": 2}}
        }),
        &h::checked(&s),
    );
}

fn unresolved_received(o: &relca_core::outcome::Outcome, reason: &str, deadline: u64) -> Vec<u64> {
    use relca_core::obligation::Status;
    let mut ids = Vec::new();
    for row in o.obligations.as_ref().expect("obligation table") {
        if row.trigger_site != "received" {
            continue;
        }
        assert_eq!(row.outcomes.len(), 1, "{row:?}");
        let r = &row.outcomes[0];
        assert_eq!(r.status, Status::Unresolved, "{row:?}");
        assert_eq!(r.reason.as_deref(), Some(reason), "{row:?}");
        assert_eq!(
            (r.triggered_at, r.deadline, r.resolved_at),
            (0, deadline, None)
        );
        ids.push(row.case_id);
    }
    ids
}

// Boundary review m2 (range-stop prefix): Held cases are received at arrival and
// their receipt obligations stay open at the stop, as for admitted cases.
// Hand order (lim-concurrent-wait calc): at 0 case1 is admitted, cases2,3 are Held;
// case1 finish at 2 tries hit 1 > 0. No case ended; deadline 6 not reached.
#[test]
fn range_stop_collects_receipt_obligations_of_held_cases() {
    let s = h::edit(
        &h::source("lim-concurrent-wait"),
        "observed case { hit: bool = false }",
        "observed case { hit: 0..0 = 0 }",
    );
    let s = h::edit(&s, "finish { hit = true }", "finish { hit += 1 }");
    let s = h::edit(&s, "  metrics { window 20min; currency \"JPY\" }\n", "");
    // A named response publishes the obligation ledger (ADR 0022).
    let s = h::edit(
        &s,
        "response case.received",
        "response delivery: case.received",
    );
    let rows = json!([{"case_id":1,"verdict":"violated"},{"case_id":2,"verdict":"inconclusive"},
        {"case_id":3,"verdict":"inconclusive"}]);
    let unknown = json!([{"case_id":1,"verdict":"inconclusive"},{"case_id":2,"verdict":"inconclusive"},
        {"case_id":3,"verdict":"inconclusive"}]);
    let hand = json!({
        "bounds": {"truncated": false},
        "properties": [
            {"verdict": "inconclusive"},
            {"verdict": "inconclusive", "cases": unknown},
            {"verdict": "violated", "cases": rows}
        ],
        "case_outcomes": [
            {"case_id":1,"arrival":0,"terminals":[]},
            {"case_id":2,"arrival":0,"terminals":[]},
            {"case_id":3,"arrival":0,"terminals":[]}
        ]
    });
    h::both(&s, &hand);
    let m = h::selected(&s);
    assert_eq!(
        unresolved_received(&engine::explore(&m), "state_range_violation", 6),
        [1, 2, 3]
    );
    assert_eq!(
        unresolved_received(&reference::explore(&m).outcome, "state_range_violation", 6),
        [1, 2, 3]
    );
}

// Boundary review m2 (abort): the abort at case1's finish leaves the receipt
// obligations of the Held cases open, with reason aborted, as for admitted cases.
// Hand order: at 0 case1 is admitted, cases2,3 are Held; at 2 the finish trigger
// adds 2 + u64::MAX minutes, which overflows, so the run aborts (unsupported).
#[test]
fn abort_collects_receipt_obligations_of_held_cases() {
    let s = h::edit(
        &h::source("lim-concurrent-wait"),
        "otherwise unresolved }",
        "otherwise unresolved; response A.run.finish -> one_of(end.done) by 18446744073709551615min after A.run.finish otherwise unresolved }",
    );
    let s = h::edit(&s, "  metrics { window 20min; currency \"JPY\" }\n", "");
    let s = h::edit(
        &s,
        "response case.received",
        "response delivery: case.received",
    );
    let m = h::selected(&s);
    let e = engine::explore(&m);
    assert!(
        e.properties
            .iter()
            .all(|p| p.verdict == relca_core::Verdict::Unsupported),
        "{e:?}"
    );
    assert_eq!(unresolved_received(&e, "aborted", 6), [1, 2, 3]);
}

// Boundary review m3: the context refusal of case.admitted points at the word.
#[test]
fn admitted_context_refusal_has_the_trigger_span() {
    let s = h::source("lim-admitted-reject.broken-context");
    let d = relca_core::syntax::parse(&s)
        .and_then(|a| relca_core::lower::lower(&a))
        .unwrap_err();
    let spans: Vec<_> = d
        .iter()
        .filter(|d| d.message == "case.admitted requires external wait or overflow")
        .map(|d| d.span.expect("span"))
        .collect();
    assert!(!spans.is_empty(), "{d:?}");
    let first = s.find("case.admitted").unwrap();
    assert_eq!(spans[0].start, first);
    for span in spans {
        assert_eq!(&s[span.start..span.end], "case.admitted");
    }
}

// Mutation K06c: simulate counts within from the arrival, not the admission.
// Hand order (lim-within calc, capacity 1): at 0 case1 is admitted and starts
// [0,2); case2 is admitted at 0 after that start; at 2 case2 starts [2,4) and
// case3 is admitted at 2, starting [4,6). hit at 2,4,6 from arrival 0, so within
// 5min holds for cases1,2 and not for case3 (6 > 5; from admission 4 <= 5).
// Durations and results are fixed, so all 10 replications take this path.
#[test]
fn simulation_counts_within_from_arrival_through_external_wait() {
    let s = h::edit(
        &h::source("lim-within.broken-strict"),
        "  verify probability",
        "  simulation replications 10 seed 1 confidence 0.95\n  verify probability",
    );
    let m = h::selected(&s);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    use relca_core::outcome::QueryTruth::{Hit, Miss};
    for o in [&e, &r.outcome] {
        assert_eq!(o.replications.len(), 10, "{o:?}");
        for rep in &o.replications {
            let queries: Vec<_> = rep.cases.iter().map(|c| c.queries.clone()).collect();
            assert_eq!(queries, [vec![Hit], vec![Hit], vec![Miss]], "{rep:?}");
        }
    }
    assert_eq!(e, r.outcome);
}

// Mutation K24: readmission waits until the same-time immediates (finishes, ends
// and starts of already admitted cases) are exhausted. No policy, so start order
// is open and an early readmitted case could take a freed unit.
// Hand order: A has 2 units. At 0 cases1-4 are admitted (concurrent 0..3 < 4),
// case5 is Held; two of cases1-4 start [0,2), the other two wait. At 2 both end
// done, the two waiting cases take both units [2,4); only then case5 is admitted
// (concurrent 2 < 4) and waits; it starts [4,6). Cases1-4 complete at {2,4},
// case5 at 6 (elapsed 6 from arrival 0, admitted 2). Deadline 6 holds for all.
#[test]
fn readmission_follows_same_time_starts_without_policy() {
    let s = h::edit(
        &h::source("lim-concurrent-wait"),
        "  policy fifo tie_by case_id\n",
        "",
    );
    let s = h::edit(&s, "concurrent cases 1", "concurrent cases 4");
    let s = h::edit(&s, "actor A { capacity 1;", "actor A { capacity 2;");
    let s = h::edit(&s, "[0min, 0min, 0min]", "[0min, 0min, 0min, 0min, 0min]");
    let s = h::edit(&s, "cases 3;", "cases 5;");
    let s = h::edit(&s, "  metrics { window 20min; currency \"JPY\" }\n", "");
    let done = |lo: u64, hi: u64| {
        json!([{"kind":"end","end":"done","completed_at":{"min":lo,"max":hi},
            "elapsed":{"min":lo,"max":hi},"work":{"min":2,"max":2},"result_at":null}])
    };
    let rows: Vec<_> = (1..=5)
        .map(|k| json!({"case_id":k,"verdict":"satisfied"}))
        .collect();
    let mut cases: Vec<_> = (1..=4)
        .map(|k| json!({"case_id":k,"arrival":0,"terminals":done(2, 4)}))
        .collect();
    cases.push(json!({"case_id":5,"arrival":0,"terminals":done(6, 6)}));
    let hand = json!({
        "bounds": {"truncated": false},
        "properties": [
            {"verdict": "satisfied"},
            {"verdict": "satisfied", "cases": rows}
        ],
        "case_outcomes": cases
    });
    h::both(&s, &hand);
    let v = h::checked(&s);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(
        v["case_outcomes"][4]["admitted_at"],
        json!({"min":2,"max":2})
    );
}
