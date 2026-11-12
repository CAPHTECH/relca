//! Dedicated strict argument/file boundary. Existing command parsing and usage
//! bytes remain unchanged; this entry point always emits a ConformanceReport.
use relca_core::{
    conformance::{self, ConformanceReport},
    diag,
};
use std::{collections::BTreeMap, fs::File, io::Read};

struct Arguments<'a> {
    model: &'a str,
    log: &'a str,
    scope: &'a str,
    configuration: Option<&'a str>,
    scenario: Option<&'a str>,
    approval_spec: Option<&'a str>,
}
fn value(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('-')
}
fn local_path(s: &str) -> bool {
    value(s)
        && !s.contains("://")
        && s != "/dev/stdin"
        && !s.starts_with("/dev/fd/")
        && !s.starts_with("/proc/self/fd/")
}
fn arguments(args: &[String]) -> Result<Arguments<'_>, &'static str> {
    let model = args.get(1).ok_or("argument")?.as_str();
    if !local_path(model) {
        return Err("argument");
    }
    let mut options = BTreeMap::new();
    let (pairs, trailing) = args[2..].as_chunks::<2>();
    let mut spec_error = trailing == ["--approval-spec"];
    if !trailing.is_empty() && !spec_error {
        return Err("argument");
    }
    for pair in pairs {
        let key = pair[0].as_str();
        let v = pair[1].as_str();
        if ![
            "--log",
            "--scope",
            "--configuration",
            "--scenario",
            "--approval-spec",
        ]
        .contains(&key)
        {
            return Err("argument");
        }
        let valid = if matches!(key, "--log" | "--approval-spec") {
            local_path(v)
        } else {
            value(v)
        };
        let duplicate = options.insert(key, v).is_some();
        if !valid || duplicate {
            if key == "--approval-spec" {
                spec_error = true;
            } else {
                return Err("argument");
            }
        }
    }
    // General syntax/required flags take precedence over sidecar errors.
    let log = *options.get("--log").ok_or("argument")?;
    let scope = *options.get("--scope").ok_or("argument")?;
    let approval_spec = options.get("--approval-spec").copied();
    if spec_error || (scope == "approval-burden") != approval_spec.is_some() {
        return Err(if scope == "approval-burden" {
            "approval_spec_argument"
        } else {
            "argument"
        });
    }
    Ok(Arguments {
        model,
        log,
        scope,
        configuration: options.get("--configuration").copied(),
        scenario: options.get("--scenario").copied(),
        approval_spec,
    })
}
fn regular_file(path: &str) -> std::io::Result<File> {
    // Check before open as well, so named pipes/devices cannot block an open.
    if !std::fs::metadata(path)?.is_file() {
        return Err(std::io::Error::other("regular file required"));
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("regular file required"));
    }
    Ok(file)
}
fn execute(args: &[String]) -> ConformanceReport {
    let approval = args
        .windows(2)
        .any(|p| p[0] == "--scope" && p[1] == "approval-burden");
    let a = match arguments(args) {
        Ok(a) => a,
        Err(reason) => {
            let r = ConformanceReport::failure(diag::E_LOG_0, reason);
            return if approval { r.approval_scope() } else { r };
        }
    };
    if !matches!(a.scope, "event-contracts" | "approval-burden") {
        let mut r = ConformanceReport::failure(diag::U_LOG_0, "scope");
        r.ledger.requested(a.configuration, a.scenario);
        return r.seal();
    }
    let mut src = String::new();
    if regular_file(a.model)
        .and_then(|mut f| f.read_to_string(&mut src))
        .is_err()
    {
        let mut r = ConformanceReport::failure(diag::E_LOG_0, "input");
        r.ledger.requested(a.configuration, a.scenario);
        return if approval {
            r.approval_scope().seal()
        } else {
            r.seal()
        };
    }
    match regular_file(a.log) {
        Ok(log) => {
            if let Some(path) = a.approval_spec {
                match regular_file(path) {
                    Ok(spec) => {
                        conformance::conform_approval(&src, log, spec, a.configuration, a.scenario)
                    }
                    Err(_) => {
                        let mut r =
                            ConformanceReport::failure(diag::E_LOG_0, "approval_spec_input")
                                .approval_scope();
                        r.configuration = a.configuration.map(String::from);
                        r.scenario = a.scenario.map(String::from);
                        r.with_model_source(&src);
                        r
                    }
                }
            } else {
                conformance::conform(&src, log, a.configuration, a.scenario)
            }
        }
        Err(_) => {
            let mut r = ConformanceReport::failure(diag::E_LOG_0, "input");
            r.configuration = a.configuration.map(String::from);
            r.scenario = a.scenario.map(String::from);
            r.with_model_source(&src);
            if approval { r.approval_scope() } else { r }
        }
    }
}
pub fn run(args: &[String]) -> ! {
    let r = execute(args).seal();
    for d in &r.diagnostics {
        eprintln!("{}: {}", d.code, d.message);
    }
    println!(
        "{}",
        serde_json::to_string(&r).expect("conformance report serializes")
    );
    std::process::exit(r.verdict.exit_code());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn close_non_spec_argument_errors_keep_the_existing_reason_in_approval_scope() {
        for args in [
            vec![
                "conform",
                "model",
                "--scope",
                "approval-burden",
                "--approval-spec",
                "local.json",
            ],
            vec![
                "conform",
                "-",
                "--log",
                "events",
                "--scope",
                "approval-burden",
                "--approval-spec",
                "local.json",
            ],
            vec![
                "conform",
                "model",
                "--log",
                "events",
                "--scope",
                "approval-burden",
                "--approval-spec",
                "local.json",
                "--log",
                "other",
            ],
            vec![
                "conform",
                "model",
                "--log",
                "events",
                "--scope",
                "approval-burden",
                "--approval-spec",
                "local.json",
                "--future",
                "value",
            ],
        ] {
            let args: Vec<_> = args.into_iter().map(String::from).collect();
            let r = execute(&args).seal();
            assert_eq!(r.schema, "relca-conformance-approval/v1");
            assert_eq!(r.verdict, relca_core::Verdict::Invalid);
            assert!(
                r.diagnostics
                    .iter()
                    .any(|d| d.reason.as_deref() == Some("argument")),
                "{args:?}: {:?}",
                r.diagnostics
            );
        }
    }
    #[test]
    fn approval_sidecar_is_opt_in_unique_and_local_even_on_refusal() {
        let base = [
            "conform",
            "model",
            "--log",
            "events",
            "--scope",
            "approval-burden",
        ];
        for suffix in [
            vec![],
            vec!["--approval-spec", ""],
            vec!["--approval-spec", "-"],
            vec!["--approval-spec", "https://example.invalid/spec"],
            vec!["--approval-spec", "/dev/stdin"],
            vec!["--approval-spec", "/dev/fd/0"],
            vec!["--approval-spec", "one", "--approval-spec", "two"],
        ] {
            let args: Vec<_> = base
                .iter()
                .copied()
                .chain(suffix)
                .map(String::from)
                .collect();
            let r = execute(&args).seal();
            assert_eq!(r.schema, "relca-conformance-approval/v1");
            assert_eq!(r.verdict, relca_core::Verdict::Invalid);
            assert!(
                r.diagnostics
                    .iter()
                    .any(|d| d.reason.as_deref() == Some("approval_spec_argument"))
            );
            assert!(r.approval_burden.as_ref().unwrap().is_null());
        }
        assert!(parse(&[
            "conform",
            "model",
            "--log",
            "events",
            "--scope",
            "approval-burden",
            "--approval-spec",
            "local.json"
        ]));
        assert!(!parse(&[
            "conform",
            "model",
            "--log",
            "events",
            "--scope",
            "event-contracts",
            "--approval-spec",
            "local.json"
        ]));
    }
    fn parse(a: &[&str]) -> bool {
        arguments(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>()).is_ok()
    }
    #[test]
    fn required_and_unique_flags_cannot_be_replaced_by_positionals_or_empty_values() {
        assert!(parse(&[
            "conform",
            "model",
            "--log",
            "events",
            "--scope",
            "event-contracts"
        ]));
        for args in [
            vec!["conform"],
            vec!["conform", "model"],
            vec!["conform", "model", "--log", "events"],
            vec!["conform", "model", "--log", "events", "--scope", ""],
            vec![
                "conform",
                "model",
                "--log",
                "events",
                "--scope",
                "event-contracts",
                "--log",
                "other",
            ],
            vec![
                "conform",
                "-",
                "--log",
                "events",
                "--scope",
                "event-contracts",
            ],
            vec![
                "conform",
                "model",
                "--log",
                "events",
                "--scope",
                "event-contracts",
                "extra",
            ],
        ] {
            assert!(!parse(&args));
        }
    }
    #[test]
    fn invalid_arguments_and_future_scope_use_the_new_report_not_global_usage() {
        let r = execute(&["conform".into()]);
        assert_eq!(r.verdict, relca_core::Verdict::Invalid);
        let r = execute(&["conform", "m", "--log", "l", "--scope", "future"].map(String::from));
        assert_eq!(r.verdict, relca_core::Verdict::Unsupported);
    }
    #[test]
    fn urls_and_standard_input_aliases_are_not_local_log_paths() {
        for path in [
            "https://example.invalid/events",
            "file:///tmp/events",
            "/dev/stdin",
            "/dev/fd/0",
            "/proc/self/fd/0",
        ] {
            assert!(!parse(&[
                "conform",
                "model",
                "--log",
                path,
                "--scope",
                "event-contracts"
            ]));
            assert!(!parse(&[
                "conform",
                path,
                "--log",
                "events",
                "--scope",
                "event-contracts"
            ]));
        }
    }
}
