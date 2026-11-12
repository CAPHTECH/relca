use serde_json::{Value, json};

pub fn day() -> String {
    let old = include_str!("../../../../fixtures/eval-mut7/model.relca");
    let new = old.replace(
        "arrivals fixed [0min, 10min]\n  bounds { cases 2; live_tasks 2; clock 30min }",
        "scenario Day { arrivals fixed [0min, 10min]; bounds { cases 2; live_tasks 2; clock 30min } }",
    );
    assert_ne!(new, old);
    new
}

fn append(src: &str, tail: &str) -> String {
    let at = src.rfind('}').unwrap();
    format!("{}\n{tail}\n}}", &src[..at])
}

pub fn summary(horizon: u32, clock: u32, cohort: bool) -> String {
    let src = include_str!("../../../../fixtures/mut2-summary/model.relca");
    let src = src.replace("bounds { cases 1; live_tasks 2; clock 10min }", &format!(
        "scenario Day {{ arrivals fixed [0min]; horizon {horizon}min; bounds {{ cases 1; live_tasks 2; clock {clock}min }} }}"));
    append(
        &src,
        if cohort {
            "property case.Done = reached(end.done)\nmetric cohort.DoneRate = expected(count(case.Done)) / arrivals"
        } else {
            "property case.Done = reached(end.done)"
        },
    )
}

pub fn mixed() -> String {
    let src = summary(10, 20, true)
        .replace("type Entry =", "people [p,cfo]\n  type Entry =")
        .replace(
            "actor Approver { capacity 1;",
            "actor Approver { executors [p];",
        );
    append(
        &src,
        "authority Grant { basis \"finite control\"; grants p to Approver.approve; issued_by cfo }\ncontract Controls { authority_basis Approver.approve Grant }",
    )
}

fn row(v: &Value, i: usize) -> &Value {
    &v["mutations"][i]
}

#[test]
fn day_preserves_all_six_rows_and_four_structural_candidates() {
    let v = crate::run(&day(), true, true);
    assert_eq!(v["original_verdict"], "satisfied");
    assert_eq!(
        v["original"]["quality"]["cohort"][0]["value"]["lower"],
        "1/50"
    );
    assert_eq!(v["run"]["completion"], "inconclusive");
    assert_eq!(v["run"]["reason"], "mutant_incomplete");
    assert_eq!(v["verdict"], "unsupported");
    for i in [0, 1, 4, 9] {
        assert_eq!(row(&v, i)["status"], "not_applicable");
    }
    for i in [2, 8] {
        assert_eq!(row(&v, i)["status"], "generated");
        let ms = row(&v, i)["mutants"].as_array().unwrap();
        assert_eq!(ms.len(), 1);
        assert!(ms[0]["report"].is_object());
        assert_eq!(ms[0]["result"], "invalid_mutant");
        assert_eq!(ms[0]["reason"], "static_invalid");
        assert!(
            ms[0]["report"]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-NAME-0")
        );
        assert!(ms[0]["report"]["scenario"].is_null());
    }
    let r = row(&v, 7);
    assert_eq!(r["skipped_sites"].as_array().unwrap().len(), 1);
    assert_eq!(r["mutants"].as_array().unwrap().len(), 4);
    for m in r["mutants"].as_array().unwrap() {
        assert_eq!(m["result"], "detected");
        assert!(m["report"].is_null());
        assert_eq!(m["assigned_changes"].as_array().unwrap().len(), 2);
        assert_eq!(m["other_changes"], json!([]));
    }
    assert!(
        v["checked"]
            .as_array()
            .unwrap()
            .contains(&json!("quality_denominator"))
    );
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("quality_changes"))
    );
}

#[test]
fn omitted_selector_blocks_six_and_keeps_kind7_unsupported() {
    let v = crate::run(&day(), true, false);
    assert_eq!(v["verdict"], "invalid");
    assert_eq!(v["run"]["completion"], "blocked");
    assert!(
        v["original"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CONF-0")
    );
    for i in [0, 2, 8, 9, 1, 4] {
        assert_eq!(row(&v, i)["status"], "blocked");
        assert_eq!(row(&v, i)["mutants"], json!([]));
    }
    assert_eq!(row(&v, 7)["status"], "unsupported");
    assert_eq!(row(&v, 7)["reason"], "quality_denominator_unavailable");
}

#[test]
fn incomplete_original_blocks_six_and_never_invents_structural_trials() {
    let v = crate::run(&summary(2, 10, true), true, true);
    assert_eq!(v["original_verdict"], "inconclusive");
    assert_eq!(v["original"]["bounds"]["truncated"], false);
    assert_eq!(v["run"]["completion"], "blocked");
    for i in [0, 2, 8, 9, 1, 4] {
        assert_eq!(row(&v, i)["status"], "blocked");
        assert_eq!(row(&v, i)["mutants"], json!([]));
    }
    assert_eq!(row(&v, 7)["status"], "unsupported");
    assert_eq!(row(&v, 7)["mutants"], json!([]));
    assert!(
        !v["checked"]
            .as_array()
            .unwrap()
            .contains(&json!("quality_denominator"))
    );
}

fn summary_oracle(h: u32, c: u32, result: &str, reason: Value) {
    let v = crate::run(&summary(h, c, true), true, true);
    assert_eq!(v["original_verdict"], "satisfied");
    let ms = row(&v, 4)["mutants"].as_array().unwrap();
    assert_eq!(ms.len(), 1);
    let m = &ms[0];
    assert_eq!(m["operator"], "duplicate_summary");
    assert_eq!(m["result"], result);
    assert_eq!(m["reason"], reason);
    for field in ["name", "arrivals", "horizon", "warmup", "drain"] {
        assert_eq!(
            m["report"]["scenario"][field],
            v["original"]["scenario"][field]
        );
    }
    // Aggregation is observed output, not a scenario selector: the inserted
    // producer adds one minute. It must not be frozen to the original range.
    let observed = &m["report"]["scenario"]["aggregation"]["observed_until"];
    assert_eq!(
        observed,
        &if c < h {
            json!({"min":null,"max":null,"reason":"truncated"})
        } else {
            json!({"min":"4","max":if h==4 {"4"} else {"5"},"reason":null})
        }
    );
    assert_eq!(m["report"]["bounds"]["truncated"], c < h);
    assert_eq!(
        v["run"]["completion"],
        if result == "detected" {
            "complete"
        } else {
            "inconclusive"
        }
    );
    assert_eq!(
        v["run"]["reason"],
        if result == "detected" {
            Value::Null
        } else {
            json!("mutant_incomplete")
        }
    );
    if result == "detected" {
        let a = m["assigned_changes"].as_array().unwrap();
        assert_eq!(a.len(), 3);
        assert!(a.iter().all(|a| a["direction"] == "held"));
        assert_eq!(a[2]["before"], 2);
        assert_eq!(a[2]["after"], 2);
    }
    let k7 = row(&v, 7)["mutants"].as_array().unwrap();
    assert_eq!(k7.len(), 2);
    assert!(
        k7.iter()
            .all(|m| m["result"] == "detected" && m["report"].is_null())
    );
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("quality_changes"))
    );
}

#[test]
fn summary_complete_preserves_two_direct_consumer_judgements() {
    summary_oracle(5, 10, "detected", Value::Null);
}
#[test]
fn summary_horizon_incomplete_cannot_be_rescued_by_structural_kind7() {
    summary_oracle(4, 10, "inconclusive", json!("assigned_check_unavailable"));
}
#[test]
fn summary_clock_incomplete_stays_computational() {
    summary_oracle(10, 4, "inconclusive", json!("truncated"));
}

#[test]
fn noncohort_keeps_kind7_unsupported_with_summary_generated() {
    let v = crate::run(&summary(5, 10, false), true, true);
    assert_eq!(row(&v, 4)["mutants"][0]["result"], "detected");
    assert_eq!(row(&v, 7)["status"], "unsupported");
    assert_eq!(row(&v, 7)["reason"], "quality_denominator_unavailable");
}

#[test]
fn expiry_summary_and_definition_comparison_coexist_without_quality_other_changes() {
    let v = crate::run(&mixed(), true, true);
    assert_eq!(v["original_verdict"], "satisfied");
    let expiry = &row(&v, 1)["mutants"][0];
    assert_eq!(expiry["operator"], "expire_authority");
    assert_eq!(expiry["result"], "detected");
    assert_eq!(expiry["site"]["authority"], "Grant");
    assert_eq!(expiry["report"]["verdict"], "violated");
    assert_eq!(row(&v, 4)["mutants"][0]["result"], "detected");
    assert_eq!(row(&v, 7)["mutants"].as_array().unwrap().len(), 2);
    for i in [0, 2, 8, 9, 1, 4] {
        for m in row(&v, i)["mutants"].as_array().unwrap() {
            assert!(m["report"].is_object());
            assert!(
                m["other_changes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|c| c["field"] != "quality" && c["check"] != "quality_denominator")
            );
        }
    }
}

#[test]
fn one_input_fault_invalidates_the_hand_quality_expectation() {
    let s = day().replace(
        "0.02: pass with correct = false",
        "0.02: pass with correct = true",
    );
    let v = crate::run(&s, false, true);
    assert_eq!(v["quality"]["cohort"][0]["value"]["lower"], "0");
    assert_ne!(v["quality"]["cohort"][0]["value"]["lower"], "1/50");
}
