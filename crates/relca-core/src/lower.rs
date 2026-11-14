//! Declaration resolution and fail-closed static checking.
//!
//! Index declarations before uses;
//! validate every capability, including unused ones; collect lexical call slots
//! and terminal declarations before checking control flow; resolve joins locally;
//! track definitely available results separately from names. Cancellation uses
//! the checkpoint's static rule, not reachability. Conservative timing facts are
//! only used for result availability. Reject ambiguous result_at aggregation.
//! Check hand-built fixture ASTs, inject one defect, then run all required checks.
//!
//! Declaration validation rules:
//! Separate declaration validation from unique-name registration; visit all bodies.
//! Mark ambiguous names before resolving uses, rather than selecting a first winner.
//! Validate each Actor's capabilities against its own capacity, including duplicates.
//! Keep unknown/ambiguous types out of downstream placeholder-based type checks.
//! Audit every return/continue/break and implicit Option short circuit for lost checks.
//! Pair reordered duplicate declarations/fields and repeated ends with static faults.
//! Calibrate one regression by restoring the Capability early exit; restore and run
//! focused lower tests, workspace clippy and check.sh offline. Public types stay fixed.
//!
//! Static-check exit inventory (source line positions are historical):
//! | Site | Disposition / checks that must survive |
//! | model type continue 863 | fixed: validate every finite member set |
//! | model Actor continue 879 | fixed: fields and each declaration's own capacity |
//! | model histories break 949 | non-applicable: one aggregate U, no E checks skipped |
//! | model returns 986, 991 | non-applicable: after all static checks; fail closed |
//! | capability return 1067 | fixed: type, duration/capacity, result and cancel fields |
//! | collect_call return 1188 | fixed: check occurrence before registration-only exit |
//! | condition return 1324 | non-applicable: no binding means no determinate type;
//! | | all sibling atoms and both statement bodies are still checked |
//! | truth return 1363 | non-applicable: no diagnostics, conservative truth only |
//! | join all return 1411 | non-applicable: any-only fields are absent; target checked |
//! | available_after_cancel return 1530 | non-applicable: no diagnostics, facts only |
//! | bounds Option exits 1584-1586 | non-applicable: all fields checked before assembly |
//! Repeated end labels have no diagnostic exit: later statements are checked too.
//! Registration is not validation: branch checks also visit excluded duplicate calls.
//! Test obligations: exit coverage, order-invariant diagnostics,
//! public-contract/path preservation, and calibrated paired regressions. The table
//! is a source audit, not an all-input proof; executable evidence is the unit suite.

use std::collections::{BTreeMap, BTreeSet};

use crate::{ast, diag, ir};
use diag::{Diagnostic, E_CANCEL_0, E_CAP_0, E_NAME_0, E_TYPE_0, Span, U_SEM_0};

mod arrival_distribution;
mod contract_scope;
mod merge;
mod priority;
mod recovery;
mod resource_time;
mod response_loss;

/// Resolve and validate every declaration, including unused capabilities.
/// Err is nonempty; no partial accepted IR escapes on error. Ok contains all
/// defaults/assumptions and warnings, including W-CUT-0 exactly once.
pub fn lower(model: &ast::Model) -> Result<ir::Model, Vec<Diagnostic>> {
    lower_with_selection(model, None, None)
}

/// Advisory relation only (ADR 0041, round 9): do not feed these rows into
/// execution, reachability, or concurrency resolution. Reports place them at
/// their output boundary (before closed quality/simulation tails), not partial IR.
pub(crate) fn read_write_assumptions(model: &ir::Model) -> Vec<ir::Assumption> {
    fn reads(condition: &ir::Cond, variable: ir::StateVarId) -> bool {
        match condition {
            ir::Cond::StateEq(state) => state.variable == variable,
            ir::Cond::IntCompare { variable: id, .. } | ir::Cond::Contains { set: id, .. } => {
                *id == variable
            }
            ir::Cond::And(conditions) | ir::Cond::Or(conditions) => {
                conditions.iter().any(|c| reads(c, variable))
            }
            ir::Cond::Eq(_) => false,
        }
    }

    fn process_readers(block: &ir::Block, variable: ir::StateVarId, starts: &mut Vec<usize>) {
        for statement in block {
            if let ir::Stmt::If {
                condition,
                then_body,
                else_body,
            } = &statement.kind
            {
                if reads(condition, variable) {
                    starts.push(statement.span.start);
                }
                process_readers(then_body, variable, starts);
                process_readers(else_body, variable, starts);
            }
        }
    }

    let mut rows = Vec::new();
    let mut single_case = 0;
    let mut case_local = 0;
    let capability_name =
        |c: &ir::Capability| format!("{}.{}", model.actors[c.actor.0].name, c.name);
    for (index, state) in model.state_variables.iter().enumerate() {
        let local = state.kind == ir::StateKind::ObservedCase;
        if !local && !model.is_shared_observed(state.kind) {
            continue;
        }
        let variable = ir::StateVarId(index);
        let writers: Vec<_> = model
            .capabilities
            .iter()
            .filter(|c| {
                c.start_writes
                    .iter()
                    .chain(c.finish_writes())
                    .any(|w| w.variable == variable)
            })
            .map(capability_name)
            .collect();
        let mut readers: Vec<_> = model
            .capabilities
            .iter()
            .filter(|c| {
                c.requires
                    .as_ref()
                    .is_some_and(|r| reads(&r.condition, variable))
                    || c.atomic
                        .as_ref()
                        .is_some_and(|a| reads(&a.condition, variable))
            })
            .map(capability_name)
            .collect();
        let mut starts = Vec::new();
        process_readers(&model.process.body, variable, &mut starts);
        starts.sort_unstable();
        starts.dedup();
        readers.extend(
            starts
                .into_iter()
                .map(|start| format!("process.{}@if:{start}", model.process.name)),
        );
        // Each Capability contributes once despite repeated reads/writes; if
        // readers are distinct byte sites. Thus this product counts the same
        // static (variable, reader, writer) set as the candidate rows, without
        // pruning unused declarations, unreachable branches or serialization.
        let pairs = readers.len() * writers.len();
        if local {
            case_local += pairs;
            continue;
        }
        if model.bounds.cases < 2 {
            single_case += pairs;
            continue;
        }
        let ir::StateKind::ActorObserved(actor) = state.kind else {
            unreachable!("shared observed state has an Actor owner")
        };
        for reader in readers {
            for writer in &writers {
                rows.push(ir::Assumption {
                    field: "concurrency.read_write".into(),
                    value: format!("{}.{}: {reader} -> {writer}", model.actors[actor.0].name, state.name),
                    affects: "共有の観測stateの静的な読取り/書込み候補。到達や同時実行を確定しない。到達不能な枝や資源による直列化も含みうる。全順序を展開し、古い値を読む経路を残す。複数案件の共有Actor stateだけを対象とし、単一案件・案件局所・per_case・潜在は一覧の対象外。候補の不在は競合なしの証明ではない。".into(),
                });
            }
        }
    }
    if single_case > 0 || case_local > 0 {
        rows.push(ir::Assumption {
            field: "concurrency.read_write_excluded".into(),
            value: format!("single_case={single_case}; case_local={case_local}"),
            affects: "一覧の対象外の静的な読取り/書込み対の種類と数。単一案件のfork兄弟もsingle_caseに含む。到達や同時実行を確定しない。潜在の読取りは数えない。候補の不在は競合なしの証明ではない。".into(),
        });
    }
    rows
}

/// Validate every scenario with a fresh lowering context, then return only the
/// selected executable model. Common declarations are never overridden.
pub fn lower_with_selection(
    model: &ast::Model,
    configuration: Option<&str>,
    scenario: Option<&str>,
) -> Result<ir::Model, Vec<Diagnostic>> {
    #[cfg(test)]
    crate::evaluation_spy::hit(0);
    let scenarios: Vec<_> = model
        .items
        .iter()
        .filter_map(|i| match i {
            ast::Item::Scenario(s) => Some(s),
            _ => None,
        })
        .collect();
    let mut compare_names = Lower::default();
    for item in &model.items {
        if let ast::Item::Compare(c) = item {
            let mut seen = BTreeSet::new();
            for n in c.under.iter().flatten() {
                if !seen.insert(&n.value) {
                    compare_names.unsupported("duplicate compare scenario", n.span);
                }
                if !scenarios.iter().any(|s| s.name.value == n.value) {
                    compare_names.error(E_NAME_0, "unknown scenario", n.span);
                }
            }
        }
    }
    if scenarios.is_empty() {
        if scenario.is_some() {
            let mut l = Lower::default();
            l.error(
                diag::E_CONF_0,
                "scenario selector requires scenario declarations",
                model.span,
            );
            l.errors.extend(compare_names.errors);
            return Err(l.errors);
        }
        let lowered = lower_effective(model, configuration, None, &[]);
        if compare_names.errors.is_empty() {
            return lowered;
        }
        let mut errors = compare_names.errors;
        errors.extend(match lowered {
            Ok(m) => m.diagnostics,
            Err(ds) => ds,
        });
        return Err(errors);
    }
    let mut errors = compare_names.errors;
    let mut names = BTreeSet::new();
    let mut selected = None;
    let mut selector = Lower::default();
    match scenario {
        None => selector.error(
            diag::E_CONF_0,
            "a scenario selector is required",
            model.span,
        ),
        Some(name) if !scenarios.iter().any(|s| s.name.value == name) => {
            selector.error(E_NAME_0, "unknown scenario", model.span)
        }
        _ => (),
    }
    errors.extend(selector.errors);
    for s in scenarios {
        if !names.insert(&s.name.value) {
            let mut l = Lower::default();
            l.unsupported("duplicate scenario declaration", s.name.span);
            errors.extend(l.errors);
        }
        let mut effective = model.clone();
        effective
            .items
            .retain(|i| !matches!(i, ast::Item::Scenario(_)));
        let common: BTreeSet<_> = effective.items.iter().filter_map(scenario_field).collect();
        let mut local = BTreeSet::new();
        let mut horizons = Vec::new();
        for item in &s.items {
            let item = match item {
                ast::ScenarioItem::ArrivalsDistribution(a) => {
                    ast::Item::ArrivalsDistribution(a.clone())
                }
                ast::ScenarioItem::ArrivalsFixed(a) => ast::Item::ArrivalsFixed(a.clone()),
                ast::ScenarioItem::ArrivalsEvery(a) => ast::Item::ArrivalsEvery(a.clone()),
                ast::ScenarioItem::Policy(p) => ast::Item::Policy(p.clone()),
                ast::ScenarioItem::Queue(q) => ast::Item::Queue(q.clone()),
                ast::ScenarioItem::Concurrent(q) => ast::Item::Concurrent(q.clone()),
                ast::ScenarioItem::Bounds(b) => ast::Item::Bounds(b.clone()),
                ast::ScenarioItem::Simulation(v) => ast::Item::Simulation(v.clone()),
                ast::ScenarioItem::Horizon(d) => {
                    horizons.push(d.clone());
                    continue;
                }
            };
            let field = scenario_field(&item).expect("closed scenario item");
            let mut l = Lower::default();
            // U10: an overflow terminal that exists only under some scenarios
            // would make the model's verify and contract references depend on
            // the selection. The common item (ADR 0039) is not affected.
            if let ast::Item::Queue(q) | ast::Item::Concurrent(q) = &item
                && matches!(q.value.overflow, ast::Overflow::End(_))
            {
                l.unsupported("scenario overflow terminal is unsupported", q.span);
            }
            if common.contains(field) {
                l.unsupported(
                    format!(
                        "{field} declared both in model and scenario {}",
                        s.name.value
                    ),
                    s.span,
                );
            }
            if !local.insert(field) {
                l.unsupported(format!("duplicate {field} declaration"), s.span);
            }
            errors.extend(l.errors);
            effective.items.push(item);
        }
        match lower_effective(&effective, configuration, Some(s), &horizons) {
            Ok(m) if scenario == Some(s.name.value.as_str()) => selected = Some(m),
            Ok(_) => (),
            Err(ds) => errors.extend(ds),
        }
    }
    let mut unique = Vec::new();
    for d in errors {
        if !unique.contains(&d) {
            unique.push(d);
        }
    }
    if unique.is_empty() {
        Ok(selected.expect("validated scenario selection"))
    } else {
        Err(unique)
    }
}

fn scenario_field(item: &ast::Item) -> Option<&'static str> {
    match item {
        ast::Item::ArrivalsFixed(_)
        | ast::Item::ArrivalsEvery(_)
        | ast::Item::ArrivalsDistribution(_) => Some("arrivals"),
        ast::Item::Bounds(_) => Some("bounds"),
        ast::Item::Policy(_) => Some("policy"),
        ast::Item::Queue(_) => Some("queue"),
        ast::Item::Concurrent(_) => Some("concurrent"),
        ast::Item::Simulation(_) => Some("simulation"),
        _ => None,
    }
}

fn lower_effective(
    model: &ast::Model,
    configuration: Option<&str>,
    scenario: Option<&ast::ScenarioDecl>,
    horizons: &[ast::Duration],
) -> Result<ir::Model, Vec<Diagnostic>> {
    let mut l = Lower {
        scenario: scenario.map(|s| ir::ScenarioRecord {
            name: s.name.value.clone(),
            span: s.span,
        }),
        ..Default::default()
    };
    for d in horizons {
        if l.horizon.is_some() {
            l.unsupported("duplicate horizon declaration", d.span);
        }
        if let Some(t) = l.ticks(d) {
            if t == 0 {
                l.error(diag::E_ARR_0, "horizon must be positive", d.span);
            }
            l.horizon = Some(t);
        }
    }
    let mut effective = model.clone();
    for item in &mut effective.items {
        if let ast::Item::ArrivalsEvery(a) = item {
            let (Some(every), Some(from), Some(until)) = (
                l.ticks(&a.value.every),
                l.ticks(&a.value.from),
                l.ticks(&a.value.until),
            ) else {
                continue;
            };
            if every == 0 {
                l.error(
                    diag::E_ARR_0,
                    "fixed_every step must be positive",
                    a.value.every.span,
                );
                continue;
            }
            if from >= until {
                l.error(diag::E_ARR_0, "fixed_every interval is empty", a.span);
                continue;
            }
            let count = 1 + (until - from - 1) / every;
            if count > 10000 {
                l.unsupported("fixed_every exceeds 10000 cases", a.span);
                continue;
            }
            l.arrival_every = Some(ir::FixedEveryRecord {
                every,
                from,
                until,
                count,
            });
            let value = (0..count)
                .map(|i| ast::Duration {
                    amount: ast::Spanned {
                        value: from + i * every,
                        span: a.span,
                    },
                    unit: ast::Spanned {
                        value: "min".into(),
                        span: a.span,
                    },
                    span: a.span,
                })
                .collect();
            *item = ast::Item::ArrivalsFixed(ast::Spanned {
                value,
                span: a.span,
            });
        }
    }
    l.model(&effective, configuration)
}

/// Metadata for diff stays outside the executable single-process IR.
#[derive(Clone)]
pub struct SelectedConfiguration {
    pub model: ir::Model,
    /// Process-local lexical inventory, separate from the model-wide EndId table.
    pub terminal_names: BTreeSet<String>,
    /// Source reference responsible for simulation alias diagnostics.
    pub process_reference_span: Span,
    pub intent: Vec<ast::Intent>,
    pub actors: BTreeSet<String>,
    pub relations: BTreeSet<String>,
}

pub fn lower_configuration(
    ast: &ast::Model,
    name: &str,
) -> Result<SelectedConfiguration, Vec<Diagnostic>> {
    lower_configuration_with_scenario(ast, name, None)
}

pub fn lower_configuration_with_scenario(
    ast: &ast::Model,
    name: &str,
    scenario: Option<&str>,
) -> Result<SelectedConfiguration, Vec<Diagnostic>> {
    let model = lower_with_selection(ast, Some(name), scenario)?;
    // Successful validation guarantees unique, resolved selection metadata.
    let configuration = ast
        .items
        .iter()
        .find_map(|item| match item {
            ast::Item::Configuration(c) if c.name.value == name => Some(c),
            _ => None,
        })
        .expect("validated configuration");
    let process = ast
        .items
        .iter()
        .find_map(|item| match item {
            ast::Item::Process(p) if p.name.value == configuration.process.value => Some(p),
            _ => None,
        })
        .expect("validated process reference");
    let mut calls = Vec::new();
    collect_calls(&process.body, &mut calls);
    let mut terminal_names = BTreeSet::new();
    collect_comparison_ends(&process.body, &mut terminal_names);
    if let Some(end) = model.overflow_end {
        terminal_names.insert(model.ends[end.0].clone());
    }
    Ok(SelectedConfiguration {
        model,
        terminal_names,
        process_reference_span: configuration.process.span,
        intent: configuration.intent.clone(),
        actors: calls.iter().map(|c| c.actor.value.clone()).collect(),
        relations: calls
            .iter()
            .map(|c| format!("{}.{}", c.actor.value, c.capability.value))
            .collect(),
    })
}

fn collect_comparison_ends(block: &ast::Block, names: &mut BTreeSet<String>) {
    for statement in block {
        match &statement.value {
            ast::Stmt::Repeat { body, on_limit, .. } => {
                collect_comparison_ends(body, names);
                collect_comparison_ends(on_limit, names);
            }
            ast::Stmt::Again | ast::Stmt::Leave | ast::Stmt::Yield(_) => {}
            ast::Stmt::End(name) => {
                names.insert(name.value.clone());
            }
            ast::Stmt::Merge {
                then_body,
                else_body,
                ..
            }
            | ast::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                collect_comparison_ends(then_body, names);
                collect_comparison_ends(else_body, names);
            }
            ast::Stmt::Fork { join, .. } => {
                if let ast::Join::Any { if_none, .. } = &join.value {
                    names.insert(if_none.value.clone());
                }
            }
            ast::Stmt::Call(c) => {
                if let Some(body) = &c.recovery {
                    collect_comparison_ends(body, names);
                }
                if let Some(retry) = &c.retry {
                    names.insert(retry.exhausted.value.clone());
                }
            }
            ast::Stmt::Wait(_)
            | ast::Stmt::WaitUntil(_)
            | ast::Stmt::ResolveEffect { .. }
            | ast::Stmt::StopReceiptUnknown => {}
        }
    }
}

pub(crate) fn collect_calls<'a>(block: &'a ast::Block, calls: &mut Vec<&'a ast::Call>) {
    for statement in block {
        match &statement.value {
            ast::Stmt::Repeat { body, on_limit, .. } => {
                collect_calls(body, calls);
                collect_calls(on_limit, calls);
            }
            ast::Stmt::Again | ast::Stmt::Leave | ast::Stmt::Yield(_) => {}
            ast::Stmt::Call(call) => {
                calls.push(call);
                if let Some(body) = &call.recovery {
                    collect_calls(body, calls);
                }
            }
            ast::Stmt::Fork { branches, .. } => calls.extend(branches),
            ast::Stmt::Merge {
                then_body,
                else_body,
                ..
            }
            | ast::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                collect_calls(then_body, calls);
                collect_calls(else_body, calls);
            }
            ast::Stmt::End(_)
            | ast::Stmt::Wait(_)
            | ast::Stmt::WaitUntil(_)
            | ast::Stmt::ResolveEffect { .. }
            | ast::Stmt::StopReceiptUnknown => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_resolution_reports_each_section_6_2_fault_once() {
        let src = |cut: &str| {
            format!(
                "model M version \"0.2\" {{ actor A {{ capacity 1 }} actor B {{ capacity 1 }}
                 process P {{ end done }} {cut}
                 bounds {{ cases 1; live_tasks 1; clock 1min }} verify reachable end.done }}"
            )
        };
        let lowered = |cut: &str| lower(&crate::syntax::parse(&src(cut)).unwrap());
        let ok = lowered(
            "cut C { include [A]
               exclude B reason \"r\" loses \"l\"
               defer A.rates reason \"r\"
               reentry [contest B -> A.recheck, review cut quarterly] }",
        )
        .unwrap();
        let cut = ok.cut.expect("resolved cut");
        assert_eq!(cut.include, vec![ir::ActorId(0)]);
        assert_eq!(cut.exclude[0].target, ir::CutTarget::Actor(ir::ActorId(1)));
        assert_eq!(cut.defer[0].target, ir::CutTarget::Label("A.rates".into()));
        assert_eq!(
            cut.reentry,
            vec!["contest B -> A.recheck", "review cut quarterly"]
        );
        let codes: Vec<_> = ok.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, vec![diag::W_CUT_1], "defer without loses only");
        for (cut, code) in [
            (
                "cut C { include [A, Z] exclude B reason \"r\" loses \"l\" }",
                E_NAME_0,
            ),
            (
                "cut C { include [A, B] exclude B reason \"r\" loses \"l\" }",
                diag::E_CUT_1,
            ),
            ("cut C { include [A] }", diag::E_CUT_1),
            (
                "cut C { include [A] exclude B reason \"r\" loses \"l\" defer B.x reason \"r\" loses \"l\" }",
                diag::E_CUT_1,
            ),
            (
                "cut C { include [A, A] exclude B reason \"r\" loses \"l\" }",
                U_SEM_0,
            ),
            (
                "cut C { include [A, B] question \"q\" question \"q\" }",
                U_SEM_0,
            ),
            ("cut C { include [A, B] } cut D { include [A, B] }", U_SEM_0),
        ] {
            let errors = lowered(cut).unwrap_err();
            let found: Vec<_> = errors.iter().map(|d| d.code.as_str()).collect();
            assert_eq!(
                found.iter().filter(|c| **c == code).count(),
                1,
                "{cut}: {found:?}"
            );
            assert!(!found.contains(&diag::W_CUT_0), "{cut}: a cut was declared");
        }
        // Label exclusions do not satisfy membership of a declared Actor.
        let errors =
            lowered("cut C { include [A] exclude Bee reason \"r\" loses \"l\" }").unwrap_err();
        assert_eq!(errors[0].code, diag::E_CUT_1);
    }

    // Review N4: the excluded-call check walks every statement form, also
    // statically unreachable branches; the same calls to an included Actor pass.
    #[test]
    fn excluded_calls_are_found_in_forks_and_both_if_branches() {
        let src = |actor: &str, body: &str| {
            format!(
                "model M version \"0.2\" {{ type T = enum {{ pass }}
                 actor A {{ capacity 2; capability c() -> T {{ duration fixed 1min; result nondet {{ pass }} }}
                            capability d() -> T {{ duration fixed 1min; result nondet {{ pass }} }} }}
                 actor B {{ capacity 1; capability c() -> T {{ duration fixed 1min; result nondet {{ pass }} }} }}
                 process P {{ {} }}
                 cut C {{ include [A] exclude B reason \"r\" loses \"l\" }}
                 bounds {{ cases 1; live_tasks 2; clock 5min }} verify reachable end.done }}",
                body.replace("X", actor)
            )
        };
        let lowered = |src: String| lower(&crate::syntax::parse(&src).unwrap());
        for body in [
            "fork f { x = A.c()\n y = X.c() } join all f\n end done",
            "x = A.c()\n if x == pass { y = X.c()\n end done } else { end done }",
            "x = A.c()\n if x == pass { end done } else { y = X.c()\n end done }",
            "x = A.c()\n if x == pass { if x == pass { end done } else { y = X.c()\n end done } } else { end done }",
        ] {
            let errors = lowered(src("B", body)).unwrap_err();
            assert_eq!(errors[0].code, diag::E_CUT_1, "{body}: {errors:?}");
            let included = body.replace("X.c()", "A.d()");
            assert!(lowered(src("A", &included)).is_ok(), "{included}");
        }
    }

    #[test]
    fn other_language_versions_and_duplicate_verifies_are_unsupported() {
        let src = |version: &str, verify: &str| {
            format!(
                "model M version \"{version}\" {{ actor A {{ capacity 1 }} process P {{ end done }}
                 bounds {{ cases 1; live_tasks 1; clock 1min }} {verify} }}"
            )
        };
        let lowered = |src: String| lower(&crate::syntax::parse(&src).unwrap());
        assert!(lowered(src("0.2", "verify reachable end.done")).is_ok());
        assert!(
            lowered(src(
                "0.2",
                "verify reachable end.done verify safety no_resource_overcommit"
            ))
            .is_ok()
        );
        for (version, verify, code) in [
            ("9.9", "verify reachable end.done", diag::U_SYN_0),
            ("0.1", "verify reachable end.done", diag::U_SYN_0),
            (
                "0.2",
                "verify reachable end.done verify reachable end.done",
                U_SEM_0,
            ),
            (
                "0.2",
                "verify safety no_resource_overcommit verify safety no_resource_overcommit",
                U_SEM_0,
            ),
        ] {
            let errors = lowered(src(version, verify)).unwrap_err();
            assert_eq!(errors[0].code, code, "{version} {verify}");
            assert_eq!(errors.len(), 2, "{errors:?}"); // plus W-CUT-0
        }
    }

    const S: Span = Span { start: 4, end: 9 };

    fn sp<T>(value: T) -> ast::Spanned<T> {
        ast::Spanned { value, span: S }
    }
    fn id(value: &str) -> ast::Ident {
        sp(value.into())
    }
    fn duration(amount: u64) -> ast::Duration {
        ast::Duration {
            amount: sp(amount),
            unit: id("min"),
            span: S,
        }
    }
    fn call(binding: &str, capability: &str) -> ast::Call {
        ast::Call {
            recovery: None,
            query_command: None,
            response_loss: None,
            retry: None,
            args: Vec::new(),
            binding: id(binding),
            actor: id("Office"),
            capability: id(capability),
            span: S,
        }
    }
    fn cap(name: &str, minutes: u64, cancel: Option<ast::Cancel>) -> ast::Capability {
        let mut items = vec![
            sp(ast::CapItem::DurationFixed(duration(minutes))),
            sp(ast::CapItem::ResultNondet(vec![id("pass").into()])),
        ];
        if let Some(cancel) = cancel {
            items.push(sp(ast::CapItem::Cancel(cancel)));
        }
        ast::Capability {
            inline_result: None,
            params: Vec::new(),
            name: id(name),
            return_type: id("Verdict"),
            items,
            span: S,
        }
    }
    fn end(name: &str) -> ast::Spanned<ast::Stmt> {
        sp(ast::Stmt::End(id(name)))
    }
    fn eq(binding: &str, value: &str) -> ast::Spanned<ast::Cond> {
        sp(ast::Cond::Eq {
            binding: id(binding),
            value: id(value),
        })
    }
    fn branch(
        condition: ast::Spanned<ast::Cond>,
        then_body: ast::Block,
        else_body: ast::Block,
    ) -> ast::Spanned<ast::Stmt> {
        sp(ast::Stmt::If {
            condition,
            then_body,
            else_body,
        })
    }
    // Same declarations/control structure as the serial-time acceptance pair.
    fn serial() -> ast::Model {
        ast::Model {
            name: id("SerialTime"),
            version: sp("0.2".into()),
            span: S,
            items: vec![
                ast::Item::Type(ast::TypeDecl {
                    record: None,
                    name: id("Verdict"),
                    members: vec![id("pass")],
                    span: S,
                }),
                ast::Item::Actor(ast::Actor {
                    name: id("Office"),
                    span: S,
                    items: vec![
                        ast::ActorItem::Capacity(sp(2)),
                        ast::ActorItem::Capability(cap("short", 3, None)),
                        ast::ActorItem::Capability(cap("long", 5, None)),
                    ],
                }),
                ast::Item::Process(ast::Process {
                    degradations: Vec::new(),
                    name: id("Check"),
                    span: S,
                    body: vec![
                        sp(ast::Stmt::Call(call("a", "short"))),
                        sp(ast::Stmt::Call(call("b", "long"))),
                        end("done"),
                    ],
                }),
                ast::Item::Bounds(ast::Bounds {
                    span: S,
                    items: vec![
                        sp(ast::Bound::Cases(sp(1))),
                        sp(ast::Bound::LiveTasks(sp(2))),
                        sp(ast::Bound::Clock(duration(8))),
                    ],
                }),
                ast::Item::Verify(sp(ast::Property::ReachableEnd(id("done")))),
            ],
        }
    }
    fn actor(model: &mut ast::Model) -> &mut ast::Actor {
        model
            .items
            .iter_mut()
            .find_map(|i| {
                if let ast::Item::Actor(a) = i {
                    Some(a)
                } else {
                    None
                }
            })
            .unwrap()
    }
    fn capability<'a>(model: &'a mut ast::Model, name: &str) -> &'a mut ast::Capability {
        actor(model)
            .items
            .iter_mut()
            .find_map(|i| {
                if let ast::ActorItem::Capability(c) = i {
                    (c.name.value == name).then_some(c)
                } else {
                    None
                }
            })
            .unwrap()
    }
    fn process(model: &mut ast::Model) -> &mut ast::Process {
        model
            .items
            .iter_mut()
            .find_map(|i| {
                if let ast::Item::Process(p) = i {
                    Some(p)
                } else {
                    None
                }
            })
            .unwrap()
    }
    fn bounds(model: &mut ast::Model) -> &mut ast::Bounds {
        model
            .items
            .iter_mut()
            .find_map(|i| {
                if let ast::Item::Bounds(b) = i {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap()
    }
    fn expect_code(model: &ast::Model, code: &str) -> Vec<Diagnostic> {
        let errors = lower(model).expect_err("bad AST must not become executable IR");
        assert!(
            errors
                .iter()
                .any(|d| d.code == code && d.severity == diag::Severity::Error),
            "expected {code}, got {errors:?}"
        );
        assert_eq!(errors.iter().filter(|d| d.code == diag::W_CUT_0).count(), 1);
        assert!(
            errors
                .iter()
                .filter(|d| d.severity == diag::Severity::Error)
                .all(|d| d.span == Some(S))
        );
        errors
    }
    fn fork(name: &str, remaining: ast::Remaining, any: bool) -> ast::Spanned<ast::Stmt> {
        sp(ast::Stmt::Fork {
            name: id(name),
            branches: vec![call("a", "short"), call("b", "long")],
            join: sp(if any {
                ast::Join::Any {
                    fork: id(name),
                    result: id("pass"),
                    tie: sp(ast::Tie::By(vec![id("a"), id("b")])),
                    remaining: sp(remaining),
                    if_none: id("no_match"),
                }
            } else {
                ast::Join::All { fork: id(name) }
            }),
        })
    }
    fn any_model(cooperative: bool) -> ast::Model {
        let mut m = serial();
        for name in ["short", "long"] {
            capability(&mut m, name)
                .items
                .push(sp(ast::CapItem::Cancel(if cooperative {
                    ast::Cancel::Cooperative {
                        ack_within: duration(1),
                    }
                } else {
                    ast::Cancel::ForbiddenAfterStart
                })));
        }
        process(&mut m).body = vec![
            fork("checks", ast::Remaining::CancelThenDrain, true),
            branch(eq("a", "pass"), vec![end("done")], vec![end("no_match")]),
        ];
        m
    }
    fn any_join(model: &mut ast::Model) -> &mut ast::Join {
        if let ast::Stmt::Fork { join, .. } = &mut process(model).body[0].value {
            &mut join.value
        } else {
            panic!("expected fork")
        }
    }

    #[test]
    fn serial_ticks_resolved_indices_and_assumptions() {
        let m = lower(&serial()).unwrap();
        assert_eq!(
            m.capabilities
                .iter()
                .map(|c| c.duration)
                .collect::<Vec<_>>(),
            [3, 5]
        );
        assert_eq!(
            m.bounds,
            ir::Bounds {
                cases: 1,
                live_tasks: 2,
                clock: 8
            }
        );
        assert_eq!(
            m.process.bindings.iter().map(|b| b.ty).collect::<Vec<_>>(),
            [ir::TypeId(0), ir::TypeId(0)]
        );
        assert!(matches!(
            m.process.body[1].kind,
            ir::Stmt::Call(ir::Call {
                binding: ir::BindingId(1),
                capability: ir::CapabilityId(1),
                ..
            })
        ));
        assert_eq!(m.ends, ["done"]);
        assert_eq!(
            m.verify[0].property,
            ir::Property::ReachableEnd(ir::EndId(0))
        );
        assert_eq!(m.diagnostics.len(), 1);
        assert_eq!(m.diagnostics[0].code, diag::W_CUT_0);
        let assumptions: BTreeMap<_, _> = m
            .assumptions
            .iter()
            .map(|a| (a.field.as_str(), a.value.as_str()))
            .collect();
        assert_eq!(
            assumptions,
            BTreeMap::from([
                ("Office.boundary", "Office"),
                ("Office.executors", "anonymous×2"),
                ("Office.control", "policy"),
                ("Office.permission", "default(executors)"),
                ("schedule", "work-conserving")
            ])
        );
        assert!(m.assumptions.iter().all(|a| !a.affects.is_empty()));
    }

    #[test]
    fn declaration_order_does_not_control_resolution() {
        let mut m = serial();
        m.items.reverse();
        let m = lower(&m).unwrap();
        assert_eq!(m.capabilities[0].return_type, ir::TypeId(0));
        assert_eq!(m.capabilities[1].actor, ir::ActorId(0));
    }

    // Compare the complete diagnostic multiset, not just one requested code.
    // Spans travel with AST nodes when the declarations are permuted.
    fn reordered_pair(model: ast::Model, reorder: impl FnOnce(&mut ast::Model), required: &[&str]) {
        let before = lower(&model).unwrap_err();
        let mut reversed = model;
        reorder(&mut reversed);
        let after = lower(&reversed).unwrap_err();
        let canonical = |diagnostics: &[Diagnostic]| {
            let mut result: Vec<_> = diagnostics.iter().map(|d| format!("{d:?}")).collect();
            result.sort();
            result
        };
        assert_eq!(canonical(&before), canonical(&after));
        for code in required {
            assert!(
                before.iter().any(|d| d.code == *code),
                "missing {code}: {before:?}"
            );
        }
        let expected = if required.iter().any(|c| c.starts_with("E-")) {
            crate::Verdict::Invalid
        } else {
            crate::Verdict::Unsupported
        };
        for diagnostics in [&before, &after] {
            assert_eq!(crate::report::fold_verdict(&[], diagnostics), expected);
            assert_eq!(
                diagnostics
                    .iter()
                    .filter(|d| d.code == diag::W_CUT_0)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn duplicate_capability_checks_both_bodies_in_either_order() {
        let mut m = serial();
        process(&mut m).body = vec![end("done")];
        let mut bad = cap("c", 1, None);
        bad.return_type = id("Missing");
        bad.span = Span { start: 20, end: 40 };
        actor(&mut m).items = vec![
            ast::ActorItem::Capability(cap("c", 0, None)),
            ast::ActorItem::Capability(bad),
        ];
        reordered_pair(
            m,
            |m| actor(m).items.reverse(),
            &[U_SEM_0, E_NAME_0, E_CAP_0],
        );
    }

    #[test]
    fn review_counterexample_and_reverse_are_invalid_exit_three() {
        let good = "capability c() -> T { duration fixed 0min; result nondet { pass } }";
        let bad = "capability c() -> Missing { duration fixed 1min; result nondet { pass } }";
        for (first, second) in [(good, bad), (bad, good)] {
            let source = format!(
                "model M version \"0.2\" {{ type T = enum {{ pass }} actor A {{ {first}; {second} }}
                 process P {{ end done }} bounds {{ cases 1; live_tasks 1; clock 1min }}
                 verify reachable end.done }}"
            );
            let report = crate::report::check(&source);
            assert_eq!(report.verdict, crate::Verdict::Invalid);
            assert_eq!(report.verdict.exit_code(), 3);
            for code in [E_NAME_0, E_CAP_0, U_SEM_0] {
                assert!(
                    report.diagnostics.iter().any(|d| d.code == code),
                    "{report:?}"
                );
            }
        }
    }

    #[test]
    fn duplicate_actor_checks_own_capacity_and_all_fields_in_either_order() {
        let mut m = serial();
        let mut bad = cap("unused", 1, None);
        bad.return_type = id("Missing");
        m.items.push(ast::Item::Actor(ast::Actor {
            name: id("Office"),
            span: Span { start: 20, end: 40 },
            items: vec![ast::ActorItem::Capability(bad)],
        }));
        reordered_pair(m, |m| m.items.reverse(), &[U_SEM_0, E_NAME_0, E_CAP_0]);
    }

    #[test]
    fn duplicate_type_checks_members_and_definite_type_errors_in_either_order() {
        let mut m = serial();
        m.items.push(ast::Item::Type(ast::TypeDecl {
            record: None,
            name: id("Verdict"),
            members: vec![],
            span: Span { start: 20, end: 40 },
        }));
        capability(&mut m, "short").items[1] =
            sp(ast::CapItem::ResultNondet(vec![id("wrong").into()]));
        reordered_pair(m, |m| m.items.reverse(), &[U_SEM_0, E_TYPE_0]);
    }

    #[test]
    fn duplicate_types_do_not_invent_type_errors_from_first_candidate() {
        let mut m = serial();
        m.items.push(ast::Item::Type(ast::TypeDecl {
            record: None,
            name: id("Verdict"),
            members: vec![id("other")],
            span: S,
        }));
        process(&mut m)
            .body
            .insert(2, branch(eq("a", "pass"), vec![], vec![]));
        reordered_pair(m, |m| m.items.reverse(), &[U_SEM_0]);
    }

    #[test]
    fn duplicate_capability_still_checks_result_types_and_ambiguous_uses() {
        let mut m = serial();
        let mut bad = cap("short", 3, None);
        bad.items[1] = sp(ast::CapItem::ResultNondet(vec![id("wrong").into()]));
        actor(&mut m).items.push(ast::ActorItem::Capability(bad));
        process(&mut m)
            .body
            .insert(2, branch(eq("a", "wrong"), vec![], vec![]));
        reordered_pair(m, |m| actor(m).items.reverse(), &[U_SEM_0, E_TYPE_0]);
    }

    #[test]
    fn duplicate_binding_checks_every_call_before_skipping_registration() {
        let mut m = serial();
        process(&mut m).body = vec![
            sp(ast::Stmt::Call(call("a", "short"))),
            sp(ast::Stmt::Call(call("a", "absent"))),
            end("done"),
        ];
        for (i, stmt) in process(&mut m).body[..2].iter_mut().enumerate() {
            if let ast::Stmt::Call(call) = &mut stmt.value {
                call.binding.span = Span {
                    start: 20 + i * 10,
                    end: 21 + i * 10,
                };
            }
        }
        reordered_pair(m, |m| process(m).body.swap(0, 1), &[U_SEM_0, E_NAME_0]);

        let mut m = serial();
        process(&mut m).body = vec![
            sp(ast::Stmt::Call(call("a", "short"))),
            sp(ast::Stmt::Call(call("a", "long"))),
            branch(eq("a", "wrong"), vec![], vec![]),
            end("done"),
        ];
        reordered_pair(m, |m| process(m).body.swap(0, 1), &[U_SEM_0, E_TYPE_0]);
    }

    #[test]
    fn duplicate_binding_does_not_hide_zero_duration_call_diagnostic() {
        let mut m = serial();
        actor(&mut m).items = vec![ast::ActorItem::Capability(cap("short", 0, None))];
        process(&mut m).body = vec![
            sp(ast::Stmt::Call(call("a", "absent"))),
            sp(ast::Stmt::Call(call("a", "short"))),
            end("done"),
        ];
        reordered_pair(m, |m| process(m).body.swap(0, 1), &[U_SEM_0, E_NAME_0]);
    }

    #[test]
    fn duplicate_fork_and_binding_keep_branch_type_cancel_and_name_checks() {
        let mut m = serial();
        let mut bad = fork("checks", ast::Remaining::CancelThenDrain, true);
        if let ast::Stmt::Fork { join, .. } = &mut bad.value
            && let ast::Join::Any {
                result,
                tie,
                if_none,
                ..
            } = &mut join.value
        {
            *result = id("wrong");
            tie.value = ast::Tie::By(vec![id("missing")]);
            *if_none = id("absent");
        }
        process(&mut m).body = vec![
            fork("checks", ast::Remaining::Drain, false),
            bad,
            end("done"),
        ];
        reordered_pair(
            m,
            |m| process(m).body.swap(0, 1),
            &[U_SEM_0, E_TYPE_0, E_CANCEL_0, E_NAME_0],
        );
    }

    #[test]
    fn repeated_terminals_do_not_stop_later_static_checks() {
        let mut m = serial();
        process(&mut m).body = vec![
            end("done"),
            branch(eq("missing", "pass"), vec![end("done")], vec![end("done")]),
            sp(ast::Stmt::Call(call("a", "absent"))),
            end("done"),
        ];
        reordered_pair(m, |m| process(m).body.reverse(), &[E_NAME_0]);
    }

    #[test]
    fn multiple_processes_check_local_bindings_and_model_wide_terminals() {
        let mut m = serial();
        m.items.push(ast::Item::Process(ast::Process {
            degradations: Vec::new(),
            name: id("Second"),
            span: S,
            body: vec![
                sp(ast::Stmt::Call(call("a", "long"))),
                branch(eq("a", "wrong"), vec![], vec![]),
                end("other"),
            ],
        }));
        m.items
            .push(ast::Item::Verify(sp(ast::Property::ReachableEnd(id(
                "absent",
            )))));
        reordered_pair(m, |m| m.items.reverse(), &[U_SEM_0, E_TYPE_0, E_NAME_0]);
    }

    #[test]
    fn duplicate_fields_and_bounds_validate_every_value_in_either_order() {
        let mut m = serial();
        actor(&mut m).items.remove(0);
        capability(&mut m, "short").items.extend([
            sp(ast::CapItem::DurationFixed(duration(0))),
            sp(ast::CapItem::ResultNondet(vec![id("wrong").into()])),
        ]);
        bounds(&mut m).items.push(sp(ast::Bound::Cases(sp(2))));
        reordered_pair(
            m,
            |m| {
                capability(m, "short").items.reverse();
                bounds(m).items.reverse();
            },
            &[U_SEM_0, E_TYPE_0, E_CAP_0],
        );
    }

    #[test]
    fn duplicate_actor_fields_and_finite_members_are_not_skipped() {
        let mut m = serial();
        let mut duplicate = actor(&mut m).clone();
        duplicate.span = Span { start: 20, end: 40 };
        duplicate.items.push(ast::ActorItem::Capacity(sp(1)));
        m.items.push(ast::Item::Actor(duplicate));
        if let ast::Item::Type(ty) = &mut m.items[0] {
            ty.members.push(ast::Spanned {
                value: "pass".into(),
                span: Span { start: 50, end: 54 },
            });
        }
        reordered_pair(
            m,
            |m| {
                m.items.reverse();
                for item in &mut m.items {
                    match item {
                        ast::Item::Type(ty) => ty.members.reverse(),
                        ast::Item::Actor(a) => a.items.reverse(),
                        _ => {}
                    }
                }
            },
            &[U_SEM_0],
        );
    }

    #[test]
    fn unknown_condition_atom_does_not_short_circuit_other_atoms() {
        let mut m = serial();
        process(&mut m).body.insert(
            2,
            branch(
                sp(ast::Cond::And(vec![
                    eq("missing", "pass"),
                    eq("a", "wrong"),
                ])),
                vec![],
                vec![],
            ),
        );
        reordered_pair(
            m,
            |m| {
                if let ast::Stmt::If { condition, .. } = &mut process(m).body[2].value
                    && let ast::Cond::And(parts) = &mut condition.value
                {
                    parts.reverse();
                }
            },
            &[E_NAME_0, E_TYPE_0],
        );
    }

    #[test]
    fn terminal_history_summary_and_missing_bounds_do_not_hide_verify_errors() {
        let mut m = any_model(false);
        if let ast::Item::Type(ty) = &mut m.items[0] {
            ty.members.push(id("fail"));
        }
        for name in ["short", "long"] {
            capability(&mut m, name).items[1] = sp(ast::CapItem::ResultNondet(vec![
                id("pass").into(),
                id("fail").into(),
            ]));
        }
        if let ast::Join::Any { if_none, .. } = any_join(&mut m) {
            *if_none = id("done");
        }
        bounds(&mut m).items.clear();
        m.items
            .push(ast::Item::Verify(sp(ast::Property::ReachableEnd(id(
                "absent",
            )))));
        reordered_pair(m, |m| m.items.reverse(), &[U_SEM_0, E_NAME_0]);
    }

    #[test]
    fn missing_capacity_fixture_and_unused_capability_are_invalid() {
        let mut m = serial();
        actor(&mut m).items.remove(0);
        process(&mut m).body = vec![end("done")];
        let errors = expect_code(&m, E_CAP_0);
        assert_eq!(errors.iter().filter(|d| d.code == E_CAP_0).count(), 2);
    }

    #[test]
    fn missing_capacity_fixture_structure() {
        let mut m = serial();
        actor(&mut m).items.remove(0);
        expect_code(&m, E_CAP_0);
    }

    #[test]
    fn zero_capacity_is_valid_not_missing() {
        let mut m = serial();
        actor(&mut m).items[0] = ast::ActorItem::Capacity(sp(0));
        assert_eq!(lower(&m).unwrap().actors[0].capacity, Some(0));
    }

    #[test]
    fn parallel_and_resource_contention_all_resolve() {
        for capacity in [1, 2] {
            let mut m = serial();
            actor(&mut m).items[0] = ast::ActorItem::Capacity(sp(capacity));
            process(&mut m).body = vec![fork("checks", ast::Remaining::Drain, false), end("done")];
            let m = lower(&m).unwrap();
            assert_eq!(m.process.forks, ["checks"]);
            assert!(
                matches!(&m.process.body[0].kind, ir::Stmt::Fork { id: ir::ForkId(0), branches, join: ir::Join::All } if branches.len() == 2)
            );
        }
    }

    #[test]
    fn any_drain_and_cooperative_fixtures_resolve() {
        for cooperative in [false, true] {
            let m = lower(&any_model(cooperative)).unwrap();
            assert!(
                matches!(&m.process.body[0].kind, ir::Stmt::Fork { join: ir::Join::Any { matches, tie: ir::Tie::By(order), remaining: ir::Remaining::CancelThenDrain, if_none: ir::EndId(1) }, .. }
                if matches == &[ir::ResultMatch { binding: ir::BindingId(0), value: ir::ValueId(0) }, ir::ResultMatch { binding: ir::BindingId(1), value: ir::ValueId(0) }] && order == &[ir::BindingId(0), ir::BindingId(1)])
            );
        }
    }

    #[test]
    fn missing_cancel_fixture_is_invalid() {
        let mut m = any_model(false);
        capability(&mut m, "long")
            .items
            .retain(|i| !matches!(i.value, ast::CapItem::Cancel(_)));
        expect_code(&m, E_CANCEL_0);
    }

    #[test]
    fn cancel_is_required_even_for_sole_guaranteed_winner() {
        let mut m = any_model(false);
        capability(&mut m, "short")
            .items
            .retain(|i| !matches!(i.value, ast::CapItem::Cancel(_)));
        if let ast::Stmt::Fork { branches, join, .. } = &mut process(&mut m).body[0].value {
            branches.pop();
            if let ast::Join::Any { tie, .. } = &mut join.value {
                tie.value = ast::Tie::By(vec![id("a")]);
            }
        }
        expect_code(&m, E_CANCEL_0);
    }

    #[test]
    fn drain_does_not_require_cancel() {
        let mut m = any_model(false);
        for name in ["short", "long"] {
            capability(&mut m, name)
                .items
                .retain(|i| !matches!(i.value, ast::CapItem::Cancel(_)));
        }
        if let ast::Join::Any { remaining, .. } = any_join(&mut m) {
            remaining.value = ast::Remaining::Drain;
        }
        lower(&m).unwrap();
    }

    #[test]
    fn unknown_actor_capability_type_binding_and_terminal() {
        let mut m = serial();
        if let ast::Stmt::Call(c) = &mut process(&mut m).body[0].value {
            c.actor = id("Nobody");
        }
        expect_code(&m, E_NAME_0);
        let mut m = serial();
        if let ast::Stmt::Call(c) = &mut process(&mut m).body[0].value {
            c.capability = id("absent");
        }
        expect_code(&m, E_NAME_0);
        let mut m = serial();
        capability(&mut m, "short").return_type = id("Unknown");
        expect_code(&m, E_NAME_0);
        let mut m = serial();
        process(&mut m)
            .body
            .insert(2, branch(eq("missing", "pass"), vec![], vec![]));
        expect_code(&m, E_NAME_0);
        let mut m = serial();
        m.items
            .push(ast::Item::Verify(sp(ast::Property::ReachableEnd(id(
                "absent",
            )))));
        expect_code(&m, E_NAME_0);
    }

    #[test]
    fn unknown_fork_branch_and_none_terminal() {
        for fault in 0..3 {
            let mut m = any_model(false);
            if let ast::Join::Any {
                fork, tie, if_none, ..
            } = any_join(&mut m)
            {
                match fault {
                    0 => *fork = id("absent"),
                    1 => tie.value = ast::Tie::By(vec![id("absent"), id("b")]),
                    _ => *if_none = id("absent"),
                }
            }
            expect_code(&m, E_NAME_0);
        }
    }

    #[test]
    fn result_values_and_comparisons_are_type_checked() {
        let mut m = serial();
        capability(&mut m, "short").items[1] =
            sp(ast::CapItem::ResultNondet(vec![id("wrong").into()]));
        expect_code(&m, E_TYPE_0);
        let mut m = serial();
        process(&mut m)
            .body
            .insert(2, branch(eq("a", "wrong"), vec![], vec![]));
        expect_code(&m, E_TYPE_0);
        let mut m = any_model(false);
        if let ast::Join::Any { result, .. } = any_join(&mut m) {
            *result = id("wrong");
        }
        expect_code(&m, E_TYPE_0);
    }

    #[test]
    fn heterogeneous_any_resolves_values_per_return_type() {
        let mut m = any_model(false);
        m.items.push(ast::Item::Type(ast::TypeDecl {
            record: None,
            name: id("Other"),
            members: vec![id("unused"), id("pass")],
            span: S,
        }));
        capability(&mut m, "long").return_type = id("Other");
        let m = lower(&m).unwrap();
        assert_eq!(m.process.bindings[1].ty, ir::TypeId(1));
        if let ir::Stmt::Fork {
            join: ir::Join::Any { matches, .. },
            ..
        } = &m.process.body[0].kind
        {
            assert_eq!(matches[0].value, ir::ValueId(0));
            assert_eq!(matches[1].value, ir::ValueId(1));
        } else {
            panic!("expected any");
        }
    }

    #[test]
    fn heterogeneous_any_rejects_member_absent_from_one_type() {
        let mut m = any_model(false);
        m.items.push(ast::Item::Type(ast::TypeDecl {
            record: None,
            name: id("Other"),
            members: vec![id("other")],
            span: S,
        }));
        capability(&mut m, "long").return_type = id("Other");
        capability(&mut m, "long").items[1] =
            sp(ast::CapItem::ResultNondet(vec![id("other").into()]));
        expect_code(&m, E_TYPE_0);
    }

    #[test]
    fn partial_duplicate_and_foreign_tie_orders_rejected() {
        for order in [
            vec![id("a")],
            vec![id("a"), id("a")],
            vec![id("a"), id("b"), id("b")],
        ] {
            let mut m = any_model(false);
            if let ast::Join::Any { tie, .. } = any_join(&mut m) {
                tie.value = ast::Tie::By(order);
            }
            expect_code(&m, U_SEM_0);
        }
        let mut m = any_model(false);
        if let ast::Join::Any { tie, .. } = any_join(&mut m) {
            tie.value = ast::Tie::By(vec![id("b"), id("a")]);
        }
        let m = lower(&m).unwrap();
        assert!(
            matches!(&m.process.body[0].kind, ir::Stmt::Fork { join: ir::Join::Any { tie: ir::Tie::By(order), .. }, .. } if order == &[ir::BindingId(1), ir::BindingId(0)])
        );
    }

    #[test]
    fn nondeterministic_tie_is_preserved() {
        let mut m = any_model(false);
        if let ast::Join::Any { tie, .. } = any_join(&mut m) {
            tie.value = ast::Tie::Nondet;
        }
        assert!(matches!(
            &lower(&m).unwrap().process.body[0].kind,
            ir::Stmt::Fork {
                join: ir::Join::Any {
                    tie: ir::Tie::Nondet,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn unsupported_cases_and_units_at_every_duration_site() {
        for cases in [0, 2, u64::MAX] {
            let mut m = serial();
            bounds(&mut m).items[0] = sp(ast::Bound::Cases(sp(cases)));
            expect_code(&m, if cases == 0 { U_SEM_0 } else { diag::E_ARR_0 });
        }
        for location in 0..3 {
            let mut m = any_model(true);
            match location {
                0 => {
                    if let ast::CapItem::DurationFixed(d) =
                        &mut capability(&mut m, "short").items[0].value
                    {
                        d.unit = id("hour");
                    }
                }
                1 => {
                    if let ast::Bound::Clock(d) = &mut bounds(&mut m).items[2].value {
                        d.unit = id("s");
                    }
                }
                _ => {
                    if let ast::CapItem::Cancel(ast::Cancel::Cooperative { ack_within }) =
                        &mut capability(&mut m, "short").items[2].value
                    {
                        ack_within.unit = id("day");
                    }
                }
            }
            expect_code(&m, U_SEM_0);
        }
    }

    #[test]
    fn missing_mandatory_fields_never_gain_defaults() {
        for index in 0..3 {
            let mut m = serial();
            bounds(&mut m).items.remove(index);
            expect_code(&m, U_SEM_0);
        }
        for index in 0..2 {
            let mut m = serial();
            capability(&mut m, "short").items.remove(index);
            expect_code(&m, U_SEM_0);
        }
        let mut m = serial();
        m.items.retain(|i| !matches!(i, ast::Item::Bounds(_)));
        expect_code(&m, U_SEM_0);
        let mut m = serial();
        m.items.retain(|i| !matches!(i, ast::Item::Verify(_)));
        expect_code(&m, U_SEM_0);
    }

    #[test]
    fn duplicate_fields_and_declarations_rejected() {
        for fault in 0..8 {
            let mut m = serial();
            match fault {
                0 => {
                    let item = m.items[0].clone();
                    m.items.push(item);
                }
                1 => {
                    let item = m.items[1].clone();
                    m.items.push(item);
                }
                2 => actor(&mut m).items.push(ast::ActorItem::Capacity(sp(2))),
                3 => {
                    let c = capability(&mut m, "short").clone();
                    actor(&mut m).items.push(ast::ActorItem::Capability(c));
                }
                4 => {
                    let field = capability(&mut m, "short").items[0].clone();
                    capability(&mut m, "short").items.push(field);
                }
                5 => bounds(&mut m).items.push(sp(ast::Bound::Cases(sp(1)))),
                6 => {
                    let item = m.items[3].clone();
                    m.items.push(item);
                }
                _ => process(&mut m)
                    .body
                    .insert(1, sp(ast::Stmt::Call(call("a", "long")))),
            }
            expect_code(&m, U_SEM_0);
        }
    }

    #[test]
    fn empty_or_duplicate_finite_sets_rejected() {
        for members in [vec![], vec![id("pass"), id("pass")]] {
            let mut m = serial();
            if let ast::Item::Type(ty) = &mut m.items[0] {
                ty.members = members.clone();
            }
            expect_code(&m, U_SEM_0);
            let mut m = serial();
            capability(&mut m, "short").items[1] = sp(ast::CapItem::ResultNondet(
                members.into_iter().map(Into::into).collect(),
            ));
            expect_code(&m, U_SEM_0);
        }
    }

    #[test]
    fn zero_duration_and_ack_boundaries() {
        let mut m = serial();
        capability(&mut m, "short").items[0] = sp(ast::CapItem::DurationFixed(duration(0)));
        lower(&m).unwrap();
        let mut m = serial();
        for name in ["short", "long"] {
            capability(&mut m, name).items[0] = sp(ast::CapItem::DurationFixed(duration(0)));
        }
        actor(&mut m).items.remove(0);
        expect_code(&m, U_SEM_0);
        process(&mut m).body = vec![end("done")];
        let ir = lower(&m).unwrap();
        assert_eq!(ir.actors[0].capacity, None);
        assert!(!ir.assumptions.iter().any(|a| a.field == "Office.executors"));
        assert!(
            ir.assumptions
                .iter()
                .any(|a| a.field == "Office.control" && a.value == "environment")
        );
        let mut m = any_model(true);
        capability(&mut m, "short").items[2] = sp(ast::CapItem::Cancel(ast::Cancel::Cooperative {
            ack_within: duration(0),
        }));
        expect_code(&m, U_SEM_0);
    }

    #[test]
    fn zero_bounds_and_full_u64_minutes_are_not_cut_off_by_lower() {
        let mut m = serial();
        bounds(&mut m).items[1] = sp(ast::Bound::LiveTasks(sp(0)));
        bounds(&mut m).items[2] = sp(ast::Bound::Clock(duration(0)));
        capability(&mut m, "short").items[0] = sp(ast::CapItem::DurationFixed(duration(u64::MAX)));
        let ir = lower(&m).unwrap();
        assert_eq!(ir.capabilities[0].duration, u64::MAX);
        assert_eq!(ir.bounds.clock, 0);
        assert_eq!(ir.bounds.live_tasks, 0);
    }

    #[test]
    fn unavailable_results_and_type_errors_are_separate() {
        let mut m = any_model(true);
        process(&mut m).body[1] =
            branch(eq("b", "wrong"), vec![end("done")], vec![end("no_match")]);
        let errors = expect_code(&m, E_TYPE_0);
        assert!(errors.iter().any(|d| d.code == U_SEM_0));
        let mut m = serial();
        process(&mut m)
            .body
            .insert(0, branch(eq("a", "pass"), vec![], vec![]));
        expect_code(&m, U_SEM_0);
    }

    #[test]
    fn capacity_contention_does_not_imply_guaranteed_uncancelled_values() {
        let mut m = any_model(false);
        actor(&mut m).items[0] = ast::ActorItem::Capacity(sp(1));
        expect_code(&m, U_SEM_0);
    }

    #[test]
    fn merged_bindings_require_definition_on_all_continuing_paths() {
        let mut m = serial();
        if let ast::Item::Type(ty) = &mut m.items[0] {
            ty.members.push(id("fail"));
        }
        capability(&mut m, "short").items[1] = sp(ast::CapItem::ResultNondet(vec![
            id("pass").into(),
            id("fail").into(),
        ]));
        process(&mut m).body = vec![
            sp(ast::Stmt::Call(call("a", "short"))),
            branch(
                eq("a", "pass"),
                vec![sp(ast::Stmt::Call(call("b", "long")))],
                vec![],
            ),
            branch(eq("b", "pass"), vec![end("done")], vec![]),
        ];
        expect_code(&m, U_SEM_0);
        if let ast::Stmt::If { else_body, .. } = &mut process(&mut m).body[1].value {
            *else_body = vec![end("other")];
        }
        lower(&m).unwrap();
    }

    #[test]
    fn conditions_preserve_grouped_boolean_tree_and_check_all_atoms() {
        let mut m = serial();
        let cond = sp(ast::Cond::And(vec![
            eq("a", "pass"),
            sp(ast::Cond::Group(Box::new(sp(ast::Cond::Or(vec![
                eq("a", "pass"),
                eq("b", "pass"),
            ]))))),
        ]));
        process(&mut m).body.insert(2, branch(cond, vec![], vec![]));
        assert!(
            matches!(&lower(&m).unwrap().process.body[2].kind, ir::Stmt::If { condition: ir::Cond::And(parts), .. } if matches!(&parts[1], ir::Cond::Or(_)))
        );
        process(&mut m).body[2] = branch(
            sp(ast::Cond::And(vec![
                eq("undefined", "pass"),
                eq("a", "wrong"),
            ])),
            vec![],
            vec![],
        );
        let errors = expect_code(&m, E_NAME_0);
        assert!(errors.iter().any(|d| d.code == E_TYPE_0));
    }

    #[test]
    fn missing_and_multiple_processes_rejected_but_empty_process_is_not_an_end() {
        let mut m = serial();
        m.items.retain(|i| !matches!(i, ast::Item::Process(_)));
        expect_code(&m, U_SEM_0);
        let mut m = serial();
        let p = m.items[2].clone();
        m.items.push(p);
        expect_code(&m, U_SEM_0);
        let mut m = serial();
        process(&mut m).body.clear();
        m.items.pop();
        m.items
            .push(ast::Item::Verify(sp(ast::Property::NoResourceOvercommit)));
        let m = lower(&m).unwrap();
        assert!(m.ends.is_empty());
        assert!(m.process.body.is_empty());
    }

    #[test]
    fn multiple_any_adoptions_are_unsupported() {
        let mut m = any_model(false);
        let mut second = fork("second", ast::Remaining::Drain, true);
        if let ast::Stmt::Fork { branches, join, .. } = &mut second.value {
            branches[0].binding = id("c");
            branches[1].binding = id("d");
            if let ast::Join::Any { tie, .. } = &mut join.value {
                tie.value = ast::Tie::Nondet;
            }
        }
        process(&mut m).body.insert(1, second);
        expect_code(&m, U_SEM_0);
    }

    #[test]
    fn terminal_with_mixed_adoption_histories_is_unsupported() {
        let mut m = any_model(false);
        if let ast::Item::Type(ty) = &mut m.items[0] {
            ty.members.push(id("fail"));
        }
        for name in ["short", "long"] {
            capability(&mut m, name).items[1] = sp(ast::CapItem::ResultNondet(vec![
                id("pass").into(),
                id("fail").into(),
            ]));
        }
        if let ast::Join::Any { if_none, .. } = any_join(&mut m) {
            *if_none = id("done");
        }
        expect_code(&m, U_SEM_0);
    }

    #[test]
    fn unused_actor_defaults_are_environment_and_never_invent_capacity() {
        let mut m = serial();
        m.items.push(ast::Item::Actor(ast::Actor {
            name: id("Document"),
            items: vec![],
            span: S,
        }));
        let m = lower(&m).unwrap();
        assert_eq!(m.actors[1].capacity, None);
        assert!(
            m.assumptions
                .iter()
                .any(|a| a.field == "Document.control" && a.value == "environment")
        );
        assert!(
            !m.assumptions
                .iter()
                .any(|a| a.field == "Document.executors")
        );
    }
}

#[derive(Default)]
struct Lower {
    merge_names: BTreeSet<String>,
    merge_target: Option<(ir::BindingId, usize)>,
    block_depth: usize,
    unstable_merge_sources: BTreeSet<String>,
    loop_variables: BTreeMap<String, ir::StateVarId>,
    loop_context: Option<repetition::LoopContext>,
    performances: Vec<ir::Performance>,
    response_caps: BTreeMap<ir::CapabilityId, (Option<ast::Ident>, bool)>,
    scenario: Option<ir::ScenarioRecord>,
    arrival_every: Option<ir::FixedEveryRecord>,
    horizon: Option<ir::Tick>,
    total_cases: u64,
    cap_context: Option<(ir::ActorId, Vec<ir::Parameter>)>,
    rejected_rule_writes: BTreeSet<ir::StateVarId>,
    builtin_person: Option<ir::TypeId>,
    admitted_allowed: bool,
    resources: Vec<ir::Resource>,
    resource_names: BTreeMap<String, ir::ResourceId>,
    multi_case: bool,
    kernels: Vec<ir::Kernel>,
    kernel_declarations: BTreeMap<String, ast::KernelDecl>,
    initial_distributions: Vec<Vec<(ir::StateValue, crate::probability::Rational)>>,
    probability_queries: Vec<ir::ProbabilityQuery>,
    state_variables: Vec<ir::StateVariable>,
    state_names: BTreeMap<String, ir::StateVarId>,
    local_names: BTreeMap<(ir::ActorId, String), ir::StateVarId>,
    builtin_bool: Option<ir::TypeId>,
    inline_results: BTreeMap<usize, ir::TypeId>,
    /// The written position of an inline enum, for messages (its IR name is
    /// internal and never shown).
    inline_names: BTreeMap<ir::TypeId, String>,
    record_parameters: BTreeSet<String>,
    record_binding_names: BTreeSet<String>,
    people: Vec<ir::Person>,
    person_names: BTreeMap<String, ir::PersonId>,
    authorities: Vec<ir::Authority>,
    authority_names: BTreeMap<String, ir::AuthorityId>,
    global_bindings: BTreeSet<String>,
    call_before: BTreeMap<ir::BindingId, BTreeSet<ir::BindingId>>,
    end_before: BTreeMap<ir::EndId, BTreeSet<ir::BindingId>>,
    route_before: BTreeMap<ir::CapabilityId, BTreeSet<ir::BindingId>>,
    errors: Vec<Diagnostic>,
    types: Vec<ir::EnumType>,
    type_names: BTreeMap<String, ir::TypeId>,
    actors: Vec<ir::Actor>,
    actor_names: BTreeMap<String, ir::ActorId>,
    capabilities: Vec<ir::Capability>,
    cap_names: BTreeMap<(String, String), ir::CapabilityId>,
    ambiguous_actors: BTreeSet<String>,
    ambiguous_caps: BTreeSet<(String, String)>,
    ambiguous_bindings: BTreeSet<String>,
    known_cap_types: BTreeSet<ir::CapabilityId>,
    unknown_param_types: BTreeSet<(ir::CapabilityId, usize)>,
    uncertain_durations: BTreeSet<ir::CapabilityId>,
    binding_types: BTreeMap<String, BTreeSet<ir::TypeId>>,
    unknown_binding_types: BTreeSet<String>,
    check_flow: bool,
    in_recovery_handler: bool,
    bindings: Vec<ir::Binding>,
    binding_names: BTreeMap<String, ir::BindingId>,
    calls: BTreeMap<String, ir::Call>,
    forks: Vec<String>,
    fork_names: BTreeMap<String, ir::ForkId>,
    fork_spans: BTreeMap<String, Span>,
    ambiguous_forks: BTreeSet<String>,
    ends: Vec<String>,
    end_names: BTreeMap<String, ir::EndId>,
    terminal_histories: BTreeMap<ir::EndId, BTreeSet<bool>>,
    called_actors: BTreeSet<ir::ActorId>,
}

mod degradation;
mod repetition;
mod work_profiles;

#[cfg(test)]
mod resource_proof_tests {
    use super::*;
    #[test]
    fn resource_k18_all_start_counts_repeated_occurrences_and_proof_overflow_is_not_runtime_abort()
    {
        let src = include_str!("../../../fixtures/rsrc-all-start/model.relca");
        let m = lower(&crate::syntax::parse(src).unwrap()).unwrap();
        let ir::Stmt::Fork { branches, .. } = &m.process.body[0].kind else {
            panic!("fork");
        };
        let mut calls = vec![branches[1].clone(), branches[1].clone()];
        calls[0].binding = ir::BindingId(0);
        calls[1].binding = ir::BindingId(1);
        let matches = vec![
            ir::ResultMatch {
                binding: ir::BindingId(0),
                value: ir::ValueId(0),
            },
            ir::ResultMatch {
                binding: ir::BindingId(1),
                value: ir::ValueId(0),
            },
        ];
        for (capacity, quantity, expected) in
            [(2, 1, true), (1, 1, false), (u64::MAX, u64::MAX, false)]
        {
            let mut l = Lower {
                actors: m.actors.clone(),
                capabilities: m.capabilities.clone(),
                resources: m.resources.clone(),
                ..Lower::default()
            };
            l.resources[0].capacity = capacity;
            l.capabilities[1].uses[0].amount = quantity;
            let mut f = Flow::default();
            l.available_after_cancel(&calls, &matches, &mut f);
            assert_eq!(f.available.contains(&ir::BindingId(1)), expected);
            assert!(l.errors.is_empty());
        }
    }
}

#[cfg(test)]
mod executor_lower_tests {
    use super::*;

    fn model() -> ast::Model {
        crate::syntax::parse(r#"model M version "0.2" {
            people [p]; type T = enum { ok }
            actor A { capacity 1; executors [p]; capability a() -> T { duration fixed 1min; result nondet { ok } } }
            permit p to A.a
            authority G { basis "test"; grants p to A.a }
            process P { end done }
            bounds { cases 1; clock 2min; live_tasks 1 }
            contract { permitted A.a }
        }"#).unwrap()
    }

    #[test]
    fn empty_hand_ast_sets_and_missing_authority_items_are_not_defaulted() {
        for case in 0..7 {
            let mut m = model();
            for item in &mut m.items {
                match item {
                    ast::Item::People(ps) if case == 0 => ps.value.clear(),
                    ast::Item::Actor(a) if case == 1 => {
                        for field in &mut a.items {
                            if let ast::ActorItem::Executors(ps) = field {
                                ps.value.clear();
                            }
                        }
                    }
                    ast::Item::Permit(p) if case == 2 => p.persons.clear(),
                    ast::Item::Authority(a) => {
                        if case == 3 {
                            for field in &mut a.items {
                                if let ast::AuthorityItem::Grants { persons, .. } = &mut field.value
                                {
                                    persons.clear();
                                }
                            }
                        }
                        if case == 4 {
                            a.items
                                .retain(|i| !matches!(i.value, ast::AuthorityItem::Basis(_)));
                        }
                        if case == 5 {
                            a.items
                                .retain(|i| !matches!(i.value, ast::AuthorityItem::Grants { .. }));
                        }
                        if case == 6 {
                            a.items.push(ast::Spanned {
                                value: ast::AuthorityItem::NotBy(Vec::new()),
                                span: a.span,
                            });
                        }
                    }
                    _ => {}
                }
            }
            let errors = lower(&m).unwrap_err();
            let code = if case == 4 || case == 5 {
                diag::E_SYN_0
            } else {
                U_SEM_0
            };
            assert!(
                errors.iter().any(|e| e.code == code),
                "case {case}: {errors:?}"
            );
        }
    }

    #[test]
    fn unresolved_person_does_not_invent_capacity_mismatch() {
        let mut m = model();
        for item in &mut m.items {
            if let ast::Item::Actor(a) = item {
                for field in &mut a.items {
                    if let ast::ActorItem::Executors(ps) = field {
                        ps.value[0].value = "missing".into();
                    }
                }
            }
        }
        let errors = lower(&m).unwrap_err();
        assert!(errors.iter().any(|e| e.code == E_NAME_0));
        assert!(!errors.iter().any(|e| e.code == E_CAP_0));
    }
}

#[cfg(test)]
mod actor_observed_boundaries {
    fn errors(src: &str) -> Vec<crate::diag::Diagnostic> {
        super::lower(&crate::syntax::parse(src).unwrap())
            .unwrap_err()
            .into_iter()
            .filter(|d| d.severity == crate::diag::Severity::Error)
            .collect()
    }

    #[test]
    fn actor_state_does_not_unlock_self_qualified_latent_kernel_writes() {
        let src = "model M version \"0.2\" {type R=enum{ok}; kernel K{* -> {1:ok with hidden=ok}} actor A{capacity 1;state visible:bool=true;latent hidden:R=ok;capability run()->R{writes hidden;duration fixed 1min;outcomes kernel K}} process P{a=A.run();end done} bounds{cases 1;live_tasks 1;clock 3min} verify reachable end.done}";
        assert_eq!(crate::report::check(src).verdict, crate::Verdict::Satisfied);
        let qualified = src.replace("writes hidden", "writes A.hidden");
        assert_ne!(src, qualified);
        let ds = errors(&qualified);
        assert!(!ds.is_empty());
        assert!(ds.iter().all(|d| d.code == crate::diag::U_SYN_0), "{ds:?}");
    }

    #[test]
    fn unused_bare_self_requires_cannot_read_an_excluded_owner() {
        let src = "model M version \"0.2\" {type R=enum{ok,no}; actor A {capacity 1; state n:0..1=0; capability run()->R {requires n == 0 else no; duration fixed 1min; result nondet{ok}}} process P{end done} cut C{exclude A reason \"out\" loses \"state\"} bounds{cases 1;live_tasks 1;clock 5min} verify safety no_resource_overcommit}";
        let ds = errors(src);
        assert_eq!(ds.len(), 1);
        assert_eq!(ds[0].code, crate::diag::E_CUT_1);
    }

    #[test]
    fn independent_kernel_result_still_requires_the_actor_target_type() {
        for declaration in ["state flag:bool=false", "state flag:0..1=0"] {
            let src = format!(
                "model M version \"0.2\" {{type R=enum{{ok}};kernel K{{* -> {{1:ok}}}} actor A{{capacity 1;{declaration};capability run()->R{{writes flag;duration fixed 1min;outcomes kernel K}}}} process P{{a=A.run();end done}} bounds{{cases 1;live_tasks 1;clock 5min}} verify safety no_resource_overcommit}}"
            );
            let ds = errors(&src);
            assert!(!ds.is_empty());
            assert!(ds.iter().all(|d| d.code == crate::diag::E_TYPE_0), "{ds:?}");
            let constant = src.replace("writes flag", "writes flag;finish {flag=0}");
            if declaration.contains("0..1") {
                assert!(super::lower(&crate::syntax::parse(&constant).unwrap()).is_ok());
            }
        }
    }

    #[test]
    fn noninteger_qualified_operators_are_unsupported_in_every_policy_context() {
        for guard in [
            "process P {if A.x != ok {end done} else {end other}}",
            "actor B {capacity 1; capability run()->R {requires A.x != ok else no; duration fixed 1min; result nondet {ok}}} process P {a=B.run(); end done}",
            "people [p]; actor B {executors [p]; capability run()->R {duration fixed 1min; result nondet {ok}}} authority G {basis \"grant\"; grants p to B.run; scope A.x != ok} process P {a=B.run(); end done}",
        ] {
            let src = format!(
                "model M version \"0.2\" {{type R=enum{{ok,no}};actor A{{latent x:R=ok}} {guard} bounds{{cases 1;live_tasks 1;clock 5min}} verify safety no_resource_overcommit}}"
            );
            let ds = errors(&src);
            assert!(!ds.is_empty());
            assert!(ds.iter().all(|d| d.code == crate::diag::U_SYN_0), "{ds:?}");
        }
    }

    #[test]
    fn duplicate_observed_latent_namespace_does_not_depend_on_declaration_order() {
        for declarations in ["state x:R=ok; latent x:R=ok", "latent x:R=ok; state x:R=ok"] {
            let src = format!(
                "model M version \"0.2\" {{type R=enum{{ok}};actor A{{capacity 1; {declarations}; capability run()->R{{writes x;duration fixed 1min;result nondet{{ok}}}}}} process P{{a=A.run();end done}} bounds{{cases 1;live_tasks 1;clock 5min}} verify safety no_resource_overcommit}}"
            );
            let ds = errors(&src);
            assert!(!ds.is_empty());
            assert!(ds.iter().all(|d| d.code == crate::diag::U_SEM_0), "{ds:?}");
        }
    }

    #[test]
    fn bare_self_integer_rhs_is_not_a_constant_even_for_equality() {
        for operator in ["==", "<"] {
            let src = format!(
                "model M version \"0.2\" {{type R=enum{{ok,no}};actor A{{capacity 1;state n:0..1=0;capability run()->R{{requires n {operator} n else no;duration fixed 1min;result nondet{{ok}}}}}} process P{{a=A.run();end done}} bounds{{cases 1;live_tasks 1;clock 5min}} verify safety no_resource_overcommit}}"
            );
            let ds = errors(&src);
            assert!(!ds.is_empty());
            assert!(ds.iter().all(|d| d.code == crate::diag::U_SYN_0), "{ds:?}");
        }
    }
}

/// The written spelling of a target: duplicates by name are refused even when
/// the name does not resolve (the pre-ADR 0032 rule); resolved aliases are
/// compared by id beside it.
fn target_name(t: &ast::StateTarget) -> (Option<String>, String) {
    match t {
        ast::StateTarget::Unqualified(n) => (None, n.value.clone()),
        ast::StateTarget::Qualified {
            actor, variable, ..
        } => (Some(actor.value.clone()), variable.value.clone()),
    }
}

#[derive(Clone, Default)]
struct Flow {
    available: BTreeSet<ir::BindingId>,
    // Empty means control has terminated; bool means an any has adopted a value.
    histories: BTreeSet<bool>,
}

/// An inactive event invariant may refer to another process's binding without
/// making it a value in this process. Process flow and state-only expressions
/// must never inherit that legacy inactive-contract exception.
enum RecordContext<'a> {
    Process(Option<&'a BTreeSet<ir::BindingId>>),
    Contract(Option<&'a BTreeSet<ir::BindingId>>),
    StateOnly,
}

impl Lower {
    fn bool_type(&mut self, span: Span) -> ir::TypeId {
        if self.type_names.contains_key("bool") {
            self.unsupported("builtin bool conflicts with declared bool", span);
        }
        if let Some(t) = self.builtin_bool {
            return t;
        }
        let t = ir::TypeId(self.types.len());
        self.types.push(ir::EnumType {
            name: "bool".into(),
            members: vec!["true".into(), "false".into()],
            record: None,
            span,
        });
        self.builtin_bool = Some(t);
        t
    }

    fn is_record(&self, ty: ir::TypeId) -> bool {
        self.types.get(ty.0).is_some_and(|t| t.record.is_some())
    }

    fn literal_value(&mut self, ty: ir::TypeId, expr: &ast::ValueExpr) -> Option<ir::ValueId> {
        let Some(fields) = &expr.fields else {
            return self.value(ty, expr);
        };
        let Some(&actual) = self.type_names.get(&expr.head.value) else {
            self.error(E_NAME_0, "undefined record type", expr.head.span);
            return None;
        };
        if actual != ty || !self.is_record(actual) {
            self.error(E_TYPE_0, "record value has different type", expr.head.span);
            return None;
        }
        let shape = self.types[ty.0].record.clone().unwrap();
        let mut supplied = BTreeMap::new();
        for (name, value) in fields {
            if !shape.fields.iter().any(|f| f.name == name.value) {
                self.error(E_TYPE_0, "record value unknown field", name.span);
                return None;
            }
            if supplied.insert(&name.value, value).is_some() {
                self.error(E_TYPE_0, "record value duplicate field", name.span);
                return None;
            }
        }
        let mut index = 0usize;
        for f in &shape.fields {
            let Some(value) = supplied.get(&f.name) else {
                self.error(E_TYPE_0, "record value missing field", expr.span);
                return None;
            };
            let v = self.value(f.ty, value)?;
            index = index * self.types[f.ty.0].members.len() + v.0;
        }
        Some(ir::ValueId(index))
    }

    fn state_expr(
        &mut self,
        base: ir::TypeId,
        optional: bool,
        expr: &ast::ValueExpr,
    ) -> Option<ir::StateValue> {
        if expr.fields.is_some() {
            self.literal_value(base, expr).map(ir::StateValue::Value)
        } else {
            self.state_value(base, optional, expr)
        }
    }

    fn record_types(&mut self, model: &ast::Model) {
        for item in &model.items {
            match item {
                ast::Item::Type(d) => {
                    let Some(decls) = &d.record else {
                        continue;
                    };
                    let id = self.type_names[&d.name.value];
                    let mut fields = Vec::new();
                    let mut seen = BTreeSet::new();
                    let before = self.errors.len();
                    for f in decls {
                        let ty = if let Some(members) = &f.inline {
                            self.unique_members(members, f.ty.span);
                            let ty = ir::TypeId(self.types.len());
                            let written: Vec<_> = f
                                .names
                                .iter()
                                .map(|n| format!("{}.{}", d.name.value, n.value))
                                .collect();
                            self.inline_names.insert(
                                ty,
                                format!(
                                    "inline enum of field{} {}",
                                    if written.len() > 1 { "s" } else { "" },
                                    written.join(", ")
                                ),
                            );
                            self.types.push(ir::EnumType {
                                name: format!(
                                    "@inline:field:{}:{}",
                                    d.name.value,
                                    f.names.first().map_or("", |n| n.value.as_str())
                                ),
                                members: members.iter().map(|n| n.value.clone()).collect(),
                                record: None,
                                span: f.ty.span,
                            });
                            Some(ty)
                        } else if f.ty.value == "bool" {
                            Some(self.bool_type(f.ty.span))
                        } else {
                            self.type_names.get(&f.ty.value).copied()
                        };
                        let Some(ty) = ty else {
                            self.error(E_NAME_0, "undefined record field type", f.ty.span);
                            continue;
                        };
                        if self.is_record(ty)
                            || matches!(f.ty.value.as_str(), "PersonId" | "CaseId")
                        {
                            self.error(
                                diag::U_SYN_0,
                                "record field type is unsupported",
                                f.ty.span,
                            );
                            continue;
                        }
                        for name in &f.names {
                            if !seen.insert(name.value.clone()) {
                                self.unsupported("duplicate record field", name.span);
                            }
                            fields.push(ir::RecordField {
                                name: name.value.clone(),
                                ty,
                            });
                        }
                    }
                    if fields.is_empty() && self.errors.len() == before {
                        self.unsupported("record requires a field", d.span);
                    }
                    let size = fields
                        .iter()
                        .try_fold(1usize, |n, f| {
                            n.checked_mul(self.types[f.ty.0].members.len())
                        })
                        .filter(|n| *n > 0 && *n <= 1024);
                    if size.is_none() {
                        self.unsupported("record domain exceeds the supported size", d.span);
                    }
                    if self.errors.len() != before {
                        continue;
                    }
                    let mut members = Vec::new();
                    for mut index in 0..size.unwrap() {
                        let mut values = Vec::new();
                        for f in fields.iter().rev() {
                            let domain = &self.types[f.ty.0].members;
                            values.push(format!("{} = {}", f.name, domain[index % domain.len()]));
                            index /= domain.len();
                        }
                        values.reverse();
                        members.push(format!("{} {{ {} }}", d.name.value, values.join(", ")));
                    }
                    self.types[id.0].members = members;
                    self.types[id.0].record = Some(ir::RecordShape { fields });
                }
                ast::Item::Actor(a) => {
                    for item in &a.items {
                        if let ast::ActorItem::Capability(c) = item {
                            if let Some(members) = &c.inline_result {
                                self.unique_members(members, c.return_type.span);
                                let ty = ir::TypeId(self.types.len());
                                self.inline_names.insert(
                                    ty,
                                    format!(
                                        "inline enum of result {}.{}",
                                        a.name.value, c.name.value
                                    ),
                                );
                                self.types.push(ir::EnumType {
                                    name: format!(
                                        "@inline:result:{}.{}",
                                        a.name.value, c.name.value
                                    ),
                                    members: members.iter().map(|n| n.value.clone()).collect(),
                                    record: None,
                                    span: c.return_type.span,
                                });
                                self.inline_results.insert(c.return_type.span.start, ty);
                            }
                            for p in &c.params {
                                if self
                                    .type_names
                                    .get(&p.ty.value)
                                    .is_some_and(|t| self.is_record(*t))
                                {
                                    self.record_parameters.insert(p.name.value.clone());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn state_decl(&mut self, d: &ast::VarDecl, kind: ir::StateKind) {
        if !d.last_write_wins.is_empty()
            && !matches!(kind, ir::StateKind::ActorObserved(a) if !self.actors[a.0].is_per_case())
        {
            self.unsupported(
                "last_write_wins outside shared Actor state is unsupported",
                d.span,
            );
        }
        for span in d.last_write_wins.iter().skip(1) {
            self.unsupported("duplicate last_write_wins", *span);
        }
        if d.case_set {
            match kind {
                ir::StateKind::ActorObserved(a) if !self.actors[a.0].is_per_case() => {}
                ir::StateKind::ActorObserved(_) => {
                    self.unsupported("set outside shared Actor state is unsupported", d.ty.span)
                }
                _ => self.error(
                    diag::U_SYN_0,
                    "set outside shared Actor state is unsupported",
                    d.ty.span,
                ),
            }
            if self.total_cases > 64 {
                self.unsupported("set element count exceeds the supported size", d.ty.span);
            }
        }
        if d.ty.value == "PersonId" && kind.is_latent() {
            self.error(
                diag::U_SYN_0,
                "PersonId latent state is unsupported",
                d.ty.span,
            );
            return;
        }
        if self
            .type_names
            .get(&d.ty.value)
            .is_some_and(|t| self.is_record(*t))
        {
            if kind.is_latent() {
                self.error(diag::U_SYN_0, "latent record is unsupported", d.ty.span);
                return;
            }
            if matches!(kind, ir::StateKind::ActorObserved(_)) {
                self.unsupported("Actor record state is unsupported", d.ty.span);
                return;
            }
            if d.distribution.is_some() {
                self.error(diag::U_SYN_0, "record distribution is unsupported", d.span);
                return;
            }
        }
        if d.integer_max.is_some() && (d.optional || matches!(kind, ir::StateKind::ActorLatent(_)))
        {
            self.error(
                diag::U_SYN_0,
                "bounded integer at unsupported state location",
                d.ty.span,
            );
        }
        let base = if d.case_set {
            Some(ir::CASE_ID_TYPE)
        } else if d.ty.value == "CaseId" {
            self.error(diag::U_SYN_0, "CaseId state type is unsupported", d.ty.span);
            Some(ir::CASE_ID_TYPE)
        } else if d.ty.value == "PersonId" {
            let t = if let Some(t) = self.builtin_person {
                t
            } else {
                let t = ir::TypeId(self.types.len());
                self.types.push(ir::EnumType {
                    name: "PersonId".into(),
                    members: self.people.iter().map(|p| p.name.clone()).collect(),
                    span: d.ty.span,
                    record: None,
                });
                self.builtin_person = Some(t);
                t
            };
            if self.people.is_empty() && !d.optional {
                self.unsupported("PersonId requires people", d.ty.span);
            }
            Some(t)
        } else if d.integer_max.is_some() {
            Some(ir::TypeId(usize::MAX))
        } else if d.ty.value == "bool" {
            if self.type_names.contains_key("bool") {
                self.unsupported("builtin bool conflicts with declared bool", d.ty.span);
            }
            if let Some(t) = self.builtin_bool {
                Some(t)
            } else {
                let t = ir::TypeId(self.types.len());
                self.types.push(ir::EnumType {
                    record: None,
                    name: "bool".into(),
                    members: vec!["true".into(), "false".into()],
                    span: d.ty.span,
                });
                self.builtin_bool = Some(t);
                Some(t)
            }
        } else {
            self.type_names.get(&d.ty.value).copied()
        };
        if base.is_none() {
            self.error(
                if matches!(
                    d.ty.value.as_str(),
                    "PersonId" | "record" | "int" | "CaseId"
                ) {
                    diag::U_SYN_0
                } else {
                    E_NAME_0
                },
                match d.ty.value.as_str() {
                    "int" => "unbounded int state type is unsupported",
                    "CaseId" => "CaseId state type is unsupported",
                    _ => "undefined state type",
                },
                d.ty.span,
            );
        }
        let observed = kind.is_observed();
        if !observed && d.optional {
            self.unsupported("latent optional is unsupported", d.span);
        }
        if d.names.is_empty() {
            self.unsupported("empty state declaration", d.span);
        }
        for name in &d.names {
            let id = ir::StateVarId(self.state_variables.len());
            let duplicate = match kind {
                ir::StateKind::ActorLatent(a)
                | ir::StateKind::ActorObserved(a)
                | ir::StateKind::Rule(a) => {
                    let key = (a, name.value.clone());
                    if let Some(previous) = self.local_names.get(&key)
                        && (kind.is_observed()
                            || self.state_variables[previous.0].kind.is_observed())
                    {
                        if kind.is_observed() {
                            self.local_names.insert(key, id);
                        }
                        true
                    } else {
                        self.local_names.insert(key, id).is_some()
                    }
                }
                _ => self.state_names.insert(name.value.clone(), id).is_some(),
            };
            if duplicate {
                self.unsupported("duplicate state variable", name.span);
            }
            let base = base.unwrap_or(ir::TypeId(usize::MAX));
            let initial = if d.case_set {
                if d.default.is_some() || d.distribution.is_some() {
                    self.error(diag::U_SYN_0, "unsupported set initializer", d.span);
                }
                ir::InitialValue::Fixed(ir::StateValue::Set(0))
            } else if d.distribution.is_some() && matches!(kind, ir::StateKind::ActorObserved(_)) {
                self.unsupported("distribution requires latent without default", d.span);
                ir::InitialValue::Fixed(ir::StateValue::None)
            } else if let Some(entries) = &d.distribution {
                if observed || d.default.is_some() {
                    self.unsupported("distribution requires latent without default", d.span);
                }
                let mut distribution = Vec::new();
                let mut seen = BTreeSet::new();
                let mut sum = crate::probability::Rational::zero();
                for (name, literal) in entries {
                    let value = self.initial_scalar(base, d.optional, d.integer_max, name);
                    let p = self.probability(literal);
                    if !seen.insert(value.map_or_else(|| name.value.clone(), |v| format!("{v:?}")))
                    {
                        self.error(diag::E_PROB_0, "duplicate distribution value", name.span);
                    }
                    if let Some(p) = p {
                        sum.0 += &p.0;
                        if let Some(value) = value {
                            distribution.push((value, p));
                        }
                    }
                }
                if sum != crate::probability::Rational::one()
                    || d.integer_max
                        .is_some_and(|n| seen.len() as u128 != u128::from(n) + 1)
                    || self
                        .types
                        .get(base.0)
                        .is_some_and(|t| !d.optional && t.members.len() != seen.len())
                {
                    self.error(
                        diag::E_PROB_0,
                        "initial distribution must cover the type and sum to one",
                        d.span,
                    );
                }
                distribution.sort_by_key(|(v, _)| match v {
                    ir::StateValue::Value(v) => v.0 as u128,
                    ir::StateValue::Integer(v) => u128::from(*v),
                    _ => u128::MAX,
                });
                let id = self.initial_distributions.len();
                self.initial_distributions.push(distribution);
                ir::InitialValue::Distribution(id)
            } else if let Some(value) = &d.default {
                ir::InitialValue::Fixed(
                    if value.fields.is_some() {
                        self.state_expr(base, d.optional, value)
                    } else {
                        self.initial_scalar(base, d.optional, d.integer_max, value)
                    }
                    .unwrap_or(ir::StateValue::None),
                )
            } else if observed {
                if !d.optional && (base.0 < self.types.len() || d.integer_max.is_some()) {
                    self.unsupported("observed state requires a default", d.span);
                }
                ir::InitialValue::Fixed(ir::StateValue::None)
            } else {
                ir::InitialValue::Nondet
            };
            self.state_variables.push(ir::StateVariable {
                last_write_wins: !d.last_write_wins.is_empty(),
                case_set_size: d.case_set.then_some(self.total_cases.min(255) as u8),
                integer_max: d.integer_max,
                name: name.value.clone(),
                kind,
                base,
                optional: d.optional,
                initial,
                span: d.span,
            });
        }
    }

    fn initial_scalar(
        &mut self,
        base: ir::TypeId,
        optional: bool,
        max: Option<u64>,
        name: &ast::Ident,
    ) -> Option<ir::StateValue> {
        if let Some(max) = max {
            let value = name
                .value
                .parse::<i128>()
                .ok()
                .filter(|v| *v >= 0 && *v <= i128::from(max));
            if value.is_none() {
                self.error(
                    E_TYPE_0,
                    if self.merge_names.contains(&name.value) {
                        "merge_read_position: integer initial value requires a literal"
                    } else {
                        "integer initial value outside its domain"
                    },
                    name.span,
                );
            }
            value.map(|v| ir::StateValue::Integer(v as u64))
        } else {
            self.state_value(base, optional, name)
        }
    }

    fn state_value(
        &mut self,
        base: ir::TypeId,
        optional: bool,
        name: &ast::Ident,
    ) -> Option<ir::StateValue> {
        if base.0 >= self.types.len() {
            return None;
        }
        if optional && name.value == "none" {
            Some(ir::StateValue::None)
        } else {
            self.value(base, name).map(ir::StateValue::Value)
        }
    }

    fn state_comparison(
        &mut self,
        variable: ir::StateVarId,
        value: &ast::Ident,
    ) -> Option<ir::Cond> {
        let v = &self.state_variables[variable.0];
        if v.case_set_size.is_some() {
            self.error(diag::U_SYN_0, "unsupported set operation", value.span);
            return None;
        }
        if self.is_record(v.base) && !(v.optional && value.value == "none") {
            self.error(
                diag::U_SYN_0,
                "record comparison is unsupported",
                value.span,
            );
            return None;
        }
        if v.integer_max.is_some() {
            let n = value.value.parse::<i128>().ok();
            if n.is_none() {
                if v.kind
                    .actor()
                    .is_some_and(|a| self.local_names.contains_key(&(a, value.value.clone())))
                {
                    self.error(
                        diag::U_SYN_0,
                        "integer comparison with a variable is unsupported",
                        value.span,
                    );
                } else {
                    self.integer_operand_error(value);
                }
            }
            return n.map(|value| ir::Cond::IntCompare {
                variable,
                operator: ir::IntComparison::Eq,
                value,
            });
        }
        self.state_value(v.base, v.optional, value)
            .map(|value| ir::Cond::StateEq(ir::StateMatch { variable, value }))
    }

    fn local_state(&mut self, actor: &ast::Ident, variable: &ast::Ident) -> Option<ir::StateVarId> {
        if variable.value == "round"
            && let Some(&id) = self.loop_variables.get(&actor.value)
        {
            if self.loop_round_in_scope(id) {
                return Some(id);
            }
            self.unsupported("loop_round_out_of_scope", actor.span);
            return None;
        }
        let Some(&a) = self.actor_names.get(&actor.value) else {
            self.error(E_NAME_0, "undefined state Actor", actor.span);
            return None;
        };
        let id = self.local_names.get(&(a, variable.value.clone())).copied();
        if id.is_none() {
            let message = if self
                .state_variables
                .iter()
                .any(|v| matches!(v.kind, ir::StateKind::ActorObserved(_)))
            {
                "undefined Actor state"
            } else {
                "undefined Actor latent"
            };
            self.error(E_NAME_0, message, variable.span);
        }
        id
    }

    fn local_comparison(
        &mut self,
        actor: &ast::Ident,
        variable: &ast::Ident,
        operator: &ast::Spanned<ast::IntComparison>,
        value: &ast::Ident,
    ) -> Option<ir::Cond> {
        let id = self.local_state(actor, variable)?;
        self.resolved_comparison(id, operator, value, true)
    }

    /// None means this is the legacy Actor-local syntax. Some(None) means a
    /// recognized record reference was rejected; never fall back and add errors.
    fn record_comparison(
        &mut self,
        name: &ast::Ident,
        field: &ast::Ident,
        operator: &ast::Spanned<ast::IntComparison>,
        value: &ast::Ident,
        context: RecordContext<'_>,
    ) -> Option<Option<ir::Cond>> {
        let (process, contract) = (
            matches!(context, RecordContext::Process(_)),
            matches!(context, RecordContext::Contract(_)),
        );
        let (allow_binding, available, inactive_contract) = match context {
            RecordContext::Process(available) => (true, available, false),
            RecordContext::Contract(available) => (true, available, available.is_none()),
            RecordContext::StateOnly => (false, None, false),
        };
        // The scalar contract guard: a latent sharing a binding's name would be
        // read where the author may mean the binding, or the reverse.
        if contract
            && self.record_binding_names.contains(&name.value)
            && self.global_bindings.contains(&name.value)
            && self
                .state_names
                .get(&name.value)
                .is_some_and(|id| self.state_variables[id.0].kind != ir::StateKind::ObservedCase)
        {
            self.unsupported(
                "invariant name is both a binding and a latent variable",
                name.span,
            );
            return Some(None);
        }
        let state = self
            .state_names
            .get(&name.value)
            .copied()
            .filter(|id| self.is_record(self.state_variables[id.0].base));
        let binding = self
            .binding_names
            .get(&name.value)
            .copied()
            .filter(|id| self.is_record(self.bindings[id.0].ty));
        if state.is_none() && binding.is_none() {
            if self.actor_names.contains_key(&name.value) {
                return None;
            }
            if !allow_binding && self.record_binding_names.contains(&name.value) {
                self.unsupported("record binding is unsupported in this condition", name.span);
                return Some(None);
            }
            // Another process's binding: the scalar twin's class, whether or
            // not flow is checked and whatever the binding is called.
            if process && self.record_binding_names.contains(&name.value) {
                self.error(E_NAME_0, "undefined result binding", name.span);
                return Some(None);
            }
            if allow_binding
                && available.is_some()
                && self.record_binding_names.contains(&name.value)
            {
                self.unsupported("contract binding belongs to another process", name.span);
                return Some(None);
            }
            if inactive_contract && self.record_binding_names.contains(&name.value) {
                return Some(None);
            }
            if self.record_parameters.contains(&name.value) {
                self.unsupported("record parameter condition is unsupported", name.span);
                return Some(None);
            }
            return None;
        }
        if self.actor_names.contains_key(&name.value) {
            self.error(diag::E_CON_0, "ambiguous Actor and record", name.span);
            return Some(None);
        }
        if operator.value != ast::IntComparison::Eq {
            self.error(
                diag::U_SYN_0,
                "record field comparison is unsupported",
                operator.span,
            );
            return Some(None);
        }
        if state.is_none() && !allow_binding {
            self.unsupported("record binding is unsupported in this condition", name.span);
            return Some(None);
        }
        let ty = state
            .map(|s| self.state_variables[s.0].base)
            .unwrap_or_else(|| self.bindings[binding.unwrap().0].ty);
        let fields = self.types[ty.0].record.as_ref().unwrap().fields.clone();
        let Some(position) = fields.iter().position(|f| f.name == field.value) else {
            self.error(E_NAME_0, "undefined record field", field.span);
            return Some(None);
        };
        let f = &fields[position];
        let Some(member) = self.value(f.ty, value) else {
            return Some(None);
        };
        if state.is_none() {
            if self.ambiguous_bindings.contains(&name.value)
                || !self.calls.contains_key(&name.value)
            {
                self.unsupported("ambiguous record binding", name.span);
                return Some(None);
            }
            if available.is_some_and(|a| !a.contains(&binding.unwrap())) {
                self.unsupported(
                    "result is not definitely available (not yet defined or possibly cancelled)",
                    name.span,
                );
            }
        }
        let stride: usize = fields[position + 1..]
            .iter()
            .map(|f| self.types[f.ty.0].members.len())
            .product();
        let radix = self.types[f.ty.0].members.len();
        let leaves: Vec<_> = (0..self.types[ty.0].members.len())
            .filter(|index| (index / stride) % radix == member.0)
            .map(|index| {
                if let Some(variable) = state {
                    ir::Cond::StateEq(ir::StateMatch {
                        variable,
                        value: ir::StateValue::Value(ir::ValueId(index)),
                    })
                } else {
                    ir::Cond::Eq(ir::ResultMatch {
                        binding: binding.unwrap(),
                        value: ir::ValueId(index),
                    })
                }
            })
            .collect();
        Some(Some(if leaves.len() == 1 {
            leaves.into_iter().next().unwrap()
        } else {
            ir::Cond::Or(leaves)
        }))
    }

    fn resolved_comparison(
        &mut self,
        id: ir::StateVarId,
        operator: &ast::Spanned<ast::IntComparison>,
        value: &ast::Ident,
        qualified: bool,
    ) -> Option<ir::Cond> {
        if operator.value == ast::IntComparison::Eq {
            return self.state_comparison(id, value);
        }
        if self.state_variables[id.0].integer_max.is_none() {
            if self.is_record(self.state_variables[id.0].base) {
                self.error(
                    diag::U_SYN_0,
                    "record comparison is unsupported",
                    operator.span,
                );
                return None;
            }
            self.error(
                if qualified {
                    diag::U_SYN_0
                } else {
                    diag::E_SYN_0
                },
                if qualified {
                    "noninteger qualified comparison is unsupported"
                } else {
                    "expected =="
                },
                operator.span,
            );
            return None;
        }
        let n = value.value.parse::<i128>().ok();
        if n.is_none() {
            let owner = self.state_variables[id.0].kind.actor();
            if owner
                .and_then(|a| self.local_names.get(&(a, value.value.clone())))
                .is_some()
            {
                self.error(
                    diag::U_SYN_0,
                    "integer comparison with a variable is unsupported",
                    value.span,
                );
            } else {
                self.integer_operand_error(value);
            }
        }
        n.map(|value| ir::Cond::IntCompare {
            variable: id,
            operator: operator.value,
            value,
        })
    }

    fn policy_condition(&mut self, c: &ast::Spanned<ast::Cond>) {
        match &c.value {
            ast::Cond::Contains { .. } => {}
            ast::Cond::Eq { binding, .. } | ast::Cond::Compare { binding, .. } => {
                if self
                    .state_names
                    .get(&binding.value)
                    .is_some_and(|id| self.state_variables[id.0].kind.is_latent())
                {
                    self.error(
                        diag::E_POL_LATENT,
                        "policy cannot read latent state",
                        binding.span,
                    );
                }
            }
            ast::Cond::LocalEq {
                actor,
                variable,
                operator,
                ..
            } => {
                // ADR 0038 R2: `e.f` where a latent case variable and a record
                // binding share the name e reads the latent side, as the scalar
                // twin `e == x` does above (E-POL-LATENT at the name).
                if !self.actor_names.contains_key(&actor.value)
                    && self.record_binding_names.contains(&actor.value)
                    && self
                        .state_names
                        .get(&actor.value)
                        .is_some_and(|id| self.state_variables[id.0].kind.is_latent())
                {
                    self.error(
                        diag::E_POL_LATENT,
                        "policy cannot read latent state",
                        actor.span,
                    );
                    return;
                }
                if self
                    .actor_names
                    .get(&actor.value)
                    .and_then(|a| self.local_names.get(&(*a, variable.value.clone())))
                    .is_some_and(|id| {
                        self.state_variables[id.0].kind.is_latent()
                            && (operator.value == ast::IntComparison::Eq
                                || self.state_variables[id.0].integer_max.is_some())
                    })
                {
                    self.error(
                        diag::E_POL_LATENT,
                        "policy cannot read Actor latent state",
                        c.span,
                    );
                }
            }
            ast::Cond::And(parts) | ast::Cond::Or(parts) => {
                for p in parts {
                    self.policy_condition(p);
                }
            }
            ast::Cond::Group(p) => self.policy_condition(p),
        }
    }

    fn policy_block(&mut self, b: &ast::Block) {
        for s in b {
            if let ast::Stmt::Repeat { body, on_limit, .. } = &s.value {
                self.policy_block(body);
                self.policy_block(on_limit);
            }
            if let ast::Stmt::WaitUntil(c) = &s.value {
                self.policy_condition(c);
            }
            if let ast::Stmt::Merge {
                condition,
                then_body,
                else_body,
                ..
            }
            | ast::Stmt::If {
                condition,
                then_body,
                else_body,
            } = &s.value
            {
                self.policy_condition(condition);
                self.policy_block(then_body);
                self.policy_block(else_body);
            }
        }
    }

    fn target_id(&self, name: &ast::StateTarget, actor: ir::ActorId) -> Option<ir::StateVarId> {
        match name {
            ast::StateTarget::Unqualified(n) => self
                .state_names
                .get(&n.value)
                .or_else(|| self.local_names.get(&(actor, n.value.clone())))
                .copied(),
            ast::StateTarget::Qualified {
                actor: a, variable, ..
            } => self
                .actor_names
                .get(&a.value)
                .and_then(|a| self.local_names.get(&(*a, variable.value.clone())))
                .copied(),
        }
    }

    fn write_target(
        &mut self,
        name: &ast::StateTarget,
        actor: ir::ActorId,
    ) -> Option<ir::StateVarId> {
        if let Some(id) = self
            .target_id(name, actor)
            .filter(|id| matches!(self.state_variables[id.0].kind, ir::StateKind::Rule(_)))
        {
            if self.rejected_rule_writes.insert(id) {
                self.unsupported("rule values cannot be written during a run", name.span());
            }
            return None;
        }
        if let ast::StateTarget::Qualified { actor: a, .. } = name
            && self
                .state_names
                .get(&a.value)
                .is_some_and(|id| self.is_record(self.state_variables[id.0].base))
        {
            self.error(
                diag::U_SYN_0,
                "record field assignment is unsupported",
                name.span(),
            );
            return None;
        }
        if let ast::StateTarget::Qualified { actor: a, .. } = name
            && self.actor_names.get(&a.value) != Some(&actor)
        {
            self.error(
                diag::U_SYN_0,
                "writes to another Actor's state are unsupported",
                name.span(),
            );
            return None;
        }
        if let Some(id) = self.target_id(name, actor) {
            if self.state_variables[id.0].kind.is_observed() {
                return Some(id);
            }
            self.error(diag::U_SYN_0, "latent writes are unsupported", name.span());
        } else {
            self.error(E_NAME_0, "undefined write target", name.span());
        }
        None
    }

    fn state_writes(
        &mut self,
        cap: &ast::Capability,
        actor: ir::ActorId,
        result_type: Option<ir::TypeId>,
    ) -> Vec<ir::StateWrite> {
        let mut writes = None;
        let mut finish = None;
        for item in &cap.items {
            match &item.value {
                ast::CapItem::Writes(names) => {
                    if !self
                        .state_variables
                        .iter()
                        .any(|v| matches!(v.kind, ir::StateKind::ActorObserved(_)))
                    {
                        let members: Vec<_> = names.iter().map(|n| (**n).clone()).collect();
                        self.unique_members(&members, item.span);
                    } else {
                        let mut seen = BTreeSet::new();
                        for n in names {
                            if let Some(id) = self.target_id(n, actor)
                                && !seen.insert(id)
                            {
                                self.unsupported("duplicate list member", n.span());
                            }
                        }
                    }
                    self.set_once(&mut writes, names, "writes", item.span);
                }
                ast::CapItem::Finish(values) => {
                    let (mut names, mut targets) = (BTreeSet::new(), BTreeSet::new());
                    for v in values {
                        let named = !names.insert(target_name(&v.target));
                        let aliased = self
                            .target_id(&v.target, actor)
                            .is_some_and(|id| !targets.insert(id));
                        if named || aliased {
                            self.unsupported("duplicate finish target", v.target.span());
                        }
                    }
                    self.set_once(&mut finish, values, "finish", item.span);
                }
                _ => {}
            }
        }
        let mut constants = BTreeMap::new();
        for assignment in finish.into_iter().flatten() {
            if let Some(id) = self.write_target(&assignment.target, actor) {
                let value = self.assignment_source(id, assignment);
                if !writes.is_some_and(|ws| ws.iter().any(|w| self.target_id(w, actor) == Some(id)))
                    && !writes.is_some_and(|ws| ws.iter().any(|w| matches!(w, ast::StateTarget::Qualified { actor: a, .. } if self.actor_names.get(&a.value) != Some(&actor)))) {
                    self.error(
                        E_NAME_0,
                        "finish target not declared in writes",
                        assignment.target.span,
                    );
                }
                if let Some(value) = value {
                    constants.insert(id, value);
                }
            }
        }
        // A field assignment is not a whole-record constant override. Once
        // rejected, do not reinterpret its enclosing writes as a result source.
        if self
            .errors
            .iter()
            .any(|d| d.message == "record field assignment is unsupported")
        {
            return Vec::new();
        }
        let mut resolved = Vec::new();
        for name in writes.into_iter().flatten() {
            let is_kernel = cap.items.iter().any(|i| {
                matches!(
                    i.value,
                    ast::CapItem::OutcomesKernel(_) | ast::CapItem::OutcomesPerformance(_)
                )
            });
            if is_kernel
                && self
                    .prob_write_target(name, actor)
                    .is_some_and(|id| self.state_variables[id.0].kind.is_latent())
            {
                continue;
            }
            if let Some(variable) = self.write_target(name, actor) {
                if !constants.contains_key(&variable) && cap.items.iter().any(|i| matches!(&i.value, ast::CapItem::Finish(vs) if vs.iter().any(|a| self.target_id(&a.target, actor) == Some(variable) && (a.value.value == "executor.id" || self.state_variables[variable.0].case_set_size.is_some())))) { continue; }
                let source = if let Some(&value) = constants.get(&variable) {
                    value
                } else {
                    if self.state_variables[variable.0].integer_max.is_some() && !cap.items.iter().any(|i| matches!(&i.value, ast::CapItem::Finish(vs) if vs.iter().any(|a| self.target_id(&a.target, actor) == Some(variable)))) && (!is_kernel || matches!(self.state_variables[variable.0].kind, ir::StateKind::ActorObserved(_))) { self.error(E_TYPE_0, "integer cannot receive enum result", name.span); }
                    let v = &self.state_variables[variable.0];
                    if v.base.0 < self.types.len() && result_type.is_some_and(|t| t != v.base) {
                        if is_kernel && v.kind == ir::StateKind::ObservedCase {
                            resolved.push(ir::StateWrite {
                                variable,
                                source: ir::WriteSource::Constant(ir::StateValue::Value(
                                    ir::ValueId(0),
                                )),
                            });
                            continue;
                        }
                        self.error(E_TYPE_0, "result writer has different base type", name.span);
                    }
                    ir::WriteSource::Result
                };
                resolved.push(ir::StateWrite { variable, source });
            }
        }
        resolved
    }

    fn assignment_source(
        &mut self,
        id: ir::StateVarId,
        a: &ast::Assignment,
    ) -> Option<ir::WriteSource> {
        let v = &self.state_variables[id.0];
        if a.value.value == "executor.id" {
            let Some((actor, _)) = &self.cap_context else {
                return None;
            };
            if self.actors[actor.0].executors.is_none() {
                self.unsupported("executor.id requires declared executors", a.value.span);
                return None;
            }
            if Some(v.base) != self.builtin_person || a.operator != ast::AssignmentOp::Assign {
                self.error(
                    E_TYPE_0,
                    "executor.id requires a PersonId target",
                    a.target.span(),
                );
                return None;
            }
            return Some(ir::WriteSource::Executor);
        }
        if v.case_set_size.is_some() && a.value.fields.is_some() {
            // ADR 0038: a record literal is never a CaseId parameter by its
            // head; the record-value type check refuses it (E-TYPE-0/E-NAME-0).
            self.literal_value(v.base, &a.value);
            return None;
        }
        if v.case_set_size.is_some() {
            if a.operator != ast::AssignmentOp::Add {
                self.error(diag::U_SYN_0, "unsupported set operation", a.target.span());
                return None;
            }
            let params = self
                .cap_context
                .as_ref()
                .map(|(_, p)| p.as_slice())
                .unwrap_or(&[]);
            let Some(parameter) = params.iter().position(|p| p.name == a.value.value) else {
                self.error(
                    E_NAME_0,
                    if self.merge_names.contains(&a.value.value) {
                        "merge_read_position: set insertion requires a CaseId parameter"
                    } else {
                        "undefined CaseId parameter"
                    },
                    a.value.span,
                );
                return None;
            };
            if params[parameter].ty != ir::CASE_ID_TYPE {
                self.error(
                    E_TYPE_0,
                    "set insertion requires a CaseId parameter",
                    a.value.span,
                );
                return None;
            }
            return Some(ir::WriteSource::InsertCase { parameter });
        }
        if v.integer_max.is_some() {
            match a.value.value.parse::<i128>() {
                Ok(operand) => Some(ir::WriteSource::Integer {
                    operator: a.operator,
                    operand,
                    span: Span {
                        start: a.target.span().start,
                        end: a.value.span.end,
                    },
                }),
                Err(_) => {
                    self.error(
                        diag::U_SYN_0,
                        if self.merge_names.contains(&a.value.value) {
                            "merge_read_position: integer effect requires constant"
                        } else {
                            "integer effect requires constant"
                        },
                        a.value.span,
                    );
                    None
                }
            }
        } else if a.operator != ast::AssignmentOp::Assign {
            self.error(
                E_TYPE_0,
                "arithmetic effect requires integer",
                a.target.span,
            );
            None
        } else {
            self.state_expr(v.base, v.optional, &a.value)
                .map(ir::WriteSource::Constant)
        }
    }

    /// A non-numeric right operand of an integer comparison. An integer state
    /// variable there is outside ADR 0029 (like a variable on the right of an
    /// assignment): unsupported. Any other name is a type error.
    fn integer_operand_error(&mut self, value: &ast::Ident) {
        if self.merge_read_name_forbidden(value) {
            return;
        }
        if self
            .state_names
            .get(&value.value)
            .is_some_and(|id| self.state_variables[id.0].integer_max.is_some())
        {
            self.error(
                diag::U_SYN_0,
                "integer comparison with a variable is unsupported",
                value.span,
            );
        } else {
            self.error(
                E_TYPE_0,
                "integer comparison requires numeric constant",
                value.span,
            );
        }
    }

    fn integer_comparison(
        &mut self,
        binding: &ast::Ident,
        operator: &ast::Spanned<ast::IntComparison>,
        value: &ast::Ident,
    ) -> Option<ir::Cond> {
        if self
            .state_names
            .get(&binding.value)
            .is_some_and(|id| self.is_record(self.state_variables[id.0].base))
            || self
                .binding_names
                .get(&binding.value)
                .is_some_and(|id| self.is_record(self.bindings[id.0].ty))
        {
            self.error(
                diag::U_SYN_0,
                "record comparison is unsupported",
                operator.span,
            );
            return None;
        }
        let id = self.state_names.get(&binding.value).copied();
        if id.is_none_or(|id| self.state_variables[id.0].integer_max.is_none()) {
            self.error(
                diag::E_SYN_0,
                "expected ==",
                Span {
                    start: operator.span.start,
                    end: operator.span.start + 1,
                },
            );
            return None;
        }
        let n = value.value.parse::<i128>().ok();
        if n.is_none() {
            self.integer_operand_error(value);
        }
        n.map(|value| ir::Cond::IntCompare {
            variable: id.unwrap(),
            operator: operator.value,
            value,
        })
    }

    fn probability(&mut self, literal: &ast::Ident) -> Option<crate::probability::Rational> {
        let p = crate::probability::Rational::decimal(&literal.value);
        if p.as_ref().is_none_or(|p| !p.valid()) || literal.value.starts_with('-') {
            self.error(diag::E_PROB_0, "probability outside 0..1", literal.span);
            None
        } else {
            p
        }
    }
    fn prob_ref(&mut self, path: &ast::Path) -> Option<ir::StateVarId> {
        if let Some(name) = path.value.first()
            && !(path.value.len() > 1 && self.actor_names.contains_key(&name.value))
            && self.merge_read_name_forbidden(name)
        {
            return None;
        }
        if path
            .value
            .first()
            .and_then(|n| self.state_names.get(&n.value))
            .is_some_and(|id| self.is_record(self.state_variables[id.0].base))
        {
            self.unsupported("record in kernel given is unsupported", path.span);
            return None;
        }
        match path.value.as_slice() {
            [n] => {
                let id = self.state_names.get(&n.value).copied();
                if id.is_none() {
                    self.error(E_NAME_0, "unknown state variable", n.span);
                }
                id
            }
            [a, v] => {
                // A kernel is declared globally; scope belongs to each actual
                // consuming call, not to the declaration of its given rows.
                if v.value == "round"
                    && let Some(&id) = self.loop_variables.get(&a.value)
                {
                    Some(id)
                } else {
                    self.local_state(a, v)
                }
            }
            _ => {
                self.error(diag::U_SYN_0, "unsupported state path", path.span);
                None
            }
        }
    }
    fn prob_write_target(
        &mut self,
        n: &ast::StateTarget,
        a: ir::ActorId,
    ) -> Option<ir::StateVarId> {
        if let Some(id) = self
            .target_id(n, a)
            .filter(|id| matches!(self.state_variables[id.0].kind, ir::StateKind::Rule(_)))
        {
            if self.rejected_rule_writes.insert(id)
                && !self.errors.iter().any(|e| {
                    e.span == Some(n.span())
                        && e.message == "rule values cannot be written during a run"
                })
            {
                self.unsupported("rule values cannot be written during a run", n.span());
            }
            return None;
        }
        if let ast::StateTarget::Qualified { actor, .. } = n
            && self.actor_names.get(&actor.value) != Some(&a)
        {
            self.error(
                diag::U_SYN_0,
                "writes to another Actor's state are unsupported",
                n.span(),
            );
            return None;
        }
        let id = self.target_id(n, a);
        if matches!(n, ast::StateTarget::Qualified { .. })
            && id.is_some_and(|id| self.state_variables[id.0].kind.is_latent())
        {
            // Kernel writers retain the legacy unqualified latent syntax;
            // Actor state does not widen that separate write capability.
            self.error(diag::U_SYN_0, "latent writes are unsupported", n.span());
        }
        if id.is_none() {
            self.error(E_NAME_0, "undefined write target", n.span);
        }
        id
    }
    fn kernel_decl(&mut self, k: &ast::KernelDecl) -> ir::Kernel {
        for extra in k.basis.iter().skip(1) {
            self.unsupported("duplicate kernel basis", extra.span);
        }
        let given: Vec<_> = k.given.iter().filter_map(|p| self.prob_ref(p)).collect();
        if given.iter().any(|id| {
            self.state_variables[id.0].case_set_size.is_some()
                || Some(self.state_variables[id.0].base) == self.builtin_person
        }) {
            self.unsupported("unsupported kernel state type", k.span);
        }
        for (p, id) in k.given.iter().zip(&given) {
            if matches!(self.state_variables[id.0].kind, ir::StateKind::Rule(_)) {
                self.unsupported("kernel reads of rule values are unsupported", p.span);
            }
            if matches!(
                self.state_variables[id.0].kind,
                ir::StateKind::ActorObserved(_)
            ) {
                self.unsupported("Actor observed state in kernels is unsupported", p.span);
            }
        }
        if given.iter().any(|id| {
            self.state_variables[id.0].integer_max.is_some()
                && !self.loop_variables.values().any(|v| v == id)
        }) {
            self.unsupported("integer kernel given is unsupported", k.span);
        }
        for b in k.rows.iter().flat_map(|r| &r.branches) {
            if b.result.value.parse::<i128>().is_ok() {
                self.unsupported("integer kernel result is unsupported", b.result.span);
            }
            for a in &b.assignments {
                if matches!(&a.target, ast::StateTarget::Qualified { .. })
                    && self
                        .target_id(&a.target, ir::ActorId(usize::MAX))
                        .is_some_and(|id| {
                            matches!(self.state_variables[id.0].kind, ir::StateKind::Rule(_))
                        })
                {
                    self.unsupported(
                        "rule values cannot be written during a run",
                        a.target.span(),
                    );
                }
                if a.value.value.parse::<i128>().is_ok()
                    || self
                        .state_names
                        .get(&a.target.value)
                        .is_some_and(|id| self.state_variables[id.0].integer_max.is_some())
                {
                    self.unsupported("integer kernel write is unsupported", a.target.span);
                }
            }
        }
        let duplicate = given.iter().collect::<BTreeSet<_>>().len() != given.len();
        if duplicate {
            self.unsupported("duplicate given variable", k.span);
        }
        let valid_given = !duplicate
            && given.len() == k.given.len()
            && given.iter().all(|id| {
                let v = &self.state_variables[id.0];
                (v.is_loop_round() || v.base.0 < self.types.len())
                    && (v.kind == ir::StateKind::ObservedCase || !v.optional)
            });
        let mut rows = Vec::new();
        let mut keys = std::collections::HashSet::new();
        for row in &k.rows {
            let mut values = Vec::new();
            if valid_given {
                match &row.pattern {
                    None if given.is_empty() => {}
                    Some(names) if !given.is_empty() && names.len() == given.len() => {
                        for (&id, n) in given.iter().zip(names) {
                            let v = &self.state_variables[id.0];
                            let value = if v.is_loop_round() {
                                let max = v.integer_max.expect("loop round domain");
                                n.value
                                    .parse::<u64>()
                                    .ok()
                                    .filter(|x| (1..=max).contains(x))
                                    .map(ir::StateValue::Integer)
                                    .or_else(|| {
                                        self.error(
                                            diag::E_PROB_0,
                                            "invalid loop round in kernel row",
                                            n.span,
                                        );
                                        None
                                    })
                            } else {
                                self.state_value(v.base, v.optional, n)
                            };
                            if let Some(value) = value {
                                values.push(value);
                            }
                        }
                    }
                    _ => self.error(
                        diag::E_PROB_0,
                        "kernel row arity or wildcard is invalid",
                        row.span,
                    ),
                }
            }
            if valid_given && !keys.insert(values.clone()) {
                self.error(diag::E_PROB_0, "duplicate kernel row", row.span);
            }
            let mut branches = Vec::new();
            let mut sum = crate::probability::Rational::zero();
            for b in &row.branches {
                if let Some(probability) = self.probability(&b.probability) {
                    sum.0 += &probability.0;
                    branches.push(ir::KernelBranch {
                        probability,
                        result: ir::ValueId(0),
                        overrides: Vec::new(),
                    });
                }
            }
            if sum != crate::probability::Rational::one() {
                self.error(
                    diag::E_PROB_0,
                    "kernel branch mass must sum to one",
                    row.span,
                );
            }
            rows.push(ir::KernelRow { values, branches });
        }
        let count = given.iter().fold(num_bigint::BigUint::from(1u8), |n, id| {
            let v = &self.state_variables[id.0];
            let size = if v.is_loop_round() {
                v.integer_max.expect("loop round domain") as usize
            } else {
                self.types.get(v.base.0).map_or(0, |t| t.members.len()) + usize::from(v.optional)
            };
            n * num_bigint::BigUint::from(size)
        });
        if valid_given && count != num_bigint::BigUint::from(keys.len()) {
            self.error(
                diag::E_PROB_0,
                "kernel rows do not cover the domain",
                k.span,
            );
        }
        ir::Kernel {
            name: k.name.value.clone(),
            basis: k.basis.first().map(|b| b.value.clone()),
            given,
            rows,
        }
    }
    fn kernel_use(
        &mut self,
        cap: &ast::Capability,
        actor: ir::ActorId,
        ty: Option<ir::TypeId>,
        writes: &[ir::StateWrite],
    ) -> (Option<ir::Kernel>, Vec<ir::StateVarId>, Vec<ir::StateVarId>) {
        let mut name = None;
        let mut reads = None;
        let mut valid_reads = true;
        for item in &cap.items {
            match &item.value {
                ast::CapItem::OutcomesKernel(n) => {
                    self.set_once(&mut name, n, "outcomes", item.span)
                }
                ast::CapItem::KernelReadsLatent(paths) => {
                    let ids: Vec<_> = paths.iter().filter_map(|p| self.prob_ref(p)).collect();
                    for (p, id) in paths.iter().zip(&ids) {
                        if matches!(self.state_variables[id.0].kind, ir::StateKind::Rule(_)) {
                            self.unsupported("kernel reads of rule values are unsupported", p.span);
                        }
                        if matches!(
                            self.state_variables[id.0].kind,
                            ir::StateKind::ActorObserved(_)
                        ) {
                            self.unsupported(
                                "Actor observed state in kernels is unsupported",
                                p.span,
                            );
                        }
                    }
                    valid_reads &= ids.len() == paths.len();
                    if ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
                        || ids.iter().any(|id| {
                            self.state_variables[id.0].kind.is_observed()
                                && !matches!(
                                    self.state_variables[id.0].kind,
                                    ir::StateKind::Rule(_)
                                )
                        })
                    {
                        self.unsupported("kernel_reads must name distinct latents", item.span);
                    }
                    self.set_once(&mut reads, ids, "kernel_reads", item.span);
                }
                _ => {}
            }
        }
        let reads = reads.unwrap_or_default();
        let Some(name) = name else {
            return (None, reads, Vec::new());
        };
        if cap
            .items
            .iter()
            .any(|i| matches!(i.value, ast::CapItem::ResultNondet(_)))
        {
            self.error(
                diag::E_PROB_0,
                "kernel and result nondet are exclusive",
                cap.span,
            );
        }
        let Some(decl) = self.kernel_declarations.get(&name.value).cloned() else {
            self.error(E_NAME_0, "unknown kernel", name.span);
            return (None, reads, Vec::new());
        };
        let mut kernel = self
            .kernels
            .iter()
            .find(|k| k.name == name.value)
            .unwrap()
            .clone();
        if valid_reads
            && kernel
                .given
                .iter()
                .any(|id| self.state_variables[id.0].kind.is_latent() && !reads.contains(id))
        {
            self.error(
                diag::E_PROB_0,
                "kernel latent absent from kernel_reads",
                cap.span,
            );
        }
        let targets: Vec<_> = cap
            .items
            .iter()
            .filter_map(|i| match &i.value {
                ast::CapItem::Writes(ns) => Some(ns),
                _ => None,
            })
            .flatten()
            .filter_map(|n| self.prob_write_target(n, actor))
            .collect();
        let latent: Vec<_> = targets
            .iter()
            .copied()
            .filter(|id| self.state_variables[id.0].kind.is_latent())
            .collect();
        for (row, astrow) in kernel.rows.iter_mut().zip(&decl.rows) {
            for (b, ab) in row.branches.iter_mut().zip(&astrow.branches) {
                let errors_before = self.errors.len();
                if let Some(t) = ty
                    && ab.result.value.parse::<i128>().is_err()
                    && let Some(v) = self.literal_value(t, &ab.result)
                {
                    b.result = v;
                }
                let mut seen = BTreeSet::new();
                for assignment in &ab.assignments {
                    if let Some(id) = self.prob_write_target(&assignment.target, actor) {
                        if self.state_variables[id.0].case_set_size.is_some()
                            || Some(self.state_variables[id.0].base) == self.builtin_person
                        {
                            self.unsupported(
                                "unsupported kernel state type",
                                assignment.target.span(),
                            );
                        }
                        if matches!(
                            self.state_variables[id.0].kind,
                            ir::StateKind::ActorObserved(_)
                        ) {
                            self.unsupported(
                                "Actor observed state in kernels is unsupported",
                                assignment.target.span(),
                            );
                        }
                        if !seen.insert(id) {
                            self.unsupported("duplicate kernel assignment", assignment.target.span);
                        }
                        if !targets.contains(&id) {
                            self.error(
                                E_NAME_0,
                                "kernel target is outside writes",
                                assignment.target.span,
                            );
                        }
                        if writes.iter().any(|w| w.variable == id && matches!(w.source,ir::WriteSource::Constant(_))) && cap.items.iter().any(|i| matches!(&i.value,ast::CapItem::Finish(vs) if vs.iter().any(|a| a.target.value == assignment.target.value))) { self.unsupported("finish and kernel with conflict",assignment.target.span); }
                        let v = &self.state_variables[id.0];
                        if let Some(value) = self.state_value(v.base, v.optional, &assignment.value)
                        {
                            b.overrides.push((id, value));
                        }
                    }
                }
                if latent.iter().any(|id| !seen.contains(id)) {
                    self.error(
                        diag::E_PROB_0,
                        "latent write missing from branch",
                        ab.result.span,
                    );
                }
                if self.errors.len() == errors_before && targets.iter().any(|id| self.state_variables[id.0].kind == ir::StateKind::ObservedCase && ty.is_some_and(|ty| ty != self.state_variables[id.0].base) && !seen.contains(id) && !cap.items.iter().any(|i| matches!(&i.value,ast::CapItem::Finish(vs) if vs.iter().any(|a| self.state_names.get(&a.target.value) == Some(id))))) { self.error(E_TYPE_0,"unoverridden result writer has different type",ab.result.span); }
            }
        }
        (Some(kernel), reads, latent)
    }
    fn probability_condition(&mut self, c: &ast::Spanned<ast::Cond>) -> Option<ir::Cond> {
        if self.merge_condition_forbidden(c) {
            return None;
        }
        match &c.value {
            ast::Cond::Contains { .. } => {
                self.error(diag::U_SYN_0, "membership at unsupported location", c.span);
                None
            }
            ast::Cond::Compare {
                binding,
                operator,
                value,
            } => self.integer_comparison(binding, operator, value),
            ast::Cond::Eq { binding, value: _ } => {
                if !self.state_names.contains_key(&binding.value) {
                    self.error(
                        if self.global_bindings.contains(&binding.value) {
                            U_SEM_0
                        } else {
                            E_NAME_0
                        },
                        "probability conditions require state",
                        binding.span,
                    );
                    None
                } else {
                    self.contract_condition(c, None)
                        .filter(|c| !matches!(c, ir::Cond::Eq(_)))
                }
            }
            ast::Cond::LocalEq {
                actor,
                variable,
                operator,
                value,
            } => {
                if let Some(c) = self.record_comparison(
                    actor,
                    variable,
                    operator,
                    value,
                    RecordContext::StateOnly,
                ) {
                    c
                } else {
                    self.contract_condition(c, None)
                }
            }
            ast::Cond::Group(p) => self.probability_condition(p),
            ast::Cond::And(parts) | ast::Cond::Or(parts) => {
                let checked: Vec<_> = parts
                    .iter()
                    .map(|p| self.probability_condition(p))
                    .collect();
                let parts: Option<Vec<_>> = checked.into_iter().collect();
                parts.map(|p| {
                    if matches!(c.value, ast::Cond::And(_)) {
                        ir::Cond::And(p)
                    } else {
                        ir::Cond::Or(p)
                    }
                })
            }
        }
    }

    // Membership is the only new analyst condition admitted for responsibility.
    // All pre-existing leaves retain probability_condition's state-only checks,
    // including diagnostics for bindings in unselected processes.
    fn responsibility_condition(&mut self, c: &ast::Spanned<ast::Cond>) -> Option<ir::Cond> {
        if self.merge_condition_forbidden(c) {
            return None;
        }
        match &c.value {
            ast::Cond::Contains { element, set } => self.membership(element, set, None),
            ast::Cond::Group(c) => self.responsibility_condition(c),
            ast::Cond::And(parts) | ast::Cond::Or(parts) => {
                let checked: Vec<_> = parts
                    .iter()
                    .map(|p| self.responsibility_condition(p))
                    .collect();
                checked.into_iter().collect::<Option<Vec<_>>>().map(|cs| {
                    if matches!(c.value, ast::Cond::And(_)) {
                        ir::Cond::And(cs)
                    } else {
                        ir::Cond::Or(cs)
                    }
                })
            }
            _ => self.probability_condition(c),
        }
    }

    fn responsibility_formula(
        &mut self,
        c: &ast::Spanned<ast::ResponsibilityCond>,
    ) -> Option<crate::responsibility::Formula> {
        use crate::responsibility::Formula as F;
        use ast::ResponsibilityCond as C;
        match &c.value {
            C::Compare(c) => self.responsibility_condition(c).map(F::State),
            C::Bool(name) => {
                if self.merge_read_name_forbidden(name) {
                    return None;
                }
                let Some(&id) = self.state_names.get(&name.value) else {
                    self.error(
                        if self.global_bindings.contains(&name.value) {
                            U_SEM_0
                        } else {
                            E_NAME_0
                        },
                        "responsibility boolean requires observed state",
                        name.span,
                    );
                    return None;
                };
                let v = &self.state_variables[id.0];
                if v.kind != ir::StateKind::ObservedCase
                    || v.optional
                    || Some(v.base) != self.builtin_bool
                {
                    self.error(
                        E_TYPE_0,
                        "bare responsibility atom requires nonoptional observed builtin bool",
                        name.span,
                    );
                    return None;
                }
                let value = self.types[v.base.0]
                    .members
                    .iter()
                    .position(|v| v == "true")
                    .unwrap();
                Some(F::Bool(ir::StateMatch {
                    variable: id,
                    value: ir::StateValue::Value(ir::ValueId(value)),
                }))
            }
            C::Group(c) => self.responsibility_formula(c),
            C::Implies(a, b) => {
                let a = self.responsibility_formula(a);
                let b = self.responsibility_formula(b);
                Some(F::Or(vec![F::Not(Box::new(a?)), b?]))
            }
            C::And(cs) | C::Or(cs) => {
                let checked: Vec<_> = cs.iter().map(|c| self.responsibility_formula(c)).collect();
                let parts: Option<Vec<_>> = checked.into_iter().collect();
                parts.map(|cs| {
                    if matches!(c.value, C::And(_)) {
                        F::And(cs)
                    } else {
                        F::Or(cs)
                    }
                })
            }
        }
    }

    fn error(&mut self, code: &str, message: impl Into<String>, span: Span) {
        self.errors.push(Diagnostic {
            code: code.into(),
            severity: diag::Severity::Error,
            message: message.into(),
            span: Some(span),
        });
    }

    fn unsupported(&mut self, message: impl Into<String>, span: Span) {
        self.error(U_SEM_0, message, span);
    }

    fn model(
        mut self,
        model: &ast::Model,
        selection: Option<&str>,
    ) -> Result<ir::Model, Vec<Diagnostic>> {
        self.validate_contract_scopes(model);
        self.index_merges(model);
        self.total_cases = model
            .items
            .iter()
            .filter_map(|i| {
                if let ast::Item::Bounds(b) = i {
                    Some(b)
                } else {
                    None
                }
            })
            .flat_map(|b| &b.items)
            .find_map(|b| {
                if let ast::Bound::Cases(n) = &b.value {
                    Some(n.value)
                } else {
                    None
                }
            })
            .unwrap_or(1);
        // Inspect every explicit count even when another scenario error (such
        // as a duplicate bounds block) will prevent executable IR construction.
        for item in &model.items {
            if let ast::Item::CasePriority(p) = item {
                for item in &model.items {
                    if let ast::Item::Bounds(b) = item {
                        for bound in &b.items {
                            if let ast::Bound::Cases(n) = &bound.value
                                && n.value != p.value.len() as u64
                            {
                                self.error(diag::E_ARR_0, "priority_case_count_mismatch", p.span);
                            }
                        }
                    }
                }
            }
        }
        self.multi_case = model.items.iter().any(|i| matches!(i,
            ast::Item::Bounds(b) if b.items.iter().any(|v| matches!(&v.value, ast::Bound::Cases(n) if n.value >= 2))));
        // ADR 0017: selection also uses the per-case route. Actor mutability,
        // cancellation availability and every Cut check remain independently gated.
        // ADR 0001 fixes the subset of the v0.2 language; another version's
        // syntax and meaning are not interpreted as if they were v0.2.
        if model.version.value != "0.2" {
            self.error(
                diag::U_SYN_0,
                "only language version \"0.2\" is supported",
                model.version.span,
            );
        }
        let mut people_seen = false;
        for item in &model.items {
            if let ast::Item::People(names) = item {
                if people_seen {
                    self.unsupported("duplicate people declaration", names.span);
                }
                people_seen = true;
                self.unique_members(&names.value, names.span);
                for name in &names.value {
                    let id = ir::PersonId(self.people.len());
                    self.person_names.entry(name.value.clone()).or_insert(id);
                    self.people.push(ir::Person {
                        name: name.value.clone(),
                        span: name.span,
                    });
                }
            }
        }
        // Tables are built in declaration order, independently of use order.
        for item in &model.items {
            if let ast::Item::Type(ty) = item {
                if matches!(ty.name.value.as_str(), "CaseId" | "PersonId") {
                    self.unsupported(
                        format!("declared {} conflicts with builtin", ty.name.value),
                        ty.name.span,
                    );
                }
                if self.type_names.contains_key(&ty.name.value) {
                    self.unsupported("duplicate type declaration", model.span);
                }
                if ty.record.is_none() {
                    self.unique_members(&ty.members, ty.span);
                }
                let id = ir::TypeId(self.types.len());
                self.type_names.entry(ty.name.value.clone()).or_insert(id);
                self.types.push(ir::EnumType {
                    record: ty
                        .record
                        .as_ref()
                        .map(|_| ir::RecordShape { fields: Vec::new() }),
                    name: ty.name.value.clone(),
                    members: ty.members.iter().map(|v| v.value.clone()).collect(),
                    span: ty.span,
                });
            }
        }
        self.record_types(model);
        // A broken record domain has no legal placeholder values. Do not feed it
        // into value/condition resolution and manufacture secondary diagnostics.
        if !self.errors.is_empty()
            && model
                .items
                .iter()
                .any(|i| matches!(i, ast::Item::Type(t) if t.record.is_some()))
        {
            return Err(self.errors);
        }
        let mut actor_declarations = Vec::new();
        for item in &model.items {
            if let ast::Item::Actor(actor) = item {
                if self.actor_names.contains_key(&actor.name.value) {
                    self.unsupported("duplicate Actor declaration", model.span);
                    self.ambiguous_actors.insert(actor.name.value.clone());
                }
                let mut capacity = None;
                let mut source = None;
                let mut executors = None;
                let mut executor_count = None;
                let mut metadata = ir::ActorMetadata::default();
                for field in &actor.items {
                    match field {
                        ast::ActorItem::Boundary(v) => self.set_once(
                            &mut metadata.boundary,
                            v.value.clone(),
                            "boundary",
                            v.span,
                        ),
                        ast::ActorItem::Grain(v) => {
                            self.set_once(&mut metadata.grain, v.value.clone(), "grain", v.span)
                        }
                        ast::ActorItem::ViaModel(v) => {
                            let parts: Vec<_> = v.value.split('@').collect();
                            if parts.len() != 2
                                || parts.iter().any(|p| p.is_empty())
                                || v.value.chars().any(|c| c.is_whitespace() || c.is_control())
                            {
                                self.error(
                                    diag::E_CON_0,
                                    "via model requires name@version without whitespace",
                                    v.span,
                                );
                            }
                            self.set_once(
                                &mut metadata.via_model,
                                v.value.clone(),
                                "via model",
                                v.span,
                            );
                        }
                        ast::ActorItem::Control(v) => {
                            self.set_once(&mut metadata.control, v.value.clone(), "control", v.span)
                        }
                        ast::ActorItem::ControlParty(span) => {
                            self.set_once(&mut metadata.control, "party".into(), "control", *span)
                        }
                        _ => {}
                    }
                    if let ast::ActorItem::Capacity(value) = field {
                        metadata.capacity_declared = true;
                        self.set_once(&mut capacity, value.value, "capacity", actor.span);
                    }
                    if let ast::ActorItem::Source(value, description) = field {
                        self.set_once(&mut source, value.value, "source", value.span);
                        metadata.source_description = description.as_ref().map(|d| d.value.clone());
                    }
                    if let ast::ActorItem::Executors(value) = field {
                        if executor_count.is_none() {
                            executor_count = Some(
                                value
                                    .value
                                    .iter()
                                    .map(|n| &n.value)
                                    .collect::<BTreeSet<_>>()
                                    .len() as u64,
                            );
                        }
                        let persons = self.persons(&value.value, value.span);
                        self.set_once(&mut executors, persons, "executors", value.span);
                    }
                }
                if let Some(count) = executor_count {
                    if capacity.is_some_and(|n| n != count) {
                        self.error(E_CAP_0, "capacity differs from executor count", actor.span);
                    }
                    capacity = Some(count);
                }
                let id = ir::ActorId(self.actors.len());
                self.actor_names
                    .entry(actor.name.value.clone())
                    .or_insert(id);
                self.actors.push(ir::Actor {
                    metadata,
                    executors,
                    source,
                    name: actor.name.value.clone(),
                    capacity,
                    span: actor.span,
                });
                actor_declarations.push((id, actor));
            }
        }
        self.actor_instances(model);
        for item in &model.items {
            if let ast::Item::Resource(r) = item {
                if self.resource_names.contains_key(&r.name.value) {
                    self.unsupported("duplicate resource declaration", r.name.span);
                }
                if self.actor_names.contains_key(&r.name.value) {
                    self.unsupported("resource name collides with Actor", r.name.span);
                }
                let id = ir::ResourceId(self.resources.len());
                self.resource_names
                    .entry(r.name.value.clone())
                    .or_insert(id);
                self.resources.push(ir::Resource {
                    name: r.name.value.clone(),
                    capacity: r.capacity.value,
                    span: r.span,
                });
            }
        }
        let mut observed_seen = false;
        let mut latent_seen = false;
        for item in &model.items {
            match item {
                ast::Item::ObservedCase(decls) | ast::Item::LatentCase(decls) => {
                    let observed = matches!(item, ast::Item::ObservedCase(_));
                    let seen = if observed {
                        &mut observed_seen
                    } else {
                        &mut latent_seen
                    };
                    if *seen {
                        self.unsupported("duplicate case state block", model.span);
                    }
                    *seen = true;
                    for d in decls {
                        self.state_decl(
                            d,
                            if observed {
                                ir::StateKind::ObservedCase
                            } else {
                                ir::StateKind::LatentCase
                            },
                        );
                    }
                }
                ast::Item::Actor(a) => {
                    let id = self.actor_names[&a.name.value];
                    let versions: Vec<_> = a
                        .items
                        .iter()
                        .filter_map(|f| {
                            if let ast::ActorItem::Version(v) = f {
                                Some(v)
                            } else {
                                None
                            }
                        })
                        .collect();
                    let has_rule = a
                        .items
                        .iter()
                        .any(|f| matches!(f, ast::ActorItem::Decides(_)));
                    if has_rule {
                        if versions.is_empty() {
                            self.error(diag::E_CON_0, "rule Actor requires version", a.name.span);
                        }
                        for v in versions.iter().skip(1) {
                            self.unsupported("duplicate version", v.span);
                        }
                        if self.actors[id.0].is_per_case() {
                            self.unsupported("per-case rule Actor is unsupported", a.span);
                        }
                        let version = versions.first().map_or(String::new(), |v| v.value.clone());
                        self.actors[id.0].metadata.rule = Some(ir::RuleInfo {
                            declared_version: version.clone(),
                            effective_version: version,
                            values: Vec::new(),
                            amendment: None,
                        });
                    }
                    for field in &a.items {
                        if let ast::ActorItem::Latent(d) = field {
                            self.state_decl(d, ir::StateKind::ActorLatent(id));
                        } else if let ast::ActorItem::State(d) = field {
                            self.state_decl(d, ir::StateKind::ActorObserved(id));
                        } else if let ast::ActorItem::Decides(d) = field {
                            let bad_type = d.optional
                                || d.case_set
                                || matches!(d.ty.value.as_str(), "PersonId" | "CaseId" | "record")
                                || self
                                    .type_names
                                    .get(&d.ty.value)
                                    .is_some_and(|t| self.is_record(*t));
                            if bad_type {
                                self.unsupported("rule value type is unsupported", d.ty.span);
                            }
                            if !bad_type && d.default.is_none() {
                                self.unsupported("rule value requires a default", d.span);
                            }
                            // Resolve downstream references without cascading from an
                            // already rejected declaration. This placeholder never escapes.
                            let mut resolved = d.clone();
                            if bad_type {
                                resolved.ty.value = "bool".into();
                                resolved.optional = false;
                                resolved.case_set = false;
                            }
                            if bad_type || d.default.is_none() {
                                resolved.default = Some(
                                    ast::Spanned {
                                        value: "true".into(),
                                        span: d.span,
                                    }
                                    .into(),
                                );
                            }
                            let start = self.state_variables.len();
                            self.state_decl(&resolved, ir::StateKind::Rule(id));
                            for index in start..self.state_variables.len() {
                                if let ir::InitialValue::Fixed(declared) =
                                    self.state_variables[index].initial
                                {
                                    self.actors[id.0]
                                        .metadata
                                        .rule
                                        .as_mut()
                                        .unwrap()
                                        .values
                                        .push(ir::RuleValue {
                                            variable: ir::StateVarId(index),
                                            declared,
                                        });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        for item in &model.items {
            if let ast::Item::Actor(a) = item {
                for field in &a.items {
                    if let ast::ActorItem::State(d) | ast::ActorItem::Decides(d) = field {
                        for n in &d.names {
                            if self.state_names.contains_key(&n.value) {
                                self.unsupported(
                                    "Actor state name conflicts with a case variable",
                                    n.span,
                                );
                            }
                        }
                    }
                }
            }
        }
        if self.errors.iter().any(|d| {
            matches!(
                d.message.as_str(),
                "latent record is unsupported"
                    | "Actor record state is unsupported"
                    | "record distribution is unsupported"
            )
        }) {
            return Err(self.errors);
        }
        self.loop_declarations(model);
        for item in &model.items {
            if let ast::Item::Kernel(k) = item {
                if self.kernel_declarations.contains_key(&k.name.value) {
                    self.unsupported("duplicate kernel", k.span);
                }
                let resolved = self.kernel_decl(k);
                self.kernels.push(resolved);
                self.kernel_declarations
                    .insert(k.name.value.clone(), k.clone());
            }
        }
        self.performance_declarations(model);
        if self.errors.iter().any(|d| d.message == "q6_capacity_limit") {
            return Err(self.errors);
        }
        self.recovery_writers(model)?;
        for (actor_id, actor) in actor_declarations {
            let (mut contains, mut overlaps) = (None, None::<Vec<ir::ActorId>>);
            for field in &actor.items {
                let (names, containment) = match field {
                    ast::ActorItem::Contains(v) => (v, true),
                    ast::ActorItem::Overlaps(v) => (v, false),
                    _ => continue,
                };
                let mut resolved = Vec::new();
                for name in &names.value {
                    if let Some(id) = self.actor_names.get(&name.value).copied() {
                        if id == actor_id {
                            self.error(diag::E_CON_0, "self Actor relation", name.span);
                        }
                        if resolved.contains(&id) {
                            if containment {
                                self.unsupported("duplicate contains member", name.span);
                            }
                        } else {
                            resolved.push(id);
                        }
                    } else {
                        self.error(E_NAME_0, "undefined related Actor", name.span);
                    }
                }
                if containment {
                    self.set_once(&mut contains, resolved, "contains", names.span);
                } else {
                    let all = overlaps.get_or_insert_with(Vec::new);
                    for id in resolved {
                        if !all.contains(&id) {
                            all.push(id);
                        }
                    }
                }
            }
            self.actors[actor_id.0].metadata.contains = contains;
            self.actors[actor_id.0].metadata.overlaps = overlaps;
            for field in &actor.items {
                if let ast::ActorItem::Capability(cap) = field {
                    self.capability(actor_id, &actor.name.value, cap, model.span);
                }
            }
        }

        for item in &model.items {
            if let ast::Item::Actor(a) = item {
                for field in &a.items {
                    if let ast::ActorItem::Capability(c) = field
                        && c.items
                            .iter()
                            .any(|i| matches!(i.value, ast::CapItem::ReceiptQuery { .. }))
                        && let Some(id) = self
                            .cap_names
                            .get(&(a.name.value.clone(), c.name.value.clone()))
                            .copied()
                    {
                        let cap = self.capabilities[id.0].clone();
                        self.capabilities[id.0].receipt_query = self.query_declaration(
                            c,
                            cap.actor,
                            &cap.params,
                            Some(cap.return_type),
                        );
                    }
                }
            }
        }
        self.performance_bindings();
        fn record_calls(block: &ast::Block, l: &mut Lower) {
            for s in block {
                let mut note = |c: &ast::Call| {
                    if l.cap_names
                        .get(&(c.actor.value.clone(), c.capability.value.clone()))
                        .is_some_and(|id| l.is_record(l.capabilities[id.0].return_type))
                    {
                        l.record_binding_names.insert(c.binding.value.clone());
                    }
                };
                match &s.value {
                    ast::Stmt::Call(c) => note(c),
                    ast::Stmt::Fork { branches, .. } => {
                        for c in branches {
                            note(c);
                        }
                    }
                    ast::Stmt::Merge {
                        then_body,
                        else_body,
                        ..
                    }
                    | ast::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        record_calls(then_body, l);
                        record_calls(else_body, l);
                    }
                    _ => {}
                }
            }
        }
        for item in &model.items {
            if let ast::Item::Process(p) = item {
                record_calls(&p.body, &mut self);
            }
        }
        self.authority_declarations(model);

        let processes: Vec<_> = model
            .items
            .iter()
            .filter_map(|item| {
                if let ast::Item::Process(p) = item {
                    Some(p)
                } else {
                    None
                }
            })
            .collect();
        let configurations: Vec<_> = model
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Configuration(c) => Some(c),
                _ => None,
            })
            .collect();
        let configured = !configurations.is_empty();
        let (selected, mut warnings) = self.configuration(model, &configurations, selection);
        if !configured && processes.len() != 1 {
            self.unsupported("exactly one process is supported", model.span);
        }
        // Terminal names are model-wide; binding/fork names are process-local,
        // even when the process count itself is unsupported.
        for p in &processes {
            self.policy_block(&p.body);
            self.collect_ends(&p.body);
            fn names(block: &ast::Block, out: &mut BTreeSet<String>) {
                for s in block {
                    match &s.value {
                        ast::Stmt::Repeat { body, on_limit, .. } => {
                            names(body, out);
                            names(on_limit, out);
                        }
                        ast::Stmt::Again | ast::Stmt::Leave | ast::Stmt::Yield(_) => {}
                        ast::Stmt::Merge {
                            binding,
                            then_body,
                            else_body,
                            ..
                        } => {
                            out.insert(binding.value.clone());
                            names(then_body, out);
                            names(else_body, out);
                        }
                        ast::Stmt::Call(c) => {
                            out.insert(c.binding.value.clone());
                            if let Some(body) = &c.recovery {
                                names(body, out);
                            }
                        }
                        ast::Stmt::Fork { branches, .. } => {
                            out.extend(branches.iter().map(|c| c.binding.value.clone()));
                        }
                        ast::Stmt::If {
                            then_body,
                            else_body,
                            ..
                        } => {
                            names(then_body, out);
                            names(else_body, out);
                        }
                        ast::Stmt::End(_)
                        | ast::Stmt::Wait(_)
                        | ast::Stmt::WaitUntil(_)
                        | ast::Stmt::ResolveEffect { .. }
                        | ast::Stmt::StopReceiptUnknown => {}
                    }
                }
            }
            names(&p.body, &mut self.global_bindings);
        }
        let processes: Vec<_> = processes
            .into_iter()
            .filter(|p| !configured || selected.is_some_and(|c| c.process.value == p.name.value))
            .collect();
        // Only new state clauses extend alias validation to nonselected processes.
        let mut state_reads = BTreeSet::new();
        fn reads(c: &ast::Spanned<ast::InvariantStateCond>, out: &mut BTreeSet<String>) {
            use ast::InvariantStateCond as C;
            fn comparison(c: &ast::Spanned<ast::Cond>, out: &mut BTreeSet<String>) {
                match &c.value {
                    ast::Cond::Eq { binding, .. } | ast::Cond::Compare { binding, .. } => {
                        out.insert(binding.value.clone());
                    }
                    ast::Cond::And(cs) | ast::Cond::Or(cs) => {
                        for c in cs {
                            comparison(c, out);
                        }
                    }
                    ast::Cond::Group(c) => comparison(c, out),
                    // Membership reads Actor state and the case context, never a
                    // case binding alias (ADR0035).
                    ast::Cond::LocalEq { .. } | ast::Cond::Contains { .. } => {}
                }
            }
            match &c.value {
                C::Bool(n) => {
                    out.insert(n.value.clone());
                }
                C::Compare(c) => comparison(c, out),
                C::And(cs) | C::Or(cs) => {
                    for c in cs {
                        reads(c, out);
                    }
                }
                C::Group(c) => reads(c, out),
            }
        }
        // ADR 0038: the prefix of a field read `x.f`; it counts below only when
        // x is a record case variable (an Actor prefix is never an alias).
        let mut record_reads = BTreeSet::new();
        fn field_reads(c: &ast::Spanned<ast::InvariantStateCond>, out: &mut BTreeSet<String>) {
            use ast::InvariantStateCond as C;
            fn comparison(c: &ast::Spanned<ast::Cond>, out: &mut BTreeSet<String>) {
                match &c.value {
                    ast::Cond::LocalEq { actor, .. } => {
                        out.insert(actor.value.clone());
                    }
                    ast::Cond::And(cs) | ast::Cond::Or(cs) => {
                        for c in cs {
                            comparison(c, out);
                        }
                    }
                    ast::Cond::Group(c) => comparison(c, out),
                    _ => {}
                }
            }
            match &c.value {
                C::Bool(_) => {}
                C::Compare(c) => comparison(c, out),
                C::And(cs) | C::Or(cs) => {
                    for c in cs {
                        field_reads(c, out);
                    }
                }
                C::Group(c) => field_reads(c, out),
            }
        }
        for item in &model.items {
            if let ast::Item::Contract(c) = item {
                for clause in &c.clauses {
                    if let ast::ContractClause::StateInvariant {
                        antecedent,
                        condition,
                    } = &clause.value
                    {
                        if let Some(a) = antecedent {
                            reads(a, &mut state_reads);
                            field_reads(a, &mut record_reads);
                        }
                        reads(condition, &mut state_reads);
                        field_reads(condition, &mut record_reads);
                    }
                }
            }
        }
        for name in record_reads {
            if self
                .state_names
                .get(&name)
                .is_some_and(|id| self.is_record(self.state_variables[id.0].base))
            {
                state_reads.insert(name);
            }
        }
        for item in &model.items {
            if let ast::Item::Process(p) = item {
                if processes
                    .iter()
                    .any(|selected| selected.name.value == p.name.value)
                {
                    continue;
                }
                let mut sites = Vec::new();
                collect_calls(&p.body, &mut sites);
                for call in sites {
                    if state_reads.contains(&call.binding.value)
                        && let Some(&id) = self.state_names.get(&call.binding.value)
                        && self.state_variables[id.0].kind == ir::StateKind::ObservedCase
                        && !self.state_alias_writer(call, id)
                    {
                        self.unsupported(
                            "observed binding alias requires a result writer",
                            call.binding.span,
                        );
                    }
                }
            }
        }
        let single_process = processes.len() == 1;
        let mut process = None;
        for p in processes {
            self.recovery_ast(p)?;
            self.bindings.clear();
            self.binding_names.clear();
            self.binding_types.clear();
            self.unknown_binding_types.clear();
            self.ambiguous_bindings.clear();
            self.calls.clear();
            self.forks.clear();
            self.fork_names.clear();
            self.fork_spans.clear();
            self.ambiguous_forks.clear();
            self.collect(&p.body);
            self.merge_types(&p.body);
            let mut sites = Vec::new();
            collect_calls(&p.body, &mut sites);
            for c in sites {
                if let Some(&variable) = self.state_names.get(&c.binding.value)
                    && self.state_variables[variable.0].kind == ir::StateKind::ObservedCase
                {
                    let valid = self
                        .cap_names
                        .get(&(c.actor.value.clone(), c.capability.value.clone()))
                        .is_some_and(|id| {
                            self.capabilities[id.0].writes.iter().any(|w| {
                                w.variable == variable
                                    && w.source == ir::WriteSource::Result
                                    && self.capabilities[id.0].result_kernels().all(|k| {
                                        k.rows.iter().flat_map(|r| &r.branches).all(|b| {
                                            b.overrides.iter().all(|(id, x)| {
                                                *id != variable
                                                    || *x == ir::StateValue::Value(b.result)
                                            })
                                        })
                                    })
                            })
                        });
                    if !valid {
                        self.unsupported(
                            "observed binding alias requires a result writer",
                            c.binding.span,
                        );
                    }
                }
            }
            // Never derive flow facts from a first-wins ambiguous declaration.
            // Names/types/cancellation still get checked in every statement.
            self.check_flow = single_process && self.errors.is_empty();
            let mut flow = Flow {
                histories: BTreeSet::from([false]),
                ..Flow::default()
            };
            let body = self.block(&p.body, &mut flow);
            let degradations = self.degradations(p, &body);
            if let Some(span) = crate::agency::concurrent_read(&body, &self.capabilities) {
                self.unsupported("read variable has a concurrent writer", span);
            }
            process = Some(ir::Process {
                degradations,
                name: p.name.value.clone(),
                bindings: self.bindings.clone(),
                forks: self.forks.clone(),
                body,
                span: p.span,
            });
        }
        for histories in self.terminal_histories.values() {
            if histories.len() > 1 {
                self.errors.push(Diagnostic {
                    code: U_SEM_0.into(),
                    severity: diag::Severity::Error,
                    message: "terminal mixes paths with and without an any adoption".into(),
                    span: Some(model.span),
                });
                break;
            }
        }
        let bounds = self.bounds(model);
        let resource_time = bounds
            .as_ref()
            .and_then(|b| self.resource_time(model, b.clock));
        let mut simulation = None;
        let mut simulation_seen = false;
        for item in &model.items {
            if let ast::Item::Simulation(s) = item {
                if simulation_seen {
                    self.unsupported("duplicate simulation declaration", s.span);
                }
                simulation_seen = true;
                let replications = s.replications.value.parse::<u64>().ok();
                let seed = s.seed.value.parse::<u64>().ok();
                let confidence = crate::probability::Rational::decimal(&s.confidence.value);
                if s.confidence
                    .value
                    .bytes()
                    .filter(u8::is_ascii_digit)
                    .count()
                    > 64
                {
                    self.error(
                        diag::E_SIM_0,
                        "confidence exceeds 64 digits",
                        s.confidence.span,
                    );
                }
                if !replications.is_some_and(|n| (1..=10000).contains(&n)) {
                    self.error(
                        diag::E_SIM_0,
                        "replications must be between 1 and 10000",
                        s.replications.span,
                    );
                }
                if seed.is_none() || s.seed.value.starts_with('-') {
                    self.error(
                        diag::E_SIM_0,
                        "seed must be an unsigned 64-bit integer",
                        s.seed.span,
                    );
                }
                if !confidence.as_ref().is_some_and(|p| {
                    *p > crate::probability::Rational::zero()
                        && *p < crate::probability::Rational::one()
                }) {
                    self.error(
                        diag::E_SIM_0,
                        "confidence must be strictly between 0 and 1",
                        s.confidence.span,
                    );
                }
                if let (Some(replications), Some(seed), Some(confidence)) =
                    (replications, seed, confidence)
                {
                    simulation.get_or_insert(ir::SimulationSpec {
                        replications,
                        seed,
                        confidence,
                        span: s.span,
                        replications_span: s.replications.span,
                        seed_span: s.seed.span,
                        confidence_span: s.confidence.span,
                    });
                }
            }
        }
        let (mut arrivals, mut start_policy, mut admission_queue) = (None, None, None);
        let mut arrival_distribution = None;
        let mut concurrent_cases = None;
        let mut overflow_end = None;
        let mut arrival_span = model.span;
        for item in &model.items {
            match item {
                ast::Item::ArrivalsDistribution(a) => {
                    arrival_span = a.span;
                    let distribution =
                        self.arrival_distribution(a, bounds.as_ref().map_or(0, |b| b.cases));
                    if let Some(distribution) = distribution {
                        if let Some((first, _)) = distribution.entries.first() {
                            self.set_once(&mut arrivals, first.clone(), "arrivals", a.span);
                        }
                        self.set_once(
                            &mut arrival_distribution,
                            distribution,
                            "arrival distribution",
                            a.span,
                        );
                    }
                }
                ast::Item::CasePriority(_) => {}
                ast::Item::ArrivalsFixed(a) => {
                    arrival_span = a.span;
                    if a.value.is_empty() {
                        self.error(diag::E_SYN_0, "empty arrival list", a.span);
                    }
                    let times: Vec<_> = a.value.iter().filter_map(|d| self.ticks(d)).collect();
                    for (i, pair) in times.windows(2).enumerate() {
                        if pair[0] > pair[1] {
                            self.error(
                                diag::E_ARR_0,
                                "arrivals must be in ascending order",
                                a.value[i + 1].span,
                            );
                            break;
                        }
                    }
                    self.set_once(&mut arrivals, times, "arrivals", a.span);
                }
                ast::Item::Policy(p) => {
                    let mut policy = ir::StartPolicy {
                        priority: p.value.priority,
                        observable: p.value.observable,
                        ..Default::default()
                    };
                    for ext in &p.value.extensions {
                        let (flag, name) = match ext.value {
                            ast::PolicyExtension::CallOrder => {
                                (&mut policy.call_order, "then call_order")
                            }
                            ast::PolicyExtension::FirstIdle => {
                                (&mut policy.assign_first_idle, "assign first_idle")
                            }
                        };
                        if *flag {
                            self.error(U_SEM_0, format!("duplicate {name}"), ext.span);
                        }
                        *flag = true;
                    }
                    if policy.priority == Some(ir::QueueTie::Nondet) && policy.call_order {
                        self.unsupported("priority_tie_incompatible", p.span);
                    }
                    self.set_once(&mut start_policy, policy, "policy", p.span);
                }
                ast::Item::Queue(q) | ast::Item::Concurrent(q) => {
                    let overflow = match &q.value.overflow {
                        ast::Overflow::Reject => ir::Overflow::Reject,
                        ast::Overflow::WaitExternalPriority(tie) => {
                            self.admitted_allowed = true;
                            if q.value.capacity.value == 0 {
                                self.error(
                                    diag::E_CON_0,
                                    "external wait requires a positive limit",
                                    q.span,
                                );
                            }
                            ir::Overflow::WaitExternalPriority(*tie)
                        }
                        ast::Overflow::WaitExternalFifo => {
                            self.admitted_allowed = true;
                            if q.value.capacity.value == 0 {
                                self.error(
                                    diag::E_CON_0,
                                    "external wait requires a positive limit",
                                    q.span,
                                );
                            }
                            ir::Overflow::WaitExternalFifo
                        }
                        ast::Overflow::End(name) => {
                            self.admitted_allowed = true;
                            let end = if let Some(&end) = self.end_names.get(&name.value) {
                                if Some(end) != overflow_end {
                                    self.error(
                                        diag::E_CON_0,
                                        "overflow terminal conflicts with a process terminal",
                                        name.span,
                                    );
                                }
                                end
                            } else if let Some(end) = overflow_end {
                                self.unsupported(
                                    "queue and concurrent limits disagree on overflow",
                                    q.span,
                                );
                                end
                            } else {
                                let end = ir::EndId(self.ends.len());
                                self.ends.push(name.value.clone());
                                self.end_names.insert(name.value.clone(), end);
                                overflow_end = Some(end);
                                end
                            };
                            ir::Overflow::End(end)
                        }
                    };
                    let limit = ir::AdmissionQueue {
                        capacity: q.value.capacity.value,
                        overflow,
                    };
                    if matches!(item, ast::Item::Queue(_)) {
                        self.set_once(&mut admission_queue, limit, "queue", q.span);
                    } else {
                        self.set_once(&mut concurrent_cases, limit, "concurrent", q.span);
                    }
                }
                _ => {}
            }
        }
        if let (Some(q), Some(c)) = (admission_queue, concurrent_cases)
            && q.overflow != c.overflow
        {
            self.unsupported(
                "queue and concurrent limits disagree on overflow",
                model.span,
            );
        }
        if let Some(b) = &bounds
            && b.cases > 0
            && arrivals.as_ref().is_some_and(|a| a.len() as u64 != b.cases)
        {
            self.error(
                diag::E_ARR_0,
                "arrival count differs from cases",
                arrival_span,
            );
        }
        if let Some(d) = self.horizon
            && arrivals.as_ref().is_some_and(|a| a.iter().any(|t| *t > d))
        {
            self.error(diag::E_ARR_0, "arrival after horizon", arrival_span);
        }
        self.shared_latent_restrictions(model);
        let mut verify = Vec::new();
        let mut contracts = Vec::new();
        let mut metadata = crate::responsibility::Metadata::default();
        let mut feature = false;
        for item in &model.items {
            if let ast::Item::Actor(a) = item {
                let actor = self.actor_names[&a.name.value];
                for field in &a.items {
                    if let ast::ActorItem::ControlParty(_) = field {
                        feature = true;
                        // Declaration multiplicity is diagnosed once when
                        // ActorMetadata is collected. This pass only projects
                        // the existing party semantics, not another validator.
                        if !metadata.parties.contains(&actor) {
                            metadata.parties.push(actor);
                        }
                    }
                }
                let mut seen = BTreeSet::new();
                for field in &a.items {
                    if let ast::ActorItem::Standing {
                        targets,
                        basis,
                        span,
                    } = field
                    {
                        feature = true;
                        if !metadata.parties.contains(&actor) {
                            self.error(diag::E_CON_0, "Standing requires control party", *span);
                        }
                        if targets.is_empty() {
                            self.error(diag::E_SYN_0, "Standing requires terminals", *span);
                        }
                        for name in targets {
                            if let Some(target) = self.standing_target(name) {
                                if target.end().is_some() && target.end() == overflow_end {
                                    self.unsupported(
                                        "Standing on an overflow terminal is unsupported",
                                        name.span(),
                                    );
                                }
                                if !seen.insert(target) {
                                    self.unsupported(
                                        if target.effect() {
                                            "duplicate Standing effect"
                                        } else {
                                            "duplicate Standing terminal"
                                        },
                                        name.span(),
                                    );
                                }
                                metadata.standings.push(crate::responsibility::Standing {
                                    actor,
                                    target,
                                    basis: basis.value.clone(),
                                    span: *span,
                                });
                            }
                        }
                    }
                }
            }
            if let ast::Item::Responsible(d) = item {
                feature = true;
                let person = self.person(&d.person);
                let within = d.within.as_ref().and_then(|d| self.ticks(d));
                let target = match &d.target {
                    ast::ResponsibilityTarget::Capability(p)
                        if p.capability.value == "maintenance" =>
                    {
                        if let Some(&actor) = self.actor_names.get(&p.actor.value) {
                            if self
                                .cap_names
                                .contains_key(&(p.actor.value.clone(), p.capability.value.clone()))
                            {
                                self.error(diag::E_CON_0,"ambiguous Actor maintenance responsibility and Capability named maintenance",p.span);
                                None
                            } else {
                                Some(crate::responsibility::Target::Maintenance(actor))
                            }
                        } else {
                            self.error(E_NAME_0, "undefined maintenance Actor", p.actor.span);
                            None
                        }
                    }
                    ast::ResponsibilityTarget::Capability(p) => self
                        .capability_path(p)
                        .map(crate::responsibility::Target::Capability),
                    ast::ResponsibilityTarget::Condition(c) => self
                        .responsibility_condition(c)
                        .map(crate::responsibility::Target::Condition),
                    ast::ResponsibilityTarget::Formula(c) => self
                        .responsibility_formula(c)
                        .map(crate::responsibility::Target::Formula),
                };
                if let (Some(person), Some(target)) = (person, target) {
                    if metadata.duties.iter().any(|x| {
                        x.person == person
                            && crate::responsibility::same_target(
                                &self.types,
                                &self.state_variables,
                                &x.target,
                                &target,
                            )
                    }) {
                        self.unsupported("duplicate responsibility", d.span);
                    }
                    metadata.duties.push(crate::responsibility::Duty {
                        person,
                        target,
                        within,
                        span: d.span,
                    });
                }
            }
        }
        let mut ordinal = 0;
        // Synthetic routes do not define ordinary result bindings. Intersect
        // their target-end availability with all ordinary invocation sites.
        for item in &model.items {
            if let ast::Item::Contract(c) = item {
                for clause in &c.clauses {
                    if let ast::ContractClause::Contest {
                        target: ast::StandingRef::End(end),
                        route,
                        ..
                    } = &clause.value
                    {
                        let cap = self
                            .cap_names
                            .get(&(route.actor.value.clone(), route.capability.value.clone()));
                        let end = self.end_names.get(&end.value);
                        if let (Some(cap), Some(end)) = (cap, end) {
                            if !self.end_before.contains_key(end) {
                                continue;
                            }
                            let before = self.end_before.get(end).cloned().unwrap_or_default();
                            self.route_before
                                .entry(*cap)
                                .and_modify(|old| {
                                    *old = old.intersection(&before).copied().collect();
                                })
                                .or_insert(before);
                        }
                    }
                }
            }
        }
        let mut contract_names = BTreeSet::new();
        // Effect routes are callable immediately after the target's normal
        // completion. Their price inventory and binding availability must not
        // be restricted to routes attached to an End.
        fn effect_calls(b: &ir::Block, cap: ir::CapabilityId, out: &mut Vec<ir::BindingId>) {
            for s in b {
                match &s.kind {
                    ir::Stmt::Call(c) | ir::Stmt::Response(c, _) if c.capability == cap => {
                        out.push(c.binding)
                    }
                    ir::Stmt::Fork { branches, .. } => out.extend(
                        branches
                            .iter()
                            .filter(|c| c.capability == cap)
                            .map(|c| c.binding),
                    ),
                    ir::Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        effect_calls(then_body, cap, out);
                        effect_calls(else_body, cap, out);
                    }
                    _ => {}
                }
            }
        }
        for item in &model.items {
            if let ast::Item::Contract(c) = item {
                for clause in &c.clauses {
                    if let ast::ContractClause::Contest {
                        target: ast::StandingRef::Commit(target),
                        route,
                        ..
                    } = &clause.value
                        && let (Some(target), Some(route), Some(process)) = (
                            self.cap_names.get(&(
                                target.actor.value.clone(),
                                target.capability.value.clone(),
                            )),
                            self.cap_names
                                .get(&(route.actor.value.clone(), route.capability.value.clone())),
                            process.as_ref(),
                        )
                    {
                        let mut sites = Vec::new();
                        effect_calls(&process.body, *target, &mut sites);
                        for site in sites {
                            let mut before =
                                self.call_before.get(&site).cloned().unwrap_or_default();
                            before.insert(site);
                            self.route_before
                                .entry(*route)
                                .and_modify(|old| {
                                    *old = old.intersection(&before).copied().collect()
                                })
                                .or_insert(before);
                        }
                    }
                }
            }
        }
        for item in &model.items {
            if let ast::Item::Contract(c) = item {
                if let Some(n) = &c.name
                    && !contract_names.insert(n.value.clone())
                {
                    self.unsupported("duplicate contract name", n.span);
                }
                for clause in &c.clauses {
                    ordinal += 1;
                    if !crate::contract_scope::applies(c, selection) {
                        continue;
                    }
                    if let ast::ContractClause::Contest {
                        party,
                        target,
                        route,
                        reversible_by,
                        respond_by,
                        rates,
                    } = &clause.value
                    {
                        if rates.is_some() && matches!(target, ast::StandingRef::Commit(_)) {
                            self.unsupported("contest_rates_effect_target", clause.span);
                        }
                        let rates = rates.as_ref().and_then(|r| self.contest_rates(r));
                        feature = true;
                        let actor = self.actor_names.get(&party.value).copied();
                        if actor.is_none() {
                            self.error(E_NAME_0, "undefined contest party", party.span);
                        }
                        let target = self.standing_target(target);
                        if target.is_some_and(|t| t.end().is_some() && t.end() == overflow_end) {
                            self.unsupported(
                                "Standing on an overflow terminal is unsupported",
                                clause.span,
                            );
                        }
                        let route = self.capability_path(route);
                        let persons = reversible_by
                            .iter()
                            .filter_map(|n| self.person(n))
                            .collect::<Vec<_>>();
                        if persons.iter().collect::<BTreeSet<_>>().len() != persons.len() {
                            self.unsupported("duplicate reversible person", clause.span);
                        }
                        if reversible_by.is_empty() {
                            self.error(
                                diag::E_SYN_0,
                                "reversible_by must not be empty",
                                clause.span,
                            );
                        }
                        if !matches!(respond_by.unit.value.as_str(), "min" | "h" | "d") {
                            self.error(
                                diag::U_SYN_0,
                                "contest time unit must be min, h or d",
                                respond_by.unit.span,
                            );
                        }
                        let response_ticks = self.ticks(respond_by);
                        if let (Some(party), Some(target), Some(route), Some(response_ticks)) =
                            (actor, target, route, response_ticks)
                        {
                            if self.errors.is_empty()
                                && (!metadata.parties.contains(&party)
                                    || !metadata
                                        .standings
                                        .iter()
                                        .any(|s| s.actor == party && s.target == target))
                            {
                                self.error(
                                    diag::E_CON_0,
                                    "contest must match party Standing",
                                    clause.span,
                                );
                            }
                            let id = metadata.contests.len();
                            metadata.contests.push(crate::contest::Contest {
                                name: c.name.as_ref().map(|n| n.value.clone()),
                                ordinal,
                                party,
                                target,
                                route,
                                reversible_by: persons,
                                respond_by: response_ticks,
                                rates,
                                span: clause.span,
                            });
                            verify.push(ir::Verify {
                                property: ir::Property::Contest(id),
                                span: clause.span,
                            });
                            let runtime = ir::ClauseId(contracts.len());
                            contracts.push(ir::ContractClause {
                                ordinal: Some(ordinal),
                                name: c.name.as_ref().map(|n| n.value.clone()),
                                kind: ir::ContractKind::ContestResponse { contest: id },
                                span: clause.span,
                            });
                            verify.push(ir::Verify {
                                property: ir::Property::Contract(runtime),
                                span: clause.span,
                            });
                        }
                        continue;
                    }
                    if let Some(kind) = self.contract_clause(&clause.value) {
                        let id = ir::ClauseId(contracts.len());
                        contracts.push(ir::ContractClause {
                            ordinal: Some(ordinal),
                            name: c.name.as_ref().map(|n| n.value.clone()),
                            kind,
                            span: clause.span,
                        });
                        verify.push(ir::Verify {
                            property: ir::Property::Contract(id),
                            span: clause.span,
                        });
                    }
                }
            }
            if let ast::Item::Verify(v) = item {
                let property = match &v.value {
                    ast::Property::ProbabilityReached {
                        condition,
                        within,
                        comparison,
                        bound,
                    } => {
                        let c = match condition {
                            ast::ProbabilityCondition::State(c) => self
                                .probability_condition(c)
                                .map(ir::ProbabilityCondition::State),
                            ast::ProbabilityCondition::End(e) => {
                                self.end(e).map(ir::ProbabilityCondition::End)
                            }
                        };
                        let b = self.probability(bound);
                        let deadline = within.as_ref().and_then(|d| self.ticks(d));
                        match (c, b) {
                            (Some(condition), Some(bound)) => {
                                let query = ir::ProbabilityQuery {
                                    condition,
                                    within: deadline,
                                    comparison: *comparison,
                                    bound,
                                };
                                if self.probability_queries.contains(&query) {
                                    self.unsupported("duplicate probability query", v.span);
                                }
                                let id = self.probability_queries.len();
                                self.probability_queries.push(query);
                                Some(ir::Property::ProbabilityReached(id))
                            }
                            _ => None,
                        }
                    }
                    ast::Property::RecoverableEnd => Some(ir::Property::RecoverableEnd),
                    ast::Property::TerminatingEnd => Some(ir::Property::TerminatingEnd),
                    ast::Property::AlmostSureEnd => Some(ir::Property::AlmostSureEnd),
                    ast::Property::NoResourceOvercommit => Some(ir::Property::NoResourceOvercommit),
                    ast::Property::ReachableEnd(name) => {
                        self.end(name).map(ir::Property::ReachableEnd)
                    }
                };
                if property.is_some_and(|p| verify.iter().any(|v: &ir::Verify| v.property == p)) {
                    // Like other duplicate declarations: one property id each.
                    self.unsupported("duplicate verification request", v.span);
                } else if let Some(property) = property {
                    verify.push(ir::Verify {
                        property,
                        span: v.span,
                    });
                }
            }
        }
        self.resolve_waivers(model, &mut contracts, selection);
        if self
            .state_variables
            .iter()
            .any(|v| v.integer_max.is_some() && !v.name.starts_with("@loop:"))
        {
            verify.push(ir::Verify {
                property: ir::Property::StateInRange,
                span: model.span,
            });
        }
        for (i, v) in self.state_variables.iter().enumerate() {
            if v.last_write_wins {
                verify.push(ir::Verify {
                    property: ir::Property::NoLostUpdate(ir::StateVarId(i)),
                    span: v.span,
                });
            }
        }
        self.configuration_preserve_names(model, &contracts, &metadata);
        // A failed probability query already carries its own fault; models
        // without one keep the ADR 0001 diagnostic even when every verify fails.
        if verify.is_empty()
            && !model.items.iter().any(|i| {
                matches!(i, ast::Item::Verify(v)
                    if matches!(v.value, ast::Property::ProbabilityReached { .. }))
                    || matches!(i, ast::Item::Contract(c) if crate::contract_scope::applies(c, selection) && c.clauses.iter().any(|c| matches!(c.value, ast::ContractClause::StateInvariant { .. })))
            })
        {
            self.unsupported("at least one verification request is required", model.span);
        }
        let quality = self.quality(model);
        let comparison = self.comparison(model, quality.as_ref());
        let (cut, cut_warnings) = self.cut(model, selected);
        warnings.extend(cut_warnings);
        let contract_selection = crate::contract_scope::Selection::from_ast(model, selection);
        warnings.extend(contract_selection.warnings());
        self.rule_amendments(model, selected);
        if !model.items.iter().any(|i| matches!(i, ast::Item::Cut(_))) {
            warnings.push(Diagnostic {
                code: diag::W_CUT_0.into(),
                severity: diag::Severity::Warning,
                message:
                    "Cut is undeclared: all declared Actors are included; exclusions are unknown"
                        .into(),
                span: None,
            });
        }
        let metrics = self.metrics(model, process.as_ref(), bounds.as_ref());
        if !self.errors.is_empty() {
            self.errors.extend(warnings);
            return Err(self.errors);
        }
        // Missing mandatory values always emitted an error above. No executable
        // placeholder is ever returned, even for a hand-constructed invalid AST.
        let (Some(process), Some(bounds)) = (process, bounds) else {
            return Err(vec![Diagnostic {
                code: U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: "missing process or explicit bounds".into(),
                span: Some(model.span),
            }]
            .into_iter()
            .chain(warnings)
            .collect());
        };
        if let Some(p) = &mut start_policy
            && p.extended()
        {
            let mut classes = ir::PolicyClasses::default();
            for (i, actor) in self.actors.iter().enumerate() {
                let id = ir::ActorId(i);
                let control = actor.metadata.control.as_deref().unwrap_or_else(|| {
                    if self.called_actors.contains(&id) {
                        "policy"
                    } else {
                        "environment"
                    }
                });
                match control {
                    "policy" => classes.policy.push(id),
                    "party" => classes.party.push(id),
                    "environment" => classes.environment.push(id),
                    other => {
                        return Err(vec![Diagnostic {
                            code: U_SEM_0.into(),
                            severity: diag::Severity::Error,
                            message: format!("unknown Actor control {other}"),
                            span: Some(actor.span),
                        }]
                        .into_iter()
                        .chain(warnings)
                        .collect());
                    }
                }
                let declared = model.items.iter().find_map(|item| match item {
                    ast::Item::Actor(a) if self.actor_names[&a.name.value] == id => {
                        a.items.iter().find_map(|field| match field {
                            ast::ActorItem::Executors(e) => Some(
                                e.value
                                    .iter()
                                    .map(|person| self.person_names[&person.value])
                                    .collect(),
                            ),
                            _ => None,
                        })
                    }
                    _ => None,
                });
                p.executor_order.push(declared);
            }
            p.classes = Some(classes);
        }
        let mut assumptions = self.assumptions();
        if contracts.iter().any(|c| {
            matches!(
                c.kind,
                ir::ContractKind::NamedResponse { .. }
                    | ir::ContractKind::Excludes { .. }
                    | ir::ContractKind::Waive { .. }
                    | ir::ContractKind::ContestResponse { .. }
            )
        }) {
            for (field, value) in [
                ("obligations.scope", "per_case_response_instances"),
                ("obligations.outcomes", "possible_outcomes_not_path_counts"),
            ] {
                assumptions.push(ir::Assumption {
                    field: field.into(),
                    value: value.into(),
                    affects: "model response obligations only; alternatives are not path counts"
                        .into(),
                });
            }
            if contracts
                .iter()
                .any(|c| matches!(c.kind, ir::ContractKind::Waive { .. }))
            {
                assumptions.push(ir::Assumption {
                    field: "waive.authority".into(),
                    value: "static_grants_all_executors".into(),
                    affects: "explicit model grants, not real-world authorization".into(),
                });
            }
        }
        for actor in &metadata.parties {
            let field = format!("{}.control", self.actors[actor.0].name);
            assumptions.retain(|a| a.field != field);
        }
        if arrivals.is_some()
            || start_policy.is_some()
            || admission_queue.is_some()
            || concurrent_cases.is_some()
            || self.scenario.is_some()
        {
            let times = arrivals.clone().unwrap_or_else(|| vec![0]);
            let mut add = |field: &str, value: String, affects: &str| {
                assumptions.push(ir::Assumption {
                    field: field.into(),
                    value,
                    affects: affects.into(),
                })
            };
            add(
                "arrivals",
                self.arrival_every.as_ref().map_or_else(
                    || {
                        format!(
                            "fixed[{}]",
                            times
                                .iter()
                                .map(|t| format!("{t}min"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    },
                    |e| {
                        format!(
                            "fixed_every {}min during [{}min, {}min) ({} cases)",
                            e.every, e.from, e.until, e.count
                        )
                    },
                ),
                "arrival times are fixed analyst assumptions; case IDs follow list order",
            );
            if self.state_variables.iter().any(|v| {
                !matches!(v.kind, ir::StateKind::ActorLatent(_))
                    && matches!(v.initial, ir::InitialValue::Distribution(_))
            }) {
                add(
                    "initial.case_independence",
                    if self.admitted_allowed {
                        "independent-per-arrival"
                    } else {
                        "independent-per-admission"
                    }
                    .into(),
                    if self.admitted_allowed {
                        "case initial distributions are drawn independently at arrival for every non-rejected case; this independence is an analyst assumption"
                    } else {
                        "case initial distributions are drawn independently for each admitted case at receipt; this independence is an analyst assumption"
                    },
                );
            }
            if let Some(p) = &start_policy {
                add(
                    "policy.queue",
                    format!(
                        "{}fifo tie_by case_id{}",
                        if p.observable { "observable " } else { "" },
                        if p.call_order { " then call_order" } else { "" }
                    ),
                    if p.call_order {
                        "方策の区分の待ちを、待ちに入った時刻・案件ID・呼出しの字句の順で比較し、開始可能な最小の1件を1遷移で開始または拒否する。Actorと資源の待ちの先頭を追い越さず、人は取り置かない。通常の呼出しはprocess本体の前順、contestの合成呼出しはその後のcontest宣言順とする。環境とpartyにはActorごとのFIFOだけが掛かる。同じ時刻に後から待ちに入るTaskは比べない。"
                    } else {
                        "each start compares only the tasks queued within the Actor at that moment, by queue-entry time then case ID; tasks entering the queue later in the same instant are not compared; ties within one case remain nondeterministic"
                    },
                );
                if let Some(classes) = &p.classes {
                    let called: Vec<_> = self
                        .called_actors
                        .iter()
                        .copied()
                        .chain(
                            metadata
                                .contests
                                .iter()
                                .map(|c| self.capabilities[c.route.0].actor),
                        )
                        .collect();
                    if p.assign_first_idle {
                        let mut affects = "方策の区分のActorには、executorsの宣言順の最初の空いている人を割り当てる。占有は全Actor・全案件で共有し、PermissionとAuthorityでは人を選ばない。".to_owned();
                        if !classes
                            .policy
                            .iter()
                            .any(|a| self.actors[a.0].executors.is_some())
                        {
                            affects.push_str(" 方策の区分にexecutorsを持つActorが無いため、この割当規則の適用先は無い。");
                        } else if !classes
                            .policy
                            .iter()
                            .any(|a| called.contains(a) && self.actors[a.0].executors.is_some())
                        {
                            affects.push_str(" 選択processと宣言されたcontest routeに、方策の区分の実行者つき呼出しは無い。");
                        }
                        add(
                            "policy.assign",
                            "first_idle(executors order)".into(),
                            &affects,
                        );
                    }
                    let mut affects = "方策が解くのはpolicy区分の宣言した開始順と割当てだけである。解かない選択は非決定のまま残る。方策はmodelの規則でActorとして切っておらず、Cutのinclude/excludeとSilent Cut検出の対象外である。安全性・契約・時間の結論は、この固定方策が生む経路と環境の全選択についてであり、別の方策の順序・割当てを保証しない。".to_owned();
                    if classes.policy.is_empty() {
                        affects.push_str(" policy区分のActorが無いため、延長の適用先は無い。");
                    } else if !classes.policy.iter().any(|a| called.contains(a)) {
                        affects.push_str(
                            " 選択processと宣言されたcontest routeにpolicy区分の呼出しは無い。",
                        );
                    }
                    fn policy_calls(
                        body: &ir::Block,
                        caps: &[ir::Capability],
                        policy: &[ir::ActorId],
                    ) -> usize {
                        body.iter()
                            .map(|s| match &s.kind {
                                ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => {
                                    usize::from(policy.contains(&caps[c.capability.0].actor))
                                }
                                ir::Stmt::Fork { branches, .. } => branches
                                    .iter()
                                    .filter(|c| policy.contains(&caps[c.capability.0].actor))
                                    .count(),
                                ir::Stmt::If {
                                    then_body,
                                    else_body,
                                    ..
                                } => {
                                    policy_calls(then_body, caps, policy)
                                        + policy_calls(else_body, caps, policy)
                                }
                                _ => 0,
                            })
                            .sum()
                    }
                    if p.call_order
                        && !p.assign_first_idle
                        && bounds.cases == 1
                        && policy_calls(&process.body, &self.capabilities, &classes.policy)
                            + metadata
                                .contests
                                .iter()
                                .filter(|c| {
                                    classes.policy.contains(&self.capabilities[c.route.0].actor)
                                })
                                .count()
                            <= 1
                    {
                        affects.push_str(" 方策の区分で同時に待つ呼出しは高々1件であり、開始順の取り合いは無い。");
                    }
                    let names = |ids: &[ir::ActorId]| {
                        ids.iter()
                            .map(|a| self.actors[a.0].name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    add(
                        "policy.classes",
                        format!(
                            "policy[{}]; environment[{}]; party[{}]",
                            names(&classes.policy),
                            names(&classes.environment),
                            names(&classes.party)
                        ),
                        &affects,
                    );
                }
            }
            for (field, limit) in [
                ("queue.capacity", admission_queue),
                ("concurrent.cases", concurrent_cases),
            ] {
                let Some(q) = limit else { continue };
                if field == "queue.capacity" && q.overflow == ir::Overflow::Reject {
                    add(
                        field,
                        format!("{} on_overflow reject", q.capacity),
                        "admitted nonterminal cases with no task ever started are counted before same-time starts; rejected cases retain their probability mass",
                    );
                    continue;
                }
                let action = match q.overflow {
                    ir::Overflow::Reject => "reject".to_owned(),
                    ir::Overflow::WaitExternalFifo => "wait_external fifo".to_owned(),
                    ir::Overflow::WaitExternalPriority(tie) => format!(
                        "wait_external priority descending tie {}",
                        match tie {
                            ir::QueueTie::Nondet => "nondet",
                            ir::QueueTie::FifoCaseId => "fifo tie_by case_id",
                        }
                    ),
                    ir::Overflow::End(e) => format!("end {}", self.ends[e.0]),
                };
                let mut affects = if field == "queue.capacity" {
                    "Admitted nonterminal cases with no task ever started are counted. "
                } else {
                    "All admitted nonterminal cases, including started cases, are counted. "
                }
                .to_owned();
                affects.push_str("Admission limits and overflow handling are model rules, not an Actor. Counts are tested before same-time starts and finishes. Concurrent cases stop counting at the business end, not contest closure.");
                match q.overflow {
                    ir::Overflow::WaitExternalFifo => affects.push_str(" Receipt and initial values occur at arrival; admission occurs when both limits have room. External FIFO may reuse slots released at the same time after immediate events settle; new arrivals do not. External wait is included in elapsed, within and receipt-based response deadlines. first_start_wait remains arrival-based and overlaps external_wait; do not add them."),
                    ir::Overflow::WaitExternalPriority(_) => affects.push_str(" Priority external readmission is restricted to Held cases, not initial admission. Receipt and initial values occur once at arrival; admission rechecks both limits after immediate events settle, one case at a time. External wait is included in elapsed, within and receipt-based response deadlines. first_start_wait remains arrival-based and overlaps external_wait; do not add them."),
                    ir::Overflow::End(_) => affects.push_str(" Overflow cases are received but not admitted and immediately reach the declared overflow terminal without creating Tasks; probability mass is retained."),
                    ir::Overflow::Reject => affects.push_str(" Rejected cases retain their probability mass."),
                }
                add(
                    field,
                    format!("{} on_overflow {action}", q.capacity),
                    &affects,
                );
            }
            if start_policy.is_some() {
                for a in &mut assumptions {
                    if a.field == "schedule" {
                        a.affects = if start_policy.as_ref().is_some_and(ir::StartPolicy::extended) {
                            "宣言した固定方策で解けない開始・割当て、区分をまたぐ競合、終了・採用・開始の同時刻の順は非決定のまま全て展開する。即時遷移が尽きるまで時刻を進めない。"
                        } else { "competing starts unresolved by the declared FIFO rule remain nondeterministic" }.into();
                    }
                }
            }
        }
        if let Some(spec) = &metrics {
            for (field, value, affects) in [
                (
                    "metrics.window",
                    format!("{}min", spec.window),
                    "全資源は共通の分析窓で利用可能と仮定する。windowは容量時間の分母であり、clockや到着を変えず、終了後のdrainを追加しない。",
                ),
                (
                    "metrics.currency",
                    spec.currency.clone(),
                    "費用はこの通貨の最小単位であり、他通貨へ換算しない。",
                ),
                (
                    "metrics.rates",
                    "model-declarations".into(),
                    "単価の出所はモデルの宣言であり、実際の賃金・価格ではない。未設定の費目を0円にせず、必要費目が一つでも欠ければ合計もnot_configuredとする。",
                ),
                (
                    "metrics.lead_time",
                    "terminal_cases_only".into(),
                    "リードタイムは終端に達した案件だけの値である。未完了・受付拒否・deadlockの件数を並べ、業務上の成功や解決を終端名だけから認定しない。",
                ),
                (
                    "metrics.observation",
                    if self.horizon.is_some() {
                        "until_horizon_or_execution_stop"
                    } else {
                        "until_execution_stop_no_extra_drain"
                    }
                    .into(),
                    "作業時間と費用は観測停止までに発生した占有と開始の値である。未完了案件の費用も含み、未観測の全期間の費用とは呼ばない。網羅で切断が一つでもあれば新指標の極値は全て未確定とする。",
                ),
            ] {
                assumptions.push(ir::Assumption {
                    field: field.into(),
                    value,
                    affects: affects.into(),
                });
            }
        }
        if let Some(s) = &self.scenario {
            assumptions.push(ir::Assumption {
                field: "scenario".into(),
                value: s.name.clone(),
                affects:
                    "比較条件を固定する。scenarioの外の宣言は全scenarioに共通で、上書きしない。"
                        .into(),
            });
            if let Some(d) = self.horizon {
                assumptions.push(ir::Assumption { field: "scenario.horizon".into(), value: format!("{d}min"), affects: "業務上の打切り。未完了を成功・失敗・受付拒否・計算上の切断に数えず、継続に依る性質は未確定。指標は打切りまでの観測値であり、業務全期間の作業量・費用の下界。Cut.horizonは記録のみ。".into() });
            }
            let drain_from = arrival_distribution
                .as_ref()
                .and_then(|d: &ir::ArrivalDistribution| d.declared_bounds().map(|(_, until)| until))
                .or_else(|| arrivals.as_ref().and_then(|a| a.last()).copied())
                .unwrap_or(0);
            assumptions.push(ir::Assumption { field: "scenario.drain".into(), value: format!("{drain_from}min..{}", self.horizon.map_or_else(|| "all_terminal_or_clock".into(), |d| format!("{d}min"))), affects: "最後の到着の後も処理を続ける。宣言上の区間であり、実停止はaggregationに記録する。".into() });
            assumptions.push(ir::Assumption { field: "scenario.aggregation".into(), value: "0min..execution_stop; warmup 0min".into(), affects: "ウォームアップは無い。全資源は共通の分析窓で利用可能と仮定し、休日・交代勤務を模擬しない。".into() });
        }
        let mut fallback_capabilities = Vec::new();
        if let Some(c) = selected {
            for intent in &c.intent {
                if let ast::Intent::Claim {
                    target,
                    keep_fallback: true,
                    span,
                    ..
                } = intent
                    && let Some(&id) = self.cap_names.get(&(
                        target.path.value[0].value.clone(),
                        target.path.value[1].value.clone(),
                    ))
                {
                    if crate::agency::lexical_has_capability(&process.body, id) {
                        return Err(vec![Diagnostic {
                            code: diag::E_DIFF_0.into(),
                            severity: diag::Severity::Error,
                            message: "kept fallback is still called on the normal path".into(),
                            span: Some(*span),
                        }]);
                    }
                    fallback_capabilities.push(id);
                }
            }
        }
        fallback_capabilities.sort();
        fallback_capabilities.dedup();
        if quality.is_some() {
            for (field, value, affects) in [
                (
                    "quality.definition",
                    "model-declarations",
                    "性質はモデルの評価条件であり現実の正しさの認定ではない。潜在は評価だけで読み、方策からは読まない。",
                ),
                (
                    "quality.denominator",
                    "all_arrivals_or_declared_property",
                    "arrivals は受付拒否・overflow・未完了・deadlockを含む全流入。0又は未確定の分母の比を0で埋めず理由つきnullとする。clock以外の計算切断もtruncatedにする。",
                ),
                (
                    "quality.form",
                    "ratio_of_expected_counts",
                    "期待件数の比であり比率の期待値でも条件付き確率でもない。分子が分母の部分集合かは検査しない。",
                ),
            ] {
                assumptions.push(ir::Assumption {
                    field: field.into(),
                    value: value.into(),
                    affects: affects.into(),
                });
            }
        }
        for actor in &self.actors {
            if let Some(rule) = &actor.metadata.rule {
                assumptions.push(ir::Assumption { field: format!("{}.rule", actor.name), value: rule.effective_version.clone(), affects: "rule values are fixed for the whole run; in-run amendment and migration are not modeled".into() });
                if let Some(a) = &rule.amendment {
                    assumptions.push(ir::Assumption { field: format!("{}.amendment", actor.name), value: format!("{} -> {}", a.from, a.to), affects: "the amendment's Authority is checked statically under the prior rule; enactment and timing of the amendment are not modeled".into() });
                }
            }
        }
        for (i, authority) in self.authorities.iter().enumerate() {
            let mut recorded = BTreeSet::new();
            for grant in &authority.rule_grants {
                for person in &grant.persons {
                    if !recorded.insert((grant.rule, *person)) {
                        continue;
                    }
                    let used = self.actors[grant.rule.0]
                        .metadata
                        .rule
                        .as_ref()
                        .and_then(|r| r.amendment.as_ref())
                        .is_some_and(|a| a.under.0 == i && a.by == *person);
                    if !used {
                        assumptions.push(ir::Assumption { field: format!("authority.{}.change.{}.{}", authority.name, self.actors[grant.rule.0].name, self.people[person.0].name), value: "unused_in_selected_configuration".into(), affects: "declared rule-change grant is not used by the selected configuration; no enactment or exercise is inferred".into() });
                    }
                }
            }
        }
        if metrics.as_ref().is_some_and(|s| s.breakdown_waits) {
            assumptions.push(ir::Assumption {
                field: "metrics.waiting".into(),
                value: "exclusive_interval_partition_v1".into(),
                affects: "区分は時刻ごとに一つ。external、in_service、contest_route、queue、intentional、unattributedの順で決める。原因は全案件のTaskの容量・資源・人の占有から読む。異なる原因又は既知と不明の混在はcombinedとし、説明できない時間を隠さない。前工程待ちは受入から、first_start_waitは到着から数える。網羅の区分の範囲の和はlead timeの範囲と一致するとは限らない。dwellとhorizonで止めたqueueの和・時間平均のlowerは固定した観測期間内の観測量の印であり、打切り後まで延長した期間の平均の下界を主張しない。".into(),
            });
        }
        if quality.as_ref().is_some_and(|q| !q.quantiles.is_empty()) {
            assumptions.push(ir::Assumption {
                field: "quality.quantile".into(), value: "left-inverse-first-hit-no-interpolation".into(),
                affects: "到着から性質への初到達時間の左の最小整数値で補間しない。全流入は不達を含み、given reachedは到達した質量だけを分母にする。網羅は案件ごとのxごとのmin/maxからの区間で同時の分布ではない。simulationは反復と案件をまとめた点推定であり、案件を独立標本とせず、区間推定でも全スケジュールの検証でもない。打切りで識別できない値は理由つきnullと下界に分ける。".into(),
            });
        }
        if !self.performances.is_empty()
            || self
                .capabilities
                .iter()
                .any(|c| c.qualified_executors.is_some())
        {
            assumptions.push(ir::Assumption { field:"work_profiles".into(), value:"analyst_declared_not_validated; start snapshot".into(),
                affects:"資格はPermissionと別。負荷は開始直前の同Actorの他のQueued Task数で、自分・Running・Held・未到着を含めず、stopped案件のQueuedは含む。固定した成績kernelのgivenは従来の終了時に評価する。実技能・疲労・資格の正当性は認定せず、durationを推測しない。".into() });
        }
        let mut lowered = ir::Model {
            contract_selection,
            case_priority: None,
            arrival_distribution,
            resource_time,
            concurrency_resolutions: self.resolved_concurrency(),
            performances: self.performances,
            fallback_capabilities,
            quality,
            comparison,
            resources: self.resources,
            metrics,
            responsibility: feature.then_some(metadata),
            simulation,
            arrivals,
            arrival_every: self.arrival_every,
            scenario: self.scenario,
            horizon: self.horizon,
            start_policy,
            admission_queue,
            concurrent_cases,
            overflow_end,
            kernels: self.kernels,
            initial_distributions: self.initial_distributions,
            probability_queries: self.probability_queries,
            state_variables: self.state_variables,
            people: self.people,
            authorities: self.authorities,
            contracts,
            agency: crate::agency::Agency::empty().into(),
            name: model.name.value.clone(),
            version: model.version.value.clone(),
            types: self.types,
            actors: self.actors,
            capabilities: self.capabilities,
            process,
            ends: self.ends,
            bounds,
            verify,
            assumptions,
            diagnostics: warnings,
            cut,
        };
        crate::contract_scope::resolve_calls(&mut lowered);
        priority::lower(model, &mut lowered, &self.called_actors)?;
        crate::evidence_basis::prepare(&mut lowered)?;
        work_profiles::supported(&lowered, model.span)?;
        if !lowered.process.degradations.is_empty() {
            lowered.assumptions.push(ir::Assumption {field:"agency.retention.normal.scope".into(),value:"whole_process_paths".into(),
                affects:"既存retentionはprocess全体。degradationだけが側別の構造ガード経路であり、実技能・保持根拠・障害検知を認定しない".into()});
            let provenance: Vec<_> = lowered.process.degradations.iter().map(|r| {
                let c = &lowered.capabilities[r.producer.0];
                let b = &lowered.process.bindings[r.condition.binding.0];
                serde_json::json!({"name":r.name,"condition":{"binding":b.name,"value":lowered.types[b.ty.0].members[r.condition.value.0]},
                    "producer":format!("{}.{}",lowered.actors[c.actor.0].name,c.name)})
            }).collect();
            lowered.assumptions.push(ir::Assumption {
                field: "agency.degradation.declarations".into(),
                value: serde_json::to_string(&provenance).expect("declaration record"),
                affects: "明示条件の生産者対応。障害検知の妥当性は未検証".into(),
            });
        }
        if !lowered.process.degradations.is_empty()
            && (!lowered.contests().is_empty() || lowered.arrival_distribution.is_some())
        {
            return Err(vec![Diagnostic {
                code: U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: "q6_feature_combination: degradation with contest or distributed arrivals"
                    .into(),
                span: Some(model.span),
            }]);
        }
        if lowered.resource_time.is_some() {
            for a in &mut lowered.assumptions {
                a.affects = a.affects
                    .replace("全資源は共通の分析窓で利用可能と仮定する。", "開始可否は宣言カレンダーと勤務表で制限する。")
                    .replace("全資源は共通の分析窓で利用可能と仮定し、休日・交代勤務を模擬しない。", "明示した休みと交代は新規開始の可否だけに作用し、継続中Taskを中断・移管しない。")
                    .replace("全資源が共通の分析窓で利用可能。", "宣言カレンダーが新規開始の枠を制限し、物理capacityは保持の上限として別に残る。");
            }
            lowered.assumptions.push(ir::Assumption {
                field: "resource_time.on_close".into(), value: "continue; resource-time-v1".into(),
                affects: "休み・勤務終了でも実行者・資源・残時間を保持し通常終了又は取消ackまで壁時計で継続する。料金時間外と予定枠超過を別量にする。現実の勤務制度は認定しない。".into(),
            });
        }
        if lowered.has_merges()
            && (!lowered.contests().is_empty()
                || lowered
                    .quality
                    .as_ref()
                    .is_some_and(|q| !q.quantiles.is_empty()))
        {
            let mut ds = lowered.diagnostics;
            ds.push(Diagnostic {
                code: U_SEM_0.into(),
                severity: diag::Severity::Error,
                message:
                    "merge_feature_combination: merge with contest or quantiles is not connected"
                        .into(),
                span: Some(model.span),
            });
            return Err(ds);
        }
        if lowered.has_loops()
            && (!lowered.contests().is_empty()
                || lowered
                    .quality
                    .as_ref()
                    .is_some_and(|q| !q.quantiles.is_empty()))
        {
            let mut ds = lowered.diagnostics;
            ds.push(Diagnostic {
                code: U_SEM_0.into(),
                severity: diag::Severity::Error,
                message:
                    "loop_feature_combination: repeat with contest or quantiles is not connected"
                        .into(),
                span: Some(model.span),
            });
            return Err(ds);
        }
        if !lowered.response_attempts().is_empty()
            && (!lowered.contests().is_empty()
                || lowered
                    .quality
                    .as_ref()
                    .is_some_and(|q| !q.quantiles.is_empty()))
        {
            let recovery = lowered.recovery().is_some();
            let mut ds = lowered.diagnostics;
            ds.push(Diagnostic {code:U_SEM_0.into(),severity:diag::Severity::Error,
                message:if recovery{"recovery_feature_combination: contest or quantiles are unsupported"}else{"retry_feature_combination: receipt/retry with contest or quantiles is unsupported"}.into(),span:Some(model.span)});
            return Err(ds);
        }
        if let Err(error) = crate::actor_table::validate(&lowered) {
            let span = error
                .contains_actor
                .and_then(|id| {
                    let actor = &lowered.actors[id.0];
                    model.items.iter().find_map(|item| match item {
                        ast::Item::Actor(a)
                            if a.span == actor.span && a.name.value == actor.name =>
                        {
                            a.items
                                .iter()
                                .find_map(|field| match field {
                                    ast::ActorItem::Contains(names) => Some(names.span),
                                    _ => None,
                                })
                                .or(Some(a.span))
                        }
                        _ => None,
                    })
                })
                .unwrap_or(model.span);
            return Err(vec![Diagnostic {
                code: error.code.into(),
                severity: diag::Severity::Error,
                message: error.message.into(),
                span: Some(span),
            }]
            .into_iter()
            .chain(lowered.diagnostics)
            .collect());
        }
        if lowered.authorities.iter().any(|a| {
            a.restrictions.restricted()
                || a.restrictions.revocable_by.is_some()
                || !a.rule_grants.is_empty()
                || a.under.is_some()
        }) || lowered.actors.iter().any(|a| a.metadata.rule.is_some())
        {
            for a in &lowered.authorities {
                if let Some(p) = a.issued_by
                    && (a.grants.iter().any(|g| g.persons.contains(&p))
                        || a.rule_grants.iter().any(|g| g.persons.contains(&p)))
                {
                    lowered.diagnostics.push(Diagnostic {
                        code: diag::W_AUTH_0.into(),
                        severity: diag::Severity::Warning,
                        message: format!(
                            "Authority {} grants to its issuer {}",
                            a.name, lowered.people[p.0].name
                        ),
                        span: a.restrictions.issuer_span,
                    });
                }
            }
        }
        for declared_amendment in selected.into_iter().flat_map(|c| &c.amendments) {
            let id = lowered
                .actors
                .iter()
                .position(|a| a.name == declared_amendment.rule.value)
                .expect("validated amendment Actor");
            let actor = &lowered.actors[id];
            if let Some(amendment) = actor
                .metadata
                .rule
                .as_ref()
                .and_then(|r| r.amendment.as_ref())
            {
                for authority in &lowered.authorities {
                    if authority
                        .under
                        .as_ref()
                        .is_some_and(|b| b.rule.0 == id && b.version == amendment.to)
                        && (authority
                            .rule_grants
                            .iter()
                            .any(|g| g.persons.contains(&amendment.by))
                            || authority
                                .grants
                                .iter()
                                .any(|g| g.persons.contains(&amendment.by)))
                    {
                        lowered.diagnostics.push(Diagnostic {
                            code: diag::W_AUTH_0.into(),
                            severity: diag::Severity::Warning,
                            message: format!(
                                "amendment of {} enables Authority {} granting to amender {}",
                                actor.name, authority.name, lowered.people[amendment.by.0].name
                            ),
                            span: Some(amendment.span),
                        });
                    }
                }
            }
        }
        if let Some(meta) = &lowered.responsibility {
            let calls = crate::responsibility::calls(&lowered);
            for s in &meta.standings {
                if let crate::contest::ContestTarget::Commit(cap) = s.target {
                    let message = if calls.values().filter(|c| c.capability == cap).count() > 1 {
                        Some("effect contest with multiple commit sites is unsupported")
                    } else if meta.contests.iter().any(|c| c.route == cap) {
                        Some("effect target used as contest route is unsupported")
                    } else {
                        None
                    };
                    if let Some(message) = message {
                        return Err(vec![Diagnostic {
                            code: U_SEM_0.into(),
                            severity: diag::Severity::Error,
                            message: message.into(),
                            span: Some(s.span),
                        }]);
                    }
                }
            }
        }
        if let Some(diagnostic) = crate::contest::condition_wait_refusal(&lowered) {
            return Err(vec![diagnostic]);
        }
        if let Some((span, name)) = crate::contest::restricted_authority_reference(&lowered) {
            return Err(vec![Diagnostic {
                code: U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: format!("contest cannot reference restricted Authority {name}"),
                span: Some(span),
            }]);
        }
        match crate::agency::derive_for_model(&lowered) {
            Ok((agency, warnings)) => {
                lowered.agency = agency;
                fn has_kernel(b: &ir::Block, m: &ir::Model) -> bool {
                    b.iter().any(|s| match &s.kind {
                        ir::Stmt::Call(c) | ir::Stmt::Response(c, _) => {
                            m.capabilities[c.capability.0].structural_kernel().is_some()
                        }
                        ir::Stmt::Fork { branches, .. } => branches
                            .iter()
                            .any(|c| m.capabilities[c.capability.0].structural_kernel().is_some()),
                        ir::Stmt::If {
                            then_body,
                            else_body,
                            ..
                        } => has_kernel(then_body, m) || has_kernel(else_body, m),
                        _ => false,
                    })
                }
                if has_kernel(&lowered.process.body, &lowered) {
                    lowered
                        .agency
                        .unchecked_mut()
                        .push("kernel_correlations".into());
                }
                if self.multi_case {
                    lowered
                        .agency
                        .unchecked_mut()
                        .push("multi_case_interference".into());
                }
                lowered.diagnostics.extend(warnings);
                if let Err(error) = crate::responsibility::derive(&mut lowered) {
                    lowered.diagnostics.push(error);
                    return Err(lowered.diagnostics);
                }
                if let Some(meta) = &lowered.responsibility {
                    let effects = crate::contest::has_effects(&lowered);
                    for (present, field, value, affects) in [
                        (
                            !meta.contests.is_empty(),
                            "contest.scope",
                            "model_structure_and_runtime_not_acceptance",
                            if effects {
                                "モデル内の構造と応答を扱う。当事者の受諾、実在のStanding、再審と救済の検証ではない。効果の異議は commit の時点で起票し、route は process の残りの作業と同じ容量と人を取り合う。"
                            } else {
                                "モデル内の構造と応答を扱う。当事者の受諾、実在のStanding、再審と救済の検証ではない。"
                            },
                        ),
                        (
                            !meta.contests.is_empty(),
                            "contest.occurrence",
                            if meta.contests.iter().any(|c| c.rates.is_some()) {
                                "configured_bernoulli_or_unconfigured_nondeterministic_at_target_end"
                            } else if effects {
                                "nondeterministic_at_target_event"
                            } else {
                                "nondeterministic_at_target_end"
                            },
                            if meta.contests.iter().any(|c| c.rates.is_some()) {
                                "ratesを宣言した句だけ対象終端で一度の条件付きBernoulli起票。他の句は非決定のまま。根拠は申告であり実当事者の頻度や受諾を認定しない。"
                            } else if effects {
                                "対象の終端または commit で起票と不起票の両方を扱う。発生率は仮定しない。"
                            } else {
                                "対象終端で起票と不起票の両方を扱う。発生率は仮定しない。"
                            },
                        ),
                        (
                            !meta.contests.is_empty(),
                            "contest.response",
                            "route_normal_finish_by_target_time_plus_deadline",
                            "モデル内の通常終了を応答とする。取消完了は応答ではない。",
                        ),
                        (
                            !meta.contests.is_empty(),
                            "contest.closure",
                            "business_end_distinct_from_execution_close",
                            if effects {
                                "業務上の終端をその End の時刻で記録し、全ての異議の応答まで実行を継続する。効果の異議との競合は End を遅らせうるが、終端の結果を救済として書き換えない。business_ended_open は業務 End の時点で異議が開いていた案件を数える。"
                            } else {
                                "業務上の終端の時刻と結果は異議の作業によって変えない。"
                            },
                        ),
                        (
                            !meta.contests.is_empty(),
                            "contest.reversal",
                            "exists-decision-per-person",
                            if effects {
                                "各人について、対象の終端または効果を直接決める判断の候補のどれか一つに実行資格・Permission・Authorityがあることを確かめる。効果自身は atomic または複数結果のときだけ候補になる。全ての原因を覆せることと、routeから人への実際の受け渡しは確かめない。"
                            } else {
                                "各人について、対象終端を直接決める判断の候補のどれか一つに実行資格・Permission・Authorityがあることを確かめる。全ての原因を覆せることと、routeから人への実際の受け渡しは確かめない。"
                            },
                        ),
                        (
                            !meta.duties.is_empty(),
                            "responsibility.scope",
                            "static-origin-refusal",
                            "責務は規範であり、観測と拒否の到達を報告する。因果的な寄与量や本人の落ち度の判定ではない。",
                        ),
                        (
                            !meta.duties.is_empty(),
                            "responsibility.time_frame",
                            if meta.duties.iter().any(|d| d.within.is_some()) {
                                if meta
                                    .responsibilities
                                    .iter()
                                    .any(|r| r.time_frame.status == "unchecked")
                                {
                                    "mixed"
                                } else {
                                    "static_upper_bound"
                                }
                            } else {
                                "unchecked"
                            },
                            if meta.duties.iter().any(|d| d.within.is_some()) {
                                "明示within行だけ、直接観測と判定から効果への到達経路のCapability上限の静的な和を比較する。資源待ち・process待機・取消・外部受け渡し・修復の実行SLAを含まない。未検査の行は合格扱いしない。"
                            } else {
                                "責務を説明・停止・修復するための時間の枠は導出していない。"
                            },
                        ),
                    ] {
                        if present {
                            lowered.assumptions.push(ir::Assumption {
                                field: field.into(),
                                value: value.into(),
                                affects: affects.into(),
                            });
                        }
                    }
                    for c in &meta.contests {
                        let Some(rates) = &c.rates else { continue };
                        let route = crate::responsibility::cap_name(&lowered, c.route);
                        for (suffix, value, affects) in [
                            (
                                "occurrence_basis",
                                rates.occurrence_basis.clone(),
                                "起票根拠の原文。資料を開かず、実測・妥当性・実当事者の代理や受諾を認定しない。",
                            ),
                            (
                                "service_time_basis",
                                rates.service_time_basis.clone(),
                                "routeの既存durationへの申告根拠。待ち・応答期限の根拠や新しい時間分布ではない。",
                            ),
                            (
                                "independence",
                                rates.independence.clone(),
                                "対象到達を条件に他案件・他句・過去の履歴・routeの結果と所要時間から独立という分析仮定。",
                            ),
                            (
                                "validation",
                                "analyst-declared-not-validated".into(),
                                "根拠を入力してもCutの保留を自動解除しない。実在のStandingや受諾の認定ではない。",
                            ),
                            (
                                "probability",
                                rates.probability.to_string(),
                                "P(filed given target)。流入当たり率でも時間当たりhazardでもない。",
                            ),
                            (
                                "target",
                                format!("end.{}", c.target.name(&lowered)),
                                "この終端への到達時に一度だけ即時起票を選ぶ。",
                            ),
                            (
                                "service_time",
                                format!("{route}.duration"),
                                "通常終了までの既存作業時間。queueやresponse_elapsedと区別し、上書きしない。",
                            ),
                        ] {
                            lowered.assumptions.push(ir::Assumption {
                                field: format!("contest.{}.{}", c.ordinal, suffix),
                                value,
                                affects: affects.into(),
                            });
                        }
                    }
                }
                if !lowered.contests().is_empty() {
                    let mut feature = Vec::new();
                    lowered.assumptions.retain(|a| {
                        if a.field.starts_with("contest.") || a.field.starts_with("responsibility.")
                        {
                            feature.push(a.clone());
                            false
                        } else {
                            true
                        }
                    });
                    let at = lowered
                        .assumptions
                        .iter()
                        .position(|a| {
                            a.field == "arrivals"
                                || a.field.starts_with("metrics.")
                                || a.field.starts_with("simulation.")
                        })
                        .unwrap_or(lowered.assumptions.len());
                    lowered.assumptions.splice(at..at, feature);
                    if let Some(at) = lowered
                        .agency
                        .unchecked()
                        .iter()
                        .position(|s| s == "multi_case_interference")
                    {
                        let word = lowered.agency.unchecked_mut().remove(at);
                        lowered.agency.unchecked_mut().push(word);
                    }
                }
                // New responsibility assumptions precede the duration block,
                // whose closed wire ordering already reserves the trailing
                // positions for arrivals/scheduler. Preserve all legacy order.
                if lowered.responsibility.as_ref().is_some_and(|meta| {
                    meta.duties.iter().any(|d| {
                        d.within.is_some()
                            || matches!(
                                d.target,
                                crate::responsibility::Target::Formula(_)
                                    | crate::responsibility::Target::Maintenance(_)
                            )
                    })
                }) {
                    let mut feature = Vec::new();
                    lowered.assumptions.retain(|a| {
                        if a.field.starts_with("responsibility.") {
                            feature.push(a.clone());
                            false
                        } else {
                            true
                        }
                    });
                    let at = lowered
                        .assumptions
                        .iter()
                        .position(|a| {
                            a.field.starts_with("duration.")
                                || a.field == "probability.scheduler"
                                || a.field == "arrivals"
                                || a.field.starts_with("metrics.")
                                || a.field.starts_with("simulation.")
                        })
                        .unwrap_or(lowered.assumptions.len());
                    lowered.assumptions.splice(at..at, feature);
                }
                if lowered.resource_time.is_some() {
                    lowered
                        .agency
                        .unchecked_mut()
                        .push("calendar_time_feasibility_not_derived".into());
                }
                if lowered.has_condition_wait() {
                    lowered
                        .agency
                        .unchecked_mut()
                        .push("condition_wait_feasibility_not_derived".into());
                }
                if let Some(d) = &lowered.arrival_distribution {
                    for row in &mut lowered.assumptions {
                        if row.field == "arrivals" {
                            row.value = "finite_complete_schedule_distribution".into();
                            row.affects = "根拠つき有限コホートの完全到着列を各反復で一回抽選する。固定列や無限到着過程とは呼ばず、全offered件数を分母にする。simulation限定。".into();
                        }
                    }
                    lowered.assumptions.push(ir::Assumption {
                        field: "arrivals.basis".into(),
                        value: d.basis.clone(),
                        affects: "入力の仮定であり実測や一様分布の既定値ではない。".into(),
                    });
                }
                Ok(lowered)
            }
            Err(error) => {
                lowered.diagnostics.push(error);
                Err(lowered.diagnostics)
            }
        }
    }

    fn contest_rates(&mut self, rates: &ast::ContestRates) -> Option<crate::contest::Rates> {
        let probability = self.probability(&rates.probability);
        if probability.as_ref().is_some_and(|p| p.is_zero()) {
            self.error(
                diag::E_PROB_0,
                "contest_probability_zero_use_cut_or_excludes: use Cut or excludes instead of erasing the contest branch",
                rates.probability.span,
            );
        }
        for basis in [&rates.occurrence_basis, &rates.service_time_basis] {
            if basis.value.trim().is_empty() {
                self.error(
                    diag::E_CON_0,
                    "contest rates basis must not be blank",
                    basis.span,
                );
            }
        }
        if rates.independence.value != "per_case_per_clause" {
            self.unsupported(
                "contest rates independence must be per_case_per_clause",
                rates.independence.span,
            );
        }
        probability.map(|probability| crate::contest::Rates {
            probability,
            occurrence_basis: rates.occurrence_basis.value.clone(),
            service_time_basis: rates.service_time_basis.value.clone(),
            independence: rates.independence.value.clone(),
        })
    }

    fn person(&mut self, name: &ast::Ident) -> Option<ir::PersonId> {
        let id = self.person_names.get(&name.value).copied();
        if id.is_none() {
            self.error(E_NAME_0, "undefined person", name.span);
        }
        id
    }

    fn persons(&mut self, names: &[ast::Ident], span: Span) -> Vec<ir::PersonId> {
        self.unique_members(names, span);
        let mut persons: Vec<_> = names.iter().filter_map(|n| self.person(n)).collect();
        persons.sort();
        persons.dedup();
        persons
    }

    fn standing_target(
        &mut self,
        target: &ast::StandingRef,
    ) -> Option<crate::contest::ContestTarget> {
        use crate::contest::ContestTarget;
        match target {
            ast::StandingRef::End(n) => self.end(n).map(ContestTarget::End),
            ast::StandingRef::Commit(p) => {
                let previous = self.errors.len();
                let cap = self.capability_path(p);
                if self.errors.len() > previous
                    && self.errors[..previous].iter().any(|d| {
                        d.code == self.errors[previous].code
                            && d.message == self.errors[previous].message
                    })
                {
                    self.errors.truncate(previous);
                }
                let cap = cap?;
                if !self.capabilities[cap.0].commits {
                    if !self.errors.iter().any(|d| {
                        d.code == diag::E_CON_0
                            && d.message == "commit requires a commits Capability"
                    }) {
                        self.error(
                            diag::E_CON_0,
                            "commit requires a commits Capability",
                            p.span,
                        );
                    }
                    return None;
                }
                Some(ContestTarget::Commit(cap))
            }
        }
    }
    fn capability_path(&mut self, path: &ast::CapabilityPath) -> Option<ir::CapabilityId> {
        let id = self
            .cap_names
            .get(&(path.actor.value.clone(), path.capability.value.clone()))
            .copied();
        if id.is_none() {
            self.error(E_NAME_0, "undefined Actor.Capability", path.span);
        }
        id
    }

    fn named_target(&mut self, cap: ir::CapabilityId, span: Span) {
        if self.actors[self.capabilities[cap.0].actor.0]
            .executors
            .is_none()
        {
            self.unsupported("executor identity is required", span);
        }
    }

    fn authority_declarations(&mut self, model: &ast::Model) {
        // Register names without issuing diagnostics here: legacy diagnostic order
        // remains source order, while delegation permits forward references.
        let declarations: Vec<_> = model
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Authority(a) => Some(a),
                _ => None,
            })
            .collect();
        let mut authority_lookup = BTreeMap::new();
        for (i, a) in declarations.iter().enumerate() {
            authority_lookup
                .entry(a.name.value.clone())
                .or_insert(ir::AuthorityId(i));
        }
        let mut binding_names = BTreeSet::new();
        for item in &model.items {
            if let ast::Item::Process(p) = item {
                let mut calls = Vec::new();
                collect_calls(&p.body, &mut calls);
                binding_names.extend(calls.iter().map(|c| c.binding.value.clone()));
            }
        }
        for item in &model.items {
            if let ast::Item::Permit(p) = item {
                let people = self.persons(&p.persons, p.span);
                if let Some(cap) = self.capability_path(&p.target) {
                    self.named_target(cap, p.span);
                    let permission = self.capabilities[cap.0]
                        .permission
                        .get_or_insert_with(Vec::new);
                    permission.extend(people);
                    permission.sort();
                    permission.dedup();
                }
            }
            if let ast::Item::Authority(a) = item {
                let id = ir::AuthorityId(self.authorities.len());
                if self.authority_names.contains_key(&a.name.value) {
                    self.unsupported("duplicate Authority", a.name.span);
                }
                self.authority_names
                    .entry(a.name.value.clone())
                    .or_insert(id);
                let (mut basis, mut issued_by, mut not_by) = (None, None, None);
                let mut grants = Vec::new();
                let mut rule_grants = Vec::new();
                let mut under = None;
                let mut grant_seen = false;
                let mut restrictions = ir::AuthorityRestrictions::default();
                let mut seen = BTreeSet::new();
                for field in &a.items {
                    let new_field = match &field.value {
                        ast::AuthorityItem::Scope(_) => Some("scope"),
                        ast::AuthorityItem::Expires(_) => Some("expires"),
                        ast::AuthorityItem::RevocableBy(_) => Some("revocable_by"),
                        ast::AuthorityItem::DelegatedFrom(_) => Some("delegated_from"),
                        _ => None,
                    };
                    if let Some(name) = new_field
                        && !seen.insert(name)
                    {
                        self.unsupported(format!("duplicate {name}"), field.span);
                        continue;
                    }
                    match &field.value {
                        ast::AuthorityItem::ChangeGrants { persons, rule } => {
                            grant_seen = true;
                            let persons = self.persons(persons, field.span);
                            if let Some(&id) = self.actor_names.get(&rule.value) {
                                if self.actors[id.0].metadata.rule.is_none() {
                                    self.error(
                                        diag::E_CON_0,
                                        "change grant requires a rule Actor",
                                        rule.span,
                                    );
                                } else {
                                    rule_grants.push(ir::RuleGrant { rule: id, persons });
                                }
                            } else {
                                self.error(E_NAME_0, "undefined rule Actor", rule.span);
                            }
                        }
                        ast::AuthorityItem::Under { rule, version } => {
                            if under.is_some() {
                                self.unsupported("duplicate under", field.span);
                            }
                            if let Some(&id) = self.actor_names.get(&rule.value) {
                                if self.actors[id.0].metadata.rule.is_none() {
                                    self.error(
                                        diag::E_CON_0,
                                        "version binding requires a rule Actor",
                                        rule.span,
                                    );
                                } else {
                                    let known = self.actors[id.0].metadata.rule.as_ref().is_some_and(|r| r.declared_version == version.value)
                                        || model.items.iter().any(|i| matches!(i, ast::Item::Configuration(c) if c.amendments.iter().any(|a| a.rule.value == rule.value && a.to.value == version.value)));
                                    if !known {
                                        self.error(
                                            E_NAME_0,
                                            "undefined rule version",
                                            version.span,
                                        );
                                    }
                                    under = Some(ir::RuleBasis {
                                        rule: id,
                                        version: version.value.clone(),
                                        span: version.span,
                                    });
                                }
                            } else {
                                self.error(E_NAME_0, "undefined rule Actor", rule.span);
                            }
                        }
                        ast::AuthorityItem::Scope(c) => {
                            restrictions.scope_span = Some(field.span);
                            restrictions.scope = self.authority_scope(c, &binding_names);
                        }
                        ast::AuthorityItem::Expires(d) => {
                            restrictions.expires_span = Some(field.span);
                            restrictions.expires = self.ticks(d);
                            restrictions.effective_expires = restrictions.expires;
                        }
                        ast::AuthorityItem::RevocableBy(n) => {
                            restrictions.revocable_span = Some(field.span);
                            restrictions.revocable_by = match (
                                self.person_names.get(&n.value),
                                self.actor_names.get(&n.value),
                            ) {
                                (Some(_), Some(_)) => {
                                    self.error(
                                        diag::E_CON_0,
                                        "ambiguous revoker person/Actor",
                                        n.span,
                                    );
                                    None
                                }
                                (Some(p), None) => Some(ir::AuthorityRevoker::Person(*p)),
                                (None, Some(a)) => Some(ir::AuthorityRevoker::Actor(*a)),
                                _ => {
                                    self.error(E_NAME_0, "undefined revoker", n.span);
                                    None
                                }
                            };
                        }
                        ast::AuthorityItem::DelegatedFrom(n) => {
                            restrictions.delegated_span = Some(field.span);
                            restrictions.delegated_from = authority_lookup.get(&n.value).copied();
                            if restrictions.delegated_from.is_none() {
                                self.error(E_NAME_0, "undefined delegation Authority", n.span);
                            }
                        }
                        ast::AuthorityItem::Basis(s) => {
                            self.set_once(&mut basis, s.value.clone(), "basis", field.span)
                        }
                        ast::AuthorityItem::IssuedBy(n) => {
                            restrictions.issuer_span = Some(field.span);
                            let person = self.person(n);
                            self.set_once(&mut issued_by, person, "issued_by", field.span);
                        }
                        ast::AuthorityItem::NotBy(names) => {
                            let persons = self.persons(names, field.span);
                            self.set_once(&mut not_by, persons, "not_by", field.span);
                        }
                        ast::AuthorityItem::Grants { persons, target } => {
                            grant_seen = true;
                            let persons = self.persons(persons, field.span);
                            if let Some(capability) = self.capability_path(target) {
                                grants.push(ir::Grant {
                                    capability,
                                    persons,
                                });
                            }
                        }
                    }
                }
                if basis.is_none() || !grant_seen {
                    self.error(diag::E_SYN_0, "Authority requires basis and grants", a.span);
                }
                let issued_by = issued_by.flatten();
                if !rule_grants.is_empty() && restrictions.restricted() {
                    self.unsupported("restricted rule-change Authority is unsupported", a.span);
                }
                if under.is_some() && !grants.is_empty() {
                    self.unsupported(
                        "version-bound Authority with Capability grants is unsupported",
                        a.span,
                    );
                }
                let not_by = not_by.unwrap_or_default();
                if issued_by.is_some_and(|p| not_by.contains(&p))
                    || grants
                        .iter()
                        .any(|g| g.persons.iter().any(|p| not_by.contains(p)))
                {
                    self.error(
                        diag::E_CON_0,
                        "not_by conflicts with grants or issuer",
                        a.span,
                    );
                }
                self.authorities.push(ir::Authority {
                    rule_grants,
                    under,
                    name: a.name.value.clone(),
                    basis: basis.unwrap_or_default(),
                    grants,
                    issued_by,
                    not_by,
                    restrictions,
                    span: a.span,
                });
            }
        }
        self.authority_delegations(&declarations);
    }

    fn authority_scope(
        &mut self,
        c: &ast::Spanned<ast::Cond>,
        bindings: &BTreeSet<String>,
    ) -> Option<ir::Cond> {
        if self.merge_condition_forbidden(c) {
            return None;
        }
        match &c.value {
            ast::Cond::Contains { element, set } => self.membership(element, set, None),
            ast::Cond::Compare {
                binding,
                operator,
                value,
            } => {
                self.policy_condition(c);
                self.integer_comparison(binding, operator, value)
            }
            ast::Cond::Eq { binding, value } => {
                let Some(&id) = self.state_names.get(&binding.value) else {
                    if bindings.contains(&binding.value) {
                        self.unsupported(
                            "authority scope cannot read process bindings",
                            binding.span,
                        );
                    } else {
                        self.error(E_NAME_0, "undefined authority scope variable", binding.span);
                    }
                    return None;
                };
                let v = &self.state_variables[id.0];
                if v.kind != ir::StateKind::ObservedCase {
                    self.error(
                        diag::E_POL_LATENT,
                        "authority scope cannot read latent state",
                        binding.span,
                    );
                    return None;
                }
                // Unlike process conditions the diagnostic identifies the variable
                // whose declared type conflicts with the scope comparison.
                if v.integer_max.is_some() {
                    return self.state_comparison(id, value);
                }
                let ty = v.base;
                let optional = v.optional;
                let valid = (optional && value.value == "none")
                    || self
                        .types
                        .get(ty.0)
                        .is_some_and(|t| t.members.contains(&value.value));
                if !valid {
                    self.error(
                        diag::E_TYPE_0,
                        "authority scope value/type mismatch",
                        binding.span,
                    );
                    return None;
                }
                self.state_comparison(id, value)
            }
            ast::Cond::LocalEq {
                actor,
                variable,
                operator,
                value,
            } => {
                if let Some(c) = self.record_comparison(
                    actor,
                    variable,
                    operator,
                    value,
                    RecordContext::StateOnly,
                ) {
                    return c;
                }
                let id = self.local_state(actor, variable)?;
                if operator.value != ast::IntComparison::Eq
                    && self.state_variables[id.0].integer_max.is_none()
                {
                    return self.resolved_comparison(id, operator, value, true);
                }
                if self.state_variables[id.0].kind.is_latent() {
                    self.error(
                        diag::E_POL_LATENT,
                        "authority scope cannot read Actor latent state",
                        c.span,
                    );
                    return None;
                }
                self.resolved_comparison(id, operator, value, true)
            }
            ast::Cond::Group(p) => self.authority_scope(p, bindings),
            ast::Cond::And(parts) | ast::Cond::Or(parts) => {
                if parts.len() < 2 {
                    self.unsupported("scope chain requires two operands", c.span);
                }
                let checked: Vec<_> = parts
                    .iter()
                    .map(|p| self.authority_scope(p, bindings))
                    .collect();
                checked.into_iter().collect::<Option<Vec<_>>>().map(|ps| {
                    if matches!(c.value, ast::Cond::And(_)) {
                        ir::Cond::And(ps)
                    } else {
                        ir::Cond::Or(ps)
                    }
                })
            }
        }
    }

    fn authority_delegations(&mut self, declarations: &[&ast::AuthorityDecl]) {
        let mut color = vec![0u8; self.authorities.len()];
        for root in 0..self.authorities.len() {
            if color[root] != 0 {
                continue;
            }
            let mut path = Vec::new();
            let mut next = Some(ir::AuthorityId(root));
            let mut invalid = false;
            while let Some(id) = next {
                if color[id.0] == 1 {
                    let names = path
                        .iter()
                        .map(|i: &usize| self.authorities[*i].name.as_str())
                        .collect::<Vec<_>>()
                        .join(" -> ");
                    let edge = *path.last().unwrap();
                    self.error(
                        diag::E_CON_0,
                        format!(
                            "Authority delegation cycle: {names} -> {}",
                            self.authorities[id.0].name
                        ),
                        self.authorities[edge].restrictions.delegated_span.unwrap(),
                    );
                    invalid = true;
                    break;
                }
                if color[id.0] >= 2 {
                    invalid = color[id.0] == 3;
                    break;
                }
                color[id.0] = 1;
                path.push(id.0);
                next = self.authorities[id.0].restrictions.delegated_from;
            }
            for i in path.into_iter().rev() {
                color[i] = if invalid { 3 } else { 2 };
                if invalid {
                    continue;
                }
                if !self.authorities[i].rule_grants.is_empty() {
                    continue;
                }
                let Some(parent_id) = self.authorities[i].restrictions.delegated_from else {
                    continue;
                };
                let parent = self.authorities[parent_id.0].clone();
                let child = self.authorities[i].clone();
                let d = declarations[i];
                let link_span = child.restrictions.delegated_span.unwrap_or(d.span);
                let issuer_span = child.restrictions.issuer_span.unwrap_or(link_span);
                let mut effective_not_by = child.not_by.clone();
                effective_not_by.extend(&parent.not_by);
                effective_not_by.sort();
                effective_not_by.dedup();
                self.authorities[i].not_by = effective_not_by.clone();
                if let (Some(c), Some(p)) = (
                    child.restrictions.expires,
                    parent.restrictions.effective_expires,
                ) && c > p
                {
                    self.error(
                        diag::E_CON_0,
                        "delegated expiry exceeds parent Authority",
                        child.restrictions.expires_span.unwrap_or(link_span),
                    );
                }
                self.authorities[i].restrictions.effective_expires = child
                    .restrictions
                    .expires
                    .or(parent.restrictions.effective_expires);
                let has = |cap, p| {
                    parent
                        .grants
                        .iter()
                        .any(|g| g.capability == cap && g.persons.contains(&p))
                };
                if child
                    .issued_by
                    .is_none_or(|p| child.grants.iter().any(|g| !has(g.capability, p)))
                {
                    self.error(
                        diag::E_CON_0,
                        format!(
                            "delegation issuer requires each capability grant from {}",
                            parent.name
                        ),
                        issuer_span,
                    );
                } else if child
                    .issued_by
                    .is_some_and(|p| effective_not_by.contains(&p))
                {
                    self.error(
                        diag::E_CON_0,
                        format!("issuer forbidden by parent Authority {}", parent.name),
                        issuer_span,
                    );
                }
                for f in &d.items {
                    if let ast::AuthorityItem::Grants { persons, target } = &f.value {
                        let Some(&cap) = self
                            .cap_names
                            .get(&(target.actor.value.clone(), target.capability.value.clone()))
                        else {
                            continue;
                        };
                        if !parent.grants.iter().any(|g| g.capability == cap) {
                            self.error(
                                diag::E_CON_0,
                                format!(
                                    "grant capability outside parent Authority {}",
                                    parent.name
                                ),
                                target.span,
                            );
                            continue;
                        }
                        for n in persons {
                            let Some(&p) = self.person_names.get(&n.value) else {
                                continue;
                            };
                            if !has(cap, p) || effective_not_by.contains(&p) {
                                self.error(
                                    diag::E_CON_0,
                                    format!(
                                        "grant outside or forbidden by parent Authority {}",
                                        parent.name
                                    ),
                                    n.span,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn resolve_waivers(
        &mut self,
        model: &ast::Model,
        contracts: &mut [ir::ContractClause],
        selection: Option<&str>,
    ) {
        let mut labels = BTreeMap::new();
        for (i, c) in contracts.iter().enumerate() {
            if let ir::ContractKind::NamedResponse {
                label: Some(label), ..
            } = &c.kind
            {
                let anonymous_scope = if c.name.is_none() {
                    model.items.iter().find_map(|item| match item {
                        ast::Item::Contract(d)
                            if d.span.start <= c.span.start && c.span.end <= d.span.end =>
                        {
                            Some(d.span.start)
                        }
                        _ => None,
                    })
                } else {
                    None
                };
                if labels
                    .insert((c.name.clone(), anonymous_scope, label.clone()), i)
                    .is_some()
                {
                    self.unsupported("duplicate response label", c.span);
                }
            }
        }
        let mut waivers = BTreeSet::new();
        for item in &model.items {
            let ast::Item::Contract(decl) = item else {
                continue;
            };
            for clause in &decl.clauses {
                let ast::ContractClause::Waive { response, .. } = &clause.value else {
                    continue;
                };
                if !crate::contract_scope::applies(decl, selection) {
                    continue;
                }
                let name = if response.len() == 2 {
                    Some(response[0].value.clone())
                } else {
                    decl.name.as_ref().map(|n| n.value.clone())
                };
                let label = response.last().expect("nonempty response reference");
                let anonymous_scope = name.is_none().then_some(decl.span.start);
                let Some(&target) = labels.get(&(name, anonymous_scope, label.value.clone()))
                else {
                    self.error(E_NAME_0, "undefined response reference", label.span);
                    continue;
                };
                if matches!(
                    contracts[target].kind,
                    ir::ContractKind::NamedResponse {
                        non_waivable: true,
                        ..
                    }
                ) {
                    self.error(diag::E_CON_0, "response is non_waivable", clause.span);
                }
                if let Some(c) = contracts.iter_mut().find(|c| c.span == clause.span)
                    && let ir::ContractKind::Waive {
                        response,
                        event,
                        reason,
                        authority,
                    } = &mut c.kind
                {
                    *response = ir::ClauseId(target);
                    let key = format!("{target}:{event:?}:{reason}:{authority:?}");
                    if !waivers.insert(key) {
                        self.unsupported("duplicate waiver declaration", clause.span);
                    }
                }
            }
        }
    }

    fn contract_event(&mut self, event: &ast::Event) -> Option<ir::Event> {
        self.contract_event_context(event, false)
    }

    fn contract_event_context(
        &mut self,
        event: &ast::Event,
        cancel_ack: bool,
    ) -> Option<ir::Event> {
        if matches!(event.kind, ast::EventKind::CancelAck) && !cancel_ack {
            self.error(
                diag::E_CON_0,
                "cancel_ack is only an excludes antecedent",
                event.span,
            );
            return None;
        }
        if !self.actor_names.contains_key(&event.actor.value) {
            self.error(E_NAME_0, "undefined contract Actor", event.actor.span);
            return None;
        }
        let Some(&capability) = self
            .cap_names
            .get(&(event.actor.value.clone(), event.capability.value.clone()))
        else {
            self.error(
                E_NAME_0,
                "undefined contract Capability",
                event.capability.span,
            );
            return None;
        };
        let kind = match &event.kind {
            ast::EventKind::CancelAck => ir::EventKind::CancelAck,
            ast::EventKind::Start => ir::EventKind::Start,
            ast::EventKind::Commit => {
                if !self.capabilities[capability.0].commits {
                    self.error(
                        diag::E_CON_0,
                        "commit requires a commits Capability",
                        event.span,
                    );
                }
                ir::EventKind::Commit
            }
            ast::EventKind::Finish(value) => {
                let ty = self.capabilities[capability.0].return_type;
                let value = match value {
                    Some(v) if self.known_cap_types.contains(&capability) => {
                        Some(self.value(ty, v)?)
                    }
                    Some(_) => return None,
                    None => None,
                };
                ir::EventKind::Finish(value)
            }
        };
        Some(ir::Event { capability, kind })
    }

    fn contract_trigger(&mut self, trigger: &ast::Trigger) -> Option<ir::Trigger> {
        match trigger {
            ast::Trigger::Received => Some(ir::Trigger::Received),
            ast::Trigger::Admitted(span) => {
                if self.admitted_allowed {
                    Some(ir::Trigger::Admitted)
                } else {
                    self.error(
                        diag::U_SYN_0,
                        "case.admitted requires external wait or overflow",
                        *span,
                    );
                    None
                }
            }
            ast::Trigger::Event(e) => self.contract_event(e).map(ir::Trigger::Event),
        }
    }

    fn contract_clause(&mut self, clause: &ast::ContractClause) -> Option<ir::ContractKind> {
        match clause {
            ast::ContractClause::Excludes { before, start } => {
                let before = self.contract_event_context(before, true)?;
                let resolved = self.contract_event(start)?;
                if resolved.kind != ir::EventKind::Start {
                    self.error(
                        diag::E_CON_0,
                        "excludes requires a start target",
                        start.span,
                    );
                }
                Some(ir::ContractKind::Excludes {
                    before,
                    start: resolved,
                })
            }
            ast::ContractClause::Waive {
                event,
                reason,
                authority,
                ..
            } => {
                let resolved = self.contract_event(event)?;
                if !matches!(
                    resolved.kind,
                    ir::EventKind::Finish(_) | ir::EventKind::Commit
                ) {
                    self.error(
                        diag::E_CON_0,
                        "waiver requires normal completion",
                        event.span,
                    );
                }
                let Some(&aid) = self.authority_names.get(&authority.value) else {
                    self.error(E_NAME_0, "undefined waiver Authority", authority.span);
                    return None;
                };
                if self.authorities[aid.0].restrictions.restricted() {
                    self.unsupported(
                        "waive cannot reference restricted Authority",
                        authority.span,
                    );
                }
                let actor = &self.actors[self.capabilities[resolved.capability.0].actor.0];
                let authorized = actor.executors.as_ref().is_some_and(|ps| {
                    !ps.is_empty()
                        && ps.iter().all(|p| {
                            self.authorities[aid.0].grants.iter().any(|g| {
                                g.capability == resolved.capability && g.persons.contains(p)
                            })
                        })
                });
                if !authorized {
                    self.error(
                        diag::E_CON_0,
                        "waiver requires a grant for every executor",
                        event.span,
                    );
                }
                Some(ir::ContractKind::Waive {
                    response: ir::ClauseId(usize::MAX),
                    event: resolved,
                    reason: reason.value.clone(),
                    authority: aid,
                })
            }
            ast::ContractClause::Contest { .. } => None,
            ast::ContractClause::Separation { left, right } => {
                let l = self.contract_event(left);
                let r = self.contract_event(right);
                let (l, r) = (l?, r?);
                self.named_target(l.capability, left.span);
                self.named_target(r.capability, right.span);
                if l == r {
                    self.error(diag::E_CON_0, "identical separation events", left.span);
                }
                Some(ir::ContractKind::Separation { left: l, right: r })
            }
            ast::ContractClause::Permitted { target } => {
                let capability = self.capability_path(target)?;
                self.named_target(capability, target.span);
                Some(ir::ContractKind::Permitted { capability })
            }
            ast::ContractClause::AuthorityBasis { target, authority } => {
                let capability = self.capability_path(target);
                let resolved = self.authority_names.get(&authority.value).copied();
                if resolved.is_none() {
                    self.error(E_NAME_0, "undefined Authority", authority.span);
                }
                let (capability, authority) = (capability?, resolved?);
                self.named_target(capability, target.span);
                if !self.authorities[authority.0]
                    .grants
                    .iter()
                    .any(|g| g.capability == capability)
                {
                    self.error(
                        diag::E_CON_0,
                        "Authority has no grant for target",
                        target.span,
                    );
                }
                Some(ir::ContractKind::AuthorityBasis {
                    capability,
                    authority,
                })
            }
            ast::ContractClause::EvidenceBasis {
                direct,
                target,
                sources,
            } => {
                let capability = self.capability_path(target)?;
                if sources.is_empty() {
                    self.error(diag::E_CON_0, "evidence_basis_empty", target.span);
                }
                let mut seen = BTreeSet::new();
                let mut resolved = Vec::new();
                for source in sources {
                    let name = source
                        .value
                        .iter()
                        .map(|n| n.value.as_str())
                        .collect::<Vec<_>>()
                        .join(".");
                    if !seen.insert(name.clone()) {
                        self.error(
                            diag::E_CON_0,
                            "evidence_basis_duplicate_source",
                            source.span,
                        );
                    }
                    if source.value.len() != 1 {
                        self.error(diag::E_CON_0, "evidence_basis_not_source", source.span);
                    } else if let Some(&id) = self.actor_names.get(&name) {
                        if self.actors[id.0].source.is_some() {
                            resolved.push(id);
                        } else {
                            self.error(diag::E_CON_0, "evidence_basis_not_source", source.span);
                        }
                    } else if self.state_names.contains_key(&name) {
                        self.error(diag::E_CON_0, "evidence_basis_not_source", source.span);
                    } else {
                        self.error(E_NAME_0, "undefined source", source.span);
                    }
                }
                Some(ir::ContractKind::EvidenceBasis {
                    capability,
                    direct: *direct,
                    sources: resolved,
                    calls: Vec::new(),
                })
            }
            ast::ContractClause::Invariant { event, condition } => {
                let event = self.contract_event(event)?;
                let sites: Vec<_> = self
                    .calls
                    .values()
                    .filter(|c| c.capability == event.capability)
                    .cloned()
                    .collect();
                let mut available: Option<BTreeSet<ir::BindingId>> = None;
                for site in &sites {
                    let mut before = self
                        .call_before
                        .get(&site.binding)
                        .cloned()
                        .unwrap_or_default();
                    if !matches!(event.kind, ir::EventKind::Start) {
                        before.insert(site.binding);
                        if let Some(alias) = self.loop_result_alias(site.binding) {
                            before.insert(alias);
                        }
                    }
                    available = Some(match available {
                        None => before,
                        Some(previous) => previous.intersection(&before).copied().collect(),
                    });
                }
                if let Some(before) = self.route_before.get(&event.capability) {
                    available = Some(match available {
                        None => before.clone(),
                        Some(previous) => previous.intersection(before).copied().collect(),
                    });
                }
                let condition = self.contract_condition(condition, available.as_ref());
                Some(ir::ContractKind::Invariant { event, condition })
            }
            ast::ContractClause::StateInvariant {
                antecedent,
                condition,
            } => {
                let a = antecedent
                    .as_ref()
                    .map(|c| self.state_invariant_condition(c));
                let b = self.state_invariant_condition(condition);
                let antecedent = match a {
                    Some(a) => Some(a?),
                    None => None,
                };
                Some(ir::ContractKind::StateInvariant {
                    antecedent,
                    condition: b?,
                })
            }
            ast::ContractClause::AtMostOnce { event } => {
                let resolved = self.contract_event(event)?;
                if resolved.kind != ir::EventKind::Commit {
                    self.error(diag::E_CON_0, "at_most_once requires commit", event.span);
                }
                Some(ir::ContractKind::AtMostOnce { event: resolved })
            }
            ast::ContractClause::RequiresEvent { event, after } => {
                let start = self.contract_event(event);
                let before = self.contract_event(after);
                if start.is_some_and(|e| e.kind != ir::EventKind::Start) {
                    self.error(diag::E_CON_0, "requires_event requires start", event.span);
                }
                Some(ir::ContractKind::RequiresEvent {
                    start: start?,
                    before: before?,
                })
            }
            ast::ContractClause::Response {
                label,
                debtor,
                non_waivable,
                trigger,
                targets,
                duration,
                after,
            } => {
                let trigger = self.contract_trigger(trigger);
                let after = self.contract_trigger(after);
                let targets: Option<Vec<_>> = targets
                    .iter()
                    .map(|t| match t {
                        ast::Target::End(n) => self.end(n).map(ir::Target::End),
                        ast::Target::Event(e) => self.contract_event(e).map(ir::Target::Event),
                    })
                    .collect();
                let within = self.ticks(duration);
                if trigger.is_some() && after.is_some() && trigger != after {
                    self.error(
                        diag::E_CON_0,
                        "response after differs from trigger",
                        duration.span,
                    );
                }
                let resolved_debtor = debtor.as_ref().and_then(|n| {
                    let person = self.person_names.get(&n.value).copied();
                    let actor = self.actor_names.get(&n.value).copied();
                    match (person, actor) {
                        (Some(_), Some(_)) => {
                            self.error(diag::E_CON_0, "ambiguous debtor", n.span);
                            None
                        }
                        (Some(p), None) => Some(ir::Debtor::Person(p)),
                        (None, Some(a)) => Some(ir::Debtor::Actor(a)),
                        _ => {
                            self.error(E_NAME_0, "undefined debtor", n.span);
                            None
                        }
                    }
                });
                if label.is_some() || *non_waivable || debtor.is_some() {
                    return Some(ir::ContractKind::NamedResponse {
                        debtor: resolved_debtor,
                        label: label.as_ref().map(|l| l.value.clone()),
                        non_waivable: *non_waivable,
                        trigger: trigger?,
                        targets: targets?,
                        within: within?,
                    });
                }
                Some(ir::ContractKind::Response {
                    trigger: trigger?,
                    targets: targets?,
                    within: within?,
                })
            }
        }
    }

    fn state_alias_writer(&self, c: &ast::Call, variable: ir::StateVarId) -> bool {
        self.cap_names
            .get(&(c.actor.value.clone(), c.capability.value.clone()))
            .is_some_and(|id| {
                let cap = &self.capabilities[id.0];
                cap.writes.iter().any(|w| {
                    w.variable == variable
                        && w.source == ir::WriteSource::Result
                        && cap.result_kernels().all(|k| {
                            k.rows.iter().flat_map(|r| &r.branches).all(|b| {
                                b.overrides.iter().all(|(id, x)| {
                                    *id != variable || *x == ir::StateValue::Value(b.result)
                                })
                            })
                        })
                })
            })
    }

    fn state_invariant_name(&mut self, n: &ast::Ident) -> Option<ir::StateVarId> {
        if self.merge_read_name_forbidden(n) {
            return None;
        }
        let Some(&id) = self.state_names.get(&n.value) else {
            self.error(
                if self.global_bindings.contains(&n.value) {
                    U_SEM_0
                } else {
                    E_NAME_0
                },
                if self.global_bindings.contains(&n.value) {
                    "state invariant reads a process binding"
                } else {
                    "undefined state invariant variable"
                },
                n.span,
            );
            return None;
        };
        if self.state_variables[id.0].kind != ir::StateKind::ObservedCase
            && self.global_bindings.contains(&n.value)
        {
            self.unsupported(
                "invariant name is both a binding and a latent variable",
                n.span,
            );
            return None;
        }
        Some(id)
    }

    fn state_invariant_comparison(&mut self, c: &ast::Spanned<ast::Cond>) -> Option<ir::Cond> {
        match &c.value {
            ast::Cond::Eq { binding, .. } | ast::Cond::Compare { binding, .. } => {
                self.state_invariant_name(binding)?;
                self.contract_condition(c, None)
            }
            // ADR 0038: `x.f` with x a record case variable or a record binding
            // takes the scalar name checks of `x == v` (binding, latent alias).
            ast::Cond::LocalEq { actor, .. }
                if !self.actor_names.contains_key(&actor.value)
                    && (self.record_binding_names.contains(&actor.value)
                        || self
                            .state_names
                            .get(&actor.value)
                            .is_some_and(|id| self.is_record(self.state_variables[id.0].base))) =>
            {
                self.state_invariant_name(actor)?;
                self.contract_condition(c, None)
            }
            ast::Cond::LocalEq { .. } | ast::Cond::Contains { .. } => {
                self.contract_condition(c, None)
            }
            ast::Cond::Group(c) => self.state_invariant_comparison(c),
            ast::Cond::And(cs) | ast::Cond::Or(cs) => {
                let checked: Vec<_> = cs
                    .iter()
                    .map(|c| self.state_invariant_comparison(c))
                    .collect();
                let parts: Option<Vec<_>> = checked.into_iter().collect();
                parts.map(|p| {
                    if matches!(c.value, ast::Cond::And(_)) {
                        ir::Cond::And(p)
                    } else {
                        ir::Cond::Or(p)
                    }
                })
            }
        }
    }

    fn state_invariant_condition(
        &mut self,
        c: &ast::Spanned<ast::InvariantStateCond>,
    ) -> Option<ir::Cond> {
        use ast::InvariantStateCond as C;
        match &c.value {
            C::Compare(c) => self.state_invariant_comparison(c),
            C::Bool(n) => {
                let id = self.state_invariant_name(n)?;
                let v = &self.state_variables[id.0];
                if v.optional || v.integer_max.is_some() || Some(v.base) != self.builtin_bool {
                    self.error(
                        E_TYPE_0,
                        "bare invariant atom requires nonoptional builtin bool",
                        n.span,
                    );
                    return None;
                }
                let value = self.types[v.base.0]
                    .members
                    .iter()
                    .position(|v| v == "true")?;
                Some(ir::Cond::StateEq(ir::StateMatch {
                    variable: id,
                    value: ir::StateValue::Value(ir::ValueId(value)),
                }))
            }
            C::Group(c) => self.state_invariant_condition(c),
            C::And(cs) | C::Or(cs) => {
                let checked: Vec<_> = cs
                    .iter()
                    .map(|c| self.state_invariant_condition(c))
                    .collect();
                let parts: Option<Vec<_>> = checked.into_iter().collect();
                parts.map(|p| {
                    if matches!(c.value, C::And(_)) {
                        ir::Cond::And(p)
                    } else {
                        ir::Cond::Or(p)
                    }
                })
            }
        }
    }

    // None availability means no lexical invocation. Names still resolve globally;
    // selected bindings still receive type checking even for an inactive event.
    fn contract_condition(
        &mut self,
        c: &ast::Spanned<ast::Cond>,
        available: Option<&BTreeSet<ir::BindingId>>,
    ) -> Option<ir::Cond> {
        if self.merge_condition_forbidden(c) {
            return None;
        }
        match &c.value {
            ast::Cond::Contains { element, set } => self.membership(element, set, None),
            ast::Cond::Compare {
                binding,
                operator,
                value,
            } => self.integer_comparison(binding, operator, value),
            ast::Cond::LocalEq {
                actor,
                variable,
                operator,
                value,
            } => {
                if let Some(c) = self.record_comparison(
                    actor,
                    variable,
                    operator,
                    value,
                    RecordContext::Contract(available),
                ) {
                    c
                } else {
                    self.local_comparison(actor, variable, operator, value)
                }
            }
            ast::Cond::Eq { binding, value } => {
                if let Some(&id) = self.state_names.get(&binding.value) {
                    // A latent sharing a binding's name would be read silently
                    // where the author may mean the binding (process conditions
                    // already reject it as E-POL-LATENT).
                    if self.state_variables[id.0].kind != ir::StateKind::ObservedCase
                        && self.global_bindings.contains(&binding.value)
                    {
                        self.unsupported(
                            "invariant name is both a binding and a latent variable",
                            binding.span,
                        );
                        return None;
                    }
                    return self.state_comparison(id, value);
                }
                if !self.global_bindings.contains(&binding.value) {
                    self.error(E_NAME_0, "undefined contract binding", binding.span);
                    return None;
                }
                let Some(&id) = self.binding_names.get(&binding.value) else {
                    if available.is_some() {
                        self.unsupported(
                            "contract binding belongs to another process",
                            binding.span,
                        );
                    }
                    return None;
                };
                let member = if self.unknown_binding_types.contains(&binding.value) {
                    None
                } else {
                    if self.is_record(self.bindings[id.0].ty) {
                        self.error(
                            diag::U_SYN_0,
                            "record comparison is unsupported",
                            value.span,
                        );
                        return None;
                    }
                    self.value(self.bindings[id.0].ty, value)
                };
                if available.is_some_and(|a| !a.contains(&id)) {
                    self.unsupported(
                        "contract result is not definitely available at every call site",
                        binding.span,
                    );
                }
                member.map(|value| ir::Cond::Eq(ir::ResultMatch { binding: id, value }))
            }
            ast::Cond::Group(inner) => self.contract_condition(inner, available),
            ast::Cond::And(parts) | ast::Cond::Or(parts) => {
                let checked: Vec<_> = parts
                    .iter()
                    .map(|p| self.contract_condition(p, available))
                    .collect();
                let parts: Option<Vec<_>> = checked.into_iter().collect();
                parts.map(|parts| {
                    if matches!(c.value, ast::Cond::And(_)) {
                        ir::Cond::And(parts)
                    } else {
                        ir::Cond::Or(parts)
                    }
                })
            }
        }
    }

    // ADR 0007. Returns the resolved Cut (None when absent or unresolvable)
    // and its warnings; errors go to self.errors like every static check.
    fn cut(
        &mut self,
        model: &ast::Model,
        selected: Option<&ast::Configuration>,
    ) -> (Option<ir::Cut>, Vec<Diagnostic>) {
        let cuts: Vec<_> = model
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Cut(c) => Some(c),
                _ => None,
            })
            .collect();
        let mut warnings = Vec::new();
        let configured = model
            .items
            .iter()
            .any(|i| matches!(i, ast::Item::Configuration(_)));
        if !configured {
            let Some(decl) = cuts.first() else {
                return (None, warnings);
            };
            for extra in &cuts[1..] {
                self.unsupported("only one cut per model is supported", extra.span);
            }
            let processes = model
                .items
                .iter()
                .filter_map(|i| match i {
                    ast::Item::Process(p) => Some(p),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let (cut, warnings) = self.cut_decl(decl, &processes, model);
            return (cut.filter(|_| cuts.len() == 1), warnings);
        }
        let mut chosen = None;
        for decl in cuts {
            // D2: lexical excluded-call checks cover *all* configurations using
            // this Cut, without resolving any other part of their process flow.
            let references: BTreeSet<_> = model
                .items
                .iter()
                .filter_map(|i| match i {
                    ast::Item::Configuration(c) if c.cut.value == decl.name.value => {
                        Some(c.process.value.as_str())
                    }
                    _ => None,
                })
                .collect();
            let processes = model
                .items
                .iter()
                .filter_map(|i| match i {
                    ast::Item::Process(p) if references.contains(p.name.value.as_str()) => Some(p),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let (cut, cut_warnings) = self.cut_decl(decl, &processes, model);
            warnings.extend(cut_warnings);
            if selected.is_some_and(|c| c.cut.value == decl.name.value) {
                chosen = cut;
            }
        }
        (chosen, warnings)
    }

    fn cut_decl(
        &mut self,
        decl: &ast::CutDecl,
        processes: &[&ast::Process],
        model: &ast::Model,
    ) -> (Option<ir::Cut>, Vec<Diagnostic>) {
        let mut warnings = Vec::new();
        let errors_before = self.errors.len();
        let actor = |this: &Self, path: &ast::Path| match path.value.as_slice() {
            [name] => this.actor_names.get(&name.value).copied(),
            _ => None,
        };
        let text = |path: &ast::Path| {
            path.value
                .iter()
                .map(|p| p.value.as_str())
                .collect::<Vec<_>>()
                .join(".")
        };
        let mut cut = ir::Cut {
            horizon: Vec::new(),
            name: decl.name.value.clone(),
            question: None,
            include: Vec::new(),
            exclude: Vec::new(),
            defer: Vec::new(),
            externality: Vec::new(),
            reentry: Vec::new(),
        };
        let mut include_names = BTreeSet::new();
        let entry = |this: &mut Self, e: &ast::Exclusion, warnings: &mut Vec<Diagnostic>| {
            if e.loses.is_none() {
                warnings.push(Diagnostic {
                    code: diag::W_CUT_1.into(),
                    severity: diag::Severity::Warning,
                    message: format!(
                        "{} has no loses: the lost capability is unstated",
                        text(&e.target)
                    ),
                    span: Some(e.target.span),
                });
            }
            ir::CutEntry {
                target: actor(this, &e.target).map_or_else(
                    || ir::CutTarget::Label(text(&e.target)),
                    ir::CutTarget::Actor,
                ),
                reason: e.reason.value.clone(),
                loses: e.loses.as_ref().map(|l| l.value.clone()),
            }
        };
        for item in &decl.items {
            match &item.value {
                ast::CutItem::Horizon(entries) => {
                    for h in entries {
                        if let Some(within) = self.ticks(&h.within) {
                            let row = ir::Horizon {
                                from: h.from.as_ref().map(|s| s.value.clone()),
                                to: h.to.value.clone(),
                                within,
                                structural_only: h.structural_only,
                            };
                            if cut.horizon.contains(&row) {
                                self.unsupported("duplicate horizon", item.span);
                            } else {
                                cut.horizon.push(row);
                            }
                        }
                    }
                }
                ast::CutItem::Question(q) => {
                    if cut.question.replace(q.value.clone()).is_some() {
                        self.unsupported("duplicate cut question", item.span);
                    }
                }
                ast::CutItem::Include(names) => {
                    for name in names {
                        if !include_names.insert(name.value.clone()) {
                            self.unsupported("duplicate cut include", name.span);
                        } else if let Some(&id) = self.actor_names.get(&name.value) {
                            cut.include.push(id);
                        } else {
                            self.error(E_NAME_0, "cut includes an undeclared Actor", name.span);
                        }
                    }
                }
                ast::CutItem::Exclude(e) => {
                    let e = entry(self, e, &mut warnings);
                    cut.exclude.push(e);
                }
                ast::CutItem::Defer(e) => {
                    let e = entry(self, e, &mut warnings);
                    cut.defer.push(e);
                }
                ast::CutItem::Externality(paths) => cut.externality.extend(paths.iter().map(text)),
                ast::CutItem::Reentry(entries) => {
                    cut.reentry.extend(entries.iter().map(|e| e.value.clone()))
                }
            }
        }
        let excluded: BTreeSet<ir::ActorId> = cut
            .exclude
            .iter()
            .filter_map(|e| match e.target {
                ir::CutTarget::Actor(id) => Some(id),
                ir::CutTarget::Label(_) => None,
            })
            .collect();
        for (name, id) in self.actor_names.clone() {
            let included = cut.include.contains(&id);
            if included && excluded.contains(&id) {
                self.error(
                    diag::E_CUT_1,
                    format!("Actor {name} is both included and excluded"),
                    decl.span,
                );
            } else if !included && !excluded.contains(&id) {
                self.error(
                    diag::E_CUT_1,
                    format!("declared Actor {name} is neither included nor excluded by the cut"),
                    decl.span,
                );
            }
        }
        for item in &decl.items {
            if let ast::CutItem::Defer(e) = &item.value
                && let Some(first) = e.target.value.first()
                && self
                    .actor_names
                    .get(&first.value)
                    .is_some_and(|id| excluded.contains(id))
            {
                self.error(
                    diag::E_CUT_1,
                    "deferring part of an excluded Actor",
                    e.target.span,
                );
            }
        }
        let excluded_names: BTreeSet<String> = self
            .actor_names
            .iter()
            .filter(|(_, id)| excluded.contains(id))
            .map(|(name, _)| name.clone())
            .collect();
        let mut calls = Vec::new();
        for p in processes {
            collect_calls(&p.body, &mut calls);
        }
        for call in calls {
            if excluded_names.contains(&call.actor.value) {
                self.error(
                    diag::E_CUT_1,
                    "the process calls a Capability of an Actor excluded by the cut",
                    call.span,
                );
            }
            if self.calls.values().any(|c| c.span == call.span)
                && let Some(&id) = self
                    .cap_names
                    .get(&(call.actor.value.clone(), call.capability.value.clone()))
            {
                let cap = &self.capabilities[id.0];
                let refs: Vec<_> = cap
                    .kernel
                    .iter()
                    .flat_map(|k| &k.given)
                    .chain(&cap.kernel_reads_latent)
                    .copied()
                    .collect();
                for id in refs {
                    if self.state_variables[id.0]
                        .kind
                        .actor()
                        .is_some_and(|a| excluded.contains(&a))
                    {
                        self.error(
                            diag::E_CUT_1,
                            "kernel reads an excluded Actor latent",
                            call.span,
                        );
                    }
                }
            }
        }
        // An excluded Actor is outside the model, so no assertion may read its
        // local latent either (no Silent Cut through an invariant).
        fn local_actors<'c>(
            c: &'c ast::Spanned<ast::Cond>,
            out: &mut Vec<&'c ast::Spanned<ast::Cond>>,
        ) {
            match &c.value {
                ast::Cond::LocalEq { .. } | ast::Cond::Contains { .. } => out.push(c),
                ast::Cond::Eq { .. } | ast::Cond::Compare { .. } => {}
                ast::Cond::And(parts) | ast::Cond::Or(parts) => {
                    parts.iter().for_each(|p| local_actors(p, out))
                }
                ast::Cond::Group(inner) => local_actors(inner, out),
            }
        }
        let mut read = Vec::new();
        fn bare_names<'c>(c: &'c ast::Spanned<ast::Cond>, out: &mut Vec<&'c ast::Ident>) {
            match &c.value {
                ast::Cond::Eq { binding, .. } | ast::Cond::Compare { binding, .. } => {
                    out.push(binding)
                }
                ast::Cond::And(cs) | ast::Cond::Or(cs) => {
                    for c in cs {
                        bare_names(c, out);
                    }
                }
                ast::Cond::Group(c) => bare_names(c, out),
                ast::Cond::Contains { set, .. } => {
                    if let [n] = set.value.as_slice() {
                        out.push(n);
                    }
                }
                ast::Cond::LocalEq { .. } => {}
            }
        }
        fn formula_actors<'c>(
            c: &'c ast::Spanned<ast::ResponsibilityCond>,
            out: &mut Vec<&'c ast::Spanned<ast::Cond>>,
        ) {
            use ast::ResponsibilityCond as C;
            match &c.value {
                C::Compare(c) => local_actors(c, out),
                C::Bool(_) => {}
                C::Group(c) => formula_actors(c, out),
                C::Implies(a, b) => {
                    formula_actors(a, out);
                    formula_actors(b, out);
                }
                C::And(cs) | C::Or(cs) => {
                    for c in cs {
                        formula_actors(c, out);
                    }
                }
            }
        }
        fn block_actors<'c>(b: &'c ast::Block, out: &mut Vec<&'c ast::Spanned<ast::Cond>>) {
            for s in b {
                if let ast::Stmt::Merge {
                    condition,
                    then_body,
                    else_body,
                    ..
                }
                | ast::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } = &s.value
                {
                    local_actors(condition, out);
                    block_actors(then_body, out);
                    block_actors(else_body, out);
                }
            }
        }
        for item in &model.items {
            // New observation guards are legal policy inputs, so they need the
            // same explicit Cut boundary as assertions. Legacy latent policy
            // diagnostics remain unchanged.
            let mut guards = Vec::new();
            match item {
                ast::Item::Process(p) if processes.iter().any(|q| q.span == p.span) => {
                    block_actors(&p.body, &mut guards)
                }
                ast::Item::Actor(a) => {
                    for f in &a.items {
                        if let ast::ActorItem::Capability(c) = f {
                            for i in &c.items {
                                if let ast::CapItem::Requires { condition, .. }
                                | ast::CapItem::Atomic { condition, .. } = &i.value
                                {
                                    local_actors(condition, &mut guards);
                                    let owner = self.actor_names[&a.name.value];
                                    if excluded.contains(&owner) {
                                        let mut names = Vec::new();
                                        bare_names(condition, &mut names);
                                        for n in names {
                                            if !self.state_names.contains_key(&n.value)
                                                && self
                                                    .local_names
                                                    .get(&(owner, n.value.clone()))
                                                    .is_some_and(|id| {
                                                        self.state_variables[id.0]
                                                            .kind
                                                            .is_observed()
                                                    })
                                            {
                                                self.error(
                                                    diag::E_CUT_1,
                                                    "condition reads an Actor excluded by the cut",
                                                    n.span,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ast::Item::Authority(a) => {
                    for i in &a.items {
                        if let ast::AuthorityItem::Scope(c) = &i.value {
                            local_actors(c, &mut guards);
                        }
                    }
                }
                _ => {}
            }
            read.extend(guards.into_iter().filter(|c| {
                if let ast::Cond::LocalEq {
                    actor, variable, ..
                } = &c.value
                {
                    self.actor_names
                        .get(&actor.value)
                        .and_then(|a| self.local_names.get(&(*a, variable.value.clone())))
                        .is_some_and(|id| {
                            matches!(
                                self.state_variables[id.0].kind,
                                ir::StateKind::ActorObserved(_) | ir::StateKind::Rule(_)
                            )
                        })
                } else {
                    matches!(c.value, ast::Cond::Contains { .. })
                }
            }));
            if let ast::Item::Responsible(ast::Responsible {
                target: ast::ResponsibilityTarget::Formula(c),
                ..
            }) = item
            {
                formula_actors(c, &mut read);
            }
            if let ast::Item::Responsible(ast::Responsible {
                target: ast::ResponsibilityTarget::Condition(condition),
                ..
            }) = item
            {
                local_actors(condition, &mut read);
            }
            if let ast::Item::QualityProperty(ast::QualityProperty {
                target: ast::QualityTarget::State(c),
                ..
            }) = item
            {
                local_actors(c, &mut read);
            }
            if let ast::Item::Verify(ast::Spanned {
                value:
                    ast::Property::ProbabilityReached {
                        condition: ast::ProbabilityCondition::State(c),
                        ..
                    },
                ..
            }) = item
            {
                local_actors(c, &mut read);
            }
            if let ast::Item::Contract(c) = item {
                for clause in &c.clauses {
                    if let ast::ContractClause::Invariant { condition, .. } = &clause.value {
                        local_actors(condition, &mut read);
                    }
                    if let ast::ContractClause::StateInvariant {
                        antecedent,
                        condition,
                    } = &clause.value
                    {
                        fn state_actors<'a>(
                            c: &'a ast::Spanned<ast::InvariantStateCond>,
                            out: &mut Vec<&'a ast::Spanned<ast::Cond>>,
                        ) {
                            use ast::InvariantStateCond as C;
                            match &c.value {
                                C::Compare(c) => local_actors(c, out),
                                C::And(cs) | C::Or(cs) => {
                                    for c in cs {
                                        state_actors(c, out);
                                    }
                                }
                                C::Group(c) => state_actors(c, out),
                                C::Bool(_) => {}
                            }
                        }
                        if let Some(a) = antecedent {
                            state_actors(a, &mut read);
                        }
                        state_actors(condition, &mut read);
                    }
                }
            }
        }
        for c in read {
            if let ast::Cond::Contains { set, .. } = &c.value {
                if let [actor, _] = set.value.as_slice()
                    && excluded_names.contains(&actor.value)
                {
                    self.error(
                        diag::E_CUT_1,
                        "condition reads an Actor excluded by the cut",
                        set.span,
                    );
                }
                continue;
            }
            let ast::Cond::LocalEq {
                actor, variable, ..
            } = &c.value
            else {
                continue;
            };
            if excluded_names.contains(&actor.value) {
                let observed = self
                    .actor_names
                    .get(&actor.value)
                    .and_then(|a| self.local_names.get(&(*a, variable.value.clone())))
                    .is_some_and(|id| {
                        matches!(
                            self.state_variables[id.0].kind,
                            ir::StateKind::ActorObserved(_) | ir::StateKind::Rule(_)
                        )
                    });
                self.error(
                    diag::E_CUT_1,
                    if observed { "condition reads an Actor excluded by the cut" } else { "an invariant or probability query reads a latent of an Actor excluded by the cut" },
                    if observed { Span { start: actor.span.start, end: variable.span.end } } else { actor.span },
                );
            }
        }
        let resolved = self.errors.len() == errors_before;
        (resolved.then_some(cut), warnings)
    }

    /// Configuration/process/Cut names and intents are model-wide declarations;
    /// selected process flow, on the other hand, is strictly process-local.
    fn rule_amendments(&mut self, model: &ast::Model, selected: Option<&ast::Configuration>) {
        let mut applied = Vec::new();
        let mut cut_errors = Vec::new();
        for item in &model.items {
            let ast::Item::Configuration(c) = item else {
                continue;
            };
            let mut seen = BTreeSet::new();
            for amendment in &c.amendments {
                let a = amendment;
                let rule = self.actor_names.get(&a.rule.value).copied();
                let person = self.person(&a.by);
                let authority = self.authority_names.get(&a.under.value).copied();
                if authority.is_none() {
                    self.error(E_NAME_0, "undefined amendment Authority", a.under.span);
                }
                let Some(rule) = rule else {
                    self.error(E_NAME_0, "undefined rule Actor", a.rule.span);
                    continue;
                };
                let Some(info) = self.actors[rule.0].metadata.rule.clone() else {
                    self.error(diag::E_CON_0, "amend requires a rule Actor", a.rule.span);
                    continue;
                };
                if !seen.insert(rule) {
                    self.unsupported("repeated amendment of one rule is unsupported", a.span);
                    continue;
                }
                let mut values = Vec::new();
                let mut assigned = BTreeSet::new();
                for (name, value) in &a.values {
                    if !assigned.insert(&name.value) {
                        self.unsupported("duplicate rule assignment", name.span);
                    }
                    if let Some(v) = info
                        .values
                        .iter()
                        .find(|v| self.state_variables[v.variable.0].name == name.value)
                    {
                        let variable = &self.state_variables[v.variable.0];
                        if let Some(value) = self.state_value(variable.base, false, value) {
                            values.push((v.variable, value));
                        }
                    } else {
                        self.error(E_NAME_0, "undefined rule value", name.span);
                    }
                }
                let cut = model.items.iter().find_map(|i| {
                    if let ast::Item::Cut(d) = i {
                        (d.name.value == c.cut.value).then_some(d)
                    } else {
                        None
                    }
                });
                if cut.is_some_and(|d| d.items.iter().any(|i| matches!(&i.value, ast::CutItem::Exclude(e) if e.target.value.len() == 1 && e.target.value[0].value == a.rule.value))) {
                    cut_errors.push(a.rule.span);
                }
                let (Some(person), Some(authority)) = (person, authority) else {
                    continue;
                };
                let auth = &self.authorities[authority.0];
                if self.errors.iter().any(|e| {
                    e.span
                        .is_some_and(|s| s.start >= auth.span.start && s.end <= auth.span.end)
                }) {
                    continue;
                }
                let fault = if a.from.value != info.declared_version {
                    Some((
                        "amendment must start from the declared rule version",
                        a.from.span,
                    ))
                } else if a.to.value == a.from.value {
                    Some(("amendment must change the rule version", a.to.span))
                } else if auth.not_by.contains(&person) {
                    Some(("rule change by a person excluded by not_by", a.by.span))
                } else if !auth
                    .rule_grants
                    .iter()
                    .any(|g| g.rule == rule && g.persons.contains(&person))
                {
                    Some((
                        "Authority does not grant change of the rule to the amender",
                        a.under.span,
                    ))
                } else if auth.under.as_ref().is_some_and(|b| {
                    c.amendments.iter().any(|other| {
                        self.actor_names.get(&other.rule.value) == Some(&b.rule)
                            && other.to.value == b.version
                    })
                }) {
                    Some((
                        "rule change is authorized only under the amended rule",
                        a.under.span,
                    ))
                } else if auth.under.as_ref().is_some_and(|b| {
                    self.actors[b.rule.0]
                        .metadata
                        .rule
                        .as_ref()
                        .is_some_and(|r| r.declared_version != b.version)
                }) {
                    // An unresolved version already has its independent name diagnostic.
                    auth.under.as_ref().filter(|b| model.items.iter().any(|i| matches!(i, ast::Item::Configuration(c) if c.amendments.iter().any(|a| self.actor_names.get(&a.rule.value) == Some(&b.rule) && a.to.value == b.version)))).map(|_| ("rule change Authority is not in force under the prior rule", a.under.span))
                } else {
                    None
                };
                if let Some((message, span)) = fault {
                    self.error(diag::E_CON_0, message, span);
                }
                if selected.is_some_and(|s| s.name.value == c.name.value) {
                    applied.push((
                        rule,
                        ir::RuleAmendment {
                            from: a.from.value.clone(),
                            to: a.to.value.clone(),
                            by: person,
                            under: authority,
                            span: a.span,
                        },
                        values,
                    ));
                }
            }
        }
        for span in cut_errors {
            self.error(
                diag::E_CUT_1,
                "amendment targets an Actor excluded by the cut",
                span,
            );
        }
        if self.errors.is_empty() {
            for (rule, amendment, values) in applied {
                let info = self.actors[rule.0].metadata.rule.as_mut().unwrap();
                info.effective_version = amendment.to.clone();
                info.amendment = Some(amendment);
                for (variable, value) in values {
                    self.state_variables[variable.0].initial = ir::InitialValue::Fixed(value);
                }
            }
        }
    }

    fn configuration<'a>(
        &mut self,
        model: &'a ast::Model,
        configurations: &[&'a ast::Configuration],
        selection: Option<&str>,
    ) -> (Option<&'a ast::Configuration>, Vec<Diagnostic>) {
        let mut warnings = Vec::new();
        if configurations.is_empty() {
            if selection.is_some() {
                self.error(
                    diag::E_CONF_0,
                    "configuration selector requires configuration declarations",
                    model.span,
                );
            }
            return (None, warnings);
        }
        let mut processes = BTreeSet::new();
        let mut cuts = BTreeSet::new();
        let mut names = BTreeSet::new();
        for item in &model.items {
            let (set, name) = match item {
                ast::Item::Process(p) => (&mut processes, &p.name),
                ast::Item::Cut(c) => (&mut cuts, &c.name),
                ast::Item::Configuration(c) => (&mut names, &c.name),
                _ => continue,
            };
            if !set.insert(name.value.as_str()) {
                self.unsupported(
                    "duplicate configuration, process or cut declaration",
                    name.span,
                );
            }
        }
        for c in configurations {
            if !processes.contains(c.process.value.as_str()) {
                self.error(
                    E_NAME_0,
                    "configuration references an undeclared process",
                    c.process.span,
                );
            }
            if !cuts.contains(c.cut.value.as_str()) {
                self.error(
                    E_NAME_0,
                    "configuration references an undeclared cut",
                    c.cut.span,
                );
            }
            self.configuration_intent_names(model, c);
        }
        for item in &model.items {
            let unused = match item {
                ast::Item::Process(p)
                    if !configurations
                        .iter()
                        .any(|c| c.process.value == p.name.value) =>
                {
                    Some(&p.name)
                }
                ast::Item::Cut(cut)
                    if !configurations.iter().any(|c| c.cut.value == cut.name.value) =>
                {
                    Some(&cut.name)
                }
                _ => None,
            };
            if let Some(name) = unused {
                warnings.push(Diagnostic {
                    code: diag::W_CONF_0.into(),
                    severity: diag::Severity::Warning,
                    message: format!("{} is not referenced by any configuration", name.value),
                    span: Some(name.span),
                });
            }
        }
        let selected = selection.and_then(|name| {
            configurations
                .iter()
                .find(|c| c.name.value == name)
                .copied()
        });
        if selection.is_none() {
            self.error(
                diag::E_CONF_0,
                "a configuration selector is required",
                model.span,
            );
        } else if selected.is_none() {
            self.error(E_NAME_0, "unknown configuration", model.span);
        }
        (selected, warnings)
    }

    fn configuration_intent_names(&mut self, model: &ast::Model, c: &ast::Configuration) {
        // Each intent belongs to its destination process, even while another
        // configuration is selected for lowering. Do not use selected IR IDs.
        let mut calls = Vec::new();
        if let Some(process) = model.items.iter().find_map(|item| match item {
            ast::Item::Process(p) if p.name.value == c.process.value => Some(p),
            _ => None,
        }) {
            collect_calls(&process.body, &mut calls);
        }
        let bindings: BTreeSet<_> = calls
            .iter()
            .map(|call| call.binding.value.as_str())
            .collect();
        let people: BTreeSet<_> = model
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::People(ps) => Some(&ps.value),
                _ => None,
            })
            .flatten()
            .map(|p| p.value.as_str())
            .collect();
        let mut ends = BTreeSet::new();
        for i in &model.items {
            if let ast::Item::Process(p) = i {
                collect_comparison_ends(&p.body, &mut ends);
            }
        }
        let mut lanes = BTreeSet::new();
        for intent in &c.intent {
            if let ast::Intent::Claim {
                action,
                target,
                keep_fallback: true,
                span,
                ..
            } = intent
                && (*action != ast::IntentAction::Retire
                    || target.kind != ast::DiffKind::Relation
                    || target.path.value.len() != 2)
            {
                self.error(
                    diag::U_SYN_0,
                    "fallback requires retire Actor.Capability",
                    *span,
                );
                continue;
            }
            let (target, lane, selector) = match intent {
                ast::Intent::Claim { target, .. } => (Some(target), "claim", None),
                ast::Intent::Preserve { on, selector, .. } => {
                    (on.as_ref(), "preserve", Some(selector.value()))
                }
                ast::Intent::Override {
                    target, subject, ..
                } => {
                    if target.kind == ast::DiffKind::Rule {
                        self.unsupported(
                            "override of a rule difference is unsupported",
                            target.path.span,
                        );
                    }
                    let a = self.actor_names.contains_key(&subject.value);
                    let p = people.contains(subject.value.as_str());
                    if !a && !p {
                        self.error(E_NAME_0, "undeclared override subject", subject.span);
                    }
                    if a && p {
                        self.error(diag::E_CON_0, "ambiguous override subject", subject.span);
                    }
                    (Some(target), "override", None)
                }
                ast::Intent::Maintain { target, person, .. } => {
                    if !people.contains(person.value.as_str()) {
                        self.error(
                            if self.actor_names.contains_key(&person.value) {
                                diag::E_CON_0
                            } else {
                                E_NAME_0
                            },
                            "undeclared maintenance person",
                            person.span,
                        );
                    }
                    (Some(target), "maintain", None)
                }
            };
            let key = (lane, target.map(|t| (t.kind, t.name())), selector);
            if !lanes.insert(key) {
                let span = target.map_or(c.span, |t| t.path.span);
                self.unsupported("duplicate intent target", span);
            }
            let Some(t) = target else {
                continue;
            };
            let parts = &t.path.value;
            let actor = parts.len() == 1 && self.actor_names.contains_key(&parts[0].value);
            let cap = parts.len() == 2
                && self
                    .cap_names
                    .contains_key(&(parts[0].value.clone(), parts[1].value.clone()));
            let end = parts.len() == 2 && parts[0].value == "end" && ends.contains(&parts[1].value);
            let binding = t.kind == ast::DiffKind::Relation
                && parts.len() == 1
                && bindings.contains(parts[0].value.as_str())
                && !matches!(
                    intent,
                    ast::Intent::Claim {
                        action: ast::IntentAction::Add | ast::IntentAction::Retire,
                        ..
                    }
                );
            if binding
                && matches!(
                    intent,
                    ast::Intent::Override { .. } | ast::Intent::Maintain { .. }
                )
            {
                self.unsupported(
                    "override and maintain of a relation binding are unsupported",
                    t.path.span,
                );
                continue;
            }
            let resolved = match t.kind {
                ast::DiffKind::ContractScope => model.items.iter().any(|i| matches!(i, ast::Item::Contract(c) if c.name.as_ref().is_some_and(|n| n.value == t.name()))),
                ast::DiffKind::Rule => actor && model.items.iter().any(|i| matches!(i, ast::Item::Actor(a) if a.name.value == parts[0].value && a.items.iter().any(|i| matches!(i, ast::ActorItem::Decides(_))))),
                ast::DiffKind::Actor | ast::DiffKind::Resource => actor,
                ast::DiffKind::Relation => cap || binding,
                ast::DiffKind::Time | ast::DiffKind::Authority => cap,
                ast::DiffKind::Meaning => end,
                ast::DiffKind::Contest => end || (parts.len() == 3 && parts[2].value == "commit" && self.cap_names.get(&(parts[0].value.clone(), parts[1].value.clone())).is_some_and(|c| self.capabilities[c.0].commits)),
                ast::DiffKind::Visibility => actor && model.items.iter().any(|i| matches!(i, ast::Item::Actor(a) if a.name.value == parts[0].value && a.items.iter().any(|i| matches!(i, ast::ActorItem::Source(..))))),
                ast::DiffKind::Qualification | ast::DiffKind::Performance | ast::DiffKind::Degradation => false,
            };
            if !resolved {
                let legacy = matches!(
                    intent,
                    ast::Intent::Claim {
                        action: ast::IntentAction::Add | ast::IntentAction::Retire,
                        ..
                    }
                );
                let known_subject = !legacy && parts.len() == 1 && (people.contains(parts[0].value.as_str())
                    || model.items.iter().any(|i| matches!(i, ast::Item::Authority(a) if a.name.value == parts[0].value)));
                self.error(
                    if actor || cap || end || known_subject {
                        diag::E_CON_0
                    } else {
                        E_NAME_0
                    },
                    "undeclared intent target or wrong difference namespace",
                    t.path.span,
                );
            }
        }
    }

    fn configuration_preserve_names(
        &mut self,
        model: &ast::Model,
        clauses: &[ir::ContractClause],
        metadata: &crate::responsibility::Metadata,
    ) {
        let ids: BTreeSet<_> = clauses
            .iter()
            .enumerate()
            .map(|(i, c)| c.property_id(ir::ClauseId(i)))
            .chain(metadata.contests.iter().map(|c| c.property_id()))
            .chain(
                crate::contract_scope::Selection::from_ast(model, None)
                    .clauses
                    .into_iter()
                    .filter(|c| c.applies_to.is_some())
                    .map(|c| c.id),
            )
            .collect();
        for item in &model.items {
            if let ast::Item::Configuration(c) = item {
                for intent in &c.intent {
                    if let ast::Intent::Preserve { selector, .. } = intent {
                        let found = match selector {
                            ast::PropertySelector::Contract(name) => {
                                model.items.iter().any(|i| matches!(i, ast::Item::Contract(c) if c.applies_to.is_some() && c.name.as_ref().is_some_and(|n| n.value == name.value))) || clauses
                                    .iter()
                                    .any(|c| c.name.as_deref() == Some(&name.value))
                                    || metadata
                                        .contests
                                        .iter()
                                        .any(|c| c.name.as_deref() == Some(&name.value))
                            }
                            ast::PropertySelector::Exact(id) => ids.contains(&id.value),
                        };
                        if !found {
                            self.error(
                                E_NAME_0,
                                "undeclared preservation selector",
                                selector.span(),
                            );
                        }
                    }
                }
            }
        }
    }

    fn unique_members(&mut self, members: &[ast::Ident], span: Span) {
        if members.is_empty() {
            self.unsupported("empty finite value set", span);
        }
        let mut counts = BTreeMap::new();
        for member in members {
            *counts.entry(&member.value).or_insert(0) += 1;
        }
        for member in members {
            if counts[&member.value] > 1 {
                self.unsupported("duplicate finite value", member.span);
            }
        }
    }

    fn comparison(
        &mut self,
        model: &ast::Model,
        quality: Option<&ir::QualitySpec>,
    ) -> Option<ir::ComparisonSpec> {
        let mut result = None;
        for item in &model.items {
            let ast::Item::Compare(c) = item else {
                continue;
            };
            if result.is_some() {
                self.unsupported("duplicate compare declaration", c.span);
            }
            for p in &c.plans {
                if !model
                    .items
                    .iter()
                    .any(|i| matches!(i, ast::Item::Configuration(d) if d.name.value == p.value))
                {
                    self.error(E_NAME_0, "unknown configuration", p.span);
                }
            }
            if let Some(n) = &c.cut
                && !model
                    .items
                    .iter()
                    .any(|i| matches!(i, ast::Item::Cut(d) if d.name.value == n.value))
            {
                self.error(E_NAME_0, "unknown cut", n.span);
            }
            let mut resolve = |n: &ast::Ident, quantile: bool| {
                let found = quality.and_then(|q| {
                    if quantile {
                        q.quantiles.iter().position(|m| m.name == n.value)
                    } else {
                        q.metrics.iter().position(|m| m.name == n.value)
                    }
                });
                if found.is_none() {
                    if quantile {
                        self.error(
                            E_NAME_0,
                            format!("unknown quantile metric: {}", n.value),
                            n.span,
                        );
                    } else {
                        self.error(E_NAME_0, "unknown cohort metric", n.span);
                    }
                }
                found
            };
            let hard_ids: Vec<_> = c.hard.iter().map(|h| resolve(&h.cohort, false)).collect();
            let objective_ids: Vec<_> = c
                .objectives
                .iter()
                .map(|o| match &o.target {
                    ast::ObjectiveRef::Cohort(n) => resolve(n, false).map(ir::ObjectiveRef::Cohort),
                    ast::ObjectiveRef::Quantile(n) => {
                        resolve(n, true).map(ir::ObjectiveRef::Quantile)
                    }
                    ast::ObjectiveRef::Overall(f) => Some(ir::ObjectiveRef::Overall(*f)),
                })
                .collect();
            let mut hard: Vec<ir::HardConstraint> = Vec::new();
            for (h, id) in c.hard.iter().zip(hard_ids) {
                let bound = self.probability(&h.bound);
                if let (Some(cohort), Some(bound)) = (id, bound) {
                    if hard
                        .iter()
                        .any(|r| r.cohort == cohort && r.op == h.op && r.bound == bound)
                    {
                        self.unsupported("duplicate hard constraint", h.span);
                    }
                    hard.push(ir::HardConstraint {
                        cohort,
                        op: h.op,
                        bound,
                        span: h.span,
                    });
                }
            }
            let mut objectives: Vec<ir::Objective> = Vec::new();
            for (o, target) in c.objectives.iter().zip(objective_ids) {
                if let Some(target) = target {
                    if objectives
                        .iter()
                        .any(|r| r.direction == o.direction && r.target == target)
                    {
                        self.unsupported("duplicate objective", o.span);
                    }
                    objectives.push(ir::Objective {
                        direction: o.direction,
                        target,
                        span: o.span,
                    });
                }
            }
            result = Some(ir::ComparisonSpec {
                plans: c.plans.iter().map(|n| n.value.clone()).collect(),
                under: c
                    .under
                    .as_ref()
                    .map(|ns| ns.iter().map(|n| n.value.clone()).collect()),
                cut: c.cut.as_ref().map(|n| n.value.clone()),
                hard,
                objectives,
                span: c.span,
            });
        }
        result
    }

    fn quality(&mut self, model: &ast::Model) -> Option<ir::QualitySpec> {
        let mut properties = Vec::new();
        let mut names = BTreeMap::new();
        for item in &model.items {
            let ast::Item::QualityProperty(p) = item else {
                continue;
            };
            if matches!(p.name.value.as_str(), "received" | "admitted" | "id") {
                self.unsupported("reserved quality property name", p.name.span);
            }
            if names.contains_key(&p.name.value) {
                self.unsupported("duplicate quality property", p.name.span);
            } else {
                names.insert(p.name.value.clone(), properties.len());
            }
            let target = match &p.target {
                ast::QualityTarget::State(c) => {
                    self.probability_condition(c).map(ir::QualityTarget::State)
                }
                ast::QualityTarget::Ends(ends) => {
                    let mut out = Vec::new();
                    for e in ends {
                        if let Some(id) = self.end(e)
                            && !out.contains(&id)
                        {
                            out.push(id);
                        }
                    }
                    Some(ir::QualityTarget::Ends(out))
                }
            };
            let within = p.within.as_ref().and_then(|d| self.ticks(d));
            if let Some(target) = target {
                properties.push(ir::QualityProperty {
                    name: p.name.value.clone(),
                    target,
                    within,
                });
            }
        }
        let mut metrics = Vec::new();
        let mut quantiles = Vec::new();
        let mut metric_names = BTreeSet::new();
        for item in &model.items {
            if let ast::Item::QuantileMetric(c) = item {
                if !metric_names.insert(c.name.value.clone()) {
                    self.unsupported("duplicate quantile metric name", c.name.span);
                }
                let p = self.probability(&c.p);
                if p.as_ref()
                    .is_some_and(|p| p == &crate::probability::Rational::zero())
                {
                    self.error(
                        crate::diag::E_PROB_0,
                        "quantile level must be positive",
                        c.p.span,
                    );
                }
                let property = names.get(&c.property.value).copied();
                if property.is_none() {
                    self.error(E_NAME_0, "unknown quality property", c.property.span);
                }
                if let (Some(p), Some(property)) = (p, property) {
                    quantiles.push(ir::QuantileMetric {
                        name: c.name.value.clone(),
                        p,
                        property,
                        given_reached: c.given_reached,
                    });
                }
                continue;
            }
            let ast::Item::CohortMetric(c) = item else {
                continue;
            };
            if !metric_names.insert(c.name.value.clone()) {
                self.unsupported(
                    if quantiles.iter().any(|q| q.name == c.name.value) {
                        "duplicate quantile metric name"
                    } else {
                        "duplicate cohort metric"
                    },
                    c.name.span,
                );
            }
            let mut resolve = |n: &ast::Ident| {
                let found = names.get(&n.value).copied();
                if found.is_none() {
                    self.error(E_NAME_0, "unknown quality property", n.span);
                }
                found
            };
            let numerator = resolve(&c.numerator);
            let denominator = match &c.denominator {
                None => Some(ir::Denominator::Arrivals),
                Some(n) => resolve(n).map(ir::Denominator::Property),
            };
            if let (Some(numerator), Some(denominator)) = (numerator, denominator) {
                metrics.push(ir::CohortMetric {
                    name: c.name.value.clone(),
                    numerator,
                    denominator,
                });
            }
        }
        model
            .items
            .iter()
            .any(|i| {
                matches!(
                    i,
                    ast::Item::QualityProperty(_)
                        | ast::Item::CohortMetric(_)
                        | ast::Item::QuantileMetric(_)
                )
            })
            .then_some(ir::QualitySpec {
                properties,
                metrics,
                quantiles,
            })
    }

    fn metrics(
        &mut self,
        model: &ast::Model,
        process: Option<&ir::Process>,
        bounds: Option<&ir::Bounds>,
    ) -> Option<ir::MetricsSpec> {
        let mut result = None;
        for item in &model.items {
            let ast::Item::Metrics(decl) = item else {
                continue;
            };
            if result.is_some() {
                self.unsupported("duplicate metrics declaration", decl.span);
            }
            let (mut window, mut currency) = (None, None);
            let mut breakdown_waits = None;
            let mut actor_rates = vec![None; self.actors.len()];
            let mut start_rates = vec![None; self.capabilities.len()];
            for item in &decl.items {
                match &item.value {
                    ast::MetricsItem::BreakdownWaits => {
                        self.set_once(
                            &mut breakdown_waits,
                            (),
                            "metrics breakdown waits",
                            item.span,
                        );
                    }
                    ast::MetricsItem::Window(d) => {
                        if !matches!(d.unit.value.as_str(), "min" | "h" | "d") {
                            self.error(
                                diag::U_SYN_0,
                                "metrics window unit must be min, h or d",
                                d.unit.span,
                            );
                        }
                        let normalized = self.ticks(d);
                        if let Some(t) = normalized {
                            self.set_once(&mut window, t, "metrics window", item.span);
                        }
                        if normalized.is_some_and(|t| t == 0 || bounds.is_some_and(|b| t < b.clock))
                        {
                            self.error(
                                diag::E_CON_0,
                                "metrics window must be positive and at least bounds.clock",
                                d.span,
                            );
                        }
                    }
                    ast::MetricsItem::Currency(c) => {
                        self.set_once(
                            &mut currency,
                            c.value.clone(),
                            "metrics currency",
                            item.span,
                        );
                        if c.value.trim().is_empty() {
                            self.error(diag::E_CON_0, "metrics currency must not be blank", c.span);
                        }
                    }
                    ast::MetricsItem::OvertimeRate { .. } => {
                        if !model
                            .items
                            .iter()
                            .any(|i| matches!(i, ast::Item::Calendar(_)))
                        {
                            self.unsupported("overtime_rate_requires_calendar", item.span);
                        }
                    }
                    ast::MetricsItem::ActorRate { actor, price } => {
                        if let Some(id) = self.actor_names.get(&actor.value).copied() {
                            self.set_once(
                                &mut actor_rates[id.0],
                                price.value,
                                "metrics Actor rate",
                                item.span,
                            );
                        } else if self.resource_names.contains_key(&actor.value) {
                            self.unsupported(
                                "resource occupancy rates are unsupported",
                                actor.span,
                            );
                        } else {
                            self.error(E_NAME_0, "undefined metrics Actor", actor.span);
                        }
                    }
                    ast::MetricsItem::StartRate { target, price } => {
                        if let Some(id) = self.capability_path(target) {
                            self.set_once(
                                &mut start_rates[id.0],
                                price.value,
                                "metrics start rate",
                                item.span,
                            );
                        }
                    }
                }
            }
            if window.is_none() || currency.is_none() {
                self.error(
                    diag::E_SYN_0,
                    "metrics requires window and currency",
                    decl.span,
                );
            }
            fn calls(b: &ir::Block, caps: &mut BTreeSet<usize>) {
                for s in b {
                    match &s.kind {
                        ir::Stmt::MergeCopy { .. } | ir::Stmt::LoopControl(_) => {}
                        ir::Stmt::Call(c) | ir::Stmt::Query(c, _) => {
                            caps.insert(c.capability.0);
                        }
                        ir::Stmt::Response(c, r) => {
                            caps.insert(c.capability.0);
                            if r.attempt == 1
                                && let Some(b) = &r.recovery
                            {
                                calls(b, caps);
                            }
                        }
                        ir::Stmt::Fork { branches, .. } => {
                            caps.extend(branches.iter().map(|c| c.capability.0))
                        }
                        ir::Stmt::If {
                            then_body,
                            else_body,
                            ..
                        } => {
                            calls(then_body, caps);
                            calls(else_body, caps);
                        }
                        ir::Stmt::End(_)
                        | ir::Stmt::Wait(_)
                        | ir::Stmt::WaitUntil(_)
                        | ir::Stmt::ResolveEffect { .. }
                        | ir::Stmt::StopReceiptUnknown => {}
                    }
                }
            }
            let mut caps = BTreeSet::new();
            if let Some(p) = process {
                calls(&p.body, &mut caps);
            }
            caps.extend(self.route_before.keys().map(|c| c.0));
            let actors: BTreeSet<_> = caps
                .iter()
                .filter_map(|i| self.capabilities.get(*i).map(|c| c.actor.0))
                .collect();
            if result.is_none() {
                result = Some(ir::MetricsSpec {
                    breakdown_waits: breakdown_waits.is_some(),
                    window: window.unwrap_or(0),
                    currency: currency.unwrap_or_default(),
                    actor_rates,
                    start_rates,
                    required_actors: actors.into_iter().map(ir::ActorId).collect(),
                    required_capabilities: caps.into_iter().map(ir::CapabilityId).collect(),
                    span: decl.span,
                });
            }
        }
        result
    }

    fn set_once<T>(&mut self, slot: &mut Option<T>, value: T, field: &str, span: Span) {
        if slot.is_some() {
            self.unsupported(format!("duplicate {field}"), span);
        } else {
            *slot = Some(value);
        }
    }

    fn ticks(&mut self, duration: &ast::Duration) -> Option<ir::Tick> {
        let factor = match duration.unit.value.as_str() {
            "min" => 1,
            "h" => 60,
            "d" => 1440,
            _ => 0,
        };
        if factor == 0 {
            self.unsupported(
                "only integer durations in min, h or d are supported",
                duration.unit.span,
            );
            None
        } else {
            let ticks = duration.amount.value.checked_mul(factor);
            if ticks.is_none() {
                self.unsupported("duration conversion overflow", duration.span);
            }
            ticks
        }
    }

    fn value(&mut self, ty: ir::TypeId, name: &ast::Ident) -> Option<ir::ValueId> {
        let enumeration = &self.types[ty.0];
        if let Some(i) = enumeration.members.iter().position(|v| v == &name.value) {
            Some(ir::ValueId(i))
        } else if self.types.iter().any(|candidate| {
            candidate.name == enumeration.name && candidate.members.contains(&name.value)
        }) {
            // Duplicate types are already unsupported. A member present in any
            // candidate is not a definite type error; never choose a declaration
            // by source order to manufacture E-TYPE-0.
            None
        } else {
            let written = self
                .inline_names
                .get(&ty)
                .cloned()
                .unwrap_or_else(|| enumeration.name.clone());
            self.error(
                E_TYPE_0,
                if self.merge_names.contains(&name.value) {
                    format!(
                        "merge_read_position: {} is not a literal member of {}",
                        name.value, written
                    )
                } else {
                    format!("{} is not a member of {}", name.value, written)
                },
                name.span,
            );
            None
        }
    }

    fn capability(
        &mut self,
        actor: ir::ActorId,
        actor_name: &str,
        cap: &ast::Capability,
        model_span: Span,
    ) {
        if cap
            .items
            .iter()
            .any(|i| matches!(i.value, ast::CapItem::ReceiptQuery { .. }))
            && cap.items.iter().any(|i| {
                !matches!(
                    i.value,
                    ast::CapItem::ReceiptQuery { .. }
                        | ast::CapItem::DurationFixed(_)
                        | ast::CapItem::DurationDistribution(_)
                        | ast::CapItem::Cancel(_)
                        | ast::CapItem::Uses(_)
                )
            })
        {
            self.unsupported("recovery_query_unmodeled", cap.span);
            return;
        }
        let key = (actor_name.to_owned(), cap.name.value.clone());
        if self.cap_names.contains_key(&key) {
            self.unsupported("duplicate Capability declaration", model_span);
            self.ambiguous_caps.insert(key.clone());
        }
        let ty = self
            .inline_results
            .get(&cap.return_type.span.start)
            .copied()
            .filter(|_| cap.inline_result.is_some())
            .or_else(|| self.type_names.get(&cap.return_type.value).copied());
        if ty.is_none() {
            self.error(E_NAME_0, "undefined return type", cap.return_type.span);
        }
        let (mut duration, mut results, mut cancel) = (None, None, None);
        let mut requires = None;
        let mut uses = None;
        let mut qualified_executors = None;
        let mut start = None;
        let mut version_check = None;
        let mut version_seen = false;
        let mut duration_distribution = None;
        let id = ir::CapabilityId(self.capabilities.len());
        let mut params = Vec::new();
        let mut param_names = BTreeSet::new();
        for (index, param) in cap.params.iter().enumerate() {
            if !param_names.insert(&param.name.value) {
                self.unsupported("duplicate parameter name", param.name.span);
            }
            let ty = if param.ty.value == "CaseId" {
                Some(ir::CASE_ID_TYPE)
            } else {
                self.type_names.get(&param.ty.value).copied()
            };
            if ty.is_none() {
                self.error(
                    if param.ty.value == "PersonId" {
                        diag::U_SYN_0
                    } else {
                        E_NAME_0
                    },
                    if param.ty.value == "PersonId" {
                        "PersonId parameter type is unsupported"
                    } else {
                        "undefined parameter type"
                    },
                    param.ty.span,
                );
                self.unknown_param_types.insert((id, index));
            }
            // Preserve the slot even in an erroneous declaration. The unknown
            // marker prevents this internal placeholder from establishing a
            // type mismatch; no erroneous model is returned as valid IR.
            params.push(ir::Parameter {
                name: param.name.value.clone(),
                ty: ty.unwrap_or(ir::TypeId(0)),
                span: param.name.span,
            });
        }
        let (mut reads, mut commits) = (None, None);
        let mut read_variables = Vec::new();
        self.cap_context = Some((actor, params.clone()));
        self.rejected_rule_writes.clear();
        if params.iter().any(|p| p.ty == ir::CASE_ID_TYPE)
            || cap
                .items
                .iter()
                .any(|i| matches!(i.value, ast::CapItem::Atomic { .. }))
        {
            for p in &params {
                if self.state_names.contains_key(&p.name)
                    || self.local_names.contains_key(&(actor, p.name.clone()))
                {
                    self.unsupported("ambiguous CaseId parameter and state name", p.span);
                }
            }
        }
        for item in &cap.items {
            match &item.value {
                ast::CapItem::QualifiedExecutors(names) => {
                    if qualified_executors.is_some() {
                        self.unsupported(
                            "q6_feature_combination: duplicate qualification",
                            item.span,
                        );
                    }
                    let mut people = Vec::new();
                    for name in names {
                        if let Some(index) = self.people.iter().position(|p| p.name == name.value) {
                            let person = ir::PersonId(index);
                            if people.contains(&person)
                                || self.actors[actor.0]
                                    .executors
                                    .as_ref()
                                    .is_some_and(|ps| !ps.contains(&person))
                            {
                                self.error(E_TYPE_0, "qualification_mismatch", name.span);
                            }
                            people.push(person);
                        } else {
                            self.error(E_NAME_0, "undefined qualified person", name.span);
                        }
                    }
                    if self.actors[actor.0].executors.is_none() {
                        self.unsupported(
                            "q6_feature_combination: anonymous qualification",
                            item.span,
                        );
                    }
                    people.sort();
                    qualified_executors = Some(people);
                }
                ast::CapItem::OutcomesPerformance(_) | ast::CapItem::ReceiptQuery { .. } => {}
                ast::CapItem::Idempotency(_) | ast::CapItem::EffectScopeModeled => {}
                ast::CapItem::VersionCheck {
                    variables,
                    reject,
                    resolve,
                } => {
                    if version_seen {
                        self.unsupported("duplicate version_check", item.span);
                    }
                    version_seen = true;
                    let mut ids = Vec::new();
                    for n in variables {
                        let id = self
                            .local_names
                            .get(&(actor, n.value.clone()))
                            .or_else(|| self.state_names.get(&n.value))
                            .copied();
                        if let Some(id) = id {
                            if ids.contains(&id) {
                                self.unsupported("duplicate version_check target", n.span);
                            }
                            ids.push(id);
                            let kind = self.state_variables[id.0].kind;
                            if kind.is_latent() {
                                self.error(
                                    diag::E_POL_LATENT,
                                    "version_check reads latent state",
                                    n.span,
                                );
                            } else if !matches!(kind, ir::StateKind::ActorObserved(a) if a == actor && !self.actors[a.0].is_per_case())
                            {
                                self.unsupported(
                                    "version_check outside shared Actor state is unsupported",
                                    n.span,
                                );
                            }
                        } else {
                            if self.merge_read_name_forbidden(n) {
                                continue;
                            }
                            self.error(E_NAME_0, "undefined version_check target", n.span);
                        }
                    }
                    ids.sort();
                    ids.dedup();
                    if let Some(reject) = ty.and_then(|t| self.value(t, reject)) {
                        version_check = Some(ir::VersionCheck {
                            resolve: *resolve,
                            variables: ids,
                            reject,
                            span: item.span,
                        });
                    }
                }
                ast::CapItem::Uses(entries) => {
                    let mut resolved = Vec::new();
                    let mut seen = BTreeSet::new();
                    for u in entries {
                        if !seen.insert(&u.resource.value) {
                            self.unsupported("duplicate resource use", u.resource.span);
                        }
                        if u.amount.value == 0 {
                            self.error(
                                E_CAP_0,
                                "resource use amount must be positive",
                                u.amount.span,
                            );
                        }
                        if let Some(&resource) = self.resource_names.get(&u.resource.value) {
                            resolved.push(ir::ResourceUse {
                                resource,
                                amount: u.amount.value,
                                span: u.span,
                            });
                        } else {
                            self.error(E_NAME_0, "undefined resource", u.resource.span);
                        }
                    }
                    resolved.sort_by_key(|u| u.resource);
                    self.set_once(&mut uses, resolved, "uses", item.span);
                }
                ast::CapItem::Requires {
                    condition,
                    otherwise,
                } => {
                    let condition = self.requires_condition(condition, actor);
                    let otherwise = ty.and_then(|t| self.literal_value(t, otherwise));
                    self.set_once(
                        &mut requires,
                        condition
                            .zip(otherwise)
                            .map(|(condition, otherwise)| ir::Requires {
                                condition,
                                otherwise,
                                span: item.span,
                            }),
                        "requires",
                        item.span,
                    );
                }
                ast::CapItem::Start(values) => {
                    let (mut names, mut targets) = (BTreeSet::new(), BTreeSet::new());
                    let mut writes = Vec::new();
                    for a in values {
                        let named = !names.insert(target_name(&a.target));
                        let aliased = self
                            .target_id(&a.target, actor)
                            .is_some_and(|id| !targets.insert(id));
                        if named || aliased {
                            self.unsupported("duplicate start target", a.target.span());
                        }
                        if let Some(variable) = self.write_target(&a.target, actor) {
                            if a.value.value == "result" {
                                self.error(
                                    diag::U_SYN_0,
                                    "start result is unsupported",
                                    a.value.span,
                                );
                                continue;
                            }
                            if let Some(source) = self.assignment_source(variable, a) {
                                writes.push(ir::StateWrite { variable, source });
                            }
                        }
                    }
                    self.set_once(&mut start, writes, "start", item.span);
                }
                ast::CapItem::Writes(_)
                | ast::CapItem::Atomic { .. }
                | ast::CapItem::Finish(_)
                | ast::CapItem::OutcomesKernel(_)
                | ast::CapItem::KernelReadsLatent(_) => {}
                ast::CapItem::Reads(names) => {
                    self.unique_members(names, item.span);
                    let mut sources = Vec::new();
                    for name in names {
                        if let Some((prefix, _)) = name.value.split_once('.')
                            && self.merge_names.contains(prefix)
                            && !self.actor_names.contains_key(prefix)
                            && !params.iter().any(|p| p.name == prefix)
                            && self.merge_read_name_forbidden(name)
                        {
                            continue;
                        }
                        if !self.actor_names.contains_key(&name.value)
                            && !self.state_names.contains_key(&name.value)
                            && !params.iter().any(|p| p.name == name.value)
                            && self.merge_read_name_forbidden(name)
                        {
                            continue;
                        }
                        if name.value.contains('.') {
                            self.error(
                                diag::U_SYN_0,
                                "Actor state reads are unsupported",
                                name.span,
                            );
                            continue;
                        }
                        match self.actor_names.get(&name.value).copied() {
                            Some(id) if self.actors[id.0].source.is_some() => sources.push(id),
                            None => {
                                let state = self.state_names.get(&name.value).copied();
                                let param = params.iter().any(|p| p.name == name.value);
                                if state.is_some() && param {
                                    self.error(E_NAME_0, "ambiguous read name", name.span);
                                } else if let Some(id) = state.filter(|id| {
                                    self.state_variables[id.0].kind == ir::StateKind::ObservedCase
                                }) {
                                    read_variables.push(id);
                                } else if !param || state.is_some() {
                                    self.error(
                                        E_NAME_0,
                                        "reads requires a declared source Actor",
                                        name.span,
                                    );
                                }
                            }
                            _ => self.error(
                                E_NAME_0,
                                "reads requires a declared source Actor",
                                name.span,
                            ),
                        }
                    }
                    self.set_once(&mut reads, sources, "reads", item.span);
                }
                ast::CapItem::Commits => self.set_once(&mut commits, true, "commits", item.span),
                ast::CapItem::DurationFixed(d) => {
                    // Keep presence separate from conversion success.
                    let ticks = self.ticks(d);
                    self.set_once(&mut duration, ticks, "duration", cap.span);
                    if d.amount.value > 0 && self.actors[actor.0].capacity.is_none() {
                        self.error(
                            E_CAP_0,
                            "positive duration requires explicit Actor capacity",
                            d.span,
                        );
                    }
                }
                ast::CapItem::DurationDistribution(entries) => {
                    let before = self.errors.len();
                    let mut resolved = Vec::new();
                    let mut seen = BTreeSet::new();
                    let mut sum = crate::probability::Rational::zero();
                    for entry in entries {
                        let d = self.ticks(&entry.duration);
                        let p = self.probability(&entry.probability);
                        if let (Some(d), Some(p)) = (d, p) {
                            if !seen.insert(d) {
                                self.error(diag::E_PROB_0, "duplicate duration value", entry.span);
                            }
                            if !p.is_zero() && d == 0 {
                                self.unsupported(
                                    "positive probability of zero duration is unsupported",
                                    entry.span,
                                );
                            }
                            if !p.is_zero() && d > 0 && self.actors[actor.0].capacity.is_none() {
                                self.error(
                                    E_CAP_0,
                                    "positive duration requires explicit Actor capacity",
                                    entry.duration.span,
                                );
                            }
                            sum.0 += &p.0;
                            resolved.push((d, p));
                        }
                    }
                    if sum != crate::probability::Rational::one() {
                        self.error(
                            diag::E_PROB_0,
                            "duration probabilities must sum to one",
                            item.span,
                        );
                    }
                    resolved.sort_by_key(|(d, _)| *d);
                    let valid = self.errors.len() == before;
                    self.set_once(&mut duration, valid.then_some(0), "duration", cap.span);
                    duration_distribution = Some(ir::DurationDistribution { entries: resolved });
                    if !valid {
                        self.uncertain_durations.insert(id);
                    }
                }
                ast::CapItem::ResultNondet(values) => {
                    if values.iter().all(|v| v.fields.is_none()) {
                        self.unique_members(
                            &values.iter().map(|v| v.head.clone()).collect::<Vec<_>>(),
                            item.span,
                        );
                    }
                    let resolved: Vec<_> = values
                        .iter()
                        .filter_map(|v| ty.and_then(|t| self.literal_value(t, v)))
                        .collect();
                    if values.iter().any(|v| v.fields.is_some())
                        && resolved.iter().collect::<BTreeSet<_>>().len() != resolved.len()
                    {
                        self.unsupported("duplicate list member", item.span);
                    }
                    self.set_once(&mut results, resolved, "result nondet", cap.span);
                }
                ast::CapItem::Cancel(c) => {
                    let resolved = match c {
                        ast::Cancel::Immediate => ir::Cancel::Immediate,
                        ast::Cancel::ForbiddenAfterStart => ir::Cancel::ForbiddenAfterStart,
                        ast::Cancel::Cooperative { ack_within } => {
                            let d = self.ticks(ack_within);
                            if d == Some(0) {
                                self.unsupported(
                                    "ack_within must be at least one tick",
                                    ack_within.span,
                                );
                            }
                            // Placeholder remains internal to an erroneous model.
                            ir::Cancel::Cooperative {
                                ack_within: d.unwrap_or(0),
                            }
                        }
                    };
                    self.set_once(&mut cancel, resolved, "cancel", cap.span);
                }
            }
        }
        if duration.is_none() {
            self.unsupported("Capability requires explicit duration", cap.span);
        }
        if results.is_none()
            && !cap
                .items
                .iter()
                .any(|i| matches!(i.value, ast::CapItem::Atomic { .. }))
            && !cap.items.iter().any(|i| {
                matches!(
                    i.value,
                    ast::CapItem::OutcomesKernel(_)
                        | ast::CapItem::OutcomesPerformance(_)
                        | ast::CapItem::ReceiptQuery { .. }
                )
            })
        {
            self.unsupported("Capability requires explicit result set", cap.span);
        }
        self.cap_names.entry(key).or_insert(id);
        if ty.is_some() {
            self.known_cap_types.insert(id);
        }
        if cap
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item.value,
                    ast::CapItem::DurationFixed(_) | ast::CapItem::DurationDistribution(_)
                )
            })
            .count()
            != 1
            || cap.items.iter().any(|item| {
                matches!(&item.value,
                ast::CapItem::DurationFixed(d) if !matches!(d.unit.value.as_str(), "min" | "h" | "d"))
            })
        {
            self.uncertain_durations.insert(id);
        }
        let atomic = self.atomic(cap, actor, ty);
        if let Some(a) = &atomic {
            results = Some(if a.commit_when {
                vec![a.commit, a.reject]
            } else {
                vec![a.reject, a.commit]
            });
        }
        let writes = if cap
            .items
            .iter()
            .any(|i| matches!(i.value, ast::CapItem::Atomic { .. }))
        {
            Vec::new()
        } else {
            self.state_writes(cap, actor, ty)
        };
        read_variables.sort();
        read_variables.dedup();
        let (kernel, kernel_reads_latent, mut kernel_writes) = if cap
            .items
            .iter()
            .any(|i| matches!(i.value, ast::CapItem::Atomic { .. }))
        {
            (None, Vec::new(), Vec::new())
        } else {
            self.kernel_use(cap, actor, ty, &writes)
        };
        if let Some(k) = &kernel {
            let support: BTreeSet<_> = k
                .rows
                .iter()
                .flat_map(|r| &r.branches)
                .filter(|b| !b.probability.is_zero())
                .map(|b| b.result)
                .collect();
            results = Some(support.into_iter().collect());
        }
        let performance =
            self.performance_use(cap, actor, ty, &writes, qualified_executors.as_deref());
        if let Some(source) = &performance {
            let support: BTreeSet<_> = source
                .kernels
                .iter()
                .flat_map(|k| &k.rows)
                .flat_map(|r| &r.branches)
                .filter(|b| !b.probability.is_zero())
                .map(|b| b.result)
                .collect();
            results = Some(support.into_iter().collect());
            if let Some(first) = source.kernels.first() {
                kernel_writes = first
                    .rows
                    .iter()
                    .flat_map(|r| &r.branches)
                    .flat_map(|b| &b.overrides)
                    .map(|(v, _)| *v)
                    .filter(|v| self.state_variables[v.0].kind.is_latent())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
            }
        }
        if let Some(v) = &version_check {
            for id in &v.variables {
                if !writes
                    .iter()
                    .chain(atomic.iter().flat_map(|a| &a.writes))
                    .chain(start.iter().flatten())
                    .any(|w| w.variable == *id)
                {
                    self.unsupported("version_check target is not written", v.span);
                }
                if start.iter().flatten().any(|w| w.variable == *id) {
                    self.unsupported(
                        "version_check with start writes to checked state is unsupported",
                        v.span,
                    );
                }
            }
            if results.as_ref().is_some_and(|rs| rs.contains(&v.reject))
                || requires
                    .as_ref()
                    .and_then(|r| r.as_ref())
                    .is_some_and(|r| r.otherwise == v.reject)
            {
                self.unsupported("version_check else value overlaps a result", v.span);
            }
            if kernel.is_some() {
                self.unsupported("version_check with kernel outcomes is unsupported", v.span);
            }
        }
        let receipt_query = None;
        if cap
            .items
            .iter()
            .any(|i| matches!(i.value, ast::CapItem::ReceiptQuery { .. }))
            && results.is_none()
        {
            results = ty.map(|t| {
                (0..self.types[t.0].members.len())
                    .map(ir::ValueId)
                    .collect()
            });
        }
        self.capabilities.push(ir::Capability {
            receipt_query,
            performance,
            qualified_executors,
            version_check,
            atomic,
            uses: uses.unwrap_or_default(),
            requires: requires.flatten(),
            start_writes: start.unwrap_or_default(),
            kernel,
            kernel_reads_latent,
            kernel_writes,
            writes,
            permission: None,
            params,
            reads: reads.unwrap_or_default(),
            read_variables,
            commits: commits.unwrap_or(false),
            name: cap.name.value.clone(),
            actor,
            return_type: ty.unwrap_or(ir::TypeId(0)),
            duration: duration.flatten().unwrap_or(0),
            duration_distribution,
            results: results.unwrap_or_default(),
            cancel: cancel.unwrap_or(ir::Cancel::Unspecified),
            span: cap.span,
        });
        if !cap
            .items
            .iter()
            .any(|i| matches!(i.value, ast::CapItem::ReceiptQuery { .. }))
        {
            self.response_capability(id, cap);
        }
        self.cap_context = None;
    }

    fn membership(
        &mut self,
        element: &ast::Path,
        set: &ast::Path,
        owner: Option<ir::ActorId>,
    ) -> Option<ir::Cond> {
        let element = match element.value.as_slice() {
            [a, b] if a.value == "case" && b.value == "id" => ir::Element::CurrentCase,
            [n] if owner.is_some() => {
                let params = self
                    .cap_context
                    .as_ref()
                    .map(|(_, p)| p.as_slice())
                    .unwrap_or(&[]);
                let Some(i) = params.iter().position(|p| p.name == n.value) else {
                    self.error(diag::U_SYN_0, "membership at unsupported location", n.span);
                    return None;
                };
                if params[i].ty != ir::CASE_ID_TYPE {
                    self.error(diag::U_SYN_0, "membership at unsupported location", n.span);
                    return None;
                }
                ir::Element::Parameter(i)
            }
            _ => {
                self.error(
                    diag::U_SYN_0,
                    "membership at unsupported location",
                    element.span,
                );
                return None;
            }
        };
        let id = match set.value.as_slice() {
            [a, v] => self.local_state(a, v),
            [n] => owner.and_then(|a| self.local_names.get(&(a, n.value.clone())).copied()),
            _ => None,
        };
        let Some(set_id) = id else {
            self.error(E_NAME_0, "undefined set state", set.span);
            return None;
        };
        if self.state_variables[set_id.0].case_set_size.is_none() {
            self.error(E_TYPE_0, "membership requires set<CaseId>", set.span);
            return None;
        }
        Some(ir::Cond::Contains {
            set: set_id,
            element,
        })
    }

    fn atomic(
        &mut self,
        cap: &ast::Capability,
        actor: ir::ActorId,
        ty: Option<ir::TypeId>,
    ) -> Option<ir::Atomic> {
        let mut atomic = None;
        for item in &cap.items {
            let ast::CapItem::Atomic {
                condition,
                then_branch,
                else_branch,
            } = &item.value
            else {
                continue;
            };
            if ty.is_some_and(|t| self.is_record(t)) {
                // ADR 0038: record results and atomic do not coexist (U-SEM-0),
                // before the branch results are read as members.
                self.unsupported("atomic with record result is unsupported", item.span);
                return None;
            }
            if atomic.is_some() {
                self.unsupported("duplicate atomic", item.span);
            }
            if cap
                .items
                .iter()
                .filter(|i| matches!(i.value, ast::CapItem::Writes(_)))
                .count()
                > 1
            {
                self.unsupported("duplicate writes", item.span);
            }
            let mut declared_targets = BTreeSet::new();
            for i in &cap.items {
                if let ast::CapItem::Writes(ws) = &i.value {
                    for target in ws {
                        if let Some(id) = self.target_id(target, actor)
                            && !declared_targets.insert(id)
                        {
                            self.unsupported("duplicate list member", target.span());
                        }
                    }
                }
            }
            for other in &cap.items {
                let label = match other.value {
                    ast::CapItem::ResultNondet(_) => "result nondet",
                    ast::CapItem::OutcomesKernel(_) => "outcomes",
                    ast::CapItem::KernelReadsLatent(_) => "kernel_reads",
                    ast::CapItem::Finish(_) => "finish",
                    _ => continue,
                };
                self.unsupported(format!("atomic with {label} is unsupported"), other.span);
            }
            let condition = self.requires_condition(condition, actor);
            let then_result = ty.and_then(|t| self.value(t, &then_branch.result));
            let else_result = ty.and_then(|t| self.value(t, &else_branch.result));
            let commit_when = !then_branch.assignments.is_empty();
            if then_branch.assignments.is_empty() == else_branch.assignments.is_empty()
                || then_result.zip(else_result).is_some_and(|(a, b)| a == b)
            {
                self.unsupported("atomic branch shape is unsupported", item.span);
            }
            let branch = if commit_when {
                then_branch
            } else {
                else_branch
            };
            let declared: Vec<_> = cap
                .items
                .iter()
                .filter_map(|i| {
                    if let ast::CapItem::Writes(ws) = &i.value {
                        Some(ws)
                    } else {
                        None
                    }
                })
                .flatten()
                .filter_map(|t| self.write_target(t, actor))
                .collect();
            let mut writes = Vec::new();
            let mut seen = BTreeSet::new();
            for a in &branch.assignments {
                if let Some(variable) = self.write_target(&a.target, actor) {
                    if !seen.insert(variable) {
                        self.unsupported("duplicate atomic target", a.target.span());
                    }
                    if !declared.contains(&variable) {
                        self.error(
                            E_NAME_0,
                            "atomic target not declared in writes",
                            a.target.span(),
                        );
                    }
                    if let Some(source) = self.assignment_source(variable, a) {
                        // ADR 0038: record state is never written inside atomic.
                        // Any value lower would accept here (only `none` into an
                        // optional record; literals stop at the parser) is refused
                        // at the target, before the public IR check (span-less).
                        if self.is_record(self.state_variables[variable.0].base) {
                            self.unsupported("atomic record write is unsupported", a.target.span());
                        } else {
                            writes.push(ir::StateWrite { variable, source });
                        }
                    }
                }
            }
            if declared.iter().any(|v| !seen.contains(v)) && !branch.assignments.iter().any(|a| matches!(&a.target, ast::StateTarget::Qualified { actor: a, .. } if self.actor_names.get(&a.value) != Some(&actor))) { self.unsupported("atomic writes require an assignment in the commit branch", item.span); }
            if let (Some(condition), Some(t), Some(e)) = (condition, then_result, else_result) {
                atomic = Some(ir::Atomic {
                    condition,
                    commit_when,
                    commit: if commit_when { t } else { e },
                    reject: if commit_when { e } else { t },
                    writes,
                    span: item.span,
                });
            }
        }
        atomic
    }

    fn collect(&mut self, block: &ast::Block) {
        for stmt in block {
            match &stmt.value {
                ast::Stmt::Repeat {
                    name,
                    at_most,
                    body,
                    on_limit,
                    ..
                } => self.collect_loop(name, at_most, body, on_limit),
                ast::Stmt::Again | ast::Stmt::Leave | ast::Stmt::Yield(_) => {}
                ast::Stmt::Call(call) => {
                    self.collect_call(call);
                    if let Some(body) = &call.recovery {
                        self.collect(body);
                    }
                }
                ast::Stmt::Fork { name, branches, .. } => {
                    if self.fork_names.contains_key(&name.value) {
                        if self.ambiguous_forks.insert(name.value.clone()) {
                            self.unsupported("duplicate fork name", self.fork_spans[&name.value]);
                        }
                        self.unsupported("duplicate fork name", name.span);
                    } else {
                        self.fork_names
                            .insert(name.value.clone(), ir::ForkId(self.forks.len()));
                        self.forks.push(name.value.clone());
                        self.fork_spans.insert(name.value.clone(), name.span);
                    }
                    if branches.is_empty() {
                        self.unsupported("empty fork", stmt.span);
                    }
                    for call in branches {
                        if call.retry.is_some()
                            || call
                                .response_loss
                                .as_ref()
                                .is_some_and(|r| !matches!(r.value, ast::ResponseLoss::None))
                        {
                            self.unsupported(
                                "retry_feature_combination: fork response channel",
                                call.span,
                            );
                        }
                        self.collect_call(call);
                    }
                }
                ast::Stmt::Merge {
                    binding,
                    then_body,
                    else_body,
                    ..
                } => {
                    self.collect_merge(binding);
                    self.collect(then_body);
                    self.collect(else_body);
                }
                ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    self.collect(then_body);
                    self.collect(else_body);
                }
                ast::Stmt::End(_)
                | ast::Stmt::Wait(_)
                | ast::Stmt::WaitUntil(_)
                | ast::Stmt::ResolveEffect { .. }
                | ast::Stmt::StopReceiptUnknown => {} // Already collected model-wide.
            }
        }
    }

    fn collect_ends(&mut self, block: &ast::Block) {
        for stmt in block {
            match &stmt.value {
                ast::Stmt::Repeat { body, on_limit, .. } => {
                    self.collect_ends(body);
                    self.collect_ends(on_limit);
                }
                ast::Stmt::Again | ast::Stmt::Leave | ast::Stmt::Yield(_) => {}
                ast::Stmt::End(name) => {
                    if !self.end_names.contains_key(&name.value) {
                        self.end_names
                            .insert(name.value.clone(), ir::EndId(self.ends.len()));
                        self.ends.push(name.value.clone());
                    }
                }
                ast::Stmt::Merge {
                    then_body,
                    else_body,
                    ..
                }
                | ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    self.collect_ends(then_body);
                    self.collect_ends(else_body);
                }
                ast::Stmt::Call(call) => {
                    if let Some(body) = &call.recovery {
                        self.collect_ends(body);
                    }
                    if let Some(r) = &call.retry {
                        let name = &r.exhausted;
                        if !self.end_names.contains_key(&name.value) {
                            self.end_names
                                .insert(name.value.clone(), ir::EndId(self.ends.len()));
                            self.ends.push(name.value.clone());
                        }
                    }
                }
                ast::Stmt::Fork { .. }
                | ast::Stmt::Wait(_)
                | ast::Stmt::WaitUntil(_)
                | ast::Stmt::ResolveEffect { .. }
                | ast::Stmt::StopReceiptUnknown => {}
            }
        }
    }

    fn collect_call(&mut self, call: &ast::Call) {
        if call.response_loss.is_some() || call.retry.is_some() || call.recovery.is_some() {
            self.unstable_merge_sources
                .insert(call.binding.value.clone());
        }
        self.response_contract(call);
        let actor = self.actor_names.get(&call.actor.value).copied();
        if actor.is_none() {
            self.error(
                E_NAME_0,
                if self.merge_names.contains(&call.actor.value) {
                    "merge_read_position: a result value is not an Actor"
                } else {
                    "undefined Actor"
                },
                call.actor.span,
            );
        }
        let cap = self
            .cap_names
            .get(&(call.actor.value.clone(), call.capability.value.clone()))
            .copied();
        if cap.is_none() && actor.is_some() {
            self.error(E_NAME_0, "undefined Capability", call.capability.span);
        }
        let candidates = self.call_candidates(call);
        if candidates.is_empty() || candidates.iter().any(|c| !self.known_cap_types.contains(c)) {
            self.unknown_binding_types
                .insert(call.binding.value.clone());
        }
        for c in &candidates {
            if self.known_cap_types.contains(c) {
                self.binding_types
                    .entry(call.binding.value.clone())
                    .or_default()
                    .insert(self.capabilities[c.0].return_type);
            }
        }
        let ambiguous = self
            .ambiguous_caps
            .contains(&(call.actor.value.clone(), call.capability.value.clone()))
            || self.ambiguous_actors.contains(&call.actor.value);
        // Validate the occurrence before deciding whether it can be registered.
        if let Some(capability) = cap.filter(|_| !ambiguous) {
            self.called_actors
                .insert(self.capabilities[capability.0].actor);
            if actor.is_some_and(|a| self.actors[a.0].capacity.is_none())
                && self.capabilities[capability.0].duration == 0
                && !self.uncertain_durations.contains(&capability)
                && self.capabilities[capability.0]
                    .duration_distribution
                    .is_none()
            {
                self.unsupported(
                    "calling a zero-duration Capability without capacity is unspecified",
                    call.span,
                );
            }
        }
        if let Some(&previous) = self.binding_names.get(&call.binding.value) {
            if self.ambiguous_bindings.insert(call.binding.value.clone()) {
                self.unsupported(
                    "duplicate or shadowed binding",
                    self.bindings[previous.0].span,
                );
            }
            self.unsupported("duplicate or shadowed binding", call.binding.span);
            self.calls.remove(&call.binding.value);
            // Only registration is skipped; all occurrence-local checks ran above.
            return;
        }
        let binding = ir::BindingId(self.bindings.len());
        self.binding_names
            .insert(call.binding.value.clone(), binding);
        self.bindings.push(ir::Binding {
            merge: None,
            repetition: None,
            name: call.binding.value.clone(),
            ty: cap
                .map(|c| self.capabilities[c.0].return_type)
                .unwrap_or(ir::TypeId(0)),
            span: call.binding.span,
        });
        if let Some(capability) = cap.filter(|c| !ambiguous && self.known_cap_types.contains(c)) {
            self.calls.insert(
                call.binding.value.clone(),
                ir::Call {
                    args: Vec::new(),
                    binding,
                    capability,
                    span: call.span,
                },
            );
        }
    }

    fn end(&mut self, name: &ast::Ident) -> Option<ir::EndId> {
        let end = self.end_names.get(&name.value).copied();
        if end.is_none() {
            self.error(E_NAME_0, "undefined terminal", name.span);
        }
        end
    }

    fn terminal(&mut self, end: ir::EndId, histories: &BTreeSet<bool>) {
        if !self.check_flow {
            return;
        }
        self.terminal_histories
            .entry(end)
            .or_default()
            .extend(histories);
    }

    fn record_end_before(&mut self, end: ir::EndId, available: &BTreeSet<ir::BindingId>) {
        self.end_before
            .entry(end)
            .and_modify(|old| {
                *old = old.intersection(available).copied().collect();
            })
            .or_insert_with(|| available.clone());
    }

    fn block(&mut self, block: &ast::Block, flow: &mut Flow) -> ir::Block {
        self.block_depth += 1;
        let mut resolved = Vec::new();
        for stmt in block {
            let kind = match &stmt.value {
                ast::Stmt::Merge {
                    binding,
                    condition,
                    then_body,
                    else_body,
                } => self.merge_block(binding, condition, then_body, else_body, flow),
                ast::Stmt::Yield(source) => self.merge_copy(source, stmt.span, flow),
                ast::Stmt::Repeat {
                    name,
                    at_most,
                    gap,
                    body,
                    on_limit,
                } => {
                    resolved.extend(self.expand_loop(
                        name,
                        at_most,
                        gap.as_ref(),
                        body,
                        on_limit,
                        flow,
                        stmt.span,
                    ));
                    None
                }
                ast::Stmt::Again | ast::Stmt::Leave => {
                    let leave = matches!(stmt.value, ast::Stmt::Leave);
                    if let Some(context) = &mut self.loop_context {
                        if leave {
                            context.leaves.push(flow.clone());
                        } else {
                            context.agains.push(flow.clone());
                        }
                        flow.histories.clear();
                        let mut updates = vec![(context.continuing, ir::StateValue::Integer(0))];
                        if leave {
                            updates.push((context.active, ir::StateValue::Integer(0)));
                        }
                        let kind = ir::Stmt::LoopControl(updates.into());
                        resolved.push(ir::Statement {
                            kind,
                            span: stmt.span,
                        });
                    } else {
                        self.error(diag::E_CON_0, "loop control outside repeat", stmt.span);
                    }
                    break;
                }
                ast::Stmt::ResolveEffect { command, evidence } => {
                    match (
                        self.binding_names.get(&command.value).copied(),
                        self.binding_names.get(&evidence.value).copied(),
                    ) {
                        (Some(command), Some(evidence)) => {
                            Some(ir::Stmt::ResolveEffect { command, evidence })
                        }
                        _ => {
                            self.error(E_NAME_0, "unknown_recovery_reference", stmt.span);
                            None
                        }
                    }
                }
                ast::Stmt::StopReceiptUnknown => {
                    flow.histories.clear();
                    Some(ir::Stmt::StopReceiptUnknown)
                }
                ast::Stmt::WaitUntil(c) => {
                    if self.merge_condition_forbidden(c) {
                        None
                    } else {
                        self.condition(c, flow).map(ir::Stmt::WaitUntil)
                    }
                }
                ast::Stmt::Wait(d) => self.ticks(d).map(ir::Stmt::Wait),
                ast::Stmt::Call(ast_call) => {
                    let mut renamed = ast_call.clone();
                    if self.loop_context.is_some()
                        && let Some(&id) = self.binding_names.get(&ast_call.binding.value)
                    {
                        renamed.binding.value = self.bindings[id.0].name.clone();
                    }
                    if let Some(call) = self.call_arguments(&renamed, flow) {
                        self.loop_call_scope(&call);
                        flow.available.insert(call.binding);
                        if let Some(alias) = self.loop_result_alias(call.binding) {
                            flow.available.insert(alias);
                        }
                        if let Some(command) = &ast_call.query_command {
                            self.binding_names
                                .get(&command.value)
                                .copied()
                                .map(|command| ir::Stmt::Query(call, command))
                                .or_else(|| {
                                    self.error(
                                        E_NAME_0,
                                        "unknown_recovery_reference",
                                        command.span,
                                    );
                                    None
                                })
                        } else if ast_call.response_loss.is_some()
                            || ast_call.retry.is_some()
                            || ast_call.recovery.is_some()
                        {
                            let mut expanded = self.expand_response(ast_call, call);
                            if let Some(body) = &ast_call.recovery {
                                let mut handler_flow = flow.clone();
                                let previous = self.in_recovery_handler;
                                self.in_recovery_handler = true;
                                let handler = self.block(body, &mut handler_flow);
                                self.in_recovery_handler = previous;
                                for s in &mut expanded {
                                    if let ir::Stmt::Response(_, r) = &mut s.kind {
                                        r.recovery = Some(handler.clone());
                                    }
                                }
                            }
                            resolved.extend(expanded);
                            None
                        } else {
                            Some(ir::Stmt::Call(call))
                        }
                    } else {
                        None
                    }
                }
                ast::Stmt::Fork {
                    name,
                    branches,
                    join,
                } => {
                    let calls: Vec<_> = branches
                        .iter()
                        .filter_map(|c| self.call_arguments(c, flow))
                        .collect();
                    for call in &calls {
                        self.loop_call_scope(call);
                    }
                    for (i, a) in calls.iter().enumerate() {
                        for b in &calls[i + 1..] {
                            if self.capabilities[a.capability.0]
                                .finish_writes()
                                .chain(self.capabilities[a.capability.0].start_writes.iter())
                                .map(|w| w.variable)
                                .chain(
                                    self.capabilities[a.capability.0]
                                        .kernel_writes
                                        .iter()
                                        .copied(),
                                )
                                .any(|x| {
                                    self.capabilities[b.capability.0]
                                        .finish_writes()
                                        .chain(
                                            self.capabilities[b.capability.0].start_writes.iter(),
                                        )
                                        .map(|w| w.variable)
                                        .chain(
                                            self.capabilities[b.capability.0]
                                                .kernel_writes
                                                .iter()
                                                .copied(),
                                        )
                                        .any(|y| x == y)
                                })
                            {
                                self.unsupported(
                                    "fork siblings write the same observed variable",
                                    stmt.span,
                                );
                            }
                        }
                    }
                    self.check_join_branches(branches, join);
                    let join = self.join(name, &calls, join, flow);
                    self.fork_names
                        .get(&name.value)
                        .copied()
                        .zip(join)
                        .map(|(id, join)| ir::Stmt::Fork {
                            id,
                            branches: calls,
                            join,
                        })
                }
                ast::Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let cond = self.condition(condition, flow);
                    let truth = cond.as_ref().map(|c| self.truth(c)).unwrap_or((true, true));
                    let (mut yes, mut no) = (flow.clone(), flow.clone());
                    if !truth.0 {
                        yes.histories.clear();
                    }
                    if !truth.1 {
                        no.histories.clear();
                    }
                    let then_body = self.block(then_body, &mut yes);
                    let else_body = self.block(else_body, &mut no);
                    flow.available = match (yes.histories.is_empty(), no.histories.is_empty()) {
                        (false, false) => {
                            yes.available.intersection(&no.available).copied().collect()
                        }
                        (false, true) => yes.available,
                        (true, false) => no.available,
                        (true, true) => flow.available.clone(),
                    };
                    flow.histories = yes.histories.union(&no.histories).copied().collect();
                    cond.map(|condition| ir::Stmt::If {
                        condition,
                        then_body,
                        else_body,
                    })
                }
                ast::Stmt::End(name) => {
                    let end = self.end(name);
                    if let Some(end) = end {
                        self.terminal(end, &flow.histories);
                        self.record_end_before(end, &flow.available);
                    }
                    flow.histories.clear();
                    end.map(ir::Stmt::End)
                }
            };
            if let Some(kind) = kind {
                let statement = ir::Statement {
                    kind,
                    span: stmt.span,
                };
                if let Some(context) = &self.loop_context {
                    resolved.push(ir::Statement {
                        kind: ir::Stmt::If {
                            condition: ir::Cond::IntCompare {
                                variable: context.continuing,
                                operator: ir::IntComparison::Eq,
                                value: 1,
                            },
                            then_body: vec![statement],
                            else_body: vec![],
                        },
                        span: stmt.span,
                    });
                } else {
                    resolved.push(statement);
                }
            }
        }
        self.block_depth -= 1;
        resolved
    }

    fn call_arguments(&mut self, ast: &ast::Call, flow: &Flow) -> Option<ir::Call> {
        let mut call = self.calls.get(&ast.binding.value).cloned();
        if let Some(c) = &call {
            self.call_before.insert(c.binding, flow.available.clone());
        }
        let params = call
            .as_ref()
            .map(|c| self.capabilities[c.capability.0].params.clone());
        if params.as_ref().is_some_and(|p| p.len() != ast.args.len()) {
            self.error(
                E_TYPE_0,
                "argument count does not match parameters",
                ast.span,
            );
        }
        let mut args = Vec::new();
        for (i, arg) in ast.args.iter().enumerate() {
            let case_param = params
                .as_ref()
                .and_then(|p| p.get(i))
                .is_some_and(|p| p.ty == ir::CASE_ID_TYPE);
            if arg.value == "case.id" {
                if !case_param {
                    self.error(
                        E_TYPE_0,
                        "case.id argument requires a CaseId parameter",
                        arg.span,
                    );
                }
                args.push(ir::CURRENT_CASE);
                continue;
            }
            if case_param {
                self.error(E_TYPE_0, "CaseId parameter requires case.id", arg.span);
                continue;
            }
            let Some(&id) = self.binding_names.get(&arg.value) else {
                self.error(E_NAME_0, "undefined argument binding", arg.span);
                continue;
            };
            if let Some(param) = params.as_ref().and_then(|p| p.get(i))
                && !call
                    .as_ref()
                    .is_some_and(|c| self.unknown_param_types.contains(&(c.capability, i)))
                && !self.unknown_binding_types.contains(&arg.value)
                && self.binding_types.get(&arg.value).is_some_and(|types| {
                    !types.is_empty() && types.iter().all(|ty| *ty != param.ty)
                })
            {
                self.error(E_TYPE_0, "argument type does not match parameter", arg.span);
            }
            let current_observed_alias = self.multi_case
                && self
                    .state_names
                    .get(&arg.value)
                    .is_some_and(|v| self.state_variables[v.0].kind == ir::StateKind::ObservedCase);
            if self.check_flow
                && !flow.available.contains(&id)
                && (self.in_recovery_handler || !current_observed_alias)
            {
                self.unsupported("argument result is not definitely available", arg.span);
            }
            args.push(id);
        }
        if let Some(call) = &mut call {
            call.args = args;
        }
        call
    }

    fn condition(&mut self, cond: &ast::Spanned<ast::Cond>, flow: &Flow) -> Option<ir::Cond> {
        match &cond.value {
            ast::Cond::Contains { element, set } => self.membership(element, set, None),
            ast::Cond::Compare {
                binding,
                operator,
                value,
            } => self.integer_comparison(binding, operator, value),
            ast::Cond::LocalEq {
                actor,
                variable,
                operator,
                value,
            } => {
                if let Some(c) = self.record_comparison(
                    actor,
                    variable,
                    operator,
                    value,
                    RecordContext::Process(self.check_flow.then_some(&flow.available)),
                ) {
                    c
                } else {
                    self.local_comparison(actor, variable, operator, value)
                }
            }
            ast::Cond::Eq { binding, value } => {
                if let Some(&id) = self.state_names.get(&binding.value) {
                    return self.state_comparison(id, value);
                }
                let Some(&id) = self.binding_names.get(&binding.value) else {
                    self.error(E_NAME_0, "undefined result binding", binding.span);
                    return None;
                };
                if self.ambiguous_bindings.contains(&binding.value)
                    || (!self.calls.contains_key(&binding.value)
                        && !self.merge_names.contains(&binding.value))
                {
                    // Check the fact common to all candidates, not the first
                    // declaration. Unknown types cannot establish a type error.
                    if !self.unknown_binding_types.contains(&binding.value) {
                        let types = self
                            .binding_types
                            .get(&binding.value)
                            .cloned()
                            .unwrap_or_default();
                        self.check_candidate_value(&types, value);
                    }
                    return None;
                }
                // Resolve the member first: unsupported availability never hides
                // a known type error, including after a cancelling join.
                let ty = self.bindings[id.0].ty;
                let member = if ty.0 < self.types.len() {
                    if self.is_record(ty) {
                        self.error(
                            diag::U_SYN_0,
                            "record comparison is unsupported",
                            value.span,
                        );
                        return None;
                    }
                    self.value(ty, value)
                } else {
                    None
                };
                if self.check_flow && !flow.available.contains(&id) {
                    self.unsupported("result is not definitely available (not yet defined or possibly cancelled)", binding.span);
                }
                member.map(|value| ir::Cond::Eq(ir::ResultMatch { binding: id, value }))
            }
            ast::Cond::Group(inner) => self.condition(inner, flow),
            ast::Cond::And(parts) | ast::Cond::Or(parts) => {
                if parts.len() < 2 {
                    self.unsupported("condition chain requires at least two operands", cond.span);
                }
                let checked: Vec<_> = parts.iter().map(|p| self.condition(p, flow)).collect();
                let conditions: Option<Vec<_>> = checked.into_iter().collect();
                conditions.map(|c| {
                    if matches!(cond.value, ast::Cond::And(_)) {
                        ir::Cond::And(c)
                    } else {
                        ir::Cond::Or(c)
                    }
                })
            }
        }
    }

    // ADR 0032: a bare requires name reaches only the owner's observed state;
    // the owner's latent and undefined names keep the case-variable path.
    fn owner_observed(&self, owner: ir::ActorId, name: &str) -> Option<ir::StateVarId> {
        self.local_names
            .get(&(owner, name.to_string()))
            .copied()
            .filter(|id| {
                matches!(
                    self.state_variables[id.0].kind,
                    ir::StateKind::ActorObserved(_) | ir::StateKind::Rule(_)
                )
            })
    }

    fn requires_condition(
        &mut self,
        cond: &ast::Spanned<ast::Cond>,
        owner: ir::ActorId,
    ) -> Option<ir::Cond> {
        if self.merge_condition_forbidden(cond) {
            return None;
        }
        match &cond.value {
            ast::Cond::Contains { element, set } => self.membership(element, set, Some(owner)),
            ast::Cond::Compare {
                binding,
                operator,
                value,
            } => {
                if !self.state_names.contains_key(&binding.value)
                    && let Some(id) = self.owner_observed(owner, &binding.value)
                {
                    return self.resolved_comparison(id, operator, value, true);
                }
                self.policy_condition(cond);
                self.integer_comparison(binding, operator, value)
            }
            ast::Cond::Eq { binding, value } => {
                let Some(id) = self
                    .state_names
                    .get(&binding.value)
                    .copied()
                    .or_else(|| self.owner_observed(owner, &binding.value))
                else {
                    self.error(
                        E_NAME_0,
                        "requires needs an observed case variable",
                        binding.span,
                    );
                    return None;
                };
                if self.state_variables[id.0].kind.is_latent() {
                    self.error(
                        diag::E_POL_LATENT,
                        "requires cannot read latent state",
                        binding.span,
                    );
                }
                self.state_comparison(id, value)
            }
            ast::Cond::LocalEq {
                actor,
                variable,
                operator,
                value,
            } => {
                if variable.value == "round"
                    && let Some(&id) = self.loop_variables.get(&actor.value)
                {
                    return self.resolved_comparison(id, operator, value, true);
                }
                if let Some(c) = self.record_comparison(
                    actor,
                    variable,
                    operator,
                    value,
                    RecordContext::StateOnly,
                ) {
                    return c;
                }
                let id = self
                    .actor_names
                    .get(&actor.value)
                    .and_then(|a| self.local_names.get(&(*a, variable.value.clone())))
                    .copied();
                if let Some(id) = id
                    && operator.value != ast::IntComparison::Eq
                    && self.state_variables[id.0].integer_max.is_none()
                {
                    return self.resolved_comparison(id, operator, value, true);
                }
                if id.is_some_and(|id| self.state_variables[id.0].kind.is_latent()) {
                    self.error(
                        diag::E_POL_LATENT,
                        "requires cannot read Actor latent",
                        cond.span,
                    );
                    None
                } else if let Some(id) = id {
                    self.resolved_comparison(id, operator, value, true)
                } else if self
                    .state_variables
                    .iter()
                    .any(|v| matches!(v.kind, ir::StateKind::ActorObserved(_)))
                {
                    self.local_state(actor, variable);
                    None
                } else {
                    self.error(
                        diag::U_SYN_0,
                        "qualified requires is unsupported",
                        cond.span,
                    );
                    None
                }
            }
            ast::Cond::Group(c) => self.requires_condition(c, owner),
            ast::Cond::And(cs) | ast::Cond::Or(cs) => {
                let checked: Vec<_> = cs
                    .iter()
                    .map(|c| self.requires_condition(c, owner))
                    .collect();
                let parts: Option<Vec<_>> = checked.into_iter().collect();
                parts.map(|p| {
                    if matches!(cond.value, ast::Cond::And(_)) {
                        ir::Cond::And(p)
                    } else {
                        ir::Cond::Or(p)
                    }
                })
            }
        }
    }

    // Conservative possible truth values, used only to avoid inventing metric
    // ambiguity on syntactically present but impossible singleton-result paths.
    fn truth(&self, cond: &ir::Cond) -> (bool, bool) {
        match cond {
            ir::Cond::IntCompare {
                variable,
                operator,
                value,
            } if self
                .loop_context
                .as_ref()
                .is_some_and(|c| c.round_variable == *variable) =>
            {
                let k = i128::from(self.loop_context.as_ref().unwrap().round);
                let yes = match operator {
                    ir::IntComparison::Eq => k == *value,
                    ir::IntComparison::Ne => k != *value,
                    ir::IntComparison::Lt => k < *value,
                    ir::IntComparison::Le => k <= *value,
                    ir::IntComparison::Gt => k > *value,
                    ir::IntComparison::Ge => k >= *value,
                };
                (yes, !yes)
            }
            ir::Cond::Contains { .. } | ir::Cond::StateEq(_) | ir::Cond::IntCompare { .. } => {
                (true, true)
            }
            ir::Cond::Eq(m) => {
                let Some(call) = self.calls.values().find(|c| c.binding == m.binding) else {
                    return if self
                        .bindings
                        .get(m.binding.0)
                        .is_some_and(|b| b.merge.is_some())
                    {
                        self.merge_truth(m)
                    } else {
                        (true, true)
                    };
                };
                let cap = &self.capabilities[call.capability.0];
                let results = cap.possible_results();
                (
                    results.contains(&m.value),
                    results.iter().any(|v| *v != m.value),
                )
            }
            ir::Cond::And(parts) => {
                let facts: Vec<_> = parts.iter().map(|c| self.truth(c)).collect();
                (facts.iter().all(|v| v.0), facts.iter().any(|v| v.1))
            }
            ir::Cond::Or(parts) => {
                let facts: Vec<_> = parts.iter().map(|c| self.truth(c)).collect();
                (facts.iter().any(|v| v.0), facts.iter().all(|v| v.1))
            }
        }
    }

    fn join(
        &mut self,
        name: &ast::Ident,
        calls: &[ir::Call],
        join: &ast::Spanned<ast::Join>,
        flow: &mut Flow,
    ) -> Option<ir::Join> {
        if let ast::Join::Any { result, .. } = &join.value
            && self.merge_names.contains(&result.value)
        {
            self.unsupported(
                "merge_feature_combination: merge_read_position",
                result.span,
            );
            return None;
        }
        let target = match &join.value {
            ast::Join::All { fork } | ast::Join::Any { fork, .. } => fork,
        };
        if target.value != name.value {
            if !self.fork_names.contains_key(&target.value) {
                self.error(E_NAME_0, "undefined fork", target.span);
            } else {
                self.unsupported(
                    "join must target its immediately preceding fork",
                    target.span,
                );
            }
        }
        let ast::Join::Any {
            result,
            tie,
            remaining,
            if_none,
            ..
        } = &join.value
        else {
            flow.available.extend(calls.iter().map(|c| c.binding));
            return Some(ir::Join::All);
        };
        let matches: Vec<_> = calls
            .iter()
            .filter_map(|call| {
                let ty = self.capabilities[call.capability.0].return_type;
                if ty.0 < self.types.len() {
                    // Static checking uses all AST branch occurrences below,
                    // including those excluded from unique-name registration.
                    self.types[ty.0]
                        .members
                        .iter()
                        .position(|v| v == &result.value)
                        .map(|value| ir::ResultMatch {
                            binding: call.binding,
                            value: ir::ValueId(value),
                        })
                } else {
                    None
                }
            })
            .collect();
        let resolved_tie = match &tie.value {
            ast::Tie::Nondet => ir::Tie::Nondet,
            ast::Tie::By(names) => {
                let mut ids = Vec::new();
                for n in names {
                    let branch = calls
                        .iter()
                        .find(|c| self.bindings[c.binding.0].name == n.value);
                    if let Some(branch) = branch {
                        ids.push(branch.binding);
                    }
                }
                ir::Tie::By(ids)
            }
        };
        let cancel = remaining.value == ast::Remaining::CancelThenDrain;
        let none = self.end(if_none);
        let no_match_possible = matches.len() == calls.len()
            && calls.iter().zip(&matches).all(|(c, m)| {
                self.capabilities[c.capability.0]
                    .finish_results()
                    .iter()
                    .any(|v| *v != m.value)
            });
        if no_match_possible && let Some(end) = none {
            self.terminal(end, &flow.histories);
        }
        if let Some(end) = none {
            let mut available = flow.available.clone();
            available.extend(calls.iter().map(|c| c.binding));
            self.record_end_before(end, &available);
        }
        let adoption_possible = calls.iter().zip(&matches).any(|(c, m)| {
            self.capabilities[c.capability.0]
                .finish_results()
                .contains(&m.value)
        });
        if self.check_flow && adoption_possible && !flow.histories.is_empty() {
            if flow.histories.contains(&true) {
                self.unsupported(
                    "multiple any adoptions on one path have ambiguous result_at",
                    join.span,
                );
            }
            flow.histories = BTreeSet::from([true]);
        } else {
            flow.histories.clear();
        }
        if cancel {
            self.available_after_cancel(calls, &matches, flow);
        } else {
            flow.available.extend(calls.iter().map(|c| c.binding));
        }
        none.map(|if_none| ir::Join::Any {
            matches,
            tie: resolved_tie,
            remaining: if cancel {
                ir::Remaining::CancelThenDrain
            } else {
                ir::Remaining::Drain
            },
            if_none,
        })
    }

    fn call_candidates(&self, call: &ast::Call) -> Vec<ir::CapabilityId> {
        self.capabilities
            .iter()
            .enumerate()
            .filter(|(_, cap)| {
                cap.name == call.capability.value
                    && self.actors[cap.actor.0].name == call.actor.value
            })
            .map(|(i, _)| ir::CapabilityId(i))
            .collect()
    }

    fn check_candidate_value(&mut self, types: &BTreeSet<ir::TypeId>, value: &ast::Ident) {
        let names: BTreeSet<_> = types
            .iter()
            .map(|t| self.types[t.0].name.as_str())
            .collect();
        if !names.is_empty()
            && !self
                .types
                .iter()
                .any(|ty| names.contains(ty.name.as_str()) && ty.members.contains(&value.value))
        {
            self.error(
                E_TYPE_0,
                "value is not a member of any candidate return type",
                value.span,
            );
        }
    }

    fn check_join_branches(&mut self, branches: &[ast::Call], join: &ast::Spanned<ast::Join>) {
        if matches!(&join.value, ast::Join::Any { result, .. } if self.merge_names.contains(&result.value))
        {
            return;
        }
        let ast::Join::Any {
            result,
            tie,
            remaining,
            ..
        } = &join.value
        else {
            return;
        };
        if let ast::Tie::By(names) = &tie.value {
            let branches: BTreeSet<_> = branches.iter().map(|b| &b.binding.value).collect();
            let named: BTreeSet<_> = names.iter().map(|n| &n.value).collect();
            for name in names {
                if !branches.contains(&name.value) {
                    self.error(E_NAME_0, "undefined branch in this fork", name.span);
                }
            }
            if names.len() != branches.len() || named != branches {
                self.unsupported(
                    "tie by must be a full, duplicate-free branch permutation",
                    tie.span,
                );
            }
        }
        for branch in branches {
            let candidates = self.call_candidates(branch);
            if !candidates.is_empty() && candidates.iter().all(|c| self.known_cap_types.contains(c))
            {
                let types = candidates
                    .iter()
                    .map(|c| self.capabilities[c.0].return_type)
                    .collect();
                self.check_candidate_value(&types, result);
            }
            // The checkpoint's rule is static, including duplicate bindings.
            // An ambiguous Capability establishes this error only when every
            // candidate omits cancel; the ambiguity itself remains unsupported.
            if remaining.value == ast::Remaining::CancelThenDrain
                && !candidates.is_empty()
                && candidates
                    .iter()
                    .all(|c| self.capabilities[c.0].cancel == ir::Cancel::Unspecified)
            {
                self.error(
                    E_CANCEL_0,
                    "cancel_then_drain requires cancel on every branch Capability",
                    branch.span,
                );
            }
        }
    }

    fn available_after_cancel(
        &self,
        calls: &[ir::Call],
        matches: &[ir::ResultMatch],
        flow: &mut Flow,
    ) {
        if self.multi_case
            || calls.iter().any(|c| {
                let cap = &self.capabilities[c.capability.0];
                cap.requires.is_some() || cap.cancel == ir::Cancel::Immediate
            })
        {
            return;
        }
        // No preceding task can still be live: every earlier call/join drains.
        // If this entire fork fits, work-conserving dispatch starts every branch
        // before time advances. Otherwise we make no start-order assumption.
        let all_start = calls.iter().all(|c| {
            let actor = self.capabilities[c.capability.0].actor;
            duration_min(&self.capabilities[c.capability.0]) > 0
                && self.actors[actor.0].capacity.is_some_and(|capacity| {
                    capacity
                        >= calls
                            .iter()
                            .filter(|c| self.capabilities[c.capability.0].actor == actor)
                            .count() as u64
                })
        });
        if !all_start {
            return;
        }
        let mut demand = vec![0u64; self.resources.len()];
        for call in calls {
            for u in &self.capabilities[call.capability.0].uses {
                let Some(sum) = demand[u.resource.0].checked_add(u.amount) else {
                    return;
                };
                demand[u.resource.0] = sum;
                if sum > self.resources[u.resource.0].capacity {
                    return;
                }
            }
        }
        for call in calls {
            let cap = &self.capabilities[call.capability.0];
            // A branch cannot be cancelled before it naturally finishes if no
            // other potentially matching branch can be adopted earlier. Finish
            // wins over an ack at this same time (ack delay is strictly positive).
            let finishes_before_request = calls.iter().zip(matches).all(|(other, m)| {
                let candidate = &self.capabilities[other.capability.0];
                other.binding == call.binding
                    || !candidate.finish_results().contains(&m.value)
                    || duration_min(candidate) >= duration_max(cap)
            });
            if cap.cancel == ir::Cancel::ForbiddenAfterStart || finishes_before_request {
                flow.available.insert(call.binding);
            }
        }
    }

    fn bounds(&mut self, model: &ast::Model) -> Option<ir::Bounds> {
        let (mut cases, mut live_tasks, mut clock) = (None, None, None);
        let mut blocks = 0;
        for item in &model.items {
            if let ast::Item::Bounds(bounds) = item {
                blocks += 1;
                for item in &bounds.items {
                    match &item.value {
                        ast::Bound::Cases(v) => {
                            self.set_once(&mut cases, v.value, "cases", model.span);
                            if v.value == 0 {
                                self.unsupported("cases must be positive", v.span);
                            }
                            if v.value >= 2
                                && !model.items.iter().any(|i| {
                                    matches!(
                                        i,
                                        ast::Item::ArrivalsFixed(_)
                                            | ast::Item::ArrivalsEvery(_)
                                            | ast::Item::ArrivalsDistribution(_)
                                    )
                                })
                            {
                                self.error(
                                    diag::E_ARR_0,
                                    "multiple cases require fixed arrivals",
                                    v.span,
                                );
                            }
                        }
                        ast::Bound::LiveTasks(v) => {
                            self.set_once(&mut live_tasks, v.value, "live_tasks", model.span)
                        }
                        ast::Bound::Clock(d) => {
                            let ticks = self.ticks(d);
                            self.set_once(&mut clock, ticks, "clock", model.span);
                        }
                    }
                }
            }
        }
        if blocks > 1 {
            self.unsupported("multiple bounds blocks", model.span);
        }
        if cases.is_none() || live_tasks.is_none() || clock.is_none() {
            self.unsupported(
                "cases, live_tasks and clock must all be explicit",
                model.span,
            );
        }
        Some(ir::Bounds {
            cases: cases?,
            live_tasks: live_tasks?,
            clock: clock.flatten()?,
        })
    }

    fn actor_instances(&mut self, model: &ast::Model) {
        for item in &model.items {
            let ast::Item::Actor(actor) = item else {
                continue;
            };
            let id = self.actor_names[&actor.name.value];
            let declarations: Vec<_> = actor
                .items
                .iter()
                .filter_map(|f| {
                    if let ast::ActorItem::Instance(v) = f {
                        Some(v)
                    } else {
                        None
                    }
                })
                .collect();
            if declarations.is_empty() {
                continue;
            }
            self.actors[id.0].metadata.instance = Some(ir::ActorInstance::PerCase);
            for duplicate in declarations.iter().skip(1) {
                self.unsupported("duplicate Actor instance", duplicate.span);
            }
            for field in &actor.items {
                let span = match field {
                    ast::ActorItem::Capacity(v) => v.span,
                    ast::ActorItem::Executors(v) => v.span,
                    ast::ActorItem::Capability(c) => c.span,
                    _ => continue,
                };
                self.unsupported(
                    "per_case Actor cannot declare capacity, executors or Capability",
                    span,
                );
            }
        }
    }

    fn shared_latent_restrictions(&mut self, model: &ast::Model) {
        if self.multi_case {
            let mut bad = Vec::new();
            for (i, v) in self.state_variables.iter().enumerate() {
                let ir::StateKind::ActorObserved(a) = v.kind else {
                    continue;
                };
                if self.actors[a.0].is_per_case() {
                    continue;
                }
                let writers: Vec<_> = self
                    .capabilities
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| {
                        c.start_writes
                            .iter()
                            .chain(c.finish_writes())
                            .any(|w| w.variable.0 == i)
                    })
                    .collect();
                for (j, (left, x)) in writers.iter().enumerate() {
                    for (right, y) in &writers[j..] {
                        if self
                            .resolve_pair(ir::StateVarId(i), *left, *right)
                            .is_none()
                        {
                            bad.push((
                                format!(
                                    "unresolved concurrent writes to {}.{} by {}.{} and {}.{} (G5)",
                                    self.actors[a.0].name,
                                    v.name,
                                    self.actors[x.actor.0].name,
                                    x.name,
                                    self.actors[y.actor.0].name,
                                    y.name
                                ),
                                y.span,
                            ));
                        }
                    }
                }
            }
            for (message, span) in bad {
                self.unsupported(message, span);
            }
        }
        let shared =
            |kind| matches!(kind, ir::StateKind::ActorLatent(a) if !self.actors[a.0].is_per_case());
        if self.multi_case
            && (self
                .state_variables
                .iter()
                .any(|v| shared(v.kind) && !matches!(v.initial, ir::InitialValue::Fixed(_)))
                || self.capabilities.iter().any(|c| {
                    c.writes
                        .iter()
                        .map(|w| w.variable)
                        .chain(c.kernel_writes.iter().copied())
                        .any(|id| shared(self.state_variables[id.0].kind))
                }))
        {
            self.unsupported(
                "multi-case Actor latent state must be fixed and read-only",
                model.span,
            );
        }
    }

    fn resolve_pair(
        &self,
        variable: ir::StateVarId,
        left: usize,
        right: usize,
    ) -> Option<ir::ConcurrencyResolution> {
        use ir::{
            AssignmentOp as O, CommutativeKind as K, ConcurrencyResolution as R, WriteSource as W,
        };
        let (a, b) = (&self.capabilities[left], &self.capabilities[right]);
        let v = &self.state_variables[variable.0];
        if self.actors[a.actor.0].capacity == Some(1) {
            return Some(R::Serialized);
        }
        for x in &a.uses {
            if let Some(y) = b.uses.iter().find(|y| y.resource == x.resource)
                && u128::from(x.amount) + u128::from(y.amount)
                    > u128::from(self.resources[x.resource.0].capacity)
            {
                return Some(R::Lock(x.resource));
            }
        }
        let kind = |c: &ir::Capability| {
            let mut result = None;
            for w in c
                .start_writes
                .iter()
                .chain(c.finish_writes())
                .filter(|w| w.variable == variable)
            {
                let k = match w.source {
                    W::InsertCase { .. } => K::InsertCase,
                    W::Integer {
                        operator: O::Add, ..
                    } => K::Add,
                    W::Integer {
                        operator: O::Subtract,
                        ..
                    } => K::Subtract,
                    _ => return None,
                };
                if result.is_some_and(|r| r != k) {
                    return None;
                }
                result = Some(k);
            }
            result
        };
        if let Some(k) = kind(a)
            && kind(b) == Some(k)
        {
            return Some(R::Commutative(k));
        }
        if [a, b].iter().all(|c| {
            c.version_check
                .as_ref()
                .is_some_and(|v| v.variables.contains(&variable))
        }) {
            return Some(R::Versioned);
        }
        v.last_write_wins.then_some(R::LastWriteWins)
    }

    fn resolved_concurrency(&self) -> Vec<ir::ResolvedPair> {
        let mut result = Vec::new();
        if !self.multi_case {
            return result;
        }
        for (i, v) in self.state_variables.iter().enumerate() {
            if !matches!(v.kind, ir::StateKind::ActorObserved(a) if !self.actors[a.0].is_per_case())
            {
                continue;
            }
            let variable = ir::StateVarId(i);
            let writers: Vec<_> = self
                .capabilities
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    c.start_writes
                        .iter()
                        .chain(c.finish_writes())
                        .any(|w| w.variable == variable)
                })
                .map(|(i, _)| i)
                .collect();
            for (j, &left) in writers.iter().enumerate() {
                for &right in &writers[j..] {
                    if let Some(resolution) = self.resolve_pair(variable, left, right) {
                        result.push(ir::ResolvedPair {
                            variable,
                            left: ir::CapabilityId(left),
                            right: ir::CapabilityId(right),
                            resolution,
                        });
                    }
                }
            }
        }
        result
    }

    fn concurrency_assumptions(&self, assumptions: &mut Vec<ir::Assumption>) {
        let pairs = self.resolved_concurrency();
        for (i, v) in self.state_variables.iter().enumerate() {
            let ir::StateKind::ActorObserved(a) = v.kind else {
                continue;
            };
            let explicit_version = self.capabilities.iter().any(|c| {
                c.version_check
                    .as_ref()
                    .is_some_and(|x| x.variables.contains(&ir::StateVarId(i)))
            });
            let rows: Vec<_> = pairs
                .iter()
                .filter(|p| {
                    p.variable.0 == i && p.resolution != ir::ConcurrencyResolution::Serialized
                })
                .collect();
            if rows.is_empty() && !explicit_version && !v.last_write_wins {
                continue;
            }
            let display = |r: &ir::ConcurrencyResolution| match r {
                ir::ConcurrencyResolution::Serialized => "serialized".into(),
                ir::ConcurrencyResolution::Lock(r) => format!("lock {}", self.resources[r.0].name),
                ir::ConcurrencyResolution::Commutative(_) => "commutative".into(),
                ir::ConcurrencyResolution::Versioned => "version_check".into(),
                ir::ConcurrencyResolution::LastWriteWins => "last_write_wins".into(),
            };
            let values: Vec<_> = rows.iter().map(|r| display(&r.resolution)).collect();
            let value = if values.is_empty() {
                if explicit_version {
                    "version_check"
                } else {
                    "last_write_wins"
                }
                .into()
            } else if values.iter().all(|v| *v == values[0]) {
                values[0].clone()
            } else {
                rows.iter()
                    .map(|r| {
                        format!(
                            "{}.{}/{}.{}: {}",
                            self.actors[self.capabilities[r.left.0].actor.0].name,
                            self.capabilities[r.left.0].name,
                            self.actors[self.capabilities[r.right.0].actor.0].name,
                            self.capabilities[r.right.0].name,
                            display(&r.resolution)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            let mut affects = "書込みの対は指定の方式で解決。書込み中の読取りは全順序を展開して古い値を読む経路を残す。読みの競合候補は列挙しない。可換の判定は値の更新だけで、相手の適格性・権限・結果への影響は判定しない。".to_owned();
            if rows
                .iter()
                .any(|r| matches!(r.resolution, ir::ConcurrencyResolution::Lock(_)))
            {
                affects.push_str("ロックの資源はActorとして切っていない。");
            }
            if explicit_version {
                affects.push_str("明示のversion_checkを適用する。");
            }
            if v.last_write_wins {
                affects.push_str("明示のlast_write_winsを適用する。");
            }
            if !self.multi_case || self.actors[a.0].capacity == Some(1) {
                affects.push_str("案件が1つ、または所有Actorの容量が1なので、案件をまたぐ競合には作用しない。単一案件内の通常Taskと異議routeの履歴追跡は行う。");
            }
            assumptions.push(ir::Assumption {
                field: format!("{}.{}.concurrency", self.actors[a.0].name, v.name),
                value,
                affects,
            });
        }
    }

    fn instance_assumptions(&self, assumptions: &mut Vec<ir::Assumption>) {
        for (i, actor) in self.actors.iter().enumerate() {
            if actor.is_per_case() {
                assumptions.push(ir::Assumption {
                    field: format!("{}.instance", actor.name),
                    value: "per_case".into(),
                    affects: "局所値は受付ごとの案件に属し、容量・実行者・Capabilityを持たない"
                        .into(),
                });
            } else if self.multi_case
                && self
                    .state_variables
                    .iter()
                    .any(|v| v.kind == ir::StateKind::ActorObserved(ir::ActorId(i)))
            {
                assumptions.push(ir::Assumption {
                    field: format!("{}.instance", actor.name), value: "shared(default)".into(),
                    affects: "局所の観測値は全案件で1組を共有し、書込みは所有Actorのstart/finishの遷移で原子的に起きる。案件の間の順序は全部展開する".into(),
                });
            }
        }
        if self.state_variables.iter().any(|v| {
            matches!(v.kind, ir::StateKind::ActorLatent(a) if self.actors[a.0].is_per_case())
                && matches!(v.initial, ir::InitialValue::Distribution(_))
        }) {
            assumptions.push(ir::Assumption {
                field: "initial.actor_case_independence".into(),
                value: "independent-per-admission".into(),
                affects: "案件ごとの潜在は案件の間で独立と仮定。同じ時刻の案件の初期分布と一層の積で引く。この独立は分析者の仮定であり、現実の根拠ではない".into(),
            });
        }
    }

    fn assumptions(&self) -> Vec<ir::Assumption> {
        let mut assumptions = Vec::new();
        for (i, actor) in self.actors.iter().enumerate() {
            let mut add = |field: &str, value: String, affects: &str| {
                assumptions.push(ir::Assumption {
                    field: format!("{}.{}", actor.name, field),
                    value,
                    affects: affects.into(),
                })
            };
            if actor.metadata.boundary.is_none() {
                add(
                    "boundary",
                    actor.name.clone(),
                    "Actor boundary is represented by its name only",
                );
            }
            if let Some(capacity) = actor.capacity.filter(|_| actor.executors.is_none()) {
                add(
                    "executors",
                    format!("anonymous×{capacity}"),
                    "executor identity, separation of duties and permission checks are unsupported",
                );
            }
            if actor.metadata.control.is_none() {
                add(
                    "control",
                    if self.called_actors.contains(&ir::ActorId(i)) {
                        "policy"
                    } else {
                        "environment"
                    }
                    .into(),
                    "classification of policy and environment choices",
                );
            }
            if actor.executors.is_none()
                || self
                    .capabilities
                    .iter()
                    .any(|c| c.actor == ir::ActorId(i) && c.permission.is_none())
            {
                add(
                    "permission",
                    "default(executors)".into(),
                    "Actor executors may execute their own Capabilities; authority is not inferred",
                );
            }
            if !actor.metadata.capacity_declared
                && let Some(executors) = &actor.executors
            {
                add(
                    "capacity",
                    format!("count(executors)={}", executors.len()),
                    "Actor capacity is the number of declared executors",
                );
            }
        }
        for (i, r) in self.resources.iter().enumerate() {
            let mut affects = "全資源が共通の分析窓で利用可能。単位は匿名。Actorとして切っていないため作用・状態・拒否は検査せず、Silent Cutの検出対象にも含めない。".to_owned();
            if !self
                .capabilities
                .iter()
                .any(|c| c.uses.iter().any(|u| u.resource == ir::ResourceId(i)))
            {
                affects.push_str(" この資源を使うCapabilityの宣言はない。");
            }
            assumptions.push(ir::Assumption {
                field: format!("resource.{}", r.name),
                value: format!("capacity {}", r.capacity),
                affects,
            });
        }
        assumptions.push(ir::Assumption {
            field: "schedule".into(), value: "work-conserving".into(),
            affects: "enabled tasks start without intentional idle; competing starts remain nondeterministic".into(),
        });
        for v in &self.state_variables {
            if matches!(
                v.initial,
                ir::InitialValue::Nondet | ir::InitialValue::Distribution(_)
            ) {
                let prefix = match v.kind {
                    ir::StateKind::ActorLatent(a) => self.actors[a.0].name.as_str(),
                    _ => "latent",
                };
                if let ir::InitialValue::Distribution(id) = v.initial {
                    let values = self.initial_distributions[id]
                        .iter()
                        .map(|(x, p)| {
                            let value = match x {
                                ir::StateValue::Value(x) => {
                                    self.types[v.base.0].members[x.0].as_str()
                                }
                                _ => "none",
                            };
                            format!("{value}={p}")
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    assumptions.push(ir::Assumption { field:format!("{prefix}.{}",v.name), value:format!("dist({values})"), affects:"initial value is drawn from the declared distribution, independently of other initial distributions; the distribution is an analyst assumption".into() });
                    continue;
                }
                assumptions.push(ir::Assumption {
                    field: format!("{prefix}.{}", v.name), value: "nondet(全値)".into(),
                    affects: "all values are explored nondeterministically; no distribution or uniform probability is assumed".into(),
                });
            }
        }
        for a in &self.authorities {
            if let Some(revoker) = a.restrictions.revocable_by {
                let value = match revoker {
                    ir::AuthorityRevoker::Person(p) => format!("person:{}", self.people[p.0].name),
                    ir::AuthorityRevoker::Actor(id) => format!("actor:{}", self.actors[id.0].name),
                };
                assumptions.push(ir::Assumption {
                    field: format!("authority.{}.revocable_by", a.name), value,
                    affects: "動的な撤回は検査していない。撤回者の記録だけであり、撤回の発生・効力・当事者の受容を検査しない。".into(),
                });
            }
        }
        for k in &self.kernels {
            let names = k
                .given
                .iter()
                .map(|id| {
                    let v = &self.state_variables[id.0];
                    match v.kind {
                        ir::StateKind::ActorLatent(a) => {
                            format!("{}.{}", self.actors[a.0].name, v.name)
                        }
                        _ => v.name.clone(),
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            assumptions.push(ir::Assumption { field:format!("kernel.{}",k.name),value:format!("given[{names}]"),affects:"outcomes and writes are drawn from the declared rows; separate draws are conditionally independent given the listed variables; the rows are an analyst assumption".into() });
        }
        for cap in &self.capabilities {
            if let Some(d) = &cap.duration_distribution {
                let values = d
                    .entries
                    .iter()
                    .map(|(t, p)| format!("{t}min={p}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                assumptions.push(ir::Assumption {
                    field: format!("duration.{}.{}", self.actors[cap.actor.0].name, cap.name),
                    value: format!("dist({values})"),
                    affects: "duration is drawn from the declared distribution, independently of outcomes and of other durations; the distribution is an analyst assumption".into(),
                });
            }
        }
        self.instance_assumptions(&mut assumptions);
        self.concurrency_assumptions(&mut assumptions);
        assumptions
    }
}

#[cfg(test)]
mod actor_declaration_diagnostic_tests {
    use crate::{Verdict, diag, report};

    #[test]
    fn metadata_error_retains_configuration_and_cut_warnings_with_contains_witness() {
        use crate::{lower, syntax};
        // Tail is downstream of the cycle but is not itself on a cycle.
        let src = "model M version \"0.2\" { type T = enum { ok } actor Tail {} actor A { contains [C,Tail]; capacity 1; capability run() -> T { duration fixed 1min; result nondet {ok} } } actor C { capacity 1; capability w() -> T { duration fixed 1min; result nondet {ok} } } actor X {} process Flow { a=A.run(); end done } process Unused { end done } cut K { include [Tail,A,C]; exclude X reason \"r\" } configuration Plan { cut K; process Flow } bounds { cases 1; live_tasks 1; clock 3min } verify reachable end.done }";
        for (source, code, clause) in [
            (src.to_owned(), diag::E_CAP_0, "contains [C,Tail]"),
            (
                src.replace("actor C {", "actor C { contains [A];"),
                diag::E_CON_0,
                "contains [A]",
            ),
        ] {
            let ast = syntax::parse(&source).unwrap();
            let ds = lower::lower_configuration(&ast, "Plan").err().unwrap();
            let e = ds
                .iter()
                .find(|d| d.severity == diag::Severity::Error)
                .unwrap();
            assert_eq!(e.code, code);
            let span = e.span.unwrap();
            assert_eq!(&source[span.start..span.end], clause);
            let warnings: Vec<_> = ds
                .iter()
                .filter(|d| d.severity == diag::Severity::Warning)
                .map(|d| d.code.as_str())
                .collect();
            assert_eq!(warnings, [diag::W_CONF_0, diag::W_CUT_1]);
        }
        // Exercise the pre-assembly error exit and the existing post-assembly
        // ADR 0067: an Agency limit now continues lowering, while the same
        // independently generated configuration/Cut warnings must survive.
        let duplicate = src.replace(
            "capacity 1; capability run",
            "capacity 1; capacity 1; capability run",
        );
        let agency = src
            .replace("contains [C,Tail];", "")
            .replace(
                "duration fixed 1min; result nondet {ok}",
                "commits; duration fixed 1min; result nondet {ok}",
            )
            .replace(
                "a=A.run(); end done",
                &format!(
                    "a=A.run(); {} z=A.run(); end done",
                    "if a == ok {} else {} ".repeat(13)
                ),
            );
        for (source, agency_limit) in [(duplicate, false), (agency, true)] {
            let ast = syntax::parse(&source).unwrap();
            let result = lower::lower_configuration(&ast, "Plan");
            let ds = if agency_limit {
                let m = result.unwrap().model;
                assert!(m.agency.limit().is_some());
                assert!(m.diagnostics.iter().any(|d| d.code == diag::W_AGY_4));
                assert!(
                    m.diagnostics
                        .iter()
                        .all(|d| d.severity != diag::Severity::Error)
                );
                m.diagnostics
            } else {
                let ds = result.err().unwrap();
                assert!(ds.iter().any(|d| d.code == diag::U_SEM_0));
                ds
            };
            for code in [diag::W_CONF_0, diag::W_CUT_1] {
                assert_eq!(ds.iter().filter(|d| d.code == code).count(), 1, "{ds:?}");
            }
        }
    }

    #[test]
    fn unsupported_time_diagnostics_name_min_h_d_without_reclassifying() {
        use crate::{ast, lower, syntax};
        let src = "model M version \"0.2\" { type T = enum { ok } actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet {ok} } } process Flow { a=A.run(); end done } bounds { cases 1; live_tasks 1; clock 3min } verify reachable end.done }";
        let duration_message = "only integer durations in min, h or d are supported";
        let window_message = "metrics window unit must be min, h or d";
        for (source, code, message) in [
            (
                src.replace("fixed 1min", "fixed 1hour"),
                diag::U_SEM_0,
                duration_message,
            ),
            (
                src.replace("duration fixed 1min", "duration {1hour:1}"),
                diag::U_SYN_0,
                "unsupported syntax: only integer durations in min, h or d are supported",
            ),
            (
                src.replace(
                    "process Flow",
                    "metrics {window 3 hour; currency \"JPY\"} process Flow",
                ),
                diag::U_SYN_0,
                "unsupported syntax: metrics window unit must be min, h or d",
            ),
        ] {
            let r = report::check(&source);
            assert_eq!(r.verdict, Verdict::Unsupported);
            assert!(
                r.diagnostics
                    .iter()
                    .any(|d| d.code == code && d.message == message),
                "{:?}",
                r.diagnostics
            );
        }
        let mut metrics = syntax::parse(&src.replace(
            "process Flow",
            "metrics {window 3min; currency \"JPY\"} process Flow",
        ))
        .unwrap();
        for item in &mut metrics.items {
            if let ast::Item::Metrics(m) = item {
                for item in &mut m.items {
                    if let ast::MetricsItem::Window(d) = &mut item.value {
                        d.unit.value = "hour".into();
                    }
                }
            }
        }
        let mut contest =
            syntax::parse(include_str!("../../../fixtures/resp-contest/model.relca")).unwrap();
        for item in &mut contest.items {
            if let ast::Item::Contract(c) = item {
                for clause in &mut c.clauses {
                    if let ast::ContractClause::Contest { respond_by, .. } = &mut clause.value {
                        respond_by.unit.value = "hour".into();
                    }
                }
            }
        }
        for (model, message) in [
            (metrics, window_message),
            (contest, "contest time unit must be min, h or d"),
        ] {
            let ds = lower::lower(&model).err().unwrap();
            assert!(
                ds.iter()
                    .any(|d| d.code == diag::U_SYN_0 && d.message == message),
                "{ds:?}"
            );
            assert!(
                ds.iter()
                    .any(|d| d.code == diag::U_SEM_0 && d.message == duration_message),
                "{ds:?}"
            );
        }
    }

    #[test]
    fn each_new_field_has_one_duplicate_validator_not_one_per_projection() {
        let base = |actor_fields: &str, declarations: &str| {
            format!(
                "model M version \"0.2\" {{ actor A {{ {actor_fields} }} actor B {{}} process P {{ end done }} {declarations} bounds {{ cases 1; live_tasks 1; clock 2min }} verify reachable end.done }}"
            )
        };
        let mut cases = vec![
            (
                "boundary",
                base("boundary \"one\"; boundary \"two\"", ""),
                "duplicate boundary",
            ),
            (
                "grain",
                base("grain model; grain arbitrary", ""),
                "duplicate grain",
            ),
            (
                "contains clauses",
                base("contains [B]; contains [B]", ""),
                "duplicate contains",
            ),
            (
                "contains members",
                base("contains [B,B]", ""),
                "duplicate contains member",
            ),
            (
                "via model",
                base("via model \"doc@v1\"; via model \"doc@v2\"", ""),
                "duplicate via model",
            ),
            (
                "debtor",
                base(
                    "",
                    "contract C { response case.received -> one_of(end.done) by 1min after case.received otherwise unresolved debtor A debtor B }",
                ),
                "repeated debtor",
            ),
            (
                "horizon",
                base(
                    "",
                    "cut C { include [A,B]; horizon job within 1h; horizon job within 60min }",
                ),
                "duplicate horizon",
            ),
        ];
        for fields in [
            "control party; control party",
            "control policy; control policy",
            "control environment; control environment",
            "control party; control policy",
            "control environment; control party",
            "control policy; control environment",
        ] {
            cases.push(("control", base(fields, ""), "duplicate control"));
        }
        for (field, source, message) in cases {
            let r = report::check(&source);
            let errors: Vec<_> = r
                .diagnostics
                .iter()
                .filter(|d| d.severity == diag::Severity::Error)
                .collect();
            assert_eq!(r.verdict, Verdict::Unsupported, "{field}: {errors:?}");
            assert_eq!(errors.len(), 1, "{field}: {errors:?}");
            assert_eq!(errors[0].code, diag::U_SEM_0, "{field}");
            assert_eq!(errors[0].message, message, "{field}");
        }
        // overlaps is deliberately repeatable: no duplicate diagnostic is valid.
        let repeated = report::check(&base("overlaps [B,B]; overlaps [B]", ""));
        assert_eq!(repeated.verdict, Verdict::Satisfied);
        assert!(
            repeated
                .diagnostics
                .iter()
                .all(|d| d.severity != diag::Severity::Error)
        );
        let rows = repeated.actors.unwrap();
        assert_eq!(rows[0].overlaps.value, serde_json::json!(["B"]));
    }

    #[test]
    fn repeated_party_control_with_standing_has_exactly_one_diagnostic_at_second_clause() {
        let source = include_str!("../../../fixtures/resp-contest/model.relca").replacen(
            "control party",
            "control party\n    control party",
            1,
        );
        let report = report::check(&source);
        let errors: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.severity == diag::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].code, diag::U_SEM_0);
        assert_eq!(errors[0].message, "duplicate control");
        let span = errors[0].span.unwrap();
        assert_eq!(&source[span.start..span.end], "control party");
        assert_eq!(span.start, source.rfind("control party").unwrap());
    }
}

fn duration_min(cap: &ir::Capability) -> ir::Tick {
    cap.duration_distribution
        .as_ref()
        .and_then(|d| {
            d.entries
                .iter()
                .filter(|(_, p)| !p.is_zero())
                .map(|(t, _)| *t)
                .min()
        })
        .unwrap_or(cap.duration)
}

fn duration_max(cap: &ir::Capability) -> ir::Tick {
    cap.duration_distribution
        .as_ref()
        .and_then(|d| {
            d.entries
                .iter()
                .filter(|(_, p)| !p.is_zero())
                .map(|(t, _)| *t)
                .max()
        })
        .unwrap_or(cap.duration)
}

#[cfg(test)]
mod duration_tests {
    use super::*;
    #[test]
    fn d_empty_hand_ast_is_probability_error() {
        let mut m =
            crate::syntax::parse(include_str!("../../../fixtures/duration-sum/model.relca"))
                .unwrap();
        for item in &mut m.items {
            if let ast::Item::Actor(a) = item {
                for item in &mut a.items {
                    if let ast::ActorItem::Capability(c) = item {
                        for item in &mut c.items {
                            if let ast::CapItem::DurationDistribution(entries) = &mut item.value {
                                entries.clear();
                            }
                        }
                    }
                }
            }
        }
        let errors = lower(&m).unwrap_err();
        assert!(errors.iter().any(|d| d.code == diag::E_PROB_0));
        assert!(
            !errors.iter().any(|d| d.code == diag::U_SEM_0),
            "empty AST must not become fixed zero"
        );
    }
}

#[cfg(test)]
mod metrics_tests {
    use super::*;

    fn source() -> &'static str {
        include_str!("../../../fixtures/met-serial/model.relca")
    }

    #[test]
    fn metrics_lower_keeps_rates_and_lexical_required_sets() {
        let model = lower(&crate::syntax::parse(source()).unwrap()).unwrap();
        let spec = model.metrics.as_ref().unwrap();
        assert_eq!(spec.window, 10);
        assert_eq!(spec.currency, "JPY");
        assert_eq!(spec.actor_rates, [Some(100)]);
        assert_eq!(spec.start_rates, [Some(2), Some(3)]);
        assert_eq!(spec.required_actors, [ir::ActorId(0)]);
        assert_eq!(
            spec.required_capabilities,
            [ir::CapabilityId(0), ir::CapabilityId(1)]
        );
        assert!(model.legacy_route());
        assert_eq!(
            model
                .assumptions
                .iter()
                .filter(|a| a.field.starts_with("metrics."))
                .count(),
            5
        );
    }

    #[test]
    fn metrics_absent_has_no_configuration_or_assumptions() {
        let source = include_str!("../../../fixtures/met-legacy/model.relca");
        let model = lower(&crate::syntax::parse(source).unwrap()).unwrap();
        assert!(model.metrics.is_none());
        assert!(
            !model
                .assumptions
                .iter()
                .any(|a| a.field.starts_with("metrics."))
        );
        assert!(model.legacy_route());
    }

    #[test]
    fn metrics_all_fixture_syntax_and_selected_lowering() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut dirs = 0;
        let mut selected = 0;
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if !path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("met-")
            {
                continue;
            }
            dirs += 1;
            let source = std::fs::read_to_string(path.join("model.relca")).unwrap();
            let ast = crate::syntax::parse(&source).unwrap();
            let names: Vec<_> = ast
                .items
                .iter()
                .filter_map(|i| match i {
                    ast::Item::Configuration(c) => Some(c.name.value.as_str()),
                    _ => None,
                })
                .collect();
            if names.is_empty() {
                lower(&ast).unwrap();
                selected += 1;
            } else {
                for name in names {
                    lower_configuration(&ast, name).unwrap();
                    selected += 1;
                }
            }
        }
        assert_eq!((dirs, selected), (24, 28));
    }
}
