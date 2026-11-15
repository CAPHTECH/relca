//! Hand controls for counting stage-only missing roots without double counting
//! a source already recorded by the old person-wide missing_sources leaf.
use relca_core::mutation::{Options, mutate_with_options};
use serde_json::{Value, json};

fn source(shared: bool, direct: bool, two_sources: bool) -> String {
    let intake_person = if shared { "p" } else { "q" };
    let roots = if two_sources { "Doc,Other" } else { "Doc" };
    let reread = if direct { "reads [Doc];" } else { "" };
    format!(
        r#"model StageMutation version "0.2" {{
      type V = enum {{ pass, fail }}
      type Ack = enum {{ ok }}
      people [p,q]
      actor Doc {{ source document }}
      actor Other {{ source record }}
      actor Intake {{ executors [{intake_person}]; capability enter() -> V {{ reads [{roots}]; duration fixed 1min; result nondet {{ pass }} }} }}
      actor Checker {{ executors [p]; capability confirm(e:V) -> V {{ {reread} duration fixed 1min; result nondet {{ pass,fail }} }} }}
      actor Ledger {{ capacity 1; capability post(v:V) -> Ack {{ commits; duration fixed 1min; result nondet {{ ok }} }} }}
      process Flow {{ e = Intake.enter(); v = Checker.confirm(e); if v == pass {{}} else {{end returned}}; k = Ledger.post(v); end posted }}
      cut C {{ include [Doc,Other,Intake,Checker,Ledger] }}
      responsible p for Checker.confirm
      bounds {{ cases 1; live_tasks 1; clock 10min }}
      verify reachable end.posted
    }}"#
    )
}

fn run(source: &str) -> Value {
    let options = Options::parse(None, Some("original_access_removed")).unwrap();
    let value =
        serde_json::to_value(mutate_with_options(source, "bpr-default", None, &options)).unwrap();
    assert_eq!(value["original"]["verdict"], "satisfied");
    value
}
fn mutant<'a>(value: &'a Value, site: &str) -> &'a Value {
    value["mutations"][9]["mutants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == site)
        .unwrap()
}

#[test]
fn stage_only_missing_source_is_still_assigned_worsening_when_person_missing_is_unchanged() {
    let value = run(&source(true, true, false));
    let m = mutant(&value, "original_access_removed:Checker.confirm:reads:Doc");
    assert_eq!(m["result"], "detected");
    let before = &value["original"]["responsibilities"][0]["observation"];
    let after = &m["report"]["responsibilities"][0]["observation"];
    assert_eq!(before["missing_sources"], json!([]));
    assert_eq!(after["missing_sources"], json!([]));
    assert_eq!(after["stages"][0]["missing"], json!(["Doc"]));
    assert!(
        m["assigned_changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["check"] == "responsibilities"
                && c["field"] == "observation.stages.v.missing"
                && c["before"] == json!([])
                && c["after"] == json!(["Doc"])
                && c["direction"] == "worsened")
    );
}

#[test]
fn same_source_newly_missing_from_person_and_stage_is_recorded_only_by_old_missing_leaf() {
    let value = run(&source(false, true, false));
    let m = mutant(&value, "original_access_removed:Checker.confirm:reads:Doc");
    assert_eq!(m["result"], "detected");
    let after = &m["report"]["responsibilities"][0];
    assert_eq!(after["observation"]["missing_sources"], json!(["Doc"]));
    assert_eq!(after["observation"]["stages"][0]["missing"], json!(["Doc"]));
    assert_eq!(after["warnings"], json!(["W-RESP-1"]));
    let changes = m["assigned_changes"].as_array().unwrap();
    assert_eq!(
        changes
            .iter()
            .filter(|c| c["field"] == "missing_sources")
            .count(),
        1
    );
    assert_eq!(
        changes
            .iter()
            .map(|c| c["field"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["missing_sources", "observation.status", "warnings"]
    );
    assert!(changes.iter().all(|c| c["direction"] == "worsened"));
}

#[test]
fn additional_stage_loss_is_detected_even_when_wresp3_already_exists() {
    let value = run(&source(true, true, true));
    let m = mutant(&value, "original_access_removed:Checker.confirm:reads:Doc");
    let before = &value["original"]["responsibilities"][0];
    let after = &m["report"]["responsibilities"][0];
    assert_eq!(before["warnings"], json!(["W-RESP-3"]));
    assert_eq!(after["warnings"], before["warnings"]);
    assert_eq!(before["observation"]["missing_sources"], json!([]));
    assert_eq!(after["observation"]["missing_sources"], json!([]));
    assert_eq!(
        before["observation"]["stages"][0]["missing"],
        json!(["Other"])
    );
    assert_eq!(
        after["observation"]["stages"][0]["missing"],
        json!(["Doc", "Other"])
    );
    assert_eq!(m["result"], "detected");
    assert_eq!(m["assigned_changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        m["assigned_changes"][0]["field"],
        "observation.stages.v.missing"
    );
    assert_eq!(m["assigned_changes"][0]["direction"], "worsened");
}

#[test]
fn removing_a_required_root_does_not_add_a_stage_improvement_record() {
    let value = run(&source(false, false, true));
    let m = mutant(&value, "original_access_removed:Intake.enter:reads:Doc");
    let after = &m["report"]["responsibilities"][0]["observation"];
    assert_eq!(after["stages"][0]["missing"], json!(["Other"]));
    let changes = m["assigned_changes"].as_array().unwrap();
    assert!(
        changes
            .iter()
            .any(|c| c["field"] == "baseline-required_source_lost" && c["direction"] == "worsened")
    );
    assert!(!changes.iter().any(|c| {
        c["field"]
            .as_str()
            .unwrap()
            .starts_with("observation.stages.")
    }));
}
