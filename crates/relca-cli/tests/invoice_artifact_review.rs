//! Boundary regression: every advertised output pointer must carry evidence.
use relca_core::{ast, syntax};
use serde_json::Value;
#[path = "support/deferred_invoice.rs"]
mod deferred_invoice;
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/response_boundary_oracle.rs"]
mod response_boundary;
use std::{
    collections::BTreeMap,
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn live(id: &str) -> Value {
    let manifest: Value =
        serde_json::from_slice(&fs::read(root().join("examples/invoice/commands.json")).unwrap())
            .unwrap();
    let row = manifest
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap();
    let args: Vec<_> = row["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code().unwrap(),
        row["expected_exit"].as_i64().unwrap() as i32
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn readme_output_pointers_resolve_to_nonempty_evidence() {
    let readme = fs::read_to_string(root().join("examples/invoice/README.md")).unwrap();
    let mut failures = Vec::new();
    let mut reports = BTreeMap::new();
    for line in readme.lines().filter(|l| l.starts_with("| I")) {
        let cells: Vec<_> = line.split('|').map(str::trim).collect();
        if cells[3] != "output" {
            continue;
        }
        let mut checked = 0;
        for clause in cells[6].split(';') {
            let Some((ids, pointers)) = clause.split_once(':') else {
                continue;
            };
            if !pointers.split_whitespace().any(|w| w.starts_with('/')) {
                continue;
            }
            for id in ids.split(',').map(str::trim) {
                if id.ends_with(".relca") {
                    continue;
                }
                let value = reports.entry(id).or_insert_with(|| live(id));
                for word in pointers.split_whitespace() {
                    if !word.starts_with('/') {
                        continue;
                    }
                    checked += 1;
                    let pointer = word
                        .split(|c: char| !c.is_ascii() || c == ',')
                        .next()
                        .unwrap();
                    let resolved = value.pointer(pointer);
                    let nonempty = resolved.is_some_and(|v| match v {
                        Value::Null => false,
                        Value::Array(a) => !a.is_empty(),
                        Value::Object(o) => !o.is_empty(),
                        _ => true,
                    });
                    if !nonempty {
                        failures.push(format!("{} {id} {pointer}", cells[1]));
                    }
                }
            }
        }
        assert!(checked > 0, "output row has no pointers: {}", cells[1]);
    }
    assert!(
        failures.is_empty(),
        "missing evidence:\n{}",
        failures.join("\n")
    );
}

fn structure(value: &impl Debug) -> String {
    let debug = format!("{value:?}");
    let mut rest = debug.as_str();
    let mut result = String::new();
    while let Some(start) = rest.find("span: Span { start: ") {
        result.push_str(&rest[..start]);
        let span = &rest[start..];
        let end = span.find('}').unwrap();
        let numbers = span["span: Span { start: ".len()..end].replace(", end: ", "");
        assert!(numbers.trim().bytes().all(|c| c.is_ascii_digit()));
        result.push_str("span: POSITION");
        rest = &span[end + 1..];
    }
    result.push_str(rest);
    result
}

#[test]
fn c_common_ast_and_latent_diagnostic_position() {
    let common = |name: &str| {
        let source =
            fs::read_to_string(root().join(format!("examples/invoice/{name}.relca"))).unwrap();
        syntax::parse(&source)
            .unwrap()
            .items
            .iter()
            .filter(|i| !matches!(i, ast::Item::Process(_) | ast::Item::Configuration(_)))
            .map(structure)
            .collect::<Vec<_>>()
    };
    assert_eq!(common("to-be-c"), common("configurations"));
    let report = live("to-be-c-check");
    let diagnostic = &report["diagnostics"][0];
    assert_eq!(diagnostic["code"], "E-POL-LATENT");
    let source = fs::read_to_string(root().join("examples/invoice/to-be-c.relca")).unwrap();
    let start = diagnostic["span"]["start"].as_u64().unwrap() as usize;
    let end = diagnostic["span"]["end"].as_u64().unwrap() as usize;
    assert_eq!(&source[start..end], "Invoice.difficulty == difficult");
    assert_eq!(start, source.find("if Invoice.difficulty").unwrap() + 3);
}

#[test]
fn approver_origin_and_refusal_have_all_input_sources() {
    for id in ["as-is-check", "to-be-p-check", "to-be-a-check"] {
        let v = live(id);
        let approver = v["agency"]["origin"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["actor"] == "Approver")
            .unwrap();
        assert_eq!(approver["direct"], serde_json::json!([]));
        assert_eq!(
            approver["indirect"],
            serde_json::json!(["Invoice", "PurchaseOrder", "ReceiptRecord"])
        );
        let judgements = v["agency"]["refusal"][0]["judgements"].as_array().unwrap();
        assert_eq!(judgements.len(), 3);
        assert!(
            judgements
                .iter()
                .all(|j| j["stops"] == "all_paths" && j["bypasses"] == 0)
        );
        assert_eq!(
            v["cut"]["externality"],
            serde_json::json!(["Vendor.dispute_effort"])
        );
        assert_eq!(
            v["cut"]["reentry"],
            serde_json::json!([
                "contest Vendor -> Clerk.recheck",
                "review cut quarterly by cfo"
            ])
        );
    }
}

#[test]
fn compare_and_diff_independent_hand_counts_and_values() {
    let compare = live("compare-exhaustive");
    let source = fs::read_to_string(root().join("examples/invoice/configurations.relca")).unwrap();
    let out = deferred_invoice::numeric_control(
        &source,
        "compare",
        &["--plans", "AsIs,ToBeP,ToBeA", "--mode", "exhaustive"],
    );
    assert_eq!(out.status.code(), Some(1));
    let numeric: Value = serde_json::from_slice(&out.stdout).unwrap();
    let plans = compare["plans"].as_array().unwrap();
    assert_eq!(plans.len(), 3);
    for (index, (plan, (name, elapsed, work, wrong, human))) in plans
        .iter()
        .zip([
            ("AsIs", 17, 17, "93/125000", "17"),
            ("ToBeP", 14, 17, "93/125000", "17"),
            ("ToBeA", 9, 12, "231/125000", "11"),
        ])
        .enumerate()
    {
        assert_eq!(plan["configuration"], name);
        assert_eq!(numeric["plans"][index]["configuration"], name);
        let report = &plan["report"];
        deferred_invoice::assert_deferred(report);
        let posted = report["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["end"] == "posted")
            .unwrap();
        assert_eq!(
            posted["elapsed"],
            serde_json::json!({"min":elapsed,"max":elapsed})
        );
        assert_eq!(posted["work"], serde_json::json!({"min":work,"max":work}));
        assert_eq!(
            report["properties"][1]["probability"],
            serde_json::json!({"min":wrong,"max":wrong})
        );
        assert_eq!(
            numeric["plans"][index]["report"]["metrics"]["overall"]["named_people_work"]["max"],
            human
        );
    }
    let table = compare["tables"]["terminals"].as_array().unwrap();
    assert_eq!(table.len(), 4); // three declared ends plus rejected
    let posted = table.iter().find(|t| t["end"] == "posted").unwrap();
    for (row, (name, elapsed, probability)) in posted["values"].as_array().unwrap().iter().zip([
        ("AsIs", 17, "122593/125000"),
        ("ToBeP", 14, "122593/125000"),
        ("ToBeA", 9, "119481/125000"),
    ]) {
        assert_eq!(row["configuration"], name);
        assert_eq!(row["status"], "reported");
        assert_eq!(
            row["value"]["elapsed"],
            serde_json::json!({"min":elapsed,"max":elapsed})
        );
        assert_eq!(
            row["value"]["probability"],
            serde_json::json!({"min":probability,"max":probability})
        );
    }
    let property_rows = compare["tables"]["properties"].as_array().unwrap();
    assert_eq!(property_rows.len(), 9 + 1); // old nine plus distinct runtime response
    assert_eq!(
        property_rows
            .iter()
            .filter(|r| r["id"] == "contract InvoiceControls #7 contest_response")
            .count(),
        1
    );
    let diff = live("diff");
    assert_eq!(
        serde_json::Value::Array(diff["differences"].as_array().unwrap()[..3].to_vec()),
        serde_json::json!([
            {"kind":"actor","target":"Extractor","change":"added","reason":null,"loses":null},
            {"kind":"relation","target":"Extractor.extract","change":"added","reason":null,"loses":null},
            {"kind":"relation","target":"Clerk.enter","change":"removed","reason":null,"loses":null}
        ])
    );
    // ADR 0031 boundary review: authority Clerk.enter (Permission [c1,c2] ->
    // null) is one of eight old rows. ADR0051 adds entry's sole producer transfer
    // Clerk.enter -> Extractor.extract. No intent/annotation: W1 9 + W3 9 = 18.
    assert_eq!(diff["differences"].as_array().unwrap().len(), 9);
    assert_eq!(
        diff["differences"][8],
        serde_json::json!({
            "kind":"relation","target":"entry","change":"modified","reason":null,"loses":null,
            "from":{"producer":"Clerk.enter"},"to":{"producer":"Extractor.extract"}
        })
    );
    assert_eq!(diff["diagnostics"].as_array().unwrap().len(), 18);
    for code in ["W-DIFF-1", "W-DIFF-3"] {
        assert_eq!(
            diff["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["code"] == code)
                .count(),
            9
        );
    }
    assert!(
        diff["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["code"] == "W-DIFF-1" || d["code"] == "W-DIFF-3")
    );
    assert_eq!(diff["terminals"].as_array().unwrap().len(), 2);
    let posted = diff["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["end"] == "posted")
        .unwrap();
    assert_eq!(
        posted["from"]["elapsed"],
        serde_json::json!({"min":17,"max":17})
    );
    assert_eq!(
        posted["to"]["elapsed"],
        serde_json::json!({"min":9,"max":9})
    );
}

#[test]
fn mutation_unsupported_reasons_and_simulation_boundary_are_explicit() {
    let v = live("as-is-mutate");
    let mutations = v["mutations"].as_array().unwrap();
    assert_eq!(mutations.len(), 10);
    for (kind, reason) in [
        ("stale_definition_write", "rule_version_unavailable"),
        (
            "old_promise_denominator_removed",
            "quality_denominator_unavailable",
        ),
    ] {
        let row = mutations.iter().find(|m| m["kind"] == kind).unwrap();
        assert_eq!(row["status"], "unsupported");
        assert_eq!(row["reason"], reason);
    }
    let source = fs::read_to_string(root().join("examples/invoice/as-is.relca")).unwrap();
    response_boundary::assert_not_applicable(&source, &v["mutations"]);
    assert_eq!(mutations[1]["status"], "generated");
    assert_eq!(mutations[1]["mutants"].as_array().unwrap().len(), 1);
    assert_eq!(mutations[1]["mutants"][0]["result"], "invalid_mutant");
    assert_eq!(mutations[1]["mutants"][0]["reason"], "static_unsupported");
    assert_eq!(mutations[4]["status"], "generated");
    assert_eq!(mutations[4]["mutants"].as_array().unwrap().len(), 4);
    let v = live("as-is-simulate");
    assert_eq!(v["verdict"], "unsupported");
    let diagnostic = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "U-SEM-0")
        .unwrap();
    assert!(
        diagnostic["message"]
            .as_str()
            .unwrap()
            .contains("at least two distinct successor actions remain")
    );
}
