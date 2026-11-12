//! Review regressions authored before the corresponding production changes.
#[path = "support/p3a_inputs.rs"]
mod input;
#[path = "support/response_loss_inputs.rs"]
mod q1;
use relca_core::{agency, compare, diff, report};
use serde_json::{Value, json};

fn lineage_source(guarded: bool) -> String {
    let judged =
        "v=Check.judge(doc);if v == pass {posted=Poster.post(doc);end done} else {end done}";
    let body = if guarded {
        format!("if gate == true {{{judged}}} else {{end done}}")
    } else {
        judged.into()
    };
    format!(
        r#"model GuardedJudgement version "0.2" {{
      type Entry=enum {{entry}}
      type V=enum {{pass,fail}}
      type Ack=enum {{committed}}
      observed case {{gate:bool=true}}
      actor Original {{source document}}
      actor Evidence {{source record}}
      actor Pulse {{capacity 1;capability run()->V {{duration fixed 1min;result nondet {{pass}}}}}}
      actor Reader {{capacity 1;capability read()->Entry {{reads [Original];duration fixed 1min;result nondet {{entry}}}}}}
      actor Check {{capacity 1;capability judge(doc:Entry)->V {{reads [doc,Evidence];duration fixed 1min;result nondet {{pass}}}}}}
      actor Poster {{capacity 1;capability post(doc:Entry)->Ack {{commits;reads [doc];duration fixed 1min;result nondet {{committed}}}}}}
      process Flow {{pulse=Pulse.run() response_loss none;doc=Reader.read();{body}}}
      cut C {{include [Original,Evidence,Pulse,Reader,Check,Poster]}}
      bounds {{cases 1;live_tasks 2;clock 10min}}
      verify safety no_resource_overcommit
    }}"#
    )
}

fn lineage_pair(from_guard: bool, to_guard: bool) {
    let from = report::check(&lineage_source(from_guard));
    let to = report::check(&lineage_source(to_guard));
    for (r, guarded) in [(&from, from_guard), (&to, to_guard)] {
        assert_eq!(
            r.verdict,
            relca_core::Verdict::Satisfied,
            "{:?}",
            r.diagnostics
        );
        let a = r.agency.as_ref().unwrap().as_derived().unwrap();
        assert_eq!(a.rules, "agency-p2-response-1");
        assert_eq!(
            a.lineage.iter().any(|j| j.capability == "Check.judge"),
            !guarded,
            "premise: the legacy producer omits only the state-guarded judgement"
        );
    }
    fn side(r: &report::Report) -> agency::ComparisonSide<'_> {
        agency::ComparisonSide {
            agency: r
                .agency
                .as_ref()
                .and_then(agency::AgencyDerivation::as_derived),
            responsibilities: r.responsibilities.as_deref(),
        }
    }
    let value = json!(agency::differences(side(&from), side(&to)));
    assert_eq!(value["status"], "derived", "{value}");
    let groups: Vec<_> = value["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["viewpoint"] == "lineage_groups")
        .collect();
    assert!(
        !groups.is_empty(),
        "missing groups are not a derived equality: {value}"
    );
    for row in groups {
        assert_eq!(row["status"], "not_comparable", "{row}");
        for (owner, guarded) in [("from", from_guard), ("to", to_guard)] {
            assert_eq!(
                row[owner]["status"],
                if guarded { "not_derived" } else { "derived" }
            );
            assert_eq!(
                row[owner]["reason"],
                if guarded {
                    json!("state_guards")
                } else {
                    Value::Null
                }
            );
        }
    }
    serde_json::from_value::<agency::Differences>(value).unwrap();
}

#[test]
fn response_lineage_guard_on_one_side_stays_unknown_in_both_directions() {
    lineage_pair(true, false);
    lineage_pair(false, true);
}

#[test]
fn response_lineage_guards_on_both_sides_are_not_derived_equality() {
    lineage_pair(true, true);
}

fn applicability(value: &Value) {
    let marks = value["unchecked"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| **x == "receipt_resolution_not_applicable")
        .count();
    assert_eq!(
        marks, 1,
        "one-sided automatic property omission must be explicit: {}",
        value["unchecked"]
    );
}

#[test]
fn compare_discloses_one_sided_receipt_resolution_in_either_plan_order() {
    let source = input::one_sided_matching_ends();
    for plans in [["A", "B"], ["B", "A"]] {
        let value = json!(compare::compare(
            &source,
            &plans,
            compare::Analysis::Exhaustive
        ));
        assert_eq!(value["verdict"], "inconclusive", "{}", value["diagnostics"]);
        applicability(&value);
    }
    let same = json!(compare::compare(
        &input::pair("g-unknown"),
        &["A", "B"],
        compare::Analysis::Exhaustive
    ));
    assert!(
        !same["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "receipt_resolution_not_applicable")
    );
}

#[test]
fn diff_discloses_one_sided_receipt_resolution_in_either_direction() {
    let source = input::one_sided_matching_ends();
    for (from, to) in [("A", "B"), ("B", "A")] {
        let value = json!(diff::diff(&source, from, to));
        assert_eq!(value["verdict"], "inconclusive", "{}", value["diagnostics"]);
        applicability(&value);
        serde_json::from_value::<diff::DiffReport>(value).unwrap();
    }
}

#[test]
fn statistical_violation_wins_unsupported_without_losing_statistics_or_u_diagnostic() {
    // Four deterministic hits; confidence7/8, M=1 => alpha1/16.
    // CP lower=(1/16)^(1/4)=1/2 > fixed bound1/10; this is statistical,
    // not a witness property or an all-schedules safety proof.
    let source = r#"model StatisticalFold version "0.2" {
      type V=enum {ok,nope}
      actor A {capacity 1;capability run()->V {duration fixed 1min;result nondet {ok}}}
      actor B {capacity 1;capability run()->V {duration fixed 1min;result nondet {ok,nope}}}
      process Before {r=A.run();if r == ok {end done} else {end other}}
      process After {r=B.run();if r == ok {end done} else {end other}}
      cut C {include [A,B]}
      configuration From {cut C;process Before}
      configuration To {cut C;process After}
      bounds {cases 1;live_tasks 1;clock 5min}
      simulation replications 4 seed 7 confidence 0.875
      verify probability reached end.done <= 0.1
    }"#;
    let value = json!(compare::compare(
        source,
        &["From", "To"],
        compare::Analysis::Simulation
    ));
    assert_eq!(value["verdict"], "violated", "{}", value["diagnostics"]);
    let a = &value["plans"][0]["report"];
    let b = &value["plans"][1]["report"];
    assert_eq!(a["verdict"], "violated");
    assert_eq!(b["verdict"], "unsupported");
    assert!(
        b["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0")
    );
    let property = a["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "probability reached #1")
        .unwrap();
    let row = &property["cases"][0];
    assert_eq!(row["basis"], "statistical");
    assert_eq!(row["hits"], 4);
    assert_eq!(row["unknown"], 0);
    assert_eq!(row["trials"], 4);
    assert_eq!(row["interval"]["lower"], "1/2");
    assert_eq!(row["interval"]["upper"], "1");
    let transported = &value["tables"]["properties"][0]["values"][0];
    for key in ["basis", "hits", "unknown", "trials", "interval", "verdict"] {
        assert_eq!(transported[key], row[key], "{key}");
    }
}
