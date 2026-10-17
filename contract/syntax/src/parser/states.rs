//! A state part (LLP 1121 D8, stage 8 A): `:active`, `:hover`,
//! `:focus-visible`, `:disabled` or `:checked` on its own line, then an
//! indented block of the state's rows, in a `style` (literal) or under a
//! node (computed, as its own attributes are). Each row is carried as an
//! attribute named for the state and the property (`:disabled
//! background-color`, [`state_attr`]), so a class's and the node's merge as
//! any row does; which rows a state takes, and on which nodes, is the
//! lowering's.

use super::*;
use crate::state::{state_attr, STATES};

impl Parser {
    /// Whether the line starts a state part: `:` and a name (`::` is a part).
    pub(super) fn at_state(&self) -> bool {
        matches!(
            (self.peek_kind(), self.peek2()),
            (TokenKind::Punct(":"), TokenKind::Ident(_))
        )
    }

    /// `:state`, through its block, its rows added to `attrs`; `literal` in
    /// a `style`. `owner` names the style or element.
    pub(super) fn state_part(
        &mut self,
        owner: &str,
        literal: bool,
        attrs: &mut Vec<Attr>,
    ) -> R<()> {
        let colon = self.next().span;
        let at = self.next();
        let TokenKind::Ident(name) = at.kind.clone() else {
            unreachable!("at_state saw a name");
        };
        let span = Span {
            end_col: at.span.end_col,
            ..colon
        };
        let state = format!(":{name}");
        if !STATES.contains(&name.as_str()) {
            return Err(SyntaxError {
                id: "syntax-state",
                message: format!(
                    "`{state}` is not a state Contract has; a state part is one of {} (LLP 1121 D8)",
                    STATES.iter().map(|s| format!("`:{s}`")).collect::<Vec<_>>().join(", ")
                ),
                span,
            });
        }
        if matches!(self.peek_kind(), TokenKind::Punct(":")) {
            return Err(SyntaxError {
                id: "syntax-state",
                message: format!("`{state}` stands alone: a state part names one state, never a combination (LLP 1121 D8)"),
                span,
            });
        }
        if attrs
            .iter()
            .any(|a| a.name.starts_with(&format!("{state} ")))
        {
            return Err(SyntaxError {
                id: "syntax-state",
                message: format!("a second `{state}` in {owner}: write its rows in one block"),
                span,
            });
        }
        self.newline()?;
        let within = format!("{owner} {state}");
        let mut declared = Vec::new();
        self.block(|p| {
            if p.at_state() || p.at_part() {
                return p.err(
                    "syntax-state",
                    format!("a block in `{state}`: a state part holds rows only, never another state or part (LLP 1121 D8)"),
                );
            }
            if literal {
                declared.extend(p.literal_line(&within)?);
            } else {
                let mut positional = Vec::new();
                p.attr_line(&mut positional, &mut declared, true)?;
                p.newline()?;
            }
            Ok(())
        })?;
        if declared.is_empty() {
            return Err(SyntaxError {
                id: "syntax-empty-block",
                message: format!(
                    "`{state}` needs its rows indented under it: `background-color = \"accent.fill-pressed\"`"
                ),
                span,
            });
        }
        for a in declared {
            let name = state_attr(&state, &a.name);
            if attrs.iter().any(|b| b.name == name) {
                return Err(SyntaxError {
                    id: "syntax-duplicate-attr",
                    message: format!("`{}` appears twice in `{state}` of {owner}", a.name),
                    span: a.span,
                });
            }
            attrs.push(Attr { name, ..a });
        }
        Ok(())
    }
}
