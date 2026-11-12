//! Tenth population: each interpreter against hand arithmetic BEFORE equality.
//! Public Outcome fields are compared without private state/trace seams.
use relca_core::{
    engine, ir, lower,
    outcome::{CaseTerminalKind, Outcome},
    reference::{self, StopReason},
    report, syntax,
};
use serde_json::{Value, json};
use std::{fs, path::Path};

const FIXTURES: [&str; 24] = [
    "arrivals-contracts",
    "arrivals-contracts.broken-short-response",
    "arrivals-count",
    "arrivals-count.broken-case-count",
    "arrivals-initial-product",
    "arrivals-initial-product.broken-fixed-initial",
    "arrivals-order",
    "arrivals-order.broken-descending",
    "arrivals-reject-mass",
    "arrivals-reject-mass.broken-larger-queue",
    "arrivals-relative-within",
    "arrivals-relative-within.broken-tight-deadline",
    "arrivals-required",
    "arrivals-required.broken-missing-arrivals",
    "arrivals-same-time-slot",
    "arrivals-same-time-slot.broken-later-arrival",
    "arrivals-shared-fixed",
    "arrivals-shared-fixed.broken-unset-shared",
    "arrivals-simultaneous",
    "arrivals-simultaneous.broken-no-policy",
    "arrivals-staggered",
    "arrivals-staggered.broken-extra-capacity",
    "arrivals-zero-queue",
    "arrivals-zero-queue.broken-positive-queue",
];
fn range(r: relca_core::outcome::Range) -> Value {
    json!({"min":r.min,"max":r.max})
}
fn prob(p: &relca_core::probability::ProbabilityRange) -> Value {
    json!({"min":p.min.to_string(),"max":p.max.to_string()})
}
fn projection(model: &ir::Model, out: &Outcome, h: &Value) -> Value {
    assert!(
        out.terminals.is_empty(),
        "new route never aggregates EndId across cases"
    );
    let cases = out.case_outcomes.as_ref().expect("new route all case IDs");
    let case_rows: Vec<_> = cases.iter().map(|c| {
        let terminals: Vec<_> = c.terminals.iter().map(|t| {
            let (kind,end) = match &t.kind {
                CaseTerminalKind::ReceiptUnknown => panic!("arrival terminal oracle does not cover receipt observations"),
                CaseTerminalKind::End(id) => ("end",json!(model.ends[id.0])),
                CaseTerminalKind::Rejected => ("rejected",Value::Null),
                CaseTerminalKind::Overflow(id) => ("overflow",json!(model.ends[id.0])),
                CaseTerminalKind::CutOff => ("cut_off",Value::Null),
            };
            let mut v = json!({"kind":kind,"end":end,"completed_at":t.completed_at.map(range),"elapsed":t.elapsed.map(range),"work":t.work.map(range),"result_at":t.result_at.map(range)});
            if let Some(p) = &t.probability { v["probability"] = prob(p); }
            v
        }).collect();
        json!({"case_id":c.case_id.0+1,"arrival":c.arrival,"terminals":terminals})
    }).collect();
    assert_eq!(
        out.properties.len(),
        h["properties"].as_array().unwrap().len()
    );
    let properties: Vec<_> = out.properties.iter().zip(h["properties"].as_array().unwrap()).map(|(p,expected)| {
        let mut v = json!({"id":expected["id"],"verdict":p.verdict});
        if let Some(cs) = &p.cases {
            let rows: Vec<_> = cs.iter().map(|c| {
                let mut r = json!({"case_id":c.case_id.0+1,"arrival":c.arrival,"verdict":c.verdict});
                if let Some(p) = &c.probability { r["probability"] = prob(p); }
                r
            }).collect();
            v["cases"] = json!(rows);
        }
        assert!(p.probability.is_none(),"no cohort probability");
        v
    }).collect();
    let admission = out.admission.as_ref().expect("new admission summary");
    json!({"case_outcomes":case_rows,"admission":{"rejected_count":range(admission.rejected_count)},"properties":properties,"terminals":[],"truncated":out.truncated})
}
/// ADR 0035 U12(意味の注記の条件と性質の行の形): atomic の check-and-insert を
/// 持たない旧いモデルでは、at_most_once の行にだけ notes ["mechanism_unmodeled"] が
/// 足される。他の行・verdict・cases は旧い手計算の期待のまま。at_most_once 以外の
/// 行に notes が付けば、この期待と一致しないので落ちる。
fn with_at_most_once_notes(source: &str, properties: &Value) -> Value {
    assert!(!source.contains("atomic"), "pre-A3 oracle only");
    let mut rows = properties.clone();
    for row in rows.as_array_mut().unwrap() {
        assert!(row.get("notes").is_none(), "hand oracle carries no notes");
        if row["id"].as_str().unwrap().ends_with(" at_most_once") {
            row["notes"] = json!(["mechanism_unmodeled"]);
        }
    }
    rows
}
fn hand(label: &str, model: &ir::Model, out: &Outcome, h: &Value) -> Value {
    assert_eq!(out.properties.len(), model.verify.len());
    for (actual, verify) in out.properties.iter().zip(&model.verify) {
        assert_eq!(actual.property, verify.property, "{label}: verify identity");
    }
    let v = projection(model, out, h);
    for k in ["case_outcomes", "admission", "properties", "terminals"] {
        assert_eq!(v[k], h[k], "{label}: independent {k} oracle");
    }
    assert_eq!(v["truncated"], h["bounds"]["truncated"]);
    v
}

#[test]
fn arrivals_population_24_dirs_24_inputs_20_runtime_4_static_complete20_excluded0() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut names: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| {
            e.file_type().unwrap().is_dir()
                && e.file_name().to_string_lossy().starts_with("arrivals-")
        })
        .map(|e| e.file_name().to_str().unwrap().to_owned())
        .collect();
    names.sort();
    assert_eq!(names, FIXTURES);
    let (mut runtime, mut rejected, mut complete) = (0, 0, 0);
    for name in names {
        let dir = root.join(&name);
        for file in ["model.relca", "calc.md", "expected.json"] {
            assert!(dir.join(file).is_file());
        }
        assert!(!dir.join("invocation.json").exists());
        let source = fs::read_to_string(dir.join("model.relca")).unwrap();
        let h: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
        if ["invalid", "unsupported"].contains(&h["verdict"].as_str().unwrap()) {
            rejected += 1;
            let ds = syntax::parse(&source)
                .and_then(|a| lower::lower(&a))
                .expect_err("static rejection required");
            let code = h["diagnostics"][0]["code"].as_str().unwrap();
            assert!(
                ds.iter()
                    .any(|d| d.code == code && d.severity == relca_core::diag::Severity::Error)
            );
            continue;
        }
        runtime += 1;
        let model = syntax::parse(&source)
            .and_then(|a| lower::lower(&a))
            .unwrap_or_else(|d| panic!("{name}: {d:?}"));
        let e = engine::explore(&model);
        let r = reference::explore(&model);
        assert_eq!(
            r.stop,
            StopReason::Complete,
            "{name}: no exclusions or enumeration limits"
        );
        complete += 1;
        let ep = hand(&format!("{name}/engine"), &model, &e, &h);
        let rp = hand(&format!("{name}/reference"), &model, &r.outcome, &h);
        assert_eq!(ep, rp, "{name}");
        assert_eq!(e.properties, r.outcome.properties);
        assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
        assert_eq!(e.admission, r.outcome.admission);
        assert!(r.outcome.diagnostics.starts_with(&model.diagnostics));
        let diagnostics = |ds: &[relca_core::diag::Diagnostic]| -> Vec<Value> {
            ds.iter()
                .map(|d| json!({"code":d.code,"severity":d.severity,"span":d.span}))
                .collect()
        };
        assert_eq!(
            diagnostics(&e.diagnostics),
            diagnostics(&r.outcome.diagnostics[model.diagnostics.len()..]),
            "{name}: runtime diagnostic inventory"
        );
        assert_eq!(serde_json::to_value(r.verdict()).unwrap(), h["verdict"]);
        let v = serde_json::to_value(report::check(&source)).unwrap();
        assert_eq!(v["verdict"], h["verdict"]);
        for k in ["case_outcomes", "admission", "terminals"] {
            assert_eq!(v[k], ep[k]);
        }
        // The Outcome carries no notes; only the public report adds them.
        assert_eq!(
            v["properties"],
            with_at_most_once_notes(&source, &ep["properties"]),
            "{name}"
        );
    }
    assert_eq!(
        (FIXTURES.len(), runtime, rejected, complete),
        (24, 20, 4, 20)
    );
}
