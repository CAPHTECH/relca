//! Implementation-owned tests from orch's MJ2 specification, not independent acceptance.
#[path = "support/q6b_oracle.rs"]
mod hand;
#[path = "support/q6b_inputs.rs"]
mod input;
#[path = "support/q6b_review_inputs.rs"]
mod review;
use relca_core::{Verdict, report};

fn source(clock: u64) -> String {
    let src = input::edit(
        &input::source("latent-ok"),
        "false -> { 1:ok }; true -> { 1:fail }",
        "false -> { 0.5:ok; 0.5:fail }; true -> { 0.5:ok; 0.5:fail }",
    );
    let src = input::edit(
        &src,
        "simulation replications 4 seed 1",
        "simulation replications 64 seed 1",
    );
    if clock == 10 {
        src
    } else {
        input::edit(&src, "clock 10min", &format!("clock {clock}min"))
    }
}

#[test]
fn positive_kernel_witness_does_not_close_a_clock_cut_degradation() {
    // Probe1 + Fast1 = 2 reaches done; Probe1 + Clerk3 = 4 exceeds clock3.
    let r = report::simulate(&source(3));
    assert_eq!(r.bounds.truncated, Some(true));
    let wire = serde_json::to_value(&r).unwrap();
    assert!(
        wire["case_outcomes"].to_string().contains("done"),
        "sampled done witness"
    );
    assert_eq!(r.verdict, Verdict::Inconclusive);
    assert_eq!(r.verdict.exit_code(), 2);
    serde_json::from_value::<report::SimulationReport>(wire).expect("own emitted cut wire decodes");
}

#[test]
fn complete_kernel_degradation_remains_satisfied_and_decodable() {
    let r = report::simulate(&source(10));
    assert_eq!(r.bounds.truncated, Some(false));
    assert_eq!(r.verdict, Verdict::Satisfied);
    assert_eq!(r.verdict.exit_code(), 0);
    serde_json::from_value::<report::SimulationReport>(serde_json::to_value(r).unwrap()).unwrap();
}

#[test]
fn seal_simulation_clamps_only_cut_degradation_success_and_vacuity() {
    let good = report::simulate(&source(10));
    for verdict in [
        Verdict::Satisfied,
        Verdict::Vacuous,
        Verdict::Violated,
        Verdict::Unsupported,
        Verdict::Invalid,
    ] {
        let mut r = good.clone();
        r.bounds.truncated = Some(true);
        r.verdict = verdict;
        let r = report::seal_simulation(r);
        let expected = if matches!(verdict, Verdict::Satisfied | Verdict::Vacuous) {
            Verdict::Inconclusive
        } else {
            verdict
        };
        assert_eq!(r.verdict, expected, "{verdict:?}");
        serde_json::from_value::<report::SimulationReport>(serde_json::to_value(r).unwrap())
            .unwrap();
    }
    let ordinary = source(10).replace(input::ROUTE, "if status == fail");
    let mut legacy = report::simulate(&ordinary);
    legacy.bounds.truncated = Some(true);
    legacy.verdict = Verdict::Satisfied;
    // This new gate does not silently change the ordinary/legacy model contract.
    assert_eq!(report::seal_simulation(legacy).verdict, Verdict::Satisfied);
}

#[test]
fn arrival_nine_withdraws_only_the_calendar_side_not_the_known_prefix() {
    // Arrival9 + Probe1 + Clerk3 + Finish2 = 15 >= through11.
    // Prefix Probe completes by10 < 11: its structural Start remains known1.
    let r = report::check(&review::coverage(9, false));
    assert!(
        !matches!(r.verdict, Verdict::Invalid | Verdict::Unsupported),
        "{:?}",
        r.diagnostics
    );
    let wire = serde_json::to_value(r).unwrap();
    let t = &wire["agency"]["degradation"];
    assert_eq!(t["status"], "derived");
    assert!(t["reason"].is_null());
    assert_eq!(t["routes"].as_array().unwrap().len(), 1);
    let route = &t["routes"][0];
    assert_eq!(
        route["common_prefix"],
        hand::counts(&[("Probe.check", 1, 1)])
    );
    assert_eq!(
        route["degraded"],
        serde_json::json!({"status":"not_derived","reason":"calendar","ends":null,"observation":null})
    );
}
