//! Twelfth, disjoint corpus. Each runtime first meets a hand oracle, then each other.
use relca_core::{
    compare::{self, Analysis},
    diff, engine, ir, lower,
    outcome::{CaseTerminalKind, Outcome},
    reference, syntax,
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::PathBuf};

const FIXTURES: [&str; 16] = [
    "compare-arrivals",
    "compare-arrivals.broken-slow-plan",
    "compare-case-diff",
    "compare-case-diff.broken-same-kernel",
    "compare-cut",
    "compare-cut.broken-other-cut",
    "compare-legacy",
    "compare-legacy.broken-longer-task",
    "compare-random",
    "compare-random.broken-alias-process",
    "compare-reject",
    "compare-reject.broken-zero-queue",
    "compare-terminal",
    "compare-terminal.broken-other-end",
    "compare-unlock",
    "compare-unlock.broken-excluded-call",
];
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn range(r: relca_core::outcome::Range) -> Value {
    json!({"min":r.min,"max":r.max})
}
fn probability(p: &relca_core::probability::ProbabilityRange) -> Value {
    json!({"min":p.min.to_string(),"max":p.max.to_string()})
}
fn id(model: &ir::Model, p: ir::Property) -> String {
    match p {
        ir::Property::ProbabilityReached(i) => format!("probability reached #{}", i + 1),
        ir::Property::NoResourceOvercommit => "safety no_resource_overcommit".into(),
        ir::Property::ReachableEnd(e) => format!("reachable end.{}", model.ends[e.0]),
        ir::Property::Contract(_) => panic!("new fixed population has no contracts"),
        ir::Property::Contest(_) => unreachable!(),
        // ADR 0025 completion properties are not in this fixed population.
        _ => unreachable!(),
    }
}
fn projection(model: &ir::Model, o: &Outcome) -> Value {
    let cases:Vec<_> = o.case_outcomes.iter().flatten().map(|c| {
        let ts:Vec<_> = c.terminals.iter().map(|t| {
            let (kind,end)=match t.kind { CaseTerminalKind::ReceiptUnknown=>panic!("compare terminal oracle does not cover receipt observations"),CaseTerminalKind::End(e)=>("end",json!(model.ends[e.0])),CaseTerminalKind::Rejected=>("rejected",Value::Null),CaseTerminalKind::Overflow(e)=>("overflow",json!(model.ends[e.0])),CaseTerminalKind::CutOff=>("cut_off",Value::Null) };
            let mut row=json!({"kind":kind,"end":end,"completed_at":t.completed_at.map(range),"elapsed":t.elapsed.map(range),"work":t.work.map(range),"result_at":t.result_at.map(range)});
            if let Some(p)=&t.probability { row["probability"]=probability(p); } row
        }).collect(); json!({"case_id":c.case_id.0+1,"arrival":c.arrival,"terminals":ts})
    }).collect();
    let properties: Vec<_> = o
        .properties
        .iter()
        .map(|p| {
            let mut row = json!({"id":id(model,p.property),"verdict":p.verdict});
            if let Some(p) = &p.probability {
                row["probability"] = probability(p);
            }
            if let Some(cs) = &p.cases {
                row["cases"] = json!(cs.iter().map(|c| {
            let mut row=json!({"case_id":c.case_id.0+1,"arrival":c.arrival,"verdict":c.verdict});
            if let Some(p)=&c.probability { row["probability"]=probability(p); } row
        }).collect::<Vec<_>>());
            }
            row
        })
        .collect();
    let terminals:Vec<_> = o.terminals.iter().map(|t| {
        let mut row=json!({"end":model.ends[t.end.0],"elapsed":range(t.elapsed),"work":range(t.work),"result_at":t.result_at.map(range)});
        if let Some(p)=&t.probability { row["probability"]=probability(p); } row
    }).collect();
    json!({"case_outcomes":cases,"properties":properties,"terminals":terminals,"truncated":o.truncated})
}
fn exact_prob(v: &Value, p: &str) {
    assert_eq!(*v, json!({"min":p,"max":p}));
}

fn hand(name: &str, side: usize, model: &ir::Model, o: &Outcome) {
    assert!(!o.truncated);
    assert_eq!(o.properties.len(), model.verify.len());
    let v = projection(model, o);
    for (p, verify) in o.properties.iter().zip(&model.verify) {
        assert_eq!(p.property, verify.property);
    }
    if name.starts_with("compare-legacy")
        || name.starts_with("compare-cut")
        || name.starts_with("compare-terminal")
    {
        let t = &v["terminals"][0];
        let longer = name.contains("longer-task");
        let (elapsed, work) = if name.starts_with("compare-legacy") {
            (
                if side == 0 {
                    if longer { 6 } else { 5 }
                } else if longer {
                    9
                } else {
                    8
                },
                if longer { 9 } else { 8 },
            )
        } else {
            (1, 1)
        };
        assert_eq!(v["terminals"].as_array().unwrap().len(), 1);
        assert_eq!(t["elapsed"], json!({"min":elapsed,"max":elapsed}));
        assert_eq!(t["work"], json!({"min":work,"max":work}));
        assert!(t["result_at"].is_null());
        let other = name.contains("other-end") && side == 1;
        assert_eq!(t["end"], if other { "other" } else { "done" });
        assert_eq!(
            v["properties"][0]["verdict"],
            if other { "violated" } else { "satisfied" }
        );
        return;
    }
    let cs = v["case_outcomes"].as_array().unwrap();
    let n = if name.starts_with("compare-unlock") {
        1
    } else if name.starts_with("compare-reject") {
        3
    } else {
        2
    };
    assert_eq!(cs.len(), n);
    assert_eq!(v["terminals"], json!([]));
    for (i, c) in cs.iter().enumerate() {
        assert_eq!(c["case_id"], i + 1);
        let ts = c["terminals"].as_array().unwrap();
        if name.starts_with("compare-case-diff") {
            assert_eq!(c["arrival"], 3 * i);
            assert_eq!(ts.len(), 2);
            let pass = if side == 0 || name.contains("broken") {
                "1/4"
            } else {
                "3/4"
            };
            let fail = if pass == "1/4" { "3/4" } else { "1/4" };
            for t in ts {
                exact_prob(
                    &t["probability"],
                    if t["end"] == "yes" { pass } else { fail },
                );
                assert_eq!(t["completed_at"], json!({"min":1+3*i,"max":1+3*i}));
                assert_eq!(t["elapsed"], json!({"min":1,"max":1}));
                assert_eq!(t["work"], json!({"min":1,"max":1}));
            }
            exact_prob(&v["properties"][0]["cases"][i]["probability"], pass);
        } else {
            assert_eq!(ts.len(), 1);
            let t = &ts[0];
            if name.starts_with("compare-reject") {
                let rejected = name.contains("zero-queue") || (side == 0 && i == 2);
                assert_eq!(c["arrival"], i);
                exact_prob(&t["probability"], "1");
                assert_eq!(t["kind"], if rejected { "rejected" } else { "end" });
                for f in ["completed_at", "elapsed", "work"] {
                    if rejected {
                        assert!(t[f].is_null());
                    } else {
                        let x = match f {
                            "completed_at" => {
                                if side == 0 {
                                    4 * (i + 1)
                                } else {
                                    i + 1
                                }
                            }
                            "elapsed" => {
                                if side == 0 {
                                    if i == 0 { 4 } else { 7 }
                                } else {
                                    1
                                }
                            }
                            _ => {
                                if side == 0 {
                                    4
                                } else {
                                    1
                                }
                            }
                        };
                        assert_eq!(t[f], json!({"min":x,"max":x}));
                    }
                }
                exact_prob(
                    &v["properties"][0]["cases"][i]["probability"],
                    if rejected { "0" } else { "1" },
                );
            } else if name.starts_with("compare-arrivals") {
                let slow = side == 0 || name.contains("slow-plan");
                let (min, max) = if slow {
                    if i == 0 { (1, 3) } else { (2, 6) }
                } else {
                    (i + 1, i + 1)
                };
                assert_eq!(c["arrival"], 0);
                exact_prob(&t["probability"], "1");
                assert_eq!(t["elapsed"], json!({"min":min,"max":max}));
                assert_eq!(t["completed_at"], t["elapsed"]);
                assert_eq!(t["work"], json!({"min":1,"max":if slow{3}else{1}}));
                exact_prob(
                    &v["properties"][1]["cases"][i]["probability"],
                    if slow {
                        if i == 0 { "1/2" } else { "1/4" }
                    } else {
                        "1"
                    },
                );
            } else {
                assert_eq!(t["completed_at"], json!({"min":2,"max":2}));
                assert_eq!(t["elapsed"], t["completed_at"]);
                assert_eq!(t["work"], t["elapsed"]);
            }
            assert!(t["result_at"].is_null());
        }
    }
    for p in v["properties"].as_array().unwrap() {
        assert_eq!(p["verdict"], "satisfied");
    }
}

fn simulation_hand(side: usize, alias: bool, v: &Value) {
    assert_eq!(v["replications"].as_array().unwrap().len(), 4);
    for (i, r) in v["replications"].as_array().unwrap().iter().enumerate() {
        let rep = i + 1;
        let process = if side == 0 || alias { "P" } else { "Q" };
        let mut expected: Vec<_> = [
            (1, "init.case.z".to_owned()),
            (0, "init.actor.A.z".to_owned()),
            (1, format!("kernel.{process}.a")),
            (1, format!("duration.{process}.a")),
        ]
        .into_iter()
        .map(|(c, s)| format!("relca-sim-v1|seed=0|rep={rep}|case={c}|site={s}|n=1"))
        .collect();
        expected.sort();
        let mut actual: Vec<_> = r["draws"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["key"].as_str().unwrap().to_owned())
            .collect();
        actual.sort();
        assert_eq!(actual, expected);
        let hit = process == "P" && rep >= 3;
        assert_eq!(
            r["cases"][0]["terminal"]["end"],
            if hit { "yes" } else { "no" }
        );
        assert_eq!(
            r["cases"][0]["queries"],
            json!([
                "hit",
                if rep >= 3 { "hit" } else { "miss" },
                if hit { "hit" } else { "miss" }
            ])
        );
        assert!(r["cases"][0]["deadlock"].is_null());
        assert!(r["cut"].is_null());
    }
}

#[test]
fn compare_population_16_dirs_32_selected_30_runtime_2_static_complete30_excluded0() {
    let mut names: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| {
            e.file_type().unwrap().is_dir()
                && e.file_name().to_string_lossy().starts_with("compare-")
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, FIXTURES);
    let (mut selected, mut runtime, mut rejected, mut complete) = (0, 0, 0, 0);
    for name in FIXTURES {
        let source = fs::read_to_string(root().join(name).join("model.relca")).unwrap();
        let ast = syntax::parse(&source).unwrap();
        let plans = if name.starts_with("compare-legacy") {
            ["ToBeP", "AsIs"]
        } else {
            ["Aplan", "Bplan"]
        };
        let mut refs = Vec::new();
        for (side, plan) in plans.iter().enumerate() {
            selected += 1;
            let m = lower::lower_configuration(&ast, plan);
            if name.ends_with("broken-excluded-call") {
                let ds = m.err().expect("nonselected excluded call remains invalid");
                assert!(ds.iter().any(|d| d.code == "E-CUT-1"));
                rejected += 1;
                continue;
            }
            let m = m.unwrap_or_else(|ds| panic!("{name}/{plan}: {ds:?}")).model;
            runtime += 1;
            if name.starts_with("compare-random") {
                let e = serde_json::to_value(engine::simulate(&m)).unwrap();
                let r = reference::simulate(&m);
                assert_eq!(r.stop, reference::StopReason::Complete);
                complete += 1;
                let rv = serde_json::to_value(r.outcome).unwrap();
                simulation_hand(side, name.contains("broken"), &e);
                simulation_hand(side, name.contains("broken"), &rv);
                for field in [
                    "replications",
                    "attempted_replications",
                    "completed_replications",
                    "truncated_replications",
                ] {
                    assert_eq!(e[field], rv[field]);
                }
            } else {
                let e = engine::explore(&m);
                let r = reference::explore(&m);
                assert_eq!(r.stop, reference::StopReason::Complete);
                complete += 1;
                hand(name, side, &m, &e);
                hand(name, side, &m, &r.outcome);
                assert_eq!(projection(&m, &e), projection(&m, &r.outcome));
                refs.push(projection(&m, &r.outcome));
            }
        }
        if name.starts_with("compare-case-diff") {
            let d = serde_json::to_value(diff::diff(&source, plans[0], plans[1])).unwrap();
            let c = serde_json::to_value(compare::compare(&source, &plans, Analysis::Exhaustive))
                .unwrap();
            for case in 0..2 {
                let dc = &d["case_outcomes"][case];
                assert_eq!(dc["case_id"], case + 1);
                for t in dc["terminals"].as_array().unwrap() {
                    for (i, side) in ["from", "to"].into_iter().enumerate() {
                        let rt = refs[i]["case_outcomes"][case]["terminals"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|r| r["end"] == t["end"])
                            .unwrap();
                        for f in [
                            "elapsed",
                            "work",
                            "completed_at",
                            "result_at",
                            "probability",
                        ] {
                            assert_eq!(t[side][f], rt[f]);
                        }
                    }
                }
                let row = c["tables"]["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["case_id"] == case + 1)
                    .unwrap();
                for (i, reference) in refs.iter().enumerate() {
                    assert_eq!(
                        row["values"][i]["probability"],
                        reference["properties"][0]["cases"][case]["probability"]
                    );
                }
            }
        }
    }
    assert_eq!((selected, runtime, rejected, complete), (32, 30, 2, 30));
}

#[test]
fn c_rejected_cases_do_not_draw_initial_values_or_reindex_keys() {
    for name in ["compare-reject", "compare-reject.broken-zero-queue"] {
        let source = fs::read_to_string(root().join(name).join("model.relca")).unwrap();
        let source=source.replace("  type T","  type Z = enum { first, second }\n  latent case { z:Z ~ {first:0.5,second:0.5} }\n  type T")
            .replace("  bounds","  simulation replications 4 seed 0 confidence 0.5\n  bounds");
        let ast = syntax::parse(&source).unwrap();
        for (side, plan) in ["Aplan", "Bplan"].into_iter().enumerate() {
            let m = lower::lower_configuration(&ast, plan).unwrap().model;
            let e = engine::simulate(&m);
            let r = reference::simulate(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            for out in [&e, &r.outcome] {
                for rep in &out.replications {
                    let admitted = if name.contains("zero-queue") {
                        0
                    } else if side == 0 {
                        2
                    } else {
                        3
                    };
                    let keys: BTreeSet<_> = rep.draws.iter().map(|d| d.key.clone()).collect();
                    let expected: BTreeSet<_> = (1..=admitted)
                        .map(|c| {
                            format!(
                                "relca-sim-v1|seed=0|rep={}|case={c}|site=init.case.z|n=1",
                                rep.replication
                            )
                        })
                        .collect();
                    assert_eq!(keys, expected);
                    assert_eq!(rep.draws.len(), admitted);
                }
            }
        }
    }
}
