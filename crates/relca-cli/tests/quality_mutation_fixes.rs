//! ADR 0045 mutation survivors on the real binary (P13, P35, P35r, O15, P34)
//! and the tightened quality assumption block of the closed matcher (O11).
//! The hand values are shared with the core test; the inputs live in the test
//! so the closed fixture population is unchanged.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/quality_mutation_inputs.rs"]
mod inputs;
use serde_json::Value;
use std::{fs, process::Command};

fn run(name: &str, src: &str, command: &str) -> (Value, i32) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!(
            "p{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
        .join("quality-mutation-fixes");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.relca"));
    fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(&path)
        .output()
        .unwrap();
    (
        serde_json::from_slice(&out.stdout).unwrap(),
        out.status.code().unwrap(),
    )
}

fn codes(v: &Value) -> Vec<&str> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect()
}

#[test]
fn binary_quality_verdict_and_exit_match_hand_values_collect_all() {
    let mut failures = vec![];
    for input in inputs::all() {
        let (v, exit) = run(input.name, &input.source, input.command);
        if v["quality"] != input.quality {
            failures.push(format!("{}: quality {}", input.name, v["quality"]));
        }
        if v["verdict"] != input.verdict || exit != input.exit {
            failures.push(format!(
                "{}: verdict {} exit {exit}",
                input.name, v["verdict"]
            ));
        }
    }
    assert!(failures.is_empty(), "hand mismatches: {failures:#?}");
}

/// O15: the sampled deadlock is reported as such (W-DEADLOCK-0 makes the
/// root violated), not folded into unfinished.
#[test]
fn sampled_deadlock_keeps_its_warning() {
    let input = inputs::deadlock_simulation();
    let (v, _) = run(input.name, &input.source, input.command);
    assert!(codes(&v).contains(&"W-DEADLOCK-0"), "{:?}", codes(&v));
}

/// P34: a quality-only numeric cut leaves the existing cut flag and the
/// budget warning untouched.
#[test]
fn quality_only_numeric_cut_is_not_an_existing_cut() {
    for input in [
        inputs::quality_only_numeric(),
        inputs::quality_only_numeric_arrivals(),
    ] {
        let (v, _) = run(input.name, &input.source, input.command);
        assert_eq!(v["bounds"]["truncated"], false, "{}", input.name);
        assert!(!codes(&v).contains(&"W-BUDGET-0"), "{:?}", codes(&v));
    }
}

/// O11: the four quality rows are one block immediately before
/// probability.scheduler when that row exists. The same block moved after
/// probability.scheduler (to the very end) is refused by the closed matcher.
#[test]
fn quality_block_after_probability_scheduler_is_refused() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/qual-contention");
    let expected: Value =
        serde_json::from_slice(&fs::read(root.join("expected.json")).unwrap()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("check")
        .arg(root.join("model.relca"))
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let invocation = fixture_match::Invocation::parse(None).unwrap();
    invocation.compare(&expected, &v).unwrap();

    let rows = v["assumptions"].as_array().unwrap();
    let fields: Vec<&str> = rows.iter().map(|a| a["field"].as_str().unwrap()).collect();
    let start = fields
        .iter()
        .position(|f| *f == "quality.definition")
        .unwrap();
    assert_eq!(
        &fields[start..],
        [
            "quality.definition",
            "quality.denominator",
            "quality.form",
            "quality.scheduler",
            "probability.scheduler"
        ],
        "control: the block precedes probability.scheduler"
    );
    let mut moved = rows.clone();
    let block: Vec<Value> = moved.drain(start..start + 4).collect();
    moved.extend(block);
    let mut broken = v.clone();
    broken["assumptions"] = Value::Array(moved);
    let err = invocation
        .compare(&expected, &broken)
        .expect_err("quality block after probability.scheduler");
    assert!(err.contains("probability.scheduler"), "{err}");
}
