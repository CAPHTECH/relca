//! ADR 0072 inputs, written independently from the measured generator.
//! No new DSL: each shape already parses on the adoption base.
#![allow(dead_code)]
use serde_json::{Value, json};

pub fn model(body: &str, width: usize) -> String {
    let judges = (1..=width)
        .map(|i| format!("actor {} {{ capacity 1; capability check(x:V)->V {{ duration fixed 1min; result nondet {{ pass, fail }} }} }}", judge(i)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "model ViewpointLimit version \"0.2\" {{
type V = enum {{ pass, fail }}
type Ack = enum {{ done }}
actor Doc {{ source document }}
actor Intake {{ capacity 1; capability start()->V {{ reads [Doc]; duration fixed 1min; result nondet {{ pass }} }} }}
{judges}
actor Ledger {{ capacity 1; capability post(x:V)->Ack {{ commits; duration fixed 1min; result nondet {{ done }} }} }}
process Flow {{ {body} }}
bounds {{ cases 1; live_tasks 8; clock 200min }}
verify safety no_resource_overcommit
}}"
    )
}
pub fn judge(i: usize) -> String {
    if i == 1 {
        "Judge".into()
    } else {
        format!("J{i}")
    }
}
pub fn gate(k: usize) -> String {
    let mut body = "s=Intake.start();".to_owned();
    for i in 1..=k {
        body += &format!("j{i}=Judge.check(s); if j{i}==pass {{}} else {{end rejected}};");
    }
    body += &format!("p=Ledger.post(j{k}); end done");
    model(&body, 1)
}
pub fn joined(k: usize, op: &str) -> String {
    let calls = (1..=k)
        .map(|i| format!("j{i}=Judge.check(s);"))
        .collect::<String>();
    let condition = (1..=k)
        .map(|i| format!("j{i}==pass"))
        .collect::<Vec<_>>()
        .join(op);
    model(
        &format!(
            "s=Intake.start(); {calls} if {condition} {{}} else {{end rejected}}; p=Ledger.post(j{k}); end done"
        ),
        1,
    )
}
pub fn stage(stages: usize, width: usize, shape: &str) -> String {
    let mut body = "s=Intake.start();".to_owned();
    let mut previous = "s".to_owned();
    for s in 1..=stages {
        let calls = (1..=width)
            .map(|i| format!("a{s}_{i}={}.check({previous});", judge(i)))
            .collect::<String>();
        if shape == "fork" {
            body += &format!("fork f{s} {{ {calls} }} join all f{s};");
        } else if shape == "serial" {
            for i in 1..=width {
                body += &format!(
                    "a{s}_{i}={}.check({previous}); if a{s}_{i}==pass {{}} else {{end rejected}};",
                    judge(i)
                );
            }
        } else {
            assert_eq!(shape, "and");
            body += &calls;
        }
        if shape != "serial" {
            let condition = (1..=width)
                .map(|i| format!("a{s}_{i}==pass"))
                .collect::<Vec<_>>()
                .join(" && ");
            body += &format!("if {condition} {{}} else {{end rejected}};");
        }
        previous = format!("a{s}_1");
    }
    model(
        &format!("{body} p=Ledger.post({previous}); end done"),
        width,
    )
}
pub fn violation() -> String {
    joined(11, " && ")
        .replace("end done", "p2=Ledger.post(j11); end done")
        .replace(
            "verify safety",
            "contract Once { at_most_once Ledger.post.commit per case.id }\nverify safety",
        )
}
pub fn simulation() -> String {
    gate(3)
        .replace("actor Doc", "latent case { coin: bool = false }\nkernel Coin given [coin] { false -> { 0.5:pass; 0.5:fail }; true -> { 0.5:pass; 0.5:fail } }\nactor Doc")
        .replace(
            "result nondet { pass, fail }",
            "kernel_reads latent [coin]; outcomes kernel Coin",
        )
        .replace(
            "verify safety",
            "simulation replications 4 seed 1 confidence 0.95\nverify safety",
        )
}
pub fn contradiction() -> String {
    model(
        "s=Intake.start(); j1=Judge.check(s); if j1==pass && j1==fail { p=Ledger.post(j1) } else {}; end done",
        1,
    )
}
pub fn state_taint() -> String {
    model(
        "s=Intake.start(); if flag==pass { p=Ledger.post(s) } else {}; end done",
        1,
    )
    .replace("actor Doc", "observed case { flag: V = pass }\nactor Doc")
}
pub fn version_domain() -> String {
    model(
        "s=Intake.start(); j1=Judge.check(s); if j1==pass {end done} else {end rejected}",
        1,
    )
    .replace(
        "actor Judge { capacity 1;",
        "actor Judge { capacity 1; state marker: V = pass;",
    )
    .replace(
        "capability check(x:V)->V {",
        "capability check(x:V)->V { version_check [marker] else fail; writes marker; finish {marker=fail}",
    )
    .replace("result nondet { pass, fail }", "result nondet { pass }")
}
pub fn requires() -> String {
    gate(2)
        .replace(
            "actor Judge { capacity 1;",
            "actor Judge { capacity 1; state permit: V = pass;",
        )
        .replace(
            "capability check(x:V)->V {",
            "capability check(x:V)->V { requires permit==pass else fail;",
        )
        .replace("result nondet { pass, fail }", "result nondet { pass }")
}
pub fn rederivation() -> String {
    joined(11, " || ")
        .replace("actor Doc", "people [j,r]\nactor Doc")
        .replace("actor Judge { capacity 1;", "actor Judge { executors [j];")
        .replace("process Flow", "actor Party { control party; standing over [end.rejected] basis \"claim\" }\nactor Route { executors [r]; capability respond()->V { duration fixed 2min; result nondet {pass} } }\npermit j to Judge.check\npermit r to Route.respond\nauthority Grant {basis \"decision\"; grants j to Judge.check}\nresponsible j for Judge.check\ncontract Controls {contest Party on end.rejected route Route.respond reversible_by [j] respond_by 2min}\nprocess Flow")
}
pub fn join_any() -> String {
    "model Any version \"0.2\" { type V=enum{pass,fail} actor A { capacity 1; capability run()->V {duration fixed 1min; result nondet{pass,fail}; cancel immediate} } actor B {capacity 1; capability run()->V {duration fixed 2min; result nondet{pass,fail}; cancel immediate}} process Flow {fork f {a=A.run(); b=B.run()} join any f where result==pass tie by [a,b] remaining cancel_then_drain if none -> returned; end done; end returned} bounds{cases 1;live_tasks 4;clock 20min} verify safety no_resource_overcommit }".into()
}
pub fn differential_models() -> Vec<(String, String)> {
    let mut models = Vec::new();
    for k in 1..=8 {
        models.push((format!("gate-{k}"), gate(k)));
        models.push((format!("and-{k}"), joined(k, " && ")));
        models.push((format!("or-{k}"), joined(k, " || ")));
    }
    for (s, w) in [(1, 2), (2, 1), (2, 2), (2, 3), (3, 2), (4, 2)] {
        for shape in ["serial", "and", "fork"] {
            models.push((format!("stage-{shape}-{s}-{w}"), stage(s, w, shape)));
        }
    }
    models.extend([
        ("G1-state".into(), state_taint()),
        ("G2-unsat".into(), contradiction()),
        ("G3-domain".into(), version_domain()),
        ("requires".into(), requires()),
        ("join-any".into(), join_any()),
        (
            "actor-zero".into(),
            gate(2).replace("actor Judge { capacity 1", "actor Judge { capacity 0"),
        ),
        (
            "uses-over-capacity".into(),
            gate(2)
                .replace("actor Doc", "resource slots capacity 1\nactor Doc")
                .replace(
                    "capability check(x:V)->V {",
                    "capability check(x:V)->V { uses slots: 2;",
                ),
        ),
    ]);
    models
}
pub fn count(vp: &Value, capability: &str) -> Value {
    let row = vp["retention"]["capabilities"]
        .as_array()
        .expect("derived retention")
        .iter()
        .find(|r| r["capability"] == capability)
        .expect("capability row");
    assert_eq!(row["status"], "derived", "{row}");
    row["normal"].clone()
}
pub fn framing(vp: &Value, end: &str) -> Value {
    assert_eq!(vp["framing"]["status"], "derived", "{vp}");
    let row = vp["framing"]["ends"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["end"] == end)
        .expect("end row");
    assert_eq!(row["status"], "derived", "{row}");
    assert_eq!(row["reachable"], true, "{row}");
    let mut result = serde_json::Map::new();
    for selector in row["selectors"].as_array().unwrap() {
        assert!(
            result
                .insert(
                    selector["call"].as_str().unwrap().into(),
                    selector["selects"].clone()
                )
                .is_none()
        );
    }
    for read in row["read_only"].as_array().unwrap() {
        assert!(
            result
                .insert(read["call"].as_str().unwrap().into(), json!("read_only"))
                .is_none()
        );
    }
    Value::Object(result)
}
pub fn assert_gate(vp: &Value, k: usize) {
    assert_eq!(vp["framing"]["complete"], true);
    let done = (1..=k)
        .map(|i| (format!("j{i}"), json!("all_contexts")))
        .collect::<serde_json::Map<_, _>>();
    let rejected = (1..=k)
        .map(|i| {
            (
                format!("j{i}"),
                json!(if i == k { "some_contexts" } else { "read_only" }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    // A single gate has no bypass context; k>=2 has an earlier rejecting gate.
    let mut rejected = Value::Object(rejected);
    if k == 1 {
        rejected["j1"] = json!("all_contexts");
    }
    assert_eq!(framing(vp, "done"), Value::Object(done));
    assert_eq!(framing(vp, "rejected"), rejected);
    assert_eq!(count(vp, "Intake.start"), json!({"min":1,"max":1}));
    assert_eq!(count(vp, "Judge.check"), json!({"min":1,"max":k}));
    assert_eq!(count(vp, "Ledger.post"), json!({"min":0,"max":1}));
}
pub fn assert_stage(vp: &Value, s: usize, w: usize) {
    let mut done = serde_json::Map::new();
    let mut rejected = serde_json::Map::new();
    for t in 1..=s {
        for i in 1..=w {
            done.insert(format!("a{t}_{i}"), json!("all_contexts"));
            rejected.insert(
                format!("a{t}_{i}"),
                json!(if t == s { "some_contexts" } else { "read_only" }),
            );
        }
    }
    assert_eq!(framing(vp, "done"), Value::Object(done));
    assert_eq!(framing(vp, "rejected"), Value::Object(rejected));
    for i in 1..=w {
        assert_eq!(
            count(vp, &format!("{}.check", judge(i))),
            json!({"min":1,"max":s})
        );
    }
    assert_eq!(count(vp, "Intake.start"), json!({"min":1,"max":1}));
    assert_eq!(count(vp, "Ledger.post"), json!({"min":0,"max":1}));
}
