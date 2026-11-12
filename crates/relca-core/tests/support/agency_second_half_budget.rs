//! Private injection adapter, following agency_viewpoints_budget.rs.
//! Link from cfg(test) implementation after the new limits exist. The adapter
//! learns only accounting k, never the semantic expected payload.
#![allow(dead_code)]
use serde_json::{Value, json};

fn source(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
fn expected(name: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(name)
                .join("expected.json"),
        )
        .unwrap(),
    )
    .unwrap()
}
fn threshold(mut run: impl FnMut(usize) -> Value, pointer: &str, upper: usize) -> usize {
    let mut low = 0;
    let mut high = upper;
    assert_eq!(run(high).pointer(pointer).unwrap()["status"], "derived");
    while low < high {
        let mid = low + (high - low) / 2;
        if run(mid).pointer(pointer).unwrap()["status"] == "derived" {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    low
}

/// `run(source, block, dimension, limit)` must change exactly one private limit.
/// The implementation records the measured k for each actually charged dimension.
pub fn effects(mut run: impl FnMut(&str, &str, &str, usize) -> Value) {
    let s = source("agy2-concentration");
    let model = crate::lower::lower(&crate::syntax::parse(&s).unwrap()).unwrap();
    // ADR 0054: this hand model's sole effect and legacy U are both Actor A;
    // people p1/p2/p3 have no duty, so A's complete warning remains known.
    let actor = model.actors.iter().find(|a| a.name == "A").unwrap();
    let responsibility = json!([{"code":"W-RESP-0","severity":"warning",
        "message":"A has no declared responsibility","span":actor.span}]);
    let notice = format!(
        "responsibility_default_candidates: process {}: U/E not_derived (derivation_limit)",
        model.process.name
    );
    let want = expected("agy2-concentration")["agency"]["viewpoints"]["effects"].clone();
    let full = run(&s, "effects", "work", 1_000_000);
    assert_eq!(full["verdict"], "satisfied");
    assert_eq!(full["agency"]["viewpoints"]["effects"], want);
    assert_eq!(
        json!(
            full["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["code"] == "W-RESP-0")
                .collect::<Vec<_>>()
        ),
        responsibility
    );
    assert!(
        !full["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!(notice))
    );
    assert_eq!(
        full["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-AGY-2")
            .count(),
        1
    );
    for (dimension, upper) in [
        ("depth", 128),
        ("paths", 4096),
        ("contexts", 65536),
        ("cells", 1_000_000),
        ("work", 1_000_000),
    ] {
        let k = threshold(
            |n| run(&s, "effects", dimension, n),
            "/agency/viewpoints/effects",
            upper,
        );
        if k == 0 {
            assert_ne!(dimension, "work", "effects must charge real work");
            assert_eq!(
                run(&s, "effects", dimension, 0),
                full,
                "unused dimension must not introduce accounting"
            );
            eprintln!("ADR0043 effects/{dimension} unused; independently inspect accounting");
            continue;
        }
        assert_eq!(run(&s, "effects", dimension, k), full, "k boundary");
        for n in [0, k - 1] {
            let failed = run(&s, "effects", dimension, n);
            let mut expected = full.clone();
            expected["agency"]["viewpoints"]["effects"] = json!({"status":"not_derived","reason":"derivation_limit","unchecked":["authority","fallback_path"]});
            expected["diagnostics"]
                .as_array_mut()
                .unwrap()
                .retain(|d| d["code"] != "W-AGY-2");
            let tags = expected["agency"]["unchecked"].as_array_mut().unwrap();
            assert!(!tags.contains(&json!("executors")));
            let at = tags.iter().position(|x| x == "fallback_path").unwrap() + 1;
            tags.insert(at, json!("executors"));
            // The new reasoned disclosure is derived from the IR process and
            // the injected NotDerived reason, not copied from the failed run.
            let at = tags
                .iter()
                .position(|word| word == "obligation_debtor")
                .unwrap_or(tags.len());
            tags.insert(at, json!(notice));
            assert_eq!(
                json!(
                    failed["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|d| d["code"] == "W-RESP-0")
                        .collect::<Vec<_>>()
                ),
                responsibility
            );
            assert_eq!(
                failed, expected,
                "effects/{dimension} k={k}, n={n}: atomic block failure, independent F/L and legacy budget"
            );
            let mut missing = failed.clone();
            missing["agency"]["unchecked"]
                .as_array_mut()
                .unwrap()
                .retain(|word| word != &json!(notice));
            assert_ne!(
                missing, expected,
                "missing disclosure rejected by the exact same Report equality"
            );
            let mut spurious = expected.clone();
            spurious["agency"]["unchecked"]
                .as_array_mut()
                .unwrap()
                .push(json!(notice));
            assert_ne!(spurious, expected, "duplicate disclosure rejected");
        }
        eprintln!("ADR0043 effects/{dimension} measured k={k}");
    }
}

/// The callback selects From/To and injects only the differences budget.
pub fn differences(mut run: impl FnMut(&str, &str, usize) -> Value) {
    let s = source("agy2-diff-items");
    let want = expected("agy2-diff-items")["agency"]["differences"].clone();
    let full = run(&s, "work", 1_000_000);
    assert_eq!(full["verdict"], "satisfied");
    assert_eq!(
        full["agency"]["differences"], want,
        "ten hand-derived meaning rows"
    );
    for (dimension, upper) in [
        ("depth", 128),
        ("paths", 4096),
        ("contexts", 65536),
        ("cells", 1_000_000),
        ("work", 1_000_000),
    ] {
        let k = threshold(|n| run(&s, dimension, n), "/agency/differences", upper);
        if k == 0 {
            assert_ne!(dimension, "work");
            assert_eq!(run(&s, dimension, 0), full);
            eprintln!("ADR0043 differences/{dimension} unused; inspect accounting");
            continue;
        }
        assert_eq!(run(&s, dimension, k), full);
        for n in [0, k - 1] {
            let failed = run(&s, dimension, n);
            let mut expected = full.clone();
            expected["agency"]["differences"] = json!({"rules":"agency-diff-p2-1","status":"not_derived","reason":"derivation_limit","unchecked":["fallback_path","workload_transfer","via_model_paths"]});
            assert_eq!(
                failed, expected,
                "differences/{dimension} k={k} n={n}: side reports, root verdict, old census and effects unchanged"
            );
        }
        eprintln!("ADR0043 differences/{dimension} measured k={k}");
    }
}

/// M24: not-derived L is not zero exercise; do not issue W-AGY-3.
pub fn fallback_retention_failure(mut run: impl FnMut(&str, usize) -> Value) {
    let s = source("agy2-keep-fallback");
    let full = run(&s, 1_000_000);
    assert_eq!(full["verdict"], "satisfied");
    assert_eq!(
        full["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-AGY-3")
            .count(),
        1
    );
    let failed = run(&s, 0);
    assert_eq!(
        failed["agency"]["viewpoints"]["retention"]["status"],
        "not_derived"
    );
    assert!(
        failed["agency"]["viewpoints"]["retention"]
            .get("capabilities")
            .is_none()
    );
    assert!(
        !failed["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "W-AGY-3")
    );
    assert_eq!(failed["verdict"], full["verdict"]);
    assert_eq!(
        failed["agency"]["viewpoints"]["effects"],
        full["agency"]["viewpoints"]["effects"]
    );
}
