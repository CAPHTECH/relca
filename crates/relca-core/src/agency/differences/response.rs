//! Only known call-bearing comparison fields, never a recursive string rewrite.
use super::*;

fn call(value: &mut Value, m: &ir::Model, budget: &mut Budget) -> Result<()> {
    let name = value.as_str().ok_or_else(invalid)?;
    let Some(id) = m.process.bindings.iter().position(|b| b.name == name) else {
        return Ok(()); // For example the separate synthetic contest namespace.
    };
    if let Some(r) = m.response_attempt(ir::BindingId(id)) {
        let command = m.process.bindings.get(r.command.0).ok_or_else(invalid)?;
        let bytes = command
            .name
            .len()
            .checked_add("response:".len() + ":attempt".len() + r.attempt.to_string().len())
            .ok_or_else(invalid)?;
        budget.copy(bytes)?; // Reserve before allocating the expanded name.
        *value = Value::String(m.comparison_site_name(ir::BindingId(id)));
    }
    Ok(())
}

fn judges(value: &mut Value, m: &ir::Model, budget: &mut Budget) -> Result<()> {
    if let Some(rows) = value.as_array_mut() {
        for row in rows {
            budget.work()?;
            if let Some(name) = row.get_mut("call") {
                call(name, m, budget)?; // Fixed Rule carriers have no call.
            }
        }
    }
    Ok(())
}

pub(super) fn normalize(value: &mut Value, m: &ir::Model, budget: &mut Budget) -> Result<()> {
    if m.response_attempts().is_empty() {
        return Ok(());
    }
    if let Some(rows) = value
        .get_mut("viewpoints")
        .and_then(|v| v.get_mut("effects"))
        .and_then(|v| v.get_mut("rows"))
        .and_then(Value::as_array_mut)
    {
        for row in &mut *rows {
            budget.work()?;
            call(row.get_mut("call").ok_or_else(invalid)?, m, budget)?;
            for field in ["j", "u"] {
                if let Some(v) = row.get_mut(field) {
                    judges(v, m, budget)?;
                }
            }
            if let Some(v) = row
                .get_mut("concentration")
                .and_then(|v| v.get_mut("generation_roots"))
            {
                judges(v, m, budget)?;
            }
        }
        let mut seen = BTreeSet::new();
        for row in rows.iter() {
            budget.copy(2)?;
            if !seen.insert((
                row["effect"].as_str().ok_or_else(invalid)?,
                row["call"].as_str().ok_or_else(invalid)?,
            )) {
                return Err(invalid()); // A collision is not a derived equality.
            }
        }
    }
    if let Some(rows) = value.get_mut("refusal").and_then(Value::as_array_mut) {
        for row in rows {
            budget.work()?;
            call(row.get_mut("call").ok_or_else(invalid)?, m, budget)?;
            if let Some(v) = row.get_mut("judgements") {
                judges(v, m, budget)?;
            }
        }
    }
    // Q1 withdraws framing; other tables are keyed by source/actor/capability
    // or aggregate lineage counts, not by generated response attempt slots.
    Ok(())
}
