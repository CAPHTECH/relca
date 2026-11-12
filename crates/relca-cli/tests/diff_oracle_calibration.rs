#[path = "support/diff_oracle.rs"]
mod oracle;
use serde_json::{Value, json};

#[test]
fn independent_walker_is_calibrated_against_hand_written_f1_to_f7_and_invoice() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in [
        "diff2-time",
        "diff2-time.broken-sequential",
        "diff2-resource",
        "diff2-resource.broken-no-call",
        "diff2-meaning",
        "diff2-meaning.broken-same-end",
        "diff2-authority",
        "diff2-authority.broken-no-call",
        "diff2-visibility",
        "diff2-visibility.broken-same-reader",
        "diff2-contest",
        "diff2-contest.broken-same-end",
        "diff2-contest-reader",
        "diff2-contest-reader.broken-no-route-read",
    ] {
        let directory = root.join("fixtures").join(name);
        let src = std::fs::read_to_string(directory.join("model.relca")).unwrap();
        let hand: Value =
            serde_json::from_slice(&std::fs::read(directory.join("expected.json")).unwrap())
                .unwrap();
        let (status, mut rows) = oracle::rows(&src, "From", "To");
        if std::env::var("RELCA_DIFF_ORACLE_FAULT").as_deref() == Ok("visibility")
            && let Some(row) = rows.iter_mut().find(|r| r["kind"] == "visibility")
        {
            eprintln!("fault applied diff visibility to");
            row["to"] = json!(["wrong-reader"]);
        }
        assert_eq!(status, "compared");
        let expected: Vec<_> = hand["differences"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["kind"] != "actor" && row["kind"] != "relation")
            .collect();
        assert_eq!(rows.len(), expected.len(), "row count {name}");
        for (actual, expected) in rows.iter().zip(expected) {
            for (key, value) in expected.as_object().unwrap() {
                assert_eq!(&actual[key], value, "{name}/{key}");
            }
        }
    }
    let source = include_str!("../../../examples/invoice/configurations.relca");
    let (status, rows) = oracle::rows(source, "AsIs", "ToBeA");
    assert_eq!(status, "compared");
    assert_eq!(
        rows.iter()
            .map(|r| (r["kind"].as_str().unwrap(), r["target"].as_str().unwrap()))
            .collect::<Vec<_>>(),
        vec![
            ("authority", "Clerk.enter"),
            ("resource", "Extractor"),
            ("time", "Clerk.match_po"),
            ("time", "Clerk.match_receipt"),
            ("visibility", "Invoice")
        ]
    );
    assert_eq!(
        rows[0]["from"],
        json!({"permission":["c1","c2"],"grants":[],"authority_basis":[],"responsibilities":[]})
    );
    assert_eq!(rows[0]["to"], Value::Null);
    assert_eq!(rows[4]["to"], json!(["Clerk", "Extractor"]));
}
