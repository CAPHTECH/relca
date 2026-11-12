//! State-invariant identity must flow through generic compare/preserve/mutation.
#[path = "support/state_invariant_acceptance.rs"]
mod h;
#[path = "support/response_boundary_oracle.rs"]
mod response_boundary;
use relca_core::{compare, diff, mutation};
use serde_json::json;
#[test]
fn compare_exhaustive_and_sample_keep_same_id_without_safety_overclaim() {
    for (mode, analysis) in [
        ("exhaustive", compare::Analysis::Exhaustive),
        ("simulation", compare::Analysis::Simulation),
    ] {
        for bad in [false, true] {
            let name = format!(
                "sinv-compare-{mode}{}",
                if bad { ".broken-bad-plan" } else { "" }
            );
            let v =
                serde_json::to_value(compare::compare(&h::source(&name), &["A", "B"], analysis))
                    .unwrap();
            h::subset(&h::expected(&name), &v);
            assert!(v["mismatch"].is_null());
            assert_eq!(v["plans"].as_array().unwrap().len(), 2);
            let rows = v["tables"]["properties"].as_array().unwrap();
            assert_eq!(rows.len(), 1);
            let row = &rows[0];
            assert_eq!(row["id"], "contract Controls #1 invariant");
            let safe = if mode == "simulation" {
                "inconclusive"
            } else {
                "satisfied"
            };
            for (i, plan) in ["A", "B"].iter().enumerate() {
                let verdict = if bad && i == 1 { "violated" } else { safe };
                assert_eq!(v["plans"][i]["configuration"], *plan);
                let report = &v["plans"][i]["report"];
                assert_eq!(
                    h::property(report, "contract Controls #1 invariant")["verdict"],
                    verdict
                );
                assert_eq!(row["values"][i]["configuration"], *plan);
                assert_eq!(row["values"][i]["verdict"], verdict);
                if mode == "simulation" {
                    assert_eq!(report["simulation"]["completed_replications"], 4);
                    assert_eq!(report["case_outcomes"][0]["terminals"][0]["count"], 4);
                } else {
                    assert_eq!(report["terminals"][0]["elapsed"], json!({"min":1,"max":1}));
                }
            }
        }
    }
}
#[test]
fn preserve_selector_counts_vacuous_not_satisfied() {
    for bad in [false, true] {
        let name = if bad {
            "sinv-preserve.broken-vacuous"
        } else {
            "sinv-preserve"
        };
        let v = serde_json::to_value(diff::diff(&h::source(name), "From", "To")).unwrap();
        h::subset(&h::expected(name), &v);
        assert_eq!(
            v["preservation"]["properties"],
            json!([{"id":"contract Controls #1 invariant","from":"satisfied","to":if bad{"vacuous"}else{"satisfied"}}])
        );
        let summary = &v["preservation"]["summary"];
        assert_eq!(summary["satisfied"], u64::from(!bad));
        assert_eq!(summary["vacuous"], u64::from(bad));
        for key in ["violated", "inconclusive", "unsupported", "invalid"] {
            assert_eq!(summary[key], 0);
        }
        assert_eq!(
            v["to"]["report"]["terminals"][0]["elapsed"],
            json!({"min":2,"max":2})
        );
    }
}
#[test]
fn mutation_refusal_cut_has_one_site_and_exact_invariant_assigned_change() {
    const UNSUPPORTED: [&str; 2] = ["stale_definition_write", "old_promise_denominator_removed"];
    for bad in [false, true] {
        let name = if bad {
            "sinv-mutation.broken-no-invariant"
        } else {
            "sinv-mutation"
        };
        let v = serde_json::to_value(mutation::mutate(&h::source(name))).unwrap();
        assert_eq!(v["verdict"], "unsupported");
        assert_eq!(v["diagnostics"], json!([]));
        assert_eq!(
            v["original"]["verdict"], "satisfied",
            "the original must pass before mutant detection is evaluated"
        );
        assert_eq!(
            v["original"]["agency"]["refusal"],
            json!([]),
            "state guard excludes legacy refusal confounder"
        );
        let rows = v["mutations"].as_array().unwrap();
        response_boundary::assert_not_applicable(&h::source(name), &v["mutations"]);
        assert_eq!(rows.len(), 10);
        let mut count = 0;
        for row in rows {
            let kind = row["kind"].as_str().unwrap();
            if UNSUPPORTED.contains(&kind) {
                assert_eq!(row["status"], "unsupported");
                continue;
            }
            let ms = row["mutants"].as_array().unwrap();
            if ["hold_delayed", "refusal_path_cut"].contains(&kind) {
                assert_eq!(row["status"], "generated");
                assert_eq!(ms.len(), 1);
                count += 1;
                let m = &ms[0];
                assert_eq!(m["id"], format!("{kind}:Flow:body/1:keep_else"));
                assert_eq!(m["operator"], "bypass_guard");
                assert_eq!(m["result"], if bad { "undetected" } else { "detected" });
                let assigned = m["assigned_changes"].as_array().unwrap();
                let inv: Vec<_> = assigned
                    .iter()
                    .filter(|c| c["key"] == "contract Controls #1 invariant")
                    .collect();
                assert_eq!(
                    inv.len(),
                    usize::from(!bad),
                    "exact invariant identity detects bypass"
                );
                if !bad {
                    assert_eq!(inv[0]["before"], "satisfied");
                    assert_eq!(inv[0]["after"], "violated");
                }
                let once = h::property(
                    &m["report"],
                    if bad {
                        "contract Controls #1 at_most_once"
                    } else {
                        "contract Controls #2 at_most_once"
                    },
                );
                assert_eq!(once["verdict"], "satisfied");
            } else {
                assert_eq!(row["status"], "not_applicable");
                assert!(ms.is_empty());
            }
        }
        assert_eq!(count, 2);
    }
}
#[test]
fn adding_satisfied_clause_does_not_change_actor_agency_or_assumptions() {
    let s = h::source("sinv-initial");
    let control = h::change(
        &s,
        "contract Controls { invariant ok == true }",
        "verify safety no_resource_overcommit",
    );
    let extended = h::change(
        &control,
        "  verify safety",
        "  contract Controls { invariant ok }\n  verify safety",
    );
    let a = serde_json::to_value(relca_core::report::check(&control)).unwrap();
    let b = serde_json::to_value(relca_core::report::check(&extended)).unwrap();
    assert_eq!(a["verdict"], "satisfied");
    assert_eq!(b["verdict"], "satisfied");
    for key in [
        "actors",
        "agency",
        "assumptions",
        "terminals",
        "cut",
        "cut_context",
    ] {
        assert_eq!(
            a[key], b[key],
            "no invented state-invariant metadata: {key}"
        );
    }
}
