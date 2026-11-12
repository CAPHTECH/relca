//! Byte-spanned lexing. Even bad tokens are retained so an earlier parser error
//! (notably an unsupported keyword) is not hidden by a later lexical error.

use crate::diag::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Word,
    Integer,
    String,
    Symbol,
    Invalid,
    Unsupported,
    Eof,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Token<'s> {
    pub kind: Kind,
    pub text: &'s str,
    pub span: Span,
}

pub(super) fn lex(src: &str) -> Vec<Token<'_>> {
    let mut chars = src.char_indices().peekable();
    let mut tokens = Vec::new();
    while let Some((start, ch)) = chars.next() {
        if matches!(ch, ' ' | '\t' | '\r') {
            continue;
        }
        let mut end = start + ch.len_utf8();
        let kind = if ch.is_ascii_alphabetic() || ch == '_' {
            while let Some(&(offset, next)) = chars.peek() {
                if !next.is_ascii_alphanumeric() && next != '_' {
                    break;
                }
                chars.next();
                end = offset + next.len_utf8();
            }
            Kind::Word
        } else if ch.is_ascii_digit() {
            while let Some(&(offset, next)) = chars.peek() {
                if !next.is_ascii_digit() {
                    break;
                }
                chars.next();
                end = offset + 1;
            }
            Kind::Integer
        } else if ch == '"' {
            let mut closed = false;
            let mut escaped = false;
            let mut unsupported = false;
            let mut multiline = false;
            for (offset, next) in chars.by_ref() {
                end = offset + next.len_utf8();
                if next == '"' && !escaped {
                    closed = true;
                    break;
                }
                multiline |= matches!(next, '\n' | '\r') || next.is_control();
                if next == '\\' && !escaped {
                    escaped = true;
                    unsupported = true;
                } else {
                    escaped = false;
                }
            }
            if !closed || multiline {
                Kind::Invalid
            } else if unsupported {
                Kind::Unsupported
            } else {
                Kind::String
            }
        } else if matches!(ch, '-' | '=' | '&' | '|' | '/')
            && chars.peek().is_some_and(|&(_, next)| {
                matches!(
                    (ch, next),
                    ('-', '>') | ('=', '=') | ('&', '&') | ('|', '|') | ('/', '/') | ('/', '*')
                )
            })
        {
            // The peek condition guarantees a second ASCII character exists.
            chars.next();
            end += 1;
            if ch == '/' {
                Kind::Unsupported
            } else {
                Kind::Symbol
            }
        } else if "{}()[],.;=:\n<>~*".contains(ch) {
            Kind::Symbol
        } else if !ch.is_ascii() {
            Kind::Unsupported
        } else {
            Kind::Invalid
        };
        tokens.push(Token {
            kind,
            text: &src[start..end],
            span: Span { start, end },
        });
    }
    tokens.push(Token {
        kind: Kind::Eof,
        text: "",
        span: Span {
            start: src.len(),
            end: src.len(),
        },
    });
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_duration_operators_and_utf8_spans() {
        let src = "3min 3 min -> == && || \"版\" あ";
        let tokens = lex(src);
        let spellings: Vec<_> = tokens.iter().map(|token| token.text).collect();
        assert_eq!(
            spellings,
            [
                "3", "min", "3", "min", "->", "==", "&&", "||", "\"版\"", "あ", ""
            ]
        );
        for token in &tokens {
            assert_eq!(&src[token.span.start..token.span.end], token.text);
        }
        assert_eq!(tokens[8].kind, Kind::String);
        assert_eq!(tokens[9].kind, Kind::Unsupported);
    }

    #[test]
    fn invalid_strings_and_unsettled_lexical_forms_are_not_ignored() {
        for src in ["\"unfinished", "\"line\nbreak\"", "@", "&", "-"] {
            assert_eq!(lex(src)[0].kind, Kind::Invalid, "{src:?}");
        }
        for src in ["\"escaped\\n\"", "// comment", "/* comment */"] {
            assert_eq!(lex(src)[0].kind, Kind::Unsupported, "{src:?}");
        }
        assert_eq!(lex("")[0].kind, Kind::Eof);
    }
}
