//! ADR 0021 C2: independent arithmetic and AST consistency.
//! Current byte snapshots run separately under ADR 0055.
use relca_core::{ast, syntax};
#[path = "support/deferred_invoice.rs"]
mod deferred_invoice;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> Vec<u8> {
    fs::read(root().join(path)).unwrap_or_else(|e| panic!("missing artifact {path}: {e}"))
}

fn text(path: &str) -> String {
    String::from_utf8(read(path)).unwrap()
}

fn model(name: &str) -> String {
    format!("examples/invoice/{name}.relca")
}

fn invoke(args: &[String]) -> Output {
    assert!(root().join(&args[1]).is_file(), "missing model {}", args[1]);
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap()
}

fn report(name: &str, command: &str, extra: &[&str]) -> (Value, i32) {
    let mut args = vec![command.to_owned(), model(name)];
    args.extend(extra.iter().map(|s| (*s).to_owned()));
    let out = invoke(&args);
    (
        serde_json::from_slice(&out.stdout).unwrap(),
        out.status.code().unwrap(),
    )
}

fn expected_manifest() -> Value {
    let mut rows = Vec::new();
    let mut add = |id: String, args: Vec<String>, exit: i32| {
        rows.push(json!({
            "id": id, "model": args[1], "args": args,
            "expected_exit": exit
        }));
    };
    for (name, check_exit) in [("as-is", 0), ("to-be-p", 0), ("to-be-a", 1)] {
        for (command, exit) in [
            ("check", check_exit),
            ("explain", check_exit),
            ("simulate", 4),
            ("mutate", 4),
        ] {
            add(
                format!("{name}-{command}"),
                vec![command.into(), model(name)],
                exit,
            );
        }
    }
    for command in ["check", "explain"] {
        add(
            format!("to-be-c-{command}"),
            vec![command.into(), model("to-be-c")],
            3,
        );
    }
    for (mode, exit) in [("exhaustive", 1), ("simulation", 4)] {
        add(
            format!("compare-{mode}"),
            vec![
                "compare".into(),
                model("configurations"),
                "--plans".into(),
                "AsIs,ToBeP,ToBeA".into(),
                "--mode".into(),
                mode.into(),
            ],
            exit,
        );
    }
    add(
        "diff".into(),
        vec![
            "diff".into(),
            model("configurations"),
            "--from".into(),
            "AsIs".into(),
            "--to".into(),
            "ToBeA".into(),
        ],
        1,
    );
    json!(rows)
}

fn valid_manifest(v: &Value) -> bool {
    // Whole-object equality also closes the key schema, paths, ordering and exit expectations.
    v == &expected_manifest()
}

fn manifest() -> Value {
    let v = serde_json::from_slice(&read("examples/invoice/commands.json")).unwrap();
    assert!(valid_manifest(&v), "closed 17-invocation manifest differs");
    v
}

fn assert_snapshot_bytes(actual: &[u8], saved: &[u8], path: &str) {
    assert_eq!(actual, saved, "snapshot byte mismatch {path}");
}

#[test]
fn a_snapshot_calibration_detects_one_byte_missing_row_duplicate_and_unknown_key() {
    let out = invoke(&["check".into(), "fixtures/coop-cancel/model.relca".into()]);
    assert_eq!(out.status.code(), Some(0));
    assert_snapshot_bytes(&out.stdout, &out.stdout, "calibration unchanged stdout");
    let mut damaged = out.stdout.clone();
    assert_eq!(damaged.pop(), Some(b'\n'));
    damaged.push(b' '); // exactly one saved byte changed, no disk write
    assert!(
        std::panic::catch_unwind(|| {
            assert_snapshot_bytes(&out.stdout, &damaged, "calibration damaged saved stdout");
        })
        .is_err(),
        "K29: changed saved stdout must trip the actual snapshot assertion"
    );
    assert!(
        std::panic::catch_unwind(|| {
            assert_snapshot_bytes(b"", b"\n", "calibration empty stderr");
        })
        .is_err()
    );
    let good = manifest();
    assert!(valid_manifest(&good));
    let mut missing = good.clone();
    missing.as_array_mut().unwrap().remove(2); // simulate must not disappear
    assert!(!valid_manifest(&missing));
    let mut duplicate = good.clone();
    duplicate[13] = duplicate[12].clone(); // C explain must not become duplicate check
    assert!(!valid_manifest(&duplicate));
    let mut unknown = good;
    unknown[0]["update"] = json!(true);
    assert!(!valid_manifest(&unknown));
}

// AST projection: remove only source-position Span fields from derived Debug.
// All variants, lexemes, declaration order, duplicates and grouping remain.
// This is not textual equality of copied model bytes.
fn ast_structure(value: &impl Debug) -> String {
    let debug = format!("{value:?}");
    let mut rest = debug.as_str();
    let mut result = String::new();
    while let Some(start) = rest.find("span: Span { start: ") {
        result.push_str(&rest[..start]);
        let span = &rest[start..];
        let end = span.find('}').unwrap();
        let numbers = span["span: Span { start: ".len()..end].replace(", end: ", "");
        assert!(numbers.trim().bytes().all(|c| c.is_ascii_digit()));
        result.push_str("span: POSITION");
        rest = &span[end + 1..];
    }
    result.push_str(rest);
    result
}

fn common(m: &ast::Model) -> Vec<String> {
    m.items
        .iter()
        .filter(|i| !matches!(i, ast::Item::Process(_) | ast::Item::Configuration(_)))
        .map(ast_structure)
        .collect()
}

fn parsed(name: &str) -> ast::Model {
    syntax::parse(&text(&model(name))).unwrap()
}

#[test]
fn a_model_common_declarations_and_process_bodies_match_ast() {
    let configured = parsed("configurations");
    let probability_bounds: Vec<_> = configured
        .items
        .iter()
        .filter_map(|item| {
            if let ast::Item::Verify(verify) = item
                && let ast::Property::ProbabilityReached {
                    comparison,
                    bound,
                    within,
                    ..
                } = &verify.value
            {
                Some((*comparison, bound.value.as_str(), within.is_none()))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        probability_bounds,
        vec![(ast::Comparison::Le, "0.001", true)],
        "independent 1/1000 threshold"
    );
    let configured_processes: Vec<_> = configured
        .items
        .iter()
        .filter_map(|i| {
            if let ast::Item::Process(p) = i {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(configured_processes.len(), 3);
    for (name, process) in [
        ("as-is", "AsIsFlow"),
        ("to-be-p", "ToBePFlow"),
        ("to-be-a", "ToBeAFlow"),
    ] {
        let single = parsed(name);
        assert_eq!(single.version.value, configured.version.value);
        assert_eq!(
            common(&single),
            common(&configured),
            "common AST drift {name}"
        );
        assert!(
            !single
                .items
                .iter()
                .any(|i| matches!(i, ast::Item::Configuration(_)))
        );
        let processes: Vec<_> = single
            .items
            .iter()
            .filter_map(|i| {
                if let ast::Item::Process(p) = i {
                    Some(p)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(processes.len(), 1);
        let counterpart = configured_processes
            .iter()
            .find(|p| p.name.value == process)
            .unwrap();
        assert_eq!(
            ast_structure(&processes[0].body),
            ast_structure(&counterpart.body),
            "process AST drift {name}"
        );
    }
}

#[test]
fn a_model_ast_projection_keeps_semantic_changes_and_ignores_positions() {
    let a = syntax::parse("model M version \"0.2\" { type T = enum { high } }").unwrap();
    let spaced = syntax::parse("\nmodel M version \"0.2\" { type T = enum { high } }\n").unwrap();
    let changed = syntax::parse("model M version \"0.2\" { type T = enum { low } }").unwrap();
    assert_eq!(common(&a), common(&spaced));
    assert_ne!(common(&a), common(&changed));
}

fn named<'a>(v: &'a Value, array: &str, field: &str, name: &str) -> &'a Value {
    let rows: Vec<_> = v
        .pointer(array)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r[field] == name)
        .collect();
    assert_eq!(rows.len(), 1, "{array} {name}");
    rows[0]
}

fn range(v: &Value, field: &str, min: &str, max: &str) {
    assert_eq!(
        v[field],
        json!({"min":min,"max":max,"reason":null}),
        "{field}"
    );
}

fn probability(v: &Value, fraction: &str) {
    assert_eq!(v["probability"], json!({"min": fraction, "max": fraction}));
}

#[test]
fn a_model_check_preflight_only_c_has_expected_static_error() {
    for (name, selector, exit) in [
        ("as-is", None, 0),
        ("to-be-p", None, 0),
        ("to-be-a", None, 1),
        ("to-be-c", None, 3),
        ("configurations", Some("AsIs"), 0),
        ("configurations", Some("ToBeP"), 0),
        ("configurations", Some("ToBeA"), 1),
    ] {
        let extra = selector
            .map(|s| vec!["--configuration", s])
            .unwrap_or_default();
        let (v, actual_exit) = report(name, "check", &extra);
        assert_eq!(actual_exit, exit, "{name} {selector:?}");
        let errors: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .map(|d| d["code"].as_str().unwrap())
            .collect();
        assert_eq!(
            errors,
            if name == "to-be-c" {
                vec!["E-POL-LATENT"]
            } else {
                vec![]
            },
            "{name}"
        );
    }
}

#[test]
fn a_calc_live_reports_match_independent_fractions_time_and_work() {
    for (name, wrong, posted, returned, elapsed, human, clerk, work, verdict, separation) in [
        (
            "as-is",
            "93/125000",
            "122593/125000",
            "2407/125000",
            17,
            16,
            14,
            17,
            "satisfied",
            "satisfied",
        ),
        (
            "to-be-p",
            "93/125000",
            "122593/125000",
            "2407/125000",
            14,
            16,
            14,
            17,
            "satisfied",
            "satisfied",
        ),
        (
            "to-be-a",
            "231/125000",
            "119481/125000",
            "5519/125000",
            9,
            10,
            8,
            12,
            "violated",
            "vacuous",
        ),
    ] {
        let (v, _) = report(name, "check", &[]);
        deferred_invoice::assert_deferred(&v);
        let out = deferred_invoice::numeric_control(&text(&model(name)), "check", &[]);
        let numeric: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            out.status.code(),
            Some(if verdict == "satisfied" { 0 } else { 1 })
        );
        let p = named(&v, "/properties", "id", "probability reached #1");
        probability(p, wrong);
        assert_eq!(p["verdict"], verdict);
        assert_eq!(v["verdict"], verdict);
        let terminal = named(&v, "/terminals", "end", "posted");
        probability(terminal, posted);
        probability(named(&v, "/terminals", "end", "returned"), returned);
        assert_eq!(terminal["elapsed"], json!({"min":elapsed,"max":elapsed}));
        assert_eq!(terminal["work"], json!({"min":work,"max":work}));
        assert_eq!(
            v["terminals"].as_array().unwrap().len(),
            2,
            "exception is unreachable"
        );
        let c = named(&numeric, "/metrics/actors", "actor", "Clerk");
        range(c, "work", &clerk.to_string(), &(clerk + 1).to_string());
        range(
            named(&numeric, "/metrics/actors", "actor", "Approver"),
            "work",
            "0",
            "2",
        );
        range(
            &numeric["metrics"]["overall"],
            "named_people_work",
            &clerk.to_string(),
            &(human + 1).to_string(),
        );
        assert_eq!(clerk + 2, human); // same posted path: both checks pass then approval runs
        let sep: Vec<_> = v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["id"].as_str().unwrap().ends_with(" separation"))
            .collect();
        assert_eq!(sep.len(), 1);
        assert_eq!(sep[0]["verdict"], separation);
        for kind in [
            " invariant",
            " at_most_once",
            " authority_basis",
            " response",
            " contest",
            " contest_response",
        ] {
            let properties: Vec<_> = v["properties"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|p| p["id"].as_str().unwrap().ends_with(kind))
                .collect();
            assert!(!properties.is_empty(), "missing {kind}");
            assert!(properties.iter().all(|p| p["verdict"] == "satisfied"));
        }
        let (explain, _) = report(name, "explain", &[]);
        assert_eq!(
            explain["explanation"]["counterexamples"],
            json!([]),
            "probability is not a path cause"
        );
    }
}

#[test]
fn a_calc_document_values_are_fixed_separately_from_reports() {
    let calc = text("examples/invoice/calc.md");
    for line in [
        "| wrong | 93/125000 | 93/125000 | 231/125000 |",
        "| posted | 122593/125000 | 122593/125000 | 119481/125000 |",
        "| returned | 2407/125000 | 2407/125000 | 5519/125000 |",
        "| elapsed | 17 | 14 | 9 |",
        "| human | 16 | 16 | 10 |",
        "| clerk | 14 | 14 | 8 |",
        "| work | 17 | 17 | 12 |",
        "| capacity | 120/14 | 120/14 | 120/8 |",
        "| threshold | 1/1000 | 1/1000 | 1/1000 |",
        "| probability_verdict | satisfied | satisfied | violated |",
        "| separation_verdict | satisfied | satisfied | vacuous |",
    ] {
        assert!(
            calc.lines().any(|l| l == line),
            "missing independent calc row {line}"
        );
    }
}

const CLAIMS: &[&str] = &[
    "I12.1-receipt",
    "I12.1-cut",
    "I12.1-plans",
    "I12.2-state",
    "I12.2-people",
    "I12.2-record",
    "I12.2-requires",
    "I12.2-ledger",
    "I12.2-authority",
    "I12.3-kernels",
    "I12.3-independence",
    "I12.4-controls",
    "I12.4-vacuous",
    "I12.4-returned",
    "I12.4-quality",
    "I12.5-time",
    "I12.5-capacity",
    "I12.5-risk",
    "I12.5-decomposition",
    "I12.5-busyday",
    "I12.5-c",
    "I12.5-b",
    "I12.6-structure",
    "I12.6-origin",
    "I12.6-responsibility",
    "I12.6-refusal",
    "I12.6-lineage",
    "I12.6-wlin2",
    "I12.6-l",
    "I12.6-maintenance",
    "I12.6-burden",
    "I12.6-concentration",
    "I12.6-contest",
    "I12.6-contest-response",
    "I12.6-ai-path",
    "I12.6-704",
    "I12.6-705",
    "I12.6-706",
    "I12.6-708",
    "I12.3-mutation",
    "I12.5-simulation",
];

fn valid_claims(readme: &str) -> bool {
    let mut seen = BTreeSet::new();
    for line in readme.lines().filter(|l| l.starts_with("| I12.")) {
        let cells: Vec<_> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells.len() != 6 || cells.iter().any(|c| c.is_empty()) || !seen.insert(cells[0]) {
            return false;
        }
        match cells[2] {
            "unsupported"
                if ["未対応", "自動検査しない", "書けない", "実装しない"]
                    .iter()
                    .any(|reason| cells[3].contains(reason)) => {}
            "src" if cells[3].len() > 10 => {}
            "output" if cells[3].contains('/') || cells[3].contains("diff") => {}
            _ => {
                eprintln!("claim has no evidence or reason: {cells:?}");
                return false;
            }
        }
    }
    seen == CLAIMS.iter().copied().collect()
}

#[test]
fn a_claims_all_required_ids_have_evidence_or_unsupported_reason_and_boundary() {
    let readme = text("examples/invoice/README.md");
    assert!(
        valid_claims(&readme),
        "missing claim/evidence/reason/boundary"
    );
    assert!(
        !valid_claims(
            &readme
                .lines()
                .filter(|l| !l.starts_with("| I12.5-b | "))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        "calibration: missing To-Be B must fail"
    );
}

#[test]
fn a_no_fake_unknown_cost_quality_and_business_value_stay_unknown() {
    let readme = text("examples/invoice/README.md");
    for boundary in [
        "not_configured",
        "登録条件付き誤り率",
        "平均/P95",
        "安定性",
        "比較順位",
        "経営要約",
        "AI を介した変化",
        "未設定",
    ] {
        assert!(
            readme.contains(boundary),
            "missing no-fake boundary {boundary}"
        );
    }
    for name in ["as-is", "to-be-p", "to-be-a"] {
        let (v, _) = report(name, "check", &[]);
        for field in [
            "occupancy_cost",
            "start_cost",
            "observed_cost_total",
            "observed_cost_per_arrival",
        ] {
            assert_eq!(
                v["metrics"]["overall"][field],
                json!({"min":null,"max":null,"reason":"not_configured"})
            );
        }
        assert_eq!(
            v["metrics"]["overall"]["business_value"],
            json!({"value":null,"reason":"not_configured"})
        );
        for field in [
            "quality",
            "business_value",
            "quantiles",
            "calendar",
            "terminal_meaning",
        ] {
            assert!(
                v["metrics"]["unchecked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(field))
            );
        }
        for row in v["metrics"]["rates"]["actors"].as_array().unwrap() {
            assert!(row["per_minute"].is_null());
            assert_eq!(row["reason"], "not_configured");
        }
    }
}
