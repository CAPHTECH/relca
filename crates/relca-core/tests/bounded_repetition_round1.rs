use relca_core::{lower, report, syntax};
use serde_json::json;

const BASE: &str = r#"model R1 version "0.2" {
type V=enum {ok}
actor A {capacity 1; capability c()->V {duration fixed 1min; result nondet {ok}}}
process P { repeat work at_most 2 { r=A.c(); again } on_limit { end limit } end done }
cut C {include [A]}
bounds {cases 1;live_tasks 1;clock 100min}
verify safety no_resource_overcommit
}"#;

fn diagnostic(source: &str, code: &str, reason: &str) {
    let ast = syntax::parse(source);
    let ds = match ast {
        Err(ds) => ds,
        Ok(ast) => lower::lower(&ast).unwrap_err(),
    };
    assert!(
        ds.iter()
            .any(|d| d.code == code && d.message.contains(reason)),
        "{ds:?}"
    );
}

#[test]
fn unreachable_after_control_is_never_silently_discarded() {
    for body in [
        "r=A.c(); again; wait 1min",
        "r=A.c(); leave; if absent == ok { end x } else { end y }",
    ] {
        diagnostic(
            &BASE.replace("r=A.c(); again", body),
            "E-CON-0",
            "loop_unreachable_after_control",
        );
    }
    diagnostic(
        &BASE.replace("on_limit { end limit }", "on_limit { leave; wait 1min }"),
        "E-CON-0",
        "loop_unreachable_after_control",
    );
}

#[test]
fn item_repetition_is_stage_unsupported_in_every_header_position() {
    for header in [
        "repeat work over lines at_most 2",
        "repeat work at_most 2 over lines",
        "repeat work at_most 2 gap fixed 1min over lines",
    ] {
        diagnostic(
            &BASE.replace("repeat work at_most 2", header),
            "U-SEM-0",
            "loop_stage_unsupported",
        );
    }
}

#[test]
fn gap_contributes_to_the_static_minimum_iteration_time() {
    let source = BASE
        .replace("duration fixed 1min", "duration fixed 0min")
        .replace("at_most 2", "at_most 2 gap fixed 10min");
    let value = serde_json::to_value(report::check(&source)).unwrap();
    assert_eq!(value["verdict"], "satisfied", "{value}");
    assert_eq!(value["terminals"][0]["elapsed"], json!({"min":10,"max":10}));
    diagnostic(
        &source.replace("gap fixed 10min", "gap fixed 0min"),
        "E-CON-0",
        "zero_time_iteration",
    );
}

#[test]
fn lineage_group_judgements_count_original_sites() {
    let value = serde_json::to_value(report::check(include_str!(
        "bounded_repetition/models/lineage.relca"
    )))
    .unwrap();
    let groups = value["agency"]["lineage_groups"].as_array().unwrap();
    let group = groups
        .iter()
        .find(|g| g["subject_sources"] == json!(["Invoice"]))
        .unwrap();
    assert_eq!(group["judgements"], 1, "{group}");
    assert_eq!(group["actors"], 1);
    let source = include_str!("bounded_repetition/models/lineage.relca")
        // Two sites and two rounds stay inside the existing Agency path budget;
        // the original one-site/three-round acceptance above remains intact.
        .replace("at_most 3", "at_most 2")
        .replace(
            "r = Rev.review(entry)",
            "r = Rev.review(entry); other = Rev.review(entry)",
        )
        .replace("if r == pass", "if r == pass && other == pass");
    let value = serde_json::to_value(report::check(&source)).unwrap();
    let group = value["agency"]["lineage_groups"]
        .as_array()
        .unwrap_or_else(|| panic!("{value}"))
        .iter()
        .find(|g| g["subject_sources"] == json!(["Invoice"]))
        .unwrap();
    assert_eq!(
        group["judgements"], 2,
        "distinct original sites must not merge by capability: {group}"
    );
    assert_eq!(group["actors"], 1);
}

#[test]
fn on_limit_has_a_phase_and_no_body_round() {
    let source = BASE.replace("on_limit { end limit }", "on_limit { last=A.c(); leave }")
        .replace("verify safety no_resource_overcommit", "verify safety no_resource_overcommit\nmetrics {window 100min;currency \"JPY\";rate A per_minute 1;rate A.c per_start 1}\ncontract Deadline {response case.received -> one_of(end.done) by 2min after case.received otherwise unresolved}");
    let value = serde_json::to_value(report::check(&source)).unwrap();
    let site = value["repetition"]["sites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["origin"]["call_site"] == "last")
        .unwrap();
    assert_eq!(site["origin"]["phase"], "on_limit");
    assert!(site["origin"]["round"].is_null());
    let task = value["metrics"]["cases"][0]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["binding"] == "@loop:work:2:last")
        .unwrap();
    assert_eq!(task["repetition"], site["origin"]);
    let body = value["repetition"]["sites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["origin"]["call_site"] == "r")
        .unwrap();
    assert_eq!(body["origin"]["phase"], "body");
    assert_eq!(body["origin"]["round"], 1);
    let explain = serde_json::to_value(relca_core::explain::explain(&source)).unwrap();
    let traces = explain["explanation"]["counterexamples"]
        .as_array()
        .unwrap();
    assert!(!traces.is_empty());
    assert!(
        traces.iter().all(|t| t["replay"]["status"] == "verified"),
        "{traces:?}"
    );
    let event = traces
        .iter()
        .flat_map(|t| t["steps"].as_array().unwrap())
        .flat_map(|s| s["events"].as_array().unwrap())
        .find(|e| e["binding"] == "@loop:work:2:last")
        .unwrap();
    assert_eq!(event["repetition"], site["origin"]);
}

#[test]
fn zero_wait_remains_a_replayable_explain_step() {
    let source = include_str!("bounded_repetition/models/L1-FFF.relca")
        .replace("r = Rev.review()", "wait 0min; r = Rev.review()");
    let value = serde_json::to_value(relca_core::explain::explain(&source)).unwrap();
    let traces = value["explanation"]["counterexamples"].as_array().unwrap();
    assert!(
        traces.iter().all(|t| t["replay"]["status"] == "verified"),
        "{traces:?}"
    );
    assert!(
        traces
            .iter()
            .flat_map(|t| t["steps"].as_array().unwrap())
            .any(|s| s["kind"] == "wait"),
        "{traces:?}"
    );
}
