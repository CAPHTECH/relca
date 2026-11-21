//! Refusals withdraw no analyst values: keep the fixed-main byte controls.
#[path = "support/test_tmp.rs"]
mod test_tmp;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
const INLINE: &str = include_str!("support/defer_rejected_inline.relca");
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn write(name: &str, source: &str) -> PathBuf {
    let path = test_tmp::root().join(format!(
        "defer-refusal-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    path
}
fn run(command: &str, path: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(path)
        .args(extra)
        .current_dir(root())
        .output()
        .unwrap()
}
fn no_effects(v: &Value) {
    match v {
        Value::Object(o) => {
            assert!(
                !o.contains_key("defer_effects"),
                "refused report acquired effects: {v}"
            );
            assert_ne!(
                o.get("code"),
                Some(&json!("W-CUT-2")),
                "refused report acquired warning: {v}"
            );
            o.values().for_each(no_effects);
        }
        Value::Array(a) => a.iter().for_each(no_effects),
        _ => {}
    }
}
fn assert_refusal(out: &Output) -> Value {
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_ne!(out.status.code(), Some(0));
    no_effects(&value);
    assert!(!String::from_utf8_lossy(&out.stderr).contains("W-CUT-2"));
    value
}
fn baseline(kind: &str, command: &str, out: &Output) {
    let source = match kind {
        "inline" => INLINE,
        "invoice" => include_str!("../../../examples/invoice/to-be-c.relca"),
        _ => panic!("unregistered refusal source"),
    };
    let saved = Command::new("gzip")
        .arg("-dc")
        .arg(root().join(format!(
            "crates/relca-cli/tests/support/defer-rejected-{kind}-{command}.snap.gz"
        )))
        .output()
        .unwrap();
    assert!(saved.status.success());
    let at = saved.stdout.iter().position(|b| *b == b'\n').unwrap();
    let lengths: Vec<usize> = std::str::from_utf8(&saved.stdout[..at])
        .unwrap()
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    assert_eq!(lengths.len(), 3);
    let data = &saved.stdout[at + 1..];
    assert_eq!(data.len(), lengths.iter().sum::<usize>());
    assert_eq!(
        out.stdout,
        relca_test_support::diagnostic::extend_saved_stdout(
            source,
            std::str::from_utf8(&data[..lengths[0]]).unwrap()
        )
        .as_bytes(),
        "{kind} {command} current stdout"
    );
    assert_eq!(
        out.stderr,
        data[lengths[0]..lengths[0] + lengths[1]],
        "{kind} {command} current stderr"
    );
    assert_eq!(
        format!("{}\n", out.status.code().unwrap()).as_bytes(),
        &data[lengths[0] + lengths[1]..]
    );
}
#[test]
fn rejected_invoice_check_and_explain_keep_all_main_bytes() {
    for command in ["check", "explain"] {
        let out = run(command, &root().join("examples/invoice/to-be-c.relca"), &[]);
        let value = assert_refusal(&out);
        assert_eq!(value["verdict"], "invalid");
        assert!(
            value["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-POL-LATENT")
        );
        baseline("invoice", command, &out);
    }
}
#[test]
fn rejected_inline_check_and_explain_keep_all_main_bytes() {
    let path = write("inline.relca", INLINE);
    for command in ["check", "explain"] {
        let out = run(command, &path, &[]);
        let value = assert_refusal(&out);
        assert_eq!(value["verdict"], "invalid");
        assert!(
            value["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-POL-LATENT")
        );
        baseline("inline", command, &out);
    }
}
#[test]
fn lower_and_cut_rejections_and_nested_command_children_have_no_effects() {
    for source in [
        INLINE.to_owned(),
        INLINE.replace("include [A]", "include []"),
    ] {
        let path = write("bad.relca", &source);
        for command in ["check", "explain", "simulate", "mutate"] {
            assert_refusal(&run(command, &path, &[]));
        }
        let configured = source.rsplit_once('}').unwrap().0.to_owned()
            + "\nconfiguration X { cut C; process P }\nconfiguration Y { cut C; process P }\n}";
        let path = write("configured.relca", &configured);
        for mode in ["exhaustive", "simulation"] {
            assert_refusal(&run("compare", &path, &["--plans", "X,Y", "--mode", mode]));
        }
        assert_refusal(&run("diff", &path, &["--from", "X", "--to", "Y"]));
        let model = write("point.relca", &source);
        let manifest = json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"refused point"},"points":[{"id":"bad","model":model.file_name().unwrap().to_str().unwrap(),"configuration":null,"scenario":null}],"targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null});
        let path = write("points.json", &manifest.to_string());
        assert_refusal(&run("sensitivity", &path, &["--mode", "exhaustive"]));
    }
}
#[test]
fn successful_lowering_but_no_indicator_report_does_not_claim_withdrawal() {
    let source = INLINE.replace(
        "if hidden == true { end wrong } else { end done }",
        "end done",
    );
    let model = relca_core::lower::lower(&relca_core::syntax::parse(&source).unwrap()).unwrap();
    assert!(
        model.diagnostics.iter().any(|d| d.code == "W-CUT-2"),
        "lowered model remains eligible for analyses that produce metrics"
    );
    let path = write("missing-simulation.relca", &source);
    let value = assert_refusal(&run("simulate", &path, &[]));
    assert!(value.get("metrics").is_none() && value.get("quality").is_none());
}
