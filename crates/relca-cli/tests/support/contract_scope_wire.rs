//! Independent wire extension: non-applicability is never a Verdict.
use super::*;
use std::collections::BTreeSet;

pub(super) fn schema() -> Ty {
    use Ty::*;
    Keyed(
        "id",
        Box::new(Obj(vec![
            ("id", Str),
            ("contract", Str),
            ("applies_to", Arr(Box::new(Str))),
            ("reason", Str),
            ("status", Lit(&["not_applicable"])),
            ("masks_call", Bool),
        ])),
    )
}

pub(super) fn marker(value: &Value) -> bool {
    value.as_str().is_some_and(|s| {
        s.strip_prefix("contract.scope.contract ")
            .is_some_and(|id| {
                let Some((name, suffix)) = id.rsplit_once(" #") else {
                    return false;
                };
                let Some((ordinal, kind)) = suffix.split_once(' ') else {
                    return false;
                };
                !name.is_empty()
                    && !name.contains(' ')
                    && ordinal
                        .parse::<usize>()
                        .is_ok_and(|n| n > 0 && n.to_string() == ordinal)
                    && [
                        "authority_basis",
                        "permitted",
                        "evidence_basis",
                        "at_most_once",
                        "requires_event",
                        "separation",
                        "excludes",
                        "invariant",
                        "response",
                        "waive",
                    ]
                    .contains(&kind)
            })
    })
}

pub(super) fn excluded(raw: &Value, id: &Value) -> bool {
    raw["contract_scope"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|r| r["id"] == *id)
}

pub(super) fn mutation(v: &Value) -> Result<(), String> {
    if v.get("contract_scope") != v["original"].get("contract_scope") {
        return Err("mutation scope differs from original".into());
    }
    let markers: Vec<_> = v["unchecked"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s.as_str().is_some_and(|s| s.starts_with("contract.scope.")))
        .collect();
    let rows = v["contract_scope"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if markers.len() != rows.len()
        || rows.iter().any(|r| {
            let expected =
                Value::String(format!("contract.scope.{}", r["id"].as_str().unwrap_or("")));
            markers.iter().filter(|m| ***m == expected).count() != 1
        })
    {
        return Err("mutation scope marker inventory".into());
    }
    Ok(())
}

pub(super) fn report(raw: &Value) -> Result<(), String> {
    let markers: Vec<_> = raw["unchecked"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v.as_str().is_some_and(|s| s.starts_with("contract.scope.")))
        .collect();
    let Some(rows) = raw.get("contract_scope") else {
        return if markers.is_empty() {
            Ok(())
        } else {
            Err("scope marker without declaration".into())
        };
    };
    check(rows, &schema(), "contract_scope")?;
    complete(rows, &schema(), "contract_scope")?;
    let rows = rows.as_array().unwrap();
    if rows.is_empty() || markers.len() != rows.len() {
        return Err("scope disclosure inventory".into());
    }
    for r in rows {
        let key = Value::String(format!("contract.scope.{}", r["id"].as_str().unwrap()));
        let names = r["applies_to"].as_array().unwrap();
        if !marker(&key)
            || r["contract"].as_str() == Some("")
            || !r["id"]
                .as_str()
                .unwrap()
                .starts_with(&format!("contract {} #", r["contract"].as_str().unwrap()))
            || names.is_empty()
            || names.iter().any(|n| n.as_str() == Some(""))
            || names
                .iter()
                .map(Value::to_string)
                .collect::<BTreeSet<_>>()
                .len()
                != names.len()
            || markers.iter().filter(|m| ***m == key).count() != 1
            || raw["properties"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|p| p["id"] == r["id"])
        {
            return Err("invalid scope row or applied/excluded overlap".into());
        }
        if r["masks_call"] == true
            && !raw["diagnostics"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|d| {
                    d["code"] == "W-CONF-2"
                        && d["message"]
                            .as_str()
                            .is_some_and(|m| m.contains(r["id"].as_str().unwrap()))
                })
        {
            return Err("masked call lost its warning".into());
        }
    }
    Ok(())
}

pub(super) fn compare_cell(mut fields: Vec<(&'static str, Ty)>) -> Ty {
    use Ty::*;
    fields.push(("status", OptionalField(Box::new(Lit(&["applied"])))));
    OneOf(vec![
        Obj(fields),
        Obj(vec![
            ("configuration", Str),
            ("status", Lit(&["not_applicable"])),
            ("verdict", OnlyNull),
            ("reason", Lit(&["configuration_scope"])),
        ]),
    ])
}

pub(super) fn comparison(v: &Value) -> Result<bool, String> {
    let rows = v["tables"]["properties"]
        .as_array()
        .ok_or("scope property rows")?;
    let plans = v["plans"].as_array().ok_or("scope plans")?;
    let scoped = plans
        .iter()
        .any(|p| p["report"].get("contract_scope").is_some())
        || rows.iter().any(|r| {
            r["values"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|c| c.get("status").is_some())
        });
    if !scoped {
        return Ok(false);
    }
    let mut asymmetric = false;
    for row in rows {
        let cells = row["values"].as_array().ok_or("scope cells")?;
        if cells.len() != plans.len() {
            return Err("scope cell count".into());
        }
        let mut statuses = BTreeSet::new();
        for (cell, plan) in cells.iter().zip(plans) {
            let not_applicable = excluded(&plan["report"], &row["id"]);
            let expected = if not_applicable {
                "not_applicable"
            } else {
                "applied"
            };
            if cell["status"] != expected {
                return Err("scope cell applicability".into());
            }
            statuses.insert(expected);
            if not_applicable
                && (!cell["verdict"].is_null() || cell["reason"] != "configuration_scope")
            {
                return Err("scope cell fabricated a verdict".into());
            }
        }
        asymmetric |= statuses.len() > 1;
    }
    let checked = v["checked"]
        .as_array()
        .ok_or("scope checked")?
        .iter()
        .filter(|x| *x == "contract_scope")
        .count();
    let warnings = v["diagnostics"]
        .as_array()
        .ok_or("scope diagnostics")?
        .iter()
        .filter(|d| d["code"] == "W-CMP-1")
        .count();
    if checked != usize::from(asymmetric) || warnings != usize::from(asymmetric) {
        return Err("asymmetric scope disclosure".into());
    }
    Ok(true)
}

pub(super) fn diff_property() -> Ty {
    use Ty::*;
    let case = |scoped| {
        Obj(vec![
            ("case_id", Int),
            ("arrival", Int),
            (
                "from",
                if scoped {
                    Null(Box::new(property_value_schema()))
                } else {
                    property_value_schema()
                },
            ),
            (
                "to",
                if scoped {
                    Null(Box::new(property_value_schema()))
                } else {
                    property_value_schema()
                },
            ),
        ])
    };
    let mut legacy = vec![
        ("id", Str),
        ("from", Verdict),
        ("to", Verdict),
        ("cases", OptionalField(Box::new(Arr(Box::new(case(false)))))),
    ];
    let old = Obj(legacy.clone());
    legacy[1].1 = Null(Box::new(Verdict));
    legacy[2].1 = Null(Box::new(Verdict));
    legacy[3].1 = OptionalField(Box::new(Arr(Box::new(case(true)))));
    legacy.extend([
        ("from_status", Lit(&["applied", "not_applicable"])),
        ("to_status", Lit(&["applied", "not_applicable"])),
    ]);
    OneOf(vec![old, Obj(legacy)])
}

pub(super) fn diff(v: &Value) -> Result<(), String> {
    if !v["properties"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|r| r.get("from_status").is_some() || r.get("to_status").is_some())
        && ["from", "to"]
            .iter()
            .all(|side| v[side]["report"].get("contract_scope").is_none())
    {
        return Ok(());
    }
    for row in v["properties"].as_array().ok_or("scope diff properties")? {
        let scoped = row.get("from_status").is_some() || row.get("to_status").is_some();
        for side in ["from", "to"] {
            let raw = &v[side]["report"];
            let ex = excluded(raw, &row["id"]);
            if scoped
                && row[format!("{side}_status")] != if ex { "not_applicable" } else { "applied" }
            {
                return Err("diff scope status".into());
            }
            let value = raw["properties"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|p| p["id"] == row["id"]);
            if ex {
                if !scoped || !row[side].is_null() || value.is_some() {
                    return Err("diff excluded verdict".into());
                }
            } else if value.is_none_or(|p| row[side] != p["verdict"]) {
                return Err("diff applied verdict".into());
            }
        }
    }
    Ok(())
}

pub(super) fn diff_legacy(v: &Value) -> Result<Value, String> {
    let mut legacy = v.clone();
    let rows = legacy["differences"]
        .as_array_mut()
        .ok_or("scope differences")?;
    for row in rows.iter().filter(|r| r["kind"] == "contract_scope") {
        let name = row["target"].as_str().ok_or("scope target")?;
        let ids = |side: &str| {
            v[side]["report"]["contract_scope"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["contract"] == name)
                .map(|c| c["id"].clone())
                .collect::<Vec<_>>()
        };
        let (a, b) = (ids("from"), ids("to"));
        let expected = if a.is_empty() && !b.is_empty() {
            "removed"
        } else if !a.is_empty() && b.is_empty() {
            "added"
        } else {
            return Err("scope difference has no directional change".into());
        };
        if row["change"] != expected {
            return Err("scope difference direction".into());
        }
    }
    rows.retain(|r| r["kind"] != "contract_scope");
    Ok(legacy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scope_metadata_has_a_hand_control_and_rejects_single_wire_faults() {
        let id = "contract Controls #1 authority_basis";
        let good = json!({"contract_scope":[{"id":id,"contract":"Controls","applies_to":["New"],"reason":"outside Old","status":"not_applicable","masks_call":false}],
            "unchecked":[format!("contract.scope.{id}")],"properties":[],"diagnostics":[]});
        report(&good).unwrap();
        for key in [
            "id",
            "contract",
            "applies_to",
            "reason",
            "status",
            "masks_call",
        ] {
            let mut bad = good.clone();
            bad["contract_scope"][0]
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(report(&bad).is_err(), "lost {key}");
        }
        for (key, value) in [
            ("status", json!("satisfied")),
            ("masks_call", json!("false")),
            ("applies_to", json!([])),
            ("applies_to", json!(["New", "New"])),
            ("extra", json!(true)),
        ] {
            let mut bad = good.clone();
            bad["contract_scope"][0][key] = value;
            assert!(report(&bad).is_err(), "forged {key}");
        }
        let mut bad = good.clone();
        bad["unchecked"] = json!([]);
        assert!(report(&bad).is_err());
        let mut bad = good.clone();
        bad["properties"] = json!([{"id":id,"verdict":"satisfied"}]);
        assert!(report(&bad).is_err());
        let mut bad = good.clone();
        bad["contract_scope"][0]["masks_call"] = json!(true);
        assert!(report(&bad).is_err());
    }

    #[test]
    fn not_applicable_cells_cannot_be_decoded_as_satisfied_or_missing_null() {
        let schema = compare_cell(vec![("configuration", Ty::Str), ("verdict", Ty::Verdict)]);
        let good = json!({"configuration":"Old","status":"not_applicable","verdict":null,"reason":"configuration_scope"});
        check(&good, &schema, "cell").unwrap();
        complete(&good, &schema, "cell").unwrap();
        for key in ["configuration", "status", "verdict", "reason"] {
            let mut bad = good.clone();
            bad.as_object_mut().unwrap().remove(key);
            assert!(
                check(&bad, &schema, "cell")
                    .and_then(|()| complete(&bad, &schema, "cell"))
                    .is_err(),
                "missing {key}"
            );
        }
        let mut bad = good.clone();
        bad["verdict"] = json!("satisfied");
        assert!(check(&bad, &schema, "cell").is_err());
        let mut bad = good;
        bad["probability"] = json!(null);
        assert!(check(&bad, &schema, "cell").is_err());
    }
}
