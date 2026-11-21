use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

const SRC: &str = include_str!("../../../fixtures/mean-repeat/model.relca");

fn once(source: &str, target: &str, replacement: &str) -> String {
    assert_eq!(source.matches(target).count(), 1, "{target}");
    source.replacen(target, replacement, 1)
}

fn both(source: &str, selection: Option<&str>) -> [Vec<Value>; 2] {
    both_with_limits(source, selection, reference::Limits::default())
}

fn both_with_limits(
    source: &str,
    selection: Option<&str>,
    limits: reference::Limits,
) -> [Vec<Value>; 2] {
    let model = lower::lower_with_selection(&syntax::parse(source).unwrap(), selection, None)
        .unwrap_or_else(|e| panic!("{selection:?}: {e:?}"));
    let r = reference::explore_with_limits(&model, limits);
    assert_eq!(r.stop, reference::StopReason::Complete);
    [engine::explore(&model), r.outcome].map(|out| {
        assert!(
            !out.diagnostics
                .iter()
                .any(|d| d.severity == relca_core::diag::Severity::Error),
            "{:?}",
            out.diagnostics
        );
        out.quality.unwrap().means.unwrap()
    })
}

fn equal_to_hand(source: &str, expected: &[Value]) {
    let looped = both(source, Some("Loop"));
    let hand = both(source, Some("Hand"));
    for values in looped.into_iter().chain(hand).map(|rows| {
        rows.into_iter()
            .map(|r| r["value"].clone())
            .collect::<Vec<_>>()
    }) {
        assert_eq!(values, expected);
    }
}

fn exact(value: &str) -> Value {
    json!({"lower":value,"upper":value,"basis":"exact","reason":null})
}

#[test]
fn repeat_means_match_unrolled_paths_including_the_limit() {
    equal_to_hand(SRC, &["9/2", "3", "15/2", "11/3"].map(exact));
    let r = report::check_with_selection(SRC, Some("Loop"), None);
    assert_eq!(r.metric_checks[0].verdict, relca_core::Verdict::Satisfied);
    let control = include_str!("../../../fixtures/mean-repeat.broken-no-gap/model.relca");
    for rows in both(control, Some("Loop")) {
        assert_eq!(
            rows.into_iter()
                .map(|r| r["value"].clone())
                .collect::<Vec<_>>(),
            ["3", "3", "15/2", "8/3"].map(exact)
        );
    }
}

#[test]
fn repeat_nonprobabilistic_choices_keep_the_unrolled_bounds() {
    let src = once(SRC, "outcomes kernel K", "result nondet { pass, fail }");
    let interval = |l, u| json!({"lower":l,"upper":u,"basis":"per_case_relaxation","reason":null});
    equal_to_hand(
        &src,
        &[
            interval("2", "7"),
            interval("2", "4"),
            interval("5", "10"),
            json!({"lower":null,"upper":null,"basis":null,"reason":"denominator_may_be_zero"}),
        ],
    );
    let r = report::check_with_selection(&src, Some("Loop"), None);
    assert_eq!(
        r.metric_checks[0].verdict,
        relca_core::Verdict::Inconclusive
    );
}

#[test]
fn fork_inside_each_round_matches_unrolled_occupancy() {
    let src = once(
        SRC,
        "  process Repeated",
        "  type Help = enum { ok }\n  actor B { capacity 1; capability help() -> Help { duration fixed 1min; result nondet { ok } } }\n  process Repeated",
    );
    let src = once(
        &src,
        "r = A.run()",
        "fork parts { r=A.run(); h=B.help() }; join all parts",
    );
    let src = once(
        &src,
        "r1 = A.run()",
        "fork first { r1=A.run(); h1=B.help() }; join all first",
    );
    let src = once(
        &src,
        "r2 = A.run()",
        "fork second { r2=A.run(); h2=B.help() }; join all second",
    );
    let src = once(&src, "include [A]", "include [A,B]");
    let src = once(
        &src,
        "rate A.run per_start 1",
        "rate A.run per_start 1; rate B per_minute 3; rate B.help per_start 0",
    );
    equal_to_hand(&src, &["9/2", "9/2", "12", "11/3"].map(exact));
}

#[test]
fn deterministic_sample_with_limit_matches_the_written_out_process() {
    let source = once(SRC, "outcomes kernel K", "result nondet { fail }");
    let source = once(
        &source,
        "  verify reachable end.done",
        "  simulation replications 2 seed 1 confidence 0.95\n  verify safety no_resource_overcommit",
    );
    for selection in ["Loop", "Hand"] {
        let model =
            lower::lower_with_selection(&syntax::parse(&source).unwrap(), Some(selection), None)
                .unwrap();
        let r = reference::simulate(&model);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for out in [engine::simulate(&model), r.outcome] {
            assert_eq!(
                out.completed_replications, 2,
                "{selection}: {:?}",
                out.diagnostics
            );
            let rows = out.quality.unwrap().means.unwrap();
            for (row, point) in rows.iter().zip(["7", "4", "10"]) {
                assert_eq!(
                    row["value"],
                    json!({"point":point,"observations":2,"censored":0,"reason":null})
                );
            }
            assert_eq!(rows[3]["value"]["reason"], "zero_denominator");
        }
    }
}

#[test]
fn split_first_examples_have_the_same_means() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
    let dir = root.join(format!("repeat-mean-split-first-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, src) in [
        (
            "repeat",
            include_str!("../../../skills/relca-design/examples/split-first-repeat.relca"),
        ),
        (
            "unrolled",
            include_str!("../../../skills/relca-design/examples/split-first.relca"),
        ),
    ] {
        let s = once(
            src,
            "  verify probability",
            r#"  metrics { window 400min; currency "JPY"
    rate Lead per_minute 1; rate Dev per_minute 1; rate Reviewer per_minute 1
    rate Lead.split per_start 0; rate Lead.integrate per_start 0; rate Lead.release per_start 0
    rate Dev.refine_a per_start 0; rate Dev.refine_b per_start 0
    rate Reviewer.check_plan per_start 0; rate Reviewer.check per_start 0
  }
  property case.Closed = reached(end.delivered || end.late_fail || end.escalated)
  property case.Delivered = reached(end.delivered)
  metric mean.Lead = mean(elapsed, case.Closed)
  metric mean.Work = mean(work)
  metric mean.Cost = mean(cost)
  metric mean.DoneLead = mean(elapsed, case.Delivered given reached)
  verify probability"#,
        );
        let path = dir.join(format!("{name}.relca"));
        std::fs::write(&path, s).unwrap();
        let source = std::fs::read_to_string(path).unwrap();
        // The reference auxiliary tree uses a branch-stack bound, even when
        // the legacy reference run itself finishes. Relax only the oracle's
        // stack limits here, not the model's clock or any shipping budget.
        let rows = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                both_with_limits(
                    &source,
                    None,
                    reference::Limits {
                        branch_depth: 2048,
                        ..reference::Limits::default()
                    },
                )
            })
            .unwrap()
            .join()
            .unwrap();
        for (i, rows) in rows.into_iter().enumerate() {
            let actual: Vec<_> = rows.into_iter().map(|r| r["value"].clone()).collect();
            assert_eq!(
                actual,
                ["3381201/25000", "4794801/25000", "4794801/25000", "4301/31"].map(exact),
                "{name}, backend {i}"
            );
        }
    }
}
