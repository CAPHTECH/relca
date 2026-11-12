//! ADR 0029 hand expectations, fixed before implementation. No numeric DTO dependency.
#[path = "support/integer_acceptance.rs"]
mod hand;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};
fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
fn model(s: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(s).unwrap_or_else(|e| panic!("parse {e:?}\n{s}")))
        .unwrap_or_else(|e| panic!("lower {e:?}\n{s}"))
}
macro_rules! fixtures { ($($test:ident => $name:literal),+ $(,)?) => { $(#[test] fn $test() { hand::subset(&hand::expected($name), &check(&hand::source($name))); })+ }; }
fixtures! {
 f01_add => "int-two-add", f01_assign => "int-two-add.broken-assignment",
 f02_upper => "int-upper", f02_overflow => "int-upper.broken-overflow",
 f03_subtract => "int-underflow", f03_underflow => "int-underflow.broken-underflow",
 f04_default => "int-default", f04_invalid_default => "int-default.broken-default",
 f05_type => "int-type", f05_mismatch => "int-type.broken-enum",
 f06_observed => "int-latent-policy", f06_latent => "int-latent-policy.broken-latent",
 f07_lt => "int-lt", f07_boundary => "int-lt.broken-boundary",
 f08_le => "int-le", f08_boundary => "int-le.broken-boundary",
 f09_gt => "int-gt", f09_boundary => "int-gt.broken-boundary",
 f10_ge => "int-ge", f10_boundary => "int-ge.broken-boundary",
 f11_ne => "int-ne", f11_boundary => "int-ne.broken-boundary",
 f12_fork => "int-fork", f12_conflict => "int-fork.broken-same-target",
 f13_start_finish => "int-start-finish", f13_zero => "int-start-finish.broken-zero-start",
 f14_set => "int-set-stop", f14_outward => "int-set-stop.broken-outward",
 f15_distribution => "int-dist", f15_threshold => "int-dist.broken-threshold",
 f16_nondet => "int-nondet", f16_threshold => "int-nondet.broken-threshold",
 f17_named => "int-class-int", f17_unbounded => "int-class-int.broken-unbounded",
 f18_named => "int-class-param", f18_caseid => "int-class-param.broken-caseid",
 f19_binding => "int-class-caseid", f19_caseid => "int-class-caseid.broken-reference",
 f20_source => "int-class-reads", f20_observed => "int-class-reads.broken-observed",
 f21_named => "int-class-stateid", f21_caseid => "int-class-stateid.broken-caseid",
 f22_enum => "int-kernel-given", f22_integer => "int-kernel-given.broken-integer",
 f23_requires => "int-requires", f23_refusal => "int-requires.broken-refused",
 f24_kernel => "int-kernel-finish", f24_overflow => "int-kernel-finish.broken-overflow",
 f25_atomic => "int-atomic", f25_overflow => "int-atomic.broken-overflow",
 f26_cancel => "int-cancel", f26_drain => "int-cancel.broken-drain",
 f27_range => "int-range-vs-clock", f27_clock => "int-range-vs-clock.broken-clock",
 f28_arrivals => "int-arrivals", f28_overflow => "int-arrivals.broken-overflow"
}
#[test]
fn population56_and_28_single_semantic_edits_are_closed() {
    let mut actual: Vec<_> = std::fs::read_dir(hand::root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("int-"))
        .collect();
    actual.sort();
    let mut names = hand::names();
    names.sort();
    assert_eq!(actual, names);
    for (base, fault, from, to) in hand::PAIRS {
        let base = format!("int-{base}");
        let s = hand::source(&base);
        assert_eq!(s.matches(from).count(), 1, "{base}");
        assert_eq!(
            s.replacen(from, to, 1),
            hand::source(&format!("{base}.broken-{fault}"))
        );
        for name in [base.clone(), format!("{base}.broken-{fault}")] {
            let input = hand::source(&name);
            assert!(
                !input.contains("//") && !input.contains('#'),
                "DSL has no comments: {name}"
            );
            assert!(
                hand::root()
                    .join("fixtures")
                    .join(name)
                    .join("calc.md")
                    .is_file()
            );
        }
    }
}
#[test]
fn legacy_skeletons_preflight_names_resources_joins_and_classification_neighbors() {
    for name in hand::names() {
        let s = hand::old_skeleton(&name);
        model(&s);
    }
}
#[test]
fn runtime46_reference_complete46_static10_excluded_by_explicit_diagnostic_only() {
    let (mut runtime, mut rejected) = (0, 0);
    for name in hand::names() {
        let e = hand::expected(&name);
        let s = hand::source(&name);
        if name == "int-class-reads.broken-observed" {
            // ADR0043 moved the old unsupported shape to accepted syntax. Keep
            // this historical 46/10 budget population on a qualified read; C5
            // and f20_observed own the newly accepted original input.
            let qualified = s.replace("reads [flag]", "reads [A.flag]");
            assert_ne!(s, qualified);
            hand::rejection(&qualified, "U-SYN-0");
            rejected += 1;
            continue;
        }
        if e["verdict"] == "invalid" || e["verdict"] == "unsupported" {
            rejected += 1;
            hand::rejection(&s, e["diagnostics"][0]["code"].as_str().unwrap());
            continue;
        }
        let m = model(&s);
        let a = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "{name}: no reference exclusions"
        );
        for (label, o) in [("engine", &a), ("reference", &r.outcome)] {
            assert_eq!(
                o.truncated,
                e["bounds"]["truncated"].as_bool().unwrap_or(false),
                "{name}/{label}"
            );
            assert_eq!(
                o.properties.len(),
                e["properties"].as_array().unwrap().len(),
                "{name}/{label}: implicit S inventory"
            );
            for (actual, expected) in o.properties.iter().zip(e["properties"].as_array().unwrap()) {
                assert_eq!(
                    serde_json::to_value(actual.verdict).unwrap(),
                    expected["verdict"],
                    "{name}/{label}: hand oracle {}",
                    expected["id"]
                );
                if let Some(p) = expected.get("probability") {
                    assert_eq!(
                        serde_json::to_value(&actual.probability).unwrap(),
                        *p,
                        "{name}/{label}: exact rational"
                    );
                }
            }
            if let Some(rows) = e["terminals"].as_array() {
                assert_eq!(
                    o.terminals.len(),
                    rows.len(),
                    "{name}/{label}: no invented end"
                );
                for t in &o.terminals {
                    let want = rows.iter().find(|e| e["end"] == m.ends[t.end.0]).unwrap();
                    assert_eq!(
                        json!({"min":t.elapsed.min,"max":t.elapsed.max}),
                        want["elapsed"]
                    );
                    assert_eq!(json!({"min":t.work.min,"max":t.work.max}), want["work"]);
                }
            }
        }
        // Only reference prepends lowering diagnostics; engine never does.
        assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
        let rt = &r.outcome.diagnostics[m.diagnostics.len()..];
        assert_eq!(
            a.diagnostics
                .iter()
                .map(|d| (&d.code, &d.severity, &d.span))
                .collect::<Vec<_>>(),
            rt.iter()
                .map(|d| (&d.code, &d.severity, &d.span))
                .collect::<Vec<_>>(),
            "{name}: runtime diagnostics"
        );
        assert_eq!(
            a.properties, r.outcome.properties,
            "{name}: after both hand checks"
        );
        runtime += 1;
    }
    assert_eq!((runtime, rejected), (46, 10));
}
