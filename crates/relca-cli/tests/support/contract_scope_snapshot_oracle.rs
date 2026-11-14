//! Ten closed, hand-calculated scope jobs; old state-IR census is unchanged.
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const NAMES: [&str; 10] = [
    "scope-a1",
    "scope-a1.broken-called",
    "scope-a2",
    "scope-a2.broken-grant",
    "scope-a3",
    "scope-a3.broken-outside",
    "scope-a4",
    "scope-a4.broken-global",
    "scope-a5",
    "scope-a5.broken-preserve",
];
const ID: &str = "contract Controls #1 authority_basis";

pub fn name(id: &str) -> Option<&str> {
    id.strip_prefix("fixture-").filter(|n| NAMES.contains(n))
}

pub fn validate_set(jobs: &[Value]) -> Result<(), String> {
    let actual: Vec<_> = jobs
        .iter()
        .filter_map(|j| j["id"].as_str().and_then(name))
        .collect();
    if actual.len() != 10
        || actual.into_iter().collect::<BTreeSet<_>>() != NAMES.into_iter().collect()
    {
        return Err("scope snapshot population must be exactly ten jobs".into());
    }
    Ok(())
}

fn require(ok: bool, what: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(what.into()) }
}
fn property<'a>(r: &'a Value, id: &str) -> Result<&'a Value, String> {
    r["properties"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["id"] == id)
        .ok_or_else(|| format!("missing {id}"))
}
fn time(r: &Value, minutes: u64) -> Result<(), String> {
    require(
        r["bounds"]["truncated"] == false,
        "hand example must finish within clock",
    )?;
    let mut rows: Vec<_> = r["terminals"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["end"] == "done")
        .collect();
    if rows.is_empty() {
        rows = r["case_outcomes"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|c| c["terminals"].as_array().into_iter().flatten())
            .filter(|t| t["kind"] == "end" && t["end"] == "done")
            .collect();
    }
    require(
        rows.len() == 1
            && rows[0]["elapsed"] == json!({"min":minutes,"max":minutes})
            && rows[0]["work"] == json!({"min":minutes,"max":minutes}),
        "hand elapsed/work 1 or 1+1 minutes",
    )
}
fn scope(r: &Value, plan: &str, masks: bool) -> Result<(), String> {
    require(
        r["contract_scope"]
            == json!([{"id":ID,"contract":"Controls","applies_to":[plan],"reason":"approval only in New","status":"not_applicable","masks_call":masks}]),
        "hand scope row",
    )?;
    require(
        r["unchecked"].as_array().is_some_and(|a| {
            a.iter()
                .filter(|s| *s == &json!(format!("contract.scope.{ID}")))
                .count()
                == 1
        }),
        "hand scope unchecked",
    )?;
    require(
        !r["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == ID),
        "scope is not a satisfied property",
    )
}

pub fn check(id: &str, args: &[String], doc: &Value, exit: i32) -> Result<(), String> {
    let n = name(id).ok_or("unknown scope job")?;
    require(
        args.get(1) == Some(&format!("fixtures/{n}/model.relca")),
        "scope source identity",
    )?;
    let (verdict, code) = match n {
        "scope-a2.broken-grant" => ("violated", 1),
        "scope-a3" | "scope-a4.broken-global" => ("vacuous", 5),
        "scope-a5.broken-preserve" => ("invalid", 3),
        _ => ("satisfied", 0),
    };
    require(
        exit == code && doc["verdict"] == verdict,
        "hand scope verdict/exit",
    )?;
    let comparator = &relca_test_support::fixture_match::CONTRACT_SCOPE_FIXTURES;
    require(comparator.contains(&n), "scope fixture is registered")?;
    if n.starts_with("scope-a4") {
        require(
            args == [
                "compare",
                &format!("fixtures/{n}/model.relca"),
                "--plans",
                "Old,New",
                "--mode",
                "exhaustive",
            ],
            "scope compare argv",
        )?;
        relca_test_support::fixture_match::validate_compare_wire(doc)?;
        require(
            doc["plans"].as_array().is_some_and(|a| a.len() == 2),
            "two plans",
        )?;
        time(&doc["plans"][0]["report"], 1)?;
        time(&doc["plans"][1]["report"], 2)?;
        let rows = doc["tables"]["properties"]
            .as_array()
            .ok_or("scope table")?;
        require(rows.len() == 2, "both property rows retained")?;
        let controls = rows
            .iter()
            .find(|r| r["id"] == ID)
            .ok_or("scope table contract")?;
        if n == "scope-a4" {
            scope(&doc["plans"][0]["report"], "New", false)?;
            require(
                controls["values"][0]
                    == json!({"configuration":"Old","status":"not_applicable","verdict":null,"reason":"configuration_scope"})
                    && controls["values"][1]["status"] == "applied"
                    && controls["values"][1]["verdict"] == "satisfied",
                "hand asymmetric cells",
            )?;
        } else {
            require(
                controls["values"][0]["verdict"] == "vacuous"
                    && controls["values"][1]["verdict"] == "satisfied",
                "global contract still vacuous in Old",
            )?;
            require(
                rows.iter().all(|r| {
                    r["values"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|c| c.get("status").is_none())
                }),
                "old input has no new status",
            )?;
            require(
                !doc["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "W-CMP-1"),
                "no asymmetric-scope warning on global contract",
            )?;
        }
    } else if n.starts_with("scope-a5") {
        require(
            args == [
                "diff",
                &format!("fixtures/{n}/model.relca"),
                "--from",
                "New",
                "--to",
                "Old",
            ],
            "scope diff argv",
        )?;
        relca_test_support::fixture_match::compare_diff_report(&json!({}), doc)?;
        time(&doc["from"]["report"], 2)?;
        time(&doc["to"]["report"], 1)?;
        scope(&doc["to"]["report"], "New", false)?;
        let row = property(doc, ID)?;
        require(
            row["from"] == "satisfied"
                && row["to"].is_null()
                && row["from_status"] == "applied"
                && row["to_status"] == "not_applicable",
            "hand removed scope values",
        )?;
        require(
            doc["differences"].as_array().unwrap().iter().any(|d| {
                d["kind"] == "contract_scope"
                    && d["target"] == "Controls"
                    && d["change"] == "removed"
            }),
            "scope removal is visible",
        )?;
        if code == 3 {
            require(
                doc["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "E-DIFF-0")
                    && doc["preservation"]["summary"]["satisfied"] == 0,
                "excluded preserve cannot pass",
            )?;
        }
    } else {
        let plan = if n.starts_with("scope-a2") || n.starts_with("scope-a3") {
            "New"
        } else {
            "Old"
        };
        require(
            args == [
                "check",
                &format!("fixtures/{n}/model.relca"),
                "--configuration",
                plan,
            ],
            "scope check argv",
        )?;
        relca_test_support::fixture_match::compare_report(&json!({}), doc)?;
        time(
            doc,
            if n.starts_with("scope-a2") || n == "scope-a1.broken-called" {
                2
            } else {
                1
            },
        )?;
        require(
            property(doc, "reachable end.done")?["verdict"] == "satisfied",
            "done is reachable",
        )?;
        match n {
            "scope-a1" => scope(doc, "New", false)?,
            "scope-a1.broken-called" => scope(doc, "New", true)?,
            "scope-a3.broken-outside" => scope(doc, "Old", true)?,
            _ => require(
                property(doc, ID)?["verdict"] == verdict,
                "applied contract verdict is not skipped",
            )?,
        }
    }
    Ok(())
}
