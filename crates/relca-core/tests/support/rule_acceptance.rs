#![allow(dead_code)]
use relca_core::{diff, explain, lower, report, syntax};
use serde_json::{Value, json};
use std::path::PathBuf;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn names() -> Vec<String> {
    serde_json::from_str(include_str!("rule_names.json")).unwrap()
}
pub fn rows() -> Vec<Value> {
    serde_json::from_str(include_str!("rule_oracles.json")).unwrap()
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join("fixtures").join(name).join("model.relca")).unwrap()
}
pub fn row(name: &str) -> Value {
    rows().into_iter().find(|r| r["name"] == name).unwrap()
}
pub fn replace(src: &str, from: &str, to: &str) -> String {
    let result = src.replace(from, to);
    assert_ne!(src, result, "control substitution must occur: {from}");
    result
}
pub fn actual(src: &str, invocation: &Value) -> Value {
    match invocation["command"].as_str().unwrap() {
        "diff" => serde_json::to_value(diff::diff(
            src,
            invocation["from"].as_str().unwrap(),
            invocation["to"].as_str().unwrap(),
        ))
        .unwrap(),
        "explain" => serde_json::to_value(explain::explain(src)).unwrap(),
        "check" => match invocation["configuration"].as_str() {
            Some(c) => serde_json::to_value(report::check_configuration(src, c)).unwrap(),
            None => serde_json::to_value(report::check(src)).unwrap(),
        },
        x => panic!("unowned invocation {x}"),
    }
}
pub fn lowered(src: &str, invocation: &Value) -> relca_core::ir::Model {
    let ast = syntax::parse(src).unwrap_or_else(|e| panic!("new syntax must parse: {e:?}"));
    match invocation["configuration"].as_str() {
        Some(c) => {
            lower::lower_configuration(&ast, c)
                .unwrap_or_else(|e| panic!("new Rule must lower: {e:?}"))
                .model
        }
        None => lower::lower(&ast).unwrap_or_else(|e| panic!("new Rule must lower: {e:?}")),
    }
}

/// Partial objects, exact arrays (terminal inventory is keyed by end).
/// This is independent of fixture_match and production serialization types.
pub fn subset(expected: &Value, actual: &Value) {
    match expected {
        Value::Object(fields) => {
            for (k, e) in fields {
                if k == "diagnostics" {
                    for d in e.as_array().unwrap() {
                        assert!(
                            actual[k]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|a| a["code"] == d["code"] && a["severity"] == d["severity"]),
                            "missing {d}: {actual}"
                        );
                    }
                } else if k == "terminals" {
                    let es = e.as_array().unwrap();
                    let ac = actual[k].as_array().unwrap();
                    assert_eq!(es.len(), ac.len(), "terminal population {actual}");
                    for t in es {
                        subset(t, ac.iter().find(|a| a["end"] == t["end"]).unwrap());
                    }
                } else {
                    subset(
                        e,
                        actual
                            .get(k)
                            .unwrap_or_else(|| panic!("missing {k}: {actual}")),
                    );
                }
            }
        }
        Value::Array(es) => {
            let ac = actual
                .as_array()
                .unwrap_or_else(|| panic!("not array: {actual}"));
            assert_eq!(es.len(), ac.len(), "array inventory");
            for (e, a) in es.iter().zip(ac) {
                subset(e, a);
            }
        }
        _ => assert_eq!(expected, actual, "independent hand oracle"),
    }
}
pub fn count(v: &Value, code: &str) -> usize {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == code)
        .count()
}
pub fn rule_item(actor: &str, version: &str, reads: &[&str]) -> Value {
    json!({"kind":"rule","actor":actor,"version":version,"reads":reads})
}
pub fn actor<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["actors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == name)
        .unwrap()
}
pub fn assert_profile(v: &Value, profile: &str, actor_name: &str) {
    let amended = profile == "B";
    let version = if amended { "r2" } else { "r1" };
    let r = &actor(v, actor_name)["rule"];
    assert_eq!(
        actor(v, actor_name)["state"]["value"],
        Value::Null,
        "decides is not an ActorObserved state field"
    );
    assert_eq!(r["version"], json!({"declared":"r1","effective":version}));
    assert_eq!(
        r["values"],
        json!([{"name":"approval_required","declared":true,"effective":!amended}])
    );
    assert_eq!(r["read_by"], json!(["Ledger.post.requires"]));
    if amended {
        assert_eq!(
            r["amendment"],
            json!({"from":"r1","to":"r2","by":"cfo","under":"RuleChange","under_version":null})
        );
    } else {
        assert_eq!(r["amendment"], Value::Null);
    }
    assert!(
        !actor(v, "Approver")
            .as_object()
            .unwrap()
            .contains_key("rule")
    );
    assert!(!actor(v, "Ledger").as_object().unwrap().contains_key("rule"));
    assert_eq!(v["agency"]["rules"], "agency-p1-rule-1");
    assert!(
        !v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guards"))
    );
    let refusal = if amended {
        json!([])
    } else {
        json!([{"effect":"Ledger.post","call":"ack","reachable":true,"judgements":[rule_item(actor_name,version,&[&format!("{actor_name}.approval_required")])]}])
    };
    assert_eq!(v["agency"]["refusal"], refusal);
    let assumptions = v["assumptions"].as_array().unwrap();
    assert!(assumptions.iter().any(|a| a == &json!({"field":format!("{actor_name}.rule"),"value":version,"affects":"rule values are fixed for the whole run; in-run amendment and migration are not modeled"})));
    if amended {
        assert!(
            assumptions.iter().any(
                |a| a["field"] == format!("{actor_name}.amendment") && a["value"] == "r1 -> r2"
            )
        );
    }
}
pub fn verify(name: &str, v: &Value) {
    let r = row(name);
    subset(&r["expected"], v);
    let o = &r["oracle"];
    if let Some(sides) = o["side_expected"].as_object() {
        for (side, hand) in sides {
            subset(hand, &v[side]["report"]);
        }
        if o["reverse"] != true {
            let rules: Vec<_> = v["differences"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["kind"] == "rule")
                .collect();
            assert_eq!(
                rules.len(),
                1,
                "including version-only/declared-forward changes"
            );
            assert_eq!(rules[0]["change"], "modified");
            assert_eq!(
                rules[0]["to"]["amendment"]["from"],
                rules[0]["from"]["version"]
            );
            assert_eq!(rules[0]["to"]["amendment"]["to"], rules[0]["to"]["version"]);
        }
    }
    if let Some(code) = o["error_code"].as_str() {
        let v = if o["error_in_sides"] == true {
            &v["from"]["report"]
        } else {
            v
        };
        let errors: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(
            errors.len(),
            o["error_count"].as_u64().unwrap() as usize,
            "{name}: {v}"
        );
        assert!(errors.iter().all(|d| d["code"] == code));
        if let Some(msg) = o["error_message"].as_str() {
            assert_eq!(errors[0]["message"], msg, "{name}");
        }
        assert_eq!(
            v["actors"],
            Value::Null,
            "static refusal cannot invent Actor analysis"
        );
        assert_eq!(v["agency"], Value::Null);
        if o["parser"] == true {
            assert_eq!(count(v, "W-CUT-0"), 0);
            assert_eq!(v["cut"], "unresolved");
        }
    }
    if let Some(profile) = o["profile"].as_str() {
        assert_profile(
            v,
            profile,
            o["rule_actor"].as_str().unwrap_or("ApprovalRule"),
        );
    }
    if o["mutable"] == true {
        assert_eq!(v["agency"]["refusal"], json!([]));
        assert!(
            v["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("state_guards"))
        );
        assert!(
            !actor(v, "ApprovalRule")
                .as_object()
                .unwrap()
                .contains_key("rule")
        );
    }
    if let Some(n) = o["auth_warnings"].as_u64() {
        assert_eq!(count(v, "W-AUTH-0"), n as usize);
    }
    if let Some(person) = o["unused_future"].as_str() {
        let rows: Vec<_> = v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["field"] == format!("authority.Future.change.ApprovalRule.{person}"))
            .collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["value"], "unused_in_selected_configuration");
        assert_eq!(
            rows[0]["affects"],
            "declared rule-change grant is not used by the selected configuration; no enactment or exercise is inferred"
        );
    }
    if o["reverse"] == true {
        assert_eq!(count(v, "E-CON-0"), 1);
        assert!(
            !v["differences"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["kind"] == "rule")
        );
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["message"] == "rule version change has no declared amendment")
        );
    }
    if o["diff"] == true || o.get("preserve").is_some() {
        let ds = v["differences"].as_array().unwrap();
        assert_eq!(ds.len(), 1, "no old eight-kind rows for identical process");
        assert_eq!(ds[0]["kind"], "rule");
        assert_eq!(ds[0]["change"], "modified");
        assert_eq!(ds[0]["target"], "ApprovalRule");
        assert_eq!(
            v["checked"].as_array().unwrap().last(),
            Some(&json!("rule")),
            "Rule comparison is explicitly checked after legacy kinds"
        );
        assert!(!v["unchecked"].as_array().unwrap().contains(&json!("rule")));
        assert_eq!(
            ds[0]["from"],
            json!({"version":"r1","values":[{"name":"approval_required","value":true}],"amendment":null})
        );
        assert_eq!(ds[0]["to"]["version"], "r2");
        if o["diff"] == true {
            assert_eq!(
                ds[0]["to"]["values"],
                json!([{"name":"approval_required","value":false}])
            );
            assert_profile(&v["from"]["report"], "A", "ApprovalRule");
            assert_profile(&v["to"]["report"], "B", "ApprovalRule");
        }
        assert_eq!(
            ds[0]["to"]["amendment"],
            json!({"from":"r1","to":"r2","by":"cfo","under":"RuleChange","under_version":null})
        );
        if let Some(n) = o["w1"].as_u64() {
            assert_eq!(count(v, "W-DIFF-1"), n as usize);
        }
        if let Some(n) = o["w3"].as_u64() {
            assert_eq!(count(v, "W-DIFF-3"), n as usize);
        }
        if let Some(bin) = o["preserve"].as_str() {
            let annotation = ds[0]["annotations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["kind"] == "preserve")
                .unwrap();
            assert_eq!(annotation["selector"], "K");
            let summary = &v["preservation"]["summary"];
            for b in [
                "satisfied",
                "vacuous",
                "violated",
                "inconclusive",
                "unsupported",
                "invalid",
            ] {
                assert_eq!(summary[b], usize::from(b == bin), "preservation bin {b}");
            }
        }
    }
    if let Some(mode) = o["partial"].as_str() {
        assert_eq!(
            actor(v, "ApprovalRule")["rule"]["values"][1],
            json!({"name":"mode","declared":mode,"effective":mode})
        );
    }
    if let Some(violated) = o["explain"].as_bool() {
        assert_eq!(v["explanation"]["status"], "complete");
        let cs = v["explanation"]["counterexamples"].as_array().unwrap();
        assert_eq!(cs.len(), usize::from(violated));
        if violated {
            let c = &cs[0];
            assert_eq!(c["status"], "verified");
            assert_eq!(c["replay"]["status"], "verified");
            assert_eq!(c["id"], "contract K #1 invariant");
            let steps = c["steps"].as_array().unwrap();
            assert!(steps.iter().any(|s| {
                s["time"] == 2
                    && s["events"].as_array().unwrap().iter().any(|e| {
                        e["kind"] == "finish" && e["actor"] == "Approver" && e["result"] == "fail"
                    })
            }));
            assert!(steps.iter().any(|s| {
                s["time"] == 3
                    && s["events"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|e| e["kind"] == "commit" && e["actor"] == "Ledger")
            }));
            assert!(steps.iter().any(|s| {
                s["values"]["observed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|x| x["name"] == "ApprovalRule.approval_required" && x["value"] == "false")
            }));
        }
    }
    if o["maintain"] == true {
        assert_eq!(count(v, "W-DIFF-3"), 0);
        let d = v["differences"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["kind"] == "rule")
            .unwrap();
        let a = d["annotations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["kind"] == "maintain")
            .unwrap();
        assert_eq!(
            a["links"],
            json!([{"actor":"ApprovalRule","from_duties":[],"to_duties":[],"status":"unlinked"}])
        );
    }
    if let Some(fifo) = o["cases"].as_str() {
        verify_case_times(v, fifo);
    }
}

pub fn verify_case_times(v: &Value, fifo: &str) {
    assert_eq!(v["case_outcomes"].as_array().unwrap().len(), 2);
    for (i, c) in v["case_outcomes"].as_array().unwrap().iter().enumerate() {
        for (end, base, work) in [("posted", 3, 3), ("held", 2, 2)] {
            let t = c["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["end"] == end)
                .unwrap();
            let (lo, hi) = if fifo == "fifo" {
                (base + 2 * i, base + 2 * i)
            } else {
                (base, base + 2)
            };
            assert_eq!(t["elapsed"], json!({"min":lo,"max":hi}));
            assert_eq!(t["work"], json!({"min":work,"max":work}));
        }
    }
}
