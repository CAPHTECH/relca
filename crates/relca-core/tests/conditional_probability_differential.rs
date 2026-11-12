//! Eighth population: each interpreter must match the independent hand oracle
//! before comparing with the other. Fixed 16 dirs / 18 inputs / 16 runtime.
use relca_core::{
    Verdict, engine, ir, lower,
    outcome::Outcome,
    reference::{self, StopReason},
    report, syntax,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const FIXTURES: [&str; 16] = [
    "prob-cancel",
    "prob-cancel.broken-ignore-cancel",
    "prob-cut",
    "prob-cut.broken-short-clock",
    "prob-finish-read",
    "prob-finish-read.broken-late-writer",
    "prob-invoice",
    "prob-invoice.broken-ai-transfer",
    "prob-mdp",
    "prob-mdp.broken-tight-upper",
    "prob-rows",
    "prob-rows.broken-missing-difficult",
    "prob-sum",
    "prob-sum.broken-mass",
    "prob-zero",
    "prob-zero.broken-positive-write",
];
fn terminal_rows(model: &ir::Model, out: &Outcome) -> BTreeMap<String, Value> {
    let rows:BTreeMap<_,_>=out.terminals.iter().map(|t|{
  let end=model.ends[t.end.0].clone();
  let probability=t.probability.as_ref().map(|p|json!({"min":p.min.to_string(),"max":p.max.to_string()}));
  (end.clone(),json!({"end":end,"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|r|json!({"min":r.min,"max":r.max})),"probability":probability}))
 }).collect();
    assert_eq!(rows.len(), out.terminals.len());
    rows
}
fn hand(label: &str, model: &ir::Model, out: &Outcome, h: &Value) {
    assert_eq!(
        out.truncated,
        h["bounds"]["truncated"].as_bool().unwrap(),
        "{label}"
    );
    let terminals: BTreeMap<_, _> = h["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["end"].as_str().unwrap().to_owned(), t.clone()))
        .collect();
    assert_eq!(
        terminal_rows(model, out),
        terminals,
        "{label}: hand terminals"
    );
    assert_eq!(
        out.properties.len(),
        h["properties"].as_array().unwrap().len(),
        "{label}: inventory"
    );
    assert_eq!(out.properties.len(), model.verify.len());
    for ((p, verify), h) in out
        .properties
        .iter()
        .zip(&model.verify)
        .zip(h["properties"].as_array().unwrap())
    {
        assert_eq!(p.property, verify.property, "{label}");
        let verdict: Verdict = serde_json::from_value(h["verdict"].clone()).unwrap();
        assert_eq!(p.verdict, verdict, "{label}: {}", h["id"]);
        let probability = p
            .probability
            .as_ref()
            .map(|r| json!({"min":r.min.to_string(),"max":r.max.to_string()}));
        assert_eq!(
            probability,
            h.get("probability").cloned(),
            "{label}: {}",
            h["id"]
        );
    }
}
fn run(label: &str, source: &str, selection: Option<&str>, h: &Value) {
    let ast = syntax::parse(source).unwrap_or_else(|d| panic!("{label}: parse {d:?}"));
    let model = match selection {
        Some(s) => lower::lower_configuration(&ast, s).map(|s| s.model),
        None => lower::lower(&ast),
    }
    .unwrap_or_else(|d| panic!("{label}: lower {d:?}"));
    let e = engine::explore(&model);
    let r = reference::explore(&model);
    assert_eq!(
        r.stop,
        StopReason::Complete,
        "{label}: zero reference exclusions, model cuts are not enumeration limits"
    );
    assert_eq!(r.truncated(), Some(e.truncated));
    hand(&format!("{label}/engine"), &model, &e, h);
    hand(&format!("{label}/reference"), &model, &r.outcome, h);
    assert_eq!(
        serde_json::to_value(r.verdict()).unwrap(),
        h["verdict"],
        "{label}"
    );
    assert_eq!(
        e.properties, r.outcome.properties,
        "{label}: all differential properties"
    );
    assert_eq!(terminal_rows(&model, &e), terminal_rows(&model, &r.outcome));
    assert!(r.outcome.diagnostics.starts_with(&model.diagnostics));
    let diagnostics = |ds: &[relca_core::diag::Diagnostic]| -> Vec<Value> {
        ds.iter()
            .map(|d| json!({"code":d.code,"severity":d.severity,"span":d.span}))
            .collect()
    };
    assert_eq!(
        diagnostics(&e.diagnostics),
        diagnostics(&r.outcome.diagnostics[model.diagnostics.len()..]),
        "{label}: full runtime diagnostic inventory"
    );
    let v = serde_json::to_value(match selection {
        Some(s) => report::check_configuration(source, s),
        None => report::check(source),
    })
    .unwrap();
    assert_eq!(v["verdict"], h["verdict"]);
    assert_eq!(v["properties"], h["properties"]);
}
#[test]
fn probability_population_16_directories_18_inputs_16_accepted_2_rejected_zero_exclusions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut dirs: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("prob-")
        })
        .collect();
    dirs.sort();
    assert_eq!(
        dirs.iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        FIXTURES
    );
    let (mut inputs, mut accepted, mut rejected, mut compared) = (0, 0, 0, 0);
    let mut errors = Vec::new();
    for d in dirs {
        let name = d.file_name().unwrap().to_str().unwrap();
        for f in ["model.relca", "calc.md", "expected.json"] {
            assert!(d.join(f).is_file());
        }
        let s = fs::read_to_string(d.join("model.relca")).unwrap();
        let h: Value =
            serde_json::from_str(&fs::read_to_string(d.join("expected.json")).unwrap()).unwrap();
        let selections = if name.starts_with("prob-invoice") {
            vec![
                (Some("AsIs"), &h["from"]["report"]),
                (Some("ToBeP"), &h["to"]["report"]),
            ]
        } else {
            vec![(None, &h)]
        };
        for (selection, h) in selections {
            inputs += 1;
            let parsed = syntax::parse(&s).and_then(|ast| match selection {
                Some(c) => lower::lower_configuration(&ast, c).map(|c| c.model),
                None => lower::lower(&ast),
            });
            if ["prob-sum.broken-mass", "prob-rows.broken-missing-difficult"].contains(&name) {
                rejected += 1;
                match parsed {
                    Ok(_) => errors.push(format!("{name}: expected E-PROB-0")),
                    Err(ds) => {
                        if !ds.iter().any(|d| {
                            d.code == "E-PROB-0" && d.severity == relca_core::diag::Severity::Error
                        }) {
                            errors.push(format!("{name}: wrong rejection {ds:?}"));
                        }
                    }
                }
                let v = serde_json::to_value(report::check(&s)).unwrap();
                assert_eq!(v["verdict"], "invalid");
                assert_eq!(v["properties"], json!([]));
                assert_eq!(v["terminals"], json!([]));
                assert!(v["agency"].is_null());
                for k in ["cases", "live_tasks", "clock", "truncated"] {
                    assert!(v["bounds"][k].is_null());
                }
            } else {
                match parsed {
                    Err(ds) => {
                        errors.push(format!("{name}/{selection:?}: unexpected rejection {ds:?}"))
                    }
                    Ok(_) => {
                        accepted += 1;
                        run(name, &s, selection, h);
                        compared += 1;
                    }
                }
            }
        }
    }
    assert_eq!(inputs, 18);
    assert_eq!(rejected, 2);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert_eq!(accepted, 16);
    assert_eq!(compared, 16);
}

#[test]
fn independent_merge_transient_and_cut_targets() {
    let base = include_str!("../../../fixtures/prob-sum/model.relca");
    let s=base.replace("0.25: ok with hit = true; 0.75: ok with hit = false","0.2: ok with hit = true; 0.3: ok with hit = true; 0.5: ok with hit = false").replace("<= 0.25","<= 0.5")
  .replace("process Flow","actor Reset { capacity 1; capability clear() -> T { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = false } } }; process Flow")
  .replace("if hit == true { end bad } else { end good }","r = Reset.clear(); end done");
    let h = json!({"verdict":"satisfied","bounds":{"truncated":false},"properties":[{"id":"safety no_resource_overcommit","verdict":"satisfied"},{"id":"probability reached #1","verdict":"satisfied","probability":{"min":"1/2","max":"1/2"}}],"terminals":[{"end":"done","elapsed":{"min":2,"max":2},"work":{"min":2,"max":2},"result_at":null,"probability":{"min":"1","max":"1"}}]});
    run("merged future does not erase past hit", &s, None, &h);
    let s = include_str!("../../../fixtures/prob-cut.broken-short-clock/model.relca")
        .replace("seen == true >= 0.75", "route == long <= 0.5");
    let mut h: Value = serde_json::from_str(include_str!(
        "../../../fixtures/prob-cut.broken-short-clock/expected.json"
    ))
    .unwrap();
    h["properties"][1] = json!({"id":"probability reached #1","verdict":"satisfied","probability":{"min":"1/2","max":"1/2"}});
    run("target seen before model clock cut", &s, None, &h);
}
