//! ADR 0001 recursive-descent parser. Names, omissions and duplicates are not
//! interpreted here. Separators are discarded, not source-bearing AST nodes.
//!
//! Pending lexical details use the plan's conservative subset: ASCII identifiers
//! and simple (unescaped, possibly UTF-8) strings. Comments, escapes and non-ASCII
//! identifiers are explicitly unsupported rather than silently interpreted.

use crate::{ast::*, diag};
use diag::{Diagnostic, Span};

mod lexer;
#[cfg(test)]
#[path = "syntax/priority_tests.rs"]
mod priority_grammar_tests;
use lexer::{Kind, Token, lex};

type Parsed<T> = Result<T, Diagnostic>;

/// Refusal evidence comes from entering a declaration at its grammar position,
/// not from searching tokens that might instead be names or malformed fields.
pub(crate) fn has_priority_declaration(src: &str) -> bool {
    parse_with_priority_context(src).1
}

/// Parse the entire source without resolving names or supplying semantic defaults.
/// Err is nonempty; malformed in-scope syntax is E-SYN-0, outside syntax U-SYN-0.
pub fn parse(src: &str) -> Result<Model, Vec<Diagnostic>> {
    parse_with_priority_context(src).0
}

fn parse_with_priority_context(src: &str) -> (Result<Model, Vec<Diagnostic>>, bool) {
    let mut parser = Parser {
        src,
        tokens: lex(src),
        pos: 0,
        depth: 0,
        priority_declaration: false,
        degradations: Vec::new(),
        degradation_depth: 0,
        recovery_depth: 0,
        repeat_depth: 0,
    };
    let result = parser.model().map_err(|diagnostic| vec![diagnostic]);
    (result, parser.priority_declaration)
}

struct Parser<'s> {
    src: &'s str,
    tokens: Vec<Token<'s>>,
    pos: usize,
    depth: usize,
    priority_declaration: bool,
    degradations: Vec<Degradation>,
    degradation_depth: usize,
    recovery_depth: usize,
    repeat_depth: usize,
}

impl<'s> Parser<'s> {
    /// Scan only model-level declarations, not strings or nested bodies. This
    /// also recognizes forward declarations without broadening atom's grammar.
    fn record_names(&self) -> std::collections::BTreeSet<&str> {
        let mut depth = 0usize;
        let ts: Vec<_> = self.tokens.iter().filter(|t| t.text != "\n").collect();
        let mut names = std::collections::BTreeSet::new();
        for (i, t) in ts.iter().enumerate() {
            if depth == 1
                && t.kind == Kind::Word
                && t.text == "type"
                && ts.get(i + 2).is_some_and(|t| t.text == "=")
                && ts
                    .get(i + 3)
                    .is_some_and(|t| t.text == "record" && t.kind == Kind::Word)
                && ts.get(i + 4).is_some_and(|t| t.text == "{")
                && let Some(name) = ts.get(i + 1)
            {
                names.insert(name.text);
            }
            if t.text == "{" && t.kind != Kind::String {
                depth += 1;
            }
            if t.text == "}" && t.kind != Kind::String {
                depth = depth.saturating_sub(1);
            }
        }
        names
    }

    fn named_type(&mut self) -> Parsed<Ident> {
        if self.at("enum") {
            return Err(self.unsupported("inline enum position is unsupported"));
        }
        if self.at("record") {
            let p = self.pos;
            self.pos += 1;
            if self.at("{") {
                let body: Vec<_> = self.tokens[self.pos + 1..]
                    .iter()
                    .filter(|t| t.text != "\n")
                    .take(2)
                    .collect();
                if (body.first().is_some_and(|t| t.text == "}")
                    && !self.declared_enum_type("record"))
                    || body.get(1).is_some_and(|t| matches!(t.text, ":" | ","))
                {
                    self.pos = p;
                    return Err(self.unsupported("inline record is unsupported"));
                }
            }
            self.pos = p;
        }
        self.ident()
    }

    fn declared_enum_type(&self, name: &str) -> bool {
        let ts: Vec<_> = self.tokens.iter().filter(|t| t.text != "\n").collect();
        let mut depth = 0usize;
        for (i, t) in ts.iter().enumerate() {
            if depth == 1
                && t.kind == Kind::Word
                && t.text == "type"
                && ts
                    .get(i + 1)
                    .is_some_and(|t| t.kind == Kind::Word && t.text == name)
                && ts.get(i + 2).is_some_and(|t| t.text == "=")
                && ts
                    .get(i + 3)
                    .is_some_and(|t| t.kind == Kind::Word && t.text == "enum")
            {
                return true;
            }
            if t.text == "{" {
                depth += 1;
            }
            if t.text == "}" {
                depth = depth.saturating_sub(1);
            }
        }
        false
    }

    fn declared_enum_member(&self, name: &str) -> bool {
        let ts: Vec<_> = self.tokens.iter().filter(|t| t.text != "\n").collect();
        let mut depth = 0usize;
        for (i, t) in ts.iter().enumerate() {
            if depth == 1
                && t.kind == Kind::Word
                && t.text == "type"
                && ts.get(i + 2).is_some_and(|t| t.text == "=")
                && ts
                    .get(i + 3)
                    .is_some_and(|t| t.kind == Kind::Word && t.text == "enum")
                && ts.get(i + 4).is_some_and(|t| t.text == "{")
                && ts[i + 5..]
                    .iter()
                    .take_while(|t| t.text != "}")
                    .any(|t| t.kind == Kind::Word && t.text == name)
            {
                return true;
            }
            if t.text == "{" {
                depth += 1;
            }
            if t.text == "}" {
                depth = depth.saturating_sub(1);
            }
        }
        false
    }

    /// A model that declares no record keeps the legacy value reading: the
    /// `{` after a value is left to the caller, as before ADR 0038.
    fn value_expr(&mut self) -> Parsed<ValueExpr> {
        let head = self.value_ident()?;
        let after_head = self.pos;
        if self.record_names().is_empty() || !self.at("{") {
            self.pos = after_head;
            return Ok(head.into());
        }
        self.expect("{")?;
        let mut fields = Vec::new();
        if !self.at("}") {
            loop {
                let name = self.ident()?;
                self.expect("=")?;
                let value = self.value_ident()?;
                if self.at("{") || self.at(".") || self.at("+") || self.at("-") {
                    return Err(self.unsupported("record field value is unsupported"));
                }
                fields.push((name, value));
                if !self.take(",") {
                    break;
                }
            }
        }
        self.expect("}")?;
        Ok(ValueExpr {
            span: self.span(head.span.start),
            head,
            fields: Some(fields),
        })
    }

    fn reject_record_comparison(&mut self) -> Parsed<()> {
        let ts: Vec<_> = self.tokens[self.pos..]
            .iter()
            .filter(|t| t.text != "\n")
            .take(9)
            .collect();
        let rhs = if ts.get(1).is_some_and(|t| matches!(t.text, "!" | "<" | ">"))
            && ts.get(2).is_some_and(|t| t.text == "=")
        {
            3
        } else {
            2
        };
        if ts.first().is_some_and(|t| t.kind == Kind::Word)
            && ts
                .get(1)
                .is_some_and(|t| matches!(t.text, "==" | "!" | "<" | ">"))
            && ts
                .get(rhs)
                .is_some_and(|t| self.record_names().contains(t.text))
            && ts.get(rhs + 1).is_some_and(|t| t.text == "{")
            && ((ts.get(rhs + 2).is_some_and(|t| t.text == "}")
                && !self.declared_enum_member(ts[rhs].text))
                || (ts.get(rhs + 3).is_some_and(|t| t.text == "=")
                    && ts
                        .get(rhs + 5)
                        .is_some_and(|t| matches!(t.text, "," | "}" | "{"))))
        {
            return Err(self.unsupported("record comparison is unsupported"));
        }
        Ok(())
    }

    fn reject_record_literal(&mut self, position: &str) -> Parsed<()> {
        self.lines();
        let ts: Vec<_> = self.tokens[self.pos..]
            .iter()
            .filter(|t| t.text != "\n")
            .take(2)
            .collect();
        if ts.len() == 2 && self.record_names().contains(ts[0].text) && ts[1].text == "{" {
            return Err(self.unsupported(position));
        }
        Ok(())
    }
    fn current(&self) -> Token<'s> {
        self.tokens[self.pos]
    }

    fn lines(&mut self) {
        while self.current().text == "\n" {
            self.pos += 1;
        }
    }

    fn separators(&mut self) {
        while matches!(self.current().text, "\n" | ";") {
            self.pos += 1;
        }
    }

    fn at(&mut self, text: &str) -> bool {
        self.lines();
        self.current().text == text && self.current().kind != Kind::Eof
    }

    fn take(&mut self, text: &str) -> bool {
        if self.at(text) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    // ADR 0013: <= and >= are two adjacent symbols read only at the
    // probability comparison, so optional<T>= keeps its old reading.
    fn at_adjacent(&mut self, first: &str, second: &str) -> bool {
        self.lines();
        let (a, b) = (
            self.current(),
            self.tokens[(self.pos + 1).min(self.tokens.len() - 1)],
        );
        a.text == first && b.text == second && a.span.end == b.span.start
    }

    fn take_adjacent(&mut self, first: &str, second: &str) -> bool {
        if self.at_adjacent(first, second) {
            self.pos += 2;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, text: &str) -> Parsed<Token<'s>> {
        self.lines();
        let token = self.current();
        if self.take(text) {
            Ok(token)
        } else {
            Err(self.error(&format!("expected {text}")))
        }
    }

    fn error(&self, message: &str) -> Diagnostic {
        let token = self.current();
        let unsupported = token.kind == Kind::Unsupported
            || (token.kind == Kind::Word && outside_keyword(token.text));
        Diagnostic {
            code: if unsupported {
                diag::U_SYN_0
            } else {
                diag::E_SYN_0
            }
            .into(),
            severity: diag::Severity::Error,
            message: if unsupported {
                format!("unsupported syntax: {}", token.text)
            } else {
                message.into()
            },
            span: Some(token.span),
        }
    }

    // At a choice among keywords (item, capability item, join kind, verify
    // kind...), an unknown word names a construct outside ADR 0001, as do
    // arguments and decimals: unsupported. Missing structure stays E-SYN-0.
    fn choice_error(&self, message: &str) -> Diagnostic {
        let mut diagnostic = self.error(message);
        if self.current().kind == Kind::Word {
            diagnostic.code = diag::U_SYN_0.into();
            diagnostic.message = format!("unsupported syntax: {}", self.current().text);
        }
        diagnostic
    }

    fn expect_choice(&mut self, text: &str) -> Parsed<Token<'s>> {
        self.lines();
        let token = self.current();
        if self.take(text) {
            Ok(token)
        } else {
            Err(self.choice_error(&format!("expected {text}")))
        }
    }

    fn parameters(&mut self) -> Parsed<Vec<Param>> {
        self.expect("(")?;
        let mut params = Vec::new();
        if !self.at(")") {
            loop {
                let name = self.ident()?;
                if !self.take(":") {
                    return Err(self.unsupported("untyped parameter"));
                }
                if self.current().kind == Kind::Integer {
                    return Err(self.unsupported("bounded integer parameter"));
                }
                let ty = self.named_type()?;
                if self.at("<") {
                    return Err(self.unsupported("parameter type constructor"));
                }
                params.push(Param { name, ty });
                if !self.take(",") {
                    break;
                }
            }
        }
        self.expect(")")?;
        Ok(params)
    }

    fn arguments(&mut self) -> Parsed<Vec<Ident>> {
        self.expect("(")?;
        let mut args = Vec::new();
        if !self.at(")") {
            loop {
                let mut arg = self.ident()?;
                if !self.record_names().is_empty() && self.at("{") {
                    return Err(self.unsupported("record argument literal is unsupported"));
                }
                if arg.value == "case" && self.take(".") {
                    let member = self.ident()?;
                    arg.value = format!("case.{}", member.value);
                    arg.span = self.span(arg.span.start);
                    if member.value != "id" {
                        let mut d = self.unsupported(&format!("{} argument", arg.value));
                        d.span = Some(arg.span);
                        return Err(d);
                    }
                }
                args.push(arg);
                if self.at(":") || self.at("=") {
                    return Err(self.unsupported("named arguments"));
                }
                if !self.take(",") {
                    break;
                }
            }
        }
        self.expect(")")?;
        Ok(args)
    }

    fn unsupported(&self, message: &str) -> Diagnostic {
        let mut diagnostic = self.error(message);
        diagnostic.code = diag::U_SYN_0.into();
        diagnostic.message = format!("unsupported syntax: {message}");
        diagnostic
    }

    fn start(&mut self) -> usize {
        self.lines();
        self.current().span.start
    }

    fn span(&self, start: usize) -> Span {
        Span {
            start,
            end: self.tokens[..self.pos]
                .iter()
                .rev()
                .find(|token| !matches!(token.text, "\n" | ";"))
                .map_or(start, |token| token.span.end),
        }
    }

    fn ident(&mut self) -> Parsed<Ident> {
        self.lines();
        let token = self.current();
        if token.kind != Kind::Word || keyword(token.text) || outside_keyword(token.text) {
            return Err(self.error("expected identifier"));
        }
        self.pos += 1;
        Ok(Spanned {
            value: token.text.into(),
            span: token.span,
        })
    }

    fn integer(&mut self) -> Parsed<Integer> {
        self.lines();
        let token = self.current();
        if token.kind != Kind::Integer {
            return Err(self.error("expected nonnegative integer"));
        }
        let value = token
            .text
            .parse::<u64>()
            .map_err(|_| self.unsupported("integer exceeds u64"))?;
        self.pos += 1;
        Ok(Spanned {
            value,
            span: token.span,
        })
    }

    fn value_ident(&mut self) -> Parsed<Ident> {
        self.lines();
        if self.current().kind == Kind::Integer || matches!(self.current().text, "+" | "-") {
            let start = self.current().span.start;
            let mut end = start;
            if matches!(self.current().text, "+" | "-") {
                end = self.current().span.end;
                self.pos += 1;
                if self.current().span.start != end || self.current().kind != Kind::Integer {
                    return Err(self.unsupported("split integer sign"));
                }
            }
            let n = self.integer()?;
            end = end.max(n.span.end);
            Ok(Spanned {
                value: self.src[start..end].into(),
                span: Span { start, end },
            })
        } else if self.at("none") {
            let span = self.current().span;
            self.pos += 1;
            Ok(Spanned {
                value: "none".into(),
                span,
            })
        } else {
            self.ident()
        }
    }

    fn probability(&mut self) -> Parsed<Ident> {
        self.lines();
        let start = self.current().span.start;
        if self.current().text == "+" {
            return Err(self.unsupported("positive probability sign"));
        }
        if self.current().text == "-" {
            self.pos += 1;
        }
        if self.current().kind != Kind::Integer {
            return Err(self.error("expected probability"));
        }
        let mut end = self.current().span.end;
        self.pos += 1;
        if self.current().text == "." {
            if self.current().span.start != end {
                return Err(self.error("split probability"));
            }
            end = self.current().span.end;
            self.pos += 1;
            if self.current().kind != Kind::Integer || self.current().span.start != end {
                return Err(self.error("expected fractional digits"));
            }
            end = self.current().span.end;
            self.pos += 1;
        }
        if self.current().text == "/"
            || (self.current().kind == Kind::Word && self.current().span.start == end)
        {
            return Err(self.unsupported("exponent or fraction probability"));
        }
        let value = self.src[start..end].to_owned();
        if value.bytes().filter(u8::is_ascii_digit).count() > 64 {
            return Err(self.unsupported("probability exceeds 64 digits"));
        }
        Ok(Spanned {
            value,
            span: Span { start, end },
        })
    }

    fn simulation_label(&mut self, word: &str) -> Parsed<()> {
        self.lines();
        if self.take(word) {
            return Ok(());
        }
        if self.current().kind == Kind::Word
            && !matches!(self.current().text, "replications" | "seed" | "confidence")
        {
            return Err(self.unsupported("simulation setting"));
        }
        if self.current().text == "{" {
            return Err(self.unsupported("simulation block"));
        }
        Err(self.error(&format!("expected {word}")))
    }

    fn simulation_integer(&mut self) -> Parsed<Ident> {
        self.lines();
        let start = self.current().span.start;
        if self.current().text == "-" {
            self.pos += 1;
        }
        if self.current().kind != Kind::Integer {
            return Err(
                if self.current().kind == Kind::Word
                    && !matches!(self.current().text, "replications" | "seed" | "confidence")
                    || self.current().text == "+"
                {
                    self.unsupported("simulation integer")
                } else {
                    self.error("expected simulation integer")
                },
            );
        }
        let end = self.current().span.end;
        self.pos += 1;
        if matches!(self.current().text, "." | "/" | "+" | "-" | "*")
            || self.current().kind == Kind::Word && self.current().span.start == end
        {
            return Err(self.unsupported("simulation integer expression"));
        }
        Ok(Spanned {
            value: self.src[start..end].into(),
            span: Span { start, end },
        })
    }

    fn simulation(&mut self) -> Parsed<SimulationDecl> {
        let start = self.expect("simulation")?.span.start;
        // At the first label, order selection is unsupported; later known
        // labels arriving early diagnose a missing mandatory label/value.
        self.lines();
        if matches!(self.current().text, "seed" | "confidence") {
            return Err(self.unsupported("simulation setting order"));
        }
        self.simulation_label("replications")?;
        let replications = self.simulation_integer()?;
        self.simulation_label("seed")?;
        let seed = self.simulation_integer()?;
        self.simulation_label("confidence")?;
        let confidence = self.probability().map_err(|mut d| {
            if d.message.contains("probability exceeds 64 digits") {
                d.code = diag::E_SIM_0.into();
                d.message = "confidence exceeds 64 digits".into();
                d.span = Some(self.span(d.span.unwrap().start));
            }
            d
        })?;
        if matches!(self.current().text, "+" | "-" | "*" | "/") {
            return Err(self.unsupported("confidence expression"));
        }
        if !matches!(self.current().text, "\n" | ";" | "}") && self.current().kind != Kind::Eof {
            return Err(self.unsupported("simulation suffix"));
        }
        Ok(SimulationDecl {
            replications,
            seed,
            confidence,
            span: self.span(start),
        })
    }

    fn state_refs(&mut self) -> Parsed<Vec<Path>> {
        self.expect("[")?;
        let mut refs = Vec::new();
        if !self.at("]") {
            loop {
                let p = self.path()?;
                if p.value.len() > 2 {
                    return Err(self.unsupported("multi-level state ref"));
                }
                refs.push(p);
                if !self.take(",") {
                    break;
                }
            }
        }
        self.expect("]")?;
        Ok(refs)
    }

    fn kernel(&mut self) -> Parsed<KernelDecl> {
        let start = self.expect("kernel")?.span.start;
        let name = self.ident()?;
        let given = if self.take("given") {
            self.state_refs()?
        } else {
            Vec::new()
        };
        let mut basis = Vec::new();
        while self.at("basis") {
            let begin = self.start();
            self.expect("basis")?;
            let token = self.string()?;
            basis.push(Spanned {
                value: token.value,
                span: self.span(begin),
            });
        }
        if !self.at("{") {
            return Err(self.unsupported("kernel header"));
        }
        self.expect("{")?;
        self.separators();
        if self.at("}") {
            return Err(self.error("kernel requires a row"));
        }
        let mut rows = Vec::new();
        while !self.at("}") {
            let begin = self.start();
            let pattern = if self.take("*") {
                None
            } else {
                self.reject_record_literal("record kernel pattern is unsupported")?;
                let mut values = vec![self.value_ident()?];
                while self.take(",") {
                    self.reject_record_literal("record kernel pattern is unsupported")?;
                    values.push(self.value_ident()?);
                }
                Some(values)
            };
            self.expect("->")?;
            self.expect("{")?;
            self.separators();
            if self.at("}") {
                return Err(self.error("row requires a branch"));
            }
            let mut branches = Vec::new();
            while !self.at("}") {
                let probability = self.probability()?;
                self.expect(":")?;
                let result = self.value_expr()?;
                let mut assignments = Vec::new();
                if self.take("with") {
                    loop {
                        let target = self.ident()?;
                        let target = if self.at(".") {
                            let error = self.unsupported("qualified with");
                            self.expect(".")?;
                            let variable = self.ident()?;
                            if !self.rule_target(&target.value, &variable.value) {
                                return Err(error);
                            }
                            let span = Span {
                                start: target.span.start,
                                end: variable.span.end,
                            };
                            StateTarget::Qualified {
                                actor: target,
                                variable,
                                span,
                            }
                        } else {
                            StateTarget::Unqualified(target)
                        };
                        self.expect("=")?;
                        self.reject_record_literal("record kernel with is unsupported")?;
                        let value = self.value_ident()?;
                        if matches!(self.current().text, "." | "+" | "-" | "*" | "/") {
                            return Err(self.unsupported("with expression"));
                        }
                        assignments.push(Assignment {
                            target,
                            value: value.into(),
                            operator: AssignmentOp::Assign,
                        });
                        if !self.take(",") {
                            break;
                        }
                    }
                }
                branches.push(KernelBranch {
                    probability,
                    result,
                    assignments,
                });
                let before = self.pos;
                self.separators();
                if !self.at("}") && before == self.pos {
                    return Err(self.error("expected branch separator"));
                }
            }
            self.expect("}")?;
            rows.push(KernelRow {
                pattern,
                branches,
                span: self.span(begin),
            });
            let before = self.pos;
            self.separators();
            if !self.at("}") && before == self.pos {
                return Err(self.error("expected row separator"));
            }
        }
        self.expect("}")?;
        Ok(KernelDecl {
            name,
            given,
            basis,
            rows,
            span: self.span(start),
        })
    }

    fn var_decl(&mut self) -> Parsed<VarDecl> {
        self.var_decl_mode(false)
    }
    fn var_decl_mode(&mut self, rule: bool) -> Parsed<VarDecl> {
        let start = self.start();
        let mut names = vec![self.ident()?];
        while self.take(",") {
            names.push(self.ident()?);
        }
        self.expect(":")?;
        let optional = self.take("optional");
        if optional {
            self.expect("<")?;
        }
        let integer_max;
        let mut case_set = false;
        let ty = if self.current().kind == Kind::Integer || self.at("-") {
            if optional {
                return Err(self.unsupported("optional bounded integer"));
            }
            let start = self.start();
            if self.at("-") {
                return Err(self.unsupported("negative integer lower bound"));
            }
            let lo = self.integer()?;
            if !self.take_adjacent(".", ".") {
                return Err(self.error("expected .."));
            }
            if lo.value != 0 {
                return Err(self.unsupported("nonzero integer lower bound"));
            }
            if self.at("-") || self.at("+") {
                return Err(self.unsupported("signed integer upper bound"));
            }
            if self.at("(") {
                return Err(self.unsupported("integer upper expression"));
            }
            let hi = self.integer()?;
            if matches!(self.current().text, "+" | "-" | "*" | "/") {
                return Err(self.unsupported("integer bound expression"));
            }
            integer_max = Some(hi.value);
            Spanned {
                value: self.src[start..hi.span.end].into(),
                span: self.span(start),
            }
        } else {
            integer_max = None;
            let ty = self.named_type()?;
            if ty.value == "set" && self.take("<") {
                if optional || !self.at("CaseId") {
                    return Err(self.unsupported("set element type is unsupported"));
                }
                self.ident()?;
                self.expect(">")?;
                case_set = true;
            }
            ty
        };
        if self.current().text == "<" {
            return Err(self.unsupported("nested or unknown type constructor"));
        }
        if optional {
            self.expect(">")?;
        }
        let mut default = None;
        let mut distribution = None;
        loop {
            let before = self.pos;
            if self.take("=") {
                if case_set && !rule {
                    return Err(self.unsupported("unsupported set initializer"));
                }
                if default.is_some() {
                    return Err(self.error("duplicate default"));
                }
                default = Some(self.value_expr()?);
            } else if self.take("~") {
                if case_set {
                    return Err(self.unsupported("unsupported set initializer"));
                }
                if self.record_names().contains(ty.value.as_str()) {
                    return Err(self.unsupported("record distribution is unsupported"));
                }
                if distribution.is_some() {
                    return Err(self.error("duplicate distribution"));
                }
                self.expect("{")?;
                let mut entries = Vec::new();
                loop {
                    let value = self.value_ident()?;
                    self.expect(":")?;
                    entries.push((value, self.probability()?));
                    if !self.take(",") {
                        break;
                    }
                }
                self.expect("}")?;
                distribution = Some(entries);
            } else {
                self.pos = before;
                break;
            }
        }
        if self.current().text == "." {
            return Err(self.unsupported("state default expression"));
        }
        let mut last_write_wins = Vec::new();
        loop {
            let before = self.pos;
            if !self.take("last_write_wins") {
                self.pos = before;
                break;
            }
            let span = self.span(self.tokens[before].span.start);
            let after = self.pos;
            if self.at(":") || self.at(",") {
                self.pos = before;
                break;
            }
            self.pos = after;
            last_write_wins.push(span);
        }
        Ok(VarDecl {
            last_write_wins,
            case_set,
            integer_max,
            distribution,
            names,
            ty,
            optional,
            default,
            span: self.span(start),
        })
    }

    fn state_block(&mut self) -> Parsed<Vec<VarDecl>> {
        self.expect("case")?;
        self.expect("{")?;
        self.separators();
        let mut declarations = Vec::new();
        while !self.at("}") {
            let d = self.var_decl()?;
            if d.case_set {
                let mut diagnostic =
                    self.unsupported("set outside shared Actor state is unsupported");
                diagnostic.span = Some(d.ty.span);
                return Err(diagnostic);
            }
            declarations.push(d);
            let before = self.pos;
            self.separators();
            if !self.at("}") && before == self.pos {
                return Err(self.error("expected variable separator"));
            }
        }
        self.expect("}")?;
        Ok(declarations)
    }

    fn duration(&mut self) -> Parsed<Duration> {
        let start = self.start();
        let amount = self.integer()?;
        if self.at(".") {
            return Err(self.unsupported("decimal duration"));
        }
        // min is reserved, but other unit spellings must reach lower unchanged.
        let unit = if self.at("min") {
            let token = self.expect("min")?;
            Spanned {
                value: token.text.into(),
                span: token.span,
            }
        } else {
            self.ident()?
        };
        Ok(Duration {
            amount,
            unit,
            span: self.span(start),
        })
    }

    fn names(&mut self, close: &str) -> Parsed<Vec<Ident>> {
        let mut names = vec![self.ident()?];
        while self.take(",") {
            names.push(self.ident()?);
        }
        self.expect(close)?;
        Ok(names)
    }

    fn discrete_duration(&mut self) -> Parsed<Duration> {
        self.lines();
        if self.current().kind == Kind::Word {
            return Err(self.unsupported("named duration"));
        }
        let d = self.duration()?;
        if !matches!(d.unit.value.as_str(), "min" | "h" | "d") {
            return Err(self.unsupported("only integer durations in min, h or d are supported"));
        }
        Ok(d)
    }

    fn metrics_integer(&mut self) -> Parsed<Integer> {
        self.lines();
        if self.current().text == "-" {
            let start = self.current().span.start;
            self.pos += 1;
            if self.current().kind == Kind::Integer {
                self.pos += 1;
            }
            return Err(Diagnostic {
                code: diag::E_SYN_0.into(),
                severity: diag::Severity::Error,
                message: "metrics values must be nonnegative integers".into(),
                span: Some(self.span(start)),
            });
        }
        if self.current().text == "+" {
            return Err(self.unsupported("positive metrics sign"));
        }
        let n = self.integer()?;
        if matches!(self.current().text, "." | "/" | "+" | "-" | "*")
            || self.current().kind == Kind::Word
                && self.current().span.start == n.span.end
                && !matches!(self.current().text, "min" | "h" | "d")
        {
            return Err(self.unsupported("metrics numeric expression"));
        }
        Ok(n)
    }

    fn metrics(&mut self) -> Parsed<MetricsDecl> {
        let start = self.expect("metrics")?.span.start;
        self.expect_choice("{")?;
        let mut items = Vec::new();
        let (mut window, mut currency) = (false, false);
        self.separators();
        while !self.at("}") {
            let item_start = self.start();
            let value = match self.current().text {
                "breakdown" => {
                    let unsupported = self.choice_error("expected metrics item");
                    self.pos += 1;
                    self.lines();
                    if !self.take("waits") {
                        return Err(unsupported);
                    }
                    MetricsItem::BreakdownWaits
                }
                "window" => {
                    self.pos += 1;
                    let amount = self.metrics_integer()?;
                    self.lines();
                    if self.current().kind == Kind::Word
                        && !matches!(self.current().text, "min" | "h" | "d")
                    {
                        return Err(self.unsupported("metrics window unit must be min, h or d"));
                    }
                    let unit = self.current();
                    if !matches!(unit.text, "min" | "h" | "d") {
                        return Err(self.error("expected time unit"));
                    }
                    self.pos += 1;
                    window = true;
                    MetricsItem::Window(Duration {
                        amount,
                        unit: Spanned {
                            value: unit.text.into(),
                            span: unit.span,
                        },
                        span: self.span(item_start),
                    })
                }
                "currency" => {
                    self.pos += 1;
                    self.lines();
                    let token = self.current();
                    if token.kind != Kind::String {
                        return Err(self.error("expected currency string"));
                    }
                    self.pos += 1;
                    currency = true;
                    MetricsItem::Currency(Spanned {
                        value: token.text[1..token.text.len() - 1].into(),
                        span: token.span,
                    })
                }
                "rate" => {
                    self.pos += 1;
                    let actor = self.ident()?;
                    if self.take(".") {
                        let capability = self.ident()?;
                        let span = Span {
                            start: actor.span.start,
                            end: capability.span.end,
                        };
                        if self.at(".") {
                            return Err(
                                self.unsupported("metrics rate path has more than two components")
                            );
                        }
                        self.expect_choice("per_start")?;
                        let price = self.metrics_integer()?;
                        MetricsItem::StartRate {
                            target: CapabilityPath {
                                actor,
                                capability,
                                span,
                            },
                            price,
                        }
                    } else if self.take("overtime_per_minute") {
                        MetricsItem::OvertimeRate {
                            actor,
                            price: self.metrics_integer()?,
                        }
                    } else {
                        self.expect_choice("per_minute")?;
                        let price = self.metrics_integer()?;
                        MetricsItem::ActorRate { actor, price }
                    }
                }
                "simulation" | "actor" | "process" | "bounds" | "verify" | "cut"
                | "configuration" => {
                    let mut d = self.error("expected closing metrics brace");
                    d.code = diag::E_SYN_0.into();
                    return Err(d);
                }
                _ => return Err(self.choice_error("expected metrics item")),
            };
            items.push(Spanned {
                value,
                span: self.span(item_start),
            });
            self.separators();
        }
        self.expect("}")?;
        if !window || !currency {
            return Err(Diagnostic {
                code: diag::E_SYN_0.into(),
                severity: diag::Severity::Error,
                message: "metrics requires window and currency".into(),
                span: Some(self.span(start)),
            });
        }
        Ok(MetricsDecl {
            items,
            span: self.span(start),
        })
    }

    fn quality_property(&mut self) -> Parsed<QualityProperty> {
        let saved = self.pos;
        let start = self.expect("property")?.span.start;
        if self.at("cohort") {
            return Err(self.unsupported("cohort probability"));
        }
        if !self.take("case") {
            self.pos = saved;
            return Err(self.unsupported("property"));
        }
        self.expect(".")?;
        let name = self.ident()?;
        self.expect("=")?;
        self.expect_choice("reached")?;
        self.expect("(")?;
        if matches!(self.current().text, "always" | "eventually") {
            return Err(self.unsupported("quality temporal operator"));
        }
        let target = if self.take("end") {
            self.expect(".")?;
            let mut ends = vec![self.ident()?];
            while self.take("||") {
                if !self.take("end") {
                    let mut d = self.unsupported("end union cannot mix with state conditions");
                    d.message = "end union cannot mix with state conditions".into();
                    return Err(d);
                }
                self.expect(".")?;
                ends.push(self.ident()?);
            }
            if !self.at(")") {
                let mut d = self.unsupported("end union cannot mix with state conditions");
                d.message = "end union cannot mix with state conditions".into();
                return Err(d);
            }
            QualityTarget::Ends(ends)
        } else {
            if self.at("case") {
                return Err(self.unsupported("quality property references are unsupported"));
            }
            QualityTarget::State(self.cond().map_err(|mut d| {
                if d.message == "expected ==" {
                    d.code = diag::U_SYN_0.into();
                    d.message = "unsupported quality condition".into();
                }
                d
            })?)
        };
        self.expect(")")?;
        let within = if self.take("within") {
            Some(self.discrete_duration()?)
        } else {
            None
        };
        Ok(QualityProperty {
            name,
            target,
            within,
            span: self.span(start),
        })
    }

    fn quality_count(&mut self) -> Parsed<Ident> {
        self.expect_choice("expected")?;
        self.expect("(")?;
        self.expect_choice("count")?;
        self.expect("(")?;
        self.expect_choice("case")?;
        self.expect(".")?;
        let name = self.ident()?;
        self.expect(")")?;
        if self.at("/") {
            let mut d = self.unsupported("expectation of ratio is unsupported");
            d.message = "expectation of ratio is unsupported".into();
            return Err(d);
        }
        self.expect(")")?;
        Ok(name)
    }

    fn cohort_metric(&mut self) -> Parsed<CohortMetric> {
        let saved = self.pos;
        let start = self.expect("metric")?.span.start;
        if !self.take("cohort") {
            self.pos = saved;
            return Err(self.unsupported("metric"));
        }
        self.expect(".")?;
        let name = self.ident()?;
        self.expect("=")?;
        let numerator = self.quality_count()?;
        self.expect("/")?;
        let denominator_start = self.start();
        let denominator = if self.take("arrivals") {
            None
        } else if self.at("expected") {
            Some(self.quality_count()?)
        } else {
            return Err(self.unsupported(
                "metric denominator must be arrivals or expected(count(case.<name>))",
            ));
        };
        if matches!(self.current().text, "/" | "*" | "+" | "-") {
            return Err(self.unsupported("quality metric arithmetic"));
        }
        Ok(CohortMetric {
            name,
            numerator,
            denominator,
            denominator_span: self.span(denominator_start),
            span: self.span(start),
        })
    }

    fn quantile_metric(&mut self) -> Parsed<QuantileMetric> {
        let start = self.expect("metric")?.span.start;
        self.expect("quantile")?;
        self.expect(".")?;
        let name = self.ident()?;
        self.expect("=")?;
        self.expect_choice("quantile")?;
        self.expect("(")?;
        let p = self.probability()?;
        self.expect(",")?;
        self.expect_choice("case")?;
        self.expect(".")?;
        let property = self.ident()?;
        let given_reached = self.take("given");
        if given_reached && !self.take("reached") {
            let mut d = self.unsupported("quantile given must be reached");
            d.message = "quantile given must be reached".into();
            return Err(d);
        }
        self.expect(")")?;
        Ok(QuantileMetric {
            name,
            p,
            property,
            given_reached,
            span: self.span(start),
        })
    }

    fn model(&mut self) -> Parsed<Model> {
        self.separators();
        let start = self.expect("model")?.span.start;
        let name = self.ident()?;
        self.expect("version")?;
        self.lines();
        let token = self.current();
        if token.kind != Kind::String {
            return Err(self.error("expected version string"));
        }
        let version = Spanned {
            value: token.text[1..token.text.len() - 1].into(),
            span: token.span,
        };
        self.pos += 1;
        self.expect("{")?;
        let mut items = Vec::new();
        self.separators();
        while !self.at("}") {
            items.push(match self.current().text {
                "case_priority" => {
                    self.priority_declaration = true;
                    let start = self.expect("case_priority")?.span.start;
                    self.expect_choice("fixed")?;
                    self.expect("[")?;
                    let mut values = Vec::new();
                    if !self.at("]") {
                        loop {
                            if self.current().kind != Kind::Integer {
                                return Err(self.choice_error("expected fixed priority integer"));
                            }
                            values.push(self.integer()?);
                            if !self.take(",") {
                                break;
                            }
                        }
                    }
                    self.expect("]")?;
                    Item::CasePriority(Spanned {
                        value: values,
                        span: self.span(start),
                    })
                }
                "calendar" => Item::Calendar(self.calendar()?),
                "person_calendar" => {
                    self.expect("person_calendar")?;
                    Item::PersonCalendar {
                        person: self.ident()?,
                        calendar: self.ident()?,
                    }
                }
                "compare"
                    if self
                        .tokens
                        .iter()
                        .skip(self.pos + 1)
                        .find(|t| t.text != "\n")
                        .is_some_and(|t| t.text == "[") =>
                {
                    Item::Compare(self.compare_decl()?)
                }
                "property" => Item::QualityProperty(self.quality_property()?),
                "metric" => {
                    let quantile = self
                        .tokens
                        .iter()
                        .skip(self.pos + 1)
                        .find(|t| t.text != "\n")
                        .is_some_and(|t| t.text == "quantile");
                    if quantile {
                        Item::QuantileMetric(self.quantile_metric()?)
                    } else {
                        Item::CohortMetric(self.cohort_metric()?)
                    }
                }
                "resource" => {
                    let start = self.expect("resource")?.span.start;
                    let name = self.ident()?;
                    self.expect("capacity")?;
                    let capacity = self.integer()?;
                    if matches!(self.current().text, "." | "+" | "-" | "*" | "/") {
                        return Err(self.unsupported("resource capacity expression"));
                    }
                    Item::Resource(ResourceDecl {
                        name,
                        capacity,
                        calendar: if self.take("calendar") {
                            Some(self.ident()?)
                        } else {
                            None
                        },
                        span: self.span(start),
                    })
                }
                "metrics" => Item::Metrics(self.metrics()?),
                "simulation" => Item::Simulation(self.simulation()?),
                "scenario" => Item::Scenario(self.scenario()?),
                "arrivals" => self.arrivals()?,
                "policy" => Item::Policy(self.start_policy()?),
                word @ ("queue" | "concurrent") => {
                    let limit = self.limit(word)?;
                    if word == "queue" {
                        Item::Queue(limit)
                    } else {
                        Item::Concurrent(limit)
                    }
                }
                word @ ("observed" | "latent") => {
                    // Only the case block is in range; observed and latent agree
                    // on U-SYN-0 for any other continuation.
                    let next = self
                        .tokens
                        .iter()
                        .skip(self.pos + 1)
                        .find(|t| t.text != "\n")
                        .map(|t| t.text);
                    if next == Some("{") {
                        // The block is there but its case word is missing.
                        self.pos += 1;
                        return Err(self.error("expected case"));
                    }
                    if next != Some("case") {
                        return Err(self.unsupported(&format!("{word} without case")));
                    }
                    self.pos += 1;
                    if word == "observed" {
                        Item::ObservedCase(self.state_block()?)
                    } else {
                        Item::LatentCase(self.state_block()?)
                    }
                }
                "type" => Item::Type(self.type_decl()?),
                "kernel" => Item::Kernel(self.kernel()?),
                "performance" => Item::Performance(self.performance()?),
                "actor" => Item::Actor(self.actor()?),
                "process" => Item::Process(self.process()?),
                "bounds" => Item::Bounds(self.bounds()?),
                "verify" => Item::Verify(self.verify()?),
                "cut" => Item::Cut(self.cut()?),
                "configuration" => Item::Configuration(self.configuration()?),
                "contract" => Item::Contract(self.contract()?),
                "people" => {
                    let start = self.expect("people")?.span.start;
                    self.expect("[")?;
                    let value = self.names("]")?;
                    Item::People(Spanned {
                        value,
                        span: self.span(start),
                    })
                }
                "permit" => Item::Permit(self.permit()?),
                "authority" => Item::Authority(self.authority()?),
                "responsible" => {
                    let start = self.expect("responsible")?.span.start;
                    let person = self.ident()?;
                    self.expect("for")?;
                    let target = if self.take("(") {
                        let c = self.responsibility_cond()?;
                        self.expect(")")?;
                        match Self::legacy_responsibility_cond(&c) {
                            Some(old) => ResponsibilityTarget::Condition(old),
                            None => ResponsibilityTarget::Formula(c),
                        }
                    } else {
                        ResponsibilityTarget::Capability(self.capability_path()?)
                    };
                    let within = if self.take("within") {
                        Some(self.duration()?)
                    } else {
                        None
                    };
                    Item::Responsible(Responsible {
                        person,
                        target,
                        within,
                        span: self.span(start),
                    })
                }
                _ => return Err(self.choice_error("expected model item or closing brace")),
            });
            self.separators();
        }
        self.expect("}")?;
        let span = self.span(start);
        self.separators();
        if self.current().kind != Kind::Eof {
            return Err(self.error("unexpected input after model"));
        }
        Ok(Model {
            name,
            version,
            items,
            span,
        })
    }

    fn calendar(&mut self) -> Parsed<CalendarDecl> {
        let start = self.expect("calendar")?.span.start;
        let name = self.ident()?;
        self.expect("through")?;
        let through = self.duration()?;
        let on_close = if self.take("on_close") {
            // Behavior is a semantic choice, including recognized unsupported
            // preemption modes; do not reject it as an identifier keyword.
            Some(self.label()?)
        } else {
            None
        };
        self.expect("{")?;
        let mut intervals = Vec::new();
        self.separators();
        while !self.at("}") {
            let start = self.expect("[")?.span.start;
            let from = self.duration()?;
            self.expect(",")?;
            let until = self.duration()?;
            self.expect(")")?;
            self.expect("capacity")?;
            let capacity = self.integer()?;
            intervals.push(CalendarInterval {
                from,
                until,
                capacity,
                span: self.span(start),
            });
            self.separators();
        }
        self.expect("}")?;
        Ok(CalendarDecl {
            name,
            through,
            on_close,
            intervals,
            span: self.span(start),
        })
    }

    fn compare_error(&self, message: &str) -> Diagnostic {
        let mut d = self.unsupported(message);
        d.message = message.into();
        d
    }

    fn compare_decl(&mut self) -> Parsed<CompareDecl> {
        use crate::ir::{Direction, HardComparison, OverallObjective};
        let start = self.expect("compare")?.span.start;
        self.expect("[")?;
        if self.at("]") {
            return Err(self.error("plans require at least two distinct ASCII identifiers"));
        }
        let plans = self.names("]")?;
        let names: Vec<_> = plans.iter().map(|p| p.value.as_str()).collect();
        if !crate::compare::valid_plans(&names) {
            return Err(self.error("plans require at least two distinct ASCII identifiers"));
        }
        self.separators();
        let under = if self.take("under") {
            self.expect("[")?;
            let names = self.names("]")?;
            if names.is_empty() {
                return Err(self.error("expected scenario"));
            }
            Some(names)
        } else {
            None
        };
        self.separators();
        // A following model-level Cut declaration is not a compare cutref.
        let next: Vec<_> = self.tokens[self.pos..]
            .iter()
            .filter(|t| t.text != "\n")
            .take(3)
            .collect();
        let cut_declaration = next.first().is_some_and(|t| t.text == "cut")
            && next.get(2).is_some_and(|t| t.text == "{");
        let cut = if !cut_declaration && self.take("cut") {
            Some(self.ident()?)
        } else {
            None
        };
        self.separators();
        let mut hard = Vec::new();
        if self.take("hard") {
            self.expect("{")?;
            self.separators();
            if self.at("}") {
                return Err(self.error("expected hard constraint"));
            }
            while !self.at("}") {
                let row_start = self.start();
                if !self.take("cohort") {
                    return Err(self.compare_error("hard constraints apply to cohort ratios only"));
                }
                self.expect(".")?;
                let cohort = self.ident()?;
                self.lines();
                let op = if self.take_adjacent("<", "=") {
                    HardComparison::Le
                } else if self.take_adjacent(">", "=") {
                    HardComparison::Ge
                } else if self.take("<") {
                    HardComparison::Lt
                } else if self.take(">") {
                    HardComparison::Gt
                } else {
                    return Err(self.error("expected comparison"));
                };
                let bound = self.probability()?;
                hard.push(HardRow {
                    cohort,
                    op,
                    bound,
                    span: self.span(row_start),
                });
                if matches!(self.current().text, "weight" | "score") {
                    return Err(self.compare_error("weighted scores are unsupported"));
                }
                if !matches!(self.current().text, "\n" | ";" | "}") {
                    return Err(self.error("expected hard row separator"));
                }
                self.separators();
            }
            self.expect("}")?;
        }
        self.separators();
        let mut objectives = Vec::new();
        if self.take("objectives") {
            self.expect("{")?;
            self.separators();
            if self.at("}") {
                return Err(self.error("expected objective"));
            }
            while !self.at("}") {
                let row_start = self.start();
                let direction = if self.take("minimize") {
                    Direction::Minimize
                } else if self.take("maximize") {
                    Direction::Maximize
                } else {
                    return Err(self.compare_error("objectives need minimize or maximize"));
                };
                self.lines();
                let target = if self.take("cohort") {
                    self.expect(".")?;
                    ObjectiveRef::Cohort(self.ident()?)
                } else if self.take("quantile") {
                    self.expect(".")?;
                    ObjectiveRef::Quantile(self.ident()?)
                } else if self.take("metrics") {
                    self.expect(".")?;
                    if !self.take("overall") {
                        return Err(self.compare_error("unsupported compare objective"));
                    }
                    self.expect(".")?;
                    let field = match self.current().text {
                        "work" => OverallObjective::Work,
                        "named_people_work" => OverallObjective::NamedPeopleWork,
                        "peak_tasks" => OverallObjective::PeakTasks,
                        "observed_cost_total" => OverallObjective::ObservedCostTotal,
                        "observed_cost_per_arrival" => OverallObjective::ObservedCostPerArrival,
                        _ => return Err(self.compare_error("unsupported compare objective")),
                    };
                    self.pos += 1;
                    ObjectiveRef::Overall(field)
                } else {
                    return Err(self.compare_error("unsupported compare objective"));
                };
                objectives.push(Objective {
                    direction,
                    target,
                    span: self.span(row_start),
                });
                if matches!(self.current().text, "weight" | "score") {
                    return Err(self.compare_error("weighted scores are unsupported"));
                }
                if !matches!(self.current().text, "\n" | ";" | "}") {
                    return Err(self.error("expected objective row separator"));
                }
                self.separators();
            }
            self.expect("}")?;
        }
        self.separators();
        match self.current().text {
            "report" => return Err(self.compare_error("compare report lists are unsupported")),
            "weight" | "score" => return Err(self.compare_error("weighted scores are unsupported")),
            _ => (),
        }
        Ok(CompareDecl {
            plans,
            under,
            cut,
            hard,
            objectives,
            span: self.span(start),
        })
    }

    fn arrivals(&mut self) -> Parsed<Item> {
        let start = self.expect("arrivals")?.span.start;
        if self.take("distribution") {
            let basis = if self.take("basis") {
                Some(self.string()?)
            } else {
                None
            };
            self.expect("{")?;
            let mut entries = Vec::new();
            self.separators();
            while !self.at("}") {
                let row_start = self.start();
                let probability = self.probability()?;
                self.expect(":")?;
                self.expect_choice("fixed")?;
                self.expect("[")?;
                let mut times = Vec::new();
                if !self.at("]") {
                    times.push(self.discrete_duration()?);
                    while self.take(",") {
                        times.push(self.discrete_duration()?);
                    }
                }
                self.expect("]")?;
                entries.push(ArrivalSchedule {
                    probability,
                    times,
                    span: self.span(row_start),
                });
                self.separators();
            }
            self.expect("}")?;
            Ok(Item::ArrivalsDistribution(ArrivalDistribution {
                basis,
                entries,
                span: self.span(start),
            }))
        } else if self.take("fixed_every") {
            if self.at("during") {
                return Err(self.error("expected duration"));
            }
            let every = self.discrete_duration()?;
            self.expect("during")?;
            if self.at("(") {
                return Err(self.unsupported("open arrival interval"));
            }
            self.expect("[")?;
            let from = self.discrete_duration()?;
            self.expect(",")?;
            let until = self.discrete_duration()?;
            if self.at("]") {
                return Err(self.unsupported("closed arrival interval"));
            }
            self.expect(")")?;
            Ok(Item::ArrivalsEvery(Spanned {
                value: FixedEveryDecl { every, from, until },
                span: self.span(start),
            }))
        } else {
            self.expect_choice("fixed")?;
            self.expect("[")?;
            let mut value = vec![self.discrete_duration()?];
            while self.take(",") {
                value.push(self.discrete_duration()?);
            }
            if matches!(self.current().text, "+" | "-" | "*" | "/") {
                return Err(self.unsupported("arrival time arithmetic"));
            }
            self.expect("]")?;
            Ok(Item::ArrivalsFixed(Spanned {
                value,
                span: self.span(start),
            }))
        }
    }

    fn priority_tie(&mut self) -> Parsed<crate::ir::QueueTie> {
        let mut tie = crate::ir::QueueTie::Nondet;
        if self.take("tie") && !self.take("nondet") {
            self.expect_choice("fifo")?;
            self.expect("tie_by")?;
            self.expect_choice("case_id")?;
            tie = crate::ir::QueueTie::FifoCaseId;
        }
        if self.at("tie") {
            return Err(Diagnostic {
                code: diag::U_SEM_0.into(),
                severity: diag::Severity::Error,
                message: "duplicate priority tie".into(),
                span: Some(self.current().span),
            });
        }
        Ok(tie)
    }

    fn priority_incompatible(&self) -> Diagnostic {
        Diagnostic {
            code: diag::U_SEM_0.into(),
            severity: diag::Severity::Error,
            message: "priority_tie_incompatible".into(),
            span: Some(self.current().span),
        }
    }

    fn follows_words(&self, words: &[&str]) -> bool {
        self.tokens
            .iter()
            .skip(self.pos)
            .filter(|t| t.text != "\n")
            .map(|t| t.text)
            .take(words.len())
            .eq(words.iter().copied())
    }

    fn start_policy(&mut self) -> Parsed<Spanned<StartPolicy>> {
        let start = self.expect("policy")?.span.start;
        let observable = self.take("observable");
        let new_priority = self.follows_words(&["priority", "descending"]);
        let priority = if new_priority {
            self.priority_declaration = true;
            self.expect("priority")?;
            self.expect_choice("descending")?;
            Some(self.priority_tie()?)
        } else {
            self.expect_choice("fifo")?;
            self.expect("tie_by")?;
            self.expect_choice("case_id")?;
            None
        };
        let mut extensions = Vec::new();
        let mut assigned = false;
        while self.at("then") || self.at("assign") {
            let token = self.current();
            if priority.is_some() && assigned && self.at("then") {
                return Err(self.priority_incompatible());
            }
            let value = if self.take("then") {
                self.expect_choice("call_order")?;
                PolicyExtension::CallOrder
            } else {
                self.expect("assign")?;
                self.expect_choice("first_idle")?;
                assigned = true;
                PolicyExtension::FirstIdle
            };
            extensions.push(Spanned {
                value,
                span: self.span(token.span.start),
            });
        }
        if priority.is_some() && self.at("tie") {
            return Err(self.priority_incompatible());
        }
        Ok(Spanned {
            value: StartPolicy {
                priority,
                observable,
                extensions,
            },
            span: self.span(start),
        })
    }

    /// `queue capacity N` / `concurrent cases N` with `on_overflow` (ADR 0039).
    fn limit(&mut self, word: &str) -> Parsed<Spanned<AdmissionQueue>> {
        let start = self.expect(word)?.span.start;
        self.expect(if word == "queue" { "capacity" } else { "cases" })?;
        let capacity = self.integer()?;
        self.expect("on_overflow")?;
        let overflow = if self.take("reject") {
            Overflow::Reject
        } else if self.take("wait_external") {
            if self.follows_words(&["priority", "descending"]) {
                self.priority_declaration = true;
                self.expect("priority")?;
                self.expect_choice("descending")?;
                Overflow::WaitExternalPriority(self.priority_tie()?)
            } else if self.take("fifo") {
                Overflow::WaitExternalFifo
            } else {
                let mut d = self.unsupported("external wait requires fifo order");
                d.message = "external wait requires fifo order".into();
                return Err(d);
            }
        } else if self.take("end") {
            Overflow::End(self.ident()?)
        } else {
            return Err(self.choice_error("expected reject"));
        };
        if matches!(overflow, Overflow::WaitExternalPriority(_))
            && matches!(self.current().text, "then" | "assign")
        {
            return Err(self.priority_incompatible());
        }
        Ok(Spanned {
            value: AdmissionQueue { capacity, overflow },
            span: self.span(start),
        })
    }

    fn scenario(&mut self) -> Parsed<ScenarioDecl> {
        let start = self.expect("scenario")?.span.start;
        let name = self.ident()?;
        self.expect("{")?;
        let mut items = Vec::new();
        self.separators();
        while !self.at("}") {
            items.push(match self.current().text {
                "arrivals" => match self.arrivals()? {
                    Item::ArrivalsFixed(a) => ScenarioItem::ArrivalsFixed(a),
                    Item::ArrivalsEvery(a) => ScenarioItem::ArrivalsEvery(a),
                    Item::ArrivalsDistribution(a) => ScenarioItem::ArrivalsDistribution(a),
                    _ => unreachable!(),
                },
                "bounds" => ScenarioItem::Bounds(self.bounds()?),
                "simulation" => ScenarioItem::Simulation(self.simulation()?),
                "horizon" => {
                    self.expect("horizon")?;
                    ScenarioItem::Horizon(self.duration()?)
                }
                "policy" => ScenarioItem::Policy(self.start_policy()?),
                // ADR 0042: the existing item form (ADR 0039), shared with the
                // model-level arm. A scenario-local `end` terminal is refused
                // by lower (U10), not by the parser.
                "queue" => ScenarioItem::Queue(self.limit("queue")?),
                "concurrent" => ScenarioItem::Concurrent(self.limit("concurrent")?),
                "compare"
                    if self
                        .tokens
                        .iter()
                        .skip(self.pos + 1)
                        .find(|t| t.text != "\n")
                        .is_some_and(|t| t.text == "[") =>
                {
                    return Err(self.unsupported("compare"));
                }
                _ => return Err(self.unsupported("scenario item")),
            });
            self.separators();
        }
        self.expect("}")?;
        Ok(ScenarioDecl {
            name,
            items,
            span: self.span(start),
        })
    }

    fn responsibility_cond(&mut self) -> Parsed<Spanned<ResponsibilityCond>> {
        self.enter()?;
        let start = self.start();
        let first = self.responsibility_atom()?;
        self.lines();
        let operator = self.current().text;
        let value = if operator == "->" {
            self.expect("->")?;
            ResponsibilityCond::Implies(Box::new(first), Box::new(self.responsibility_atom()?))
        } else if matches!(operator, "&&" | "||") {
            let mut parts = vec![first];
            while self.take(operator) {
                parts.push(self.responsibility_atom()?);
            }
            if self.at("&&") || self.at("||") {
                return Err(self.error("mixed && and || require parentheses"));
            }
            if operator == "&&" {
                ResponsibilityCond::And(parts)
            } else {
                ResponsibilityCond::Or(parts)
            }
        } else {
            first.value
        };
        if self.at("->") || self.at("&&") || self.at("||") {
            return Err(self.unsupported("responsibility operators require explicit grouping"));
        }
        if self.at(">")
            || self.at("<")
            || self.at("!")
            || self.at("=")
            || self.at("(")
            || self.at(".")
        {
            return Err(self.unsupported("responsibility target expression"));
        }
        self.depth -= 1;
        Ok(Spanned {
            value,
            span: self.span(start),
        })
    }

    fn responsibility_atom(&mut self) -> Parsed<Spanned<ResponsibilityCond>> {
        if self.at("end") {
            return Err(self.unsupported("end is not a state expression"));
        }
        let start = self.start();
        if self.at("!") {
            return Err(self.unsupported("responsibility negation"));
        }
        let saved = self.pos;
        let value = if self.take("(") {
            let c = self.responsibility_cond()?;
            self.expect(")")?;
            ResponsibilityCond::Group(Box::new(c))
        } else {
            let binding = self.ident()?;
            if binding.value == "executor" && self.at(".") {
                return Err(self.unsupported("executor state expression"));
            }
            let variable = if self.take(".") {
                Some(self.ident()?)
            } else {
                None
            };
            if self.at(".") {
                return Err(self.unsupported("multi-level state path"));
            }
            if self.at("in") {
                self.pos = saved;
                let c = self.atom()?;
                return Ok(Spanned {
                    span: c.span,
                    value: ResponsibilityCond::Compare(c),
                });
            }
            // ADR 0032 accepts only `Actor.x == value` here; other qualified
            // comparisons keep the U-SYN-0 below.
            if variable.is_none() && matches!(self.current().text, "<" | ">" | "!") {
                let old_operator = self.pos;
                self.pos = saved;
                let c = self.atom()?;
                if matches!(&c.value, Cond::Compare { value, .. } if value.value.parse::<i128>().is_err())
                {
                    self.pos = old_operator;
                    return Err(self.unsupported("responsibility target expression"));
                }
                return Ok(Spanned {
                    span: c.span,
                    value: ResponsibilityCond::Compare(c),
                });
            }
            if self.at("(")
                || self.at("!")
                || self.at("!=")
                || self.at(">")
                || self.at("<")
                || self.at("=")
            {
                return Err(self.unsupported("responsibility target expression"));
            }
            if self.take("==") {
                let operator = Spanned {
                    value: IntComparison::Eq,
                    span: self.tokens[self.pos - 1].span,
                };
                let rhs = self.value_ident()?;
                let cond = match variable {
                    Some(variable) => Cond::LocalEq {
                        actor: binding,
                        variable,
                        operator,
                        value: rhs,
                    },
                    None => Cond::Eq {
                        binding,
                        value: rhs,
                    },
                };
                ResponsibilityCond::Compare(Spanned {
                    value: cond,
                    span: self.span(start),
                })
            } else if variable.is_some() {
                return Err(self.unsupported("bare local state in responsibility target"));
            } else {
                ResponsibilityCond::Bool(binding)
            }
        };
        Ok(Spanned {
            value,
            span: self.span(start),
        })
    }

    fn legacy_responsibility_cond(c: &Spanned<ResponsibilityCond>) -> Option<Spanned<Cond>> {
        let value = match &c.value {
            ResponsibilityCond::Compare(c) => return Some(c.clone()),
            ResponsibilityCond::Bool(_) | ResponsibilityCond::Implies(..) => return None,
            ResponsibilityCond::Group(c) => {
                Cond::Group(Box::new(Self::legacy_responsibility_cond(c)?))
            }
            ResponsibilityCond::And(cs) | ResponsibilityCond::Or(cs) => {
                let parts: Option<Vec<_>> =
                    cs.iter().map(Self::legacy_responsibility_cond).collect();
                if matches!(c.value, ResponsibilityCond::And(_)) {
                    Cond::And(parts?)
                } else {
                    Cond::Or(parts?)
                }
            }
        };
        Some(Spanned {
            value,
            span: c.span,
        })
    }

    fn type_decl(&mut self) -> Parsed<TypeDecl> {
        let start = self.expect("type")?.span.start;
        let name = self.ident()?;
        self.expect("=")?;
        if self.take("record") {
            self.expect("{")?;
            self.separators();
            if self.at("}") {
                return Err(self.error("record requires a field"));
            }
            let mut fields = Vec::new();
            while !self.at("}") {
                let mut names = vec![self.ident()?];
                while self.take(",") {
                    names.push(self.ident()?);
                }
                self.expect(":")?;
                self.lines();
                let (ty, inline) = if self.at("enum") {
                    let start = self.expect("enum")?.span.start;
                    self.expect("{")?;
                    let members = self.names("}")?;
                    (
                        Spanned {
                            value: "@inline".into(),
                            span: self.span(start),
                        },
                        Some(members),
                    )
                } else {
                    if self.current().kind == Kind::Integer
                        || matches!(
                            self.current().text,
                            "optional" | "PersonId" | "CaseId" | "set"
                        )
                    {
                        return Err(self.unsupported("record field type is unsupported"));
                    }
                    let ty = self.named_type().map_err(|mut d| {
                        if d.message.contains("inline record") {
                            d.message =
                                "unsupported syntax: record field type is unsupported".into();
                        }
                        d
                    })?;
                    if self.at("<") {
                        return Err(self.unsupported("nested or unknown type constructor"));
                    }
                    (ty, None)
                };
                let after_type = self.pos;
                if self.at("=") || self.at("~") {
                    return Err(
                        self.unsupported("record field default or distribution is unsupported")
                    );
                }
                self.pos = after_type;
                fields.push(RecordFieldDecl { names, ty, inline });
                let before = self.pos;
                self.separators();
                if !self.at("}") && before == self.pos {
                    return Err(self.error("expected field separator"));
                }
            }
            self.expect("}")?;
            return Ok(TypeDecl {
                name,
                members: Vec::new(),
                record: Some(fields),
                span: self.span(start),
            });
        }
        self.expect("enum")?;
        self.expect("{")?;
        let members = self.names("}")?;
        Ok(TypeDecl {
            name,
            members,
            record: None,
            span: self.span(start),
        })
    }

    fn persons(&mut self) -> Parsed<Vec<Ident>> {
        if self.take("[") {
            self.names("]")
        } else {
            Ok(vec![self.ident()?])
        }
    }

    fn capability_path(&mut self) -> Parsed<CapabilityPath> {
        let start = self.start();
        let actor = self.ident()?;
        if actor.value == "change" && !self.at(".") {
            return Err(self.unsupported("rule-change grants"));
        }
        self.expect(".")?;
        let capability = self.ident()?;
        if self.at(".") {
            return Err(self.unsupported("event suffix on Capability path"));
        }
        Ok(CapabilityPath {
            actor,
            capability,
            span: self.span(start),
        })
    }

    fn permit(&mut self) -> Parsed<PermitDecl> {
        let start = self.expect("permit")?.span.start;
        let persons = self.persons()?;
        self.expect("to")?;
        let target = self.capability_path()?;
        Ok(PermitDecl {
            persons,
            target,
            span: self.span(start),
        })
    }

    fn authority(&mut self) -> Parsed<AuthorityDecl> {
        let start = self.expect("authority")?.span.start;
        let name = self.ident()?;
        self.expect("{")?;
        let mut items = Vec::new();
        self.separators();
        while !self.at("}") {
            let begin = self.start();
            let value = if self.take("basis") {
                self.lines();
                let token = self.current();
                if token.kind != Kind::String {
                    return Err(self.error("expected basis string"));
                }
                self.pos += 1;
                AuthorityItem::Basis(Spanned {
                    value: token.text[1..token.text.len() - 1].into(),
                    span: token.span,
                })
            } else if self.take("grants") {
                let persons = self.persons()?;
                self.expect("to")?;
                if self.at("change")
                    && self.tokens[self.pos + 1..]
                        .iter()
                        .find(|t| t.text != "\n")
                        .is_none_or(|t| t.text != ".")
                {
                    self.expect("change")?;
                    AuthorityItem::ChangeGrants {
                        persons,
                        rule: self.ident()?,
                    }
                } else {
                    AuthorityItem::Grants {
                        persons,
                        target: self.capability_path()?,
                    }
                }
            } else if self.at("under") {
                let error = self.unsupported("under");
                self.expect("under")?;
                let parsed = (|| {
                    Ok(AuthorityItem::Under {
                        rule: self.ident()?,
                        version: self.string()?,
                    })
                })();
                parsed.map_err(|_: Diagnostic| error)?
            } else if self.take("issued_by") {
                AuthorityItem::IssuedBy(self.ident()?)
            } else if self.take("not_by") {
                self.expect("[")?;
                AuthorityItem::NotBy(self.names("]")?)
            } else if self.take("scope") {
                AuthorityItem::Scope(self.cond()?)
            } else if self.take("expires") {
                AuthorityItem::Expires(self.duration()?)
            } else if self.take("revocable_by") {
                AuthorityItem::RevocableBy(self.ident()?)
            } else if self.take("delegated_from") {
                AuthorityItem::DelegatedFrom(self.ident()?)
            } else {
                return Err(self.choice_error("expected Authority item"));
            };
            items.push(Spanned {
                value,
                span: self.span(begin),
            });
            self.separators();
        }
        self.expect("}")?;
        Ok(AuthorityDecl {
            name,
            items,
            span: self.span(start),
        })
    }

    fn instance_item(&mut self) -> Parsed<ActorItem> {
        let start = self.expect("instance")?.span.start;
        if self.at(";") || self.at("}") || self.current().kind == Kind::Eof {
            return Err(self.error("expected instance value"));
        }
        if !self.at("per_case") {
            return Err(self.unsupported("Actor instance"));
        }
        self.expect("per_case")?;
        Ok(ActorItem::Instance(Spanned {
            value: ActorInstance::PerCase,
            span: self.span(start),
        }))
    }

    fn actor(&mut self) -> Parsed<Actor> {
        let start = self.expect("actor")?.span.start;
        let name = self.ident()?;
        self.expect("{")?;
        let mut items = Vec::new();
        let mut version_error = None;
        let mut decides = false;
        let result = (|| {
            self.separators();
            while !self.at("}") {
                items.push(if self.take("calendar") {
                    ActorItem::Calendar(self.ident()?)
                } else if self.at("version") {
                    if version_error.is_none() {
                        version_error = Some(self.unsupported("version"));
                    }
                    self.expect("version")?;
                    ActorItem::Version(self.string()?)
                } else if self.at("decides") {
                    let error = self.unsupported("decides");
                    self.expect("decides")?;
                    let parsed = self.var_decl_mode(true);
                    let d = parsed.map_err(|d| {
                        if d.message.contains("state default expression") {
                            self.unsupported("conditional rule value")
                        } else if d.message
                            == "unsupported syntax: inline enum position is unsupported"
                        {
                            // This is a recognized Rule declaration with an
                            // unsupported type position, not old malformed
                            // decides syntax. Preserve its inner token/span and
                            // do not let an earlier version mask the cause.
                            decides = true;
                            d
                        } else {
                            error.clone()
                        }
                    })?;
                    if d.names.len() != 1 {
                        return Err(error);
                    }
                    decides = true;
                    if d.integer_max.is_some() {
                        return Err(self.unsupported("integer rule value type"));
                    }
                    if self.at("last_write_wins") {
                        return Err(error);
                    }
                    if matches!(self.current().text, "==" | "&&" | "||" | ".")
                        || self.at_adjacent("!", "=")
                    {
                        return Err(self.unsupported("conditional rule value"));
                    }
                    decides = true;
                    ActorItem::Decides(d)
                } else if self.take("boundary") {
                    ActorItem::Boundary(self.string()?)
                } else if self.at("instance") {
                    self.instance_item()?
                } else if self.take("grain") {
                    ActorItem::Grain(self.label()?)
                } else if self.at("contains") || self.at("overlaps") {
                    let contains = self.take("contains");
                    if !contains {
                        self.expect("overlaps")?;
                    }
                    let start = self.tokens[self.pos - 1].span.start;
                    self.expect("[")?;
                    let value = if self.take("]") {
                        Vec::new()
                    } else {
                        self.names("]")?
                    };
                    let names = Spanned {
                        value,
                        span: self.span(start),
                    };
                    if contains {
                        ActorItem::Contains(names)
                    } else {
                        ActorItem::Overlaps(names)
                    }
                } else if self.take("via") {
                    self.expect("model")?;
                    ActorItem::ViaModel(self.string()?)
                } else if self.take("capacity") {
                    let start = self.tokens[self.pos - 1].span.start;
                    let mut capacity = self.integer()?;
                    capacity.span = self.span(start);
                    ActorItem::Capacity(capacity)
                } else if self.take("executors") {
                    let start = self.tokens[self.pos - 1].span.start;
                    if self.at("{") {
                        return Err(self.unsupported("brace executors"));
                    }
                    self.expect("[")?;
                    let value = self.names("]")?;
                    ActorItem::Executors(Spanned {
                        value,
                        span: self.span(start),
                    })
                } else if self.take("control") {
                    let start = self.tokens[self.pos - 1].span.start;
                    if self.take("party") {
                        ActorItem::ControlParty(self.span(start))
                    } else if self.at("policy") || self.at("environment") {
                        ActorItem::Control(self.label()?)
                    } else {
                        return Err(self.choice_error("expected policy, environment or party"));
                    }
                } else if self.take("standing") {
                    let start = self.tokens[self.pos - 1].span.start;
                    self.expect("over")?;
                    self.expect("[")?;
                    let mut targets = vec![self.standing_ref()?];
                    while self.take(",") {
                        targets.push(self.standing_ref()?);
                    }
                    self.expect("]")?;
                    self.expect("basis")?;
                    self.lines();
                    let token = self.current();
                    if token.kind != Kind::String {
                        return Err(self.error("expected standing basis string"));
                    }
                    self.pos += 1;
                    ActorItem::Standing {
                        targets,
                        basis: Spanned {
                            value: token.text[1..token.text.len() - 1].into(),
                            span: token.span,
                        },
                        span: self.span(start),
                    }
                } else if self.take("source") {
                    let start = self.start();
                    let value = if self.take("document") {
                        SourceKind::Document
                    } else if self.take("record") {
                        SourceKind::Record
                    } else {
                        self.expect_choice("tacit")?;
                        SourceKind::Tacit
                    };
                    self.lines();
                    let description = if self.current().kind == Kind::String {
                        if value != SourceKind::Tacit {
                            return Err(self.unsupported("source description"));
                        }
                        let token = self.string()?;
                        Some(Spanned {
                            value: token.value,
                            span: token.span,
                        })
                    } else {
                        None
                    };
                    ActorItem::Source(
                        Spanned {
                            value,
                            span: self.span(start),
                        },
                        description,
                    )
                } else if self.at("capability") {
                    ActorItem::Capability(self.capability()?)
                } else if self.take("latent") {
                    ActorItem::Latent(self.var_decl()?)
                } else if self.take("state") {
                    ActorItem::State(self.var_decl()?)
                } else {
                    return Err(self.choice_error("expected capacity, capability or closing brace"));
                });
                self.separators();
            }
            self.expect("}")?;
            Ok(())
        })();
        if !decides
            && !result.as_ref().is_err_and(|d: &Diagnostic| {
                d.message.contains("conditional rule value")
                    || d.message.contains("integer rule value type")
            })
            && let Some(error) = version_error
        {
            return Err(error);
        }
        result?;
        Ok(Actor {
            name,
            items,
            span: self.span(start),
        })
    }

    fn capability(&mut self) -> Parsed<Capability> {
        let start = self.expect("capability")?.span.start;
        let name = self.ident()?;
        let params = self.parameters()?;
        self.expect("->")?;
        if self.current().kind == Kind::Integer {
            return Err(self.unsupported("bounded integer return type"));
        }
        let (return_type, inline_result) = if self.at("enum") {
            let start = self.expect("enum")?.span.start;
            self.expect("{")?;
            let members = self.names("}")?;
            (
                Spanned {
                    value: "@inline".into(),
                    span: self.span(start),
                },
                Some(members),
            )
        } else {
            (self.named_type()?, None)
        };
        if self.at("<") {
            return Err(self.unsupported("return type constructor"));
        }
        self.expect("{")?;
        let mut items = Vec::new();
        self.separators();
        while !self.at("}") {
            let start = self.start();
            let declaration_start = self.pos;
            let value = if self.take("qualified_executors") {
                self.expect("[")?;
                CapItem::QualifiedExecutors(if self.take("]") {
                    Vec::new()
                } else {
                    self.names("]")?
                })
            } else if self.take("idempotency") {
                self.expect("key")?;
                self.lines();
                if matches!(
                    self.current().text,
                    "writes"
                        | "duration"
                        | "atomic"
                        | "result"
                        | "cancel"
                        | "commits"
                        | "finish"
                        | "start"
                        | "requires"
                        | "uses"
                        | "reads"
                        | "}"
                ) {
                    self.pos = declaration_start;
                    return Err(self.choice_error("expected capability item or closing brace"));
                }
                let key = self.ident()?;
                if self.at(".") {
                    self.pos = declaration_start;
                    return Err(self.choice_error("expected capability item or closing brace"));
                }
                CapItem::Idempotency(key)
            } else if self.take("effect_scope") {
                self.expect_choice("modeled")?;
                CapItem::EffectScopeModeled
            } else if self.take("version_check") {
                self.expect("[")?;
                let variables = self.names("]")?;
                self.expect("else")?;
                self.lines();
                if matches!(
                    self.current().text,
                    "commits"
                        | "duration"
                        | "result"
                        | "writes"
                        | "finish"
                        | "start"
                        | "requires"
                        | "atomic"
                        | "uses"
                        | "version_check"
                        | "cancel"
                        | "outcomes"
                ) {
                    return Err(self.error("expected version_check else value"));
                }
                // ADR 0038: a record literal is not opened at this position.
                self.reject_record_literal("record version_check reject is unsupported")?;
                let reject = self.value_ident()?;
                let resolve = if self.follows_words(&["resolve", "priority", "descending"]) {
                    self.priority_declaration = true;
                    self.expect("resolve")?;
                    self.expect_choice("priority")?;
                    self.expect_choice("descending")?;
                    let tie = if self.take("tie_by") {
                        self.expect_choice("case_id")?;
                        crate::ir::ResolutionTie::CaseId
                    } else {
                        if self.take("tie") {
                            self.expect_choice("nondet")?;
                        }
                        crate::ir::ResolutionTie::Nondet
                    };
                    Some(tie)
                } else {
                    None
                };
                if resolve.is_some()
                    && matches!(
                        self.current().text,
                        "then" | "assign" | "tie" | "tie_by" | "resolve"
                    )
                {
                    return Err(self.priority_incompatible());
                }
                CapItem::VersionCheck {
                    resolve,
                    variables,
                    reject,
                }
            } else if self.take("uses") {
                let mut uses = Vec::new();
                loop {
                    let start = self.start();
                    let resource = self.ident()?;
                    self.expect(":")?;
                    let amount = self.integer()?;
                    if matches!(self.current().text, "." | "+" | "-" | "*" | "/") {
                        return Err(self.unsupported("resource amount expression"));
                    }
                    uses.push(ResourceUse {
                        resource,
                        amount,
                        span: self.span(start),
                    });
                    if !self.take(",") {
                        break;
                    }
                }
                CapItem::Uses(uses)
            } else if self.take("requires") {
                let condition = self.cond()?;
                if !self.take("else") {
                    let mut d = self.unsupported("requires needs explicit else");
                    d.span = Some(Span {
                        start,
                        end: start + "requires".len(),
                    });
                    return Err(d);
                }
                CapItem::Requires {
                    condition,
                    otherwise: self.value_expr()?,
                }
            } else if self.take("start") {
                self.expect("{")?;
                self.separators();
                let mut assignments = Vec::new();
                while !self.at("}") {
                    let target = self.state_target("qualified start target", false)?;
                    let operator = self.assignment_operator()?;
                    if self.at("result") {
                        return Err(self.unsupported("start result"));
                    }
                    let value = self.effect_value()?;
                    if matches!(self.current().text, "." | "+" | "-" | "*" | "/") {
                        return Err(self.unsupported("start expression"));
                    }
                    assignments.push(Assignment {
                        target,
                        value,
                        operator,
                    });
                    let before = self.pos;
                    self.separators();
                    if !self.at("}") && before == self.pos {
                        return Err(self.error("expected assignment separator"));
                    }
                }
                self.expect("}")?;
                CapItem::Start(assignments)
            } else if self.take("outcomes") {
                if self.take("performance") {
                    CapItem::OutcomesPerformance(self.ident()?)
                } else if self.take("kernel") {
                    CapItem::OutcomesKernel(self.ident()?)
                } else {
                    return Err(self.unsupported("outcomes without kernel"));
                }
            } else if self.take("kernel_reads") {
                self.expect("latent")?;
                CapItem::KernelReadsLatent(self.state_refs()?)
            } else if self.take("writes") {
                let bracket = self.take("[");
                let mut names = vec![self.state_target("qualified writes", false)?];
                if bracket {
                    while self.take(",") {
                        names.push(self.state_target("qualified writes", false)?);
                    }
                    self.expect("]")?;
                }
                CapItem::Writes(names)
            } else if self.take("finish") {
                self.expect("{")?;
                self.separators();
                let mut assignments = Vec::new();
                while !self.at("}") {
                    let target = self.state_target("qualified finish target", true)?;
                    let operator = self.assignment_operator()?;
                    let value = self.effect_value()?;
                    if matches!(self.current().text, "." | "+" | "-" | "*" | "/") {
                        return Err(self.unsupported("finish expression"));
                    }
                    assignments.push(Assignment {
                        target,
                        value,
                        operator,
                    });
                    let before = self.pos;
                    self.separators();
                    if !self.at("}") && before == self.pos {
                        return Err(self.error("expected assignment separator"));
                    }
                }
                self.expect("}")?;
                CapItem::Finish(assignments)
            } else if self.take("atomic") {
                self.expect("{")?;
                self.expect_choice("if")?;
                let condition = self.cond()?;
                let then_branch = self.atomic_branch()?;
                self.expect("else")?;
                if self.at("if") {
                    return Err(self.unsupported("atomic else if"));
                }
                let else_branch = self.atomic_branch()?;
                self.expect("}")?;
                CapItem::Atomic {
                    condition,
                    then_branch,
                    else_branch,
                }
            } else if self.take("reads") {
                self.lines();
                if self.current().kind == Kind::Word
                    && !keyword(self.current().text)
                    && !outside_keyword(self.current().text)
                    && !matches!(
                        self.current().text,
                        "source"
                            | "document"
                            | "record"
                            | "tacit"
                            | "reads"
                            | "commits"
                            | "cut"
                            | "include"
                            | "exclude"
                            | "defer"
                            | "question"
                            | "reason"
                            | "loses"
                            | "horizon"
                            | "externality"
                            | "reentry"
                            | "intent"
                            | "add"
                            | "retire"
                            | "keep"
                            | "fallback"
                            | "preserve"
                            | "override"
                            | "maintain"
                            | "from"
                            | "to"
                    )
                {
                    return Err(self.unsupported("unbracketed reads"));
                }
                self.expect("[")?;
                let mut names = Vec::new();
                loop {
                    let path = self.path()?;
                    names.push(Spanned {
                        value: path
                            .value
                            .iter()
                            .map(|p| p.value.as_str())
                            .collect::<Vec<_>>()
                            .join("."),
                        span: path.span,
                    });
                    // Pre-C5 order (names("]")): `,` continues, else `]` is
                    // expected, so `reads [Doc PO]` keeps "expected ]".
                    if !self.take(",") {
                        self.expect("]")?;
                        break;
                    }
                }
                CapItem::Reads(names)
            } else if self.take("receipt_query") {
                let target = self.ident().map_err(|_| self.recovery_error())?;
                self.expect("key").map_err(|_| self.recovery_error())?;
                let key = self.ident().map_err(|_| self.recovery_error())?;
                self.expect("availability")
                    .map_err(|_| self.recovery_error())?;
                let availability = if self.take("available") {
                    QueryAvailability::Available
                } else if self.take("unavailable") {
                    QueryAvailability::Unavailable
                } else {
                    self.expect("nondet").map_err(|_| self.recovery_error())?;
                    QueryAvailability::Nondet
                };
                CapItem::ReceiptQuery {
                    target,
                    key,
                    availability,
                }
            } else if self.take("commits") {
                CapItem::Commits
            } else if self.take("duration") {
                if self.at("distribution")
                    && (self.performance_source_ahead()
                        || items.iter().any(|i: &Spanned<CapItem>| {
                            matches!(i.value, CapItem::OutcomesPerformance(_))
                        }))
                {
                    let mut error = self
                        .unsupported("q6_feature_combination: performance duration distribution");
                    error.code = diag::U_SEM_0.into();
                    return Err(error);
                }
                if self.take("{") {
                    let mut entries = Vec::new();
                    loop {
                        let start = self.start();
                        let duration = self.discrete_duration()?;
                        self.expect(":")?;
                        let probability = self.probability()?;
                        entries.push(DurationEntry {
                            duration,
                            probability,
                            span: self.span(start),
                        });
                        if !self.take(",") {
                            break;
                        }
                    }
                    self.expect("}")?;
                    CapItem::DurationDistribution(entries)
                } else {
                    self.expect_choice("fixed")?;
                    CapItem::DurationFixed(self.duration()?)
                }
            } else if self.take("result") {
                self.expect_choice("nondet")?;
                self.expect("{")?;
                if self.record_names().is_empty() {
                    // Without a record declaration, the legacy name list.
                    CapItem::ResultNondet(self.names("}")?.into_iter().map(Into::into).collect())
                } else {
                    let mut values = vec![self.value_expr()?];
                    while self.take(",") {
                        values.push(self.value_expr()?);
                    }
                    self.expect("}")?;
                    CapItem::ResultNondet(values)
                }
            } else if self.take("cancel") {
                let cancel = if self.take("immediate") {
                    Cancel::Immediate
                } else if self.take("cooperative") {
                    self.expect("ack_within")?;
                    Cancel::Cooperative {
                        ack_within: self.duration()?,
                    }
                } else {
                    self.expect_choice("forbidden_after_start")?;
                    Cancel::ForbiddenAfterStart
                };
                CapItem::Cancel(cancel)
            } else {
                return Err(self.choice_error("expected capability item or closing brace"));
            };
            items.push(Spanned {
                value,
                span: self.span(start),
            });
            self.separators();
        }
        self.expect("}")?;
        Ok(Capability {
            name,
            params,
            return_type,
            inline_result,
            items,
            span: self.span(start),
        })
    }

    fn process(&mut self) -> Parsed<Process> {
        let start = self.expect("process")?.span.start;
        let name = self.ident()?;
        let body = self.block()?;
        Ok(Process {
            name,
            degradations: std::mem::take(&mut self.degradations),
            body,
            span: self.span(start),
        })
    }

    /// Start/finish source: ADR 0035 `executor.id`, otherwise the ADR 0038 value
    /// expression (a record literal only when the model declares a record).
    fn effect_value(&mut self) -> Parsed<ValueExpr> {
        if self.at("{") {
            return Err(self.unsupported("unsupported set operation"));
        }
        let saved = self.pos;
        let value = self.effect_ident()?;
        if value.value == "executor.id" {
            return Ok(value.into());
        }
        self.pos = saved;
        self.value_expr()
    }

    fn effect_ident(&mut self) -> Parsed<Ident> {
        if self.at("{") {
            return Err(self.unsupported("unsupported set operation"));
        }
        let mut value = self.value_ident()?;
        let saved = self.pos;
        if value.value == "executor" && self.take(".") && self.take("id") {
            value.value = "executor.id".into();
            value.span = self.span(value.span.start);
        } else {
            // Preserve the old scalar parser and its phase-specific diagnostic
            // for every spelling other than the new literal executor.id.
            self.pos = saved;
        }
        Ok(value)
    }

    fn atomic_branch(&mut self) -> Parsed<AtomicBranch> {
        self.expect("{")?;
        self.separators();
        let mut assignments = Vec::new();
        while !self.at("result") {
            if self.at("}") {
                return Err(self.error("expected result"));
            }
            let nested_atomic = self.current().text == "atomic"
                && self.tokens[self.pos + 1..]
                    .iter()
                    .find(|t| t.text != "\n")
                    .is_some_and(|t| t.text == "{");
            if matches!(self.current().text, "if" | "else") || nested_atomic {
                return Err(self.unsupported("nested atomic branch"));
            }
            let target = self.state_target("atomic target", true)?;
            let operator = self.assignment_operator()?;
            let value = self.effect_ident()?;
            if matches!(self.current().text, "." | "+" | "-" | "*" | "/" | "{") {
                return Err(self.unsupported("atomic expression"));
            }
            assignments.push(Assignment {
                operator,
                target,
                value: value.into(),
            });
            let before = self.pos;
            self.separators();
            if before == self.pos && !self.at("}") {
                return Err(self.error("expected assignment separator"));
            }
        }
        self.expect("result")?;
        if self.at("nondet") {
            return Err(self.unsupported("atomic result nondet"));
        }
        let result = self.value_ident()?;
        self.separators();
        if !self.at("}") {
            return Err(self.unsupported("atomic statement after result"));
        }
        self.expect("}")?;
        Ok(AtomicBranch {
            assignments,
            result,
        })
    }

    // Guard recursive blocks/conditions before the Rust stack can overflow.
    // This is an explicit implementation limit, not a model execution bound.
    fn enter(&mut self) -> Parsed<()> {
        if self.depth >= 128 {
            let mut diagnostic = self.error("syntax nesting limit exceeded");
            diagnostic.code = diag::U_SYN_0.into();
            return Err(diagnostic);
        }
        self.depth += 1;
        Ok(())
    }

    fn block(&mut self) -> Parsed<Block> {
        self.enter()?;
        self.expect("{")?;
        let mut body = Vec::new();
        self.separators();
        while !self.at("}") {
            body.push(self.stmt()?);
            self.separators();
        }
        self.expect("}")?;
        self.depth -= 1;
        Ok(body)
    }

    fn stmt(&mut self) -> Parsed<Spanned<Stmt>> {
        if self.at("repeat") {
            let mut ts = self.tokens[self.pos + 1..]
                .iter()
                .filter(|t| t.text != "\n");
            if ts
                .next()
                .is_some_and(|t| t.kind == Kind::Word && t.text != "=")
                && ts
                    .next()
                    .is_some_and(|t| matches!(t.text, "at_most" | "{" | "gap" | "over"))
            {
                return self.repeat_statement();
            }
        }
        if self.repeat_depth > 0 {
            for (word, value) in [("again", Stmt::Again), ("leave", Stmt::Leave)] {
                if self.position_word(word) {
                    let start = self.start();
                    self.expect(word)?;
                    return Ok(Spanned {
                        value,
                        span: self.span(start),
                    });
                }
            }
            if ["fork", "degrade"].iter().any(|w| self.position_word(w)) {
                return Err(self.loop_combination());
            }
        }
        if self.at("if") {
            return self.if_statement();
        }
        if self.recovery_depth > 0
            && [
                "resolve", "stop", "fork", "cancel", "degrade", "retry", "on",
            ]
            .iter()
            .any(|w| self.position_word(w))
        {
            return self.recovery_statement();
        }
        self.ordinary_statement()
    }

    fn loop_combination(&self) -> Diagnostic {
        let mut d = self.unsupported("loop_feature_combination");
        d.code = diag::U_SEM_0.into();
        d
    }

    fn repeat_statement(&mut self) -> Parsed<Spanned<Stmt>> {
        if self.repeat_depth > 0 || self.recovery_depth > 0 || self.degradation_depth > 0 {
            // Inspect the header only, excluding the contextual loop name.
            // Preserve the former combination diagnostic for every non-over
            // nested shape, including a malformed/missing mandatory header.
            let over = self.tokens[self.pos..]
                .iter()
                .filter(|t| t.text != "\n")
                .skip(2)
                .take_while(|t| !matches!(t.text, "{" | ";" | "}"))
                .any(|t| t.kind == Kind::Word && t.text == "over");
            let mut d = self.loop_combination();
            if over {
                d.message = "loop_stage_unsupported".into();
            }
            return Err(d);
        }
        let start = self.expect("repeat")?.span.start;
        let name = self.ident()?;
        if self.at("over") {
            let mut d = self.loop_combination();
            d.message = "loop_stage_unsupported".into();
            return Err(d);
        }
        self.expect("at_most")?;
        let at_most = self.integer()?;
        if self.at("over") {
            let mut d = self.loop_combination();
            d.message = "loop_stage_unsupported".into();
            return Err(d);
        }
        let gap = if self.take("gap") {
            self.expect("fixed")?;
            Some(self.duration()?)
        } else {
            None
        };
        if self.at("over") {
            let mut d = self.loop_combination();
            d.message = "loop_stage_unsupported".into();
            return Err(d);
        }
        self.repeat_depth += 1;
        let body = self.block()?;
        self.separators();
        self.expect("on_limit")?;
        let on_limit = self.block()?;
        self.repeat_depth -= 1;
        Ok(Spanned {
            value: Stmt::Repeat {
                name,
                at_most,
                gap,
                body,
                on_limit,
            },
            span: self.span(start),
        })
    }

    // Separate recursive If and recovery-only frames preserve the existing
    // depth limit on the default unoptimized test stack.
    fn if_statement(&mut self) -> Parsed<Spanned<Stmt>> {
        let start = self.expect("if")?.span.start;
        let condition = self.cond()?;
        let then_body = self.block()?;
        self.separators();
        self.expect("else")?;
        let else_body = self.block()?;
        Ok(Spanned {
            value: Stmt::If {
                condition,
                then_body,
                else_body,
            },
            span: self.span(start),
        })
    }

    fn recovery_statement(&mut self) -> Parsed<Spanned<Stmt>> {
        if self.position_word("resolve") {
            let start = self.start();
            self.expect("resolve")?;
            self.expect("effect").map_err(|_| self.recovery_error())?;
            let command = self.ident().map_err(|_| self.recovery_error())?;
            self.expect("using").map_err(|_| self.recovery_error())?;
            let evidence = self.ident().map_err(|_| self.recovery_error())?;
            return Ok(Spanned {
                value: Stmt::ResolveEffect { command, evidence },
                span: self.span(start),
            });
        }
        if self.position_word("stop") {
            let start = self.start();
            self.expect("stop")?;
            self.expect("receipt_unknown")
                .map_err(|_| self.recovery_error())?;
            return Ok(Spanned {
                value: Stmt::StopReceiptUnknown,
                span: self.span(start),
            });
        }
        Err(self.unsupported("unsupported recovery statement"))
    }

    fn ordinary_statement(&mut self) -> Parsed<Spanned<Stmt>> {
        if self.at("compare")
            && self
                .tokens
                .iter()
                .skip(self.pos + 1)
                .find(|t| t.text != "\n")
                .is_some_and(|t| t.text == "[")
        {
            return Err(self.unsupported("compare"));
        }
        if matches!(self.current().text, "property" | "metric")
            && self
                .tokens
                .iter()
                .skip(self.pos + 1)
                .find(|t| t.text != "\n")
                .is_some_and(|t| matches!(t.text, "case" | "cohort" | "quantile"))
        {
            return Err(self.unsupported("quality declarations are only supported at model scope"));
        }
        let start = self.start();
        if self.current().text == "metrics"
            && self
                .tokens
                .iter()
                .skip(self.pos + 1)
                .find(|t| t.text != "\n")
                .is_some_and(|t| t.text == "{")
        {
            return Err(self.unsupported("metrics block is only supported at model scope"));
        }
        if self.is_degradation_statement() {
            return self.degradation_stmt(start);
        }
        let wait_statement = self.at("wait")
            && self
                .tokens
                .iter()
                .skip(self.pos + 1)
                .find(|t| t.text != "\n")
                .is_none_or(|t| t.text != "=");
        let value = if wait_statement {
            if self.degradation_depth > 0 {
                return self.degradation_wait_error();
            }
            self.take("wait");
            self.lines();
            if self.take("until") {
                if self.repeat_depth > 0 {
                    return Err(self.loop_combination());
                }
                if self.recovery_depth > 0 {
                    return Err(self.unsupported("condition wait in recovery"));
                }
                Stmt::WaitUntil(self.cond()?)
            } else {
                if self.current().kind == Kind::Word {
                    return Err(self.unsupported("nonconstant wait"));
                }
                Stmt::Wait(self.duration()?)
            }
        } else if self.take("fork") {
            let name = self.ident()?;
            self.expect("{")?;
            self.separators();
            let mut branches = vec![self.call()?];
            self.separators();
            while !self.at("}") {
                branches.push(self.call()?);
                self.separators();
            }
            self.expect("}")?;
            self.separators();
            let join = self.join()?;
            Stmt::Fork {
                name,
                branches,
                join,
            }
        } else if self.take("end") {
            Stmt::End(self.ident()?)
        } else {
            Stmt::Call(self.call()?)
        };
        Ok(Spanned {
            value,
            span: self.span(start),
        })
    }

    // Keep new syntax's owned locals out of the recursive ordinary-stmt frame.
    // The existing nesting limit (128) and default test stack remain unchanged.
    fn is_degradation_statement(&mut self) -> bool {
        self.at("degrade")
            && self
                .tokens
                .iter()
                .skip(self.pos + 1)
                .find(|t| t.text != "\n")
                .is_some_and(|t| t.text != "=")
    }

    fn degradation_wait_error(&mut self) -> Parsed<Spanned<Stmt>> {
        let mut d = self.unsupported("q6_feature_combination: wait in degradation");
        d.code = diag::U_SEM_0.into();
        Err(d)
    }

    fn degradation_stmt(&mut self, start: usize) -> Parsed<Spanned<Stmt>> {
        self.expect("degrade")?;
        if self.depth != 1 {
            let mut d = self.unsupported("q6_feature_combination: nested degradation");
            d.code = diag::U_SEM_0.into();
            return Err(d);
        }
        let name = self.ident()?;
        self.expect("when")?;
        let condition = self.cond()?;
        self.degradation_depth += 1;
        let then_body = self.block()?;
        self.separators();
        self.expect("else")?;
        let else_body = self.block()?;
        self.degradation_depth -= 1;
        self.degradations.push(Degradation {
            name,
            span: self.span(start),
        });
        Ok(Spanned {
            value: Stmt::If {
                condition,
                then_body,
                else_body,
            },
            span: self.span(start),
        })
    }

    fn call(&mut self) -> Parsed<Call> {
        let start = self.start();
        let binding = self.ident()?;
        // `word word` cannot start a call: the first word is a statement
        // keyword outside ADR 0001 (an unknown statement kind). `a A.c()` is
        // a call missing `=`, so it stays a structural error.
        // Newlines may separate any two tokens of a call, so the lookahead
        // skips them too: classification must not depend on line breaks.
        let keyword = self.pos - 1;
        self.lines();
        let next = self.tokens[self.pos + 1..]
            .iter()
            .find(|token| token.text != "\n")
            .map_or("", |token| token.text);
        if self.current().kind == Kind::Word && next != "." {
            self.pos = keyword;
            return Err(self.choice_error("expected statement"));
        }
        self.expect("=")?;
        let actor = self.ident()?;
        self.expect(".")?;
        let capability = self.ident()?;
        let args = self.arguments()?;
        if self.repeat_depth > 0
            && ["response_loss", "retry", "on"]
                .iter()
                .any(|w| self.position_word(w))
        {
            return Err(self.loop_combination());
        }
        let after_args = self.pos;
        let response_loss = if self.take("response_loss") {
            let loss_start = self.tokens[self.pos - 1].span.start;
            let value = if self.take("none") {
                ResponseLoss::None
            } else if self.take("nondet") {
                ResponseLoss::Nondet
            } else if self.take("sequence") {
                self.expect("[")?;
                let mut receipts = Vec::new();
                loop {
                    let at = self.start();
                    let value = if self.take("delivered") {
                        Receipt::Delivered
                    } else {
                        self.expect_choice("unknown")?;
                        Receipt::Unknown
                    };
                    receipts.push(Spanned {
                        value,
                        span: self.span(at),
                    });
                    if !self.take(",") {
                        break;
                    }
                }
                self.expect("]")?;
                ResponseLoss::Sequence(receipts)
            } else {
                self.expect_choice("probability")?;
                let probability = self.probability()?;
                let basis = if self.take("basis") {
                    Some(self.string()?)
                } else {
                    None
                };
                ResponseLoss::Probability { probability, basis }
            };
            Some(Spanned {
                value,
                span: self.span(loss_start),
            })
        } else {
            self.pos = after_args;
            None
        };
        // Preserve the legacy U-SYN-0 classification of bare `retry 3`.
        let saved = self.pos;
        let retry = if self.take("retry") {
            let retry_start = self.tokens[self.pos - 1].span.start;
            if !self.take("max_attempts") {
                self.pos = saved;
                return Err(self.unsupported("retry outside the finite call grammar"));
            }
            let max_attempts = self.integer()?;
            self.expect("when")?;
            let receipt_unknown = self.take("receipt");
            if receipt_unknown {
                self.expect("unknown")?;
            }
            let has_results = !receipt_unknown || self.take("or");
            let results = if has_results {
                self.expect("result")?;
                self.expect("[")?;
                self.names("]")?
            } else {
                Vec::new()
            };
            self.expect("backoff")?;
            self.expect("fixed")?;
            let backoff = self.duration()?;
            self.expect("exhausted")?;
            self.expect("end")?;
            let exhausted = self.ident()?;
            Some(Retry {
                max_attempts,
                receipt_unknown,
                results,
                backoff,
                exhausted,
                span: self.span(retry_start),
            })
        } else {
            self.pos = saved;
            None
        };
        let mut query_command = None;
        let saved = self.pos;
        if self.position_word("for") && self.take("for") && self.take("command") {
            query_command = Some(Box::new(self.ident().map_err(|_| self.recovery_error())?));
            let finite_retry = self.at("retry")
                && self
                    .tokens
                    .iter()
                    .skip(self.pos + 1)
                    .find(|t| t.text != "\n")
                    .is_some_and(|t| t.text == "max_attempts");
            if self.at("response_loss") || finite_retry {
                let mut d = self.error("recovery_feature_combination");
                d.code = diag::U_SEM_0.into();
                return Err(d);
            }
        } else {
            self.pos = saved;
        }
        let mut recovery = None;
        let saved = self.pos;
        if self.take("on") {
            if !self.take("receipt_unknown") {
                self.pos = saved;
            } else {
                if self.recovery_depth > 0 {
                    return Err(self.unsupported("nested recovery handler"));
                }
                self.recovery_depth += 1;
                recovery = Some(Box::new(self.block().map_err(|mut d| {
                    if d.code == diag::E_SYN_0 {
                        d.message = "invalid_recovery_syntax".into();
                    }
                    d
                })?));
                self.recovery_depth -= 1;
                let after = self.pos;
                if self.take("on") && self.take("receipt_unknown") {
                    let mut d = self.error("invalid_recovery_contract");
                    d.code = diag::E_CON_0.into();
                    return Err(d);
                }
                self.pos = after;
            }
        } else {
            self.pos = saved;
        }
        Ok(Call {
            recovery,
            query_command,
            binding,
            actor,
            capability,
            args,
            response_loss,
            retry,
            span: self.span(start),
        })
    }

    fn position_word(&self, word: &str) -> bool {
        let mut next = self.tokens.iter().skip(self.pos).filter(|t| t.text != "\n");
        next.next().is_some_and(|t| t.text == word) && next.next().is_none_or(|t| t.text != "=")
    }

    fn recovery_error(&self) -> Diagnostic {
        self.error("invalid_recovery_syntax")
    }

    fn join(&mut self) -> Parsed<Spanned<Join>> {
        let start = self.expect("join")?.span.start;
        let value = if self.take("all") {
            Join::All {
                fork: self.ident()?,
            }
        } else {
            self.expect_choice("any")?;
            let fork = self.ident()?;
            self.expect("where")?;
            self.expect("result")?;
            self.expect("==")?;
            self.reject_record_literal("record join result is unsupported")?;
            let result = self.ident()?;
            self.separators();
            let tie_start = self.expect("tie")?.span.start;
            let tie = if self.take("by") {
                self.expect("[")?;
                Tie::By(self.names("]")?)
            } else {
                self.expect_choice("nondet")?;
                Tie::Nondet
            };
            let tie = Spanned {
                value: tie,
                span: self.span(tie_start),
            };
            self.separators();
            let remaining_start = self.expect("remaining")?.span.start;
            let remaining = if self.take("cancel_then_drain") {
                Remaining::CancelThenDrain
            } else {
                self.expect_choice("drain")?;
                Remaining::Drain
            };
            let remaining = Spanned {
                value: remaining,
                span: self.span(remaining_start),
            };
            self.separators();
            self.expect("if")?;
            self.expect("none")?;
            self.expect("->")?;
            let if_none = self.ident()?;
            Join::Any {
                fork,
                result,
                tie,
                remaining,
                if_none,
            }
        };
        Ok(Spanned {
            value,
            span: self.span(start),
        })
    }

    fn cond(&mut self) -> Parsed<Spanned<Cond>> {
        self.enter()?;
        let start = self.start();
        let first = self.atom()?;
        self.lines();
        let operator = self.current().text;
        let condition = if matches!(operator, "&&" | "||") {
            let mut operands = vec![first];
            while self.take(operator) {
                operands.push(self.atom()?);
            }
            if self.at("&&") || self.at("||") {
                return Err(self.error("mixed && and || require parentheses"));
            }
            Spanned {
                value: if operator == "&&" {
                    Cond::And(operands)
                } else {
                    Cond::Or(operands)
                },
                span: self.span(start),
            }
        } else {
            first
        };
        self.depth -= 1;
        Ok(condition)
    }

    fn assignment_operator(&mut self) -> Parsed<AssignmentOp> {
        if self.at("*") || self.at("/") {
            return Err(self.unsupported("integer multiplication or division"));
        }
        if self.take_adjacent("+", "=") {
            Ok(AssignmentOp::Add)
        } else if self.take_adjacent("-", "=") {
            Ok(AssignmentOp::Subtract)
        } else {
            self.expect("=")?;
            Ok(AssignmentOp::Assign)
        }
    }

    // Only Actor-level declarations enable new target syntax. Strings and nested
    // Capability items do not count; forward declarations do.
    fn has_actor_state(&self) -> bool {
        let mut depth = 0usize;
        let mut actor_depth = None;
        let mut actor_pending = false;
        for (index, t) in self.tokens.iter().enumerate() {
            if t.kind == Kind::Word && t.text == "actor" && depth == 1 {
                actor_pending = true;
            }
            match t.text {
                "{" if t.kind != Kind::String => {
                    depth += 1;
                    if actor_pending {
                        actor_depth = Some(depth);
                        actor_pending = false;
                    }
                }
                "}" if t.kind != Kind::String => {
                    if actor_depth == Some(depth) {
                        actor_depth = None;
                    }
                    depth = depth.saturating_sub(1);
                }
                "state" if t.kind == Kind::Word && actor_depth == Some(depth) => return true,
                "decides" if t.kind == Kind::Word && actor_depth == Some(depth) => {
                    let mut after = self.tokens[index + 1..].iter().filter(|t| t.text != "\n");
                    if after.next().is_some_and(|t| t.kind == Kind::Word)
                        && after.next().is_some_and(|t| t.text == ":")
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    // A qualified kernel assignment only opens the new Rule refusal path;
    // existing qualified writes to mutable/latent Actors keep their parser error.
    fn rule_target(&self, owner: &str, variable: &str) -> bool {
        let mut depth = 0usize;
        let mut pending = None;
        let mut actor = None;
        for (index, token) in self.tokens.iter().enumerate() {
            if token.kind == Kind::String {
                continue;
            }
            if token.text == "actor" && depth == 1 {
                pending = self.tokens[index + 1..]
                    .iter()
                    .find(|t| t.text != "\n")
                    .map(|t| t.text);
            }
            match token.text {
                "{" => {
                    depth += 1;
                    if depth == 2 {
                        actor = pending.take();
                    }
                }
                "}" => {
                    if depth == 2 {
                        actor = None;
                    }
                    depth = depth.saturating_sub(1);
                }
                "decides" if depth == 2 && actor == Some(owner) => {
                    let mut next = self.tokens[index + 1..].iter().filter(|t| t.text != "\n");
                    if next
                        .next()
                        .is_some_and(|t| t.text == variable && t.kind == Kind::Word)
                        && next.next().is_some_and(|t| t.text == ":")
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn state_target(&mut self, legacy: &str, repair: bool) -> Parsed<StateTarget> {
        let name = self.ident()?;
        if !self.at(".") {
            return Ok(StateTarget::Unqualified(name));
        }
        if !repair && !self.has_actor_state() && self.record_names().is_empty() {
            return Err(self.unsupported(legacy));
        }
        self.expect(".")?;
        let variable = self.ident()?;
        let span = Span {
            start: name.span.start,
            end: variable.span.end,
        };
        if self.at(".") {
            return Err(self.unsupported("multi-level state path"));
        }
        if repair && !self.has_actor_state() && self.record_names().is_empty() {
            let mut d = self.unsupported(legacy);
            d.span = Some(span);
            return Err(d);
        }
        Ok(StateTarget::Qualified {
            actor: name,
            variable,
            span,
        })
    }

    fn comparison_operator(&mut self) -> Parsed<Spanned<IntComparison>> {
        let start = self.start();
        let value = if self.take("==") {
            IntComparison::Eq
        } else if self.take_adjacent("!", "=") {
            IntComparison::Ne
        } else if self.take_adjacent("<", "=") {
            IntComparison::Le
        } else if self.take_adjacent(">", "=") {
            IntComparison::Ge
        } else if self.take("<") {
            IntComparison::Lt
        } else if self.take(">") {
            IntComparison::Gt
        } else {
            return Err(self.error("expected =="));
        };
        Ok(Spanned {
            value,
            span: self.span(start),
        })
    }

    fn atom(&mut self) -> Parsed<Spanned<Cond>> {
        if self.at("!") {
            return Err(self.unsupported("negation is unsupported"));
        }
        self.reject_record_comparison()?;
        if self.at("end") {
            return Err(self.unsupported("end is not a state expression"));
        }
        let start = self.start();
        let value = if self.take("(") {
            let inner = self.cond()?;
            self.expect(")")?;
            Cond::Group(Box::new(inner))
        } else {
            let binding = self.ident()?;
            if binding.value == "executor" && self.at(".") {
                return Err(self.unsupported("executor state expression"));
            }
            // Membership is contextual: keep both paths unresolved for lowering.
            let saved = self.pos;
            let mut parts = vec![binding.clone()];
            if self.take(".") {
                parts.push(self.ident()?);
            }
            if self.take("in") {
                let element = Spanned {
                    value: parts,
                    span: self.span(start),
                };
                let set = self.path()?;
                return Ok(Spanned {
                    value: Cond::Contains { element, set },
                    span: self.span(start),
                });
            }
            if binding.value == "case"
                && parts.get(1).is_some_and(|p| p.value == "id")
                && self.current().text != "not"
            {
                return Err(self.unsupported("case.id comparison is unsupported"));
            }
            self.pos = saved;
            if self.take(".") {
                let variable = self.ident()?;
                if self.at(".") {
                    return Err(self.unsupported("multi-level state path"));
                }
                let operator = self.comparison_operator()?;
                let value = self.value_ident()?;
                if matches!(self.current().text, "." | "+" | "-" | "*" | "/")
                    && (operator.value != IntComparison::Eq || self.has_actor_state())
                {
                    return Err(self.unsupported("comparison expression"));
                }
                Cond::LocalEq {
                    actor: binding,
                    variable,
                    operator,
                    value,
                }
            } else {
                if self.take("==") {
                    Cond::Eq {
                        binding,
                        value: self.value_ident()?,
                    }
                } else {
                    let start = self.start();
                    let operator = if self.take_adjacent("!", "=") {
                        IntComparison::Ne
                    } else if self.take_adjacent("<", "=") {
                        IntComparison::Le
                    } else if self.take_adjacent(">", "=") {
                        IntComparison::Ge
                    } else if self.take("<") {
                        IntComparison::Lt
                    } else if self.take(">") {
                        IntComparison::Gt
                    } else {
                        return Err(self.error("expected =="));
                    };
                    Cond::Compare {
                        binding,
                        operator: Spanned {
                            value: operator,
                            span: self.span(start),
                        },
                        value: self.value_ident()?,
                    }
                }
            }
        };
        if self.has_actor_state() && matches!(self.current().text, "." | "+" | "-" | "*" | "/") {
            return Err(self.unsupported("comparison expression"));
        }
        Ok(Spanned {
            value,
            span: self.span(start),
        })
    }

    fn bounds(&mut self) -> Parsed<Bounds> {
        let start = self.expect("bounds")?.span.start;
        self.expect("{")?;
        self.lines();
        let mut items = vec![self.bound()?];
        loop {
            let before = self.pos;
            self.separators();
            if self.take("}") {
                break;
            }
            if before == self.pos {
                return Err(self.error("expected bound separator or closing brace"));
            }
            items.push(self.bound()?);
        }
        Ok(Bounds {
            items,
            span: self.span(start),
        })
    }

    fn bound(&mut self) -> Parsed<Spanned<Bound>> {
        let start = self.start();
        let value = if self.take("cases") {
            Bound::Cases(self.integer()?)
        } else if self.take("live_tasks") {
            Bound::LiveTasks(self.integer()?)
        } else {
            self.expect_choice("clock")?;
            Bound::Clock(self.duration()?)
        };
        Ok(Spanned {
            value,
            span: self.span(start),
        })
    }

    fn string(&mut self) -> Parsed<Spanned<String>> {
        self.lines();
        let token = self.current();
        if token.kind != Kind::String {
            return Err(self.error("expected string"));
        }
        self.pos += 1;
        Ok(Spanned {
            value: token.text[1..token.text.len() - 1].into(),
            span: token.span,
        })
    }

    fn path(&mut self) -> Parsed<Path> {
        let start = self.start();
        let mut parts = vec![self.ident()?];
        while self.take(".") {
            parts.push(self.ident()?);
        }
        Ok(Spanned {
            value: parts,
            span: self.span(start),
        })
    }

    fn exclusion(&mut self) -> Parsed<Exclusion> {
        let target = self.path()?;
        self.expect("reason")?;
        let reason = self.string()?;
        let loses = if self.take("loses") {
            Some(self.string()?)
        } else {
            None
        };
        Ok(Exclusion {
            target,
            reason,
            loses,
        })
    }

    // A re-entry route is free text up to a top-level comma or bracket; it is
    // recorded from the source bytes, never interpreted.
    fn entry(&mut self) -> Parsed<Spanned<String>> {
        self.lines();
        let start = self.current().span.start;
        while !matches!(
            self.current().text,
            "," | "]" | "[" | "{" | "}" | "\n" | ";"
        ) && self.current().kind != Kind::Eof
        {
            // ADR 0007: an entry excludes every bracket, round ones included.
            if matches!(self.current().kind, Kind::Invalid | Kind::Unsupported)
                || matches!(self.current().text, "(" | ")" | "<" | ">" | "~" | "*")
            {
                return Err(self.error("unexpected token in reentry"));
            }
            self.pos += 1;
        }
        let span = self.span(start);
        if span.start >= span.end || self.tokens[self.pos - 1].span.end <= start {
            return Err(self.error("expected reentry entry"));
        }
        Ok(Spanned {
            value: self.src[span.start..span.end].into(),
            span,
        })
    }

    // Recording labels are not identifiers: reserved words are legitimate here.
    fn label(&mut self) -> Parsed<Ident> {
        self.lines();
        let token = self.current();
        if token.kind != Kind::Word {
            return Err(self.error("expected label"));
        }
        self.pos += 1;
        Ok(Spanned {
            value: token.text.into(),
            span: token.span,
        })
    }

    fn horizon(&mut self) -> Parsed<Horizon> {
        let first = self.label()?;
        let (from, to) = if self.at(".") {
            if !self.take_adjacent(".", ".") {
                return Err(self.error("expected adjacent .."));
            }
            (Some(first), self.label()?)
        } else {
            (None, first)
        };
        self.expect("within")?;
        let within = self.discrete_duration()?;
        let structural_only = self.take("structural_only");
        Ok(Horizon {
            from,
            to,
            within,
            structural_only,
        })
    }

    fn cut(&mut self) -> Parsed<CutDecl> {
        let start = self.expect("cut")?.span.start;
        let name = self.ident()?;
        self.expect("{")?;
        let mut items = Vec::new();
        self.separators();
        while !self.at("}") {
            let item_start = self.start();
            let value = if self.take("horizon") {
                let mut entries = vec![self.horizon()?];
                loop {
                    let saved = self.pos;
                    if !self.take(";") {
                        self.pos = saved;
                        break;
                    }
                    let probe = self.pos;
                    let next = self.label().is_ok() && (self.at("within") || self.at("."));
                    self.pos = probe;
                    if !next {
                        self.pos = saved;
                        break;
                    }
                    entries.push(self.horizon()?);
                }
                CutItem::Horizon(entries)
            } else if self.take("question") {
                CutItem::Question(self.string()?)
            } else if self.take("include") {
                self.expect("[")?;
                CutItem::Include(self.names("]")?)
            } else if self.take("exclude") {
                CutItem::Exclude(self.exclusion()?)
            } else if self.take("defer") {
                CutItem::Defer(self.exclusion()?)
            } else if self.take("externality") {
                self.expect("[")?;
                let mut paths = vec![self.path()?];
                while self.take(",") {
                    paths.push(self.path()?);
                }
                self.expect("]")?;
                CutItem::Externality(paths)
            } else if self.take("reentry") {
                self.expect("[")?;
                let mut entries = vec![self.entry()?];
                while self.take(",") {
                    entries.push(self.entry()?);
                }
                self.expect("]")?;
                CutItem::Reentry(entries)
            } else {
                // horizon and every other word are outside ADR 0007.
                return Err(self.choice_error("expected cut item or closing brace"));
            };
            items.push(Spanned {
                value,
                span: self.span(item_start),
            });
            self.separators();
        }
        self.expect("}")?;
        Ok(CutDecl {
            name,
            items,
            span: self.span(start),
        })
    }

    // These words are contextual, not globally reserved identifiers. Within a
    // configuration/intent, a misplaced known clause is a structural error;
    // unknown commands remain outside ADR 0008.
    fn configuration_error(&self, message: &str) -> Diagnostic {
        if matches!(
            self.current().text,
            "cut"
                | "process"
                | "intent"
                | "add"
                | "retire"
                | "reason"
                | "loses"
                | "change"
                | "preserve"
                | "override"
                | "maintain"
                | "on"
                | "by"
                | "from"
                | "normal_path"
                | "keep"
                | "fallback"
        ) {
            self.error(message)
        } else {
            self.choice_error(message)
        }
    }

    fn configuration_clause(&mut self, clause: &str) -> Parsed<()> {
        if self.take(clause) {
            return Ok(());
        }
        Err(self.configuration_error(&format!("expected {clause}")))
    }

    fn configuration(&mut self) -> Parsed<Configuration> {
        let start = self.expect("configuration")?.span.start;
        // The old unnamed block form is still outside the language.
        if self.at("{") {
            return Err(self.unsupported("unnamed configuration"));
        }
        let name = self.ident()?;
        self.expect("{")?;
        self.separators();
        self.configuration_clause("cut")?;
        let cut = self.ident()?;
        self.separators();
        self.configuration_clause("process")?;
        let process = self.ident()?;
        self.separators();
        let mut amendments = Vec::new();
        while self.at("amend") {
            amendments.push(self.amendment()?);
            self.separators();
        }
        let mut intent = Vec::new();
        if self.take("intent") {
            self.expect("{")?;
            self.separators();
            while !self.at("}") {
                intent.push(self.configuration_intent()?);
                self.separators();
            }
            self.expect("}")?;
            self.separators();
        }
        if !self.at("}") {
            return Err(self.configuration_error("expected configuration closing brace"));
        }
        self.expect("}")?;
        Ok(Configuration {
            name,
            cut,
            process,
            amendments,
            intent,
            span: self.span(start),
        })
    }

    fn amendment(&mut self) -> Parsed<Amendment> {
        let error = self.unsupported("amend");
        let start = self.expect("amend")?.span.start;
        let result = (|| {
            let rule = self.ident()?;
            self.expect("from")?;
            let from = self.string()?;
            self.expect("to")?;
            let to = self.string()?;
            self.expect("by")?;
            let by = self.ident()?;
            self.expect("under")?;
            let under = self.ident()?;
            self.expect("{")?;
            self.separators();
            let mut values = Vec::new();
            loop {
                let name = self.ident()?;
                self.expect("=")?;
                let value = self.value_ident()?;
                if matches!(self.current().text, "==" | "&&" | "||" | ".")
                    || self.at_adjacent("!", "=")
                {
                    return Err(self.unsupported("conditional rule value"));
                }
                values.push((name, value));
                self.separators();
                if self.take("}") {
                    break;
                }
            }
            Ok(Amendment {
                rule,
                from,
                to,
                by,
                under,
                values,
                span: self.span(start),
            })
        })();
        result.map_err(|d: Diagnostic| {
            if d.message.contains("conditional rule value") {
                d
            } else {
                error
            }
        })
    }

    fn diff_target(&mut self, explicit: bool) -> Parsed<DiffTarget> {
        self.lines();
        let kind = DiffKind::parse(self.current().text);
        if kind.is_some() {
            self.pos += 1;
        } else if explicit {
            return Err(self.configuration_error("expected difference kind"));
        }
        let path = if self.at("end") {
            let token = self.expect("end")?;
            self.expect(".")?;
            let label = self.ident()?;
            Spanned {
                value: vec![
                    Spanned {
                        value: "end".into(),
                        span: token.span,
                    },
                    label,
                ],
                span: self.span(token.span.start),
            }
        } else {
            self.path()?
        };
        if path.value.len() > 2
            && !(kind == Some(DiffKind::Contest)
                && path.value.len() == 3
                && path.value[2].value == "commit")
        {
            return Err(self.unsupported("intent targets are Actor or Actor.Capability"));
        }
        Ok(DiffTarget {
            kind: kind.unwrap_or(if path.value.len() == 1 {
                DiffKind::Actor
            } else {
                DiffKind::Relation
            }),
            path,
        })
    }

    fn configuration_intent(&mut self) -> Parsed<Intent> {
        let start = self.start();
        if self.take("preserve") {
            let selector = if self.at("}") || self.at(";") || self.at("on") {
                return Err(self.error("expected preservation selector"));
            } else if self.current().text.starts_with('"') {
                PropertySelector::Exact(self.string()?)
            } else {
                PropertySelector::Contract(self.ident()?)
            };
            let on = if self.take("on") {
                Some(self.diff_target(true)?)
            } else {
                None
            };
            return Ok(Intent::Preserve {
                selector,
                on,
                span: self.span(start),
            });
        }
        let annotation = if self.take("override") {
            Some(true)
        } else if self.take("maintain") {
            Some(false)
        } else {
            None
        };
        if let Some(over) = annotation {
            let target = self.diff_target(false)?;
            self.configuration_clause("by")?;
            if self.at("[") {
                return Err(self.unsupported("subject lists"));
            }
            let subject = self.ident()?;
            if self.at(".") {
                return Err(self.unsupported("capability subjects"));
            }
            let reason = if self.take("reason") {
                Some(self.string()?)
            } else {
                None
            };
            return Ok(if over {
                Intent::Override {
                    target,
                    subject,
                    reason,
                    span: self.span(start),
                }
            } else {
                Intent::Maintain {
                    target,
                    person: subject,
                    reason,
                    span: self.span(start),
                }
            });
        }
        let action = if self.take("add") {
            IntentAction::Add
        } else if self.take("retire") {
            IntentAction::Retire
        } else if self.take("change") {
            IntentAction::Change
        } else {
            return Err(self.configuration_error("expected configuration intent"));
        };
        let target = if action == IntentAction::Change {
            self.diff_target(true)?
        } else {
            let path = self.path()?;
            if path.value.len() > 2 {
                return Err(Diagnostic {
                    code: diag::U_SYN_0.into(),
                    severity: diag::Severity::Error,
                    message: "intent targets are Actor or Actor.Capability".into(),
                    span: Some(path.span),
                });
            }
            DiffTarget {
                kind: if path.value.len() == 1 {
                    DiffKind::Actor
                } else {
                    DiffKind::Relation
                },
                path,
            }
        };
        let keep_fallback = if self.take("from") {
            if action != IntentAction::Retire {
                return Err(self.error("from normal_path requires retire"));
            }
            if !self.take("normal_path") {
                return Err(self.unsupported("unsupported retire path"));
            }
            self.expect("keep")?;
            self.expect("fallback")?;
            if target.path.value.len() != 2 {
                return Err(self.unsupported("fallback requires Actor.Capability"));
            }
            true
        } else {
            false
        };
        let reason = if keep_fallback {
            if self.take("reason") {
                Some(self.string()?)
            } else {
                None
            }
        } else {
            self.configuration_clause("reason")?;
            Some(self.string()?)
        };
        let loses = if action != IntentAction::Add && self.take("loses") {
            Some(self.string()?)
        } else {
            None
        };
        Ok(Intent::Claim {
            action,
            target,
            reason,
            keep_fallback,
            loses,
            span: self.span(start),
        })
    }

    fn contract_error(&self, message: &str) -> Diagnostic {
        if matches!(
            self.current().text,
            "contract"
                | "invariant"
                | "at_most_once"
                | "response"
                | "requires_event"
                | "separation"
                | "permitted"
                | "authority_basis"
                | "one_of"
                | "by"
                | "after"
                | "otherwise"
                | "unresolved"
                | "start"
                | "finish"
                | "commit"
                | "per"
                | "case"
                | "id"
                | "received"
                | "end"
        ) {
            let mut d = self.error(message);
            d.code = diag::E_SYN_0.into();
            d
        } else {
            self.choice_error(message)
        }
    }

    fn contract_word(&mut self, word: &str) -> Parsed<()> {
        if self.take(word) {
            Ok(())
        } else {
            Err(self.contract_error(&format!("expected {word}")))
        }
    }

    fn event(&mut self) -> Parsed<Event> {
        self.event_in(false)
    }

    /// Separation compares executors, so the executor-less receipt and
    /// terminal events are unsupported there. Elsewhere they keep the
    /// ADR 0010 classification of a known word in the wrong position.
    fn event_in(&mut self, separation: bool) -> Parsed<Event> {
        self.event_context(separation, false)
    }

    fn event_context(&mut self, separation: bool, cancel_ack: bool) -> Parsed<Event> {
        let start = self.start();
        if separation && self.at("end") {
            return Err(self.unsupported("terminal is not a Task event"));
        }
        let actor = self.ident()?;
        if !self.take(".") {
            let mut error = self.unsupported("state antecedent");
            error.span = Some(actor.span);
            return Err(error);
        }
        let capability = self.ident()?;
        if separation && actor.value == "case" && capability.value == "received" && !self.at(".") {
            return Err(self.unsupported("receipt has no executor"));
        }
        if actor.value == "case" && capability.value == "admitted" && !self.at(".") {
            return Err(self.unsupported("case.admitted is only a response trigger"));
        }
        self.expect(".")?;
        let kind = if cancel_ack && self.take("cancel_ack") {
            EventKind::CancelAck
        } else if self.take("start") {
            EventKind::Start
        } else if self.take("commit") {
            EventKind::Commit
        } else if self.take("finish") {
            EventKind::Finish(if self.take("(") {
                self.reject_record_literal("record event result is unsupported")?;
                let value = self.ident()?;
                self.expect(")")?;
                Some(value)
            } else {
                None
            })
        } else {
            return Err(self.contract_error("expected event kind"));
        };
        Ok(Event {
            actor,
            capability,
            kind,
            span: self.span(start),
        })
    }

    fn trigger(&mut self) -> Parsed<Trigger> {
        let saved = self.pos;
        if self.take("case") && self.take(".") && self.take("received") {
            return Ok(Trigger::Received);
        }
        self.pos = saved;
        let start = self.start();
        if self.take("case") && self.take(".") && self.take("admitted") && !self.at(".") {
            return Ok(Trigger::Admitted(self.span(start)));
        }
        self.pos = saved;
        Ok(Trigger::Event(self.event()?))
    }

    fn target(&mut self) -> Parsed<Target> {
        if self.take("end") {
            self.expect(".")?;
            Ok(Target::End(self.ident()?))
        } else {
            Ok(Target::Event(self.event()?))
        }
    }

    fn standing_ref(&mut self) -> Parsed<StandingRef> {
        self.lines();
        if self.current().kind == Kind::Word && !self.at("end") {
            let saved = self.pos;
            let parsed = (|| {
                let start = self.start();
                let actor = self.ident()?;
                self.expect(".")?;
                let capability = self.ident()?;
                let mut path = CapabilityPath {
                    actor,
                    capability,
                    span: self.span(start),
                };
                self.expect(".")?;
                self.expect("commit")?;
                path.span = self.span(start);
                if self.at(".") || self.at("(") {
                    return Err(self.unsupported("standing supports only end and commit targets"));
                }
                Ok(StandingRef::Commit(path))
            })();
            if parsed.is_ok() {
                return parsed;
            }
            self.pos = saved;
            return Err(self.unsupported("standing supports only end and commit targets"));
        }
        self.expect("end")?;
        self.expect(".")?;
        Ok(StandingRef::End(self.ident()?))
    }

    fn invariant_clause(&mut self) -> Parsed<ContractClause> {
        self.lines();
        let look: Vec<_> = self.tokens[self.pos..]
            .iter()
            .filter(|t| t.text != "\n")
            .take(4)
            .copied()
            .collect();
        let terminal = look.first().is_some_and(|t| t.text == "end");
        if terminal
            && !(look.get(1).is_some_and(|t| t.text == ".")
                && look.get(2).is_some_and(|t| t.kind == Kind::Word))
        {
            return Err(self.error("expected identifier"));
        }
        if terminal
            || (look.first().is_some_and(|t| t.text == "case")
                && look.get(1).is_some_and(|t| t.text == ".")
                && look.get(2).is_some_and(|t| t.text == "received")
                && look.get(3).is_none_or(|t| t.text != "."))
        {
            let mut d = self.unsupported("terminal or receipt antecedent");
            if look.len() >= 3 && look[1].text == "." {
                d.span = Some(Span {
                    start: look[0].span.start,
                    end: look[2].span.end,
                });
            }
            return Err(d);
        }
        if look.len() == 4
            && look[0].kind == Kind::Word
            && look[1].text == "."
            && look[2].kind == Kind::Word
            && look[3].text == "."
        {
            let event = self.event()?;
            self.expect("->")?;
            let c = self.invariant_cond(true)?;
            if self.at("->") {
                return Err(self.unsupported("nested invariant implication"));
            }
            return Ok(ContractClause::Invariant {
                event,
                condition: Self::event_invariant_cond(c),
            });
        }
        let left = self.invariant_cond(false)?;
        let (antecedent, condition) = if self.take("->") {
            (Some(left), self.invariant_cond(false)?)
        } else {
            (None, left)
        };
        if self.at("->") {
            return Err(self.unsupported("nested invariant implication"));
        }
        Ok(ContractClause::StateInvariant {
            antecedent,
            condition,
        })
    }

    // Reconstruct the legacy tree exactly; bare atoms have already been refused.
    fn event_invariant_cond(c: Spanned<InvariantStateCond>) -> Spanned<Cond> {
        let value = match c.value {
            InvariantStateCond::Compare(c) => return c,
            InvariantStateCond::Group(c) => Cond::Group(Box::new(Self::event_invariant_cond(*c))),
            InvariantStateCond::And(cs) => {
                Cond::And(cs.into_iter().map(Self::event_invariant_cond).collect())
            }
            InvariantStateCond::Or(cs) => {
                Cond::Or(cs.into_iter().map(Self::event_invariant_cond).collect())
            }
            InvariantStateCond::Bool(_) => unreachable!("event bare atom refused by parser"),
        };
        Spanned {
            value,
            span: c.span,
        }
    }

    fn invariant_cond(&mut self, event: bool) -> Parsed<Spanned<InvariantStateCond>> {
        self.enter()?;
        let start = self.start();
        let first = self.invariant_atom(event)?;
        self.lines();
        let operator = self.current().text;
        let result = if matches!(operator, "&&" | "||") {
            let mut parts = vec![first];
            while self.take(operator) {
                parts.push(self.invariant_atom(event)?);
            }
            if self.at("&&") || self.at("||") {
                return Err(self.error("mixed && and || require parentheses"));
            }
            Spanned {
                value: if operator == "&&" {
                    InvariantStateCond::And(parts)
                } else {
                    InvariantStateCond::Or(parts)
                },
                span: self.span(start),
            }
        } else {
            first
        };
        self.depth -= 1;
        Ok(result)
    }

    fn invariant_atom(&mut self, event: bool) -> Parsed<Spanned<InvariantStateCond>> {
        let start = self.start();
        if self.take("(") {
            let inner = self.invariant_cond(event)?;
            if self.at("->") {
                return Err(self.unsupported("nested invariant implication"));
            }
            self.expect(")")?;
            // Same trailing check as atom's group, for old and new clauses alike.
            if self.has_actor_state() && matches!(self.current().text, "." | "+" | "-" | "*" | "/")
            {
                return Err(self.unsupported("comparison expression"));
            }
            return Ok(Spanned {
                value: InvariantStateCond::Group(Box::new(inner)),
                span: self.span(start),
            });
        }
        if !event
            && matches!(
                self.current().text,
                "forall"
                    | "exists"
                    | "always"
                    | "eventually"
                    | "until"
                    | "clock"
                    | "task"
                    | "queue"
                    | "resource"
                    | "obligation"
                    | "history"
            )
        {
            return Err(self.unsupported("state invariant expression"));
        }
        let look: Vec<_> = self.tokens[self.pos..]
            .iter()
            .filter(|t| t.text != "\n")
            .take(4)
            .copied()
            .collect();
        let boundary = |t: &Token<'_>| {
            matches!(
                t.text,
                "&&" | "||"
                    | "->"
                    | ")"
                    | ";"
                    | "}"
                    | "invariant"
                    | "at_most_once"
                    | "response"
                    | "requires_event"
                    | "separation"
                    | "permitted"
                    | "authority_basis"
                    | "excludes"
                    | "waive"
                    | "contest"
            ) || t.kind == Kind::Eof
        };
        if look
            .first()
            .is_some_and(|t| t.kind == Kind::Word && !keyword(t.text) && !outside_keyword(t.text))
            && look.get(1).is_some_and(boundary)
        {
            if event {
                if look[1].text == "->" {
                    let mut d = self.unsupported("nested invariant implication");
                    d.span = Some(look[1].span);
                    return Err(d);
                }
                return Err(self.unsupported("bare event invariant atom"));
            }
            let name = self.ident()?;
            return Ok(Spanned {
                span: name.span,
                value: InvariantStateCond::Bool(name),
            });
        }
        if !event
            && look.len() == 4
            && look[0].kind == Kind::Word
            && look[1].text == "."
            && look[2].kind == Kind::Word
            && boundary(&look[3])
        {
            let mut d = self.unsupported("qualified bare invariant atom");
            d.span = Some(Span {
                start: look[0].span.start,
                end: look[2].span.end,
            });
            return Err(d);
        }
        self.atom().map(|c| Spanned {
            span: c.span,
            value: InvariantStateCond::Compare(c),
        })
    }

    fn contest_rates(&mut self) -> Parsed<ContestRates> {
        let begin = self.expect("rates")?.span.start;
        self.expect("{")?;
        self.separators();
        let mut occurrence = None;
        let mut service_time_basis = None;
        let mut independence = None;
        while !self.at("}") {
            let repeated = if self.take("occurrence") {
                if occurrence.is_some() {
                    true
                } else {
                    self.expect("probability")?;
                    let probability = self.probability()?;
                    self.expect("basis")?;
                    occurrence = Some((probability, self.string()?));
                    false
                }
            } else if self.take("service_time") {
                if service_time_basis.is_some() {
                    true
                } else {
                    self.expect("basis")?;
                    service_time_basis = Some(self.string()?);
                    false
                }
            } else if self.take("independence") {
                if independence.is_some() {
                    true
                } else {
                    independence = Some(self.ident()?);
                    false
                }
            } else {
                return Err(self.unsupported("contest rates field"));
            };
            if repeated {
                let mut d = self.error("duplicate contest rates field");
                d.code = diag::U_SEM_0.into();
                return Err(d);
            }
            self.separators();
        }
        // Missing declarations are syntax errors; none has a default value.
        let (probability, occurrence_basis) = occurrence
            .ok_or_else(|| self.error("contest rates requires occurrence probability and basis"))?;
        let service_time_basis = service_time_basis
            .ok_or_else(|| self.error("contest rates requires service_time basis"))?;
        let independence =
            independence.ok_or_else(|| self.error("contest rates requires independence"))?;
        self.expect("}")?;
        let span = self.span(begin);
        if self.at("rates") {
            let mut d = self.error("duplicate contest rates block");
            d.code = diag::U_SEM_0.into();
            return Err(d);
        }
        Ok(ContestRates {
            probability,
            occurrence_basis,
            service_time_basis,
            independence,
            span,
        })
    }

    fn contract(&mut self) -> Parsed<ContractDecl> {
        let start = self.expect("contract")?.span.start;
        let name = if self.at("{") {
            None
        } else {
            Some(self.ident()?)
        };
        self.expect("{")?;
        self.separators();
        let mut clauses = Vec::new();
        while !self.at("}") {
            let begin = self.start();
            let value = if self.take("excludes") {
                let before = self.event_context(false, true)?;
                if !self.take(",") {
                    return Err(self.unsupported("short excludes form"));
                }
                ContractClause::Excludes {
                    before,
                    start: self.event()?,
                }
            } else if self.take("waive") {
                let mut response = vec![self.ident()?];
                if self.take(".") {
                    response.push(self.ident()?);
                    if self.at(".") {
                        return Err(self.unsupported("event-form waive"));
                    }
                }
                self.expect("on")?;
                let event = self.event_context(false, true)?;
                self.expect("reason")?;
                let reason = self.string()?;
                self.expect("authorized_by")?;
                let authority = self.ident()?;
                ContractClause::Waive {
                    response,
                    event,
                    reason,
                    authority,
                }
            } else if self.take("contest") {
                let party = self.ident()?;
                if self.at(".") {
                    return Err(self.unsupported("event-form contest"));
                }
                self.expect("on")?;
                let target = self.standing_ref()?;
                self.expect("route")?;
                let route = self.capability_path()?;
                self.expect("reversible_by")?;
                self.expect("[")?;
                let reversible_by = self.names("]")?;
                self.expect("respond_by")?;
                let respond_by = self.discrete_duration()?;
                let rates = if self.at("rates") {
                    Some(self.contest_rates()?)
                } else {
                    None
                };
                ContractClause::Contest {
                    party,
                    target,
                    route,
                    reversible_by,
                    respond_by,
                    rates,
                }
            } else if self.take("separation") {
                let left = self.event_in(true)?;
                self.expect(",")?;
                ContractClause::Separation {
                    left,
                    right: self.event_in(true)?,
                }
            } else if self.take("permitted") {
                ContractClause::Permitted {
                    target: self.capability_path()?,
                }
            } else if self.take("authority_basis") {
                let target = self.capability_path()?;
                ContractClause::AuthorityBasis {
                    target,
                    authority: self.ident()?,
                }
            } else if self.take("invariant") {
                self.invariant_clause()?
            } else if self.take("at_most_once") {
                let event = self.event()?;
                self.contract_word("per")?;
                self.contract_word("case")?;
                self.expect(".")?;
                self.contract_word("id")?;
                ContractClause::AtMostOnce { event }
            } else if self.take("response") {
                let saved = self.pos;
                let candidate = self.ident()?;
                let label = if self.take(":") {
                    Some(candidate)
                } else {
                    self.pos = saved;
                    None
                };
                let trigger = self.trigger()?;
                self.expect("->")?;
                self.contract_word("one_of")?;
                self.expect("(")?;
                let mut targets = vec![self.target()?];
                while self.take(",") {
                    targets.push(self.target()?);
                }
                self.expect(")")?;
                self.contract_word("by")?;
                let duration = self.duration()?;
                self.contract_word("after")?;
                let after = self.trigger()?;
                self.contract_word("otherwise")?;
                if !self.take("unresolved") {
                    return Err(self.unsupported("otherwise form"));
                }
                let mut non_waivable = false;
                let mut debtor = None;
                loop {
                    if self.take("non_waivable") {
                        if non_waivable {
                            let mut d = self.error("repeated non_waivable flag");
                            d.code = diag::U_SEM_0.into();
                            return Err(d);
                        }
                        non_waivable = true;
                    } else if self.take("debtor") {
                        if debtor.is_some() {
                            let mut d = self.error("repeated debtor");
                            d.code = diag::U_SEM_0.into();
                            return Err(d);
                        }
                        debtor = Some(self.ident()?);
                    } else {
                        break;
                    }
                }
                ContractClause::Response {
                    label,
                    debtor,
                    non_waivable,
                    trigger,
                    targets,
                    duration,
                    after,
                }
            } else if self.take("requires_event") {
                let event = self.event()?;
                self.contract_word("after")?;
                ContractClause::RequiresEvent {
                    event,
                    after: self.event()?,
                }
            } else {
                return Err(self.contract_error("expected contract clause"));
            };
            let span = self.span(begin);
            clauses.push(Spanned { value, span });
            self.lines();
            if self.current().text != "}"
                && self.current().text != ";"
                && !self.src[span.end..self.current().span.start].contains('\n')
            {
                return Err(self.contract_error("expected contract clause separator"));
            }
            self.separators();
        }
        self.expect("}")?;
        Ok(ContractDecl {
            name,
            clauses,
            span: self.span(start),
        })
    }

    fn verify(&mut self) -> Parsed<Verify> {
        let start = self.expect("verify")?.span.start;
        let value = if self.take("probability") {
            if self.at("cohort") {
                return Err(self.unsupported("cohort probability"));
            }
            self.expect("reached")?;
            let condition = self.probability_condition()?;
            let within = if self.take("within") {
                Some(Box::new(self.discrete_duration()?))
            } else {
                None
            };
            let comparison = if self.take_adjacent("<", "=") {
                Comparison::Le
            } else if self.take_adjacent(">", "=") {
                Comparison::Ge
            } else {
                return Err(self.unsupported("probability comparison"));
            };
            Property::ProbabilityReached {
                condition,
                within,
                comparison,
                bound: self.probability()?,
            }
        } else if self.at("recoverable") || self.at("terminating") || self.at("almost_sure") {
            let property = if self.take("recoverable") {
                Property::RecoverableEnd
            } else if self.take("terminating") {
                Property::TerminatingEnd
            } else {
                self.expect("almost_sure")?;
                Property::AlmostSureEnd
            };
            self.expect("end")?;
            if self.at(".") {
                return Err(self.unsupported("completion takes the union of ends"));
            }
            property
        } else if self.take("safety") {
            self.expect_choice("no_resource_overcommit")?;
            Property::NoResourceOvercommit
        } else {
            self.expect_choice("reachable")?;
            self.expect("end")?;
            self.expect(".")?;
            Property::ReachableEnd(self.ident()?)
        };
        Ok(Spanned {
            value,
            span: self.span(start),
        })
    }

    fn probability_condition(&mut self) -> Parsed<ProbabilityCondition> {
        // Look ahead without consuming the ordinary state's parentheses. Only
        // a standalone, optionally parenthesized end label uses this grammar.
        let mut depth = 0;
        let mut cursor = self.pos;
        loop {
            while self.tokens.get(cursor).is_some_and(|t| t.text == "\n") {
                cursor += 1;
            }
            if self.tokens.get(cursor).is_some_and(|t| t.text == "(") {
                depth += 1;
                cursor += 1;
            } else {
                break;
            }
        }
        if !self.tokens.get(cursor).is_some_and(|t| t.text == "end") {
            return self.cond().map(ProbabilityCondition::State);
        }
        if self.depth.saturating_add(depth) >= 128 {
            return Err(self.unsupported("condition nesting limit"));
        }
        for _ in 0..depth {
            self.expect("(")?;
        }
        self.expect("end")?;
        self.expect(".")?;
        let end = self.ident()?;
        for _ in 0..depth {
            self.lines();
            if matches!(
                self.current().text,
                "&&" | "||" | "==" | "!=" | "!" | "<" | ">"
            ) {
                return Err(self.unsupported("end probability condition must be standalone"));
            }
            self.expect(")")?;
        }
        if !self.at("within") && !self.at("<") && !self.at(">") {
            return Err(self.unsupported("end probability condition must be standalone"));
        }
        Ok(ProbabilityCondition::End(end))
    }
}

#[cfg(test)]
mod record_parser_tests {
    use super::*;

    #[test]
    fn inactive_record_contract_binding_is_not_promoted_or_misread_as_actor() {
        let src = r#"model Inactive version "0.2" {
          type T=enum{ok}; type R=record{f:bool}
          actor A {capacity 1
            capability r()->R {duration fixed 1min;result nondet{R{f=true}}}
            capability c()->T {commits;duration fixed 1min;result nondet{ok}}
          }
          process P {e=A.r();a=A.c();end done}
          process Q {end done}
          cut Scope {include[A]}
          configuration One {cut Scope;process P}
          configuration Two {cut Scope;process Q}
          contract C {invariant A.c.commit -> e.f == true}
          bounds {cases 1;live_tasks 1;clock 5min}
          verify safety no_resource_overcommit
        }"#;
        assert_eq!(
            crate::report::check_configuration(src, "One").verdict,
            crate::Verdict::Satisfied
        );
        assert_eq!(
            crate::report::check_configuration(src, "Two").verdict,
            crate::Verdict::Vacuous
        );
        let unavailable = src.replace(
            "process Q {end done}",
            "process Q {if e.f == true {end done} else {end no}}",
        );
        // The scalar twin (`if e == ...` in Q) is E-NAME-0 undefined result
        // binding; the record field read of P's binding is the same class.
        let r = crate::report::check_configuration(&unavailable, "Two");
        assert_eq!(r.verdict, crate::Verdict::Invalid);
        let errors: Vec<_> = r
            .diagnostics
            .iter()
            .filter(|d| d.severity == crate::diag::Severity::Error)
            .map(|d| (d.code.as_str(), d.message.as_str()))
            .collect();
        assert_eq!(errors, [("E-NAME-0", "undefined result binding")]);
    }

    #[test]
    fn enum_named_record_empty_capability_keeps_legacy_lower_diagnostics() {
        let src = r#"model EmptyLegacy version "0.2" {
          type record = enum { yes }
          actor A { capacity 1; capability run() -> record {} }
          process P { end done }
          bounds { cases 1; live_tasks 1; clock 2min }
          verify safety no_resource_overcommit
        }"#;
        let ast = parse(src).expect("empty Capability remains representable in the legacy AST");
        let ds = crate::lower::lower(&ast).unwrap_err();
        assert_eq!(ds.len(), 3, "{ds:?}");
        assert_eq!(ds.iter().filter(|d| d.code == diag::U_SEM_0).count(), 2);
        assert_eq!(ds.iter().filter(|d| d.code == diag::W_CUT_0).count(), 1);
        assert!(
            ds.iter()
                .any(|d| d.message == "Capability requires explicit duration")
        );
        assert!(
            ds.iter()
                .any(|d| d.message == "Capability requires explicit result set")
        );
    }

    #[test]
    fn whole_record_binding_comparisons_keep_unsupported_classification() {
        for operator in ["==", "!=", "<", "<=", ">", ">="] {
            let src = format!(
                r#"model M version "0.2" {{
              type R=record {{ f:bool }}
              actor A{{capacity 1;capability run()->R{{duration fixed 1min;result nondet{{R{{f=true}}}}}}}}
              process P{{e=A.run();if e {operator} none {{end yes}} else {{end no}}}}
              bounds{{cases 1;live_tasks 1;clock 2min}}
              verify safety no_resource_overcommit
            }}"#
            );
            let report = crate::report::check(&src);
            assert_eq!(
                report.verdict,
                crate::Verdict::Unsupported,
                "{operator}: {:?}",
                report.diagnostics
            );
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|d| d.code == diag::U_SYN_0 && d.message.contains("record comparison"))
            );
        }
    }

    #[test]
    fn record_lookahead_preserves_legacy_newline_defaults_and_effects() {
        let src = r#"model M version "0.2" {
          type T=enum {ok}
          observed case {
            x:T=ok
            y:T=ok
          }
          actor A {capacity 1;capability run()->T {
            duration fixed 1min
            result nondet {ok}
            writes [x,y]
            start {
              x=ok
              y=ok
            }
            finish {
              x=ok
              y=ok
            }
          }}
          process P {a=A.run();end done}
          bounds {cases 1;live_tasks 1;clock 2min}
          verify safety no_resource_overcommit
        }"#;
        assert_eq!(crate::report::check(src).verdict, crate::Verdict::Satisfied);
    }

    #[test]
    fn record_fields_and_whole_values_accept_newline_declaration_separators() {
        let src = r#"model M version "0.2" {
          type R=record {
            f:enum {a,b}
            g:bool
          }
          observed case {
            x:R=R{f=a,g=true}
            y:optional<R>
          }
          process P {if x.g == true {end done} else {end no}}
          bounds {cases 1;live_tasks 1;clock 2min}
          verify reachable end.done
        }"#;
        assert_eq!(crate::report::check(src).verdict, crate::Verdict::Satisfied);
    }

    #[test]
    fn record_rejection_lookahead_does_not_read_strings_or_enum_if_bodies() {
        let src = r#"model M version "0.2" {
          type R=record { f:bool }
          type E=enum { R, other }
          actor A {capacity 1; capability run()->E {duration fixed 1min;result nondet {R,other}}}
          process P {e=A.run(); if e == R {end yes} else {end no}}
          bounds {cases 1;live_tasks 1;clock 2min} verify safety no_resource_overcommit
        }"#;
        assert!(crate::lower::lower(&parse(src).unwrap()).is_ok());
        for body in ["more=A.run();end yes", ""] {
            let variant = src
                .replace("if e == R {end yes}", &format!("if e == R {{{body}}}"))
                .replace("else {end no}", "else {end no};end done");
            assert!(crate::lower::lower(&parse(&variant).unwrap()).is_ok());
        }
        let src = src.replace("type R=record { f:bool }", "");
        let src = src.replace(
            "capacity 1",
            "boundary \"type R = record { f:bool }\";capacity 1",
        );
        assert!(crate::lower::lower(&parse(&src).unwrap()).is_ok());
    }

    #[test]
    fn record_literals_are_rejected_in_existing_scalar_only_positions() {
        let prefix = r#"model M version "0.2" {type R=record {f:bool};type T=enum{ok};"#;
        for body in [
            "actor A{capacity 1; capability run()->T{duration fixed 1min;result nondet{ok}}} process P{a=A.run(R{f=true});end done}",
            "actor A{capacity 1; capability run()->T{duration fixed 1min;result nondet{ok}}} process P{end done} contract C{at_most_once A.run.finish(R{f=true}) per case.id}",
            "kernel K {* -> {1:ok with x=R{f=true}}} process P{end done}",
            "kernel K {R{f=true} -> {1:ok}} process P{end done}",
            "actor A{capacity 1;capability run()->R{duration fixed 1min;result nondet{R{f=true}}}} process P{fork f{a=A.run()} join any f where result == R{f=true} tie nondet remaining drain if none -> no;end done}",
        ] {
            let s = format!(
                "{prefix}{body} bounds{{cases 1;live_tasks 1;clock 2min}} verify safety no_resource_overcommit }}"
            );
            let ds = parse(&s).unwrap_err();
            assert_eq!(ds.len(), 1);
            assert_eq!(ds[0].code, diag::U_SYN_0, "{body}: {ds:?}");
            assert!(ds[0].message.contains("record"), "{body}: {ds:?}");
        }
    }
}

fn keyword(word: &str) -> bool {
    matches!(
        word,
        "model"
            | "version"
            | "type"
            | "enum"
            | "actor"
            | "capacity"
            | "capability"
            | "duration"
            | "fixed"
            | "result"
            | "nondet"
            | "cancel"
            | "cooperative"
            | "ack_within"
            | "forbidden_after_start"
            | "process"
            | "fork"
            | "if"
            | "else"
            | "end"
            | "join"
            | "all"
            | "any"
            | "where"
            | "tie"
            | "by"
            | "remaining"
            | "cancel_then_drain"
            | "drain"
            | "none"
            | "bounds"
            | "cases"
            | "live_tasks"
            | "clock"
            | "verify"
            | "safety"
            | "no_resource_overcommit"
            | "reachable"
            | "min"
    )
}

fn outside_keyword(word: &str) -> bool {
    matches!(
        word,
        "uses"
            | "resource"
            | "retry"
            | "boundary"
            | "grain"
            | "contains"
            | "overlaps"
            | "control"
            | "via"
            | "state"
            | "responsible"
            | "standing"
            | "configuration"
            | "scenario"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(items: &str) -> Model {
        parse(&format!("model M version \"0.2\" {{ {items} }}"))
            .unwrap_or_else(|diagnostics| panic!("{items}: {diagnostics:?}"))
    }

    fn error(src: &str, code: &str) -> Diagnostic {
        let diagnostics = parse(src).expect_err(src);
        assert!(!diagnostics.is_empty());
        let diagnostic = diagnostics.into_iter().next().unwrap();
        assert_eq!(diagnostic.code, code, "{src}: {diagnostic:?}");
        assert_eq!(diagnostic.severity, diag::Severity::Error);
        let span = diagnostic.span.expect("syntax errors have source spans");
        assert!(span.start <= span.end && span.end <= src.len());
        assert!(src.is_char_boundary(span.start) && src.is_char_boundary(span.end));
        diagnostic
    }

    fn item_error(items: &str, code: &str) {
        error(&format!("model M version \"0.2\" {{ {items} }}"), code);
    }

    #[test]
    fn line_breaks_do_not_change_the_unsupported_or_invalid_classification() {
        for (one_line, code) in [
            ("process P { a = A.c(x: y) }", diag::U_SYN_0),
            ("actor A { capability c(x) -> T {} }", diag::U_SYN_0),
            ("process P { a A.c() }", diag::E_SYN_0),
            ("process P { wait a }", diag::U_SYN_0),
        ] {
            item_error(one_line, code);
            // Every space becomes a line break; the class must not change.
            item_error(&one_line.replace(' ', "\n"), code);
            // A single break before or after each token boundary.
            for (i, _) in one_line.match_indices(['(', '.', ' ']) {
                for at in [i, i + 1] {
                    let mut split = one_line.to_string();
                    split.insert(at, '\n');
                    item_error(&split, code);
                }
            }
        }
    }

    #[test]
    fn cut_items_outside_adr_0007_or_malformed_are_classified() {
        item_error(
            "cut C { horizon receipt..resolution within 60hour }",
            diag::U_SYN_0,
        );
        item_error("cut C { scope [A] }", diag::U_SYN_0);
        for items in [
            "cut C { exclude B }",
            "cut C { exclude B reason }",
            "cut C { exclude B loses \"l\" }",
            "cut C { include [] }",
            "cut C { reentry [] }",
            "cut C { reentry [a, ] }",
            "cut C { reentry [a(b)] }",
            "cut C { reentry [(] }",
            "cut C { reentry [)] }",
            "cut C { reentry [a{b}] }",
            "cut C { reentry [a[b]] }",
            "cut C { externality [A.] }",
            "cut { include [A] }",
        ] {
            item_error(items, diag::E_SYN_0);
        }
        let parsed = model(
            "cut C { reentry [contest Vendor -> Clerk.recheck, review cut quarterly by cfo] }",
        );
        let Item::Cut(cut) = &parsed.items[0] else {
            panic!()
        };
        let CutItem::Reentry(entries) = &cut.items[0].value else {
            panic!()
        };
        let texts: Vec<_> = entries.iter().map(|e| e.value.as_str()).collect();
        assert_eq!(
            texts,
            vec![
                "contest Vendor -> Clerk.recheck",
                "review cut quarterly by cfo"
            ]
        );
    }

    #[test]
    fn unknown_words_at_keyword_choices_are_unsupported_but_broken_structure_is_invalid() {
        let cap = |item: &str| format!("actor A {{ capability c() -> T {{ {item} }} }}");
        let fork = "fork f { a = A.c() }";
        let any =
            |rest: &str| format!("process P {{ {fork} join any f where result == x {rest} }}");
        for items in [
            "metric m = elapsed".to_string(),
            "property p = x".into(),
            "actor A { reads ledger }".into(),
            cap("duration unknown"),
            cap("duration lognormal 3min"),
            cap("result dist { pass }"),
            cap("cancel deferred"),
            cap("timeout 3min"),
            "actor A { capability c(x) -> T {} }".into(),
            "process P { a = A.c(x: y) }".into(),
            "process P { wait a }".into(),
            format!("process P {{ {fork} join first f }}"),
            any("tie random remaining drain if none -> done"),
            any("tie nondet remaining abandon if none -> done"),
            "bounds { horizon 3min }".into(),
            "bounds { clock 1.5min }".into(),
            "verify safety no_double_payment".into(),
            "verify liveness end.done".into(),
        ] {
            item_error(&items, diag::U_SYN_0);
        }
        for items in [
            format!("process P {{ {fork} end done }}"),
            "process P { a = A.c( }".into(),
            "actor A { capability c(;) -> T {} }".into(),
            cap("duration 3min"),
            cap("duration fixed"),
            "verify safety".into(),
            "process P { a A.c() }".into(),
        ] {
            item_error(&items, diag::E_SYN_0);
        }
    }

    fn process(model: &Model) -> &Process {
        match &model.items[0] {
            Item::Process(process) => process,
            other => panic!("expected process, got {other:?}"),
        }
    }

    #[test]
    fn every_fixture_obeys_its_expected_syntax_diagnostics() {
        // New D1 syntax is asserted in excludes_waive; preserve this old corpus.
        const D1: [&str; 24] = [
            "ctx-excludes",
            "ctx-excludes.broken-start",
            "ctx-nonwaivable",
            "ctx-nonwaivable.broken-waive",
            "ctx-obligation",
            "ctx-obligation.broken-excludes-target",
            "ctx-waive",
            "ctx-waive-authority",
            "ctx-waive-authority.broken-no-authority",
            "ctx-waive-names",
            "ctx-waive-names.broken-unknown-response",
            "ctx-waive.broken-after-deadline",
            "ctx-contest",
            "ctx-contest-branches",
            "ctx-contest-branches.broken-zero-capacity",
            "ctx-contest-clock",
            "ctx-contest-clock.broken-short-clock",
            "ctx-contest-metrics",
            "ctx-contest-metrics.broken-missing-route-rate",
            "ctx-contest-origin",
            "ctx-contest-origin.broken-arrival-clock",
            "ctx-contest-resource",
            "ctx-contest-resource.broken-slow-route",
            "ctx-contest.broken-late",
        ];
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut count = 0;
        let mut diff_count = 0;
        let mut missing_join_seen = false;
        for entry in std::fs::read_dir(fixtures).unwrap() {
            let path = entry.unwrap().path();
            if !path.is_dir() || !path.join("model.relca").is_file() {
                continue;
            }
            if crate::quality_population::fixture(path.file_name().unwrap().to_str().unwrap())
                || crate::ir::rule_fixture(path.file_name().unwrap().to_str().unwrap())
                || crate::evaluation_population::fixture(
                    path.file_name().unwrap().to_str().unwrap(),
                )
                || crate::quantile_population::fixture(path.file_name().unwrap().to_str().unwrap())
            {
                continue;
            }
            if crate::contest::population::fixture(path.file_name().unwrap().to_str().unwrap()) {
                continue;
            }
            if crate::agency::second_half_fixture(path.file_name().unwrap().to_str().unwrap()) {
                continue;
            }
            if crate::mutation::extension_fixture(path.file_name().unwrap().to_str().unwrap()) {
                continue;
            }
            if crate::engine::policy_population::fixture(
                path.file_name().unwrap().to_str().unwrap(),
            ) || crate::engine::limits_population::fixture(
                path.file_name().unwrap().to_str().unwrap(),
            ) {
                continue;
            }
            if crate::scenario_population::fixture(path.file_name().unwrap().to_str().unwrap()) {
                continue;
            }
            if crate::ir::resource_fixture(path.file_name().unwrap().to_str().unwrap()) {
                continue;
            }
            if crate::engine::conflict_population::fixture(
                path.file_name().unwrap().to_str().unwrap(),
            ) {
                continue;
            }
            if crate::engine::caseset_population::fixture(
                path.file_name().unwrap().to_str().unwrap(),
            ) {
                let src = std::fs::read_to_string(path.join("model.relca")).unwrap();
                if let Err(ds) = parse(&src) {
                    let e: serde_json::Value = serde_json::from_str(
                        &std::fs::read_to_string(path.join("expected.json")).unwrap(),
                    )
                    .unwrap();
                    assert!(
                        e["diagnostics"].as_array().is_some_and(|es| es
                            .iter()
                            .any(|d| ds.iter().any(|x| d["code"] == x.code))),
                        "{}: {ds:?}",
                        path.display()
                    );
                }
                continue;
            }
            if D1.contains(&path.file_name().unwrap().to_str().unwrap()) {
                continue;
            }
            if crate::engine::actor_observed_population::fixture(
                path.file_name().unwrap().to_str().unwrap(),
            ) {
                let src = std::fs::read_to_string(path.join("model.relca")).unwrap();
                let parsed = parse(&src);
                if path.file_name().unwrap() == "aobs-reserved.broken-set" {
                    assert!(parsed.is_err_and(|ds| ds.iter().any(|d| d.code == diag::U_SYN_0)));
                } else {
                    assert!(parsed.is_ok(), "{}: {parsed:?}", path.display());
                }
                continue;
            }
            if crate::integer_tests::fixture(path.file_name().unwrap().to_str().unwrap()) {
                let src = std::fs::read_to_string(path.join("model.relca")).unwrap();
                let parsed = parse(&src);
                if path.file_name().unwrap() == "int-class-caseid.broken-reference" {
                    assert!(parsed.is_err_and(|ds| ds.iter().any(|d| d.code == diag::U_SYN_0)));
                } else {
                    assert!(parsed.is_ok(), "{}: {parsed:?}", path.display());
                }
                continue;
            }
            if crate::diff::extension_population::fixture(
                path.file_name().unwrap().to_str().unwrap(),
            ) {
                let src = std::fs::read_to_string(path.join("model.relca")).unwrap();
                assert!(parse(&src).is_ok(), "{}", path.display());
                diff_count += 1;
                continue;
            }
            if crate::engine::record_fixture(path.file_name().unwrap().to_str().unwrap()) {
                let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                let oracle: serde_json::Value =
                    serde_json::from_str(include_str!("../tests/support/record_oracle.json"))
                        .unwrap();
                let src = std::fs::read_to_string(path.join("model.relca")).unwrap();
                let parsed = parse(&src);
                if oracle[&name]["phase"] == "parse" {
                    assert!(parsed.is_err(), "{name}: expected parse refusal");
                } else {
                    assert!(parsed.is_ok(), "{name}: {parsed:?}");
                }
                continue;
            }
            // Close the new population, but still parse every valid new model.
            let wait_names: &[&str] = include!("../tests/support/waits_names.rs");
            if wait_names.contains(&path.file_name().unwrap().to_str().unwrap()) {
                continue;
            }
            if crate::state_invariant_population::fixture(
                path.file_name().unwrap().to_str().unwrap(),
            ) {
                continue;
            }
            crate::engine::authority_population::fixture(
                path.file_name().unwrap().to_str().unwrap(),
            );
            count += 1;
            let src = std::fs::read_to_string(path.join("model.relca")).unwrap();
            let expected: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path.join("expected.json")).unwrap())
                    .unwrap();
            // Expectations are partial reports. Diff fixtures may specify
            // diagnostics only on the selected sides, not at the root. Collect
            // all specified scopes; never skip a fixture or a syntax error.
            let scopes = [
                &expected,
                &expected["from"]["report"],
                &expected["to"]["report"],
                &expected["original"],
            ];
            let codes: Vec<_> = scopes
                .into_iter()
                .filter_map(|scope| scope.get("diagnostics"))
                .flat_map(|diagnostics| {
                    diagnostics
                        .as_array()
                        .expect("diagnostics expectation is an array")
                })
                .map(|diagnostic| {
                    diagnostic["code"]
                        .as_str()
                        .expect("diagnostic code is a string")
                })
                .filter(|code| [diag::E_SYN_0, diag::U_SYN_0].contains(code))
                .collect();
            let parsed = parse(&src);
            if crate::ir::per_case_fixture(path.file_name().unwrap().to_str().unwrap()) {
                assert!(parsed.is_ok(), "{}: {parsed:?}", path.display());
            }
            if crate::engine::requires_fixture(path.file_name().unwrap().to_str().unwrap()) {
                if path.file_name().unwrap() == "req-explicit-else.broken-no-else" {
                    assert!(
                        parsed
                            .as_ref()
                            .is_err_and(|ds| ds.iter().any(|d| d.code == diag::U_SYN_0))
                    );
                } else {
                    assert!(parsed.is_ok(), "{}: {parsed:?}", path.display());
                }
                continue;
            }
            if codes.is_empty() {
                assert!(parsed.is_ok(), "{}: {parsed:?}", path.display());
            } else {
                let diagnostics = parsed.expect_err(&path.display().to_string());
                for code in codes {
                    assert!(
                        diagnostics.iter().any(|diagnostic| diagnostic.code == code),
                        "{}: {diagnostics:?}",
                        path.display()
                    );
                }
            }
            if path.file_name().unwrap() == "parallel-time.broken-missing-join" {
                missing_join_seen = true;
            }
        }
        assert!(
            count >= 10,
            "expected at least the ten Phase 0 fixtures, saw {count}"
        );
        assert_eq!(diff_count, 38, "ADR 0031 is a separate closed population");
        assert!(
            missing_join_seen,
            "missing-join fault fixture was not visited"
        );
    }

    #[test]
    fn declarations_preserve_order_omissions_and_duplicates() {
        let parsed = model(
            "type T = enum { x, y, x }; type T = enum { z }
            actor A { capacity 2; capacity 0; capability c() -> Missing {}
                capability c() -> T { duration fixed 3min; result nondet { y, x, y };
                    duration fixed 5 min; cancel cooperative ack_within 1min;
                    cancel forbidden_after_start } }
            actor Empty {} process P {} process P {}
            bounds { cases 2; cases 1; live_tasks 0; clock 0min }
            bounds { clock 8 hour }
            verify safety no_resource_overcommit; verify reachable end.unknown",
        );
        assert_eq!(parsed.items.len(), 10);
        let Item::Type(decl) = &parsed.items[0] else {
            panic!()
        };
        assert_eq!(
            decl.members
                .iter()
                .map(|name| name.value.as_str())
                .collect::<Vec<_>>(),
            ["x", "y", "x"]
        );
        let Item::Actor(actor) = &parsed.items[2] else {
            panic!()
        };
        assert_eq!(actor.items.len(), 4);
        assert!(matches!(&actor.items[0], ActorItem::Capacity(value) if value.value == 2));
        assert!(matches!(&actor.items[1], ActorItem::Capacity(value) if value.value == 0));
        let ActorItem::Capability(empty) = &actor.items[2] else {
            panic!()
        };
        assert_eq!(empty.return_type.value, "Missing");
        assert!(empty.items.is_empty());
        let ActorItem::Capability(cap) = &actor.items[3] else {
            panic!()
        };
        assert_eq!(cap.items.len(), 5);
        assert!(matches!(&cap.items[0].value, CapItem::DurationFixed(d)
            if d.amount.value == 3 && d.unit.value == "min"));
        assert!(matches!(&cap.items[2].value, CapItem::DurationFixed(d)
            if d.amount.value == 5 && d.unit.value == "min"));
        assert!(
            matches!(&cap.items[3].value, CapItem::Cancel(Cancel::Cooperative { ack_within })
            if ack_within.amount.value == 1)
        );
        assert!(matches!(
            &cap.items[4].value,
            CapItem::Cancel(Cancel::ForbiddenAfterStart)
        ));
        let Item::Bounds(bounds) = &parsed.items[6] else {
            panic!()
        };
        assert_eq!(bounds.items.len(), 4);
        let Item::Bounds(other) = &parsed.items[7] else {
            panic!()
        };
        assert!(matches!(&other.items[0].value, Bound::Clock(d) if d.unit.value == "hour"));
        assert!(matches!(
            parsed.items[8],
            Item::Verify(Spanned {
                value: Property::NoResourceOvercommit,
                ..
            })
        ));
        assert!(
            matches!(&parsed.items[9], Item::Verify(Spanned { value: Property::ReachableEnd(name), .. })
            if name.value == "unknown")
        );
    }

    #[test]
    fn empty_model_and_unresolved_names_are_syntax_not_semantics() {
        assert!(model("").items.is_empty());
        let parsed = model(
            "process Unknown { missing = ActorUndefined.capUndefined()
            if missing == NotAValue { end NotAnEnd } else { end AlsoUnknown }
            later = Another.missing() }",
        );
        let body = &process(&parsed).body;
        assert_eq!(body.len(), 3);
        assert!(
            matches!(&body[0].value, Stmt::Call(call) if call.binding.value == "missing"
            && call.actor.value == "ActorUndefined" && call.capability.value == "capUndefined")
        );
    }

    #[test]
    fn all_and_any_joins_retain_every_clause() {
        for (tie, remaining) in [("by [b, a]", "cancel_then_drain"), ("nondet", "drain")] {
            let parsed = model(&format!("process P {{
                fork f {{ a = A.c(); b = B.d() }}; join all other
                fork g {{ x = C.e() }}
                join any unknown where result == Value; tie {tie}; remaining {remaining}; if none -> absent
                end done }}"));
            let body = &process(&parsed).body;
            assert_eq!(body.len(), 3);
            let Stmt::Fork {
                name,
                branches,
                join,
            } = &body[0].value
            else {
                panic!()
            };
            assert_eq!(name.value, "f");
            assert_eq!(branches.len(), 2);
            assert!(matches!(&join.value, Join::All { fork } if fork.value == "other"));
            let Stmt::Fork { branches, join, .. } = &body[1].value else {
                panic!()
            };
            assert_eq!(branches.len(), 1);
            let Join::Any {
                fork,
                result,
                tie: parsed_tie,
                remaining: parsed_remaining,
                if_none,
            } = &join.value
            else {
                panic!()
            };
            assert_eq!(fork.value, "unknown");
            assert_eq!(result.value, "Value");
            assert_eq!(if_none.value, "absent");
            if tie == "nondet" {
                assert_eq!(parsed_tie.value, Tie::Nondet);
                assert_eq!(parsed_remaining.value, Remaining::Drain);
            } else {
                let Tie::By(names) = &parsed_tie.value else {
                    panic!()
                };
                assert_eq!(
                    names
                        .iter()
                        .map(|name| name.value.as_str())
                        .collect::<Vec<_>>(),
                    ["b", "a"]
                );
                assert_eq!(parsed_remaining.value, Remaining::CancelThenDrain);
            }
        }
    }

    #[test]
    fn fork_requires_an_immediately_following_join() {
        for continuation in ["", "end done", "a = A.c()", "if a == x {} else {}"] {
            item_error(
                &format!("process P {{ fork f {{ a = A.c() }} {continuation} }}"),
                diag::E_SYN_0,
            );
        }
        item_error("process P { join all f }", diag::E_SYN_0);
        item_error(
            "process P { fork f { end done } join all f }",
            diag::E_SYN_0,
        );
    }

    #[test]
    fn grouped_conditions_preserve_tree_without_implicit_precedence() {
        let parsed = model(
            "process P {
            if a == x && (b == y || (c == z)) && d == w {
                if a == x {} else { end nested }
            } else { end failed }
            if a == x || b == y || c == z {} else {}
        }",
        );
        let Stmt::If {
            condition,
            then_body,
            ..
        } = &process(&parsed).body[0].value
        else {
            panic!()
        };
        let Cond::And(operands) = &condition.value else {
            panic!()
        };
        assert_eq!(operands.len(), 3);
        assert!(matches!(operands[0].value, Cond::Eq { .. }));
        let Cond::Group(inner) = &operands[1].value else {
            panic!()
        };
        let Cond::Or(inner_operands) = &inner.value else {
            panic!()
        };
        assert_eq!(inner_operands.len(), 2);
        assert!(matches!(inner_operands[1].value, Cond::Group(_)));
        assert!(matches!(then_body[0].value, Stmt::If { .. }));
        let Stmt::If { condition, .. } = &process(&parsed).body[1].value else {
            panic!()
        };
        assert!(matches!(&condition.value, Cond::Or(operands) if operands.len() == 3));
    }

    #[test]
    fn unparenthesized_mixed_boolean_operators_are_invalid() {
        for condition in [
            "a == x && b == y || c == z",
            "a == x || b == y && c == z",
            "(a == x && b == y || c == z)",
            "a == x && (b == y || c == z && d == w)",
        ] {
            item_error(
                &format!("process P {{ if {condition} {{}} else {{}} }}"),
                diag::E_SYN_0,
            );
        }
    }

    #[test]
    fn empty_required_lists_missing_clauses_and_delimiters_are_invalid() {
        for items in [
            "type T = enum {}",
            "type T = enum { x, }",
            "type T = enum { x y }",
            "actor A { capability c() -> T { result nondet {} } }",
            "actor A { capability c() -> T { result nondet { x, } } }",
            "process P { fork f {} join all f }",
            "bounds {}",
            "bounds { cases 1 clock 2min }",
            "process P { if a == x {} }",
            "process P { if (a == x {} else {} }",
            "process P { if a == {} else {} }",
            "process P { fork f { a = A.c() } join any f where result == x tie by [] remaining drain if none -> done }",
            "process P { fork f { a = A.c() } join any f where result == x tie by [a,] remaining drain if none -> done }",
            "process P { fork f { a = A.c() } join any f where result == x remaining drain if none -> done }",
            "process P { fork f { a = A.c() } join any f where result == x tie nondet if none -> done }",
            "process P { fork f { a = A.c() } join any f where result == x tie nondet remaining drain }",
            "process P { fork f { a = A.c() } join any f where result == x tie nondet remaining drain if none done }",
        ] {
            item_error(items, diag::E_SYN_0);
        }
    }

    #[test]
    fn separators_and_continued_join_clauses_accept_newlines_or_semicolons() {
        let newline = "model M version \"0.2\" {
            type T = enum { x }
            actor A { capacity 2
                capability c() -> T { duration fixed 3 min
                    result nondet { x }
                    cancel forbidden_after_start
                }
            }
            process P { fork f { a = A.c()
                b = A.c()
            }
            join any f where result == x
                tie nondet
                remaining drain
                if none -> missing
            end done
            }
            bounds { cases 1
                live_tasks 2
                clock 3min }
            verify reachable end.done
        }";
        let semicolon = newline.replace('\n', ";");
        // Newlines inside structural braces are item separators, not required
        // between the words of a production. These sources only use that subset.
        assert!(parse(newline).is_ok());
        assert!(parse(&semicolon).is_ok());
        assert!(parse(&newline.replace('\n', "\r\n")).is_ok());
        item_error("actor A { capability c(;) -> T {} }", diag::E_SYN_0);
    }

    #[test]
    fn non_min_units_and_out_of_range_semantic_values_reach_lower() {
        let parsed = model(
            "actor A { capacity 0; capability c() -> T {
            duration fixed 4sec; cancel cooperative ack_within 0 hour } }
            bounds { cases 99; live_tasks 0; clock 5days }",
        );
        let Item::Actor(actor) = &parsed.items[0] else {
            panic!()
        };
        let ActorItem::Capability(cap) = &actor.items[1] else {
            panic!()
        };
        assert!(matches!(&cap.items[0].value, CapItem::DurationFixed(d) if d.unit.value == "sec"));
        assert!(
            matches!(&cap.items[1].value, CapItem::Cancel(Cancel::Cooperative { ack_within })
            if ack_within.amount.value == 0 && ack_within.unit.value == "hour")
        );
        let Item::Bounds(bounds) = &parsed.items[1] else {
            panic!()
        };
        assert!(matches!(&bounds.items[2].value, Bound::Clock(d) if d.unit.value == "days"));
    }

    #[test]
    fn unsupported_keywords_report_u_syn_even_before_lexically_invalid_payloads() {
        for item in [
            "uses slots: 1",
            "outcomes { 1.0: pass }",
            "executors { a }",
            "configuration {}",
            "members [p]",
            "retry 3",
            "boundary \"actor\"",
            "latent truth: T",
        ] {
            item_error(item, diag::U_SYN_0);
        }
        // ADR 0013 addendum U8: this newly admitted position requires a row.
        item_error("kernel K {}", diag::E_SYN_0);
        for item in [
            "members [p]",
            "outcomes { 1.0: pass }",
            "kernel K {}",
            "retry 2",
        ] {
            item_error(
                &format!("actor A {{ capability c() -> T {{ {item} }} }}"),
                diag::U_SYN_0,
            );
        }
        item_error("actor A { executors { a } }", diag::U_SYN_0);
        item_error("process P { retry 3 }", diag::U_SYN_0);
        // A keyword substring is an ordinary unresolved name, not unsupported.
        assert_eq!(
            model("actor outcomes_backup {} actor resource2 {}")
                .items
                .len(),
            2
        );
    }

    #[test]
    fn integers_are_exact_u64_and_never_wrap_or_default() {
        let parsed = model("actor A { capacity 18446744073709551615 }");
        let Item::Actor(actor) = &parsed.items[0] else {
            panic!()
        };
        assert!(matches!(&actor.items[0], ActorItem::Capacity(n) if n.value == u64::MAX));
        for number in ["-1", "1.5", "+1"] {
            item_error(&format!("actor A {{ capacity {number} }}"), diag::E_SYN_0);
        }
        for number in ["-1", "+1"] {
            item_error(&format!("bounds {{ clock {number}min }}"), diag::E_SYN_0);
        }
        // Beyond u64 is an implementation limit like the nesting limit, and a
        // decimal duration is a v0.2 form outside ADR 0001: unsupported.
        for number in ["18446744073709551616", "999999999999999999999999999999999"] {
            item_error(&format!("actor A {{ capacity {number} }}"), diag::U_SYN_0);
            item_error(&format!("bounds {{ clock {number}min }}"), diag::U_SYN_0);
        }
        item_error("bounds { clock 1.5min }", diag::U_SYN_0);
    }

    #[test]
    fn entire_source_is_consumed_and_truncated_input_is_not_accepted() {
        let valid = "model M version \"0.2\" {}";
        for suffix in ["garbage", "model N version \"0.2\" {}", "@", "}"] {
            error(&format!("{valid} {suffix}"), diag::E_SYN_0);
        }
        error(&format!("{valid} configuration {{}}"), diag::U_SYN_0);
        assert!(parse(&format!(";\n{valid};\n")).is_ok());
        for src in [
            "",
            "model",
            "model M",
            "model M version",
            "model M version \"0.2\" {",
            "model M version \"0.2\" { actor A {",
            "model M version \"unfinished",
            "model M version \"0.2\" { process P { a = A.c(",
            "model M version \"0.2\" { bounds { clock 3",
            "not a Relca model",
        ] {
            error(src, diag::E_SYN_0);
        }
    }

    #[test]
    fn source_spans_use_original_utf8_bytes_and_exclude_separators() {
        let src = "\nmodel M version \"版0.2\" {\nprocess P { a = A.c();\nend done }\n}\n";
        let parsed = parse(src).unwrap();
        assert_eq!(&src[parsed.span.start..parsed.span.end], src.trim());
        assert_eq!(parsed.version.value, "版0.2");
        assert_eq!(
            &src[parsed.version.span.start..parsed.version.span.end],
            "\"版0.2\""
        );
        let Item::Process(process) = &parsed.items[0] else {
            panic!()
        };
        assert_eq!(
            &src[process.body[0].span.start..process.body[0].span.end],
            "a = A.c()"
        );
        assert_eq!(
            &src[process.body[1].span.start..process.body[1].span.end],
            "end done"
        );
        let bad = "model M version \"版\" { @ }";
        let span = error(bad, diag::E_SYN_0).span.unwrap();
        assert_eq!(&bad[span.start..span.end], "@");
        assert_eq!(span.start, bad.find('@').unwrap());
        let eof = "model M version \"版\" {";
        assert_eq!(
            error(eof, diag::E_SYN_0).span,
            Some(Span {
                start: eof.len(),
                end: eof.len()
            })
        );
    }

    #[test]
    fn undecided_lexical_extensions_are_explicitly_unsupported() {
        for src in [
            "model M version \"escaped\\n\" {}",
            "model 日本 version \"0.2\" {}",
            "// comment\nmodel M version \"0.2\" {}",
            "model M version \"0.2\" { /* ignored? */ }",
        ] {
            error(src, diag::U_SYN_0);
        }
    }

    #[test]
    fn nesting_limit_returns_a_diagnostic_instead_of_overflowing_the_stack() {
        let condition = format!("{}a == x{}", "(".repeat(200), ")".repeat(200));
        item_error(
            &format!("process P {{ if {condition} {{}} else {{}} }}"),
            diag::U_SYN_0,
        );
        let statements = format!(
            "{}end done{}",
            "if a == x {".repeat(200),
            "} else {}".repeat(200)
        );
        item_error(&format!("process P {{ {statements} }}"), diag::U_SYN_0);
    }

    #[test]
    fn token_truncations_and_small_arbitrary_inputs_never_panic() {
        let src = include_str!("../../../fixtures/coop-cancel/model.relca");
        for (offset, _) in src.char_indices() {
            let _ = parse(&src[..offset]);
        }
        let alphabet = [
            "{", "}", "(", ")", "\"", "\\", "\n", ";", "a", "0", "é", "\0", "&", "=",
        ];
        for first in alphabet {
            for second in alphabet {
                for third in alphabet {
                    let _ = parse(&format!("{first}{second}{third}"));
                }
            }
        }
    }
}
mod work_profiles;
