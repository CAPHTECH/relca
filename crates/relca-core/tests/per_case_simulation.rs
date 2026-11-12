//! Keys are independently spelled, SHA sampling is independent of runtime code.
use relca_core::{engine, reference, report};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[path = "support/percase_acceptance.rs"]
mod p;
fn key(rep: u64, case: u64, site: &str) -> String {
    format!("relca-sim-v1|seed=7|rep={rep}|case={case}|site={site}|n=1")
}
fn sample(s: &str) -> Vec<Value> {
    let s = if s.contains("simulation replications") {
        s.to_owned()
    } else {
        s.replace(
            "  bounds",
            "  simulation replications 4 seed 7 confidence 0.95\n  bounds",
        )
    };
    let m = p::model(&s);
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    vec![
        serde_json::to_value(engine::simulate(&m)).unwrap(),
        serde_json::to_value(r.outcome).unwrap(),
    ]
}
fn draws(rep: &Value) -> Vec<String> {
    rep["draws"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["key"].as_str().unwrap().to_owned())
        .collect()
}
fn difficult(key: &str) -> bool {
    let bytes = Sha256::digest(key.as_bytes());
    let k = u64::from_be_bytes(bytes[..8].try_into().unwrap()) as u128;
    5 * k >= 4 * (1u128 << 64)
}

#[test]
fn f12_complete_keys_single_legacy_single_arrivals_and_two_cases() {
    for name in ["pcase-keys", "pcase-keys.broken-unmarked-single"] {
        let src = p::source(name);
        for s in [
            src.clone(),
            src.replace("  bounds", "  arrivals fixed [0min]\n  bounds"),
        ] {
            let case = if name.contains("broken") { 0 } else { 1 };
            let runs = sample(&s);
            for v in &runs {
                assert_eq!(v["completed_replications"], 1);
                assert_eq!(
                    draws(&v["replications"][0]),
                    vec![key(1, case, "init.actor.Invoice.difficulty")]
                );
                assert_eq!(v["replications"][0]["cases"][0]["queries"], json!(["hit"]));
            }
            assert_eq!(runs[0]["replications"], runs[1]["replications"]);
        }
    }
    let s = p::source("pcase-joint");
    let runs = sample(&s);
    for v in &runs {
        assert_eq!(v["completed_replications"], 4);
        for (i, rep) in v["replications"].as_array().unwrap().iter().enumerate() {
            let ks = vec![
                key(i as u64 + 1, 1, "init.actor.Invoice.difficulty"),
                key(i as u64 + 1, 2, "init.actor.Invoice.difficulty"),
            ];
            assert_eq!(draws(rep), ks, "one key per admitted case, no shared case0");
            for (j, k) in ks.iter().enumerate() {
                assert_eq!(
                    rep["cases"][j]["queries"],
                    json!([if difficult(k) { "hit" } else { "miss" }])
                );
            }
        }
    }
    assert_eq!(runs[0]["replications"], runs[1]["replications"]);
}

#[test]
fn f4_f5_m14_rejected_future_and_late_draw_only_once_original_case_ids() {
    for (name, ids) in [
        ("pcase-reject", vec![1]),
        ("pcase-reject.broken-zero-queue", vec![]),
        ("pcase-late", vec![1, 2]),
        ("pcase-late.broken-clock", vec![1]),
    ] {
        for v in sample(&p::source(name)) {
            assert_eq!(v["completed_replications"], 4);
            for (i, rep) in v["replications"].as_array().unwrap().iter().enumerate() {
                assert_eq!(
                    draws(rep),
                    ids.iter()
                        .map(|c| key(i as u64 + 1, *c, "init.actor.Invoice.difficulty"))
                        .collect::<Vec<_>>(),
                    "{name}: no draws for Rejected/Future, no redraw for already received"
                );
            }
        }
    }
    let s = p::source("pcase-reject")
        .replace("[0min, 0min]", "[0min, 0min, 3min]")
        .replace("cases 2", "cases 3")
        .replace("clock 30min", "clock 4min");
    for v in sample(&s) {
        for (i, rep) in v["replications"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                draws(rep),
                vec![
                    key(i as u64 + 1, 1, "init.actor.Invoice.difficulty"),
                    key(i as u64 + 1, 3, "init.actor.Invoice.difficulty")
                ]
            );
            assert_eq!(rep["cases"][1]["terminal"]["kind"], "rejected");
        }
    }
}

#[test]
fn f3_joint_actor_case_factors_use_separate_sites_and_do_not_redraw() {
    let s = p::source("pcase-case-product").replace(
        "  bounds",
        "  simulation replications 64 seed 7 confidence 0.95\n  bounds",
    );
    let runs = sample(&s);
    for v in &runs {
        assert_eq!(v["completed_replications"], 64);
        for (i, rep) in v["replications"].as_array().unwrap().iter().enumerate() {
            let mut expected = vec![];
            for c in 1..=2 {
                let d = key(i as u64 + 1, c, "init.actor.Invoice.difficulty");
                let x = key(i as u64 + 1, c, "init.case.x");
                let digest = Sha256::digest(x.as_bytes());
                let yes = u64::from_be_bytes(digest[..8].try_into().unwrap()) < (1u64 << 62);
                assert_eq!(
                    rep["cases"][c as usize - 1]["queries"],
                    json!([if difficult(&d) && yes { "hit" } else { "miss" }])
                );
                expected.extend([d, x]);
            }
            let mut actual = draws(rep);
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected, "four factors, no combined/cohort key");
        }
    }
    assert_eq!(runs[0]["replications"], runs[1]["replications"]);
}

#[test]
fn f15_nondeterminism_is_not_uniform_and_fixed_has_no_initial_draw() {
    let s = p::source("pcase-nondet");
    for v in sample(&s) {
        assert_eq!(v["attempted_replications"], 1);
        assert_eq!(v["completed_replications"], 0);
        assert_eq!(v["replications"], json!([]));
        let ds = v["diagnostics"].as_array().unwrap();
        let d = ds.iter().find(|d| d["code"] == "U-SEM-0").unwrap();
        let message = d["message"].as_str().unwrap();
        assert!(
            message.contains("cases [1") && message.contains("init.actor.Invoice.difficulty"),
            "{d}"
        );
    }
    for v in sample(&p::source("pcase-nondet.broken-fixed")) {
        assert_eq!(v["completed_replications"], 4);
        for r in v["replications"].as_array().unwrap() {
            assert!(draws(r).is_empty());
        }
    }
    let v = serde_json::to_value(report::simulate(&s.replace(
        "  bounds",
        "  simulation replications 4 seed 7 confidence 0.95\n  bounds",
    )))
    .unwrap();
    assert_eq!(v["verdict"], "unsupported");
}

#[test]
fn f17_each_document_is_initialized_at_receipt_not_at_each_read() {
    for v in sample(&p::source("pcase-same-document")) {
        for (i, r) in v["replications"].as_array().unwrap().iter().enumerate() {
            let initial: Vec<_> = draws(r)
                .into_iter()
                .filter(|s| s.contains("site=init."))
                .collect();
            assert_eq!(
                initial,
                vec![
                    key(i as u64 + 1, 1, "init.actor.Invoice.difficulty"),
                    key(i as u64 + 1, 2, "init.actor.Invoice.difficulty")
                ]
            );
            for c in r["cases"].as_array().unwrap() {
                assert_eq!(
                    c["queries"][1], "miss",
                    "same-document mismatch is impossible"
                );
            }
        }
    }
}

#[test]
fn compare_common_random_catalog_uses_case_initial_sites_and_nested_assumptions() {
    use relca_core::compare::{self, Analysis};
    let s=p::source("pcase-common").replace("  bounds","  process Other { a = Reader.first(); b = Reader.second(); end done }\n  configuration A { cut Scope; process Flow }\n  configuration B { cut Scope; process Other }\n  simulation replications 4 seed 7 confidence 0.95\n  bounds");
    for mode in [Analysis::Exhaustive, Analysis::Simulation] {
        let v = serde_json::to_value(compare::compare(&s, &["A", "B"], mode)).unwrap();
        let shared = &v["common_random"]["shared"];
        if mode == Analysis::Simulation {
            assert_eq!(shared[0]["case_ids"], json!([1, 2]));
            assert_eq!(shared[0]["sites"], json!(["init.actor.Invoice.difficulty"]));
            assert_eq!(shared[1]["sites"], json!([]));
            assert_eq!(shared[1]["case_ids"], json!([0]));
        }
        for plan in v["plans"].as_array().unwrap() {
            p::assumptions(&plan["report"], &["Invoice"], true);
            assert_eq!(
                plan["report"]["agency"]["common_causes"]["causes"][0]["instance"],
                "per_case"
            );
        }
    }
}
