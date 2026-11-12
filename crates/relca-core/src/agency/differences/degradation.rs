//! Side-specific structural dependency is distinct from whole-process L.
//! Missing/withdrawn evidence is never a known empty dependency or equality.
use super::*;

fn side(table: &Value, route: Option<&str>, item: &str, owner: &str) -> Result<Cell> {
    let at = format!("{owner}.agency.degradation");
    if table.is_null() {
        return Ok(unknown(&format!("{at}:null")));
    }
    if table["status"] == "not_derived" {
        let reason = table["reason"].as_str().ok_or_else(invalid)?;
        return Ok(unknown(&format!("{at}:{reason}")));
    }
    let route = route.ok_or_else(invalid)?;
    let at = format!("{at}.{route}.{item}");
    let row = table["routes"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .find(|r| r["name"] == route);
    let Some(row) = row else {
        return Ok(unknown(&format!("{at}:null")));
    };
    let value = &row[item];
    if item == "common_prefix" {
        return Ok(Cell::Known(value.clone()));
    }
    match value["status"].as_str() {
        Some("derived") => Ok(Cell::Known(value.clone())),
        Some("not_derived") => {
            let reason = value["reason"].as_str().ok_or_else(invalid)?;
            Ok(unknown(&format!("{at}:{reason}")))
        }
        _ => Err(invalid()),
    }
}

pub(super) fn append(
    a: &Value,
    z: &Value,
    out: &mut Vec<MeaningRow>,
    b: &mut Budget,
) -> Result<()> {
    let (a, z) = (&a["degradation"], &z["degradation"]);
    // Legacy inputs have neither declaration nor table: do not add rows to them.
    if a.is_null() && z.is_null() {
        return Ok(());
    }
    let mut routes = Vec::new();
    for table in [a, z] {
        if let Some(rows) = table["routes"].as_array() {
            for row in rows {
                b.work()?;
                let name = row["name"].as_str().ok_or_else(invalid)?;
                if !routes
                    .iter()
                    .any(|r: &Option<String>| r.as_deref() == Some(name))
                {
                    b.copy(1)?;
                    routes.push(Some(name.to_owned()));
                }
            }
        }
    }
    if routes.is_empty() {
        b.copy(1)?;
        routes.push(None);
    }
    for item in ["normal", "degraded", "common_prefix"] {
        for route in &routes {
            b.work()?;
            let x = side(a, route.as_deref(), item, "from")?;
            let y = side(z, route.as_deref(), item, "to")?;
            let (status, from, to) = match (&x, &y) {
                (Cell::Known(x), Cell::Known(y)) if x == y => continue,
                (Cell::Known(x), Cell::Known(y)) => (MeaningStatus::Changed, x.clone(), y.clone()),
                _ => (MeaningStatus::NotComparable, status(&x), status(&y)),
            };
            b.copy(5)?;
            let row = MeaningRow {
                viewpoint: "degradation".into(),
                item: item.into(),
                key: MeaningKey {
                    call: route.clone(),
                    ..MeaningKey::default()
                },
                status,
                from,
                to,
            };
            row.validate().map_err(|_| invalid())?;
            out.push(row);
        }
    }
    Ok(())
}

pub(super) fn validate_status(
    value: &Value,
    owner: &str,
    route: Option<&str>,
    item: &str,
) -> std::result::Result<(), String> {
    if value["status"] == "derived" && value["reason"].is_null() {
        return Ok(());
    }
    let reason = value["reason"]
        .as_str()
        .ok_or("missing degradation reason")?;
    let table = format!("{owner}.agency.degradation:");
    let valid = if let Some(r) = reason.strip_prefix(&table) {
        r == "null" || super::super::degradation::known_reason(r)
    } else {
        route
            .and_then(|route| {
                reason.strip_prefix(&format!("{owner}.agency.degradation.{route}.{item}:"))
            })
            .is_some_and(|r| {
                r == "null" || item != "common_prefix" && super::super::degradation::known_reason(r)
            })
    };
    if value["status"] != "not_derived" || !valid {
        return Err("invalid degradation comparability reason".into());
    }
    Ok(())
}
