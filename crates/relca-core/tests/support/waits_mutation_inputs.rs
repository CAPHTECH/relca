#![allow(dead_code)]
use serde_json::{Value, json};
#[path = "waits_integration_inputs.rs"]
mod a7;
#[path = "mutation_integration_inputs.rs"]
mod c6;
#[path = "response_boundary_oracle.rs"]
mod response_boundary;

pub fn hq() -> String {
    c6::replace(&a7::hq(false, false), "horizon 3min", "horizon 12min")
}
pub fn summary(horizon: u64, clock: u64, all_six: bool) -> String {
    let s = c6::S.replace("result nondet { pass, fail }", "result nondet { pass }");
    let s = c6::replace(&s, "bounds { cases 1; live_tasks 2; clock 10min }", "");
    let mut s = c6::add(
        &s,
        &format!(
            "policy fifo tie_by case_id\nscenario Day {{ arrivals fixed [0min,0min]; horizon {horizon}min; bounds {{ cases 2; live_tasks 1; clock {clock}min }} }}\nmetrics {{ window 20min; currency \"JPY\"; breakdown waits }}\nproperty case.Done = reached(end.done)\nmetric cohort.DoneRate = expected(count(case.Done)) / arrivals"
        ),
    );
    if all_six {
        s = c6::replace(&s, "arrivals fixed [0min,0min]", "arrivals fixed [0min]");
        s = c6::replace(&s, "bounds { cases 2;", "bounds { cases 1;");
        s = c6::replace(
            &s,
            "actor Extract { via model \"doc@v3\"; capacity 1;",
            "actor Extract { via model \"doc@v3\"; executors [p];",
        );
        s = c6::add(
            &s,
            "people [p,cfo]\nauthority G { basis \"prior permission\"; grants p to Extract.make; issued_by cfo }\ncontract K { authority_basis Extract.make G }",
        );
    }
    s
}
pub fn manual_summary(s: &str) -> String {
    c6::manual(s)
}
pub fn waiting_only() -> String {
    let s = c6::replace(
        &c6::guard_horizon(8),
        "else { end rejected }",
        "else { wait 2min; end rejected }",
    );
    a7::quality(&c6::add(
        &s,
        "metrics { window 20min; currency \"JPY\"; breakdown waits }",
    ))
}
pub fn masked(s: &str) -> String {
    a7::masked(s)
}
pub fn configurations(s: &str) -> String {
    c6::add(
        s,
        "configuration Left { cut Scope; process Flow }\nconfiguration Right { cut Scope; process Flow }",
    )
}
pub fn rule_refusal() -> String {
    c6::add(
        &c6::rule(true, true),
        "metrics { window 20min; currency \"JPY\"; breakdown waits }",
    )
}
pub fn no_metrics_no_waiting(r: &Value) {
    assert!(r.get("metrics").is_none());
    assert!(
        !r["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"] == "metrics.waiting")
    );
}
pub fn range(n: &str) -> Value {
    a7::range(n)
}
pub fn order(r: &Value) {
    a7::assumption_order(r, false)
}
pub fn only(v: &Value, kind: usize) -> &Value {
    let rows = v["mutations"][kind]["mutants"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "kind {kind}");
    &rows[0]
}
pub fn cases(
    r: &Value,
    leads: [&str; 2],
    services: [&str; 2],
    queues: [&str; 2],
    actor: &str,
    total: &str,
    avg: &str,
) {
    assert_eq!(r["metrics"]["status"], "complete");
    assert_eq!(r["metrics"]["cases"].as_array().unwrap().len(), 2);
    for (ix, row) in r["metrics"]["cases"].as_array().unwrap().iter().enumerate() {
        let terminal = row["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["end"] == "done")
            .unwrap();
        assert_eq!(terminal["lead_time"], range(leads[ix]));
        assert_eq!(terminal["breakdown"]["in_service"], range(services[ix]));
        assert_eq!(
            terminal["breakdown"]["queue"]["actor_capacity"],
            range(queues[ix])
        );
        assert!(row.get("dwell").is_none());
    }
    let a = r["metrics"]["actors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["actor"] == actor)
        .unwrap();
    assert_eq!(a["queue"]["wait_total"], json!({"stats":range(total)}));
    assert_eq!(
        a["queue"]["length_time_average"],
        json!({"stats":range(avg)})
    );
    assert_eq!(a["queue"]["length_max"], range("1"));
    assert_eq!(r["quality"]["cohort"][0]["value"]["lower"], "1");
    assert_eq!(r["quality"]["cohort"][0]["value"]["upper"], "1");
}
pub fn envelope(v: &Value, control: &Value) {
    envelope_with_ends(v, control, &["rejected", "done"]);
}
pub fn envelope_with_ends(v: &Value, control: &Value, ends: &[&str]) {
    assert_eq!(
        v["verdict"], "unsupported",
        "{}",
        v["original"]["diagnostics"]
    );
    assert_eq!(v["original_verdict"], "satisfied");
    assert_eq!(v["mutations"].as_array().unwrap().len(), 10);
    assert_eq!(v["mutations"][5]["status"], "unsupported");
    response_boundary::assert_no_declared_sites(&v["mutations"]);
    // ADR0047 intake: the immutable lexical inventory, not observed outcomes,
    // determines structural kind7. Waiting does not add a candidate or a driver.
    let row = &v["mutations"][7];
    assert_eq!(row, &control["mutations"][7]);
    if ends.len() < 2 {
        assert_eq!(v["run"]["candidates_total"], 0);
        assert!(v["run"]["reason"].is_null());
        assert_eq!(row["status"], "not_applicable");
        assert_eq!(row["reason"], "no_matching_site");
        assert_eq!(row["mutants"], json!([]));
        assert_eq!(
            row["skipped_sites"],
            json!([{"cohort":"DoneRate","status":"not_applicable","reason":"no_matching_site"}])
        );
    } else {
        assert_eq!(row["status"], "generated");
        assert_eq!(row["skipped_sites"], json!([]));
        let mutants = row["mutants"].as_array().unwrap();
        assert_eq!(mutants.len(), ends.len());
        for (m, end) in mutants.iter().zip(ends) {
            assert_eq!(m["site"]["removed_end"], *end);
            assert_eq!(m["result"], "detected");
            assert!(m["report"].is_null());
            assert_eq!(m["operator"], "remove_denominator_end");
        }
    }
    assert_eq!(v["run"], control["run"]);
    assert_eq!(v["original"]["scenario"], control["original"]["scenario"]);
    assert!(v["original"]["scenario"].is_object());
    assert_eq!(v["original"]["scenario"]["name"], "Day");
    order(&v["original"]);
    for k in [0, 1, 2, 4, 8, 9] {
        assert_eq!(
            v["mutations"][k]["status"],
            control["mutations"][k]["status"]
        );
        let ms = v["mutations"][k]["mutants"].as_array().unwrap();
        let cs = control["mutations"][k]["mutants"].as_array().unwrap();
        assert_eq!(ms.len(), cs.len());
        for (m, c) in ms.iter().zip(cs) {
            for f in [
                "id",
                "operator",
                "site",
                "result",
                "reason",
                "assigned_changes",
            ] {
                assert_eq!(m[f], c[f], "kind{k}/{f}");
            }
            // Selection/declaration metadata are invariant. observed_until is
            // dynamic: the hand-edited summary extends5 to7 (or cutoff6).
            assert_eq!(m["report"]["scenario"], c["report"]["scenario"]);
            for f in ["name", "arrivals", "horizon", "warmup", "drain"] {
                assert_eq!(m["report"]["scenario"][f], v["original"]["scenario"][f]);
            }
            assert_eq!(m["report"]["quality"], c["report"]["quality"]);
            assert_eq!(m["report"]["statistics"], c["report"]["statistics"]);
            assert!(
                m["report"]["metrics"]["overall"]
                    .get("queue_wait_observed")
                    .is_some()
            );
            assert!(
                m["assigned_changes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|c| c["check"] != "metrics" && c["check"] != "quality")
            );
            order(&m["report"]);
        }
    }
}
pub fn held(m: &Value) {
    assert_eq!(m["result"], "detected");
    assert_eq!(m["reason"], Value::Null);
    let cs = m["assigned_changes"].as_array().unwrap();
    assert_eq!(cs.len(), 3);
    assert_eq!(cs[0]["before"], json!(["Invoice"]));
    assert_eq!(cs[0]["after"], json!(["Invoice"]));
    assert_eq!(cs[1]["before"], json!([]));
    assert_eq!(cs[1]["after"], json!([]));
    assert_eq!(cs[2]["before"], 2);
    assert_eq!(cs[2]["after"], 2);
    assert!(cs.iter().all(|c| c["direction"] == "held"));
}
