//! Run every fixture through the real CLI and the self-tested wire comparator.

#[path = "support/current_expectations.rs"]
mod current_expectations;

#[path = "../src/fixture_match_impl.rs"]
mod fixture_match;

use serde_json::Value;
use std::{path::Path, process::Command};

#[test]
fn fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(0);
}

#[test]
fn configuration_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(1);
}

#[test]
fn agency_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(2);
}

/// The invoice example cut down to the Phase 0 grammar (v0.2 §21 stop
/// condition). A fourth fixed population; never absorbed into the others.
const INVOICE_FIXTURES: [&str; 4] = [
    "invoice-phase0-ai",
    "invoice-phase0-ai.broken-unguarded-approval",
    "invoice-phase0-parallel",
    "invoice-phase0-parallel.broken-one-clerk",
];

#[test]
fn invoice_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(3);
}

#[test]
fn contract_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(4);
}

#[test]
fn contract_scope_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(47);
}

#[test]
fn executor_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(5);
}

#[test]
fn state_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(6);
}

#[test]
fn probability_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(7);
}

#[test]
fn duration_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(8);
}

#[test]
fn arrivals_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(9);
}

#[test]
fn simulation_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(10);
}

#[test]
fn compare_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(11);
}

#[test]
fn responsibility_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(12);
}

#[test]
fn business_metrics_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(13);
}

#[test]
fn mutation_fixtures_match_binary_reports_and_all_sites() {
    run_fixtures(14);
}

#[test]
fn explain_fixtures_match_binary_reports_and_all_paths() {
    run_fixtures(15);
}

#[test]
fn excludes_waive_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(16);
}

#[test]
fn contest_runtime_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(17);
}

#[test]
fn actor_table_cut_debtor_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(18);
}

#[test]
fn common_cause_wlin2_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(19);
}

#[test]
fn completion_questions_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(20);
}

#[test]
fn requires_start_cancel_wait_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(21);
}

#[test]
fn authority_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(22);
}

#[test]
fn responsibility_extensions_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(23);
}

#[test]
fn per_case_document_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(24);
}

#[test]
fn bounded_integer_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(25);
}

#[test]
fn configuration_diff_extensions_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(26);
}

#[test]
fn resource_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(27);
}

#[test]
fn actor_observed_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(28);
}

#[test]
fn agency_viewpoints_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(29);
}

#[test]
fn fixed_policy_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(30);
}

#[test]
fn state_invariant_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(31);
}

#[test]
fn caseset_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(32);
}

#[test]
fn explicit_limits_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(33);
}

#[test]
fn effect_standing_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(34);
}

#[test]
fn record_inline_enum_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(35);
}

#[test]
fn conflict_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(36);
}

#[test]
fn scenario_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(37);
}

#[test]
fn agency_second_half_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(38);
}

#[test]
fn quality_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(39);
}

#[test]
fn rule_actor_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(40);
}

#[test]
fn mutation_default_extension_fixtures_match_binary_reports_and_all_sites() {
    run_fixtures(41);
}

#[test]
fn waiting_decomposition_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(42);
}

#[test]
fn evaluation_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(43);
}

#[test]
fn quantile_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(44);
}

const ACCEPTANCE_GAP_FIXTURES: [&str; 9] =
    include!("../../relca-core/tests/support/acceptance_gap_names.rs");

#[test]
fn acceptance_gap_fixtures_match_binary_reports_and_exit_codes() {
    run_fixtures(45);
}

const REVIEW_MUTATION_FIXTURES: [&str; 2] = [
    "mut3-expiry-window",
    "mut3-expiry-window.broken-late-arrival",
];
#[test]
fn mutation_review_time_fixtures_match_binary_and_strict_hand_expectations() {
    run_fixtures(46);
}

fn run_fixtures(group: usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut directories = Vec::new();
    for entry in std::fs::read_dir(&root).expect("fixture root must exist") {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            directories.push(entry.path());
        }
    }
    directories.sort();
    assert!(
        !directories.is_empty(),
        "no fixtures found at {}",
        root.display()
    );
    // ADR 0003 layout: calc.md everywhere, every correct fixture has at least
    // one <name>.broken-<fault> pair, and every broken one has its correct base.
    let names: Vec<String> = directories
        .iter()
        .map(|d| d.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    let mut layout = Vec::new();
    for (directory, name) in directories.iter().zip(&names) {
        if !directory.join("calc.md").is_file() {
            layout.push(format!("{name}: calc.md is missing"));
        }
        match name.split_once(".broken-") {
            Some((base, fault)) if !fault.is_empty() => {
                if !names.iter().any(|n| n == base) {
                    layout.push(format!("{name}: correct fixture {base} is missing"));
                }
            }
            Some(_) => layout.push(format!("{name}: fault name is empty")),
            None => {
                let prefix = format!("{name}.broken-");
                if !names.iter().any(|n| n.starts_with(&prefix)) {
                    layout.push(format!("{name}: no broken pair"));
                }
            }
        }
    }
    assert!(layout.is_empty(), "fixture layout:\n{}", layout.join("\n"));
    let mut populations = [0; 48];
    for name in &names {
        let groups = [
            fixture_match::LEGACY_FIXTURES.contains(&name.as_str()),
            name.starts_with("config-diff-"),
            fixture_match::AGENCY_FIXTURES.contains(&name.as_str()),
            INVOICE_FIXTURES.contains(&name.as_str()),
            fixture_match::CONTRACT_FIXTURES.contains(&name.as_str()),
            fixture_match::EXECUTOR_FIXTURES.contains(&name.as_str()),
            fixture_match::STATE_FIXTURES.contains(&name.as_str()),
            fixture_match::PROB_FIXTURES.contains(&name.as_str()),
            fixture_match::DURATION_FIXTURES.contains(&name.as_str()),
            fixture_match::ARRIVALS_FIXTURES.contains(&name.as_str()),
            fixture_match::SIM_FIXTURES.contains(&name.as_str()),
            fixture_match::COMPARE_FIXTURES.contains(&name.as_str()),
            fixture_match::RESPONSIBILITY_FIXTURES.contains(&name.as_str()),
            fixture_match::METRICS_FIXTURES.contains(&name.as_str()),
            fixture_match::MUTATION_FIXTURES.contains(&name.as_str()),
            fixture_match::EXPLAIN_FIXTURES.contains(&name.as_str()),
            fixture_match::EXCLUDES_WAIVE_FIXTURES.contains(&name.as_str()),
            fixture_match::CONTEST_RUNTIME_FIXTURES.contains(&name.as_str()),
            fixture_match::ACTOR_TABLE_FIXTURES.contains(&name.as_str()),
            fixture_match::COMMON_CAUSE_FIXTURES.contains(&name.as_str()),
            fixture_match::COMPLETION_FIXTURES.contains(&name.as_str()),
            fixture_match::REQUIRES_FIXTURES.contains(&name.as_str()),
            fixture_match::AUTHORITY_FIXTURES.contains(&name.as_str()),
            fixture_match::RESPONSIBILITY_EXTENSION_FIXTURES.contains(&name.as_str()),
            fixture_match::PERCASE_FIXTURES.contains(&name.as_str()),
            fixture_match::INTEGER_FIXTURES.contains(&name.as_str()),
            fixture_match::CONFIGURATION_DIFF_EXTENSION_FIXTURES.contains(&name.as_str()),
            fixture_match::RESOURCE_FIXTURES.contains(&name.as_str()),
            fixture_match::ACTOR_OBSERVED_FIXTURES.contains(&name.as_str()),
            fixture_match::AGENCY_VIEWPOINTS_FIXTURES.contains(&name.as_str()),
            fixture_match::POLICY_FIXTURES.contains(&name.as_str()),
            fixture_match::STATE_INVARIANT_FIXTURES.contains(&name.as_str()),
            fixture_match::CASESET_FIXTURES.contains(&name.as_str()),
            fixture_match::LIMITS_FIXTURES.contains(&name.as_str()),
            fixture_match::effect_standing_fixture(name),
            fixture_match::RECORD_FIXTURES.contains(&name.as_str()),
            fixture_match::CONFLICT_FIXTURES.contains(&name.as_str()),
            fixture_match::SCENARIO_FIXTURES.contains(&name.as_str()),
            fixture_match::AGENCY_SECOND_HALF_FIXTURES.contains(&name.as_str()),
            fixture_match::QUALITY_FIXTURES.contains(&name.as_str()),
            fixture_match::rule_actor_fixture(name),
            fixture_match::mutation_default_fixture(name),
            fixture_match::WAITS_FIXTURES.contains(&name.as_str()),
            fixture_match::EVALUATION_FIXTURES.contains(&name.as_str()),
            fixture_match::QUANTILE_FIXTURES.contains(&name.as_str()),
            ACCEPTANCE_GAP_FIXTURES.contains(&name.as_str()),
            REVIEW_MUTATION_FIXTURES.contains(&name.as_str()),
            fixture_match::CONTRACT_SCOPE_FIXTURES.contains(&name.as_str()),
        ];
        assert_eq!(
            groups.iter().filter(|&&yes| yes).count(),
            1,
            "unclassified or multiply classified fixture {name}"
        );
        populations[groups.iter().position(|&yes| yes).unwrap()] += 1;
    }
    assert_eq!(
        populations[..7],
        [16, 12, 9, 4, 19, 14, 16],
        "old 28, Agency 9, invoice 4, contract 19 and executor 14 remain separate from state 16"
    );
    assert_eq!(
        populations[..8],
        [16, 12, 9, 4, 19, 14, 16, 16],
        "old seven populations remain separate from probability 16"
    );
    assert_eq!(populations[..9], [16, 12, 9, 4, 19, 14, 16, 16, 18]);
    assert_eq!(populations[..9].iter().sum::<usize>(), 124);
    assert_eq!(populations[9], 24);
    assert_eq!(
        populations[..10].iter().sum::<usize>(),
        106 + 18 + 24,
        "old 124 and new 24 are disjoint and exhaustive"
    );
    assert_eq!(populations[10], 24);
    assert_eq!(
        populations[..11].iter().sum::<usize>(),
        148 + 24,
        "old 148 and simulation 24 are disjoint"
    );
    assert_eq!(populations[11], 16);
    assert_eq!(
        populations[..12].iter().sum::<usize>(),
        172 + 16,
        "old 172 and compare 16 are disjoint and exhaustive"
    );
    assert_eq!(populations[12], 19);
    assert_eq!(populations[..13].iter().sum::<usize>(), 188 + 19);
    assert_eq!(populations[13], 24);
    assert_eq!(
        populations[..14].iter().sum::<usize>(),
        207 + 24,
        "14 old groups unchanged"
    );
    assert_eq!(populations[14], 12);
    assert_eq!(
        populations[..15].iter().sum::<usize>(),
        231 + 12,
        "15 old groups unchanged"
    );
    assert_eq!(populations[15], 12);
    assert_eq!(populations[16], 12);
    assert_eq!(populations[17], 12);
    assert_eq!(populations[18], 28);
    assert_eq!(populations[19], 24);
    assert_eq!(populations[20], 24);
    assert_eq!(populations[21], 32);
    assert_eq!(populations[22], 40);
    assert_eq!(populations[23], 30);
    assert_eq!(populations[24], 34);
    assert_eq!(populations[25], 56);
    assert_eq!(populations[26], 38);
    assert_eq!(populations[27], 60);
    assert_eq!(populations[28], 48);
    assert_eq!(populations[29], 46);
    assert_eq!(populations[30], 74);
    assert_eq!(populations[31], 124);
    let caseset = fixture_match::CASESET_FIXTURES.len();
    assert_eq!(populations[32], caseset);
    let limits = fixture_match::LIMITS_FIXTURES.len();
    assert_eq!(populations[33], limits);
    assert_eq!(populations[34], 86);
    assert_eq!(populations[35], 146);
    let conflicts = fixture_match::CONFLICT_FIXTURES.len();
    assert_eq!(populations[36], conflicts);
    let scenarios = fixture_match::SCENARIO_FIXTURES.len();
    assert_eq!(populations[37], scenarios);
    let agency2 = fixture_match::AGENCY_SECOND_HALF_FIXTURES.len();
    assert_eq!(populations[38], agency2);
    let quality = fixture_match::QUALITY_FIXTURES.len();
    assert_eq!(populations[39], quality);
    assert_eq!(populations[40], 140);
    assert_eq!(populations[41], 79);
    let waits = fixture_match::WAITS_FIXTURES.len();
    assert_eq!(populations[42], waits);
    let evaluation = fixture_match::EVALUATION_FIXTURES.len();
    assert_eq!(populations[43], evaluation);
    let quantiles = fixture_match::QUANTILE_FIXTURES.len();
    assert_eq!(populations[44], quantiles);
    assert_eq!(quantiles, 67);
    assert_eq!(populations[45], 9);
    assert_eq!(populations[46], 2);
    assert_eq!(populations[47], 10);
    assert_eq!(
        names.len(),
        355 + 32
            + 40
            + 30
            + 34
            + 56
            + 38
            + 60
            + 48
            + 46
            + 74
            + 124
            + caseset
            + limits
            + 86
            + 146
            + conflicts
            + scenarios
            + agency2
            + quality
            + 140
            + 79
            + waits
            + evaluation
            + quantiles
            + 9
            + 2
            + 10,
        "48 disjoint exhaustive groups; all 47 historical groups unchanged"
    );
    let mut failures = Vec::new();
    let mut undetected = Vec::new();
    for directory in directories {
        let name = directory.file_name().unwrap().to_string_lossy();
        let legacy = fixture_match::LEGACY_FIXTURES.contains(&name.as_ref());
        let current = if fixture_match::CONTRACT_SCOPE_FIXTURES.contains(&name.as_ref()) {
            47
        } else if legacy {
            0
        } else if name.starts_with("config-diff-") {
            1
        } else if INVOICE_FIXTURES.contains(&name.as_ref()) {
            3
        } else if fixture_match::CONTRACT_FIXTURES.contains(&name.as_ref()) {
            4
        } else if fixture_match::EXECUTOR_FIXTURES.contains(&name.as_ref()) {
            5
        } else if fixture_match::STATE_FIXTURES.contains(&name.as_ref()) {
            6
        } else if fixture_match::PROB_FIXTURES.contains(&name.as_ref()) {
            7
        } else if fixture_match::DURATION_FIXTURES.contains(&name.as_ref()) {
            8
        } else if fixture_match::ARRIVALS_FIXTURES.contains(&name.as_ref()) {
            9
        } else if fixture_match::SIM_FIXTURES.contains(&name.as_ref()) {
            10
        } else if fixture_match::COMPARE_FIXTURES.contains(&name.as_ref()) {
            11
        } else if fixture_match::RESPONSIBILITY_FIXTURES.contains(&name.as_ref()) {
            12
        } else if fixture_match::EXCLUDES_WAIVE_FIXTURES.contains(&name.as_ref()) {
            16
        } else if fixture_match::CONTEST_RUNTIME_FIXTURES.contains(&name.as_ref()) {
            17
        } else if fixture_match::ACTOR_TABLE_FIXTURES.contains(&name.as_ref()) {
            18
        } else if fixture_match::COMMON_CAUSE_FIXTURES.contains(&name.as_ref()) {
            19
        } else if fixture_match::COMPLETION_FIXTURES.contains(&name.as_ref()) {
            20
        } else if fixture_match::REQUIRES_FIXTURES.contains(&name.as_ref()) {
            21
        } else if fixture_match::AUTHORITY_FIXTURES.contains(&name.as_ref()) {
            22
        } else if fixture_match::RESPONSIBILITY_EXTENSION_FIXTURES.contains(&name.as_ref()) {
            23
        } else if fixture_match::PERCASE_FIXTURES.contains(&name.as_ref()) {
            24
        } else if fixture_match::INTEGER_FIXTURES.contains(&name.as_ref()) {
            25
        } else if fixture_match::CONFIGURATION_DIFF_EXTENSION_FIXTURES.contains(&name.as_ref()) {
            26
        } else if fixture_match::RESOURCE_FIXTURES.contains(&name.as_ref()) {
            27
        } else if fixture_match::ACTOR_OBSERVED_FIXTURES.contains(&name.as_ref()) {
            28
        } else if fixture_match::AGENCY_VIEWPOINTS_FIXTURES.contains(&name.as_ref()) {
            29
        } else if fixture_match::POLICY_FIXTURES.contains(&name.as_ref()) {
            30
        } else if fixture_match::STATE_INVARIANT_FIXTURES.contains(&name.as_ref()) {
            31
        } else if fixture_match::CASESET_FIXTURES.contains(&name.as_ref()) {
            32
        } else if fixture_match::LIMITS_FIXTURES.contains(&name.as_ref()) {
            33
        } else if fixture_match::effect_standing_fixture(&name) {
            34
        } else if fixture_match::RECORD_FIXTURES.contains(&name.as_ref()) {
            35
        } else if fixture_match::CONFLICT_FIXTURES.contains(&name.as_ref()) {
            36
        } else if fixture_match::SCENARIO_FIXTURES.contains(&name.as_ref()) {
            37
        } else if fixture_match::AGENCY_SECOND_HALF_FIXTURES.contains(&name.as_ref()) {
            38
        } else if fixture_match::QUALITY_FIXTURES.contains(&name.as_ref()) {
            39
        } else if fixture_match::rule_actor_fixture(&name) {
            40
        } else if fixture_match::mutation_default_fixture(&name) {
            41
        } else if fixture_match::WAITS_FIXTURES.contains(&name.as_ref()) {
            42
        } else if fixture_match::EVALUATION_FIXTURES.contains(&name.as_ref()) {
            43
        } else if fixture_match::QUANTILE_FIXTURES.contains(&name.as_ref()) {
            44
        } else if ACCEPTANCE_GAP_FIXTURES.contains(&name.as_ref()) {
            45
        } else if REVIEW_MUTATION_FIXTURES.contains(&name.as_ref()) {
            46
        } else if name.starts_with("explain-") {
            assert!(
                fixture_match::EXPLAIN_FIXTURES.contains(&name.as_ref()),
                "unclassified explain fixture {name}"
            );
            15
        } else if name.starts_with("mut-") {
            assert!(
                fixture_match::MUTATION_FIXTURES.contains(&name.as_ref()),
                "unclassified mutation fixture {name}"
            );
            14
        } else if name.starts_with("met-") {
            assert!(
                fixture_match::METRICS_FIXTURES.contains(&name.as_ref()),
                "unclassified metrics fixture {name}"
            );
            13
        } else {
            2
        };
        if current != group {
            continue;
        }
        let invocation_path = directory.join("invocation.json");
        let invocation_value: Option<Value> = if invocation_path.exists() {
            Some(serde_json::from_str(&std::fs::read_to_string(&invocation_path).unwrap()).unwrap())
        } else {
            None
        };
        let invocation = fixture_match::Invocation::parse(invocation_value.as_ref())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        if legacy {
            assert_eq!(invocation, fixture_match::Invocation::LegacyCheck);
        }
        let expected_path = directory.join("expected.json");
        let model_path = directory.join("model.relca");
        assert!(model_path.is_file(), "{name}: model.relca is missing");
        let mut expected: Value = serde_json::from_str(
            &std::fs::read_to_string(&expected_path)
                .unwrap_or_else(|error| panic!("{}: {error}", expected_path.display())),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", expected_path.display()));
        let metadata = expected
            .as_object_mut()
            .expect("expected must be an object")
            .remove("undetected");
        assert!(
            metadata.is_none() || metadata.as_ref().is_some_and(Value::is_boolean),
            "{name}: undetected must be boolean"
        );
        // A malformed expectation is a failure even when the fixture is
        // undetected: otherwise its comparison error would only be listed.
        if let Err(error) = invocation.validate_expected(&expected) {
            failures.push(format!("{name}: {error}"));
            continue;
        }
        // Expected verdict controls exit expectations, not the binary's own claim.
        let Some(verdict) = expected
            .get("verdict")
            .and_then(|v| serde_json::from_value::<relca_core::Verdict>(v.clone()).ok())
        else {
            failures.push(format!("{name}: expected verdict is required"));
            continue;
        };
        let args = invocation.args(&model_path);
        let scenario = args
            .iter()
            .position(|a| a == "--scenario")
            .map(|i| args[i + 1].as_str());
        let source = std::fs::read_to_string(&model_path).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(&args)
            .output()
            .expect("cannot launch relca");
        current_expectations::migrate_with_scenario(&name, &source, &mut expected, scenario);
        let result = (|| -> Result<(), String> {
            let actual: Value = serde_json::from_slice(&output.stdout)
                .map_err(|error| format!("stdout must contain exactly one JSON value: {error}"))?;
            if let (Some(from), Some(to)) = (
                expected["from"]["configuration"].as_str(),
                expected["to"]["configuration"].as_str(),
            ) {
                let (null, mismatch) =
                    current_expectations::diff_state_with_scenario(&source, from, to, scenario);
                current_expectations::check_diff_marker(&actual, null, mismatch)?;
            }
            current_expectations::assert_k4(&name, &source, &actual);
            invocation.compare(&expected, &actual)?;
            if current == 14 || current == 41 {
                fixture_match::compare_mutation_fixture_sites(&name, &actual)?;
            }
            if output.status.code() != Some(verdict.exit_code()) {
                return Err(format!(
                    "exit {:?} != {}",
                    output.status.code(),
                    verdict.exit_code()
                ));
            }
            Ok(())
        })();
        if metadata == Some(Value::Bool(true)) {
            undetected.push(format!(
                "{name}: {}",
                result
                    .err()
                    .unwrap_or_else(|| "expectations matched; still marked undetected".into())
            ));
        } else if let Err(error) = result {
            failures.push(format!("{name}: {error}"));
        }
    }
    eprintln!(
        "undetected fixtures ({}):\n{}",
        undetected.len(),
        undetected.join("\n")
    );
    assert!(
        failures.is_empty(),
        "fixture failures:\n{}",
        failures.join("\n")
    );
}
