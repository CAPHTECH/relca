//! Declaration-level checks run before selection, including excluded contracts.
use super::*;

impl Lower {
    pub(super) fn validate_contract_scopes(&mut self, model: &ast::Model) {
        let configurations: BTreeSet<_> = model
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Configuration(c) => Some(c.name.value.as_str()),
                _ => None,
            })
            .collect();
        let contracts: Vec<_> = model
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Contract(c) => Some(c),
                _ => None,
            })
            .collect();
        for c in &contracts {
            if let Some((names, _)) = &c.applies_to {
                if c.name.is_none() {
                    self.error(diag::U_SYN_0, "contract_scope_anonymous", c.span);
                }
                if configurations.is_empty() {
                    self.error(diag::E_CONF_0, "contract_scope_no_configuration", c.span);
                }
                if names.is_empty() {
                    self.error(diag::E_CON_0, "contract_scope_empty", c.span);
                }
                let mut seen = BTreeSet::new();
                for n in names {
                    if !seen.insert(&n.value) {
                        self.error(diag::E_CON_0, "contract_scope_duplicate", n.span);
                    }
                    if !configurations.is_empty() && !configurations.contains(n.value.as_str()) {
                        self.error(E_NAME_0, "contract_scope_unknown_configuration", n.span);
                    }
                }
                if c.clauses
                    .iter()
                    .any(|c| matches!(c.value, ast::ContractClause::Contest { .. }))
                {
                    self.unsupported("contract_scope_contest", c.span);
                }
            }
            for clause in &c.clauses {
                let ast::ContractClause::Waive { response, .. } = &clause.value else {
                    continue;
                };
                let target = if response.len() == 2 {
                    contracts.iter().copied().find(|d| {
                        d.name
                            .as_ref()
                            .is_some_and(|n| n.value == response[0].value)
                    })
                } else {
                    Some(*c)
                };
                let Some(target) = target else { continue };
                if let Some((target_names, _)) = &target.applies_to {
                    let subset = match &c.applies_to {
                        Some((names, _)) => names
                            .iter()
                            .all(|n| target_names.iter().any(|t| t.value == n.value)),
                        None => configurations
                            .iter()
                            .all(|n| target_names.iter().any(|t| t.value == *n)),
                    };
                    if !subset {
                        self.error(
                            diag::E_CON_0,
                            "contract_scope_waive_not_subset",
                            clause.span,
                        );
                    }
                }
            }
        }
    }
}
