//! Explicit capability replacements explain only rows that disappear under a
//! simultaneous, comparison-only substitution of the from-side call inventory.
use super::{Change, DiffReport, Kind, static_projection};
use crate::{ast, diag, ir, lower};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    Direct,
    Derived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "intent", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoveredBy {
    Replace { from: String, to: String, via: Via },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplacementCover {
    pub kind: Kind,
    pub target: String,
    pub change: Change,
    pub via: Via,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replacement {
    pub from: String,
    pub to: String,
    pub reason: String,
    pub loses: Option<String>,
    pub covers: Vec<ReplacementCover>,
    pub span: diag::Span,
}

fn path(p: &ast::CapabilityPath) -> String {
    format!("{}.{}", p.actor.value, p.capability.value)
}

fn capability(m: &ir::Model, name: &str) -> ir::CapabilityId {
    m.capabilities
        .iter()
        .enumerate()
        .find_map(|(i, _)| {
            let id = ir::CapabilityId(i);
            (static_projection::capname(m, id) == name).then_some(id)
        })
        .expect("statically resolved replacement capability")
}

pub(super) fn apply(
    result: &mut DiffReport,
    from: &lower::SelectedConfiguration,
    to: &lower::SelectedConfiguration,
    src: &str,
    covered: &mut [bool],
    guidance: bool,
) -> Vec<bool> {
    let mut annotations = vec![false; covered.len()];
    let mut declarations: Vec<_> = to
        .intent
        .iter()
        .filter_map(|i| match i {
            ast::Intent::Replace {
                from,
                to,
                reason,
                loses,
                span,
            } => Some(Replacement {
                from: path(from),
                to: path(to),
                reason: reason.value.clone(),
                loses: loses.as_ref().map(|l| l.value.clone()),
                covers: Vec::new(),
                span: *span,
            }),
            _ => None,
        })
        .collect();
    if declarations.is_empty() {
        return annotations;
    }
    declarations.sort_by(|a, b| a.from.cmp(&b.from));
    let mut valid = Vec::new();
    for (i, declaration) in declarations.iter().enumerate() {
        let matching = [
            (declaration.from.as_str(), Change::Removed),
            (declaration.to.as_str(), Change::Added),
        ]
        .iter()
        .all(|(name, change)| {
            result
                .differences
                .iter()
                .any(|d| d.kind == Kind::Relation && d.target == *name && d.change == *change)
        });
        if matching {
            valid.push((
                i,
                capability(&from.model, &declaration.from),
                capability(&from.model, &declaration.to),
            ));
        } else {
            super::intent::unmatched(
                result,
                format!(
                    "replace {} with {} has no matching removed and added relations",
                    declaration.from, declaration.to
                ),
                declaration.span,
                guidance,
            );
        }
        if declaration.loses.is_none() {
            result.diagnostic(
                diag::W_DIFF_2,
                format!(
                    "replace {} with {} does not record loses",
                    declaration.from, declaration.to
                ),
                Some(declaration.span),
            );
        }
    }
    let substitutions: BTreeMap<_, _> = valid.iter().map(|(_, a, b)| (*a, *b)).collect();
    let remaining = static_projection::replacement_difference_keys(src, from, to, &substitutions);
    // Precompute each leave-one-out and single-declaration comparison once.
    // Declaration order never controls either attribution or output order.
    let alternatives: Vec<_> = valid
        .iter()
        .map(|(i, a, b)| {
            let mut others = substitutions.clone();
            others.remove(a);
            (
                *i,
                static_projection::replacement_difference_keys(src, from, to, &others),
                static_projection::replacement_difference_keys(
                    src,
                    from,
                    to,
                    &BTreeMap::from([(*a, *b)]),
                ),
            )
        })
        .collect();
    for (index, row) in result.differences.iter_mut().enumerate() {
        if covered[index]
            || !matches!(
                row.kind,
                Kind::Actor | Kind::Relation | Kind::Authority | Kind::Resource | Kind::Visibility
            )
        {
            continue;
        }
        let key = (row.kind, row.target.clone());
        if remaining.contains(&key) {
            continue;
        }
        let necessary: BTreeSet<_> = alternatives
            .iter()
            .filter(|(_, others, _)| others.contains(&key))
            .map(|(i, _, _)| *i)
            .collect();
        let mut contributors: Vec<_> = alternatives
            .iter()
            .filter(|(i, _, alone)| {
                if necessary.is_empty() {
                    !alone.contains(&key)
                } else {
                    necessary.contains(i)
                }
            })
            .map(|(i, _, _)| *i)
            .collect();
        if contributors.is_empty() {
            // The full projection erased the row, but redundancy can make no
            // declaration necessary and no single declaration sufficient.
            contributors.extend(valid.iter().map(|(i, _, _)| *i));
        }
        for i in contributors {
            let d = &mut declarations[i];
            let via = if row.kind == Kind::Relation
                && ((row.target == d.from && row.change == Change::Removed)
                    || (row.target == d.to && row.change == Change::Added))
            {
                Via::Direct
            } else {
                Via::Derived
            };
            row.covered_by.push(CoveredBy::Replace {
                from: d.from.clone(),
                to: d.to.clone(),
                via,
            });
            d.covers.push(ReplacementCover {
                kind: row.kind,
                target: row.target.clone(),
                change: row.change,
                via,
            });
            covered[index] = true;
            annotations[index] |= via == Via::Direct || d.loses.is_some();
        }
    }
    result.replacements = Some(declarations);
    annotations
}
