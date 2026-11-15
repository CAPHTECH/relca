use super::*;

impl Lower {
    /// Resolve before lowering any Capability, including forward record reads.
    pub(super) fn registration_declarations(&mut self, model: &ast::Model) {
        for item in &model.items {
            let ast::Item::Actor(actor) = item else {
                continue;
            };
            for field in &actor.items {
                let ast::ActorItem::Capability(cap) = field else {
                    continue;
                };
                for item in &cap.items {
                    let ast::CapItem::Registers(names) = &item.value else {
                        continue;
                    };
                    for name in names {
                        let Some(&id) = self.actor_names.get(&name.value) else {
                            continue;
                        };
                        self.registration_targets.insert(id);
                        let target = model
                            .items
                            .iter()
                            .find_map(|item| match item {
                                ast::Item::Actor(a) if a.name.value == name.value => Some(a),
                                _ => None,
                            })
                            .unwrap();
                        if target
                            .items
                            .iter()
                            .any(|i| matches!(i, ast::ActorItem::Source(..)))
                        {
                            self.error(diag::E_CON_0, "registration_target_is_source", name.span);
                        }
                        if target.items.iter().any(|i| {
                            matches!(
                                i,
                                ast::ActorItem::Capacity(_)
                                    | ast::ActorItem::Executors(_)
                                    | ast::ActorItem::Capability(_)
                                    | ast::ActorItem::State(_)
                                    | ast::ActorItem::Latent(_)
                                    | ast::ActorItem::Decides(_)
                            )
                        }) {
                            self.error(
                                diag::E_CON_0,
                                "registration_target_has_behavior",
                                name.span,
                            );
                        }
                    }
                }
            }
        }
    }
}
