//! Ninth population, independent hand oracle first, engine/reference equality second.
use relca_core::{
    engine, ir, lower,
    outcome::Outcome,
    reference::{self, StopReason},
    report, syntax,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const FIXTURES: [&str; 18] = [
    "duration-cancel",
    "duration-cancel.broken-ignore-cancel",
    "duration-clock",
    "duration-clock.broken-short-clock",
    "duration-contention",
    "duration-contention.broken-extra-capacity",
    "duration-duplicate",
    "duration-duplicate.broken-duplicate-value",
    "duration-no-lookahead",
    "duration-no-lookahead.broken-late-choice",
    "duration-parallel",
    "duration-parallel.broken-tight-upper",
    "duration-serial",
    "duration-serial.broken-tight-upper",
    "duration-sum",
    "duration-sum.broken-mass",
    "duration-zero",
    "duration-zero.broken-late-positive",
];
fn terminals(model: &ir::Model, out: &Outcome) -> BTreeMap<String, Value> {
    let rows: BTreeMap<_, _> = out.terminals.iter().map(|t| {
        let end = model.ends[t.end.0].clone();
        let p = t.probability.as_ref().expect("duration models use probability");
        (end.clone(), json!({"end":end,"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|r|json!({"min":r.min,"max":r.max})),"probability":{"min":p.min.to_string(),"max":p.max.to_string()}}))
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
    let expected: BTreeMap<_, _> = h["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["end"].as_str().unwrap().to_owned(), t.clone()))
        .collect();
    assert_eq!(
        terminals(model, out),
        expected,
        "{label}: independent hand terminal oracle"
    );
    assert_eq!(
        out.properties.len(),
        h["properties"].as_array().unwrap().len()
    );
    assert_eq!(out.properties.len(), model.verify.len());
    for ((actual, verify), expected) in out
        .properties
        .iter()
        .zip(&model.verify)
        .zip(h["properties"].as_array().unwrap())
    {
        assert_eq!(actual.property, verify.property);
        assert_eq!(
            serde_json::to_value(actual.verdict).unwrap(),
            expected["verdict"],
            "{label}: {}",
            expected["id"]
        );
        let p = actual
            .probability
            .as_ref()
            .map(|p| json!({"min":p.min.to_string(),"max":p.max.to_string()}));
        assert_eq!(
            p,
            expected.get("probability").cloned(),
            "{label}: {}",
            expected["id"]
        );
    }
}
fn run(label: &str, source: &str, h: &Value) {
    let model = syntax::parse(source)
        .and_then(|a| lower::lower(&a))
        .unwrap_or_else(|d| panic!("{label}: {d:?}"));
    let e = engine::explore(&model);
    let r = reference::explore(&model);
    assert_eq!(
        r.stop,
        StopReason::Complete,
        "{label}: no reference exclusions or enumeration limits"
    );
    hand(&format!("{label}/engine"), &model, &e, h);
    hand(&format!("{label}/reference"), &model, &r.outcome, h);
    assert_eq!(serde_json::to_value(r.verdict()).unwrap(), h["verdict"]);
    assert_eq!(e.properties, r.outcome.properties, "{label}");
    assert_eq!(terminals(&model, &e), terminals(&model, &r.outcome));
    assert!(r.outcome.diagnostics.starts_with(&model.diagnostics));
    let diagnostics = |ds: &[relca_core::diag::Diagnostic]| -> Vec<Value> {
        ds.iter()
            .map(|d| json!({"code":d.code,"severity":d.severity,"span":d.span}))
            .collect()
    };
    assert_eq!(
        diagnostics(&e.diagnostics),
        diagnostics(&r.outcome.diagnostics[model.diagnostics.len()..]),
        "{label}: runtime diagnostics"
    );
    let v = serde_json::to_value(report::check(source)).unwrap();
    assert_eq!(v["verdict"], h["verdict"]);
    assert_eq!(v["properties"], h["properties"]);
}

#[test]
fn duration_population_18_directories_18_inputs_16_runtime_2_rejected_zero_exclusions() {
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
                    .starts_with("duration-")
        })
        .collect();
    dirs.sort();
    assert_eq!(
        dirs.iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        FIXTURES
    );
    let (mut inputs, mut runtime, mut rejected) = (0, 0, 0);
    let mut errors = Vec::new();
    for dir in dirs {
        let name = dir.file_name().unwrap().to_str().unwrap();
        for file in ["model.relca", "expected.json", "calc.md"] {
            assert!(dir.join(file).is_file());
        }
        assert!(
            !dir.join("invocation.json").exists(),
            "all new fixtures are single checks"
        );
        let source = fs::read_to_string(dir.join("model.relca")).unwrap();
        let h: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
        inputs += 1;
        if [
            "duration-sum.broken-mass",
            "duration-duplicate.broken-duplicate-value",
        ]
        .contains(&name)
        {
            rejected += 1;
            match syntax::parse(&source).and_then(|a| lower::lower(&a)) {
                Ok(_) => errors.push(format!("{name}: expected E-PROB-0")),
                Err(ds) => {
                    if !ds.iter().any(|d| {
                        d.code == "E-PROB-0" && d.severity == relca_core::diag::Severity::Error
                    }) {
                        errors.push(format!("{name}: wrong rejection {ds:?}"));
                    }
                }
            }
            let v = serde_json::to_value(report::check(&source)).unwrap();
            assert_eq!(v["verdict"], "invalid");
            assert_eq!(v["terminals"], json!([]));
            assert_eq!(v["properties"], json!([]));
            assert!(v["agency"].is_null());
            for key in ["cases", "live_tasks", "clock", "truncated"] {
                assert!(v["bounds"][key].is_null());
            }
        } else {
            match syntax::parse(&source).and_then(|a| lower::lower(&a)) {
                Err(ds) => errors.push(format!("{name}: unexpected rejection {ds:?}")),
                Ok(_) => {
                    run(name, &source, &h);
                    runtime += 1;
                }
            }
        }
    }
    assert_eq!(inputs, 18);
    assert_eq!(rejected, 2);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert_eq!(runtime, 16);
}

#[test]
fn d_clock_vs_state_budget_public_outcomes() {
    let source = include_str!("../../../fixtures/duration-clock/model.relca");
    let model = syntax::parse(source)
        .and_then(|a| lower::lower(&a))
        .expect("duration syntax");
    for budget in [0, 1] {
        let out = engine::explore_with_budget(&model, budget);
        assert!(out.truncated);
        assert_eq!(out.properties.len(), 2);
        for property in &out.properties {
            let p = property.probability.as_ref().unwrap();
            assert_eq!(
                (p.min.to_string(), p.max.to_string()),
                ("0".into(), "1".into())
            );
            assert_eq!(property.verdict, relca_core::Verdict::Inconclusive);
        }
    }
    let hit = source
        .replace("hit: bool = false", "hit: bool = true")
        .replace("within 2min <= 0.5", "within 0min >= 1");
    let model = syntax::parse(&hit).and_then(|a| lower::lower(&a)).unwrap();
    let seed = engine::explore_with_budget(&model, 0);
    let p = seed.properties[0].probability.as_ref().unwrap();
    assert_eq!(
        (p.min.to_string(), p.max.to_string()),
        ("0".into(), "1".into()),
        "unobserved seed is not a receipt witness"
    );
    let mut witnessed_before_cut = false;
    for budget in 2..64 {
        let out = engine::explore_with_budget(&model, budget);
        let p = out.properties[0].probability.as_ref().unwrap();
        if out.truncated
            && out.diagnostics.iter().any(|d| d.code == "W-BUDGET-0")
            && p.min.to_string() == "1"
            && p.max.to_string() == "1"
        {
            assert_eq!(out.properties[0].verdict, relca_core::Verdict::Satisfied);
            witnessed_before_cut = true;
            break;
        }
    }
    assert!(
        witnessed_before_cut,
        "receipt hit must survive a later budget cut"
    );
}

#[test]
fn d_hazard_three_both_interpreters_match_fractions() {
    let base = include_str!("../../../fixtures/duration-sum/model.relca");
    let source = base
        .replace("3min:0.5,5min:0.5", "1min:0.25,2min:0.25,3min:0.5")
        .replace("within 3min <= 0.5", "within 2min <= 0.5");
    let h = json!({"verdict":"satisfied","bounds":{"truncated":false},"properties":[{"id":"safety no_resource_overcommit","verdict":"satisfied"},{"id":"probability reached #1","verdict":"satisfied","probability":{"min":"1/2","max":"1/2"}}],"terminals":[{"end":"done","elapsed":{"min":1,"max":3},"work":{"min":1,"max":3},"result_at":null,"probability":{"min":"1","max":"1"}}]});
    run("q2=(1/4)/(3/4)=1/3", &source, &h);
}
