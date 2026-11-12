//! Normative lexical positions: ../relation_producer_reads.md.
//! Two-pass definition/use relation, independent of production Shape.
use relca_core::ir;
use std::collections::BTreeMap;

// Wildcards deliberately panic when a future IR variant reaches this oracle.
// Explicitly ignored variants below must stay explicit, never a no-op wildcard.
#[allow(unreachable_patterns)]
fn flatten<'a>(block: &'a ir::Block, out: &mut Vec<&'a ir::Stmt>) {
    for statement in block {
        out.push(&statement.kind);
        match &statement.kind {
            ir::Stmt::Wait(_) | ir::Stmt::Call(_) | ir::Stmt::Fork { .. } | ir::Stmt::End(_) => {}
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                flatten(then_body, out);
                flatten(else_body, out);
            }
            unknown => panic!("unclassified Stmt in read oracle: {unknown:?}"),
        }
    }
}

#[allow(unreachable_patterns)]
fn condition(c: &ir::Cond, binding: ir::BindingId) -> bool {
    match c {
        ir::Cond::Eq(result) => result.binding == binding,
        ir::Cond::And(children) | ir::Cond::Or(children) => {
            // Evaluate every child even when an earlier child already reads it:
            // a new unclassified condition must not hide behind short circuit.
            children
                .iter()
                .map(|c| condition(c, binding))
                .fold(false, |a, b| a | b)
        }
        ir::Cond::StateEq(_) => false,
        ir::Cond::IntCompare { .. } => false,
        ir::Cond::Contains { .. } => false,
        unknown => panic!("unclassified Cond in read oracle: {unknown:?}"),
    }
}

#[allow(unreachable_patterns)]
fn join_reads(join: &ir::Join, binding: ir::BindingId) -> bool {
    match join {
        ir::Join::All => false,
        ir::Join::Any {
            matches,
            tie: _,
            remaining: _,
            if_none: _,
        } => matches.iter().any(|m| m.binding == binding),
        unknown => panic!("unclassified Join in read oracle: {unknown:?}"),
    }
}

#[allow(unreachable_patterns)]
fn definitions(statement: &ir::Stmt) -> Vec<&ir::Call> {
    match statement {
        ir::Stmt::Call(call) => vec![call],
        ir::Stmt::Fork { branches, .. } => branches.iter().collect(),
        ir::Stmt::Wait(_) | ir::Stmt::If { .. } | ir::Stmt::End(_) => vec![],
        unknown => panic!("unclassified Stmt definition: {unknown:?}"),
    }
}

#[allow(unreachable_patterns)]
fn reads(statement: &ir::Stmt, binding: ir::BindingId) -> bool {
    let arguments = |c: &ir::Call| c.args.contains(&binding) && binding != ir::CURRENT_CASE;
    match statement {
        ir::Stmt::Call(call) => arguments(call),
        ir::Stmt::Fork { branches, join, .. } => {
            branches.iter().map(arguments).fold(false, |a, b| a | b) | join_reads(join, binding)
        }
        ir::Stmt::If { condition: c, .. } => condition(c, binding),
        ir::Stmt::Wait(_) => false,
        ir::Stmt::End(_) => false,
        unknown => panic!("unclassified Stmt read: {unknown:?}"),
    }
}

pub fn inventory(model: &ir::Model) -> BTreeMap<String, String> {
    let mut statements = vec![];
    flatten(&model.process.body, &mut statements);
    let mut result = BTreeMap::new();
    for call in statements.iter().flat_map(|s| definitions(s)) {
        let used = statements
            .iter()
            .map(|s| reads(s, call.binding))
            .fold(false, |a, b| a | b);
        if used {
            let capability = &model.capabilities[call.capability.0];
            assert!(
                result
                    .insert(
                        model.process.bindings[call.binding.0].name.clone(),
                        format!(
                            "{}.{}",
                            model.actors[capability.actor.0].name, capability.name
                        ),
                    )
                    .is_none(),
                "duplicate result definition"
            );
        }
    }
    // Even a process without Call must classify its conditions and joins.
    for statement in statements {
        let _ = reads(statement, ir::CURRENT_CASE);
    }
    result
}
