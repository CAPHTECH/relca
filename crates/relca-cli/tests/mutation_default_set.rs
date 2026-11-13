#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/mutation_default_acceptance.rs"]
mod hand;
#[path = "support/ordered_jobs.rs"]
mod ordered_jobs;
use serde_json::Value;
use std::process::Command;

#[test]
fn real_cli_all_new_sites_held_boundaries_exit_and_closed_wire() {
    let mut failures = vec![];
    let expected = hand::oracle();
    let jobs: Vec<_> = expected.as_object().unwrap().iter().collect();
    let outputs = ordered_jobs::map(&jobs, |(name, _)| {
        Command::new(env!("CARGO_BIN_EXE_relca"))
            .arg("mutate")
            .arg(hand::root().join(name).join("model.relca"))
            .output()
            .unwrap()
    });
    for ((name, e), o) in jobs.into_iter().zip(outputs) {
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        let verdict: relca_core::Verdict =
            serde_json::from_value(e["expected"]["verdict"].clone()).unwrap();
        assert_eq!(o.status.code(), Some(verdict.exit_code()), "{name}");
        if std::panic::catch_unwind(|| hand::check(name, &v)).is_err() {
            failures.push(name.clone());
        }
        if let Err(error) = fixture_match::validate_mutation_wire(&v) {
            failures.push(format!("{name}: {error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
