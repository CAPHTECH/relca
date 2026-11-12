//! Hand-derived close-out inputs; see resource_calendars_close.md.
#![allow(dead_code)]
use serde_json::{Value, json};

pub fn contest(wait: bool, restricted: bool, bindings: usize, effect: bool) -> String {
    assert!(bindings >= 1);
    let calls = (0..bindings)
        .map(|i| format!("v{i} = A.judge();\n"))
        .collect::<String>();
    let waiting = if wait { "wait until v0 == pass;" } else { "" };
    let restriction = if restricted { "expires 100min;" } else { "" };
    let target = if effect { "P.post.commit" } else { "end.done" };
    format!(
        r#"model CloseContest version "0.2" {{
      type V = enum {{ pass, fail }}
      people [p, cfo]
      actor A {{ executors [p]; capability judge() -> V {{ duration fixed 1min; result nondet {{ pass }} }} }}
      actor P {{ capacity 1; capability post() -> V {{ commits; duration fixed 1min; result nondet {{ pass }} }} }}
      actor Clerk {{ capacity 1; capability recheck() -> V {{ duration fixed 1min; result nondet {{ pass }} }} }}
      actor Vendor {{ control party; standing over [{target}] basis "claim" }}
      authority Grant {{ basis "grant"; grants p to A.judge; {restriction} issued_by cfo }}
      process Flow {{ {calls} {waiting} if v0 == pass {{ posted = P.post(); end done }} else {{ end rejected }} }}
      cut C {{ include [A, P, Clerk, Vendor] }}
      contract Controls {{ contest Vendor on {target} route Clerk.recheck reversible_by [p] respond_by 5min }}
      bounds {{ cases 1; live_tasks 1; clock 10min }}
      verify safety no_resource_overcommit
    }}"#
    )
}

pub fn diff(wait_from: bool, wait_to: bool) -> String {
    let mut source = include_str!("../../../../fixtures/diff2-override/model.relca").to_string();
    if wait_from {
        source = source.replace(
            "process Before { seed = A.seed(); g = A.g(seed);",
            "process Before { seed = A.seed(); g = A.g(seed); wait until g == pass;",
        );
    }
    if wait_to {
        source = source.replace(
            "process After { seed = A.seed(); g = A.g(seed);",
            "process After { seed = A.seed(); g = A.g(seed); wait until g == pass;",
        );
    }
    source
}

pub fn mixed(shape: usize, condition: bool) -> String {
    let wait = if condition {
        "wait until approval == pass"
    } else {
        "wait 0min"
    };
    let flow = match shape {
        0 => format!(
            "if approval == pass {{ }} else {{ end returned }}; before = Ledger.post(); {wait}; posted = Ledger.post(); end done"
        ),
        1 => format!(
            "{wait}; if approval == pass {{ posted = Ledger.post(); end done }} else {{ end returned }}"
        ),
        2 => format!(
            "if approval == pass {{ {wait} }} else {{ end returned }}; posted = Ledger.post(); end done"
        ),
        _ => panic!("unknown shape"),
    };
    super::review::CONDITION.replace(
        "wait until approval == pass\n    posted = Ledger.post()\n    end done",
        &flow,
    )
}

pub fn mixed_assert(value: &Value, shape: usize, condition: bool) {
    assert_eq!(
        value["verdict"],
        if shape == 1 && condition {
            "violated"
        } else {
            "satisfied"
        }
    );
    let row = value["responsibilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["target"]["value"] == "Approver.approve")
        .unwrap();
    if condition {
        assert_eq!(row["refusal"]["status"], "not_derived");
        assert_eq!(row["refusal"]["reason"], "condition_wait_control");
        assert_eq!(row["refusal"]["effects"], json!([]));
    } else {
        assert_eq!(row["refusal"]["status"], "present");
        assert_eq!(row["refusal"]["reason"], Value::Null);
        assert_eq!(
            row["refusal"]["effects"].as_array().unwrap().len(),
            if shape == 0 { 2 } else { 1 }
        );
    }
}

pub fn refused(value: &Value, reason: &str) {
    assert_eq!(value["verdict"], "unsupported", "{value}");
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0" && d["message"].as_str().unwrap().contains(reason)),
        "{value}"
    );
}
