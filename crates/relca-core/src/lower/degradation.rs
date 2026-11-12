use super::*;

#[cfg(test)]
#[path = "degradation_tests.rs"]
mod tests;

impl Lower {
    fn degradation_note_error(&mut self, reason: &str, span: Span) {
        if !self
            .errors
            .iter()
            .any(|d| d.severity == diag::Severity::Error)
        {
            self.unsupported(format!("invalid_degradation_annotation: {reason}"), span);
        }
    }
    pub(super) fn degradations(
        &mut self,
        p: &ast::Process,
        body: &ir::Block,
    ) -> Vec<ir::Degradation> {
        if p.degradations.is_empty() {
            return Vec::new();
        }
        if p.degradations.len() > 1 {
            self.unsupported("q6_capacity_limit: one degradation per process", p.span);
        }
        fn supported(b: &ast::Block) -> bool {
            b.iter().all(|s| match &s.value {
                ast::Stmt::Call(c) => {
                    c.retry.is_none()
                        && c.response_loss
                            .as_ref()
                            .is_none_or(|r| matches!(r.value, ast::ResponseLoss::None))
                }
                ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => supported(then_body) && supported(else_body),
                ast::Stmt::End(_) => true,
                _ => false,
            })
        }
        if !supported(&p.body) {
            self.unsupported(
                "q6_feature_combination: degradation requires sequential known-result control",
                p.span,
            );
        }
        let mut routes = Vec::new();
        for route in &p.degradations {
            // Initial scope deliberately permits only top-level annotations.
            let Some(s) = body.iter().find(|s| s.span == route.span) else {
                self.degradation_note_error("missing_statement", route.span);
                continue;
            };
            let ir::Stmt::If {
                condition,
                then_body,
                else_body,
            } = &s.kind
            else {
                self.degradation_note_error("not_if", route.span);
                continue;
            };
            let ir::Cond::Eq(condition) = condition else {
                self.error(
                    diag::U_SYN_0,
                    "degrade requires known result binding equality",
                    route.span,
                );
                continue;
            };
            let Some(producer) = self.calls.values().find(|c| c.binding == condition.binding)
            else {
                self.degradation_note_error("missing_producer", route.span);
                continue;
            };
            routes.push(ir::Degradation {
                name: route.name.value.clone(),
                span: route.span,
                condition: *condition,
                producer: producer.capability,
                degraded_calls: crate::degradation::calls(then_body),
                normal_calls: crate::degradation::calls(else_body),
            });
        }
        routes
    }
}
