use super::*;
use serde_json::{Value, json};

fn change(l: &mut Limits, dimension: &str, n: usize) {
    match dimension {
        "depth" => l.depth = n,
        "paths" => l.paths = n,
        "contexts" => l.contexts = n,
        "cells" => l.cells = n,
        "work" => l.work = n,
        _ => panic!("unknown dimension"),
    }
}
fn injected(s: &str, block: &str, dimension: &str, n: usize, config: bool) -> Value {
    let ast = crate::syntax::parse(s).unwrap();
    let mut m = if config {
        crate::lower::lower_configuration(&ast, "To").unwrap().model
    } else {
        crate::lower::lower(&ast).unwrap()
    };
    let mut r = if config {
        crate::report::check_configuration(s, "To")
    } else {
        crate::report::check(s)
    };
    let mut limits = [DEFAULT; 3];
    let mut effect_limit = DEFAULT;
    if block == "effects" {
        change(&mut effect_limit, dimension, n);
    } else {
        change(&mut limits[2], dimension, n);
    }
    // Inject through the real producer's finalization, including warning
    // withdrawal and executors retention. The adapter must not recreate those
    // rules itself: doing so would mask a defect in production finalization.
    let original_lower = m.diagnostics.clone();
    assert_eq!(
        &r.diagnostics[..original_lower.len()],
        original_lower.as_slice()
    );
    let (agency, warnings) = derive_with_new_limits(&m, DEFAULT, limits, effect_limit).unwrap();
    m.agency = agency.into();
    m.diagnostics.retain(|d| {
        ![
            diag::W_AGY_0,
            diag::W_AGY_1,
            diag::W_AGY_2,
            diag::W_AGY_3,
            diag::W_LIN_2,
            diag::W_RESP_0,
            diag::W_RESP_1,
            diag::W_RESP_2,
            diag::W_CONTEST_0,
        ]
        .contains(&d.code.as_str())
    });
    m.diagnostics.extend(warnings);
    crate::responsibility::derive(&mut m).unwrap();
    r.diagnostics.splice(..original_lower.len(), m.diagnostics);
    r.agency = Some(m.agency);
    serde_json::to_value(r).unwrap()
}
#[test]
fn effects_each_private_k_boundary_is_atomic_and_independent() {
    second_half_budget::effects(|s, block, dimension, n| injected(s, block, dimension, n, false));
}
#[test]
fn differences_each_private_k_boundary_preserves_sides() {
    second_half_budget::differences(|s, dimension, n| {
        let mut r = crate::diff::diff(s, "From", "To");
        let mut limits = DEFAULT;
        change(&mut limits, dimension, n);
        r.finish_with_agency_comparison(|from, to| differences::with_limits(from, to, limits));
        serde_json::to_value(r).unwrap()
    });
}
#[test]
fn underived_retention_is_not_zero_exercise() {
    second_half_budget::fallback_retention_failure(|s, n| {
        injected(s, "retention", "work", n, true)
    });
}
fn model() -> ir::Model {
    crate::lower::lower(
        &crate::syntax::parse(include_str!(
            "../../../../fixtures/agy2-reads-observed/model.relca"
        ))
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn malformed_read_indices_latents_cycles_and_concurrent_readers_fail_closed() {
    let m = model();
    let reader = m
        .capabilities
        .iter()
        .position(|c| !c.read_variables.is_empty())
        .unwrap();
    let mut bad = m.clone();
    bad.capabilities[reader].read_variables = vec![ir::StateVarId(usize::MAX)];
    assert_eq!(derive(&bad).unwrap_err().code, diag::U_SEM_0);
    let latent = m
        .state_variables
        .iter()
        .position(|v| v.kind.is_latent())
        .unwrap();
    let mut bad = m.clone();
    bad.capabilities[reader].read_variables = vec![ir::StateVarId(latent)];
    assert_eq!(derive(&bad).unwrap_err().code, diag::U_SEM_0);
    let mut bad = m.clone();
    let ir::Stmt::Call(c) = &mut bad.process.body[1].kind else {
        panic!("call");
    };
    let id = c.capability;
    c.args[0] = c.binding;
    bad.capabilities[id.0].params[0].ty = bad.capabilities[id.0].return_type;
    assert_eq!(derive(&bad).unwrap_err().code, diag::U_SEM_0);
    for name in [
        "agy2-static-fork-reader-first.broken-concurrent",
        "agy2-static-fork-writer-first.broken-concurrent",
    ] {
        let s = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(name)
                .join("model.relca"),
        )
        .unwrap();
        let ast = crate::syntax::parse(&s).unwrap();
        assert!(crate::lower::lower(&ast).unwrap_err().iter().any(
            |d| d.code == diag::U_SEM_0 && d.message == "read variable has a concurrent writer"
        ));
    }
}
#[test]
fn atomic_fork_insertion_cannot_certify_normal_path_false() {
    let s = include_str!("../../../../fixtures/agy2-atomic-u.broken-second-post/model.relca");
    let mut m = crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap();
    let mut calls = Vec::new();
    m.process.body.retain(|s| {
        if let ir::Stmt::Call(c) = &s.kind
            && m.capabilities[c.capability.0].atomic.is_some()
        {
            calls.push(c.clone());
            false
        } else {
            true
        }
    });
    assert_eq!(calls.len(), 2);
    m.process.forks.push("public".into());
    let stmt = ir::Statement {
        kind: ir::Stmt::Fork {
            id: ir::ForkId(0),
            branches: calls,
            join: ir::Join::All,
        },
        span: m.process.span,
    };
    m.process.body.insert(m.process.body.len() - 1, stmt);
    match derive(&m) {
        Err(d) => assert_eq!(d.code, diag::U_SEM_0),
        Ok((a, _)) => {
            let value = serde_json::to_value(a).unwrap();
            for row in value["viewpoints"]["effects"]["rows"].as_array().unwrap() {
                assert_eq!(row["state_refusers"][0]["on_normal_path"], "unknown");
                assert!(row.get("j").is_none() && row.get("u").is_none());
            }
        }
    }
}

#[test]
fn mechanism_key_proof_does_not_require_unrelated_arguments_to_be_case_ids() {
    // Hand proof: key is case.id and keys starts empty. `other:T` does not
    // participate in membership or insertion. The first call cannot reject;
    // the second has a preceding insertion and must remain unknown statically.
    let s = include_str!("../../../../fixtures/aset-static-enum-insert/model.relca");
    let m = crate::lower::lower(&crate::syntax::parse(s).unwrap()).unwrap();
    let value = serde_json::to_value(&m.agency).unwrap();
    let rows = value["viewpoints"]["effects"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["state_refusers"][0]["on_normal_path"], false);
    assert_eq!(rows[0]["process_judgements"], "derived");
    assert_eq!(rows[1]["state_refusers"][0]["on_normal_path"], "unknown");
    assert!(rows[1].get("j").is_none() && rows[1].get("u").is_none());
    assert_eq!(
        m.diagnostics
            .iter()
            .filter(|d| d.code == diag::W_AGY_2)
            .count(),
        1
    );
}
#[test]
fn unavailable_mismatch_equal_unknown_and_one_sided_effect_are_distinct() {
    let s = include_str!("../../../../fixtures/agy2-diff-items.broken-state-guard/model.relca");
    let diff = crate::diff::diff(s, "From", "To");
    for (left, right) in [
        (None, None),
        (diff.agency.from.as_ref(), None),
        (None, diff.agency.to.as_ref()),
    ] {
        let d = differences(
            ComparisonSide {
                agency: left.and_then(AgencyDerivation::as_derived),
                responsibilities: None,
            },
            ComparisonSide {
                agency: right.and_then(AgencyDerivation::as_derived),
                responsibilities: None,
            },
        );
        let v = serde_json::to_value(d).unwrap();
        assert_eq!(v["reason"], "agency_unavailable");
        assert!(v.get("rows").is_none());
    }
    for (a, z) in [
        (None, None),
        (diff.agency.from.clone(), None),
        (None, diff.agency.to.clone()),
    ] {
        let mut x = diff.from.clone();
        x.report.agency = a;
        let mut y = diff.to.clone();
        y.report.agency = z;
        let r = crate::diff::DiffReport::uncomputed(x, y);
        assert_eq!(
            r.unchecked
                .iter()
                .filter(|w| *w == "agency_differences")
                .count(),
            1
        );
    }
    let mismatch = crate::diff::DiffReport::uncomputed(diff.from.clone(), diff.to.clone());
    assert_eq!(
        serde_json::to_value(mismatch).unwrap()["agency"]["differences"]["reason"],
        "comparison_mismatch"
    );
    let unknown = differences(
        ComparisonSide {
            agency: diff
                .agency
                .to
                .as_ref()
                .and_then(AgencyDerivation::as_derived),
            responsibilities: None,
        },
        ComparisonSide {
            agency: diff
                .agency
                .to
                .as_ref()
                .and_then(AgencyDerivation::as_derived),
            responsibilities: None,
        },
    );
    let v = serde_json::to_value(unknown).unwrap();
    assert!(
        v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["viewpoint"] == "framing" && r["status"] == "not_comparable")
    );
    let s = include_str!("../../../../fixtures/agy2-jue/model.relca");
    let a = crate::report::check(s);
    let z = crate::report::check(&s.replace("commits;", ""));
    let v = serde_json::to_value(differences(
        ComparisonSide {
            agency: a.agency.as_ref().and_then(AgencyDerivation::as_derived),
            responsibilities: None,
        },
        ComparisonSide {
            agency: z.agency.as_ref().and_then(AgencyDerivation::as_derived),
            responsibilities: None,
        },
    ))
    .unwrap();
    assert!(
        v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["viewpoint"] == "effects" && r["item"] == "e" && r["to"].is_null())
    );
}
#[test]
fn derived_wire_roundtrip_and_semantic_forgery_refusals() {
    let r = crate::diff::diff(
        include_str!("../../../../fixtures/agy2-diff-items/model.relca"),
        "From",
        "To",
    );
    let v = serde_json::to_value(&r.agency.differences).unwrap();
    assert!(serde_json::from_value::<Differences>(v.clone()).is_ok());
    let mut root_extra = v.clone();
    root_extra["score"] = json!(1);
    assert!(serde_json::from_value::<Differences>(root_extra).is_err());
    for mutate in 0..5 {
        let mut bad = v.clone();
        match mutate {
            0 => {
                let x = bad["rows"][0].clone();
                bad["rows"].as_array_mut().unwrap().push(x);
            }
            1 => bad["rows"][0]["item"] = json!("score"),
            2 => bad["rows"][0]["score"] = json!(1),
            3 => bad["rows"][0]["call"] = json!("forged"),
            _ => bad["rows"][0]["from"] = json!({"score":1}),
        }
        assert!(serde_json::from_value::<Differences>(bad).is_err());
    }
    let mut a = serde_json::to_value(model().agency).unwrap();
    a["viewpoints"].as_object_mut().unwrap().remove("effects");
    assert!(
        serde_json::from_value::<Agency>(a).is_err(),
        "new rules require effects"
    );
    let r = crate::diff::diff(
        include_str!("../../../../fixtures/agy2-diff-items.broken-state-guard/model.relca"),
        "From",
        "To",
    );
    let mut bad = serde_json::to_value(r.agency.differences).unwrap();
    let row = bad["rows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["viewpoint"] == "framing" && r["status"] == "not_comparable")
        .unwrap();
    row["from"]["reason"] = json!("state_guards");
    assert!(
        serde_json::from_value::<Differences>(bad).is_err(),
        "a derived normal side requires null reason"
    );
}

#[test]
fn public_ast_keep_shape_and_incomplete_resource_framing_remain_fail_closed() {
    let s = include_str!("../../../../fixtures/agy2-keep-fallback/model.relca");
    let mut ast = crate::syntax::parse(s).unwrap();
    for item in &mut ast.items {
        if let crate::ast::Item::Configuration(c) = item {
            for intent in &mut c.intent {
                if let crate::ast::Intent::Claim {
                    keep_fallback: true,
                    target,
                    ..
                } = intent
                {
                    target.path.value.truncate(1);
                    target.kind = crate::ast::DiffKind::Actor;
                }
            }
        }
    }
    assert!(
        crate::lower::lower_configuration(&ast, "To")
            .err()
            .unwrap()
            .iter()
            .any(|d| d.code == diag::U_SYN_0)
    );
    let s = include_str!("../../../../fixtures/agy2-rsrc-callorder/model.relca");
    let r = crate::report::check(s);
    let d = differences(
        ComparisonSide {
            agency: r.agency.as_ref().and_then(AgencyDerivation::as_derived),
            responsibilities: None,
        },
        ComparisonSide {
            agency: r.agency.as_ref().and_then(AgencyDerivation::as_derived),
            responsibilities: None,
        },
    );
    let value = serde_json::to_value(d).unwrap();
    let row = value["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["viewpoint"] == "framing" && r["item"] == "actors")
        .unwrap();
    assert_eq!(row["from"]["reason"], "resource_infeasible");
    assert_eq!(row["to"]["reason"], "resource_infeasible");
}

#[test]
fn empty_reads_preserve_legacy_identifier_error_before_lowering() {
    let s = include_str!("../../../../fixtures/agency-origin/model.relca").replacen(
        "reads [Invoice]",
        "reads []",
        1,
    );
    let ds = crate::syntax::parse(&s).unwrap_err();
    assert_eq!(ds[0].code, diag::E_SYN_0);
    assert_eq!(ds[0].message, "expected identifier");
    let start = s.find("reads []").unwrap() + 7;
    assert_eq!(
        ds[0].span,
        Some(diag::Span {
            start,
            end: start + 1
        })
    );
    assert_eq!(crate::report::check(&s).verdict, crate::Verdict::Invalid);
}

#[test]
fn unavailable_agency_marker_follows_existing_metric_words() {
    let s = include_str!("../../../../fixtures/met-diff/model.relca");
    // A DSL selection failure exits before constructing root metric tables.
    let failed = crate::diff::diff(s, "AsIs", "Missing");
    assert!(failed.metrics.is_none());
    assert!(failed.agency.to.is_none());
    assert_eq!(failed.unchecked.last().unwrap(), "agency_differences");
    // Exercise the public report/optional-Agency seam with an already computed
    // metric table; do not pretend the current DSL produces this combination.
    let mut d = crate::diff::diff(s, "AsIs", "ToBeP");
    assert!(!d.metrics.as_ref().unwrap().is_empty());
    d.agency.to = None;
    d.to.report.agency = None;
    d.finish_with_agency_comparison(differences);
    assert_eq!(d.unchecked.last().unwrap(), "agency_differences");
    assert!(
        d.unchecked
            .iter()
            .any(|word| word == "metric_semantic_alignment")
    );
}
