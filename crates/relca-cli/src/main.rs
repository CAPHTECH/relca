use relca_core::{Verdict, compare, diag, diff, explain, mutation, report};
mod conform;
mod sensitivity;

const COMMANDS: &[&str] = &[
    "check",
    "diff",
    "verify",
    "simulate",
    "compare",
    "explain",
    "mutate",
    "sensitivity",
    "conform",
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verdict = match args.first().map(String::as_str) {
        Some("sensitivity") => std::process::exit(sensitivity::run(&args).exit_code()),
        Some("conform") => conform::run(&args),
        Some("explain") => {
            let result = match flags(&args, &["--configuration", "--scenario"], EXPLAIN) {
                Some((path, options)) => match std::fs::read_to_string(path) {
                    Ok(src) => explain::explain_with_selection(
                        &src,
                        options.get("--configuration").copied(),
                        options.get("--scenario").copied(),
                    ),
                    Err(error) => explain::input_failure_with_selection(
                        &format!("Cannot read {path}: {error}"),
                        options.get("--configuration").copied(),
                        options.get("--scenario").copied(),
                    ),
                },
                _ => explain::argument_failure(
                    "usage: relca explain <path> [--configuration NAME] [--scenario NAME]",
                ),
            };
            for diagnostic in &result.report.diagnostics {
                eprintln!("{}: {}", diagnostic.code, diagnostic.message);
            }
            if !args.iter().any(|a| a == "--configuration")
                && result
                    .report
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "E-CONF-0" && d.message.contains("configuration"))
            {
                eprintln!("hint: relca explain <path> --configuration NAME [--scenario NAME]");
            }
            for diagnostic in &result.explanation.diagnostics {
                eprintln!("explain: {}: {}", diagnostic.code, diagnostic.message);
            }
            println!(
                "{}",
                serde_json::to_string(&result).expect("explain report serializes")
            );
            std::process::exit(result.report.verdict.exit_code());
        }
        Some("mutate") => {
            // Keep pre-existing refusal diagnostics byte-identical for old argv.
            let usage = if args.iter().any(|s| {
                matches!(
                    s.as_str(),
                    "--mutant-limit" | "--kinds" | "--sites" | "--configuration"
                )
            }) {
                "usage: relca mutate <path> [--set bpr-default] [--configuration NAME] [--scenario NAME] [--mutant-limit N] [--kinds k1,k2] [--sites ID1,ID2]"
            } else {
                "usage: relca mutate <path> [--set bpr-default] [--scenario NAME]"
            };
            let parsed = flags(
                &args,
                &[
                    "--set",
                    "--configuration",
                    "--scenario",
                    "--mutant-limit",
                    "--kinds",
                    "--sites",
                ],
                MUTATE,
            )
            .filter(|(_, o)| o.get("--set").is_none_or(|s| *s == "bpr-default"))
            .filter(|(_, o)| o.get("--configuration").is_none_or(|v| (CHECK.value)(v)))
            .and_then(|(path, options)| {
                let mutation_options = mutation::Options::parse(
                    options.get("--mutant-limit").copied(),
                    options.get("--kinds").copied(),
                )
                .ok()?
                .with_sites(options.get("--sites").copied())
                .ok()?
                .with_configuration(options.get("--configuration").copied());
                Some((path, options, mutation_options))
            });
            let result = match parsed {
                Some((path, options, mutation_options)) => match std::fs::read_to_string(path) {
                    Ok(src) => mutation::mutate_with_options(
                        &src,
                        "bpr-default",
                        options.get("--scenario").copied(),
                        &mutation_options,
                    ),
                    Err(error) => mutation::input_failure_with_options(
                        &format!("Cannot read {path}: {error}"),
                        options.get("--scenario").copied(),
                        &mutation_options,
                    ),
                },
                None => mutation::argument_failure(usage),
            };
            if let Some(original) = &result.original {
                for d in &original.diagnostics {
                    eprintln!("original: {}: {}", d.code, d.message);
                }
            }
            for row in &result.mutations {
                for mutant in &row.mutants {
                    if let Some(report) = &mutant.report {
                        for d in &report.diagnostics {
                            eprintln!("{}: {}: {}", mutant.id, d.code, d.message);
                        }
                    }
                }
            }
            for d in &result.diagnostics {
                eprintln!("mutate: {}: {}", d.code, d.message);
            }
            println!(
                "{}",
                serde_json::to_string(&result).expect("mutation report serializes")
            );
            std::process::exit(result.verdict.exit_code());
        }
        Some("check") => {
            let result = match flags(&args, &["--configuration", "--scenario"], CHECK) {
                Some((path, options)) => match std::fs::read_to_string(path) {
                    Ok(src) => report::check_with_selection(
                        &src,
                        options.get("--configuration").copied(),
                        options.get("--scenario").copied(),
                    ),
                    Err(error) => input_failure_selected(
                        format!("Cannot read {path}: {error}"),
                        options.get("--configuration").copied(),
                        options.get("--scenario").copied(),
                    ),
                },
                _ => input_failure(
                    "usage: relca check <path> [--configuration NAME] [--scenario NAME]".into(),
                ),
            };
            for diagnostic in &result.diagnostics {
                eprintln!("{}: {}", diagnostic.code, diagnostic.message);
            }
            println!(
                "{}",
                serde_json::to_string(&result).expect("report serializes")
            );
            std::process::exit(result.verdict.exit_code());
        }
        Some("simulate") => {
            let result = match flags(&args, &["--configuration", "--scenario"], SIMULATE) {
                Some((path, options)) => match std::fs::read_to_string(path) {
                    Ok(src) => report::simulate_with_selection(
                        &src,
                        options.get("--configuration").copied(),
                        options.get("--scenario").copied(),
                    ),
                    Err(error) => {
                        let mut r = report::simulation_input_failure(&format!(
                            "Cannot read {path}: {error}"
                        ));
                        r.ledger.requested(
                            options.get("--configuration").copied(),
                            options.get("--scenario").copied(),
                        );
                        report::seal_simulation(r)
                    }
                },
                _ => report::simulation_input_failure(
                    "usage: relca simulate <path> [--configuration NAME] [--scenario NAME]",
                ),
            };
            for diagnostic in &result.diagnostics {
                eprintln!("{}: {}", diagnostic.code, diagnostic.message);
            }
            println!(
                "{}",
                serde_json::to_string(&result).expect("simulation report serializes")
            );
            std::process::exit(result.verdict.exit_code());
        }
        Some("compare") => {
            let usage = "usage: relca compare <path> --plans A,B[,C...] --mode exhaustive|simulation [--scenario NAME]";
            let result = if let Some((path, plans, analysis, scenario)) = compare_args(&args) {
                match std::fs::read_to_string(path) {
                    Ok(src) => compare::compare_with_scenario(&src, &plans, analysis, scenario),
                    Err(error) => compare::input_failure_with_scenario(
                        &format!("Cannot read {path}: {error}"),
                        &plans,
                        analysis,
                        scenario,
                    ),
                }
            } else {
                compare::argument_failure(usage)
            };
            for plan in &result.plans {
                for diagnostic in plan.report.diagnostics() {
                    eprintln!(
                        "configuration {}: {}: {}",
                        plan.configuration, diagnostic.code, diagnostic.message
                    );
                }
            }
            for diagnostic in &result.diagnostics {
                eprintln!("compare: {}: {}", diagnostic.code, diagnostic.message);
            }
            println!(
                "{}",
                serde_json::to_string(&result).expect("compare report serializes")
            );
            std::process::exit(result.verdict.exit_code());
        }
        Some("diff") => {
            if let Some((path, from, to, scenario)) = diff_args(&args) {
                let result = match std::fs::read_to_string(path) {
                    Ok(src) => diff::diff_with_scenario(&src, from, to, scenario),
                    Err(error) => diff::DiffReport::uncomputed(
                        diff::Side {
                            configuration: from.into(),
                            report: input_failure_selected(
                                format!("Cannot read {path}: {error}"),
                                Some(from),
                                scenario,
                            ),
                        },
                        diff::Side {
                            configuration: to.into(),
                            report: input_failure_selected(
                                format!("Cannot read {path}: {error}"),
                                Some(to),
                                scenario,
                            ),
                        },
                    ),
                };
                for (side, diagnostics) in [
                    ("from", &result.from.report.diagnostics),
                    ("to", &result.to.report.diagnostics),
                    ("diff", &result.diagnostics),
                ] {
                    for diagnostic in diagnostics {
                        eprintln!("{side}: {}: {}", diagnostic.code, diagnostic.message);
                    }
                }
                println!(
                    "{}",
                    serde_json::to_string(&result).expect("diff report serializes")
                );
                std::process::exit(result.verdict.exit_code());
            }
            eprintln!("usage: relca diff <path> --from NAME --to NAME [--scenario NAME]");
            Verdict::Invalid
        }
        Some("verify") => {
            if args
                .get(1)
                .and_then(|path| std::fs::read_to_string(path).ok())
                .and_then(|src| relca_core::syntax::parse(&src).ok())
                .is_some_and(|m| m.uses_priority())
            {
                let r = relca_core::ledger::unsupported(
                    "verify",
                    "priority_feature_combination: verify backend is not connected to priority disciplines",
                );
                for d in &r.diagnostics {
                    eprintln!("{}: {}", d.code, d.message);
                }
                println!(
                    "{}",
                    serde_json::to_string(&r).expect("priority refusal wire")
                );
                std::process::exit(r.verdict.exit_code());
            }
            eprintln!(
                "relca verify: not implemented; outside Phase 1; external backend is not connected"
            );
            Verdict::Unsupported
        }
        Some(cmd) if COMMANDS.contains(&cmd) => {
            eprintln!("relca {cmd}: not implemented");
            Verdict::Unsupported
        }
        _ => {
            eprintln!("usage: relca <{}> ...", COMMANDS.join("|"));
            Verdict::Invalid
        }
    };
    let mode = if args.first().is_some_and(|s| s == "verify") {
        "verify"
    } else {
        "cli_refusal"
    };
    let result = relca_core::ledger::refusal(mode, verdict);
    for diagnostic in &result.diagnostics {
        eprintln!("{}: {}", diagnostic.code, diagnostic.message);
    }
    println!(
        "{}",
        serde_json::to_string(&result).expect("refusal serializes")
    );
    std::process::exit(result.verdict.exit_code());
}

/// PRE-B5 acceptance of each command's path and of its old flag values,
/// byte-compatible with the old binary (boundary review m4, ADR 0042). Only the
/// new `--scenario` value follows the new rule: nonempty, not starting with '-'.
struct Shape {
    path: fn(&str) -> bool,
    value: fn(&str) -> bool,
}
const EXPLAIN: Shape = Shape {
    path: |p| !p.is_empty() && !p.starts_with('-'),
    value: |v| !v.is_empty() && !v.starts_with("--"),
};
const MUTATE: Shape = Shape {
    path: |p| !p.is_empty() && !p.starts_with('-'),
    value: |_| true, // --set is checked against bpr-default by the caller.
};
const CHECK: Shape = Shape {
    path: |_| true,
    value: |v| !v.starts_with("--"),
};
const SIMULATE: Shape = Shape {
    path: |p| !p.is_empty() && !p.starts_with("--"),
    value: |v| !v.is_empty() && !v.starts_with("--"),
};
const COMPARE: Shape = Shape {
    path: |p| !p.is_empty() && !p.starts_with("--"),
    value: |_| true, // --plans and --mode are validated by compare_args.
};
const DIFF: Shape = Shape {
    path: |p| !p.starts_with("--"),
    value: |v| !v.starts_with("--"),
};

fn flags<'a>(
    args: &'a [String],
    allowed: &[&str],
    shape: Shape,
) -> Option<(&'a str, std::collections::BTreeMap<&'a str, &'a str>)> {
    let path = args.get(1)?.as_str();
    if !(shape.path)(path) || !(args.len() - 2).is_multiple_of(2) {
        return None;
    }
    let mut options = std::collections::BTreeMap::new();
    for pair in args[2..].as_chunks::<2>().0 {
        let (key, value) = (pair[0].as_str(), pair[1].as_str());
        let value_ok = if key == "--scenario" {
            !value.is_empty() && !value.starts_with('-')
        } else {
            (shape.value)(value)
        };
        if !allowed.contains(&key) || !value_ok || options.insert(key, value).is_some() {
            return None;
        }
    }
    Some((path, options))
}

fn compare_args(args: &[String]) -> Option<(&str, Vec<&str>, compare::Analysis, Option<&str>)> {
    let (path, options) = flags(args, &["--plans", "--mode", "--scenario"], COMPARE)?;
    let plans = options.get("--plans")?;
    let mode = options.get("--mode")?;
    let plans: Vec<_> = plans.split(',').collect();
    if !compare::valid_plans(&plans) {
        return None;
    }
    let analysis = match *mode {
        "exhaustive" => compare::Analysis::Exhaustive,
        "simulation" => compare::Analysis::Simulation,
        _ => return None,
    };
    Some((path, plans, analysis, options.get("--scenario").copied()))
}

fn diff_args(args: &[String]) -> Option<(&str, &str, &str, Option<&str>)> {
    let (path, options) = flags(args, &["--from", "--to", "--scenario"], DIFF)?;
    Some((
        path,
        options.get("--from")?,
        options.get("--to")?,
        options.get("--scenario").copied(),
    ))
}

fn input_failure(message: String) -> report::Report {
    report::seal(report::Report {
        contract_scope: Vec::new(),
        repetition: None,
        response_recovery: None,
        priority_policy: None,
        work_profiles: None,
        uncertainty: relca_core::uncertainty::Uncertainty::not_lowered(),
        resource_time: None,
        ledger: relca_core::ledger::Ledger::input_failure("exhaustive"),
        response_receipts: None,
        response_analysis: None,
        scenario: None,
        quality: None,
        state_range_violations: Vec::new(),
        capability_refusals: None,
        unchecked: Vec::new(),
        actors: None,
        cut_context: report::CutContext::default(),
        via_model_paths: None,
        contest_runtime: None,
        contest_summary: None,
        obligations: None,
        metrics: None,
        responsibilities: None,
        contest_paths: None,
        case_outcomes: None,
        admission: None,
        agency: None,
        verdict: Verdict::Invalid,
        mode: report::Mode::Exhaustive,
        unit: report::Unit::Min,
        model: report::ModelMetadata {
            name: None,
            sha256: None,
        },
        cut: report::Cut::Status(report::CutStatus::Unresolved),
        assumptions: Vec::new(),
        diagnostics: vec![diag::Diagnostic {
            code: diag::E_SYN_0.into(),
            severity: diag::Severity::Error,
            message,
            span: None,
        }],
        bounds: report::Bounds {
            range_violation_stops: None,
            cases: None,
            live_tasks: None,
            clock: None,
            truncated: None,
        },
        properties: Vec::new(),
        terminals: Vec::new(),
    })
}

fn input_failure_selected(
    message: String,
    configuration: Option<&str>,
    scenario: Option<&str>,
) -> report::Report {
    let mut r = input_failure(message);
    r.ledger.requested(configuration, scenario);
    report::seal(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreadable_input_retains_unknown_metadata_and_cut() {
        let report = input_failure("unreadable".into());
        assert_eq!(report.verdict, Verdict::Invalid);
        assert_eq!(
            report::fold_verdict(&report.properties, &report.diagnostics),
            report.verdict
        );
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["cut"], "unresolved");
        assert!(json["model"]["sha256"].is_null());
        assert!(json["bounds"]["clock"].is_null());
        assert!(json["bounds"]["truncated"].is_null());
    }
}
