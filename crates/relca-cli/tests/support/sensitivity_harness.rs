//! Local-only CLI plumbing; it contains no expected analysis values.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
pub fn data() -> PathBuf {
    root().join("crates/relca-core/tests/data/sensitivity")
}
pub struct Output {
    pub value: Value,
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub exit: i32,
}
pub fn cli(args: &[&str]) -> Output {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(root())
        .env("CARGO_NET_OFFLINE", "true")
        .env("TMPDIR", root().join(".tmp"))
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    let exit = o.status.code().expect("normal CLI exit, not signal");
    assert!((0..=5).contains(&exit));
    assert!(o.stdout.ends_with(b"\n"));
    assert_eq!(
        o.stdout.iter().filter(|b| **b == b'\n').count(),
        1,
        "one JSON object on stdout"
    );
    Output {
        value: serde_json::from_slice(&o.stdout).expect("CLI JSON, not empty usage"),
        stdout: o.stdout,
        stderr: String::from_utf8(o.stderr).unwrap(),
        exit,
    }
}
pub fn dir() -> PathBuf {
    let p = root().join(".tmp").join(format!(
        "q7t-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&p).unwrap();
    p
}
pub fn write(dir: &Path, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
    assert!(dir.starts_with(root().join(".tmp")));
    let p = dir.join(name);
    fs::write(&p, bytes).unwrap();
    p
}
pub fn point(id: &str, model: &str) -> Value {
    json!({"id":id,"model":model,"configuration":null,"scenario":null})
}
pub fn manifest(points: Vec<Value>, simulation: bool) -> Value {
    json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"handwritten acceptance points"},
        "points":points,"targets":[{"kind":"probability_query","id":"probability reached #1"}],
        "confidence":if simulation {json!("7/8")} else {Value::Null}})
}
pub fn run(path: &Path, mode: &str) -> Output {
    cli(&["sensitivity", path.to_str().unwrap(), "--mode", mode])
}
pub fn fixed(name: &str, mode: &str) -> Output {
    run(&data().join(name), mode)
}
pub fn custom(points: &[(&str, &str)], simulation: bool) -> (PathBuf, Value) {
    let d = dir();
    let points = points
        .iter()
        .enumerate()
        .map(|(n, (id, source))| {
            let file = format!("p{n}.relca");
            write(&d, &file, source);
            point(id, &file)
        })
        .collect();
    let m = manifest(points, simulation);
    (d, m)
}
pub fn run_custom(d: &Path, m: &Value, mode: &str) -> Output {
    let p = write(d, "manifest.json", serde_json::to_vec(m).unwrap());
    run(&p, mode)
}
pub fn runner(v: &Value) {
    assert_eq!(
        v["schema"], "relca-sensitivity-result/v1",
        "new runner/manifest meaning unimplemented: mode={} verdict={} diagnostics={}",
        v["mode"], v["verdict"], v["diagnostics"]
    );
}
pub fn reject(o: &Output, exit: i32, code: &str, reason: &str) {
    runner(&o.value);
    assert_eq!(o.exit, exit, "{}", o.stderr);
    assert_eq!(
        o.value["verdict"],
        if exit == 3 { "invalid" } else { "unsupported" }
    );
    let found = o.value["diagnostics"].as_array().unwrap().iter().any(|d| {
        d["code"] == code
            && d["severity"] == "error"
            && d["message"].as_str().is_some_and(|s| s.starts_with(reason))
    });
    assert!(found, "missing {code}/{reason}: {}", o.value["diagnostics"]);
    assert!(o.stderr.contains(code), "stderr lost diagnostic {code}");
}
pub fn no_executed_prefix(v: &Value) {
    if let Some(points) = v["points"].as_array() {
        for p in points {
            assert_ne!(
                p["status"], "evaluated",
                "capacity/gate refusal executed prefix"
            );
            if let Some(r) = p.get("report").filter(|r| !r.is_null()) {
                assert_eq!(r["ledger"]["backends"]["builtin"]["status"], "not_run");
            }
        }
    }
}
