//! Configuration selection is verification metadata, never execution state.
use crate::{Verdict, ast, diag};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Excluded {
    pub id: String,
    pub contract: String,
    pub applies_to: Vec<String>,
    pub reason: String,
    pub status: Status,
    pub masks_call: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    #[serde(rename = "not_applicable")]
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    Applied,
    NotApplicable,
}

pub(crate) fn inventory<'a>(
    ids: impl Iterator<Item = &'a str>,
    s: &'a Selection,
) -> std::collections::BTreeSet<&'a str> {
    ids.filter(|id| *id != "receipt_resolution")
        .chain(
            s.clauses
                .iter()
                .filter(|c| c.excluded)
                .map(|c| c.id.as_str()),
        )
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    pub clauses: Vec<Clause>,
    pub configuration: Option<String>,
    pub configuration_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    pub id: String,
    pub contract: Option<String>,
    pub applies_to: Option<(Vec<String>, String)>,
    pub excluded: bool,
    pub masks_call: bool,
    pub has_capability: bool,
    pub span: diag::Span,
    pub capabilities: Vec<(String, String)>,
}

pub(crate) fn applies(c: &ast::ContractDecl, selection: Option<&str>) -> bool {
    selection.is_none_or(|name| {
        c.applies_to
            .as_ref()
            .is_none_or(|(ns, _)| ns.iter().any(|n| n.value == name))
    })
}

impl Selection {
    pub(crate) fn from_ast(model: &ast::Model, selection: Option<&str>) -> Self {
        let configurations: Vec<_> = model
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Configuration(c) => Some(c),
                _ => None,
            })
            .collect();
        let selected = configurations
            .iter()
            .find(|c| Some(c.name.value.as_str()) == selection);
        let process = selected.and_then(|c| {
            model.items.iter().find_map(|i| match i {
                ast::Item::Process(p) if p.name.value == c.process.value => Some(p),
                _ => None,
            })
        });
        let mut calls = Vec::new();
        if let Some(p) = process {
            crate::lower::collect_calls(&p.body, &mut calls);
        }
        // Before lowering succeeds, preserve conservative scope disclosure on
        // refusals too. Validated models are resolved against the shared IR
        // inventory below, exactly as W-UNREACH counts calls.
        let routes: Vec<_> = model
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Contract(c) => Some(c),
                _ => None,
            })
            .flat_map(|c| &c.clauses)
            .filter_map(|c| match &c.value {
                ast::ContractClause::Contest { route, .. } => Some(route),
                _ => None,
            })
            .collect();
        let mut out = Self {
            configuration: selected.map(|c| c.name.value.clone()),
            configuration_count: configurations.len(),
            clauses: Vec::new(),
        };
        let mut ordinal = 0;
        for item in &model.items {
            let ast::Item::Contract(c) = item else {
                continue;
            };
            for clause in &c.clauses {
                ordinal += 1;
                let (kind, targets) = targets(&clause.value);
                let excluded = selected.is_some() && !applies(c, selection);
                let masks_call = targets.is_empty()
                    || targets.iter().any(|(a, cap)| {
                        calls
                            .iter()
                            .any(|c| &c.actor.value == *a && &c.capability.value == *cap)
                            || (excluded
                                && routes
                                    .iter()
                                    .any(|r| &r.actor.value == *a && &r.capability.value == *cap))
                    });
                let name = c.name.as_ref().map(|n| n.value.clone());
                out.clauses.push(Clause {
                    id: format!(
                        "contract{} #{ordinal} {kind}",
                        name.as_ref().map(|n| format!(" {n}")).unwrap_or_default()
                    ),
                    contract: name,
                    applies_to: c.applies_to.as_ref().map(|(ns, reason)| {
                        (
                            ns.iter().map(|n| n.value.clone()).collect(),
                            reason.value.clone(),
                        )
                    }),
                    excluded,
                    masks_call,
                    has_capability: !targets.is_empty(),
                    span: clause.span,
                    capabilities: targets
                        .iter()
                        .map(|(a, c)| ((*a).clone(), (*c).clone()))
                        .collect(),
                });
            }
        }
        out
    }

    pub fn scoped(&self) -> bool {
        self.clauses.iter().any(|c| c.applies_to.is_some())
    }

    pub(crate) fn excluded(&self) -> Result<Vec<Excluded>, diag::Diagnostic> {
        self.clauses
            .iter()
            .filter(|c| c.excluded)
            .map(|c| {
                let invalid = |code: &str, reason: &str| diag::Diagnostic {
                    code: code.into(),
                    severity: diag::Severity::Error,
                    span: Some(c.span),
                    message: reason.into(),
                };
                let contract = c
                    .contract
                    .clone()
                    .ok_or_else(|| invalid(diag::U_SYN_0, "contract_scope_anonymous"))?;
                let (applies_to, reason) = c
                    .applies_to
                    .as_ref()
                    .ok_or_else(|| invalid(diag::U_SEM_0, "contract_scope_declaration_missing"))?;
                Ok(Excluded {
                    id: c.id.clone(),
                    contract,
                    applies_to: applies_to.clone(),
                    reason: reason.clone(),
                    status: Status::NotApplicable,
                    masks_call: c.masks_call,
                })
            })
            .collect()
    }

    pub(crate) fn warnings(&self) -> Vec<diag::Diagnostic> {
        self.clauses.iter().filter(|c| c.excluded && c.masks_call).map(|c| diag::Diagnostic {
            code: diag::W_CONF_2.into(), severity: diag::Severity::Warning, span: Some(c.span),
            message: format!("{} is outside configuration scope but its target may be called in configuration {}", c.id, self.configuration.as_deref().unwrap_or_default()),
        }).collect()
    }

    pub(crate) fn guidance<'a>(
        &self,
        properties: impl Iterator<Item = (&'a str, Verdict)>,
    ) -> Vec<diag::Diagnostic> {
        if self.configuration_count < 2 {
            return Vec::new();
        }
        let Some(configuration) = &self.configuration else {
            return Vec::new();
        };
        properties.filter(|(_, verdict)| *verdict == Verdict::Vacuous).filter_map(|(id, _)| {
            let c = self.clauses.iter().find(|c| c.id == id && !c.excluded && c.has_capability && !c.masks_call)?;
            Some(diag::Diagnostic { code: diag::W_CONF_1.into(), severity: diag::Severity::Warning, span: Some(c.span), message: format!("{} is vacuous in configuration {configuration} whose process does not call its target; declare applies_to if the control is absent by design", c.id) })
        }).collect()
    }
}

pub(crate) fn resolve_calls(model: &mut crate::ir::Model) {
    let calls = crate::completion::lexical_capabilities(model);
    let names: std::collections::BTreeSet<_> = calls
        .iter()
        .filter_map(|id| model.capabilities.get(id.0))
        .map(|c| (model.actors[c.actor.0].name.clone(), c.name.clone()))
        .collect();
    for c in &mut model.contract_selection.clauses {
        // W-CONF-1 intentionally retains its existing process-only guidance;
        // masks_call is published only for excluded declarations (W-CONF-2).
        if c.excluded {
            c.masks_call =
                c.capabilities.is_empty() || c.capabilities.iter().any(|c| names.contains(c));
        }
    }
    model.diagnostics.retain(|d| d.code != diag::W_CONF_2);
    model
        .diagnostics
        .extend(model.contract_selection.warnings());
}

fn targets(c: &ast::ContractClause) -> (&'static str, Vec<(&String, &String)>) {
    use ast::ContractClause as C;
    fn event(e: &ast::Event) -> (&String, &String) {
        (&e.actor.value, &e.capability.value)
    }
    fn path(p: &ast::CapabilityPath) -> (&String, &String) {
        (&p.actor.value, &p.capability.value)
    }
    match c {
        C::Excludes { before, start } => ("excludes", vec![event(before), event(start)]),
        C::Waive { event: e, .. } => ("waive", vec![event(e)]),
        C::Contest { route, target, .. } => {
            let mut capabilities = vec![path(route)];
            if let ast::StandingRef::Commit(target) = target {
                capabilities.push(path(target));
            }
            ("contest_response", capabilities)
        }
        C::Separation { left, right } => ("separation", vec![event(left), event(right)]),
        C::Permitted { target } => ("permitted", vec![path(target)]),
        C::AuthorityBasis { target, .. } => ("authority_basis", vec![path(target)]),
        C::EvidenceBasis { target, .. } => ("evidence_basis", vec![path(target)]),
        C::Invariant { event: e, .. } => ("invariant", vec![event(e)]),
        C::StateInvariant { .. } => ("invariant", vec![]),
        C::AtMostOnce { event: e } => ("at_most_once", vec![event(e)]),
        C::RequiresEvent { event: e, after } => ("requires_event", vec![event(e), event(after)]),
        C::Response {
            trigger,
            targets,
            after,
            ..
        } => {
            let mut events = Vec::new();
            for t in [trigger, after] {
                if let ast::Trigger::Event(e) = t {
                    events.push(event(e));
                }
            }
            for t in targets {
                if let ast::Target::Event(e) = t {
                    events.push(event(e));
                }
            }
            ("response", events)
        }
    }
}

pub(crate) fn attach<D: From<diag::Diagnostic>>(
    rows: &mut Vec<Excluded>,
    unchecked: &mut Vec<String>,
    diagnostics: &mut Vec<D>,
    selection: &Selection,
) {
    *rows = match selection.excluded() {
        Ok(rows) => rows,
        Err(diagnostic) => {
            diagnostics.push(diagnostic.into());
            Vec::new()
        }
    };
    for c in rows {
        let key = format!("contract.scope.{}", c.id);
        if !unchecked.contains(&key) {
            unchecked.push(key);
        }
    }
}

#[cfg(test)]
mod round_one_tests {
    use super::*;

    #[test]
    fn non_executable_aliases_do_not_create_a_call_site() {
        let source = include_str!("../../relca-cli/tests/contract_scope/m1.relca").replace(
            "contract Controls {",
            "contract Controls applies_to [New] reason \"r\" {",
        );
        let ast = crate::syntax::parse(&source).unwrap();
        let mut model = crate::lower::lower_with_selection(&ast, Some("Old"), None).unwrap();
        let cap = crate::ir::CapabilityId(
            model
                .capabilities
                .iter()
                .position(|c| c.name == "approve")
                .unwrap(),
        );
        let span = model.process.span;
        model.process.body.push(crate::ir::Statement {
            span,
            kind: crate::ir::Stmt::LoopControl(crate::ir::LoopControl {
                updates: vec![],
                aliases: vec![crate::ir::Call {
                    args: vec![],
                    binding: crate::ir::BindingId(0),
                    capability: cap,
                    span,
                }],
            }),
        });
        assert!(!crate::completion::lexical_capabilities(&model).contains(&cap));
        resolve_calls(&mut model);
        assert!(!model.contract_selection.excluded().unwrap()[0].masks_call);
    }

    #[test]
    fn hand_ast_anonymous_scope_returns_diagnostic_instead_of_panicking() {
        let source = include_str!("../../relca-cli/tests/contract_scope/m1.relca").replace(
            "contract Controls {",
            "contract Controls applies_to [New] reason \"r\" {",
        );
        let mut model = crate::syntax::parse(&source).unwrap();
        for item in &mut model.items {
            if let ast::Item::Contract(c) = item {
                c.name = None;
            }
        }
        let errors = crate::lower::lower_with_selection(&model, Some("Old"), None).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|d| d.code == diag::U_SYN_0 && d.message == "contract_scope_anonymous")
        );
        let selected = Selection::from_ast(&model, Some("Old"));
        let result = std::panic::catch_unwind(|| selected.excluded());
        assert!(
            result.is_ok(),
            "scope projection must not panic on rejected hand AST"
        );
        assert!(format!("{:?}", result.unwrap()).contains("contract_scope_anonymous"));
    }
}
