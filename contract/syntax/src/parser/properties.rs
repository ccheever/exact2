//! Properties use literal test values, never application expressions.

use super::*;

impl Parser {
    pub(super) fn property_decl(&mut self) -> R<PropertyDecl> {
        let token = self.next();
        let TokenKind::Ident(word) = token.kind else {
            unreachable!()
        };
        let kind = match word.as_str() {
            "always" => PropertyKind::Always,
            "after" if self.at_ident("clock") => {
                self.next();
                PropertyKind::Clock
            }
            "after" => PropertyKind::After(self.str_lit("the action's name")?),
            "never" => {
                self.expect_word("refuses")?;
                PropertyKind::NeverRefuses
            }
            _ => unreachable!(),
        };
        let name = self.str_lit("the property's name")?;
        self.newline()?;
        let lines = if kind == PropertyKind::NeverRefuses {
            Vec::new()
        } else {
            self.block(|p| p.property_line(&kind))?
        };
        Ok(PropertyDecl {
            span: token.span,
            kind,
            name,
            lines,
        })
    }

    fn property_line(&mut self, kind: &PropertyKind) -> R<PropertyLine> {
        let span = self.peek().span;
        let after = matches!(kind, PropertyKind::After(_) | PropertyKind::Clock);
        let form = if self.at_ident("expect") {
            self.next();
            self.expect_word("state")?;
            let (name, _) = self.ident()?;
            if self.at_punct(".") || self.at_punct("[") {
                return self.err(
                    "syntax-property-name",
                    "a property names a state or a derive by its bare name; fields are stage 2",
                );
            }
            let test = if self.eat_punct("==") {
                if self.at_ident("old") || self.at_ident("not") {
                    if !after {
                        return self.err(
                            "syntax-property-old",
                            "`old` is admitted only inside `after`",
                        );
                    }
                    if self.at_ident("not") {
                        self.next();
                        self.expect_word("old")?;
                        PropertyTest::NotOld
                    } else {
                        self.next();
                        if self.eat_punct("+") {
                            PropertyTest::Add(self.step_number("a number after `old +`")?)
                        } else if self.eat_punct("-") {
                            PropertyTest::Subtract(self.step_number("a number after `old -`")?)
                        } else {
                            PropertyTest::Old
                        }
                    }
                } else {
                    PropertyTest::Equal(self.property_value(false)?)
                }
            } else if self.at_ident("in") {
                self.next();
                let mut values = Vec::new();
                while !self.property_line_end() {
                    values.push(self.property_value(true)?);
                }
                PropertyTest::In(values)
            } else if self.eat_punct(">=") {
                PropertyTest::AtLeast(self.step_number("a lower bound")?)
            } else if self.eat_punct("<=") {
                PropertyTest::AtMost(self.step_number("an upper bound")?)
            } else if self.at_ident("whole") {
                self.next();
                PropertyTest::Whole
            } else if self.at_ident("stays") {
                if after {
                    return self.err(
                        "syntax-property-stays",
                        "`stays` is admitted only inside `always`",
                    );
                }
                self.next();
                PropertyTest::Stays(self.property_value(false)?)
            } else {
                return self.err(
                    "syntax-property-form",
                    "a property tests `==`, `in`, `>=`, `<=`, `whole`, or `stays`",
                );
            };
            PropertyForm::Hold { name, test }
        } else {
            if !after {
                return self.err("syntax-property-frame", "frame lines are admitted only inside `after`; `always` takes `expect state` lines");
            }
            let (word, _) = self.ident()?;
            match word.as_str() {
                "only" => {
                    self.expect_word("writes")?;
                    let mut names = Vec::new();
                    while !self.property_line_end() {
                        names.push(self.ident()?.0);
                    }
                    PropertyForm::OnlyWrites(names)
                }
                "writes" => {
                    self.expect_word("none")?;
                    PropertyForm::OnlyWrites(Vec::new())
                }
                "command" => {
                    let (name, _) = self.ident()?;
                    let mut args = Vec::new();
                    while !self.property_line_end() {
                        args.push(self.property_value(false)?);
                    }
                    PropertyForm::Command { name, args }
                }
                _ => return Err(SyntaxError {
                    id: "syntax-property-form",
                    message: "a property takes `expect state`, `only writes`, `writes none`, or `command`".into(),
                    span,
                }),
            }
        };
        self.newline()?;
        Ok(PropertyLine { span, form })
    }

    fn property_line_end(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof
        )
    }

    fn property_value(&mut self, membership: bool) -> R<PropertyValue> {
        if self.at_punct("-") || matches!(self.peek_kind(), TokenKind::Number(_)) {
            return self.step_number("a test value").map(PropertyValue::Number);
        }
        let token = self.next();
        Ok(match token.kind {
            TokenKind::Str(s) => PropertyValue::String(s),
            TokenKind::Ident(w) if w == "true" || w == "false" => PropertyValue::Bool(w == "true"),
            TokenKind::Ident(w) if w == "none" => PropertyValue::None,
            TokenKind::Ident(w) if membership && w == "some" => PropertyValue::Some,
            TokenKind::Punct("[") => {
                self.expect_punct("]")?;
                PropertyValue::EmptyList
            }
            _ => return Err(SyntaxError {
                id: "syntax-property-value",
                message: "a property value is a number, string, bool, `none`, or `[]`; `in` also admits `some`".into(),
                span: token.span,
            }),
        })
    }
}
