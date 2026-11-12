use super::*;

impl Parser<'_> {
    pub(super) fn performance_source_ahead(&self) -> bool {
        let mut depth = 0usize;
        let mut words = self.tokens[self.pos..]
            .iter()
            .filter(|t| t.text != "\n")
            .peekable();
        while let Some(token) = words.next() {
            match token.text {
                "}" if depth == 0 => break,
                "{" => depth += 1,
                "}" => depth -= 1,
                "outcomes"
                    if depth == 0 && words.peek().is_some_and(|t| t.text == "performance") =>
                {
                    return true;
                }
                _ => {}
            }
        }
        false
    }
    pub(super) fn performance(&mut self) -> Parsed<Performance> {
        let start = self.expect("performance")?.span.start;
        let name = self.ident()?;
        self.expect("basis")?;
        let basis = self.string()?;
        self.expect("{")?;
        self.separators();
        self.expect("load")?;
        let queued = if self.take("none") {
            false
        } else if self.take("queued_same_actor_at_start") {
            true
        } else {
            let mut d = self.unsupported("load_metric_unsupported");
            d.code = diag::U_SEM_0.into();
            return Err(d);
        };
        self.separators();
        let mut rows = Vec::new();
        while !self.at("}") {
            self.expect("executor")?;
            let executor = self.ident()?;
            self.expect("{")?;
            self.separators();
            let mut bands = Vec::new();
            while !self.at("}") {
                let (from, until) = if queued {
                    self.expect("[")?;
                    let from = self.integer()?;
                    self.expect(",")?;
                    let until = if self.take("infinity") {
                        None
                    } else {
                        Some(self.integer()?)
                    };
                    self.expect(")")?;
                    self.expect(":")?;
                    (from.value, until.map(|v| v.value))
                } else {
                    (0, None)
                };
                self.expect("kernel")?;
                let kernel = self.ident()?;
                bands.push(PerformanceBand {
                    from,
                    until,
                    kernel,
                });
                self.separators();
            }
            self.expect("}")?;
            rows.push(PerformanceExecutor { executor, bands });
            self.separators();
        }
        self.expect("}")?;
        Ok(Performance {
            name,
            basis,
            queued,
            rows,
            span: self.span(start),
        })
    }
}
