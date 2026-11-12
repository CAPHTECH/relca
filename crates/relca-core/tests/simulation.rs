//! ADR 0016 public acceptance: hand-authored oracles precede production code.
//! No engine-private seams or production statistics helpers are used here.
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use relca_core::{diff, report};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
#[path = "support/authority_population.rs"]
mod authority_population;
#[path = "support/conflict_population.rs"]
mod conflict_population;
#[path = "support/configuration_diff_population.rs"]
mod diff2_population;
#[path = "support/percase_acceptance.rs"]
mod pcase_population;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn source(name: &str) -> String {
    fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
fn simulated(src: &str) -> Value {
    serde_json::to_value(report::simulate(src)).unwrap()
}
fn fixture(name: &str) -> Value {
    simulated(&source(name))
}
fn row(v: &Value, query: usize, case: usize) -> &Value {
    &v["properties"][query]["cases"][case]
}
fn subset(expected: &Value, actual: &Value, path: &str) {
    match expected {
        Value::Object(o) => {
            for (key, value) in o {
                if key == "absent_diagnostics" {
                    for forbidden in value.as_array().unwrap() {
                        assert!(
                            !actual["diagnostics"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|d| d["code"] == *forbidden)
                        );
                    }
                } else {
                    assert!(actual.get(key).is_some(), "{path}: missing {key}");
                    subset(value, &actual[key], &format!("{path}.{key}"));
                }
            }
        }
        Value::Array(es) => {
            let vs = actual
                .as_array()
                .unwrap_or_else(|| panic!("{path}: array required"));
            if path.ends_with("diagnostics") {
                for e in es {
                    let a = vs.iter().find(|v| v["code"] == e["code"]).unwrap();
                    subset(e, a, path);
                }
            } else {
                assert_eq!(es.len(), vs.len(), "{path}");
                for (i, e) in es.iter().enumerate() {
                    let a = if path.ends_with("properties") {
                        vs.iter().find(|v| v["id"] == e["id"]).unwrap()
                    } else {
                        &vs[i]
                    };
                    subset(e, a, &format!("{path}[{i}]"));
                }
            }
        }
        _ => assert_eq!(expected, actual, "{path}"),
    }
}

#[test]
fn simulation_24_hand_authored_fixtures_and_single_fault_pairs() {
    let mut names: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| {
            e.file_type().unwrap().is_dir() && e.file_name().to_string_lossy().starts_with("sim-")
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names.len(), 24);
    let mut counts = [0; 3]; // completed / runtime U / static invalid
    for name in &names {
        let h: Value = serde_json::from_str(
            &fs::read_to_string(root().join(name).join("expected.json")).unwrap(),
        )
        .unwrap();
        let actual = fixture(name);
        subset(&h, &actual, name);
        let slot = if h["verdict"] == "invalid" {
            2
        } else if h["verdict"] == "unsupported" {
            1
        } else {
            0
        };
        counts[slot] += 1;
    }
    assert_eq!(counts, [20, 2, 2]);
    for (base, broken, before, after) in [
        (
            "keys",
            "flipped-query",
            "verify probability reached z == first",
            "verify probability reached z == second",
        ),
        ("hazard", "short-clock", "clock 3min", "clock 1min"),
        (
            "actor-initial",
            "missing-distribution",
            " ~ {first:0.5,second:0.5}",
            "",
        ),
        ("cut", "short-clock", "clock 3min", "clock 2min"),
        ("fifo", "no-policy", "  policy fifo tie_by case_id\n", ""),
        (
            "reachable",
            "unseen-end",
            "verify reachable end.done",
            "verify reachable end.other",
        ),
        ("contract", "short-response", "by 2min", "by 1min"),
        ("deadlock", "zero-capacity", "capacity 1", "capacity 0"),
        (
            "rejection",
            "positive-queue",
            "queue capacity 0",
            "queue capacity 1",
        ),
        (
            "required",
            "missing-simulation",
            "  simulation replications 4 seed 0 confidence 0.95\n",
            "",
        ),
        (
            "range",
            "zero-replications",
            "replications 1",
            "replications 0",
        ),
        ("statistical", "tight-threshold", "<= 0.75", "<= 0.25"),
    ] {
        let a = source(&format!("sim-{base}"));
        assert_eq!(a.matches(before).count(), 1, "single fault {base}");
        assert_eq!(
            a.replacen(before, after, 1),
            source(&format!("sim-{base}.broken-{broken}"))
        );
    }
}

fn rational(n: u64, d: u64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn parse_rational(s: &str) -> BigRational {
    let (n, d) = s.split_once('/').unwrap_or((s, "1"));
    BigRational::new(n.parse::<BigInt>().unwrap(), d.parse::<BigInt>().unwrap())
}
/// Direct binomial coefficient sum, not the production tail recurrence.
fn tail(n: u32, x: u32, p: &BigRational, upper: bool) -> BigRational {
    let mut sum = BigRational::zero();
    for j in 0..=n {
        if (upper && j < x) || (!upper && j > x) {
            continue;
        }
        let mut coefficient = BigInt::one();
        for k in 0..j {
            coefficient = coefficient * (n - k) / (k + 1);
        }
        sum += BigRational::from_integer(coefficient)
            * p.pow(j as i32)
            * (BigRational::one() - p).pow((n - j) as i32);
    }
    sum
}
fn grid(n: u32, x: u32, alpha: &BigRational, lower: bool) -> BigRational {
    if lower && x == 0 {
        return BigRational::zero();
    }
    if !lower && x == n {
        return BigRational::one();
    }
    let (mut lo, mut hi) = (0, 1048576);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        let t = tail(n, x, &rational(mid, 1048576), lower);
        if t == *alpha {
            return rational(mid, 1048576);
        }
        if (t < *alpha) == lower {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    rational(if lower { lo } else { hi }, 1048576)
}
fn statistical(hit: bool, n: u64, confidence: &str, queries: &str) -> String {
    format!(
        "model M version \"0.2\" {{\nobserved case {{ hit:bool = {hit} }}\nprocess P {{ end done }}\nbounds {{ cases 1; live_tasks 1; clock 1min }}\n{queries}\nsimulation replications {n} seed 0 confidence {confidence}\n}}\n"
    )
}

#[test]
fn s_exact_equality_all_four_predicates_and_degenerate_endpoints() {
    for (hit, query, expected, lo, hi) in [
        (true, ">= 0.25", "satisfied", "1/4", "1"),
        (true, "<= 0.25", "inconclusive", "1/4", "1"),
        (false, "<= 0.75", "satisfied", "0", "3/4"),
        (false, ">= 0.75", "inconclusive", "0", "3/4"),
        (true, ">= 0", "satisfied", "1/4", "1"),
        (true, "<= 0", "violated", "1/4", "1"),
        (true, "<= 1", "satisfied", "1/4", "1"),
        (true, ">= 1", "inconclusive", "1/4", "1"),
        (false, ">= 0", "satisfied", "0", "3/4"),
        (false, "<= 0", "inconclusive", "0", "3/4"),
        (false, ">= 1", "violated", "0", "3/4"),
        (false, "<= 1", "satisfied", "0", "3/4"),
    ] {
        let v = simulated(&statistical(
            hit,
            1,
            "0.5",
            &format!("verify probability reached hit == true {query}"),
        ));
        assert_eq!(row(&v, 0, 0)["verdict"], expected, "{hit} {query}");
        assert_eq!(row(&v, 0, 0)["interval"], json!({"lower":lo,"upper":hi}));
    }
}

#[test]
fn s_exact_tail_direct_coefficient_oracle_and_public_verdicts() {
    for (n, p, fs, gs, denominator) in [
        (
            4,
            rational(1, 2),
            vec![1, 5, 11, 15, 16],
            vec![16, 15, 11, 5, 1],
            16,
        ),
        (2, rational(1, 4), vec![9, 15, 16], vec![16, 7, 1], 16),
    ] {
        for x in 0..=n {
            assert_eq!(tail(n, x, &p, false), rational(fs[x as usize], denominator));
            assert_eq!(tail(n, x, &p, true), rational(gs[x as usize], denominator));
        }
    }
    for hit in [false, true] {
        for n in [1, 2, 4] {
            for (text, p) in [
                ("0", rational(0, 1)),
                ("0.25", rational(1, 4)),
                ("0.5", rational(1, 2)),
                ("0.75", rational(3, 4)),
                ("1", rational(1, 1)),
            ] {
                for op in ["<=", ">="] {
                    let x = if hit { n } else { 0 };
                    let a = rational(1, 4);
                    let (s, v) = if op == "<=" {
                        (
                            (x == n && p.is_one()) || (x < n && tail(n, x, &p, false) <= a),
                            x > 0 && tail(n, x, &p, true) < a,
                        )
                    } else {
                        (
                            (x == 0 && p.is_zero()) || (x > 0 && tail(n, x, &p, true) <= a),
                            x < n && tail(n, x, &p, false) < a,
                        )
                    };
                    let expected = if s {
                        "satisfied"
                    } else if v {
                        "violated"
                    } else {
                        "inconclusive"
                    };
                    let out = simulated(&statistical(
                        hit,
                        n.into(),
                        "0.5",
                        &format!("verify probability reached hit == true {op} {text}"),
                    ));
                    assert_eq!(row(&out, 0, 0)["verdict"], expected, "n{n}/h{x}/{op}{text}");
                }
            }
        }
    }
}

#[test]
fn s_display_outward_grid_and_not_a_verdict_oracle() {
    let v = fixture("sim-cut");
    let lower = parse_rational(row(&v, 0, 0)["interval"]["lower"].as_str().unwrap());
    let step = rational(1, 1048576);
    assert_eq!(lower, grid(4, 4, &rational(1, 8), true));
    assert!(lower.pow(4) <= rational(1, 8));
    assert!((&lower + &step).pow(4) > rational(1, 8));
    let v = fixture("sim-rejection");
    let upper = parse_rational(row(&v, 0, 0)["interval"]["upper"].as_str().unwrap());
    assert_eq!(upper, grid(4, 0, &rational(1, 8), false));
    assert!((BigRational::one() - &upper).pow(4) <= rational(1, 8));
    assert!((BigRational::one() - upper + step).pow(4) > rational(1, 8));
    // Exact decimal between floor grid 623487/Q and true fourth root of 1/8.
    let p = parse_rational("59460355/100000000");
    assert!(p > lower && p.pow(4) < rational(1, 8));
    let s = source("sim-cut").replace("<= 0.5", "<= 0.59460355");
    assert_eq!(
        row(&simulated(&s), 0, 0)["verdict"],
        "violated",
        "do not compare rounded lower to threshold"
    );
}

#[test]
fn s_bonferroni_rows_include_rejected_and_unarrived_cases() {
    let query = "verify probability reached hit == true <= 0.5";
    let a = statistical(false, 4, "0.875", query);
    let one = simulated(&a);
    assert_eq!(row(&one, 0, 0)["verdict"], "satisfied");
    assert_eq!(row(&one, 0, 0)["interval"]["upper"], "1/2");
    let b = a.replace(
        query,
        &format!("{query}\nverify probability reached hit == true within 1min <= 0.5"),
    );
    let two = simulated(&b);
    assert_eq!(two["simulation"]["multiplicity"]["rows"], 2);
    assert_eq!(two["simulation"]["multiplicity"]["alpha_after"], "1/32");
    assert_eq!(row(&two, 0, 0)["verdict"], "inconclusive");
    let v = fixture("sim-rejection");
    assert_eq!(v["simulation"]["multiplicity"]["rows"], 2);
    for c in v["properties"][0]["cases"].as_array().unwrap() {
        assert_eq!(c["trials"], 4);
    }
    let s = source("sim-rejection").replace("[0min,1min]", "[0min,8min]");
    let v = simulated(&s);
    assert_eq!(v["simulation"]["multiplicity"]["rows"], 2);
    assert_eq!(row(&v, 0, 1)["unknown"], 4);
    assert_eq!(row(&v, 0, 1)["trials"], 4);
}

#[test]
fn s_cut_relative_clock_live_known_end_reject_and_unarrived() {
    let s=source("sim-hazard").replace("duration {1min:0.5,3min:0.5}","duration fixed 2min")
        .replace("clock 3min","clock 7min")
        .replace("verify probability reached hit == true >= 0", "arrivals fixed [5min,6min]\npolicy fifo tie_by case_id\nverify probability reached hit == true within 1min >= 0\nverify probability reached hit == true within 2min >= 0")
        .replace("cases 1","cases 2");
    let v = simulated(&s);
    for (q, c, h, u) in [(0, 0, 0, 0), (0, 1, 0, 0), (1, 0, 4, 0), (1, 1, 0, 4)] {
        assert_eq!(row(&v, q, c)["hits"], h);
        assert_eq!(row(&v, q, c)["unknown"], u);
    }
    assert_eq!(
        v["case_outcomes"][0]["terminals"][0]["count"], 4,
        "completed case survives global cut"
    );
    assert_eq!(v["case_outcomes"][1]["unfinished_count"], 4);
    let live = source("sim-cut.broken-short-clock")
        .replace("live_tasks 1", "live_tasks 0")
        .replace("hit == true <=", "hit == true within 2min <=")
        .replace("hit == true >=", "hit == true within 2min >=");
    let v = simulated(&live);
    assert_eq!(
        row(&v, 0, 0)["unknown"],
        4,
        "clock exemption cannot apply to live cut"
    );
    let clock = live.replace("live_tasks 0", "live_tasks 1");
    assert_eq!(row(&simulated(&clock), 0, 0)["unknown"], 0);
}

#[test]
fn s_nondet_observed_dead_branches_and_later_refusal_withdraws_witnesses() {
    let dead = "model M version \"0.2\" {\ntype T = enum {a,b}\nobserved case {flag:bool = false}\nactor A {capacity 1; capability c() -> T {duration fixed 1min; result nondet {a,b}}}\nprocess P {if flag == true {x=A.c();end other} else {end done}}\nbounds {cases 1;live_tasks 1;clock 3min}\nverify reachable end.done\nsimulation replications 4 seed 0 confidence 0.5\n}";
    assert_eq!(
        simulated(dead)["verdict"],
        "satisfied",
        "unvisited nondet is not a static refusal"
    );
    let s=source("sim-keys")
        .replace("process P { a = A.c(); if a == first { end yes } else { end no } }", "process P { a = A.c(); if a == first { b = B.c(); end yes } else { end no } }")
        .replace("  bounds", "  actor B { capacity 1; capability c() -> T { duration fixed 1min; result nondet {first,second} } }\n  contract C { invariant A.c.finish -> y == first }\n  bounds");
    // rep1/2 pick second: invariant violations. First nondet is only rep3, after first kernel result.
    let v = simulated(&s);
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(v["simulation"]["attempted_replications"], 3);
    assert_eq!(v["simulation"]["completed_replications"], 2);
    assert!(v["bounds"]["truncated"].is_null());
    assert!(v["admission"].is_null());
    assert_eq!(v["case_outcomes"][0]["terminals"], json!([]));
    for p in v["properties"].as_array().unwrap() {
        assert_eq!(p["verdict"], "unsupported");
        for c in p["cases"].as_array().unwrap() {
            for k in ["hits", "unknown", "trials", "interval"] {
                assert!(c.get(k).is_none());
            }
            assert_eq!(c["basis"], "sample");
            assert!(c["witness"].is_null());
        }
    }
    let d = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "U-SEM-0")
        .unwrap();
    assert!(d["message"].as_str().unwrap().contains("replication 3"));
    assert!(d["span"].is_object());
    // FIFO does not resolve two starts in the same case.
    let fork = source("sim-deadlock")
        .replace("a = A.a();", "fork f {a=A.a();b=A.a()}; join all f;")
        .replace("live_tasks 1", "live_tasks 2")
        .replace("  bounds", "  policy fifo tie_by case_id\n  bounds");
    assert_eq!(simulated(&fork)["verdict"], "unsupported");
}

fn append_simulation(s: &str) -> String {
    let i = s.rfind('}').unwrap();
    format!(
        "{}\n simulation replications 4 seed 0 confidence 0.5\n{}",
        &s[..i],
        &s[i..]
    )
}
// ADR 0016 addendum (U8): spans follow real source bytes. Map spans in the
// appended source back across the inserted clause; every other byte must match.
fn unshift_spans(v: &mut Value, at: u64, len: u64) {
    if let Some(o) = v.as_object_mut() {
        if let Some(sp) = o.get_mut("span").and_then(Value::as_object_mut) {
            for k in ["start", "end"] {
                if let Some(n) = sp.get(k).and_then(Value::as_u64)
                    && n > at
                {
                    sp.insert(k.into(), json!(n - len));
                }
            }
        }
        for x in o.values_mut() {
            unshift_spans(x, at, len);
        }
    } else if let Some(a) = v.as_array_mut() {
        for x in a {
            unshift_spans(x, at, len);
        }
    }
}
fn remove_hash(v: &mut Value) {
    if let Some(o) = v.as_object_mut() {
        if let Some(m) = o.get_mut("model").and_then(Value::as_object_mut) {
            m.remove("sha256");
        }
        for x in o.values_mut() {
            remove_hash(x);
        }
    } else if let Some(a) = v.as_array_mut() {
        for x in a {
            remove_hash(x);
        }
    }
}

#[path = "support/agency_viewpoints_acceptance.rs"]
mod agency_viewpoints_population;
#[path = "support/actor_observed_population.rs"]
mod aobs_population;
#[path = "support/completion_population.rs"]
mod done_population;
#[path = "support/effect_standing_population.rs"]
mod effect_population;
#[path = "support/integer_acceptance.rs"]
mod integer_population;
#[path = "support/limits_population.rs"]
mod limits_population;
#[path = "support/common_cause_population.rs"]
mod lin_population;
#[path = "support/mutation_default_population.rs"]
mod mutation_default_population;
#[path = "support/policy_population.rs"]
mod policy_population;
#[path = "support/record_population.rs"]
mod record_population;
#[path = "support/requires_acceptance.rs"]
mod req_population;
#[path = "support/resource_acceptance.rs"]
mod resource_population;
#[path = "support/responsibility_extension_population.rs"]
mod resp2_population;
#[path = "support/scenario_population.rs"]
mod scenario_population;
#[path = "support/state_invariant_population.rs"]
mod sinv_population;

#[test]
fn s_check_ignored_all_148_old_inputs_check_and_diff_json_keys_and_assumptions() {
    let mut count = 0;
    let mut compare_names = Vec::new();
    let mut responsibility_names = Vec::new();
    let mut metrics_names = Vec::new();
    for entry in fs::read_dir(root()).unwrap().map(|e| e.unwrap()) {
        if !entry.file_type().unwrap().is_dir()
            || entry.file_name().to_string_lossy().starts_with("sim-")
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if scenario_population::fixture(&name)
            || (include!("support/waits_names.rs") as &[&str]).contains(&name.as_str())
        {
            continue;
        }
        if sinv_population::NAMES.contains(&name.as_str()) {
            continue;
        }
        if lin_population::fixture(&name)
            || aobs_population::fixture(&name)
            || conflict_population::fixture(&name)
            || serde_json::from_str::<Vec<String>>(include_str!("support/caseset_names.json"))
                .unwrap()
                .contains(&name)
            || done_population::fixture(&name)
            || req_population::fixture(&name)
            || authority_population::fixture(&name)
            || resp2_population::fixture(&name)
            || pcase_population::fixture(&name)
            || integer_population::fixture(&name)
            || diff2_population::fixture(&name)
            || resource_population::fixture(&name)
            || agency_viewpoints_population::fixture(&name)
            || policy_population::fixture(&name)
            || limits_population::fixture(&name)
            || effect_population::fixture(&name)
            || record_population::fixture(&name)
            || mutation_default_population::fixture(&name)
            || include!("support/quality_names.rs").contains(&name.as_str())
            || serde_json::from_str::<Vec<String>>(include_str!("support/rule_names.json"))
                .unwrap()
                .contains(&name)
            || include!("support/evaluation_names.rs").contains(&name.as_str())
            || include!("support/quantile_names.rs").contains(&name.as_str())
            || include!("support/acceptance_gap_names.rs").contains(&name.as_str())
        {
            continue;
        }
        // Keep the historical 148 inputs separate from ADR 0023 acceptance.
        const ACTOR_FIXTURES: [&str; 28] = [
            "act-arrival-units",
            "act-arrival-units.broken-later-first",
            "act-composite",
            "act-composite.broken-team-work",
            "act-control",
            "act-control.broken-explicit-environment",
            "act-cut-context",
            "act-cut-context.broken-no-cut",
            "act-debtor",
            "act-debtor-actor",
            "act-debtor-actor.broken-omitted",
            "act-debtor-value",
            "act-debtor-value.broken-other-person",
            "act-debtor.broken-unknown",
            "act-defaults",
            "act-defaults.broken-explicit-boundary",
            "act-grain",
            "act-grain.broken-swapped",
            "act-horizon",
            "act-horizon.broken-short-horizon",
            "act-legacy",
            "act-legacy.broken-longer-task",
            "act-overlap",
            "act-overlap.broken-relation",
            "act-units",
            "act-units.broken-duration-day",
            "act-via",
            "act-via.broken-upstream-only",
        ];
        if ACTOR_FIXTURES.contains(&name.as_str()) {
            continue;
        }
        if name.starts_with("explain-") {
            const EXPLAIN_FIXTURES: [&str; 12] = [
                "explain-deadlock",
                "explain-deadlock.broken-zero-capacity",
                "explain-executors",
                "explain-executors.broken-shared-worker",
                "explain-invariant",
                "explain-invariant.broken-cut-guard",
                "explain-once",
                "explain-once.broken-repeat-result",
                "explain-requires",
                "explain-requires.broken-cut-guard",
                "explain-response",
                "explain-response.broken-late-finish",
            ];
            assert!(
                EXPLAIN_FIXTURES.contains(&name.as_str()),
                "unclassified explain fixture {name}"
            );
            continue;
        }
        if name.starts_with("mut-") {
            const MUTATION_FIXTURES: [&str; 12] = [
                "mut-empty",
                "mut-empty.broken-invalid",
                "mut-evidence",
                "mut-evidence.broken-no-independent-read",
                "mut-gate",
                "mut-gate.broken-unsupported",
                "mut-guard",
                "mut-guard.broken-no-contract",
                "mut-limit",
                "mut-limit.broken-many-sites",
                "mut-origin",
                "mut-origin.broken-route-only",
            ];
            assert!(
                MUTATION_FIXTURES.contains(&name.as_str()),
                "unclassified mutation fixture {name}"
            );
            continue;
        }
        if name.starts_with("met-") {
            assert!(
                METRICS_FIXTURES.contains(&name.as_str()),
                "unclassified metrics fixture {name}"
            );
            metrics_names.push(name);
            continue;
        }
        if name.starts_with("resp-") {
            assert!(
                RESPONSIBILITY_FIXTURES.contains(&name.as_str()),
                "unclassified responsibility fixture {name}"
            );
            responsibility_names.push(name);
            continue;
        }
        if name.starts_with("compare-") {
            assert!(
                COMPARE_FIXTURES.contains(&name.as_str()),
                "unclassified compare fixture {name}"
            );
            compare_names.push(name);
            continue;
        }
        // D1 is independently accepted; the historical 148 inputs stay fixed.
        const D1: [&str; 24] = [
            "ctx-excludes",
            "ctx-excludes.broken-start",
            "ctx-nonwaivable",
            "ctx-nonwaivable.broken-waive",
            "ctx-obligation",
            "ctx-obligation.broken-excludes-target",
            "ctx-waive",
            "ctx-waive-authority",
            "ctx-waive-authority.broken-no-authority",
            "ctx-waive-names",
            "ctx-waive-names.broken-unknown-response",
            "ctx-waive.broken-after-deadline",
            "ctx-contest",
            "ctx-contest-branches",
            "ctx-contest-branches.broken-zero-capacity",
            "ctx-contest-clock",
            "ctx-contest-clock.broken-short-clock",
            "ctx-contest-metrics",
            "ctx-contest-metrics.broken-missing-route-rate",
            "ctx-contest-origin",
            "ctx-contest-origin.broken-arrival-clock",
            "ctx-contest-resource",
            "ctx-contest-resource.broken-slow-route",
            "ctx-contest.broken-late",
        ];
        if D1.contains(&name.as_str()) {
            continue;
        }
        let dir = entry.path();
        let s = fs::read_to_string(dir.join("model.relca")).unwrap();
        let added = append_simulation(&s);
        let inv: Value = fs::read_to_string(dir.join("invocation.json"))
            .ok()
            .map(|s| serde_json::from_str(&s).unwrap())
            .unwrap_or(json!({"command":"check"}));
        let run = |src: &str| -> Value {
            if inv["command"] == "diff" {
                serde_json::to_value(diff::diff(
                    src,
                    inv["from"].as_str().unwrap(),
                    inv["to"].as_str().unwrap(),
                ))
                .unwrap()
            } else if let Some(c) = inv["configuration"].as_str() {
                serde_json::to_value(report::check_configuration(src, c)).unwrap()
            } else {
                serde_json::to_value(report::check(src)).unwrap()
            }
        };
        let (mut before, mut after) = (run(&s), run(&added));
        remove_hash(&mut before);
        remove_hash(&mut after);
        let at = s.rfind('}').unwrap() as u64;
        unshift_spans(&mut after, at, (added.len() - s.len()) as u64);
        assert_eq!(
            before,
            after,
            "{}: complete check/diff JSON including assumptions and spans",
            dir.display()
        );
        count += 1;
    }
    assert_eq!(count, 148, "do not shrink the existing population");
    compare_names.sort();
    assert_eq!(compare_names, COMPARE_FIXTURES);
    responsibility_names.sort();
    assert_eq!(responsibility_names, RESPONSIBILITY_FIXTURES);
    metrics_names.sort();
    assert_eq!(metrics_names, METRICS_FIXTURES);
}

const METRICS_FIXTURES: [&str; 24] = [
    "met-cancel",
    "met-cancel.broken-drain",
    "met-compare",
    "met-compare.broken-longer-task",
    "met-diff",
    "met-diff.broken-free-start",
    "met-exclusions",
    "met-exclusions.broken-larger-queue",
    "met-fifo",
    "met-fifo.broken-shorter-second-step",
    "met-legacy",
    "met-legacy.broken-enabled",
    "met-missing-rate",
    "met-missing-rate.broken-free-rate",
    "met-parallel",
    "met-parallel.broken-one-slot",
    "met-person",
    "met-person.broken-shared-person",
    "met-positive-support",
    "met-positive-support.broken-positive-early-time",
    "met-sample",
    "met-sample.broken-short-clock",
    "met-serial",
    "met-serial.broken-longer-task",
];

const RESPONSIBILITY_FIXTURES: [&str; 19] = [
    "resp-contest",
    "resp-contest.broken-no-authority",
    "resp-contest.broken-no-permission",
    "resp-contest.broken-route-excluded",
    "resp-contest.broken-zero-deadline",
    "resp-cut",
    "resp-cut.broken-no-explicit-cut",
    "resp-diff",
    "resp-diff.broken-no-authorized-decision",
    "resp-missing-duty",
    "resp-missing-duty.broken-no-duty",
    "resp-origin",
    "resp-origin.broken-no-direct-read",
    "resp-refusal",
    "resp-refusal.broken-cut-guard",
    "resp-standing",
    "resp-standing.broken-not-party",
    "resp-state",
    "resp-state.broken-no-direct-read",
];

const COMPARE_FIXTURES: [&str; 16] = [
    "compare-arrivals",
    "compare-arrivals.broken-slow-plan",
    "compare-case-diff",
    "compare-case-diff.broken-same-kernel",
    "compare-cut",
    "compare-cut.broken-other-cut",
    "compare-legacy",
    "compare-legacy.broken-longer-task",
    "compare-random",
    "compare-random.broken-alias-process",
    "compare-reject",
    "compare-reject.broken-zero-queue",
    "compare-terminal",
    "compare-terminal.broken-other-end",
    "compare-unlock",
    "compare-unlock.broken-excluded-call",
];

#[test]
fn s_syntax_matrix_semantic_syntax_scope_context_words_and_byte_spans() {
    let base = source("sim-range");
    let clause = "simulation replications 1 seed 18446744073709551615 confidence 0.5";
    let matrix = [
        (
            "simulation replications 0 seed 0 confidence 0.5",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications 10001 seed 0 confidence 0.5",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications -1 seed 0 confidence 0.5",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications 18446744073709551616 seed 0 confidence 0.5",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed -1 confidence 0.5",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed 18446744073709551616 confidence 0.5",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed 0 confidence 0",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed 0 confidence 1",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed 0 confidence -0",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed 0 confidence -0.5",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed 0 confidence 1.1",
            "E-SIM-0",
            "invalid",
        ),
        (
            "simulation replications seed 0 confidence 0.5",
            "E-SYN-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed confidence 0.5",
            "E-SYN-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed 0 confidence",
            "E-SYN-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed 0 confidence 0.",
            "E-SYN-0",
            "invalid",
        ),
        (
            "simulation replications 1; seed 0 confidence 0.5",
            "E-SYN-0",
            "invalid",
        ),
        (
            "simulation replications 1 seed 0 confidence 0. 5",
            "E-SYN-0",
            "invalid",
        ),
        (
            "simulation seed 0 replications 1 confidence 0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation { replications 1 seed 0 confidence 0.5 }",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation Settings replications 1 seed 0 confidence 0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications 1.5 seed 0 confidence 0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications 1e1 seed 0 confidence 0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications +1 seed 0 confidence 0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications 1/2 seed 0 confidence 0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications R seed 0 confidence 0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications 1 seed 0.5 confidence 0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications 1 seed +1 confidence 0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications 1 seed random confidence 0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications 1 seed 0 confidence 1/2",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications 1 seed 0 confidence 5e-1",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications 1 seed 0 confidence +0.5",
            "U-SYN-0",
            "unsupported",
        ),
        (
            "simulation replications 1 seed 0 confidence 0.5 interval wilson",
            "U-SYN-0",
            "unsupported",
        ),
    ];
    for (text, code, verdict) in matrix {
        let s = base.replace(clause, text);
        for v in [
            simulated(&s),
            serde_json::to_value(report::check(&s)).unwrap(),
        ] {
            assert_eq!(v["verdict"], verdict, "{text}");
            let errors: Vec<_> = v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["severity"] == "error")
                .collect();
            assert!(!errors.is_empty(), "{text}");
            assert!(
                errors.iter().all(|d| d["code"] == code),
                "{text}: {errors:?}"
            );
            assert!(v["bounds"]["cases"].is_null());
            assert!(v["agency"].is_null());
            for d in errors {
                let start = d["span"]["start"].as_u64().unwrap() as usize;
                let end = d["span"]["end"].as_u64().unwrap() as usize;
                assert!(
                    start <= end
                        && end <= s.len()
                        && s.is_char_boundary(start)
                        && s.is_char_boundary(end)
                );
            }
        }
    }
    for digits in [63, 64] {
        // integer zero plus decimal digits = 64/65
        let s = base.replace(
            "confidence 0.5",
            &format!("confidence 0.{}1", "0".repeat(digits - 1)),
        );
        assert_eq!(
            simulated(&s)["verdict"],
            if digits == 63 { "satisfied" } else { "invalid" }
        );
    }
    for r in [1, 10000] {
        let s = base.replace("replications 1", &format!("replications {r}"));
        assert_eq!(simulated(&s)["verdict"], "satisfied");
    }
    let duplicate = base.replace(clause, &format!("{clause}\n{clause}"));
    let v = simulated(&duplicate);
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0")
    );
    let newline = base.replace(clause, &clause.replace(' ', "\n"));
    assert_eq!(simulated(&newline)["verdict"], "satisfied");
    let words = "type T = enum { simulation, replications, seed, confidence }\nactor simulation { capacity 1; capability seed() -> T {duration fixed 1min;result nondet {confidence}}}\n";
    let s = base.replace(
        "  process P { end done }",
        &format!("{words}process P {{replications=simulation.seed();end done}}"),
    );
    assert_eq!(simulated(&s)["verdict"], "satisfied");
    for extra in [
        "warmup 1min",
        "measurement 1min",
        "drain",
        "stop_arrivals_at 1min",
        "early_stop",
        "sequential_test",
        "interval wilson",
        "random_method pcg",
        "compare A B",
        "metrics throughput",
        "arrivals renewal_every 1min",
        "arrivals poisson 1",
        "policy random",
        "policy fixed_order",
    ] {
        let s = base.replace(clause, &format!("{clause}\n{extra}"));
        assert_ne!(s, base, "unsupported suffix control must edit the source");
        let v = simulated(&s);
        assert_eq!(v["verdict"], "unsupported", "{extra}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SYN-0"),
            "{extra}"
        );
    }
    for scope in ["actor A", "process Q", "contract C", "bounds"] {
        let s = base.replace(clause, &format!("{scope} {{{clause}}}"));
        let v = simulated(&s);
        assert_eq!(v["verdict"], "unsupported", "{scope}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SYN-0")
        );
    }
}

#[test]
fn s_statistical_consistency_is_not_exhaustive_probability_equality() {
    let v = fixture("sim-statistical");
    let c = row(&v, 0, 0);
    assert_eq!(
        (c["hits"].as_u64(), c["trials"].as_u64()),
        (Some(4956), Some(10000))
    );
    assert!(parse_rational(c["interval"]["lower"].as_str().unwrap()) <= rational(1, 2));
    assert!(parse_rational(c["interval"]["upper"].as_str().unwrap()) >= rational(1, 2));
    let s =
        source("sim-statistical").replace("  verify probability reached hit == true >= 0.5\n", "");
    let v = simulated(&s);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(
        v["simulation"]["multiplicity"]["alpha_after"],
        "1/4000000000000"
    );
}

#[test]
fn simulation_configuration_public_entry_does_not_change_check_selection_rules() {
    let s = append_simulation(&source("config-diff-add-actor"));
    let v = serde_json::to_value(report::simulate_configuration(&s, "AsIs")).unwrap();
    assert_eq!(v["mode"], "simulation");
    assert_eq!(v["verdict"], "inconclusive", "safety absence is not proof");
    assert_eq!(v["case_outcomes"][0]["terminals"][0]["count"], 4);
    let v = serde_json::to_value(report::simulate_configuration(&s, "Unknown")).unwrap();
    assert_eq!(v["verdict"], "invalid");
    let new = s.replace("  bounds", "  arrivals fixed [0min]\n  bounds");
    let v = serde_json::to_value(report::simulate_configuration(&new, "AsIs")).unwrap();
    assert_eq!(v["verdict"], "inconclusive", "safety absence is not proof");
    assert_eq!(v["case_outcomes"][0]["terminals"][0]["count"], 4);
    let v = serde_json::to_value(report::simulate_configuration(&new, "Unknown")).unwrap();
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-NAME-0")
    );
    let v = serde_json::to_value(report::simulate(&new)).unwrap();
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CONF-0")
    );
}

#[test]
fn s_witness_independent_public_unselected_kernel_branch_and_inactive_contract() {
    let s = source("sim-keys").replace(
        "  bounds",
        "  contract C { invariant A.c.finish -> y == second }\n  bounds",
    );
    let v = simulated(&s);
    let p = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "contract C #1 invariant")
        .unwrap();
    assert_eq!(p["verdict"], "violated");
    assert_eq!(p["cases"][0]["basis"], "witness");
    assert_eq!(
        p["cases"][0]["witness"]["replication"], 3,
        "rep1/2 choose second; speculative first-branch violation must not leak"
    );
    let inactive=s.replace("contract C { invariant A.c.finish", "actor Unused { capacity 1; capability c() -> T {duration fixed 1min;result nondet {first}} }\n  contract C { invariant Unused.c.finish");
    let v = simulated(&inactive);
    let p = v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "contract C #1 invariant")
        .unwrap();
    assert_eq!(
        p["verdict"], "inconclusive",
        "unseen antecedent is not vacuous"
    );
    assert_eq!(p["cases"][0]["basis"], "sample");
    assert!(p["cases"][0]["witness"].is_null());
}

#[test]
fn s_witness_independent_public_executor_contracts_single_allocation() {
    let permitted =
        append_simulation(&source("exec-permitted").replace("executors [p,q]", "executors [p]"));
    let authority =
        append_simulation(&source("exec-authority").replace("executors [p,q]", "executors [p]"));
    let separation = append_simulation(&source("exec-separation"));
    for (correct, broken, id) in [
        (
            &permitted,
            permitted.replace("  permit p to Runner.run\n", ""),
            "contract Controls #1 permitted",
        ),
        (
            &authority,
            authority.replace("grants p to Runner.run; ", ""),
            "contract Controls #1 authority_basis",
        ),
        (
            &separation,
            separation.replace("executors [a1]", "executors [c1]"),
            "contract Controls #1 separation",
        ),
    ] {
        let a = simulated(correct);
        let b = simulated(&broken);
        let find = |v: &Value| {
            v["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == id)
                .unwrap()
                .clone()
        };
        let a = find(&a);
        let b = find(&b);
        assert_eq!(
            a["verdict"], "inconclusive",
            "{id}: sampled compliance is not proof"
        );
        assert_eq!(a["cases"][0]["basis"], "sample");
        assert_eq!(
            b["verdict"], "violated",
            "{id}: allocation must not be silently filtered"
        );
        assert_eq!(b["cases"][0]["basis"], "witness");
        assert_eq!(b["cases"][0]["witness"]["replication"], 1);
    }
}

#[test]
fn s_syntax_matrix_numeric_full_lexeme_utf8_and_missing_labels() {
    // Non-ASCII bytes before the clause make byte spans differ from char indices.
    let base = source("sim-range").replacen(
        "  verify reachable end.done\n",
        "  verify reachable end.done\n  cut K { question \"日本語で byte span を検査\" }\n",
        1,
    );
    assert!(
        base.contains("日本語"),
        "non-ASCII bytes must precede the clause"
    );
    for (before, after, lexeme) in [
        ("replications 1", "replications -12", "-12"),
        (
            "replications 1",
            "replications 18446744073709551616",
            "18446744073709551616",
        ),
        ("seed 18446744073709551615", "seed -1", "-1"),
        (
            "seed 18446744073709551615",
            "seed 18446744073709551616",
            "18446744073709551616",
        ),
        ("confidence 0.5", "confidence -0.0", "-0.0"),
    ] {
        let s = base.replace(before, after);
        let v = simulated(&s);
        let d = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["code"] == "E-SIM-0")
            .unwrap();
        let start = d["span"]["start"].as_u64().unwrap() as usize;
        let end = d["span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&s[start..end], lexeme);
        assert_eq!(v["verdict"], "invalid");
    }
    let clause = "simulation replications 1 seed 18446744073709551615 confidence 0.5";
    for replacement in [
        "simulation 1 seed 0 confidence 0.5",
        "simulation replications 1 0 confidence 0.5",
        "simulation replications 1 seed 0 0.5",
        "simulation replications 1 confidence 0.5",
        "simulation replications 1 seed 0",
        "simulation replications",
        "simulation",
    ] {
        let s = base.replace(clause, replacement);
        let v = simulated(&s);
        assert_eq!(v["verdict"], "invalid", "{replacement}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-SYN-0"),
            "{replacement}"
        );
    }
    for extra in [
        "simulation replications 1+1 seed 0 confidence 0.5",
        "simulation replications 1 seed 1/2 confidence 0.5",
        "simulation replications 1 seed 1e0 confidence 0.5",
        "simulation replications 1 seed 0 confidence 0.5+0.1",
    ] {
        let v = simulated(&base.replace(clause, extra));
        assert_eq!(v["verdict"], "unsupported", "{extra}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SYN-0")
        );
    }
    let s = base.replace(
        "confidence 0.5",
        &format!("confidence 0.{}", "9".repeat(63)),
    );
    assert_eq!(simulated(&s)["verdict"], "satisfied");
    let v=simulated(&base.replace("  process P { end done }",&format!("actor A {{capacity 1;capability c() -> T {{{clause}}}}}\ntype T = enum {{ok}}\nprocess P {{end done}}")));
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SYN-0")
    );
}
