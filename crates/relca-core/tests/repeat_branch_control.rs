//! Each supported Loop and its explicitly unrolled Hand must match the
//! minute-by-minute table in repeat_branch_control.md, not just each other.
use relca_core::{Verdict, engine, ir, lower, outcome, reference, syntax};
use std::fmt::Write as _;

const MODEL: &str = r#"model BranchControl version "0.2" {
  type V = enum { pass, fail }
  actor A { capacity 1
    capability review() -> V { duration fixed 1min; result nondet { pass, fail } }
    capability work() -> V { duration fixed 2min; result nondet { pass } }
  }
  actor B { capacity 1
    capability work() -> V { duration fixed 3min; result nondet { pass } }
  }
  cut C { include [A,B] }
  bounds { cases 1; live_tasks 2; clock 30min }
  verify safety no_resource_overcommit
  process P { $PROCESS }
  $VERIFY
}"#;

type Terminal<'a> = (&'a str, (u64, u64), (u64, u64));

const fn exact(end: &str, elapsed: u64, work: u64) -> Terminal<'_> {
    (end, (elapsed, elapsed), (work, work))
}

fn model(process: &str, expected: &[Terminal<'_>]) -> String {
    let mut verifies = String::new();
    for (name, _, _) in expected {
        writeln!(verifies, "verify reachable end.{name}").unwrap();
    }
    MODEL
        .replace("$PROCESS", process)
        .replace("$VERIFY", &verifies)
}

fn observe<'a>(m: &'a ir::Model, o: &outcome::Outcome) -> Vec<Terminal<'a>> {
    let row = |end: ir::EndId, elapsed: outcome::Range, work: outcome::Range| {
        (
            m.ends[end.0].as_str(),
            (elapsed.min, elapsed.max),
            (work.min, work.max),
        )
    };
    let mut terminals: Vec<_> = if let Some(cases) = &o.case_outcomes {
        // WaitUntil uses the case-aware interpreter even for one case.
        assert_eq!(cases.len(), 1);
        cases[0]
            .terminals
            .iter()
            .map(|t| {
                let outcome::CaseTerminalKind::End(end) = t.kind else {
                    panic!("expected a normal terminal, got {t:?}");
                };
                row(end, t.elapsed.unwrap(), t.work.unwrap())
            })
            .collect()
    } else {
        o.terminals
            .iter()
            .map(|t| row(t.end, t.elapsed, t.work))
            .collect()
    };
    terminals.sort_unstable();
    terminals
}

fn backends(label: &str, src: &str, mut check: impl FnMut(&str, &ir::Model, &outcome::Outcome)) {
    let ast = syntax::parse(src).unwrap_or_else(|ds| panic!("{label}: {ds:?}\n{src}"));
    let m = lower::lower(&ast).unwrap_or_else(|ds| panic!("{label}: {ds:?}\n{src}"));
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete, "{label}");
    check("engine", &m, &engine::explore(&m));
    check("reference", &m, &r.outcome);
}

fn compare(
    label: &str,
    loop_src: &str,
    hand: &str,
    expected: &[Terminal<'_>],
    errors: &mut Vec<String>,
) {
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    // Check Hand first, in both backends, even when Loop is wrong.
    for (kind, process) in [("Hand", hand), ("Loop", loop_src)] {
        backends(label, &model(process, &expected), |backend, m, o| {
            let got = observe(m, o);
            if got != expected
                || o.truncated
                || o.properties.len() != 1 + expected.len()
                || o.properties.iter().any(|p| p.verdict != Verdict::Satisfied)
                || o.diagnostics
                    .iter()
                    .any(|d| d.code.starts_with("E-") || d.code.starts_with("U-"))
            {
                let verdicts: Vec<_> = o.properties.iter().map(|p| p.verdict).collect();
                errors.push(format!("{label} {kind}/{backend}: expected {expected:?}, got {got:?}; verdicts {verdicts:?}; diagnostics {:?}", o.diagnostics));
            }
        });
    }
}

#[derive(Clone, Copy, Debug)]
enum Exit {
    Again,
    Leave,
    End,
}

impl Exit {
    fn dsl(self) -> &'static str {
        match self {
            Self::Again => "again",
            Self::Leave => "leave",
            Self::End => "end early",
        }
    }
    fn hand(self, next: &str, leave: &str) -> String {
        match self {
            Self::Again => next,
            Self::Leave => leave,
            Self::End => "end early",
        }
        .into()
    }
}

struct Tail {
    name: &'static str,
    work: &'static str,
    exit: Exit,
    // Time and work added to the path that reaches the tail (independent of lowering).
    extra: (u64, u64),
}

// This release refuses `fork` and `wait until` in a repeat body or on_limit
// (loop_feature_combination), so those tails are not part of the matrix.
const TAILS: &[Tail] = &[
    Tail {
        name: "call",
        work: "a# = A.work();",
        exit: Exit::Leave,
        extra: (2, 2),
    },
    Tail {
        name: "wait",
        work: "wait 4min;",
        exit: Exit::Leave,
        extra: (4, 0),
    },
    Tail {
        name: "if",
        work: "if r# == pass { wait 5min } else { wait 6min };",
        exit: Exit::Leave,
        extra: (5, 0),
    },
    Tail {
        name: "again",
        work: "",
        exit: Exit::Again,
        extra: (0, 0),
    },
    Tail {
        name: "leave",
        work: "",
        exit: Exit::Leave,
        extra: (0, 0),
    },
    Tail {
        name: "end",
        work: "",
        exit: Exit::End,
        extra: (0, 0),
    },
];

impl Tail {
    fn loop_dsl(&self) -> String {
        let exit = if matches!(self.exit, Exit::End) {
            "end tail"
        } else {
            self.exit.dsl()
        };
        format!("{} {exit}", self.work.replace('#', ""))
    }
    fn hand(&self, suffix: &str, next: &str, leave: &str) -> String {
        let exit = self.exit.hand(next, leave);
        let exit = if matches!(self.exit, Exit::End) {
            "end tail"
        } else {
            &exit
        };
        format!("{} {exit}", self.work.replace('#', suffix))
    }
}

#[test]
fn body_matrix() {
    use Exit::*;
    let mut errors = vec![];
    for branch in [Again, Leave, End] {
        for tail in TAILS {
            let loop_src = format!(
                "repeat rounds at_most 2 {{ r = A.review(); if r == fail {{ {} }} else {{}}; {} }} on_limit {{ end limit }} end done",
                branch.dsl(),
                tail.loop_dsl()
            );
            let hand_round = |n: &str, next: &str| {
                let fail = branch.hand(next, "end done");
                let pass = tail.hand(n, next, "end done");
                format!("r{n} = A.review(); if r{n} == fail {{ {fail} }} else {{ {pass} }}")
            };
            let hand = if matches!((branch, tail.exit), (Again, Again)) {
                "r1 = A.review(); r2 = A.review(); end limit".into()
            } else {
                hand_round("1", &hand_round("2", "end limit"))
            };
            let (dt, dw) = tail.extra;
            let pass_end = if matches!(tail.exit, End) {
                "tail"
            } else {
                "done"
            };
            let expected = match (branch, tail.exit) {
                (Again, Again) => vec![exact("limit", 2, 2)],
                (Again, _) => vec![
                    (pass_end, (1 + dt, 2 + dt), (1 + dw, 2 + dw)),
                    exact("limit", 2, 2),
                ],
                (Leave, Again) => vec![("done", (1, 2), (1, 2)), exact("limit", 2, 2)],
                (Leave, Leave) => vec![("done", (1, 1 + dt), (1, 1 + dw))],
                (Leave, End) => vec![exact("done", 1, 1), exact("tail", 1, 1)],
                (End, Again) => vec![("early", (1, 2), (1, 2)), exact("limit", 2, 2)],
                (End, _) => vec![exact("early", 1, 1), exact(pass_end, 1 + dt, 1 + dw)],
            };
            compare(
                &format!("body {branch:?} / {}", tail.name),
                &loop_src,
                &hand,
                &expected,
                &mut errors,
            );
        }
    }
    assert!(errors.is_empty(), "\n{}", errors.join("\n"));
}

#[test]
fn on_limit_matrix() {
    use Exit::*;
    let mut errors = vec![];
    for branch in [Again, Leave, End] {
        for tail in TAILS {
            let loop_src = format!(
                "repeat rounds at_most 2 {{ wait 1min; again }} on_limit {{ r = A.review(); if r == fail {{ {} }} else {{}}; {} }} after = A.work(); end done",
                branch.dsl(),
                tail.loop_dsl()
            );
            let unsupported = match (branch, tail.name) {
                (_, "fork" | "wait until") => Some(("U-SEM-0", "loop_feature_combination")),
                (Again, _) | (_, "again") => Some(("E-CON-0", "loop_again_in_limit")),
                _ => None,
            };
            if let Some((code, reason)) = unsupported {
                // Again has no Hand equivalent in this phase. Fork/WaitUntil
                // individually have ordinary DSL equivalents but are not allowed here.
                let ds = syntax::parse(&model(&loop_src, &[]))
                    .and_then(|ast| lower::lower(&ast))
                    .unwrap_err();
                assert!(
                    ds.iter()
                        .any(|d| d.code == code && d.message.ends_with(reason)),
                    "{branch:?} / {}: {ds:?}",
                    tail.name
                );
                continue;
            }
            let fail = branch.hand("", "fail_done = A.work(); end done");
            let pass = tail.hand("1", "", "pass_done = A.work(); end done");
            let hand = format!(
                "wait 1min; wait 1min; r1 = A.review(); if r1 == fail {{ {fail} }} else {{ {pass} }}"
            );
            let (dt, dw) = tail.extra;
            let expected = match (branch, tail.exit) {
                (Leave, Leave) => vec![("done", (5, 5 + dt), (3, 3 + dw))],
                (Leave, End) => vec![exact("done", 5, 3), exact("tail", 3, 1)],
                (End, Leave) => vec![exact("early", 3, 1), exact("done", 5 + dt, 3 + dw)],
                (End, End) => vec![exact("early", 3, 1), exact("tail", 3, 1)],
                _ => unreachable!(),
            };
            compare(
                &format!("on_limit {branch:?} / {}", tail.name),
                &loop_src,
                &hand,
                &expected,
                &mut errors,
            );
        }
    }
    assert!(errors.is_empty(), "\n{}", errors.join("\n"));
}

#[test]
fn nested_and_two_transferring_branches() {
    let mut errors = vec![];
    for (name, body, hand, expected) in [
        (
            "nested body",
            "r = A.review(); if r == fail { if r == fail { again } else {} } else {}; leave",
            "r1 = A.review(); if r1 == pass { end done } else { r2 = A.review(); if r2 == pass { end done } else { end limit } }",
            vec![("done", (1, 2), (1, 2)), exact("limit", 2, 2)],
        ),
        (
            "again / end",
            "r = A.review(); if r == pass { end early } else { again }; leave",
            "r1 = A.review(); if r1 == pass { end early } else { r2 = A.review(); if r2 == pass { end early } else { end limit } }",
            vec![("early", (1, 2), (1, 2)), exact("limit", 2, 2)],
        ),
        (
            "again / leave",
            "r = A.review(); if r == fail { again } else { leave }; leave",
            "r1 = A.review(); if r1 == pass { end done } else { r2 = A.review(); if r2 == pass { end done } else { end limit } }",
            vec![("done", (1, 2), (1, 2)), exact("limit", 2, 2)],
        ),
        (
            "leave / end",
            "r = A.review(); if r == pass { end early } else { leave }; again",
            "r1 = A.review(); if r1 == pass { end early } else { end done }",
            vec![exact("done", 1, 1), exact("early", 1, 1)],
        ),
    ] {
        let loop_src =
            format!("repeat rounds at_most 2 {{ {body} }} on_limit {{ end limit }} end done");
        compare(name, &loop_src, hand, &expected, &mut errors);
    }
    for (name, limit, hand_limit, expected) in [
        (
            "nested on_limit",
            "if r == fail { if r == fail { leave } else {} } else {}; end tail",
            "if r == fail { after = A.work(); end done } else { end tail }",
            vec![exact("done", 4, 3), exact("tail", 2, 1)],
        ),
        (
            "on_limit leave / end",
            "if r == fail { leave } else { end early }; end tail",
            "if r == fail { after = A.work(); end done } else { end early }",
            vec![exact("done", 4, 3), exact("early", 2, 1)],
        ),
    ] {
        let loop_src = format!(
            "repeat rounds at_most 1 {{ wait 1min; again }} on_limit {{ r = A.review(); {limit} }} after = A.work(); end done"
        );
        let hand = format!("wait 1min; r = A.review(); {hand_limit}");
        compare(name, &loop_src, &hand, &expected, &mut errors);
    }
    assert!(errors.is_empty(), "\n{}", errors.join("\n"));
}

#[test]
fn three_round_probability_and_work() {
    let loop_src = "repeat rounds at_most 3 { r = A.review(); if r == fail { a = A.work(); again } else {}; leave } on_limit { end limit } end done";
    let hand = "r1 = A.review(); if r1 == pass { end done } else { a1 = A.work();
                r2 = A.review(); if r2 == pass { end done } else { a2 = A.work();
                r3 = A.review(); if r3 == pass { end done } else { a3 = A.work(); end limit } } }";
    let expected = [("done", (1, 7), (1, 7)), exact("limit", 9, 9)];
    for (kind, process) in [("Hand", hand), ("Loop", loop_src)] {
        let src = model(process, &expected)
            .replace("actor A", "observed case { coin: bool = true }\n kernel K given [coin] { true -> { 0.75: pass; 0.25: fail }; false -> { 1: pass } }\n actor A")
            .replace("result nondet { pass, fail }", "outcomes kernel K")
            .replace("verify reachable end.limit", "verify reachable end.limit\n verify probability reached end.done within 9min >= 0.984375\n verify probability reached end.limit within 9min >= 0.015625");
        backends(kind, &src, |backend, m, o| {
            assert_eq!(observe(m, o), expected, "{kind}/{backend}");
            assert!(!o.truncated);
            assert_eq!(o.properties.len(), 5);
            assert!(o.properties.iter().all(|p| p.verdict == Verdict::Satisfied));
            let fractions: Vec<_> = o
                .properties
                .iter()
                .filter_map(|p| p.probability.as_ref())
                .map(|p| (p.min.to_string(), p.max.to_string()))
                .collect();
            assert_eq!(
                fractions,
                [
                    ("63/64".into(), "63/64".into()),
                    ("1/64".into(), "1/64".into())
                ],
                "{kind}/{backend}"
            );
        });
    }
}
