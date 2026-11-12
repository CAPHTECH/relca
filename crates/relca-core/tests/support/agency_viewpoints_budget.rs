//! Included by implementation's cfg(test) module, with a private injection adapter.
//! Expected output comes from A06, not from a full-limit production run.
#![allow(dead_code)]
use serde_json::{Value, json};
pub fn verify(mut run: impl FnMut(&str, &str, usize) -> Value) {
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/agy-soft-budget/expected.json"
    ))
    .unwrap();
    let hand = &expected["agency"]["viewpoints"];
    let full = run("framing", "work", 1_000_000);
    assert_eq!(full["verdict"], "satisfied");
    assert_eq!(full["diagnostics"], json!([]));
    assert_eq!(full["agency"]["viewpoints"], *hand);
    for block in ["observation", "framing", "retention"] {
        for (dimension, upper) in [
            ("depth", 128),
            ("paths", 4096),
            ("contexts", 65536),
            ("cells", 1_000_000),
            ("work", 1_000_000),
        ] {
            let zero = run(block, dimension, 0);
            if block == "observation" && ["paths", "contexts"].contains(&dimension) {
                assert_eq!(zero, full, "O must not invent accounting");
                continue;
            }
            let mut low = 0;
            let mut high = upper;
            while low < high {
                let mid = low + (high - low) / 2;
                if run(block, dimension, mid)["agency"]["viewpoints"][block]["status"] == "derived"
                {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            if low == 0 {
                assert_ne!(
                    dimension, "work",
                    "every block must charge actual model visits"
                );
                assert!(
                    block == "observation" || !["depth", "paths", "cells"].contains(&dimension),
                    "A06 has nested branches and nonempty F/L payloads: {block}/{dimension} must be charged"
                );
                assert!(
                    block != "framing" || dimension != "contexts",
                    "A06 has nonempty F contexts"
                );
                assert_eq!(
                    zero, full,
                    "unused {block}/{dimension} must not change output"
                );
                eprintln!("ADR0034 unused {block}/{dimension}; verify against private accounting");
                continue;
            }
            assert_eq!(
                run(block, dimension, low),
                full,
                "{block}/{dimension} k={low}"
            );
            for limit in [0, low - 1] {
                let failed = run(block, dimension, limit);
                let mut want = full.clone();
                let mut b = json!({"status":"not_derived","reason":"derivation_limit"});
                match block {
                    "observation" => b["fallback"] = json!("unchecked"),
                    "framing" => {
                        b["complete"] = json!(false);
                        b["unchecked"] = hand[block]["unchecked"].clone();
                    }
                    "retention" => {
                        b["notice"] = hand[block]["notice"].clone();
                        b["unchecked"] = hand[block]["unchecked"].clone();
                    }
                    _ => unreachable!(),
                }
                want["agency"]["viewpoints"][block] = b;
                assert_eq!(
                    failed, want,
                    "whole block only: {block}/{dimension} limit={limit} k={low}"
                );
            }
            eprintln!("ADR0034 measured {block}/{dimension} k={low}");
        }
    }
}
