//! ADR 0022 追記 4 の残り: (1) M1 の修正を参照インタプリタでも実行して確かめる。
//! (2) contest モデルの状態の予算での切断を固定する。期待値は手計算か、完全な探索との関係。
use relca_core::{engine, lower, outcome::Outcome, reference, syntax};

const WITH: &str = r#"model ContestRuntime version "0.2" {
  type T = enum { pass, fail }
  people [j,p]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 4min; result nondet { fail } } }
  actor Route { executors [p]; capability respond() -> T { duration fixed 2min; result nondet { pass } } }
  actor Party { control party; standing over [end.returned] basis "claim" }
  permit j to Judge.judge
  permit p to Route.respond
  authority G { basis "decision"; grants j to Judge.judge }
  process Flow { d = Judge.judge(); if d == fail { end returned } else { end posted } }
  cut C { include [Judge,Route,Party] }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 4min }
}"#;

const SAME_END: &str = r#"model SameEnd version "0.2" {
  type T = enum { pass, fail }
  people [j,p]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 4min; result nondet { fail } } }
  actor Route { executors [p]; capability respond() -> T { duration fixed 2min; result nondet { pass } } }
  actor Party { control party; standing over [end.returned] basis "claim" }
  permit j to Judge.judge
  permit p to Route.respond
  authority G { basis "decision"; grants j to Judge.judge }
  process Flow { end returned }
  cut C { include [Judge,Route,Party] }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 4min }
}"#;

fn model(src: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(src).unwrap()).unwrap()
}

fn without_contest(src: &str) -> String {
    let line = src
        .lines()
        .find(|l| l.trim_start().starts_with("contract Controls { contest"))
        .unwrap();
    src.replace(line, "  contract Controls { }")
}

fn completed(o: &Outcome) -> Vec<(usize, u64, u64)> {
    o.case_outcomes
        .as_ref()
        .unwrap()
        .iter()
        .map(|c| {
            let t = c.terminals[0].completed_at.unwrap();
            (c.case_id.0, t.min, t.max)
        })
        .collect()
}

fn reference_outcome(m: &relca_core::ir::Model) -> Outcome {
    let r = reference::explore(m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    let mut o = r.outcome;
    o.diagnostics.drain(..m.diagnostics.len());
    o
}

#[test]
fn reference_and_engine_keep_pre_terminal_fifo_with_and_without_the_contest() {
    // Judge has one executor j; both cases arrive at 0. FIFO tie_by case_id
    // gives case1 (internal id 0) the judge 0..4 and case2 4..8, with or
    // without a contest.
    for src in [WITH.to_owned(), without_contest(WITH)] {
        let m = model(&src);
        let r = reference_outcome(&m);
        assert_eq!(completed(&r), [(0, 4, 4), (1, 8, 8)], "reference");
        assert_eq!(
            completed(&engine::explore(&m)),
            [(0, 4, 4), (1, 8, 8)],
            "engine"
        );
        assert_eq!(engine::explore(&m), r, "whole outcome");
    }
}

#[test]
fn reference_agrees_on_same_time_ends() {
    let m = model(SAME_END);
    assert_eq!(engine::explore(&m), reference_outcome(&m));
}

#[test]
fn a_state_budget_cut_on_a_contest_model_keeps_only_sound_prefix_facts() {
    let m = model(WITH);
    let full = engine::explore(&m);
    assert!(!full.truncated);
    let full_rows = full.contest_runtime.as_ref().unwrap();
    let mut cut_seen = false;
    for budget in 1..200 {
        let o = engine::explore_with_budget(&m, budget);
        if !o.truncated {
            assert_eq!(o, full, "budget {budget} complete must equal the full run");
            continue;
        }
        cut_seen = true;
        let rows = o.contest_runtime.as_ref().unwrap();
        assert_eq!(o.properties.len(), full.properties.len());
        for (p, whole) in o.properties.iter().zip(&full.properties) {
            // A cut never invents a violation or a success.
            if p.verdict == relca_core::Verdict::Violated {
                assert_eq!(
                    whole.verdict,
                    relca_core::Verdict::Violated,
                    "budget {budget}"
                );
            }
            // The static contest row is structural and does not depend on the cut.
            if matches!(p.property, relca_core::ir::Property::Contest(_)) {
                assert_eq!(p.verdict, whole.verdict, "budget {budget}");
                continue;
            }
            // Short of a witnessed violation, a cut run concludes nothing:
            // neither satisfied nor vacuous (the full run triggers every row).
            if p.verdict != relca_core::Verdict::Violated {
                assert_eq!(
                    p.verdict,
                    relca_core::Verdict::Inconclusive,
                    "budget {budget}: {:?}",
                    p.property
                );
            }
        }
        for row in rows {
            assert_ne!(
                row.coverage,
                relca_core::obligation::Coverage::Complete,
                "budget {budget}"
            );
            let whole = full_rows.iter().find(|r| r.id == row.id).unwrap();
            for a in &row.alternatives {
                if a.status == relca_core::contest::RuntimeStatus::Unresolved {
                    assert!(a.closed_at.is_none() && a.response_at.is_none());
                    assert!(a.route_work.is_none() && a.route_wait.is_none());
                    assert!(a.route_cost.min.is_none() && a.route_cost.max.is_none());
                } else {
                    // A settled branch seen on the explored prefix is a branch of the
                    // full run. Under a cut its cost is withdrawn (null, truncated),
                    // so the cost is compared only as withdrawn.
                    assert!(a.route_cost.min.is_none() && a.route_cost.max.is_none());
                    let same = |b: &relca_core::contest::Alternative| {
                        let mut b = b.clone();
                        b.route_cost = a.route_cost.clone();
                        b == *a
                    };
                    assert!(
                        whole.alternatives.iter().any(same),
                        "budget {budget}: {a:?}"
                    );
                }
            }
        }
        let open = &o.contest_summary.as_ref().unwrap().business_ended_open;
        assert!(open.min.is_none() && open.max.is_none(), "budget {budget}");
    }
    assert!(cut_seen, "no budget produced a cut");
}
