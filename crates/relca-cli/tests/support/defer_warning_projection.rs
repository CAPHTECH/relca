// Source-only oracle for the additive warning. Never calls lower or the IR
// resolver, and does not learn expectations from the new output field.
#[allow(dead_code)]
pub fn legacy_defer_diagnostics(
    report: &Value,
    source: &str,
    selection: Option<&str>,
) -> Result<Vec<Value>, String> {
    use relca_core::ast;
    use std::collections::{BTreeMap, BTreeSet};
    let mut expected = Vec::new();
    let has_values = report.get("verdict").is_none()
        || report["metrics"].is_object()
        || report["quality"].is_object();
    if has_values && let Ok(model) = relca_core::syntax::parse(source) {
        let mut actors = BTreeSet::new();
        let mut capabilities = BTreeSet::new();
        let mut cuts = BTreeMap::new();
        let mut configurations = BTreeMap::new();
        let mut indicators = false;
        for item in &model.items {
            match item {
                ast::Item::Actor(a) => {
                    actors.insert(a.name.value.as_str());
                    for field in &a.items {
                        if let ast::ActorItem::Capability(c) = field {
                            capabilities.insert(format!("{}.{}", a.name.value, c.name.value));
                        }
                    }
                }
                ast::Item::Cut(c) => {
                    cuts.insert(c.name.value.as_str(), c);
                }
                ast::Item::Configuration(c) => {
                    configurations.insert(c.name.value.as_str(), c.cut.value.as_str());
                }
                ast::Item::Metrics(_) | ast::Item::QualityProperty(_) => indicators = true,
                _ => {}
            }
        }
        let selected = if configurations.is_empty() {
            (cuts.len() == 1).then(|| *cuts.values().next().unwrap())
        } else {
            selection
                .and_then(|s| configurations.get(s))
                .and_then(|c| cuts.get(c))
                .copied()
        };
        if indicators && let Some(cut) = selected {
            for item in &cut.items {
                let ast::CutItem::Defer(e) = &item.value else {
                    continue;
                };
                let target = e
                    .target
                    .value
                    .iter()
                    .map(|n| n.value.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                if actors.contains(target.as_str()) || capabilities.contains(&target) {
                    continue;
                }
                let hint = target
                    .split_once('.')
                    .filter(|(a, _)| actors.contains(a))
                    .map(|(a, c)| format!(" (Actor {a} has no capability {c})"))
                    .unwrap_or_default();
                expected.push(serde_json::json!({
                    "code":"W-CUT-2", "severity":"warning",
                    "message":format!("defer {target} matches no Actor or Actor.capability{hint}: every metrics and quality value is withdrawn as not_configured"),
                    "span":{"start":e.target.span.start,"end":e.target.span.end}
                }));
            }
        }
    }
    let diagnostics = report["diagnostics"]
        .as_array()
        .ok_or("diagnostics array")?;
    let observed: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["code"] == "W-CUT-2")
        .cloned()
        .collect();
    if observed != expected {
        return Err(format!(
            "source-bound W-CUT-2 mismatch: observed {observed:?}, expected {expected:?}"
        ));
    }
    Ok(diagnostics
        .iter()
        .filter(|d| d["code"] != "W-CUT-2")
        .cloned()
        .collect())
}
