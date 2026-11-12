//! Public first-bad-state detail and independent replay acceptance.
#[path = "support/state_invariant_acceptance.rs"]
mod h;
use relca_core::explain;
use serde_json::{Value, json};
fn explain(src: &str) -> Value {
    assert!(!src.contains("configuration"));
    serde_json::to_value(explain::explain(src)).unwrap()
}
fn location(s: &str, start: usize, end: usize) -> Value {
    let pos = |n| {
        let a = &s[..n];
        (
            a.bytes().filter(|b| *b == b'\n').count() + 1,
            a.rsplit('\n').next().unwrap().chars().count() + 1,
        )
    };
    let (line, column) = pos(start);
    let (end_line, end_column) = pos(end);
    json!({"span":{"start":start,"end":end},"line":line,"column":column,"end_line":end_line,"end_column":end_column})
}
fn fragment(s: &str, text: &str) -> Value {
    let a = s.find(text).unwrap();
    location(s, a, a + text.len())
}
fn check_locations(s: &str, v: &Value) {
    match v {
        Value::Object(o) => {
            if o.contains_key("end_column") {
                let a = v["span"]["start"].as_u64().unwrap() as usize;
                let b = v["span"]["end"].as_u64().unwrap() as usize;
                assert!(a <= b && b <= s.len());
                assert_eq!(*v, location(s, a, b));
            }
            for v in o.values() {
                check_locations(s, v);
            }
        }
        Value::Array(a) => {
            for v in a {
                check_locations(s, v)
            }
        }
        _ => {}
    }
}
fn counterexample(v: &Value) -> &Value {
    assert_eq!(v["verdict"], "violated");
    assert_eq!(v["explanation"]["status"], "complete");
    let c = v["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "contract Controls #1 invariant")
        .unwrap();
    assert_eq!(c["replay"], json!({"status":"verified","reason":null}));
    assert_eq!(c["status"], "verified");
    c
}
#[test]
fn initial_shortest_has_zero_transitions_and_complete_read_detail() {
    let s = h::source("sinv-initial.broken-initial");
    let v = explain(&s);
    let c = counterexample(&v);
    check_locations(&s, &v["explanation"]);
    assert_eq!(c["transition_count"], 0);
    let steps = c["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1);
    let stage = &steps[0];
    assert_eq!(stage["index"], 0);
    assert_eq!(stage["time"], 0);
    assert_eq!(stage["case_id"], 1);
    let violation = &stage["violation"];
    assert_eq!(violation["at_step"], 0);
    assert!(violation["trigger_step"].is_null());
    assert!(violation["deadline"].is_null());
    assert_eq!(
        violation["property_location"],
        fragment(&s, "invariant ok == true")
    );
    assert_eq!(
        violation["state_invariant"],
        json!({"case_id":1,"antecedent":true,"condition":false,"values":[{"name":"ok","value":"false","visibility":"observed","location":fragment(&s,"ok: bool = false") }]})
    );
    assert!(
        c["related_locations"]
            .as_array()
            .unwrap()
            .contains(&fragment(&s, "ok: bool = false"))
    );
}
#[test]
fn first_bad_finish_and_start_report_exact_values_and_assignment_locations() {
    for (name, time, kind, names, values, write) in [
        (
            "sinv-approval.broken-bypass",
            3,
            "finish",
            vec!["registered", "approval"],
            vec!["true", "fail"],
            "registered = true",
        ),
        (
            "sinv-rewrite.broken-reapprove",
            5,
            "finish",
            vec!["registered", "approval"],
            vec!["true", "fail"],
            "writes approval",
        ),
        (
            "sinv-start-effect.broken-order",
            1,
            "start",
            vec!["phase", "registered"],
            vec!["running", "true"],
            "phase = running",
        ),
        (
            "sinv-transient.broken-same-time",
            3,
            "finish",
            vec!["po", "receipt"],
            vec!["pass", "none"],
            "writes po",
        ),
        (
            "sinv-sticky.broken-bad",
            1,
            "finish",
            vec!["ok"],
            vec!["false"],
            "ok = false",
        ),
    ] {
        let s = h::source(name);
        let v = explain(&s);
        let c = counterexample(&v);
        check_locations(&s, &v["explanation"]);
        let steps = c["steps"].as_array().unwrap();
        let last = steps.last().unwrap();
        assert_eq!(last["time"], time);
        assert_eq!(last["kind"], kind);
        assert!(
            steps[..steps.len() - 1]
                .iter()
                .all(|s| s["violation"].is_null()),
            "shortest prefix stops at first bad state"
        );
        let d = &last["violation"]["state_invariant"];
        assert_eq!(d["case_id"], 1);
        assert_eq!(d["antecedent"], true);
        assert_eq!(d["condition"], false);
        let rows = d["values"].as_array().unwrap();
        assert_eq!(rows.len(), names.len());
        for ((r, n), value) in rows.iter().zip(names).zip(values) {
            assert_eq!(r["name"], n);
            assert_eq!(r["value"], value);
            assert_eq!(r["visibility"], "observed");
        }
        // writes targets map to the identifier, assignment effects to the exact
        // assignment. Never accept the whole capability as a guessed span.
        let text = write.strip_prefix("writes ").unwrap_or(write);
        let start = s.find(write).unwrap() + if write.starts_with("writes ") { 7 } else { 0 };
        assert!(
            c["related_locations"]
                .as_array()
                .unwrap()
                .contains(&location(&s, start, start + text.len())),
            "exact final write {write}"
        );
    }
}
#[test]
fn cross_case_target_is_ended_reader_not_the_writing_stage_case() {
    let s = h::source("sinv-ended-clean.broken-increment");
    let v = explain(&s);
    let c = counterexample(&v);
    check_locations(&s, &v["explanation"]);
    let steps = c["steps"].as_array().unwrap();
    let last = steps.last().unwrap();
    assert_eq!(last["time"], 3);
    assert_eq!(last["kind"], "finish");
    assert_ne!(
        c["case_id"], last["case_id"],
        "target is the old fail case; stage belongs to pass writer"
    );
    assert!(
        steps
            .iter()
            .any(|s| s["case_id"] == c["case_id"] && s["time"] == 2 && s["kind"] == "end")
    );
    let d = &last["violation"]["state_invariant"];
    assert_eq!(d["case_id"], c["case_id"]);
    assert_eq!(d["antecedent"], true);
    assert_eq!(d["condition"], false);
    let vs = d["values"].as_array().unwrap();
    assert_eq!(vs.len(), 2);
    assert_eq!(vs[0]["name"], "approval");
    assert_eq!(vs[0]["value"], "fail");
    assert_eq!(vs[1]["name"], "Ledger.posted");
    assert_eq!(vs[1]["value"], "1");
    let own = last["values"]["observed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "approval")
        .unwrap();
    assert_eq!(own["value"], "pass", "stage values retain writer semantics");
}
#[test]
fn short_circuit_read_union_is_syntactic_deduplicated_and_declaration_ordered() {
    let s = h::source("sinv-approval.broken-bypass");
    let t = h::change(
        &s,
        "registered -> approval == pass",
        "registered -> approval == pass || (registered == false && approval == pass)",
    );
    let v = explain(&t);
    let c = counterexample(&v);
    let rows =
        c["steps"].as_array().unwrap().last().unwrap()["violation"]["state_invariant"]["values"]
            .as_array()
            .unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["registered", "approval"]
    );
}
#[test]
fn qualified_write_location_matches_actor_and_variable_and_related_steps_name_the_bad_state() {
    let s = r#"model M version "0.2" {
  type R = enum { ok, bad }
  actor A { capacity 1; state x: R = ok; state y: R = ok; capability run() -> R { duration fixed 1min; result nondet { ok }; writes [A.y, A.x]; finish { A.y = ok; A.x = bad } } }
  process P { r = A.run(); end done }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 3min }
  contract Controls { invariant A.x == ok }
}
"#;
    let v = explain(s);
    let c = counterexample(&v);
    check_locations(s, &v["explanation"]);
    // receipt0, queue1, start2, advance to 1min3, finish4: the result writes ok
    // to both, then the finish effect sets A.x = bad. Only A.x is read.
    assert_eq!(c["transition_count"], 4);
    let steps = c["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 5);
    let last = steps.last().unwrap();
    assert_eq!(
        (last["index"].clone(), last["kind"].clone()),
        (json!(4), json!("finish"))
    );
    assert_eq!(last["violation"]["at_step"], 4);
    assert_eq!(last["violation"]["related_steps"], json!([4]));
    assert_eq!(
        c["related_locations"],
        json!([fragment(s, "invariant A.x == ok"), fragment(s, "A.x = bad")]),
        "the final write to A.x, not the same Actor's A.y = ok"
    );
}
