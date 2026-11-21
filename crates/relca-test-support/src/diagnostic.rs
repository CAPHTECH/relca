//! Independent source-coordinate expectations for hand-authored diagnostics.
use relca_core::diag::{Location, Span};
use serde_json::{Value, json};

/// Only these presentation changes are projected in the migrated tests. Old
/// detail on unrelated diagnostics remains part of the comparison.
pub const NEW_REASONS: &[&str] = &[
    "simulation_unresolved_nondeterminism",
    "concurrent_read_write",
    "concurrent_writers",
    "result_not_definitely_available",
    "executor_identity_unavailable",
];

pub fn facts(rows: &[relca_core::diag::Diagnostic]) -> Vec<relca_core::diag::Diagnostic> {
    rows.iter()
        .cloned()
        .map(|mut row| {
            if let Some(detail) = &row.presentation.detail {
                if NEW_REASONS.contains(&detail.reason.as_str()) {
                    row.presentation.detail = None;
                }
            }
            row
        })
        .collect()
}

pub fn simulation_facts(
    mut outcome: relca_core::outcome::SimulationOutcome,
) -> relca_core::outcome::SimulationOutcome {
    outcome.diagnostics = facts(&outcome.diagnostics);
    outcome
}

pub fn row_facts(mut row: Value) -> Value {
    if let Some(reason) = row["detail"]["reason"].as_str() {
        if NEW_REASONS.contains(&reason) {
            row["detail"] = Value::Null;
        }
    }
    row
}

/// Remove only complete, JSON-backed diagnostic supplements after their own
/// header. A corrupt or unrecognized line stays in the old-channel comparison.
pub fn stderr_facts(stderr: &[u8], report: &Value) -> Vec<u8> {
    fn supplements(value: &Value, blocks: &mut Vec<(String, String)>) {
        if let Some(obj) = value.as_object() {
            if let (Some(code), Some(message), Some(detail)) = (
                value["code"].as_str(),
                value["message"].as_str(),
                value["detail"].as_object(),
            ) && obj.keys().any(|key| key == "severity")
            {
                let reason = detail["reason"].as_str().unwrap();
                if NEW_REASONS.contains(&reason) {
                    use std::fmt::Write as _;
                    let mut block = String::new();
                    let loc = &value["location"];
                    if loc.is_object() {
                        writeln!(block, "  at {}:{}", loc["line"], loc["column"]).unwrap();
                    }
                    writeln!(block, "  kind: {}", detail["kind"].as_str().unwrap()).unwrap();
                    if let Some(cause) = detail["cause"].as_str() {
                        writeln!(block, "  cause: {cause}").unwrap();
                    }
                    for row in detail["blockers"].as_array().unwrap() {
                        write!(block, "  blocked by: {}", row["role"].as_str().unwrap()).unwrap();
                        if let Some(name) = row["name"].as_str() {
                            write!(block, " {name}").unwrap();
                        }
                        if let Some(rounds) = row["rounds"].as_array() {
                            write!(
                                block,
                                " (rounds {})",
                                rounds
                                    .iter()
                                    .map(Value::to_string)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                            .unwrap();
                        }
                        let loc = &row["location"];
                        if loc.is_object() {
                            write!(block, " at {}:{}", loc["line"], loc["column"]).unwrap();
                        }
                        block.push('\n');
                    }
                    writeln!(block, "  see: {}", detail["see"].as_str().unwrap()).unwrap();
                    blocks.push((format!("{code}: {message}\n"), block));
                }
            }
            obj.values().for_each(|v| supplements(v, blocks));
        } else if let Some(rows) = value.as_array() {
            rows.iter().for_each(|v| supplements(v, blocks));
        }
    }
    let mut blocks = Vec::new();
    supplements(report, &mut blocks);
    let mut bytes = stderr;
    let mut result = Vec::with_capacity(bytes.len());
    while let Some((line, rest)) = bytes
        .split_inclusive(|b| *b == b'\n')
        .next()
        .map(|line| (line, &bytes[line.len()..]))
    {
        result.extend_from_slice(line);
        bytes = blocks
            .iter()
            .find_map(|(header, block)| {
                let prefix = line.strip_suffix(header.as_bytes())?;
                if !prefix.is_empty() && !prefix.ends_with(b": ") {
                    return None;
                }
                rest.strip_prefix(block.as_bytes())
            })
            .unwrap_or(rest);
    }
    result
}

#[cfg(test)]
#[test]
fn presentation_projection_keeps_even_single_byte_faults() {
    let row = json!({"code":"U-SEM-0","severity":"error","message":"old","span":{"start":2,"end":5},
        "location":{"line":2,"column":1,"end_line":2,"end_column":4},
        "detail":{"reason":"concurrent_writers","kind":"analysis_not_connected","cause":"fork_shared_write",
            "blockers":[{"role":"call","name":"w","rounds":[1,3],"location":{"line":4,"column":9,"end_line":4,"end_column":12}}],"see":"guide"}});
    let original = "owner: U-SEM-0: old\n";
    let supplement = "  at 2:1\n  kind: analysis_not_connected\n  cause: fork_shared_write\n  blocked by: call w (rounds 1, 3) at 4:9\n  see: guide\n";
    let project = |text: String| stderr_facts(text.as_bytes(), &row);
    assert_eq!(
        project(format!("{original}{supplement}")),
        original.as_bytes()
    );
    for faulty in [
        format!("{original}{supplement} "),
        format!("{original}{supplement}    \n"),
        format!("{original}{supplement}{original}"),
        format!("{original}{}", supplement.replace("guide", "guidE")),
        format!("{}{supplement}", original.replace("old", "Old")),
    ] {
        assert_ne!(project(faulty), original.as_bytes());
    }
    for (good, bad) in [
        ("at 2:1", "at 2:2"),
        ("call w", "call W"),
        ("1, 3", "1,3"),
        ("at 4:9", "at 4:8"),
    ] {
        let faulty = format!("{original}{}", supplement.replace(good, bad));
        assert_eq!(project(faulty.clone()), faulty.as_bytes());
    }

    let expected = json!({"code":"U-SEM-0","severity":"error","message":"old","span":{"start":2,"end":5},
        "location":{"line":2,"column":1,"end_line":2,"end_column":4},"detail":null});
    assert_eq!(row_facts(row.clone()), expected);
    let mut faulty_row = row.clone();
    faulty_row["message"] = json!("old!");
    assert_ne!(row_facts(faulty_row), expected);

    // An old reason with the same header and otherwise identical supplement
    // is still part of both comparison channels.
    let mut old_row = row.clone();
    old_row["detail"]["reason"] = json!("policy_feature_combination");
    assert_eq!(row_facts(old_row.clone()), old_row);
    let full = format!("{original}{supplement}");
    assert_eq!(stderr_facts(full.as_bytes(), &old_row), full.as_bytes());
    let old_row: relca_core::diag::Diagnostic = serde_json::from_value(old_row).unwrap();
    assert!(facts(std::slice::from_ref(&old_row)) == [old_row]);

    let row: relca_core::diag::Diagnostic = serde_json::from_value(row).unwrap();
    let projected = facts(std::slice::from_ref(&row));
    assert!(projected[0].presentation.detail.is_none());
    let mut bad = row;
    bad.message.push('!');
    assert_ne!(facts(&[bad]), projected);
}

pub fn location(source: &str, span: Option<Span>) -> Option<Location> {
    fn at(source: &str, byte: usize) -> Option<(usize, usize)> {
        let (line, column) = source
            .get(..byte)?
            .chars()
            .fold((1, 1), |(line, column), c| {
                if c == '\n' {
                    (line + 1, 1)
                } else {
                    (line, column + 1)
                }
            });
        Some((line, column))
    }
    let span = span.filter(|s| s.start <= s.end)?;
    let (line, column) = at(source, span.start)?;
    let (end_line, end_column) = at(source, span.end)?;
    Some(Location {
        line,
        column,
        end_line,
        end_column,
    })
}

/// Complete an independently authored expected row; never call this on output.
pub fn expected_row(source: &str, mut hand: Value) -> Value {
    let span =
        serde_json::from_value(hand.get("span").expect("hand diagnostic span").clone()).unwrap();
    assert!(hand.get("detail").is_none() && hand.get("location").is_none());
    hand["location"] = json!(location(source, span));
    hand["detail"] = Value::Null;
    hand
}

/// Insert the two fields into saved compact CLI stdout, preserving all other
/// bytes. The input is the regression expectation, not the output under test.
pub fn extend_saved_stdout(source: &str, saved: &str) -> String {
    let mut out = String::with_capacity(saved.len());
    let mut from = 0;
    for (start, _) in saved.match_indices("{\"code\":") {
        let mut stream = serde_json::Deserializer::from_str(&saved[start..]).into_iter::<Value>();
        let Some(Ok(row)) = stream.next() else {
            continue;
        };
        if !["severity", "message", "span"]
            .iter()
            .all(|k| row.get(k).is_some())
        {
            continue;
        }
        // New recordings already use the six-key wire. Keep the whole row
        // byte-for-byte for the caller's exact comparison.
        if row.get("location").is_some() || row.get("detail").is_some() {
            assert!(row.get("location").is_some() && row.get("detail").is_some());
            continue;
        }
        let end = start + stream.byte_offset() - 1;
        assert_eq!(saved.as_bytes()[end], b'}');
        let span = serde_json::from_value(row["span"].clone()).unwrap();
        out.push_str(&saved[from..end]);
        out.push_str(",\"location\":");
        out.push_str(&serde_json::to_string(&location(source, span)).unwrap());
        out.push_str(",\"detail\":null");
        from = end;
    }
    out.push_str(&saved[from..]);
    out
}

/// Compare reports from different sources after separately checking both sets
/// of derived coordinates. The other five fields remain in the comparison.
pub fn check_and_clear_locations(source: &str, value: &mut Value) {
    match value {
        Value::Object(row) => {
            if ["code", "severity", "message", "span"]
                .iter()
                .all(|k| row.contains_key(*k))
            {
                let span = serde_json::from_value(row["span"].clone()).unwrap();
                assert_eq!(row.get("location"), Some(&json!(location(source, span))));
                row.insert("location".into(), Value::Null);
            }
            for child in row.values_mut() {
                check_and_clear_locations(source, child);
            }
        }
        Value::Array(items) => {
            for item in items {
                check_and_clear_locations(source, item);
            }
        }
        _ => {}
    }
}
