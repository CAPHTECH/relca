use serde_json::{Value, json};
use std::{path::Path, process::Command};

#[path = "diagnostic_details/unit4.rs"]
mod unit4;
#[path = "diagnostic_details/unit5.rs"]
mod unit5;
#[path = "diagnostic_details/unit6.rs"]
mod unit6;
#[path = "diagnostic_details/unit7.rs"]
mod unit7;
#[path = "diagnostic_details/unit8.rs"]
mod unit8;

fn check(source: &str, args: &[&str]) -> (i32, Value, String) {
    invoke("check", source, args)
}

fn invoke(command: &str, source: &str, args: &[&str]) -> (i32, Value, String) {
    // Process-specific names allow concurrent tests and leave no tracked input.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!(
        "diagnostic-{}-{}.relca",
        std::process::id(),
        std::thread::current().name().unwrap()
    ));
    std::fs::write(&file, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg(command)
        .arg(&file)
        .args(args)
        .output()
        .unwrap();
    std::fs::remove_file(file).unwrap();
    (
        output.status.code().unwrap(),
        serde_json::from_slice(&output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn location(line: usize, column: usize, end_line: usize, end_column: usize) -> Value {
    json!({"line":line,"column":column,"end_line":end_line,"end_column":end_column})
}

fn diagnostic<'a>(report: &'a Value, message: &str) -> &'a Value {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["message"] == message)
        .unwrap_or_else(|| panic!("missing {message}: {}", report["diagnostics"]))
}

#[test]
fn syntax_locations_and_stderr() {
    // Bad token at the end of the line; EOF is zero width, including after LF.
    for (src, expected) in [
        ("model M version !", location(1, 17, 1, 18)),
        ("model M version \"0.2\" {\n", location(2, 1, 2, 1)),
        ("model M version \"0.2\" {", location(1, 24, 1, 24)),
    ] {
        let (exit, result, stderr) = check(src, &[]);
        assert_eq!(exit, 3);
        let d = &result["diagnostics"][0];
        assert_eq!(d["code"], "E-SYN-0");
        assert_eq!(d.get("detail"), Some(&Value::Null));
        assert_eq!(d["location"], expected);
        assert_eq!(
            stderr,
            format!("E-SYN-0: {}\n", d["message"].as_str().unwrap())
        );
    }
}

const BASE: &str = r#"model M version "0.2" {
  type V = enum { ok }
  actor A { capacity 2
    capability make() -> V { duration fixed 1min; result nondet { ok } }
    capability first(x: V) -> V { duration fixed 1min; result nondet { ok } }
    capability second(x: V) -> V { duration fixed 1min; result nondet { ok } }
  }
  process P {
    d = A.make()
    a = A.first(d)
    b = A.second(d)
    end done
  }
  cut C { question "確認"; include [A] }
  bounds { cases 1; live_tasks 2; clock 10min }
  contract V { same_version A.first(x), A.second(x) }
}
"#;

#[test]
fn unicode_multiline_and_spanless_locations() {
    // The DSL has no comments. Its Japanese Cut question exercises the same
    // UTF-8 boundary, and `Unknown` begins at character 35, not byte 39.
    let (_, report, _) = check(&BASE.replace("include [A]", "include [Unknown]"), &[]);
    let d = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "E-NAME-0")
        .unwrap();
    assert_eq!(d["location"], location(14, 35, 14, 42));

    let (_, report, _) = check(BASE, &["--configuration", "Missing"]);
    let d = diagnostic(
        &report,
        "configuration selector requires configuration declarations",
    );
    assert_eq!(d["location"], location(1, 1, 17, 2));

    let (exit, report, _) = check(
        &BASE.replace("  cut C { question \"確認\"; include [A] }\n", ""),
        &[],
    );
    assert_eq!(exit, 0);
    let d = diagnostic(
        &report,
        "Cut is undeclared: all declared Actors are included; exclusions are unknown",
    );
    assert_eq!(d.get("span"), Some(&Value::Null));
    assert_eq!(d.get("location"), Some(&Value::Null));
    assert_eq!(d.get("detail"), Some(&Value::Null));
}

fn blocker(role: &str, name: Option<&str>, loc: Value) -> Value {
    json!({"role":role,"name":name,"location":loc})
}

fn at_exit(mut row: Value, round: u64, phase: &str, leave: Option<u64>) -> Value {
    let mut exit = json!({"loop_name":"rework","round":round,"phase":phase});
    if let Some(ordinal) = leave {
        exit["leave_site"] = json!(ordinal);
    }
    row["exits"] = json!([exit]);
    row
}

/// The string oracle uses only the hand-written positions and blockers, never
/// the CLI's JSON or formatter. Compare the whole block, including newlines,
/// and reject supplementary lines stranded elsewhere in stderr.
fn assert_supplement(stderr: &str, message: &str, at: (usize, usize), expected: &Value) {
    let (line, column) = at;
    let kind = expected["kind"].as_str().unwrap();
    let mut lines = vec![
        format!("  at {line}:{column}\n"),
        format!("  kind: {kind}\n"),
    ];
    if let Some(cause) = expected["cause"].as_str() {
        lines.push(format!("  cause: {cause}\n"));
    }
    for row in expected["blockers"].as_array().unwrap() {
        let role = row["role"].as_str().unwrap();
        let mut line = match role {
            "exit" => "  provenance:".to_owned(),
            "excluded_edge" => "  not counted: excluded_edge".to_owned(),
            _ => format!("  blocked by: {role}"),
        };
        if let Some(name) = row["name"].as_str() {
            line += &format!(" {name}");
        }
        if let Some(rounds) = row["rounds"].as_array() {
            let rounds = rounds
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            line += &format!(" (rounds {rounds})");
        }
        if let Some(exits) = row["exits"].as_array() {
            for exit in exits {
                line += &format!(
                    " in exit {} round {} {}",
                    exit["loop_name"].as_str().unwrap(),
                    exit["round"],
                    exit["phase"].as_str().unwrap()
                );
                if let Some(leave) = exit["leave_site"].as_u64() {
                    line += &format!(" leave {leave}");
                }
            }
        }
        if row["location"].is_object() {
            line += &format!(
                " at {}:{}",
                row["location"]["line"], row["location"]["column"]
            );
        }
        lines.push(line + "\n");
    }
    lines.push(format!("  see: {}\n", expected["see"].as_str().unwrap()));
    let header = format!("U-SEM-0: {message}\n");
    let mut stderr_lines = stderr.split_inclusive('\n');
    assert!(stderr_lines.by_ref().any(|line| line == header), "{stderr}");
    let actual = stderr_lines
        .take_while(|line| line.starts_with("  "))
        .collect::<Vec<_>>();
    assert_eq!(actual, lines, "{message}");
    assert_eq!(
        stderr.lines().filter(|line| line.starts_with("  ")).count(),
        lines.len()
    );
}

const SAME: &str = "same_version_feature_combination";
const SAME_SEE: &str = "docs/guide/dsl.md#確かめた版と使う版を揃える";

fn same(src: &str, args: &[&str], line: usize, cause: &str, kind: &str, blockers: Vec<Value>) {
    let (exit, report, stderr) = check(src, args);
    let d = diagnostic(&report, SAME);
    assert_eq!(exit, 4);
    assert_eq!(report["verdict"], "unsupported");
    assert_eq!(d["code"], "U-SEM-0");
    let expected =
        json!({"reason":SAME,"kind":kind,"cause":cause,"blockers":blockers,"see":SAME_SEE});
    assert_eq!(d["detail"], expected);
    assert_supplement(&stderr, SAME, (line, 16), &expected);
}

#[test]
fn same_version_negative_and_rewriting() {
    let (exit, report, _) = check(BASE, &[]);
    assert_eq!(exit, 0);
    assert!(
        !report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"] == SAME)
    );

    let non_result = BASE
        .replace("x: V", "x: CaseId")
        .replace("(d)", "(case.id)");
    same(
        &non_result,
        &[],
        16,
        "non_result_argument",
        "rewrite",
        vec![
            blocker("non_result_call", Some("a"), location(10, 5, 10, 25)),
            blocker("non_result_call", Some("b"), location(11, 5, 11, 26)),
        ],
    );
    let fork = non_result.replace(
        "    a = A.first(case.id)\n    b = A.second(case.id)",
        "    fork f { a = A.first(case.id); b = A.second(case.id) }\n    join all f",
    );
    same(
        &fork,
        &[],
        16,
        "non_result_argument",
        "rewrite",
        vec![
            blocker("non_result_call", Some("a"), location(10, 14, 10, 34)),
            blocker("non_result_call", Some("b"), location(10, 36, 10, 57)),
        ],
    );
}

#[test]
fn evidence_required_subset_exposes_candidate() {
    // x01's definite missing Extra takes precedence over uncertain Doc.
    let src = SNAPSHOT
        .replace(
            "actor Notes {}",
            "actor Notes {}\n  actor Extra { source document }",
        )
        .replace("include [A, Doc, Notes]", "include [A, Doc, Notes, Extra]");
    let (_, report, stderr) = check(&src.replace("A.use [Doc]", "A.use [Doc, Extra]"), &[]);
    assert_eq!(report["properties"][0]["verdict"], "violated");
    assert!(!stderr.contains(EB));
    evidence(
        &src,
        &[],
        26,
        vec![
            blocker("conditional_source", Some("Doc"), location(4, 3, 4, 32)),
            blocker("excluded_edge", Some("i -> d"), location(16, 5, 16, 17)),
            blocker("excluded_edge", Some("p -> d"), location(18, 18, 18, 31)),
            blocker("target_call", Some("u"), location(21, 5, 21, 16)),
        ],
    );
}

#[test]
fn same_version_unconnected_combinations() {
    let response = BASE.replace("d = A.make()", "d = A.make() response_loss none");
    same(
        &response,
        &[],
        16,
        "response",
        "analysis_not_connected",
        vec![blocker("response", Some("d"), location(9, 5, 9, 36))],
    );

    let degradation = BASE.replace(
        "    a = A.first(d)\n    b = A.second(d)",
        "    degrade D when d == ok { a = A.first(d) } else { b = A.second(d) }",
    );
    same(
        &degradation,
        &[],
        15,
        "degradation",
        "analysis_not_connected",
        vec![blocker("degradation", Some("D"), location(10, 5, 10, 71))],
    );

    let fork = BASE.replace("    a = A.first(d)\n    b = A.second(d)", "    fork f { a = A.first(d); b = A.second(d) }\n    join any f where result == ok tie nondet remaining drain if none -> done");
    same(
        &fork,
        &[],
        16,
        "join_any",
        "analysis_not_connected",
        vec![blocker("join_any", Some("f"), location(10, 5, 11, 77))],
    );

    let recovery = include_str!("snapshots/inputs/r2-recovery.relca").replace(
        "    capability ship_known(p: Pay)",
        "    capability confirm(p: Pay) -> Ack {duration fixed 1min; result nondet {recorded}}\n    capability ship_known(p: Pay)",
    ).replace("  scenario Full", "  contract V { same_version Shop.confirm(p), Shop.ship_known(p) }\n  scenario Full");
    same(
        &recovery,
        &["--scenario", "Full"],
        43,
        "recovery",
        "analysis_not_connected",
        vec![
            blocker("response", Some("pay"), location(27, 5, 37, 6)),
            blocker("recovery_handler", Some("pay"), location(28, 7, 36, 36)),
        ],
    );
}

const EB: &str = "evidence_basis_conditional_snapshot";
const EB_SEE: &str = "docs/guide/dsl.md#根拠源を求める契約";
const SNAPSHOT: &str = r#"model M version "0.2" {
  type V = enum { ok, fail }
  observed case { brief: optional<V> }
  actor Doc { source document }
  actor Notes {}
  actor A { capacity 2
    capability take() -> V { reads [Doc]; writes brief; duration fixed 1min; result nondet { ok } }
    capability patch() -> V { writes brief; duration fixed 1min; result nondet { ok } }
    capability choose() -> V { duration fixed 1min; result nondet { ok, fail } }
    capability draft() -> V { reads [brief]; duration fixed 1min; result nondet { ok } }
    capability save(d: V) -> V { registers [Notes]; duration fixed 1min; result nondet { ok } }
    capability use() -> V { reads [Notes]; duration fixed 1min; result nondet { ok } }
  }
  process P {
    i = A.take()
    c = A.choose()
    if c == ok { p = A.patch() } else {}
    d = A.draft()
    s = A.save(d)
    u = A.use()
    end done
  }
  cut C { include [A, Doc, Notes] }
  bounds { cases 1; live_tasks 2; clock 20min }
  contract Basis { evidence_basis A.use [Doc] }
}
"#;

fn evidence(src: &str, args: &[&str], line: usize, blockers: Vec<Value>) {
    let (exit, report, stderr) = check(src, args);
    let message = format!("contract Basis #1 evidence_basis: {EB}");
    let d = diagnostic(&report, &message);
    assert_eq!(exit, 4);
    assert_eq!(report["verdict"], "unsupported");
    assert_eq!(d["code"], "U-SEM-0");
    let expected = json!({"reason":EB,"kind":"property_undecided","cause":null,"blockers":blockers,"see":EB_SEE});
    assert_eq!(d["detail"], expected);
    assert_supplement(&stderr, &message, (line, 20), &expected);
}

#[test]
fn evidence_record_candidate_and_both_branches_control() {
    evidence(
        SNAPSHOT,
        &[],
        25,
        vec![
            blocker("conditional_source", Some("Doc"), location(4, 3, 4, 32)),
            blocker("excluded_edge", Some("i -> d"), location(15, 5, 15, 17)),
            blocker("excluded_edge", Some("p -> d"), location(17, 18, 17, 31)),
            blocker("target_call", Some("u"), location(20, 5, 20, 16)),
        ],
    );

    let control = SNAPSHOT
        .replace("} else {}", "} else { q = A.take() }")
        .replace(
            "patch() -> V { writes",
            "patch() -> V { reads [Doc]; writes",
        );
    let (exit, report, stderr) = check(&control, &[]);
    assert_eq!(exit, 0);
    assert_eq!(report["properties"][0]["verdict"], "satisfied");
    assert!(!stderr.contains(EB));
}

#[test]
fn evidence_fork_writer_and_optional_retry_attempt() {
    let fork = SNAPSHOT.replace("i = A.take()", "i = A.patch()").replace(
        "if c == ok { p = A.patch() } else {}",
        "fork f { p = A.take(); q = A.choose() }; join all f",
    );
    evidence(
        &fork,
        &[],
        25,
        vec![
            blocker("conditional_source", Some("Doc"), location(4, 3, 4, 32)),
            blocker("excluded_edge", Some("i -> d"), location(15, 5, 15, 18)),
            blocker("excluded_edge", Some("p -> d"), location(17, 14, 17, 26)),
            blocker("target_call", Some("u"), location(20, 5, 20, 16)),
        ],
    );

    let retry = SNAPSHOT.replace("writes brief; duration fixed 1min; result nondet { ok } }\n    capability choose", "writes brief; duration fixed 1min; result nondet { ok, fail } }\n    capability choose")
        .replace("p = A.patch()", "p = A.patch() response_loss none\n      retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end failed");
    evidence(
        &retry,
        &[],
        26,
        vec![
            blocker("conditional_source", Some("Doc"), location(4, 3, 4, 32)),
            blocker("excluded_edge", Some("i -> d"), location(15, 5, 15, 17)),
            blocker(
                "excluded_edge",
                Some("p (attempt 1) -> d"),
                location(17, 18, 18, 86),
            ),
            blocker(
                "excluded_edge",
                Some("p (attempt 2) -> d"),
                location(17, 18, 18, 86),
            ),
            blocker("target_call", Some("u"), location(21, 5, 21, 16)),
        ],
    );
}

const LOOP_HAND: &str = include_str!("diagnostic_details/l_p7.relca");

const MIXED_EVIDENCE: &str = include_str!("diagnostic_details/mixed_evidence.relca");
const MIXED_SAME: &str = include_str!("diagnostic_details/mixed_same_version.relca");

#[test]
fn candidates_are_tied_to_their_own_exit() {
    let body = |row| at_exit(row, 1, "body", Some(1));
    let limit = |row| at_exit(row, 1, "on_limit", Some(2));
    // `b1` is mandatory on both paths. `x1` can displace Policy on the
    // body exit; `x2` can displace Doc on the limit exit. Each exit has
    // exactly one mandatory register call (`r` or `r3`).
    evidence(
        include_str!("diagnostic_details/two_exits.relca"),
        &[],
        41,
        vec![
            limit(blocker(
                "conditional_source",
                Some("Doc"),
                location(5, 3, 5, 32),
            )),
            body(blocker(
                "conditional_source",
                Some("Policy"),
                location(6, 3, 6, 35),
            )),
            body(blocker(
                "excluded_edge",
                Some("b1 -> r"),
                location(23, 7, 23, 24),
            )),
            limit(blocker(
                "excluded_edge",
                Some("b1 -> r3"),
                location(23, 7, 23, 24),
            )),
            body(blocker(
                "excluded_edge",
                Some("x1 -> r"),
                location(27, 25, 27, 44),
            )),
            body(blocker("exit", None, location(29, 9, 29, 14))),
            limit(blocker(
                "excluded_edge",
                Some("x2 -> r3"),
                location(33, 22, 33, 41),
            )),
            limit(blocker("exit", None, location(35, 7, 35, 12))),
            body(blocker("target_call", Some("u"), location(37, 5, 37, 20))),
            limit(blocker("target_call", Some("u"), location(37, 5, 37, 20))),
        ],
    );
}

#[test]
fn diagnostic_see_targets_existing_dsl_sections() {
    let guide = include_str!("../../../docs/guide/dsl.md");
    for see in [
        EB_SEE,
        SAME_SEE,
        unit4::LOOP_SEE,
        unit4::REG_SEE,
        unit4::MERGE_SEE,
        unit5::SEE,
    ] {
        let wanted = see.strip_prefix("docs/guide/dsl.md#").unwrap();
        assert!(
            guide
                .lines()
                .filter(|line| line.starts_with('#'))
                .filter_map(|line| line.trim_start_matches('#').strip_prefix(' '))
                .any(|heading| heading.replace(' ', "-") == wanted),
            "{see}"
        );
    }
}

#[test]
fn independent_coordinate_oracle_matches_hand_intervals() {
    use relca_core::diag::Span;
    use relca_test_support::diagnostic::location as oracle;
    // The tab counts as one scalar. U+0301 is a separate scalar; the emoji
    // occupies four bytes but one column. CR is a scalar before LF.
    let text = "\tA😀e\u{301}\r\n猫\r\n";
    for (span, want) in [
        (Span { start: 1, end: 2 }, location(1, 2, 1, 3)),
        (Span { start: 2, end: 6 }, location(1, 3, 1, 4)),
        (Span { start: 9, end: 10 }, location(1, 6, 1, 7)),
        (Span { start: 11, end: 14 }, location(2, 1, 2, 2)),
        (Span { start: 6, end: 16 }, location(1, 4, 3, 1)),
        (Span { start: 16, end: 16 }, location(3, 1, 3, 1)),
    ] {
        assert_eq!(json!(oracle(text, Some(span))), want);
    }
    assert_eq!(oracle(text, Some(Span { start: 3, end: 6 })), None);
}

#[test]
fn snapshot_sources_include_transitive_arguments() {
    let doc = blocker("conditional_source", Some("Doc"), location(4, 3, 4, 32));
    let policy = blocker("conditional_source", Some("Policy"), location(5, 3, 5, 35));
    let i = blocker("excluded_edge", Some("i -> d"), location(15, 5, 15, 17));
    let p = blocker("excluded_edge", Some("p -> d"), location(17, 18, 17, 31));
    let n = blocker("excluded_edge", Some("n -> u"), location(19, 18, 19, 30));
    let u = blocker("target_call", Some("u"), location(20, 5, 20, 17));
    let standalone = MIXED_EVIDENCE
        .replace(" reads [memo];", "")
        .replace("[Doc, Policy]", "[Doc]");
    evidence(
        &standalone,
        &[],
        25,
        vec![doc.clone(), i.clone(), p.clone(), u.clone()],
    );
    evidence(
        MIXED_EVIDENCE,
        &[],
        25,
        vec![
            doc.clone(),
            policy.clone(),
            i.clone(),
            p.clone(),
            n.clone(),
            u.clone(),
        ],
    );
    // Both brief writers now read Doc, but exclusions are still lexical;
    // Policy remains uncertain because `n` may not run.
    evidence(
        &MIXED_EVIDENCE.replace("A.patch()", "A.take()"),
        &[],
        25,
        vec![
            policy,
            i.clone(),
            blocker("excluded_edge", Some("p -> d"), location(17, 18, 17, 30)),
            n,
            u.clone(),
        ],
    );
    evidence(
        &MIXED_EVIDENCE.replace("if c == ok { n = A.note() } else {}", "n = A.note()"),
        &[],
        25,
        vec![doc.clone(), i, p, u],
    );
    let transitive = standalone
        .replace("u = A.use(d)", "x = A.forward(d)\n    u = A.use(x)")
        .replace("    capability use", "    capability forward(x: V) -> V { duration fixed 1min; result nondet { ok } }\n    capability use");
    evidence(
        &transitive,
        &[],
        27,
        vec![
            doc,
            blocker("excluded_edge", Some("i -> d"), location(16, 5, 16, 17)),
            blocker("excluded_edge", Some("p -> d"), location(18, 18, 18, 31)),
            blocker("target_call", Some("u"), location(22, 5, 22, 17)),
        ],
    );

    let e4 = include_str!("../../relca-core/tests/bounded_repetition/models/eb-alias-e4.relca");
    let second = |row| at_exit(row, 2, "body", None);
    let doc = blocker("conditional_source", Some("Doc"), location(5, 3, 5, 32));
    // Only a round-two exit can carry the uncertain value. The single
    // `leave` has no ordinal and the exit row has no source span.
    evidence(
        e4,
        &["--configuration", "Loop"],
        49,
        vec![
            second(doc.clone()),
            second(blocker("target_call", Some("u"), location(27, 5, 27, 20))),
            second(blocker("exit", None, Value::Null)),
        ],
    );
    evidence(
        e4,
        &["--configuration", "Hand"],
        49,
        vec![
            doc,
            blocker("excluded_edge", Some("i -> d2"), location(31, 5, 31, 23)),
            blocker("excluded_edge", Some("p1 -> d2"), location(36, 23, 36, 41)),
            blocker("target_call", Some("u2"), location(39, 23, 39, 40)),
        ],
    );
}

#[test]
fn same_version_reports_later_unconnected_cause_without_changing_primary() {
    let a = blocker("non_result_call", Some("a"), location(10, 5, 10, 25));
    let b = blocker("non_result_call", Some("b"), location(11, 5, 11, 26));
    let e = blocker("response", Some("e"), location(12, 5, 12, 36));
    same(
        MIXED_SAME,
        &[],
        17,
        "non_result_argument",
        "analysis_not_connected",
        vec![a.clone(), b.clone(), e.clone()],
    );
    let reverse = MIXED_SAME
        .replace("    e = A.make() response_loss none\n", "")
        .replace(
            "    a = A.first",
            "    e = A.make() response_loss none\n    a = A.first",
        );
    same(
        &reverse,
        &[],
        17,
        "response",
        "analysis_not_connected",
        vec![
            blocker("response", Some("e"), location(10, 5, 10, 36)),
            blocker("non_result_call", Some("a"), location(11, 5, 11, 25)),
            blocker("non_result_call", Some("b"), location(12, 5, 12, 26)),
        ],
    );
    let three = MIXED_SAME.replace("    end done", "    fork f { g = A.make(); h = A.make() }\n    join any f where result == ok tie nondet remaining drain if none -> done\n    end done");
    same(
        &three,
        &[],
        19,
        "non_result_argument",
        "analysis_not_connected",
        vec![
            a.clone(),
            b.clone(),
            e,
            blocker("join_any", Some("f"), location(13, 5, 14, 77)),
        ],
    );
    same(
        &MIXED_SAME.replace("    e = A.make() response_loss none\n", ""),
        &[],
        16,
        "non_result_argument",
        "rewrite",
        vec![a, b],
    );
}

#[test]
fn evidence_exit_scopes_and_unrolled_control() {
    let scoped = |row| at_exit(row, 1, "on_limit", Some(2));
    evidence(
        LOOP_HAND,
        &["--configuration", "Loop"],
        54,
        vec![
            blocker("conditional_source", Some("Doc"), location(5, 3, 5, 32)),
            blocker("excluded_edge", Some("i -> r3"), location(29, 22, 29, 40)),
            blocker("excluded_edge", Some("p -> r3"), location(29, 50, 29, 67)),
            blocker("exit", None, location(31, 7, 31, 12)),
            blocker("target_call", Some("u"), location(33, 5, 33, 20)),
        ]
        .into_iter()
        .map(scoped)
        .collect(),
    );
    evidence(
        LOOP_HAND,
        &["--configuration", "Hand"],
        54,
        vec![
            blocker("conditional_source", Some("Doc"), location(5, 3, 5, 32)),
            blocker("excluded_edge", Some("i -> r3"), location(45, 22, 45, 40)),
            blocker("excluded_edge", Some("p -> r3"), location(45, 50, 45, 67)),
            blocker("target_call", Some("u2"), location(47, 7, 47, 24)),
        ],
    );
    // The baseline last-writer proof needs a dominating writer as well as
    // Doc at both candidate writes; it does not combine branch coverage.
    let control = LOOP_HAND
        .replace("p = Clerk.patch()", "p = Clerk.patchd()")
        .replace(
            "      if g == pass",
            "      base = Clerk.intake()\n      if g == pass",
        );
    for name in ["Loop", "Hand"] {
        let (exit, result, stderr) = check(&control, &["--configuration", name]);
        assert_eq!(exit, 0);
        assert_eq!(result["properties"][0]["verdict"], "satisfied");
        assert!(!stderr.contains(EB));
    }
}
