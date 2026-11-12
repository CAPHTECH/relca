//! No sampled-safety claim, and no monitoring draw or unselected witness.
#[path = "support/state_invariant_acceptance.rs"]
mod h;
use relca_core::{engine, reference, report};
use serde_json::{Value, json};
#[test]
fn deterministic_receipt_violation_aggregates_four_chosen_paths_in_both_routes() {
    for name in ["sinv-simulation", "sinv-simulation.broken-initial"] {
        let broken = name.contains("broken");
        let original = h::source(name);
        for arrivals in ["", "arrivals fixed [0min]"] {
            let s = if arrivals.is_empty() {
                original.clone()
            } else {
                h::change(
                    &original,
                    "  contract Controls",
                    &format!("  {arrivals}\n  contract Controls"),
                )
            };
            let m = h::model(&s);
            let e = engine::simulate(&m);
            let r = reference::simulate(&m);
            for out in [&e, &r.outcome] {
                assert_eq!(out.completed_replications, 4);
                assert_eq!(out.truncated_replications, Some(0));
                assert!(out.diagnostics.is_empty());
                assert_eq!(out.replications.len(), 4);
                for (i, rep) in out.replications.iter().enumerate() {
                    assert_eq!(rep.replication, i as u64 + 1);
                    assert!(rep.draws.is_empty(), "monitoring is not a random site");
                    assert!(rep.cut.is_none());
                    assert_eq!(rep.cases.len(), 1);
                    let c = &rep.cases[0];
                    assert_eq!(c.case_id, 1);
                    assert_eq!(c.terminal.as_ref().unwrap().end.as_deref(), Some("done"));
                    assert_eq!(c.violations, vec![broken]);
                }
            }
            assert_eq!(e.replications, r.outcome.replications);
            let v = serde_json::to_value(report::simulate(&s)).unwrap();
            h::subset(&h::expected(name), &v);
            let p = h::property(&v, "contract Controls #1 invariant");
            let c = &p["cases"][0];
            assert_eq!(c["basis"], if broken { "witness" } else { "sample" });
            assert_eq!(
                c["witness"],
                if broken {
                    json!({"replication":1})
                } else {
                    Value::Null
                }
            );
            assert_eq!(v["case_outcomes"][0]["terminals"][0]["count"], 4);
        }
    }
}
#[test]
fn unresolved_nondeterminism_retracts_statistics_instead_of_uniformizing() {
    let s = h::source("sinv-approval");
    let s = h::change(
        &s,
        "  contract Controls",
        "  simulation replications 4 seed 17 confidence 0.95\n  contract Controls",
    );
    let m = h::model(&s);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    for out in [&e, &r.outcome] {
        assert_eq!(out.completed_replications, 0);
        assert!(out.replications.is_empty());
        assert!(out.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
    }
    let v = serde_json::to_value(report::simulate(&s)).unwrap();
    assert_eq!(v["verdict"], "unsupported");
    assert!(v["bounds"]["truncated"].is_null());
    for p in v["properties"].as_array().unwrap() {
        assert_eq!(p["verdict"], "unsupported");
        for c in p["cases"].as_array().unwrap() {
            assert!(c["witness"].is_null());
        }
    }
}
#[test]
fn candidate_bad_positive_mass_is_not_a_witness_when_every_selected_branch_is_good() {
    // A degenerate distribution still goes through the probability runtime but
    // cannot select the bad ValueId0 placeholder. No monitoring draw is added.
    let base = h::source("sinv-seed-mass");
    let s = h::change(
        &base,
        "  contract Controls",
        "  simulation replications 4 seed 17 confidence 0.95\n  contract Controls",
    );
    let m = h::model(&s);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    for o in [&e, &r.outcome] {
        assert_eq!(o.completed_replications, 4);
        for rep in &o.replications {
            assert_eq!(rep.cases[0].violations, vec![false]);
        }
    }
    assert_eq!(e.replications, r.outcome.replications);
    let v = serde_json::to_value(report::simulate(&s)).unwrap();
    assert_eq!(v["verdict"], "inconclusive");
    assert!(h::property(&v, "contract Controls #1 invariant")["cases"][0]["witness"].is_null());
}

#[test]
fn positive_mass_unselected_bad_successor_does_not_leak_into_replication1_or2() {
    use sha2::{Digest, Sha256};
    let base = std::fs::read_to_string(h::root().join("fixtures/sim-keys/model.relca")).unwrap();
    let s = h::change(
        &base,
        "  bounds",
        "  contract Controls { invariant y == second }\n  bounds",
    );
    // Independent integer half-interval arithmetic, not engine/reference draws.
    for (rep, first) in [(1, false), (2, false), (3, true), (4, false)] {
        let key = format!("relca-sim-v1|seed=0|rep={rep}|case=1|site=kernel.P.a|n=1");
        let d = Sha256::digest(key.as_bytes());
        let k = u64::from_be_bytes(d[..8].try_into().unwrap());
        // Rep4 is deliberately computed, not assumed from a frequency target.
        if rep <= 3 {
            assert_eq!(k < (1u64 << 63), first);
        }
    }
    let m = h::model(&s);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    assert_eq!(e.completed_replications, 4);
    assert_eq!(e.replications, r.outcome.replications);
    assert!(!e.replications[0].cases[0].violations[0]);
    assert!(!e.replications[1].cases[0].violations[0]);
    assert!(e.replications[2].cases[0].violations[0]);
    let v = serde_json::to_value(report::simulate(&s)).unwrap();
    assert_eq!(
        h::property(&v, "contract Controls #1 invariant")["cases"][0]["witness"]["replication"],
        3
    );
}
