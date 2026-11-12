#![allow(dead_code)]
use serde_json::{Value, json};

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
pub fn masked(s: &str) -> String {
    assert!(s.contains("breakdown waits"));
    s.replace("breakdown waits", "               ")
}
pub fn append(s: &str, text: &str) -> String {
    let pos = s.rfind('}').unwrap();
    format!("{}\n{text}\n{}", &s[..pos], &s[pos..])
}
pub fn quality(s: &str) -> String {
    append(
        s,
        "property case.Done = reached(end.done)\nmetric cohort.DoneRate = expected(count(case.Done)) / arrivals",
    )
}
pub fn hq(clock: bool, simulation: bool) -> String {
    let name = if simulation {
        if clock {
            "wait-sample.broken-short-horizon"
        } else {
            "wait-sample"
        }
    } else if clock {
        "wait-horizon-queue.broken-clock"
    } else {
        "wait-horizon-queue"
    };
    quality(&fixture(name))
}
pub fn refusal() -> String {
    quality(&fixture("wait-no-policy-sample.broken-no-policy"))
}
pub fn rule() -> String {
    r#"model WaitingRule version "0.2" {
type T = enum { ok }
people [p, cfo]
actor Rule { version "r1"; decides pause: bool = true }
actor A { executors [p]
 capability prep() -> T { duration fixed 3min; result nondet { ok } }
 capability finish() -> T { duration fixed 5min; result nondet { ok } }
}
authority Change { basis "prior rule"; grants cfo to change Rule; issued_by cfo }
process P { a = A.prep(); if Rule.pause == true { wait 2min } else { wait 0min }; b = A.finish(); end done }
cut C { include [Rule, A] }
configuration Before { cut C; process P }
configuration After { cut C; process P; amend Rule from "r1" to "r2" by cfo under Change { pause = false } }
bounds { cases 1; live_tasks 1; clock 20min }
metrics { window 20min; currency "JPY"; breakdown waits }
verify reachable end.done
}"#.into()
}
pub fn fields(v: &Value) -> Vec<&str> {
    v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["field"].as_str().unwrap())
        .collect()
}
pub fn assumption_order(v: &Value, simulation: bool) {
    let fs = fields(v);
    let q = [
        "quality.definition",
        "quality.denominator",
        "quality.form",
        if simulation {
            "quality.estimation"
        } else {
            "quality.scheduler"
        },
    ];
    let at: Vec<_> = q
        .iter()
        .map(|f| {
            let xs: Vec<_> = fs
                .iter()
                .enumerate()
                .filter(|(_, a)| *a == f)
                .map(|(i, _)| i)
                .collect();
            assert_eq!(xs.len(), 1, "one {f}: {fs:?}");
            xs[0]
        })
        .collect();
    assert!(
        at.windows(2).all(|w| w[0] < w[1]),
        "old quality order: {fs:?}"
    );
    let waiting: Vec<_> = fs
        .iter()
        .enumerate()
        .filter(|(_, f)| **f == "metrics.waiting")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(waiting.len(), 1, "one waiting row: {fs:?}");
    assert!(
        at.iter().all(|i| *i < waiting[0]),
        "all four quality rows precede waiting: {fs:?}"
    );
    assert!(
        fs.iter()
            .enumerate()
            .filter(|(_, f)| matches!(**f, "quality.quantile" | "probability.scheduler"))
            .all(|(i, _)| i > waiting[0])
    );
    let mut old = fs.clone();
    old.retain(|f| *f != "metrics.waiting");
    assert_eq!(fs.len(), old.len() + 1);
}

pub fn missing_simulation_disclosure(v: &Value) {
    assert!(
        v.get("metrics").is_none(),
        "E-SIM-0 performs no metric computation"
    );
    assert_eq!(
        fields(v),
        vec![
            "quality.definition",
            "quality.denominator",
            "quality.form",
            "quality.estimation"
        ],
        "B6's four refusal rows only: no metrics.waiting without metrics"
    );
}
pub fn range(n: &str) -> Value {
    json!({"min":n,"max":n,"reason":null})
}
pub fn disclosure(v: &Value) -> Value {
    json!({"agency":v["agency"],"obligations":v["obligations"],"diagnostics":v["diagnostics"]})
}
pub fn ratio(v: &Value) -> &Value {
    v["quality"]["cohort"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "DoneRate")
        .unwrap()
}

pub fn crossing(name: &str) -> String {
    let s = fixture(name);
    assert!(!s.contains("metrics {"));
    append(
        &s,
        "metrics { window 20min; currency \"JPY\"; breakdown waits }",
    )
}
pub fn partition_equals_service(metrics: &Value, end: &str, min: &str, max: &str) {
    let row = metrics["cases"][0]["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["end"] == end)
        .unwrap();
    let service = json!({"min":min,"max":max,"reason":null});
    assert_eq!(row["lead_time"], service);
    assert_eq!(
        row["breakdown"],
        json!({"external":range("0"),"in_service":service,"contest_route":range("0"),"queue":{
        "actor_capacity":range("0"),"resource":range("0"),"executor":range("0"),"combined":range("0")},"intentional":range("0"),"unattributed":range("0")})
    );
    assert_eq!(metrics["overall"]["queue_wait_observed"], range("0"));
}

/// Post-integration review inputs. Hand derivations precede execution in the md.
pub fn revocation(quality_on: bool, rule_on: bool, revocable: bool) -> String {
    let mut s = append(
        &fixture("auth-revocation"),
        "metrics { window 20min; currency \"JPY\"; breakdown waits }",
    );
    if quality_on {
        s = quality(&s);
    }
    if rule_on {
        s = append(
            &s,
            "actor Rule { version \"r1\"; decides pause: bool = true }",
        );
    }
    if !revocable {
        s = s.replace("revocable_by p", "              ");
    }
    s
}

pub fn completion(question: &str, rule_on: bool) -> String {
    let s = append(&hq(false, true), &format!("verify {question} end"));
    if rule_on {
        // Cut membership is explicit; an unrelated Cut error must not cause RED.
        append(
            &s.replace("include [A]", "include [A, Rule]"),
            "actor Rule { version \"r1\"; decides pause: bool = true }",
        )
    } else {
        s
    }
}

pub fn rule_quality(sim: bool) -> String {
    let s = quality(&rule());
    if sim {
        append(&s, "simulation replications 4 seed 1 confidence 0.5")
    } else {
        s
    }
}

pub fn completed_rule_quality(v: &Value, sim: bool) {
    let value = &ratio(v)["value"];
    let expected = if sim {
        let count = json!({"mean":"1","min":"1","max":"1","samples":4,"reason":null});
        json!({"point":"1","numerator":count,"denominator":count,"samples":4,"reason":null})
    } else {
        json!({"lower":"1","upper":"1","basis":"exact","reason":null})
    };
    assert_eq!(*value, expected);
}

pub fn missing_clause_rule_scenario() -> String {
    append(
        &hq(false, false).replace("include [A]", "include [A, Rule]"),
        "actor Rule { version \"r1\"; decides pause: bool = true }",
    )
}

pub fn quality_block(v: &Value, sim: bool, _rule_on: bool) {
    let fs = fields(v);
    let expected = vec![
        "quality.definition",
        "quality.denominator",
        "quality.form",
        if sim {
            "quality.estimation"
        } else {
            "quality.scheduler"
        },
    ];
    let actual: Vec<_> = fs
        .iter()
        .copied()
        .filter(|f| f.starts_with("quality."))
        .collect();
    assert_eq!(actual, expected, "closed quality block: {fs:?}");
    let first = fs.iter().position(|f| *f == expected[0]).unwrap();
    assert_eq!(
        &fs[first..first + expected.len()],
        expected.as_slice(),
        "quality rows including Rule must be adjacent: {fs:?}"
    );
}

pub fn no_metric_disclosure(v: &Value, sim: bool, quality_on: bool, rule_on: bool) {
    assert!(v.get("metrics").is_none(), "no metric computation: {v}");
    if quality_on {
        quality_block(v, sim, rule_on);
    } else {
        assert!(fields(v).iter().all(|f| !f.starts_with("quality.")));
    }
    assert!(
        !fields(v).contains(&"metrics.waiting"),
        "metrics.waiting without metrics: {:?}",
        fields(v)
    );
}

pub fn refusal_status(v: &Value, verdict: &str, code: &str, message: &str) {
    assert_eq!(v["verdict"], verdict);
    let verdict: relca_core::Verdict = serde_json::from_value(v["verdict"].clone()).unwrap();
    assert_eq!(
        verdict.exit_code(),
        if code == "E-SIM-0" || code == "E-SYN-0" {
            3
        } else {
            4
        }
    );
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == code && d["message"].as_str().unwrap().contains(message)),
        "expected refusal, not an unrelated input defect: {v}"
    );
    if v.get("simulation").is_some() {
        assert_eq!(v["simulation"]["attempted_replications"], 0);
        assert_eq!(v["simulation"]["completed_replications"], 0);
    }
}

pub const OLD_METRIC_FIELDS: [&str; 5] = [
    "metrics.window",
    "metrics.currency",
    "metrics.rates",
    "metrics.lead_time",
    "metrics.observation",
];

pub fn legacy_refusal_equal(actual: &Value, control: &Value) {
    let mut projected = actual.clone();
    projected["assumptions"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["field"] != "metrics.waiting");
    // Same-width masking preserves all spans, but necessarily changes source hash.
    projected["model"]["sha256"] = control["model"]["sha256"].clone();
    assert_eq!(
        projected, *control,
        "only waiting and the source hash may differ: preserve ALL legacy rows/payload/order"
    );
}

pub fn legacy_revocation(v: &Value, revocable: bool) {
    let fs = fields(v);
    assert_eq!(fs.contains(&"authority.G.revocable_by"), revocable);
    assert_eq!(
        v["unchecked"]
            .as_array()
            .is_some_and(|a| a.contains(&json!("dynamic_authority_revocation"))),
        revocable
    );
    let metric_fields: Vec<_> = fs
        .iter()
        .copied()
        .filter(|f| f.starts_with("metrics.") && *f != "metrics.waiting")
        .collect();
    assert_eq!(
        metric_fields,
        if revocable {
            OLD_METRIC_FIELDS.to_vec()
        } else {
            vec![]
        },
        "do not erase pre-A7 sibling rows to fix waiting"
    );
    if revocable {
        let rows = v["assumptions"].as_array().unwrap();
        assert_eq!(
            rows.iter()
                .find(|r| r["field"] == "authority.G.revocable_by")
                .unwrap()["value"],
            "person:p"
        );
    }
}

pub fn diff_input(contest: bool, mismatch: bool) -> String {
    if contest {
        let s = append(
            &fixture("wait-contest"),
            "contract Delivery { response Approver.approve.finish -> one_of(end.returned, end.posted) by 20min after Approver.approve.finish otherwise unresolved }\nconfiguration Left { cut Scope; process Flow }\nconfiguration Right { cut Scope; process Flow }",
        );
        if mismatch {
            append(
                &s.replace("Right { cut Scope", "Right { cut Other"),
                "cut Other { include [Approver, Ledger, Clerk, Vendor] }",
            )
        } else {
            s
        }
    } else {
        let s = fixture("wait-compare-diff");
        if mismatch {
            append(
                &s.replace("Right { cut C", "Right { cut Other"),
                "cut Other { include [A, B] }",
            )
        } else {
            s
        }
    }
}

/// Literal inventories, derived in waits_integration.md, never read from output.
pub fn diff_unchecked(contest: bool, mismatch: bool, waits: bool) -> Value {
    let words: &[&str] = match (contest, mismatch, waits) {
        (false, false, false) => &[
            "metric_differences",
            "metric_ranking",
            "metric_semantic_alignment",
        ],
        (false, false, true) => &[
            "metric_differences",
            "metric_ranking",
            "metric_semantic_alignment",
            "waiting_tables",
        ],
        (true, false, false) => &[
            "metric_differences",
            "metric_ranking",
            "metric_semantic_alignment",
            "obligation_tables",
            "contest_runtime_tables",
        ],
        (true, false, true) => &[
            "metric_differences",
            "metric_ranking",
            "metric_semantic_alignment",
            "obligation_tables",
            "contest_runtime_tables",
            "waiting_tables",
        ],
        (false, true, false) => &[
            "time_intent",
            "resource",
            "meaning",
            "authority",
            "visibility",
            "contest",
            "preserve",
            "override",
            "maintain",
            "actor",
            "relation",
            "intent_claims",
            "retire_loses",
            "case_outcomes",
            "case_properties",
            "agency_differences",
        ],
        (false, true, true) => &[
            "time_intent",
            "resource",
            "meaning",
            "authority",
            "visibility",
            "contest",
            "preserve",
            "override",
            "maintain",
            "actor",
            "relation",
            "intent_claims",
            "retire_loses",
            "waiting_tables",
            "case_outcomes",
            "case_properties",
            "agency_differences",
        ],
        (true, true, false) => &[
            "time_intent",
            "resource",
            "meaning",
            "authority",
            "visibility",
            "contest",
            "preserve",
            "override",
            "maintain",
            "actor",
            "relation",
            "intent_claims",
            "retire_loses",
            "responsibility",
            "obligation_tables",
            "contest_runtime_tables",
            "agency_differences",
        ],
        (true, true, true) => &[
            "time_intent",
            "resource",
            "meaning",
            "authority",
            "visibility",
            "contest",
            "preserve",
            "override",
            "maintain",
            "actor",
            "relation",
            "intent_claims",
            "retire_loses",
            "responsibility",
            "obligation_tables",
            "contest_runtime_tables",
            "waiting_tables",
            "agency_differences",
        ],
    };
    // ADR0051: static comparison completes even when runtime metrics abort;
    // only Cut mismatch retains the old relation-unknown inventory unchanged.
    let mut result = json!(words);
    if !mismatch {
        result.as_array_mut().unwrap().extend([
            json!("relation_state_producers"),
            json!("relation_binding_identity"),
        ]);
    }
    result
}

pub fn diff_aborted_input() -> String {
    let s = diff_input(true, false);
    let old = "by 20min after Approver.approve.finish";
    assert_eq!(s.matches(old).count(), 1);
    s.replace(
        old,
        "by 18446744073709551615min after Approver.approve.finish",
    )
}

pub fn diff_aborted_unchecked(waits: bool) -> Value {
    if waits {
        json!([
            "metric_tables",
            "obligation_tables",
            "contest_runtime_tables",
            "waiting_tables",
            "terminals",
            "relation_state_producers",
            "relation_binding_identity"
        ])
    } else {
        json!([
            "metric_tables",
            "obligation_tables",
            "contest_runtime_tables",
            "terminals",
            "relation_state_producers",
            "relation_binding_identity"
        ])
    }
}
