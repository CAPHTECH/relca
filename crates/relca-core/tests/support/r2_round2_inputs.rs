//! Round-two test construction, not production validation or transition logic.
use crate::r2_inputs as i;
use crate::{ir, lower, syntax};

pub(crate) fn source(position: &str, alias: bool) -> String {
    let mut s = i::source("R01");
    s = i::edit(
        &s,
        "actor Shop {",
        "actor Source {capacity 1; capability observe()->PaymentStatus {duration fixed 1min; result nondet {still_unknown}}}\nactor Shop {",
    );
    s = i::edit(
        &s,
        "include [Psp,CS,Finance,Shop]",
        "include [Psp,CS,Finance,Shop,Source]",
    );
    s = i::edit(
        &s,
        "decision = CS.decide(status)",
        "decision = CS.decide(order)",
    );
    if alias {
        s = i::edit(
            &s,
            "approval: optional<Verdict>",
            "approval: optional<Verdict>; order: optional<PaymentStatus>",
        );
        s = i::edit(
            &s,
            "capability observe()->PaymentStatus {",
            "capability observe()->PaymentStatus {writes order;",
        );
        s = i::edit(&s, "cases 1; live_tasks 2", "cases 2; live_tasks 2");
        s = i::edit(&s, "arrivals fixed [0min]", "arrivals fixed [0min,0min]");
    }
    match position {
        "before" => i::edit(
            &s,
            "process Flow { pay",
            "process Flow { order = Source.observe()\npay",
        ),
        "after" => i::edit(
            &s,
            "normal = Shop.ship_known(pay)",
            "order = Source.observe()\nnormal = Shop.ship_known(pay)",
        ),
        "sibling" => {
            let open = s.find("process Flow {").unwrap();
            let cut = s[open..].find("cut C {").unwrap() + open;
            let end = s[..cut].rfind('}').unwrap();
            let body = &s[open + "process Flow {".len()..end];
            format!(
                "{}process Flow {{ before = Source.observe()\nif before == still_unknown {{order = Source.observe(); end shipped}} else {{{body}}} }}{}",
                &s[..open],
                &s[end + 1..]
            )
        }
        _ => panic!("position"),
    }
}

/// Start with valid lowered IR, then move the producer without re-lowering.
pub(crate) fn bad_prefix(position: &str) -> ir::Model {
    let mut m = lower::lower(&syntax::parse(&source("before", false)).unwrap()).unwrap();
    let producer = m.process.body.remove(0);
    match position {
        "after" => m.process.body.push(producer),
        "sibling" => {
            let state = m
                .state_variables
                .iter()
                .position(|v| v.name == "refunded")
                .unwrap();
            let ir::InitialValue::Fixed(value) = m.state_variables[state].initial else {
                panic!("fixed")
            };
            let span = producer.span;
            let end = ir::Statement {
                span,
                kind: ir::Stmt::End(ir::EndId(
                    m.ends.iter().position(|e| e == "shipped").unwrap(),
                )),
            };
            let body = std::mem::take(&mut m.process.body);
            m.process.body.push(ir::Statement {
                span,
                kind: ir::Stmt::If {
                    condition: ir::Cond::StateEq(ir::StateMatch {
                        variable: ir::StateVarId(state),
                        value,
                    }),
                    then_body: vec![producer, end],
                    else_body: body,
                },
            });
        }
        _ => panic!("position"),
    }
    m
}

/// G's ordinary finish writer, distinct from its atomic branch writes.
pub(crate) fn outside_atomic_reader() -> ir::Model {
    let mut m = lower::lower(&syntax::parse(&i::source("R01")).unwrap()).unwrap();
    let mut variable = m
        .state_variables
        .iter()
        .find(|v| v.name == "refunded")
        .unwrap()
        .clone();
    variable.name = "extra_fact".into();
    let ir::InitialValue::Fixed(value) = variable.initial else {
        panic!("fixed")
    };
    let id = ir::StateVarId(m.state_variables.len());
    m.state_variables.push(variable);
    let target = m.response_attempts()[0].1.command_call.capability;
    m.capabilities[target.0].writes.push(ir::StateWrite {
        variable: id,
        source: ir::WriteSource::Constant(value),
    });
    m.capabilities
        .iter_mut()
        .find(|c| c.name == "decide")
        .unwrap()
        .read_variables
        .push(id);
    m
}
