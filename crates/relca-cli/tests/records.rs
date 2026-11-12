//! CLI crossings and fault-calibrated wire oracles for ADR0038.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/record_acceptance.rs"]
mod hand;
use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

fn run(args: &[&str]) -> (i32, Value) {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(hand::root())
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stdout))),
    )
}
fn scratch(s: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let root = hand::root().join("target");
    assert!(root.is_dir());
    let dir = root.join(format!("record-acceptance-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{}.relca", N.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&p, s).unwrap();
    p
}
#[test]
fn all_record_fixture_invocations_have_expected_exit_and_closed_wire() {
    let mut failures = Vec::new();
    for name in hand::names() {
        let command = if name.starts_with("rec-explain") {
            "explain"
        } else {
            "check"
        };
        let path = format!("fixtures/{name}/model.relca");
        let (exit, v) = run(&[command, &path]);
        let e = hand::expected_for_source(&name, &hand::source(&name));
        let verdict: relca_core::Verdict = serde_json::from_value(e["verdict"].clone()).unwrap();
        if v["verdict"] != e["verdict"] || exit != verdict.exit_code() {
            failures.push(format!(
                "{name}: expected {} exit{}, got {} exit{exit}",
                e["verdict"],
                verdict.exit_code(),
                v["verdict"]
            ));
            continue;
        }
        hand::report_oracle(&name, &v);
        let iv = json!({"command":command});
        let invocation =
            fixture_match::Invocation::parse(if command == "check" { None } else { Some(&iv) })
                .unwrap();
        invocation.compare(&e, &v).unwrap();
    }
    assert!(
        failures.is_empty(),
        "record/inline enum not implemented:\n{}",
        failures.join("\n")
    );
}

#[test]
fn all146_expectations_are_well_typed_partial_reports_not_wire_echoes() {
    let names = hand::names();
    assert_eq!(names.len(), 146);
    assert_eq!(
        fixture_match::RECORD_FIXTURES,
        names.iter().map(String::as_str).collect::<Vec<_>>()
    );
    for name in names {
        let e = hand::expected(&name);
        let path = hand::root()
            .join("fixtures")
            .join(&name)
            .join("invocation.json");
        let inv: Option<Value> = path
            .exists()
            .then(|| serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap());
        let invocation = fixture_match::Invocation::parse(inv.as_ref()).unwrap();
        invocation
            .validate_expected(&e)
            .unwrap_or_else(|error| panic!("{name}: malformed authored expectation: {error}"));
        assert!(
            invocation.compare(&json!({}), &e).is_err(),
            "partial expected must not validate as a full Report: {name}"
        );
    }
}

fn witness(v: &Value) -> bool {
    let Some(rows) = v["explanation"]["counterexamples"].as_array() else {
        return false;
    };
    if v["verdict"] != "violated" || v["explanation"]["status"] != "complete" || rows.len() != 1 {
        return false;
    }
    let c = &rows[0];
    if c["replay"]["status"] != "verified" {
        return false;
    }
    let Some(steps) = c["steps"].as_array() else {
        return false;
    };
    let Some(last) = steps.last() else {
        return false;
    };
    let Some(values) = last["values"]["observed"].as_array() else {
        return false;
    };
    let value = |name: &str, want: &str| {
        values
            .iter()
            .any(|r| r["name"] == name && r["value"].as_str() == Some(want))
    };
    last["time"] == 2
        && value("entry", "Entry { confidence = high }")
        && value("approval", "none")
        && value("registered", "true")
        && steps.iter().any(|s| {
            s["time"] == 2
                && s["events"].as_array().is_some_and(|es| {
                    es.iter()
                        .any(|e| e["kind"] == "commit" && e["actor"] == "Ledger")
                })
        })
}
#[test]
fn t10_display_projection_rejects_numbers_partial_records_and_wrong_time_or_replay() {
    let v = json!({"verdict":"violated","explanation":{"status":"complete","counterexamples":[{"replay":{"status":"verified"},"steps":[{"time":2,"events":[{"kind":"commit","actor":"Ledger"}],"values":{"observed":[{"name":"entry","value":"Entry { confidence = high }"},{"name":"approval","value":"none"},{"name":"registered","value":"true"}]}}]}]}});
    assert!(
        witness(&v),
        "synthetic projection control, not a full wire Report"
    );
    for value in [
        json!(0),
        json!("0"),
        json!("high"),
        json!({"confidence":"high"}),
    ] {
        let mut bad = v.clone();
        bad["explanation"]["counterexamples"][0]["steps"][0]["values"]["observed"][0]["value"] =
            value;
        assert!(!witness(&bad));
    }
    let mut bad = v.clone();
    bad["explanation"]["counterexamples"][0]["steps"][0]["time"] = json!(3);
    assert!(!witness(&bad));
    let mut bad = v.clone();
    bad["explanation"]["counterexamples"][0]["replay"]["status"] = json!("unsupported");
    assert!(!witness(&bad));
}
fn locations(v: &Value, source: &str) {
    match v {
        Value::Object(o) => {
            if o.contains_key("span") && o.contains_key("line") {
                let a = v["span"]["start"].as_u64().unwrap() as usize;
                let b = v["span"]["end"].as_u64().unwrap() as usize;
                assert!(a <= b && b <= source.len());
                let at = |n: usize| {
                    let before = &source[..n];
                    (
                        before.bytes().filter(|b| *b == b'\n').count() + 1,
                        before.rsplit('\n').next().unwrap().chars().count() + 1,
                    )
                };
                let (line, column) = at(a);
                let (end_line, end_column) = at(b);
                assert_eq!(
                    (
                        v["line"].as_u64().unwrap(),
                        v["column"].as_u64().unwrap(),
                        v["end_line"].as_u64().unwrap(),
                        v["end_column"].as_u64().unwrap()
                    ),
                    (
                        line as u64,
                        column as u64,
                        end_line as u64,
                        end_column as u64
                    )
                );
            }
            for v in o.values() {
                locations(v, source);
            }
        }
        Value::Array(xs) => {
            for v in xs {
                locations(v, source);
            }
        }
        _ => {}
    }
}
#[test]
fn t10_shortest_high_bypass_has_canonical_string_values_and_verified_replay() {
    let name = "rec-explain.broken-bypass";
    let (exit, v) = run(&["explain", &format!("fixtures/{name}/model.relca")]);
    assert_eq!(exit, 1);
    assert!(witness(&v), "canonical record witness: {v}");
    locations(&v, &hand::source(name));
    let mut bad = v.clone();
    let steps = bad["explanation"]["counterexamples"][0]["steps"]
        .as_array_mut()
        .unwrap();
    let last = steps.last_mut().unwrap();
    let entry = last["values"]["observed"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["name"] == "entry")
        .unwrap();
    entry["value"] = json!(0);
    assert!(
        !witness(&bad),
        "numeric ValueId display must be rejected even when verdict/time unchanged"
    );
}

#[test]
fn t19_refused_record_literal_is_binding_only_and_never_a_case_write() {
    let s = hand::source("rec-record-refusal");
    let s = hand::changed(
        &s,
        "process Flow",
        "actor Probe { capacity 1; capability run() -> ProbeAck { duration fixed 1min; result nondet { done } } }\ntype ProbeAck = enum { done }\ncontract C { invariant Probe.run.finish -> entry.confidence == high }\nprocess Flow",
    );
    let s = hand::changed(&s, "e = Work.run();", "e = Work.run(); p = Probe.run();");
    let p = scratch(&s);
    let (exit, v) = run(&["explain", p.to_str().unwrap()]);
    assert_eq!(exit, 1);
    assert_eq!(v["capability_refusals"].as_array().unwrap().len(), 1);
    assert_eq!(
        v["capability_refusals"][0]["result"],
        "Entry { confidence = low }"
    );
    assert_eq!(v["capability_refusals"][0]["at"], json!({"min":0,"max":0}));
    let c = &v["explanation"]["counterexamples"][0];
    assert_eq!(c["replay"]["status"], "verified");
    let steps = c["steps"].as_array().unwrap();
    let values = steps.last().unwrap()["values"]["observed"]
        .as_array()
        .unwrap();
    assert!(
        values
            .iter()
            .any(|r| r["name"] == "entry" && r["value"] == "none")
    );
    assert!(!steps.iter().any(|s| {
        s["actor"] == "Work"
            && ["start", "finish", "commit"]
                .iter()
                .any(|k| s["kind"] == *k)
    }));
}

#[test]
fn record_kernel_simulation_is_sample_not_proof_and_preserves_draw_identity() {
    let s = hand::source("rec-prob-field");
    let s = hand::changed(&s, ">= 0.75", ">= 0");
    let s = hand::changed(
        &s,
        "verify safety",
        "simulation replications 24 seed 19 confidence 0.95\nverify safety",
    );
    let p = scratch(&s);
    let (exit, v) = run(&["simulate", p.to_str().unwrap()]);
    assert_eq!(exit, 2, "sampled safety is not proof");
    assert_eq!(v["mode"], "simulation");
    assert_eq!(v["simulation"]["completed_replications"], 24);
    assert_eq!(v["simulation"]["truncated_replications"], 0);
    let a = relca_core::engine::simulate(&hand::model(&s));
    assert_eq!(a.completed_replications, 24);
    assert!(
        a.replications
            .iter()
            .all(|r| r.cases.iter().all(|c| c.terminal.is_some()))
    );
    let swapped = hand::changed(
        &s,
        "Pair { flag = false, color = green }",
        "Pair { color = green, flag = false }",
    );
    let b = relca_core::engine::simulate(&hand::model(&swapped));
    let draws = |o: &relca_core::outcome::SimulationOutcome| {
        o.replications
            .iter()
            .map(|r| r.draws.iter().map(|d| d.key.clone()).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        draws(&a),
        draws(&b),
        "literal field order must not enter sampling keys"
    );
}

fn comparison_source() -> String {
    String::from(
        r#"model Plans version "0.2" {
      type Entry=record { confidence: enum {high,low} }
      actor Work {capacity 1; capability run()->Entry {duration fixed 1min; result nondet {Entry {confidence=high}}} }
      process A {a=Work.run(); if a.confidence==high {end done}else{end done}}
      process B {a=Work.run(); b=Work.run(); if b.confidence==high {end done}else{end done}}
      cut C {include [Work]}
      configuration A {cut C; process A}
      configuration B {cut C; process B}
      bounds {cases 1;live_tasks 1;clock 4min}
      verify safety no_resource_overcommit
    }"#,
    )
}

fn label_byte_map(base: &str, label: &str) -> Vec<usize> {
    let edits = [
        ("actor Extractor {", "actor Extractor { grain model;"),
        ("Entry", "Slip"),
        ("confidence", "grade"),
    ];
    let mut rebuilt = String::new();
    let mut map = vec![0];
    let mut i = 0;
    while i < base.len() {
        if let Some((from, to)) = edits.iter().find(|(from, _)| base[i..].starts_with(from)) {
            rebuilt.push_str(to);
            for k in 1..=to.len() {
                map.push(i + k * from.len() / to.len());
            }
            i += from.len();
        } else {
            assert!(base.is_char_boundary(i));
            let c = base[i..].chars().next().unwrap();
            rebuilt.push(c);
            for k in 1..=c.len_utf8() {
                map.push(i + k);
            }
            i += c.len_utf8();
        }
    }
    assert_eq!(
        rebuilt, label,
        "byte map derives only from declared source edits"
    );
    assert_eq!(map.len(), label.len() + 1);
    map
}
fn sha(bytes: &[u8]) -> String {
    let mut c = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(bytes).unwrap();
    let o = c.wait_with_output().unwrap();
    assert!(o.status.success());
    String::from_utf8(o.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}
fn map_label(v: &mut Value, map: &[usize], base: &str) {
    match v {
        Value::Object(o) => {
            if let Some(span) = o.get_mut("span").filter(|s| s.is_object()) {
                for key in ["start", "end"] {
                    let n = span[key].as_u64().unwrap() as usize;
                    span[key] = json!(map[n]);
                }
            }
            if o.contains_key("span") && o.contains_key("line") {
                let a = o["span"]["start"].as_u64().unwrap() as usize;
                let b = o["span"]["end"].as_u64().unwrap() as usize;
                for (n, line, col) in [(a, "line", "column"), (b, "end_line", "end_column")] {
                    let before = &base[..n];
                    o.insert(
                        line.into(),
                        json!(before.bytes().filter(|b| *b == b'\n').count() + 1),
                    );
                    o.insert(
                        col.into(),
                        json!(before.rsplit('\n').next().unwrap().chars().count() + 1),
                    );
                }
            }
            for (k, value) in o.iter_mut() {
                if k != "span" {
                    map_label(value, map, base);
                }
            }
        }
        Value::Array(xs) => {
            for value in xs {
                map_label(value, map, base);
            }
        }
        Value::String(s) => {
            if s == "Slip { grade = high }" {
                *s = "Entry { confidence = high }".into();
            } else if s == "Slip { grade = low }" {
                *s = "Entry { confidence = low }".into();
            }
        }
        _ => {}
    }
}
fn label_projection(mut label: Value, base: &Value, map: &[usize], source: &str) -> Value {
    map_label(&mut label, map, source);
    label["model"]["sha256"] = base["model"]["sha256"].clone();
    let actors = label["actors"].as_array_mut().unwrap();
    let actor = actors
        .iter_mut()
        .find(|a| a["name"] == "Extractor")
        .unwrap();
    assert_eq!(actor["grain"], json!({"value":"model","status":"declared"}));
    actor["grain"] = json!({"value":null,"status":"default"});
    label
}
#[test]
fn t09_label_normalizer_and_its_negative_controls_are_calibrated_on_legacy_scaffolds() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../relca-core/tests/support/record_legacy_skeletons.json"
    ))
    .unwrap();
    let source = cases["rec-approval"].as_str().unwrap();
    let renamed = cases["rec-label"].as_str().unwrap();
    let a = serde_json::to_value(relca_core::report::check(source)).unwrap();
    let b = serde_json::to_value(relca_core::report::check(renamed)).unwrap();
    assert_eq!(a["verdict"], "satisfied");
    assert_eq!(b["verdict"], "satisfied");
    let map = label_byte_map(source, renamed);
    assert_eq!(label_projection(b.clone(), &a, &map, source), a);
    for fault in ["time", "extra", "verdict"] {
        let mut bad = b.clone();
        match fault {
            "time" => bad["terminals"][0]["work"]["max"] = json!(99),
            "extra" => bad["extra"] = json!(1),
            _ => bad["verdict"] = json!("violated"),
        }
        assert_ne!(
            label_projection(bad, &a, &map, source),
            a,
            "{fault} is not an allowed projection"
        );
    }
}
#[test]
fn t09_full_report_label_comparison_only_maps_declared_names_grain_hash_and_bytes() {
    let (_, a) = run(&["check", "fixtures/rec-approval/model.relca"]);
    let (_, b) = run(&["check", "fixtures/rec-label/model.relca"]);
    assert_eq!(a["verdict"], "satisfied");
    assert_eq!(b["verdict"], "satisfied");
    assert_ne!(a["model"]["sha256"], b["model"]["sha256"]);
    let source = hand::source("rec-approval");
    let renamed = hand::source("rec-label");
    assert_eq!(a["model"]["sha256"], sha(source.as_bytes()));
    assert_eq!(b["model"]["sha256"], sha(renamed.as_bytes()));
    let map = label_byte_map(&source, &renamed);
    assert_eq!(
        label_projection(b.clone(), &a, &map, &source),
        a,
        "all report values and all unrelated keys remain"
    );
    for fault in ["time", "extra", "verdict"] {
        let mut bad = b.clone();
        match fault {
            "time" => bad["terminals"][0]["elapsed"]["min"] = json!(99),
            "extra" => bad["invented"] = json!(true),
            _ => bad["verdict"] = json!("violated"),
        }
        assert_ne!(
            label_projection(bad, &a, &map, &source),
            a,
            "normalizer must not erase {fault}"
        );
    }
}
#[test]
fn compare_and_diff_keep_record_lowering_in_every_side_report_without_new_keys() {
    let p = scratch(&comparison_source());
    let p = p.to_str().unwrap();
    let (exit, v) = run(&["compare", p, "--plans", "B,A", "--mode", "exhaustive"]);
    assert_eq!(exit, 0);
    assert_eq!(v["plans"].as_array().unwrap().len(), 2);
    for (i, name, time) in [(0, "B", 2), (1, "A", 1)] {
        assert_eq!(v["plans"][i]["configuration"], name);
        assert_eq!(
            v["plans"][i]["report"]["terminals"][0]["elapsed"],
            json!({"min":time,"max":time})
        );
        assert_eq!(
            v["plans"][i]["report"]["terminals"][0]["work"],
            json!({"min":time,"max":time})
        );
    }
    let (exit, v) = run(&["diff", p, "--from", "A", "--to", "B"]);
    assert_eq!(exit, 0);
    assert_eq!(v["from"]["report"]["verdict"], "satisfied");
    assert_eq!(v["to"]["report"]["verdict"], "satisfied");
    assert_eq!(
        v["from"]["report"]["terminals"][0]["elapsed"],
        json!({"min":1,"max":1})
    );
    assert_eq!(
        v["to"]["report"]["terminals"][0]["elapsed"],
        json!({"min":2,"max":2})
    );
    for side in ["from", "to"] {
        assert!(v[side]["report"].get("record_fields").is_none());
    }
}
