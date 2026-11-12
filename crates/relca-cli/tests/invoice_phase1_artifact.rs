//! C8: closed command inventory and hand arithmetic independent of ADR 0055 snapshots.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/response_boundary_oracle.rs"]
mod response_boundary;
use relca_core::{ast, syntax};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

#[path = "support/deferred_invoice.rs"]
mod deferred_invoice;
#[path = "support/invoice_c8_review.rs"]
mod review;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn text(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("missing {path}: {e}"))
}
fn source(name: &str) -> String {
    text(&format!("examples/invoice/{name}.relca"))
}
fn expected() -> Value {
    let mut rows = Vec::new();
    let mut add = |id: &str, model: &str, command: &str, extra: &[&str], exit: i32| {
        let model = format!("examples/invoice/{model}.relca");
        let mut args = vec![command.to_owned(), model.clone()];
        args.extend(extra.iter().map(|a| (*a).to_owned()));
        rows.push(json!({"id":id,"model":model,"args":args,"expected_exit":exit}));
    };
    for (id, cfg, exit) in [
        ("p1-as-is-check", "AsIs", 0),
        ("p1-to-be-p-check", "ToBeP", 0),
        ("p1-to-be-a-check", "ToBeA", 1),
    ] {
        add(id, "phase1", "check", &["--configuration", cfg], exit);
    }
    for (mode, exit) in [("exhaustive", 1), ("simulation", 4)] {
        add(
            &format!("p1-compare-{mode}"),
            "phase1",
            "compare",
            &["--plans", "AsIs,ToBeP,ToBeA", "--mode", mode],
            exit,
        );
    }
    add(
        "p1-diff",
        "phase1",
        "diff",
        &["--from", "AsIs", "--to", "ToBeA"],
        1,
    );
    for (command, exit) in [("check", 1), ("explain", 1), ("simulate", 4), ("mutate", 4)] {
        add(
            &format!("p1-a-{command}"),
            "to-be-a-phase1",
            command,
            &[],
            exit,
        );
    }
    for (id, cfg, exit) in [("rule-a-check", "ToBeA", 5), ("rule-b-check", "ToBeB", 1)] {
        add(id, "rules", "check", &["--configuration", cfg], exit);
    }
    add(
        "rule-diff",
        "rules",
        "diff",
        &["--from", "ToBeA", "--to", "ToBeB"],
        1,
    );
    for (id, cfg) in [
        ("as-is", "AsIs"),
        ("to-be-p", "ToBeP"),
        ("to-be-a", "ToBeA"),
    ] {
        add(
            &format!("busyday-{id}-simulate"),
            "busyday",
            "simulate",
            &["--configuration", cfg, "--scenario", "BusyDay"],
            4,
        );
    }
    add(
        "busyday-compare-simulation",
        "busyday",
        "compare",
        &[
            "--plans",
            "AsIs,ToBeP,ToBeA",
            "--mode",
            "simulation",
            "--scenario",
            "BusyDay",
        ],
        4,
    );
    add(
        "p1-compare-quantile",
        "quantile-compare",
        "compare",
        &["--plans", "AsIs,ToBeP,ToBeA", "--mode", "exhaustive"],
        1,
    );
    json!(rows)
}
fn manifest() -> Value {
    let rows: Value = serde_json::from_str(&text("examples/invoice/commands-phase1.json")).unwrap();
    assert_eq!(rows, expected(), "closed eighteen-invocation manifest");
    rows
}
fn invoke(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap()
}
fn live(id: &str) -> Value {
    let rows = manifest();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap();
    let args: Vec<_> = row["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_owned())
        .collect();
    let out = invoke(&args);
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        out.status.code(),
        Some(row["expected_exit"].as_i64().unwrap() as i32),
        "{id}: {v}"
    );
    v
}
fn edited(src: &str, command: &str, extra: &[&str]) -> (Value, i32) {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = root().join(".tmp");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "invoice-c8-{}-{}.relca",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(&path)
        .args(extra)
        .current_dir(root())
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();
    (
        serde_json::from_slice(&out.stdout).unwrap(),
        out.status.code().unwrap(),
    )
}

fn numeric_control(id: &str) -> Value {
    let raw = live(id);
    if let Some(plans) = raw["plans"].as_array() {
        for p in plans {
            deferred_invoice::assert_deferred(&p["report"]);
        }
    } else {
        deferred_invoice::assert_deferred(&raw);
    }
    let rows = manifest();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap();
    let args: Vec<_> = row["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    let out = deferred_invoice::numeric_control(&text(args[1]), args[0], &args[2..]);
    assert_eq!(
        out.status.code(),
        Some(row["expected_exit"].as_i64().unwrap() as i32)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn change(src: &str, from: &str, to: &str) -> String {
    assert!(src.contains(from), "missing single-edit witness {from}");
    assert_ne!(from, to);
    src.replace(from, to)
}
fn named<'a>(rows: &'a Value, field: &str, name: &str) -> &'a Value {
    let found: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r[field] == name)
        .collect();
    assert_eq!(found.len(), 1, "unique {field}={name}: {rows}");
    found[0]
}
fn exact(n: &str) -> Value {
    json!({"lower":n,"upper":n,"basis":"exact","reason":null})
}
fn range(lo: &str, hi: &str) -> Value {
    json!({"min":lo,"max":hi,"reason":null})
}
fn assert_raw(actual: &[u8], saved: &[u8]) {
    assert_eq!(actual, saved, "unprojected artifact bytes");
}
fn without_atomic(s: &str) -> String {
    let s = change(s, "state keys: set<CaseId>", "");
    let s = change(
        &s,
        "writes [keys,registered]",
        "writes registered; finish { registered = true }",
    );
    change(
        &s,
        "atomic { if key in keys { result rejected_duplicate } else { keys += key; registered = true; result committed } }",
        "result nondet { committed }",
    )
}

#[test]
fn closure_new_eighteen_old_seventeen_models_and_disjoint_output_names() {
    let rows = manifest();
    let old: Value = serde_json::from_str(&text("examples/invoice/commands.json")).unwrap();
    let mut old_expected = Vec::new();
    for (model, exit) in [("as-is", 0), ("to-be-p", 0), ("to-be-a", 1)] {
        for (cmd, code) in [
            ("check", exit),
            ("explain", exit),
            ("simulate", 4),
            ("mutate", 4),
        ] {
            old_expected.push((
                format!("{model}-{cmd}"),
                json!([cmd, format!("examples/invoice/{model}.relca")]),
                code,
            ));
        }
    }
    for cmd in ["check", "explain"] {
        old_expected.push((
            format!("to-be-c-{cmd}"),
            json!([cmd, "examples/invoice/to-be-c.relca"]),
            3,
        ));
    }
    for (mode, exit) in [("exhaustive", 1), ("simulation", 4)] {
        old_expected.push((
            format!("compare-{mode}"),
            json!([
                "compare",
                "examples/invoice/configurations.relca",
                "--plans",
                "AsIs,ToBeP,ToBeA",
                "--mode",
                mode
            ]),
            exit,
        ));
    }
    old_expected.push((
        "diff".into(),
        json!([
            "diff",
            "examples/invoice/configurations.relca",
            "--from",
            "AsIs",
            "--to",
            "ToBeA"
        ]),
        1,
    ));
    assert_eq!(old.as_array().unwrap().len(), 17);
    for (row, (id, args, exit)) in old.as_array().unwrap().iter().zip(old_expected) {
        assert_eq!(row["id"], id);
        assert_eq!(row["args"], args);
        assert_eq!(row["expected_exit"], exit);
    }
    let mut ids = BTreeSet::new();
    let mut models = BTreeSet::new();
    for row in old
        .as_array()
        .unwrap()
        .iter()
        .chain(rows.as_array().unwrap())
    {
        assert!(ids.insert(row["id"].as_str().unwrap()));
        assert_eq!(row["model"], row["args"][1]);
        assert!(root().join(row["model"].as_str().unwrap()).is_file());
    }
    for row in rows.as_array().unwrap() {
        models.insert(row["model"].as_str().unwrap());
    }
    assert_eq!(
        models,
        [
            "phase1",
            "to-be-a-phase1",
            "rules",
            "busyday",
            "quantile-compare"
        ]
        .map(|m| format!("examples/invoice/{m}.relca"))
        .iter()
        .map(String::as_str)
        .collect()
    );
    let good = expected();
    let mut bad = good.clone();
    bad.as_array_mut().unwrap().remove(0);
    assert_ne!(bad, good);
    let mut bad = good.clone();
    bad[1] = bad[0].clone();
    assert_ne!(bad, good);
    let mut bad = good.clone();
    bad[0]["update"] = json!(true);
    assert_ne!(bad, good);
}

#[test]
fn snapshot_comparator_rejects_one_byte_and_empty_stderr_corruption() {
    assert_raw(b"{}\n", b"{}\n");
    for (actual, bad) in [
        (b"{}\n".as_slice(), b"{} ".as_slice()),
        (b"".as_slice(), b"\n".as_slice()),
    ] {
        assert!(std::panic::catch_unwind(|| assert_raw(actual, bad)).is_err());
    }
}

fn assert_hand_calc_document(s: &str) {
    for line in [
        "| WrongPerArrival | 93/125000 | 93/125000 | 231/125000 |",
        "| Resolved expected_count | 1 | 1 | 1 |",
        "| PostedP95 | 17 | 14 | 9 |",
        "| PostedReachedP95 | 17 | 14 | 9 |",
        "| poV | 6 | 6 | 1 |",
        "| receiptV | 9 | 6 | 1 |",
    ] {
        assert!(s.lines().any(|l| l == line), "independent hand row {line}");
    }
    assert!(s.contains("ceil(480/4)=120"));
    assert!(!s.contains("Ruleを持つモデルでは通常の品質は計算しない"));
    assert!(s.contains("品質を撤回せず宣言どおり計算する"));
    assert!(s.contains("条件付き免除 To-Be B の誤登録率ではない"));
}

#[test]
fn hand_calc_document_is_fixed_separately_from_snapshots() {
    let s = text("examples/invoice/calc.md");
    assert_hand_calc_document(&s);
    let missing_distinction = s.replace("条件付き免除 To-Be B の誤登録率ではない", "");
    assert!(std::panic::catch_unwind(|| assert_hand_calc_document(&missing_distinction)).is_err());
    let obsolete = format!("{s}\nRuleを持つモデルでは通常の品質は計算しない\n");
    assert!(std::panic::catch_unwind(|| assert_hand_calc_document(&obsolete)).is_err());
}

#[test]
fn live_quality_time_waits_and_quantiles_follow_hand_fractions() {
    for (id, wrong, posted, returned, lead, work, clerk, human, predecessors) in [
        (
            "p1-as-is-check",
            "93/125000",
            "122593/125000",
            "2407/125000",
            17,
            17,
            14,
            16,
            [6, 9, 14, 16],
        ),
        (
            "p1-to-be-p-check",
            "93/125000",
            "122593/125000",
            "2407/125000",
            14,
            17,
            14,
            16,
            [6, 6, 11, 13],
        ),
        (
            "p1-to-be-a-check",
            "231/125000",
            "119481/125000",
            "5519/125000",
            9,
            12,
            8,
            10,
            [1, 1, 6, 8],
        ),
    ] {
        let v = numeric_control(id);
        assert_eq!(v["bounds"]["truncated"], false);
        let q = &v["quality"];
        for (name, n) in [
            ("WrongPosting", wrong),
            ("Resolved", "1"),
            ("Posted", posted),
        ] {
            let p = named(&q["properties"], "name", name);
            assert_eq!(p["expected_count"], exact(n));
            assert_eq!(
                p["cases"],
                json!([{"case_id":1,"arrival":0,"min":n,"max":n,"reason":null}])
            );
        }
        let ratio = named(&q["cohort"], "name", "WrongPerArrival");
        assert_eq!(ratio["denominator"], json!({"kind":"arrivals","count":1}));
        assert_eq!(ratio["form"], "ratio_of_expected_counts");
        assert_eq!(ratio["value"], exact(wrong));
        for r in q["outcomes_per_arrival"].as_array().unwrap() {
            let n = if r["end"] == "posted" {
                posted
            } else if r["end"] == "returned" {
                returned
            } else {
                "0"
            };
            assert_eq!(r["rate"], exact(n));
        }
        for (name, form) in [
            ("PostedP95", "all_arrivals"),
            ("PostedReachedP95", "reached_only"),
        ] {
            let r = named(&q["quantiles"], "name", name);
            assert_eq!(r["value"], exact(&lead.to_string()));
            assert_eq!(r["p"], "19/20");
            assert_eq!(r["form"], form);
            assert!(r["lower_bound"].is_null());
        }
        let terminal = named(&v["terminals"], "end", "posted");
        assert_eq!(terminal["elapsed"], json!({"min":lead,"max":lead}));
        assert_eq!(terminal["work"], json!({"min":work,"max":work}));
        let m = &v["metrics"];
        let case = &m["cases"][0];
        assert_eq!(case["case_id"], 1);
        let b = &named(&case["terminals"], "end", "posted")["breakdown"];
        assert_eq!(b["in_service"], range(&lead.to_string(), &lead.to_string()));
        for k in ["external", "contest_route", "intentional", "unattributed"] {
            assert_eq!(b[k], range("0", "0"));
        }
        for k in ["actor_capacity", "resource", "executor", "combined"] {
            assert_eq!(b["queue"][k], range("0", "0"));
        }
        for a in m["actors"].as_array().unwrap() {
            for p in [
                "/queue/wait_total/stats",
                "/queue/length_max",
                "/queue/length_time_average/stats",
            ] {
                assert_eq!(a.pointer(p).unwrap(), &range("0", "0"));
            }
        }
        for (binding, n) in ["poV", "receiptV", "approvalV", "posted"]
            .into_iter()
            .zip(predecessors)
        {
            let t = named(&case["tasks"], "binding", binding);
            assert_eq!(t["predecessor_wait"], range(&n.to_string(), &n.to_string()));
            for c in t["wait_causes"].as_object().unwrap().values() {
                assert_eq!(c, &range("0", "0"));
            }
        }
        if lead != 17 {
            let j = named(&case["joins"], "fork", "checks");
            assert_eq!(j["mode"], "all");
            assert_eq!(
                named(&j["branches"], "binding", "poV")["join_wait"],
                range("2", "2")
            );
            assert_eq!(
                named(&j["branches"], "binding", "receiptV")["join_wait"],
                range("0", "0")
            );
        }
        assert_eq!(
            named(&m["actors"], "actor", "Clerk")["work"],
            range(&clerk.to_string(), &(clerk + 1).to_string())
        );
        assert_eq!(
            m["overall"]["named_people_work"],
            range(&clerk.to_string(), &(human + 1).to_string())
        );
        for f in [
            "occupancy_cost",
            "start_cost",
            "observed_cost_total",
            "observed_cost_per_arrival",
        ] {
            assert_eq!(
                m["overall"][f],
                json!({"min":null,"max":null,"reason":"not_configured"})
            );
        }
    }
}

#[test]
fn live_agency_retention_origin_via_and_declared_boundaries() {
    for (id, enter, extract) in [
        ("p1-as-is-check", 1, 0),
        ("p1-to-be-p-check", 1, 0),
        ("p1-to-be-a-check", 0, 1),
    ] {
        let v = live(id);
        let a = &v["agency"];
        let rows = &a["viewpoints"]["retention"]["capabilities"];
        for (cap, n) in [("Clerk.enter", enter), ("Extractor.extract", extract)] {
            let r = named(rows, "capability", cap);
            assert_eq!(r["normal"], json!({"min":n,"max":n}));
            assert_eq!(r["basis"], "unknown");
        }
        if enter == 0 {
            assert_eq!(
                named(rows, "capability", "Clerk.enter")["fallback"],
                "kept_by_intent"
            );
        }
        assert_eq!(
            named(&a["origin"], "actor", "Approver")["direct"],
            json!([])
        );
        assert_eq!(
            named(&a["origin"], "actor", "Approver")["indirect"],
            json!(["Invoice", "PurchaseOrder", "ReceiptRecord"])
        );
        assert_eq!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["code"] == "W-LIN-2")
                .count(),
            1
        );
        assert_eq!(v["cut_context"]["horizon"].as_array().unwrap().len(), 2);
        if extract == 1 {
            assert!(v["via_model_paths"].is_object());
            assert!(v["via_model_paths"].to_string().contains("doc-extract@v3"));
        }
        let once: Vec<_> = v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["id"].as_str().unwrap().ends_with(" at_most_once"))
            .collect();
        assert_eq!(once.len(), 1);
        assert_eq!(once[0]["verdict"], "satisfied");
        assert!(once[0].get("notes").is_none());
    }
}

#[test]
fn live_hard_and_human_objective_do_not_make_overlapping_intervals_dominate() {
    let v = numeric_control("p1-compare-exhaustive");
    let e = &v["evaluation"];
    for (cfg, status, hard) in [
        ("AsIs", "admissible", "satisfied"),
        ("ToBeP", "admissible", "satisfied"),
        ("ToBeA", "excluded", "violated"),
    ] {
        let p = named(&e["plans"], "configuration", cfg);
        assert_eq!(p["status"], status);
        assert_eq!(p["hard_verdict"], hard);
    }
    assert_eq!(e["pareto"]["among"], json!(["AsIs", "ToBeP"]));
    assert_eq!(e["pareto"]["non_dominated"], json!(["AsIs", "ToBeP"]));
    assert_eq!(e["pareto"]["dominated"], json!([]));
    assert_eq!(
        e["objectives"][0]["objective"],
        "minimize metrics.overall.named_people_work"
    );
}

#[test]
fn c7_dependent_quantile_objective_is_not_silently_dropped() {
    let v = numeric_control("p1-compare-quantile");
    let e = &v["evaluation"];
    assert_eq!(e["objectives"].as_array().unwrap().len(), 1);
    assert_eq!(
        e["objectives"][0]["objective"],
        "minimize quantile.PostedP95"
    );
    assert_eq!(e["pareto"]["non_dominated"], json!(["ToBeP"]));
    assert_eq!(
        e["pareto"]["dominated"],
        json!([{"configuration":"AsIs","dominated_by":["ToBeP"]}])
    );
    for (cfg, n) in [("AsIs", "17"), ("ToBeP", "14"), ("ToBeA", "9")] {
        assert_quantile_objective_cell(
            named(&e["objectives"][0]["values"], "configuration", cfg),
            cfg,
            n,
        );
    }
}

fn assert_quantile_objective_cell(actual: &Value, configuration: &str, n: &str) {
    // ObjectiveCell is flat, unlike quality.quantiles[*].value (ADR0047/0049).
    assert_eq!(
        actual,
        &json!({"configuration":configuration,"lower":n,"upper":n,"basis":"exact","reason":null})
    );
}

#[test]
fn quantile_objective_cell_checker_rejects_wrong_bounds_and_nested_quality_shape() {
    let good =
        json!({"configuration":"AsIs","lower":"17","upper":"17","basis":"exact","reason":null});
    assert_quantile_objective_cell(&good, "AsIs", "17");
    let mut wrong = good.clone();
    wrong["upper"] = json!("18");
    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("reason");
    for bad in [
        wrong,
        missing,
        json!({"configuration":"AsIs","value":exact("17")}),
    ] {
        assert!(
            std::panic::catch_unwind(|| assert_quantile_objective_cell(&bad, "AsIs", "17"))
                .is_err()
        );
    }
}

#[test]
fn live_rule_is_static_full_exemption_not_conditional_quality() {
    for (id, effective, required) in [
        ("rule-a-check", "r2026-04", true),
        ("rule-b-check", "r2026-05", false),
    ] {
        let v = numeric_control(id);
        let r = &named(&v["actors"], "name", "ApprovalRule")["rule"];
        assert_eq!(r["version"]["effective"], effective);
        assert_eq!(
            named(&r["values"], "name", "approval_required")["effective"],
            required
        );
        assert!(!v["quality"]["checked"].as_array().unwrap().is_empty());
        assert_eq!(
            named(&v["quality"]["cohort"], "name", "WrongPerArrival")["value"],
            exact(if required { "231/125000" } else { "81/12500" })
        );
        assert!(
            !v["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"] == "quality.rule")
        );
        assert!(
            !v["terminals"].as_array().unwrap().is_empty(),
            "quality withdrawal must not stop execution"
        );
    }
    assert!(
        live("rule-diff")["differences"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "rule" && r["target"] == "ApprovalRule")
    );
    let s = source("rules");
    for bad in [
        change(&s, "by cfo under RuleChange", "by a1 under RuleChange"),
        change(
            &s,
            "under ApprovalRule \"r2026-04\"",
            "under ApprovalRule \"r2026-05\"",
        ),
    ] {
        let (v, exit) = edited(&bad, "check", &["--configuration", "ToBeB"]);
        assert_eq!(exit, 3);
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-CON-0")
        );
    }
}

#[test]
fn live_busyday_declares_120_arrivals_and_rejects_unresolved_choices() {
    for id in [
        "busyday-as-is-simulate",
        "busyday-to-be-p-simulate",
        "busyday-to-be-a-simulate",
    ] {
        let v = live(id);
        assert_eq!(v["verdict"], "unsupported");
        assert_eq!(
            v["scenario"]["arrivals"],
            json!({"form":"fixed_every","every":4,"from":0,"until":480,"count":120})
        );
        assert_eq!(v["scenario"]["horizon"], 960);
        assert_eq!(v["scenario"]["drain"], json!({"from":476,"until":960}));
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0"
                    && d["message"].as_str().unwrap().contains("successor actions"))
        );
    }
    assert_eq!(live("busyday-compare-simulation")["verdict"], "unsupported");
}

#[test]
fn live_mutation_seven_kinds_keeps_three_unsupported_and_incomplete_run() {
    let v = live("p1-a-mutate");
    let ms = &v["mutations"];
    let unsupported: BTreeMap<_, _> = ms
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["status"] == "unsupported")
        .map(|m| (m["kind"].as_str().unwrap(), m["reason"].as_str().unwrap()))
        .collect();
    assert_eq!(
        unsupported,
        BTreeMap::from([("stale_definition_write", "rule_version_unavailable"),])
    );
    response_boundary::assert_not_applicable(&source("to-be-a-phase1"), &v["mutations"]);
    let k = named(ms, "kind", "old_promise_denominator_removed");
    assert_eq!(k["status"], "generated");
    assert_eq!(k["mutants"].as_array().unwrap().len(), 3);
    assert!(
        k["mutants"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["result"] == "detected")
    );
    assert_eq!(v["run"]["completion"], "inconclusive");
    assert_eq!(v["run"]["reason"], "mutant_incomplete");
}

#[test]
fn live_single_fault_keeps_truth_hidden() {
    let s = source("to-be-a-phase1");
    let bad = change(
        &s,
        "if poV == pass && receiptV == pass",
        "if Invoice.difficulty == difficult",
    );
    let (v, exit) = edited(&bad, "check", &[]);
    assert_eq!(exit, 3);
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-POL-LATENT")
    );
}

#[test]
fn live_duplicate_key_is_atomic_not_a_second_commit() {
    let s = source("to-be-a-phase1");
    let double = change(
        &s,
        "if posted == committed { end posted } else { end exception }",
        "again = Ledger.post(case.id,poV,receiptV,approvalV)\nif again == rejected_duplicate { end posted } else { end exception }",
    );
    let (v, exit) = edited(&double, "check", &[]);
    assert_eq!(exit, 1, "{v}");
    assert_eq!(
        named(&v["terminals"], "end", "posted")["elapsed"],
        json!({"min":10,"max":10})
    );
    let once = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"].as_str().unwrap().ends_with(" at_most_once"))
        .unwrap();
    assert_eq!(once["verdict"], "satisfied");
}

#[test]
fn live_removing_via_cannot_infer_ai_from_actor_name() {
    let s = source("to-be-a-phase1");
    let no_via = change(&s, "via model \"doc-extract@v3\";", "");
    let (v, exit) = edited(&no_via, "check", &[]);
    assert_eq!(exit, 1);
    assert!(v.get("via_model_paths").is_none());
}

fn structure(v: &impl std::fmt::Debug) -> String {
    let debug = format!("{v:?}");
    let mut s = debug.as_str();
    let mut result = String::new();
    // Some AST variants carry a bare Span (e.g. ControlParty), not a
    // field named span. Mask positions in both forms, never semantic fields.
    while let Some(i) = s.find("Span { start: ") {
        result.push_str(&s[..i]);
        let end = s[i..].find('}').unwrap() + i;
        result.push_str("POSITION");
        s = &s[end + 1..];
    }
    result.push_str(s);
    result
}
#[test]
fn model_common_ast_and_a_process_are_identical_across_supplements() {
    let parse = |n: &str| syntax::parse(&source(n)).unwrap();
    let common = |m: &ast::Model| {
        m.items
            .iter()
            .filter(|i| {
                !matches!(
                    i,
                    ast::Item::Process(_) | ast::Item::Configuration(_) | ast::Item::Compare(_)
                )
            })
            .map(structure)
            .collect::<Vec<_>>()
    };
    let p = parse("phase1");
    let a = parse("to-be-a-phase1");
    // Only the C7-dependent objective token is masked for this AST comparison.
    // The unmodified source is separately executed by c7_dependent_* above.
    let q_source = change(
        &source("quantile-compare"),
        "objectives { minimize quantile.PostedP95 }",
        "objectives { minimize metrics.overall.named_people_work }",
    );
    let q = syntax::parse(&q_source).unwrap();
    assert_eq!(structure(&p), structure(&q));
    assert_eq!(common(&p), common(&a));
    assert_eq!(common(&p), common(&q));
    let flow = |m: &ast::Model| {
        m.items
            .iter()
            .filter_map(|i| {
                if let ast::Item::Process(p) = i {
                    if p.name.value == "ToBeAFlow" {
                        Some(structure(&p.body))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(flow(&p), flow(&a));
    assert_eq!(flow(&p), flow(&q));
}

#[test]
fn control_without_atomic_checks_hand_numbers_but_not_duplicate_mechanism() {
    // A deliberately narrower substrate control, never substituted for the
    // combined-atomic expectations above. No snapshot is used as an oracle.
    let s = deferred_invoice::without_defer(&without_atomic(&source("phase1")));
    for (cfg, code, wrong, lead, work, human, clerk) in [
        ("AsIs", 0, "93/125000", 17, 17, 16, 14),
        ("ToBeP", 0, "93/125000", 14, 17, 16, 14),
        ("ToBeA", 1, "231/125000", 9, 12, 10, 8),
    ] {
        let (v, exit) = edited(&s, "check", &["--configuration", cfg]);
        assert_eq!(exit, code, "{v}");
        assert_eq!(
            named(&v["quality"]["cohort"], "name", "WrongPerArrival")["value"],
            exact(wrong)
        );
        assert_eq!(
            named(&v["quality"]["properties"], "name", "Resolved")["expected_count"],
            exact("1")
        );
        for q in ["PostedP95", "PostedReachedP95"] {
            assert_eq!(
                named(&v["quality"]["quantiles"], "name", q)["value"],
                exact(&lead.to_string())
            );
        }
        let t = named(&v["terminals"], "end", "posted");
        assert_eq!(t["elapsed"], json!({"min":lead,"max":lead}));
        assert_eq!(t["work"], json!({"min":work,"max":work}));
        let b = &named(&v["metrics"]["cases"][0]["terminals"], "end", "posted")["breakdown"];
        assert_eq!(b["in_service"], range(&lead.to_string(), &lead.to_string()));
        assert_eq!(
            v["metrics"]["overall"]["named_people_work"],
            range(&clerk.to_string(), &(human + 1).to_string())
        );
    }
}

// Minimal semantic control and single-change RED from ADR0035 Agency meaning.
// The one-valued conditional kernel and the one-valued nondeterministic result
// have identical execution semantics here. Only the selected given-bearing
// kernel exposes the combination; the two Capabilities are distinct.
const ATOMIC_CONDITIONAL: &str = r#"model AtomicConditional version "0.2" {
  type V = enum { pass }
  type Ack = enum { committed, rejected_duplicate }
  latent case { flag: bool = true }
  kernel K given [flag] { true -> { 1: pass }; false -> { 1: pass } }
  actor Ledger { capacity 1; state keys: set<CaseId>
    capability prepare() -> V { duration fixed 1min; kernel_reads latent [flag]; outcomes kernel K }
    capability post(key:CaseId) -> Ack {
      commits; duration fixed 1min; writes keys
      atomic { if key in keys { result rejected_duplicate } else { keys += key; result committed } }
    }
  }
  process P { d = Ledger.prepare(); a = Ledger.post(case.id); end done }
  cut C { include [Ledger] }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}"#;
fn assert_atomic_agency(v: &Value, code: i32) {
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["verdict"], "satisfied");
    assert!(v["agency"].is_object());
    assert!(
        v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guards"))
    );
    assert_eq!(
        named(&v["terminals"], "end", "done")["elapsed"],
        json!({"min":2,"max":2})
    );
    assert_eq!(
        named(&v["terminals"], "end", "done")["work"],
        json!({"min":2,"max":2})
    );
    assert_eq!(v["bounds"]["truncated"], false);
}
#[test]
fn atomic_conditional_kernel_does_not_erase_agency() {
    let (v, code) = edited(ATOMIC_CONDITIONAL, "check", &[]);
    assert_atomic_agency(&v, code);
}
#[test]
fn atomic_kernel_free_control_preserves_agency() {
    let s = change(
        ATOMIC_CONDITIONAL,
        "outcomes kernel K",
        "result nondet { pass }",
    );
    let (v, code) = edited(&s, "check", &[]);
    assert_atomic_agency(&v, code);
    let out = invoke(&[
        "check".into(),
        "fixtures/aset-atomic-probability/model.relca".into(),
    ]);
    assert_eq!(out.status.code(), Some(0));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["agency"].is_object());
    assert!(
        v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guards"))
    );
}

#[test]
fn declared_record_id_requires_cancel_and_atomic_are_not_silently_omitted() {
    let s = source("to-be-a-phase1");
    for witness in [
        "type Entry = record { confidence: enum { high, low } }",
        "input_by: optional<PersonId>",
        "finish { input_by = executor.id }",
        "requires po == pass && receipt == pass else fail",
        "requires approval == pass && po == pass && receipt == pass else held",
        "state keys: set<CaseId>",
        "atomic { if key in keys { result rejected_duplicate } else { keys += key; registered = true; result committed } }",
        "cancel forbidden_after_start",
        "via model \"doc-extract@v3\"",
        "responsible a1 for (registered -> approval == pass)",
        "responsible s1 for Extractor.maintenance",
    ] {
        assert!(s.contains(witness), "missing source contract {witness}");
    }
    assert_eq!(s.matches("cancel cooperative ack_within 1min").count(), 5);
    syntax::parse(&s).unwrap();
}

#[test]
fn control_rule_without_atomic_checks_static_amendment_and_normal_quality() {
    let s = deferred_invoice::without_defer(&without_atomic(&source("rules")));
    for (cfg, code, effective) in [("ToBeA", 5, "r2026-04"), ("ToBeB", 1, "r2026-05")] {
        let (v, exit) = edited(&s, "check", &["--configuration", cfg]);
        assert_eq!(exit, code, "{v}");
        let r = &named(&v["actors"], "name", "ApprovalRule")["rule"];
        assert_eq!(r["version"]["effective"], effective);
        assert!(!v["quality"]["checked"].as_array().unwrap().is_empty());
        assert_eq!(
            named(&v["quality"]["cohort"], "name", "WrongPerArrival")["value"],
            exact(if cfg == "ToBeA" {
                "231/125000"
            } else {
                "81/12500"
            })
        );
        assert!(
            !v["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"] == "quality.rule")
        );
        assert!(!v["terminals"].as_array().unwrap().is_empty());
    }
    for bad in [
        change(&s, "by cfo under RuleChange", "by a1 under RuleChange"),
        change(
            &s,
            "under ApprovalRule \"r2026-04\"",
            "under ApprovalRule \"r2026-05\"",
        ),
    ] {
        let (v, exit) = edited(&bad, "check", &["--configuration", "ToBeB"]);
        assert_eq!(exit, 3);
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-CON-0")
        );
    }
}

#[test]
fn readme_phase1_pointers_resolve_separately_from_legacy_colon_pointers() {
    let mut reports = BTreeMap::new();
    let readme = text("examples/invoice/README.md");
    let mut count = 0;
    for row in readme.lines().filter(|l| l.starts_with("| I12.")) {
        let cells: Vec<_> = row.split('|').map(str::trim).collect();
        assert_eq!(cells.len(), 8);
        for clause in cells[6].split(';') {
            if let Some((id, pointers)) = clause.trim().split_once(" → ") {
                let v = reports.entry(id.to_owned()).or_insert_with(|| live(id));
                for token in pointers.split_whitespace().filter(|t| t.starts_with('/')) {
                    let p = token
                        .split(|c: char| !c.is_ascii() || c == ',')
                        .next()
                        .unwrap();
                    let value = v
                        .pointer(p)
                        .unwrap_or_else(|| panic!("{} {id} {p}", cells[1]));
                    assert!(!value.is_null(), "{} {id} {p}", cells[1]);
                    count += 1;
                }
            }
        }
    }
    assert!(
        count >= 25,
        "supplemental evidence must not silently disappear"
    );
}
