//! Every stage location of every explain counterexample against a line/column
//! oracle computed here from the byte span alone (ADR 0021 追記 2 の残り:
//! a fault in the first line's column was caught by no test).
use serde_json::Value;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn explain(path: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["explain", path.to_str().unwrap()])
        .output()
        .unwrap();
    serde_json::from_slice(&out.stdout).unwrap()
}

/// 1-based line and 1-based column counted in chars from the line start.
fn line_column(src: &str, byte: usize) -> (u64, u64) {
    let before = &src[..byte];
    let line = before.matches('\n').count() + 1;
    let start = before.rfind('\n').map_or(0, |n| n + 1);
    (line as u64, src[start..byte].chars().count() as u64 + 1)
}

fn locations(v: &Value, out: &mut Vec<Value>) {
    match v {
        Value::Object(m) => {
            if m.contains_key("span") && m.contains_key("line") {
                out.push(v.clone());
            }
            for x in m.values() {
                locations(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| locations(x, out)),
        _ => {}
    }
}

fn check_all(src: &str, report: &Value) -> usize {
    let mut found = Vec::new();
    locations(&report["explanation"], &mut found);
    for l in &found {
        let start = l["span"]["start"].as_u64().unwrap() as usize;
        let end = l["span"]["end"].as_u64().unwrap() as usize;
        let (line, column) = line_column(src, start);
        let (end_line, end_column) = line_column(src, end);
        assert_eq!(
            (&l["line"], &l["column"], &l["end_line"], &l["end_column"]),
            (
                &Value::from(line),
                &Value::from(column),
                &Value::from(end_line),
                &Value::from(end_column)
            ),
            "{l}"
        );
    }
    found.len()
}

fn flatten(src: &str) -> String {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut clause = vec![false; lines.len()];
    let mut inside = false;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        if inside && t.starts_with('}') {
            inside = false;
        } else if inside {
            clause[i] = !t.is_empty();
        }
        if t.starts_with("contract ") && t.ends_with('{') {
            inside = true;
        }
    }
    let mut out = String::with_capacity(src.len());
    for (i, l) in lines.iter().enumerate() {
        out.push_str(l);
        if i + 1 < lines.len() {
            out.push(if clause[i] && clause[i + 1] { ';' } else { ' ' });
        }
    }
    assert_eq!(out.len(), src.len());
    out
}

#[test]
fn every_explain_location_matches_the_span_on_multi_and_single_line_sources() {
    let dir = test_tmp::root().join(format!("relca-explain-locations-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut checked = 0;
    let mut first_line = 0;
    for entry in fs::read_dir(root().join("fixtures")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if !name.starts_with("explain-") {
            continue;
        }
        let src = fs::read_to_string(path.join("model.relca")).unwrap();
        let multi = explain(&path.join("model.relca"));
        checked += check_all(&src, &multi);
        // Same byte length with every newline replaced by one byte: a space,
        // or the clause separator ';' between two clause lines of a contract
        // block (a newline separates clauses there). Spans are unchanged and
        // every location moves to line 1.
        let flat = flatten(&src);
        let flat_path = dir.join(format!("{name}.relca"));
        fs::write(&flat_path, &flat).unwrap();
        let single = explain(&flat_path);
        let mut a = Vec::new();
        let mut b = Vec::new();
        locations(&multi["explanation"], &mut a);
        locations(&single["explanation"], &mut b);
        let spans = |v: &[Value]| v.iter().map(|l| l["span"].clone()).collect::<Vec<_>>();
        assert_eq!(spans(&a), spans(&b), "{name}: spans depend on line breaks");
        first_line += check_all(&flat, &single);
        assert!(b.iter().all(|l| l["line"] == 1), "{name}");
        assert_eq!(multi["verdict"], single["verdict"], "{name}");
        fs::remove_file(&flat_path).unwrap();
    }
    assert!(checked > 100, "too few locations checked: {checked}");
    assert_eq!(checked, first_line);
}

#[test]
fn columns_count_chars_not_bytes_after_a_multibyte_literal_on_the_same_line() {
    // The authority basis precedes every located element; flattened onto one
    // line, each column then sits after the multibyte literal.
    let name = "explain-executors.broken-shared-worker";
    let src = fs::read_to_string(root().join("fixtures").join(name).join("model.relca")).unwrap();
    assert_eq!(src.matches("basis \"table\"").count(), 1);
    let src = flatten(&src).replace("basis \"table\"", "basis \"台帳の規程\"");
    assert!(src.find("台帳").unwrap() < src.find("process").unwrap());
    let dir = test_tmp::root().join(format!("relca-explain-utf8-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.relca");
    fs::write(&path, &src).unwrap();
    let report = explain(&path);
    assert_eq!(report["verdict"], "violated", "{report}");
    assert!(check_all(&src, &report) > 10);
    fs::remove_file(&path).unwrap();
}
