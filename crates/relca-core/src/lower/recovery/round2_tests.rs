#[cfg(test)]
mod round2_tests {
    use super::*;
    #[test]
    fn round2_prefix_rejects_sibling_and_future_dsl_arguments() {
        let results: Vec<_> = ["sibling", "after"]
            .into_iter()
            .map(|position| {
                let source = crate::r2_round2_inputs::source(position, false);
                let rejected = crate::lower::lower(&crate::syntax::parse(&source).unwrap())
                    .err()
                    .is_some_and(|ds| {
                        ds.iter().any(|d| {
                            d.code == U_SEM_0
                                && d.message == "argument result is not definitely available"
                        })
                    });
                (position, rejected)
            })
            .collect();
        assert!(results.iter().all(|(_, rejected)| *rejected), "{results:?}");
    }
    #[test]
    fn round2_alias_does_not_exempt_a_future_handler_argument() {
        let source = crate::r2_round2_inputs::source("after", true);
        let ds = crate::lower::lower(&crate::syntax::parse(&source).unwrap())
            .expect_err("handler argument needs its producer, not merely a state alias");
        assert!(
            ds.iter()
                .any(|d| d.code == U_SEM_0
                    && d.message == "argument result is not definitely available"),
            "{ds:?}"
        );
    }
    #[test]
    fn round2_ordinary_finish_writer_is_protected_in_lower() {
        let m = crate::r2_round2_inputs::outside_atomic_reader();
        let l = Lower {
            actors: m.actors.clone(),
            capabilities: m.capabilities.clone(),
            state_variables: m.state_variables.clone(),
            actor_names: m
                .actors
                .iter()
                .enumerate()
                .map(|(id, a)| (a.name.clone(), ir::ActorId(id)))
                .collect(),
            cap_names: m
                .capabilities
                .iter()
                .enumerate()
                .map(|(id, c)| {
                    (
                        (m.actors[c.actor.0].name.clone(), c.name.clone()),
                        ir::CapabilityId(id),
                    )
                })
                .collect(),
            state_names: m
                .state_variables
                .iter()
                .enumerate()
                .map(|(id, v)| (v.name.clone(), ir::StateVarId(id)))
                .collect(),
            ..Lower::default()
        };
        let ast = crate::syntax::parse(&crate::r2_inputs::source("R01")).unwrap();
        let p = ast
            .items
            .iter()
            .find_map(|i| {
                if let ast::Item::Process(p) = i {
                    Some(p)
                } else {
                    None
                }
            })
            .unwrap();
        let ds = l.recovery_ast(p).unwrap_err();
        assert_eq!(ds[0].code, E_TYPE_0);
        assert_eq!(ds[0].message, "recovery_provider_state_leak");
    }
}
