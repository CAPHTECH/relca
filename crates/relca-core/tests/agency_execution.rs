//! Hand-computed execution projections, independent of Agency derivation output.
//! Public check entry points only; reference/engine comparison is a separate test.

use relca_core::report;
use serde_json::{Value, json};
use std::path::Path;

fn point(end: &str, elapsed: u64, work: u64, result_at: Option<u64>) -> Value {
    json!({"end":end,"elapsed":{"min":elapsed,"max":elapsed},"work":{"min":work,"max":work},"result_at":result_at.map(|n|json!({"min":n,"max":n}))})
}

fn sorted(value: &Value) -> Vec<Value> {
    let mut rows = value.as_array().unwrap().clone();
    rows.sort_by_key(|v| v["end"].as_str().unwrap().to_owned());
    rows
}

#[test]
fn all_nine_fixture_executions_match_hand_computed_time_work_and_completion() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let cases = [
        (
            "agency-lineage",
            None,
            vec![point("rejected", 4, 4, None), point("done", 5, 5, None)],
        ),
        (
            "agency-lineage.broken-missing-evidence",
            None,
            vec![point("rejected", 4, 4, None), point("done", 5, 5, None)],
        ),
        (
            "agency-lineage.broken-reused-source",
            None,
            vec![point("rejected", 4, 4, None), point("done", 5, 5, None)],
        ),
        (
            "agency-origin",
            None,
            vec![point("rejected", 2, 2, None), point("done", 3, 3, None)],
        ),
        (
            "agency-origin.broken-no-direct-read",
            None,
            vec![point("rejected", 2, 2, None), point("done", 3, 3, None)],
        ),
        (
            "agency-any",
            None,
            vec![point("rejected", 2, 3, None), point("done", 3, 4, Some(2))],
        ),
        (
            "agency-any.broken-single-valued-a",
            None,
            vec![point("done", 3, 4, Some(2))],
        ),
        (
            "agency-refusal",
            Some("AsIs"),
            vec![point("rejected", 3, 3, None), point("done", 4, 4, None)],
        ),
        (
            "agency-refusal",
            Some("ToBe"),
            vec![point("rejected", 3, 3, None), point("done", 4, 4, None)],
        ),
        (
            "agency-refusal.broken-cut-guard",
            Some("AsIs"),
            vec![point("rejected", 3, 3, None), point("done", 4, 4, None)],
        ),
        (
            "agency-refusal.broken-cut-guard",
            Some("ToBe"),
            vec![point("done", 4, 4, None)],
        ),
    ];
    assert_eq!(
        cases.len(),
        11,
        "9 directories, two selected sides for each diff"
    );
    let mut failures = Vec::new();
    for (name, configuration, expected) in cases {
        let source = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
        let report = match configuration {
            Some(name) => report::check_configuration(&source, name),
            None => report::check(&source),
        };
        let actual = serde_json::to_value(report).unwrap();
        if actual["verdict"] != "satisfied" {
            failures.push(format!("{name} {configuration:?}: {actual}"));
            continue;
        }
        assert_eq!(actual["bounds"]["truncated"], false);
        assert_eq!(
            sorted(&actual["terminals"]),
            sorted(&json!(expected)),
            "{name} {configuration:?}"
        );
        assert_eq!(
            actual["properties"],
            json!([
                {"id":"reachable end.done","verdict":"satisfied"},
                {"id":"safety no_resource_overcommit","verdict":"satisfied"}
            ])
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn metadata_free_control_preserves_each_of_the_four_hand_completion_combinations() {
    // Already-supported syntax control. This is not an Agency oracle: it
    // establishes the execution witness used by the metadata test below.
    let source = r#"model Control version "0.2" {
      type V = enum { pass, fail }
      actor A { capacity 1
        capability input() -> V { duration fixed 1min; result nondet { pass, fail } }
        capability output() -> V { duration fixed 1min; result nondet { pass, fail } }
      }
      process P { i = A.input(); o = A.output()
        if i == pass { if o == pass { end pp } else { end pf } }
        else { if o == pass { end fp } else { end ff } }
      }
      bounds { cases 1; live_tasks 4; clock 32min }
      verify reachable end.pp
      verify reachable end.pf
      verify reachable end.fp
      verify reachable end.ff
      verify safety no_resource_overcommit
    }"#;
    let actual = serde_json::to_value(report::check(source)).unwrap();
    assert_eq!(actual["verdict"], "satisfied");
    assert_eq!(
        sorted(&actual["terminals"]),
        sorted(&json!([
            point("pp", 2, 2, None),
            point("pf", 2, 2, None),
            point("fp", 2, 2, None),
            point("ff", 2, 2, None)
        ]))
    );
    assert_eq!(actual["bounds"]["truncated"], false);
}

#[test]
fn metadata_and_actual_argument_values_do_not_choose_nondeterministic_results() {
    // Each input result reaches BOTH output results. Guard each combination
    // into a named end so a result-pruning implementation cannot hide in ranges.
    let source = r#"model Independence version "0.2" {
      type V = enum { pass, fail }
      actor Source { source document }
      actor A { capacity 1
        capability input() -> V { reads [Source]; duration fixed 1min; result nondet { pass, fail } }
        capability output(v:V) -> V { reads [Source]; commits; duration fixed 1min; result nondet { pass, fail } }
      }
      process P { i = A.input(); o = A.output(i)
        if i == pass { if o == pass { end pp } else { end pf } }
        else { if o == pass { end fp } else { end ff } }
      }
      bounds { cases 1; live_tasks 4; clock 32min }
      verify reachable end.pp
      verify reachable end.pf
      verify reachable end.fp
      verify reachable end.ff
      verify safety no_resource_overcommit
    }"#;
    let stripped = source
        .replace("source document", "")
        .replace("reads [Source];", "")
        .replace("commits;", "")
        .replace("output(v:V)", "output()")
        .replace("A.output(i)", "A.output()");
    for input in [source, stripped.as_str()] {
        let actual = serde_json::to_value(report::check(input)).unwrap();
        assert_eq!(actual["verdict"], "satisfied", "{actual}");
        assert_eq!(actual["bounds"]["truncated"], false);
        assert_eq!(
            sorted(&actual["terminals"]),
            sorted(&json!([
                point("pp", 2, 2, None),
                point("pf", 2, 2, None),
                point("fp", 2, 2, None),
                point("ff", 2, 2, None)
            ]))
        );
        assert!(
            actual["properties"]
                .as_array()
                .unwrap()
                .iter()
                .all(|p| p["verdict"] == "satisfied")
        );
    }
}
