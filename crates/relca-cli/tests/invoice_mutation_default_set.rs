//! ADR0046 T14: hand arithmetic and exact new rows, independently of snapshots.
#[path = "support/deferred_invoice.rs"]
mod deferred_invoice;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/response_boundary_oracle.rs"]
mod response_boundary;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn range(report: &Value, end: &str, elapsed: (u64, u64), work: (u64, u64)) {
    let t = report["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["end"] == end)
        .unwrap();
    assert_eq!(t["elapsed"], json!({"min":elapsed.0,"max":elapsed.1}));
    assert_eq!(t["work"], json!({"min":work.0,"max":work.1}));
}
fn hand(name: &str, v: &Value, deferred: bool) {
    fixture_match::validate_mutation_wire(v).unwrap();
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(v["run"]["completion"], "inconclusive");
    assert_eq!(v["run"]["reason"], "mutant_incomplete");
    let expiry = &v["mutations"][1];
    assert_eq!(expiry["status"], "generated");
    let ms = expiry["mutants"].as_array().unwrap();
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0]["id"], "authority_expired:ApprovalGrant:expires");
    assert_eq!(ms[0]["result"], "invalid_mutant");
    assert_eq!(ms[0]["reason"], "static_unsupported");
    assert_eq!(ms[0]["site"]["affected_bindings"], json!(["approvalV"]));
    assert_eq!(ms[0]["report"]["verdict"], "unsupported");
    assert!(
        ms[0]["report"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0"
                && d["message"]
                    .as_str()
                    .unwrap()
                    .contains("restricted Authority ApprovalGrant"))
    );
    let (process, original_post, post, work, returned, return_work) = match name {
        "as-is" => ("AsIsFlow", 17, 23, 23, (20, 22), (20, 22)),
        "to-be-p" => ("ToBePFlow", 14, 20, 23, (17, 19), (20, 22)),
        "to-be-a" => ("ToBeAFlow", 9, 10, 13, (7, 9), (10, 12)),
        _ => panic!("closed invoice models"),
    };
    range(
        &v["original"],
        "posted",
        (original_post, original_post),
        if name == "to-be-a" {
            (12, 12)
        } else {
            (17, 17)
        },
    );
    let summary = &v["mutations"][4];
    assert_eq!(summary["status"], "generated");
    let ms = summary["mutants"].as_array().unwrap();
    assert_eq!(ms.len(), if name == "as-is" { 4 } else { 2 });
    for (i, (consumer, cap, subjects, independent, count)) in [
        (
            "receiptV",
            "Clerk.match_receipt",
            json!(["Invoice"]),
            json!(["ReceiptRecord"]),
            2,
        ),
        (
            "approvalV",
            "Approver.approve",
            json!(["Invoice", "PurchaseOrder", "ReceiptRecord"]),
            json!([]),
            1,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let m = &ms[i];
        assert_eq!(
            m["id"],
            format!("summary_reused:{process}:body/0:entry->{consumer}")
        );
        assert_eq!(m["operator"], "duplicate_summary");
        assert_eq!(m["result"], "detected");
        assert!(m["reason"].is_null());
        assert_eq!(m["site"]["duplicate_binding"], "entry_dup");
        let key = format!("{consumer}:{cap}");
        assert_eq!(
            m["assigned_changes"],
            json!([
                {"check":"agency.lineage","key":key,"field":"subject_sources","before":subjects,"after":subjects,"direction":"held"},
                {"check":"agency.lineage","key":key,"field":"independent_sources","before":independent,"after":independent,"direction":"held"},
                {"check":"agency.lineage_groups","key":serde_json::to_string(&subjects).unwrap(),"field":"judgements","before":count,"after":count,"direction":"held"}
            ])
        );
        range(&m["report"], "posted", (post, post), (work, work));
        range(&m["report"], "returned", returned, return_work);
        // Both extractions precede all checks; the final case latent write is
        // shared. Only for these kernels, the hand-computed error mass stays
        // (4/5)(1/100)(1/5)(3/20)(1/10)+(1/5)(3/50)(1/2)(2/5)(3/10)
        // =93/125000 (AI:1/50 and3/20 =>231/125000), not a general theorem.
        let wrong = if name == "to-be-a" {
            "231/125000"
        } else {
            "93/125000"
        };
        let property = m["report"]["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "probability reached #1")
            .unwrap();
        assert_eq!(property["probability"], json!({"min":wrong,"max":wrong}));
        // The +1 upper endpoint is an optional post-return contest route,
        // not ordinary posted work. Posted human occupancy is22/22/10.
        let (min, max) = if name == "to-be-a" {
            ("8", "11")
        } else {
            ("20", "23")
        };
        assert_eq!(
            m["report"]["metrics"]["overall"]["named_people_work"],
            if deferred {
                json!({"min":null,"max":null,"reason":"not_configured"})
            } else {
                json!({"min":min,"max":max,"reason":null})
            }
        );
    }
    if name == "as-is" {
        for (i, b, path, post, ret) in [
            (2, "poV", "body/1", 20, (17, 19)),
            (3, "receiptV", "body/2", 22, (19, 21)),
        ] {
            let m = &ms[i];
            assert_eq!(
                m["id"],
                format!("summary_reused:AsIsFlow:{path}:{b}->posted")
            );
            assert_eq!(m["result"], "undetected");
            assert_eq!(m["reason"], "no_assigned_check");
            assert_eq!(m["assigned_changes"], json!([]));
            range(&m["report"], "posted", (post, post), (post, post));
            range(&m["report"], "returned", ret, ret);
        }
    }
    for (i, r) in [
        (5, "rule_version_unavailable"),
        (7, "quality_denominator_unavailable"),
    ] {
        assert_eq!(v["mutations"][i]["status"], "unsupported");
        assert_eq!(v["mutations"][i]["reason"], r);
    }
    let source =
        std::fs::read_to_string(root().join(format!("examples/invoice/{name}.relca"))).unwrap();
    let source = if deferred {
        source
    } else {
        deferred_invoice::without_defer(&source)
    };
    response_boundary::assert_not_applicable(&source, &v["mutations"]);
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("quality_changes"))
    );
}

#[test]
fn invoice_live_new_rows_and_hand_times_are_not_learned_from_snapshots() {
    for name in ["as-is", "to-be-p", "to-be-a"] {
        let o = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(["mutate", &format!("examples/invoice/{name}.relca")])
            .current_dir(root())
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(4));
        hand(name, &serde_json::from_slice(&o.stdout).unwrap(), true);
        let source =
            std::fs::read_to_string(root().join(format!("examples/invoice/{name}.relca"))).unwrap();
        let control = deferred_invoice::numeric_control(&source, "mutate", &[]);
        assert_eq!(control.status.code(), Some(4));
        hand(
            name,
            &serde_json::from_slice(&control.stdout).unwrap(),
            false,
        );
    }
}

#[test]
fn regenerated_mutation_snapshots_must_also_satisfy_the_hand_oracle() {
    // The same independent hand oracle checks saved current golden content.
    for name in ["as-is", "to-be-p", "to-be-a"] {
        let output = Command::new("python3")
            .args([
                "-B",
                "scripts/snapshots.py",
                "--show",
                &format!("invoice-{name}-mutate"),
            ])
            .current_dir(root())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let v: Value = serde_json::from_slice(&output.stdout).unwrap();
        hand(name, &v, true);
    }
}

#[test]
fn to_be_c_hidden_truth_gate_blocks_all_six_generated_kinds() {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["mutate", "examples/invoice/to-be-c.relca"])
        .current_dir(root())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["original_verdict"], "invalid");
    response_boundary::assert_blocked(&v["mutations"]);
    for i in [0, 1, 2, 4, 8, 9] {
        assert_eq!(v["mutations"][i]["status"], "blocked");
        assert_eq!(v["mutations"][i]["mutants"], json!([]));
    }
}
