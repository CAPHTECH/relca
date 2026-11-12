#[path = "../../relca-core/tests/support/quantile_acceptance.rs"]
mod h;
#[path = "../../relca-core/tests/support/quantile_mutation_inputs.rs"]
mod input;
use serde_json::Value;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

fn run(source: &str, sampled: bool, exit: i32) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!(
            "c7-mut-{}-{}.relca",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    std::fs::write(&path, source).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(if sampled { "simulate" } else { "check" })
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(exit));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let stderr = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            format!(
                "{}: {}\n",
                d["code"].as_str().unwrap(),
                d["message"].as_str().unwrap()
            )
        })
        .collect::<String>();
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        stderr,
        "CLI diagnostics are not silently dropped"
    );
    v
}
fn check_case(id: &str) {
    let c = input::cases().into_iter().find(|c| c.id == id).unwrap();
    let v = run(&c.source, c.sampled, c.exit);
    assert_eq!(v["verdict"], c.verdict, "{id}");
    assert_eq!(v["quality"]["quantiles"], c.rows, "{id}: real CLI vs hand");
}
macro_rules! cases {
    ($($id:ident),* $(,)?) => {$ (#[test] fn $id() {check_case(stringify!($id));})*};
}
cases!(
    f16a,
    f18l,
    f18s1,
    f18r,
    f18s,
    f19sa_censor_never,
    f25s,
    f24i,
    f40b,
    within_censor_arrivals,
    within_censor_legacy,
    float_p_sample,
    float_p_exhaustive,
    never_censor_reference
);

#[test]
fn arithmetic_cut_preserves_real_exit_and_old_diagnostics() {
    let source = input::arithmetic();
    let old = run(&h::no_quantiles(&source), false, 1);
    let full = run(&source, false, 1);
    assert_eq!(old["verdict"], "violated");
    let codes: Vec<_> = old["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].clone())
        .collect();
    assert_eq!(
        codes,
        vec![Value::from("W-DEADLOCK-0"), Value::from("W-BUDGET-0")]
    );
    assert_eq!(
        full["quality"]["quantiles"][0]["value"]["reason"],
        "truncated"
    );
    assert_eq!(h::old_projection(full), h::old_projection(old));
}
