//! Closed hand values for the intent fixtures, separate from the historical
//! state-IR census. No parser/lower/engine result is used to compute expectations.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const NAMES: [&str; 16] = [
    "intent-a1",
    "intent-a1.broken-no-loses",
    "intent-a2",
    "intent-a2.broken-extra-reader",
    "intent-a3",
    "intent-a3.broken-no-replace",
    "intent-a4",
    "intent-a4.broken-mismatch",
    "intent-a5",
    "intent-a5.broken-wrong-partner",
    "intent-gap",
    "intent-gap.broken-missing-replace",
    "intent-legacy",
    "intent-legacy.broken-preserve-scope",
    "intent-split",
    "intent-split.broken-retained-source",
];

pub fn name(id: &str) -> Option<&str> {
    id.strip_prefix("fixture-")
        .filter(|n| relca_test_support::fixture_match::intent_fixture(n))
}

pub fn validate_set(jobs: &[Value]) -> Result<(), String> {
    let found: Vec<_> = jobs
        .iter()
        .filter_map(|j| j["id"].as_str().and_then(name))
        .collect();
    require(
        found.len() == NAMES.len()
            && found.into_iter().collect::<BTreeSet<_>>() == NAMES.into_iter().collect(),
        "intent job set",
    )?;
    require(
        NAMES.into_iter().collect::<BTreeSet<_>>()
            == relca_test_support::fixture_match::INTENT_FIXTURES
                .into_iter()
                .collect(),
        "intent registration disagreement",
    )
}

fn require(ok: bool, what: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(what.into()) }
}
type Key = (String, String, String);
fn key(kind: &str, target: &str, change: &str) -> Key {
    (kind.into(), target.into(), change.into())
}
fn row_key(r: &Value) -> Result<Key, String> {
    Ok((
        r["kind"].as_str().ok_or("row kind")?.into(),
        r["target"].as_str().ok_or("row target")?.into(),
        r["change"].as_str().ok_or("row change")?.into(),
    ))
}
fn audit() -> BTreeSet<Key> {
    [
        ("actor", "Audit"),
        ("relation", "Audit.log"),
        ("authority", "Audit.log"),
        ("resource", "Audit"),
    ]
    .map(|(k, n)| key(k, n, "added"))
    .into_iter()
    .collect()
}
fn swap(old: &str) -> BTreeSet<Key> {
    let mut keys = BTreeSet::new();
    for kind in ["actor", "resource"] {
        keys.insert(key(kind, old, "removed"));
        keys.insert(key(kind, "Bot", "added"));
    }
    for kind in ["relation", "authority"] {
        keys.insert(key(kind, &format!("{old}.check"), "removed"));
        keys.insert(key(kind, "Bot.check", "added"));
    }
    keys.insert(key("visibility", "Doc", "modified"));
    keys.insert(key("relation", "v", "modified"));
    keys
}

struct Hand {
    minutes: [u64; 2],
    warnings: [usize; 3],
    exit: i32,
    rows: BTreeSet<Key>,
    covered: BTreeSet<Key>,
    declarations: Vec<(
        &'static str,
        &'static str,
        &'static str,
        Option<&'static str>,
    )>,
}
fn hand(n: &str) -> Hand {
    let mut h = Hand {
        minutes: [3, 2],
        warnings: [0, 0, 0],
        exit: 0,
        rows: swap("Clerk"),
        covered: swap("Clerk"),
        declarations: vec![("Clerk.check", "Bot.check", "auto", Some("eye"))],
    };
    match n {
        "intent-a1" => {}
        "intent-a1.broken-no-loses" => {
            h.warnings = [0, 1, 8];
            h.declarations[0].3 = None;
        }
        "intent-a2"
        | "intent-a2.broken-extra-reader"
        | "intent-a3"
        | "intent-a3.broken-no-replace" => {
            h.minutes = [4, 4];
            h.rows.remove(&key("actor", "Clerk", "removed"));
            h.rows.remove(&key("resource", "Clerk", "removed"));
            h.covered = h.rows.clone();
            h.rows.extend(audit());
            if n == "intent-a2" {
                h.warnings = [4, 0, 4];
            } else {
                h.covered.remove(&key("visibility", "Doc", "modified"));
                h.warnings = [5, 0, 5];
            }
            if n == "intent-a3.broken-no-replace" {
                h.declarations.clear();
                h.covered.clear();
                h.warnings = [12, 0, 12];
            }
        }
        "intent-a4" => {
            h.declarations[0].2 = "x";
            h.declarations[0].3 = None;
            h.warnings = [0, 1, 8];
        }
        "intent-a4.broken-mismatch" => {
            h.declarations = vec![("Desk.post", "Bot.check", "x", None)];
            h.covered.clear();
            h.warnings = [10, 1, 10];
            h.exit = 3;
        }
        "intent-a5" | "intent-a5.broken-wrong-partner" => {
            h.minutes = [4, 2];
            h.rows = swap("Robot");
            h.covered = h.rows.clone();
            h.declarations = vec![("Robot.check", "Bot.check", "migration", Some("support"))];
            if n.ends_with("broken-wrong-partner") {
                h.declarations[0].0 = "Clerk.check";
                h.covered.clear();
                h.warnings = [10, 0, 10];
                h.exit = 3;
            }
        }
        "intent-gap" | "intent-gap.broken-missing-replace" => {
            h.minutes = [5, 5];
            h.rows.clear();
            h.declarations.clear();
            for (old, new, reason) in [
                ("O.o1", "X.a", "r1"),
                ("O.o2", "X.b", "r2"),
                ("O.o3", "Y.a", "r3"),
                ("O.o4", "Y.b", "r4"),
            ] {
                for kind in ["relation", "authority"] {
                    h.rows.insert(key(kind, old, "removed"));
                    h.rows.insert(key(kind, new, "added"));
                }
                h.declarations.push((old, new, reason, Some("l")));
            }
            for kind in ["actor", "resource"] {
                for (name, change) in [("O", "removed"), ("X", "added"), ("Y", "added")] {
                    h.rows.insert(key(kind, name, change));
                }
            }
            h.rows.insert(key("visibility", "Doc", "modified"));
            h.covered = h.rows.clone();
            if n.ends_with("broken-missing-replace") {
                h.declarations.pop();
                h.warnings = [6, 0, 6];
                for kind in ["relation", "authority"] {
                    h.covered.remove(&key(kind, "O.o4", "removed"));
                    h.covered.remove(&key(kind, "Y.b", "added"));
                }
                for kind in ["actor", "resource"] {
                    h.covered.remove(&key(kind, "O", "removed"));
                }
            }
        }
        "intent-legacy" | "intent-legacy.broken-preserve-scope" => {
            h.minutes = [4, 2];
            h.rows = swap("Robot");
            h.covered.clear();
            h.declarations.clear();
            h.exit = 3;
            h.warnings = if n == "intent-legacy" {
                [10, 0, 10]
            } else {
                [10, 0, 0]
            };
        }
        "intent-split" => {
            h.minutes = [6, 4];
            h.rows.extend(audit());
            h.rows.insert(key("relation", "w", "modified"));
            h.covered.remove(&key("visibility", "Doc", "modified"));
            h.warnings = [6, 0, 6];
        }
        "intent-split.broken-retained-source" => {
            h.minutes = [6, 5];
            h.rows = [
                ("actor", "Bot", "added"),
                ("relation", "Bot.check", "added"),
                ("authority", "Bot.check", "added"),
                ("resource", "Bot", "added"),
                ("visibility", "Doc", "modified"),
                ("relation", "v", "modified"),
            ]
            .map(|(k, n, c)| key(k, n, c))
            .into_iter()
            .collect();
            h.covered.clear();
            h.warnings = [6, 0, 6];
            h.exit = 3;
        }
        _ => panic!("unregistered hand fixture {n}"),
    }
    h
}

pub fn check(id: &str, args: &[String], doc: &Value, exit: i32) -> Result<(), String> {
    let n = name(id).ok_or("unknown intent job")?;
    let from = if n.starts_with("intent-a5") || n.starts_with("intent-legacy") {
        "C"
    } else {
        "A"
    };
    require(
        args == [
            "diff",
            &format!("fixtures/{n}/model.relca"),
            "--from",
            from,
            "--to",
            "B",
        ],
        "intent argv",
    )?;
    relca_test_support::fixture_match::compare_diff_report(&json!({}), doc)?;
    let h = hand(n);
    require(
        exit == h.exit && doc["verdict"] == if exit == 0 { "satisfied" } else { "invalid" },
        "intent verdict/exit",
    )?;
    for (i, side) in ["from", "to"].into_iter().enumerate() {
        let t = doc[side]["report"]["terminals"]
            .as_array()
            .ok_or("side terminals")?;
        require(
            t.len() == 1
                && t[0]["end"] == "done"
                && t[0]["elapsed"] == json!({"min":h.minutes[i],"max":h.minutes[i]})
                && t[0]["work"] == json!({"min":h.minutes[i],"max":h.minutes[i]}),
            "hand serial minutes",
        )?;
    }
    let rows = doc["differences"].as_array().ok_or("differences")?;
    let actual = rows
        .iter()
        .map(row_key)
        .collect::<Result<BTreeSet<_>, _>>()?;
    require(
        actual == h.rows && actual.len() == rows.len(),
        "hand difference inventory",
    )?;
    let covered = rows
        .iter()
        .filter(|r| r.get("covered_by").is_some())
        .map(row_key)
        .collect::<Result<BTreeSet<_>, _>>()?;
    require(covered == h.covered, "hand covered inventory")?;
    let ds = doc["diagnostics"].as_array().ok_or("diagnostics")?;
    for (i, code) in ["W-DIFF-1", "W-DIFF-2", "W-DIFF-3"].into_iter().enumerate() {
        require(
            ds.iter().filter(|d| d["code"] == code).count() == h.warnings[i],
            "hand warning count",
        )?;
    }
    let errors: Vec<_> = ds.iter().filter(|d| d["severity"] == "error").collect();
    require(
        errors.len() == usize::from(h.exit == 3) && errors.iter().all(|d| d["code"] == "E-DIFF-0"),
        "hand errors",
    )?;
    require(
        errors.iter().all(|d| {
            d["message"]
                .as_str()
                .is_some_and(|m| m.contains("intent from C to B") == (n == "intent-legacy"))
        }),
        "guidance cause",
    )?;
    let pair = n.starts_with("intent-a5") || n.starts_with("intent-gap");
    require(
        if pair {
            doc["intent_block"] == json!({"from":from,"to":"B"})
        } else {
            doc.get("intent_block").is_none()
        },
        "hand block selection",
    )?;
    let declarations = doc.get("replacements").and_then(Value::as_array);
    require(
        if h.declarations.is_empty() {
            declarations.is_none()
        } else {
            declarations.is_some_and(|d| d.len() == h.declarations.len())
        },
        "hand declaration inventory",
    )?;
    if let Some(ds) = declarations {
        for (d, (from, to, reason, loses)) in ds.iter().zip(&h.declarations) {
            require(
                d["from"] == *from
                    && d["to"] == *to
                    && d["reason"] == *reason
                    && d["loses"] == json!(loses),
                "hand declaration values/order",
            )?;
        }
    }
    for row in rows.iter().filter(|r| r.get("covered_by").is_some()) {
        require(
            row["reason"].is_null() && row["loses"].is_null(),
            "replacement text occurs only on declaration",
        )?;
        let target = row["target"].as_str().ok_or("target")?;
        let selected: Vec<_> = h.declarations.iter().filter(|(from, to, _, _)| {
            if !n.starts_with("intent-gap") { return true; }
            match (row["kind"].as_str(), target) {
                (Some("visibility"), "Doc") => n == "intent-gap" || *from == "O.o3",
                (Some("actor" | "resource"), "O") => true,
                (Some("actor" | "resource"), "X") => to.starts_with("X."),
                (Some("actor" | "resource"), "Y") => to.starts_with("Y."),
                _ => *from == target || *to == target,
            }
        }).map(|(from, to, _, _)| json!({"intent":"replace","from":from,"to":to,"via":if row["kind"] == "relation" && (target == *from || target == *to) {"direct"} else {"derived"}})).collect();
        require(row["covered_by"] == json!(selected), "hand attribution/via")?;
    }
    // The independent hand source functions for the only two value-producing
    // dimensions: reader sets and consumed binding producers.
    let by_target: BTreeMap<_, _> = rows
        .iter()
        .map(|r| {
            (
                (r["kind"].as_str().unwrap(), r["target"].as_str().unwrap()),
                r,
            )
        })
        .collect();
    let docrow = by_target.get(&("visibility", "Doc")).ok_or("Doc row")?;
    let old = if from == "C" { "Robot" } else { "Clerk" };
    let (before, after) = if n.starts_with("intent-gap") {
        (json!([]), json!(["X", "Y"]))
    } else if n == "intent-split.broken-retained-source" {
        (json!([old]), json!(["Bot", "Clerk"]))
    } else if matches!(
        n,
        "intent-a2.broken-extra-reader"
            | "intent-a3"
            | "intent-a3.broken-no-replace"
            | "intent-split"
    ) {
        (json!([old]), json!(["Audit", "Bot"]))
    } else {
        (json!([old]), json!(["Bot"]))
    };
    require(
        docrow["from"] == before && docrow["to"] == after,
        "hand readers",
    )?;
    if !n.starts_with("intent-gap") {
        let v = by_target.get(&("relation", "v")).ok_or("v row")?;
        require(
            v["from"] == json!({"producer":format!("{old}.check")})
                && v["to"] == json!({"producer":"Bot.check"}),
            "hand v producer",
        )?;
    }
    if n == "intent-split" {
        let w = by_target.get(&("relation", "w")).ok_or("w row")?;
        require(
            w["from"] == json!({"producer":"Clerk.check"})
                && w["to"] == json!({"producer":"Audit.log"}),
            "hand w producer",
        )?;
    }
    Ok(())
}
