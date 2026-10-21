//! A component's `params` section (LLP 1121 D15): `name: kind = "default"`
//! and its attributes, one param a line.

use super::{Parser, R};
use crate::lexer::TokenKind;
use crate::params::{ComponentParam, ParamAttr, ParamValue, ATTRIBUTES};

impl Parser {
    /// `params` and its block of params.
    pub(super) fn params_section(&mut self) -> R<Vec<ComponentParam>> {
        let section = self.next().span;
        self.newline()?;
        let list = self.required_block(section, "a `params` section", |p| p.param())?;
        let mut seen: Vec<&ComponentParam> = Vec::new();
        for param in &list {
            if let Some(first) = seen.iter().find(|s| s.name == param.name) {
                return super::duplicate("param", &param.name, param.span, first.span);
            }
            seen.push(param);
        }
        Ok(list)
    }

    /// `radius: length = "radius.md"  range="0..40"  step=1`.
    fn param(&mut self) -> R<ComponentParam> {
        // A field's name, a reserved word too (`if: color = …`).
        let (name, span) = self.field_name()?;
        self.expect_punct(":")?;
        // A kind with a hyphen (`box-shadow`) is one word.
        let (mut kind, mut end) = self.ident()?;
        while self.at_punct("-")
            && self.peek().span.line == end.line
            && self.peek().span.col == end.end_col
        {
            self.next();
            let (more, at) = self.ident()?;
            kind = format!("{kind}-{more}");
            end = at;
        }
        self.expect_punct("=")?;
        let TokenKind::Str(default) = self.peek_kind().clone() else {
            return self.err(
                "syntax-param-default",
                format!("param `{name}`'s default is a string: a token's value, as a row writes it (`\"radius.md\"`)"),
            );
        };
        let default_span = self.next().span;
        let mut attrs: Vec<ParamAttr> = Vec::new();
        while let TokenKind::Ident(attr) = self.peek_kind().clone() {
            let at = self.next().span;
            if !ATTRIBUTES.contains(&attr.as_str()) {
                return Err(super::SyntaxError {
                    id: "syntax-param-attribute",
                    message: format!(
                        "param `{name}`: `{attr}` is no param attribute ({})",
                        ATTRIBUTES.join(", ")
                    ),
                    span: at,
                });
            }
            if attrs.iter().any(|a| a.name == attr) {
                return Err(super::SyntaxError {
                    id: "syntax-duplicate-declaration",
                    message: format!("param `{name}` says `{attr}` twice"),
                    span: at,
                });
            }
            self.expect_punct("=")?;
            let value = match self.peek_kind().clone() {
                TokenKind::Str(s) => ParamValue::Str(s),
                TokenKind::Number(n) => ParamValue::Number(n),
                _ => {
                    return self.err(
                        "syntax-param-attribute",
                        format!("param `{name}`'s `{attr}` is a string or a number"),
                    )
                }
            };
            self.next();
            attrs.push(ParamAttr {
                name: attr,
                value,
                span: at,
            });
        }
        self.newline()?;
        Ok(ComponentParam {
            name,
            kind,
            default,
            default_span,
            attrs,
            span,
        })
    }
}
