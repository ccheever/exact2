//! r6's spelling of a component's parts (LLP 1139 D2; the spelling is §9
//! Q7's, `crate::anatomy`): a `parts` section, `name: group group …` a
//! line.

use super::{Parser, R};
use crate::anatomy::PartDecl;

impl Parser {
    /// `parts` and its block of parts.
    pub(super) fn parts_section(&mut self) -> R<Vec<PartDecl>> {
        let section = self.next().span;
        self.newline()?;
        let list = self.required_block(section, "a `parts` section", |p| p.part_decl())?;
        let mut seen: Vec<&PartDecl> = Vec::new();
        for part in &list {
            if let Some(first) = seen.iter().find(|s| s.name == part.name) {
                return super::duplicate("part", &part.name, part.span, first.span);
            }
            seen.push(part);
        }
        Ok(list)
    }

    /// `surface: paint shape space`.
    fn part_decl(&mut self) -> R<PartDecl> {
        let (name, span) = self.field_name()?;
        self.expect_punct(":")?;
        let mut groups = Vec::new();
        while let crate::lexer::TokenKind::Ident(_) = self.peek_kind() {
            groups.push(self.ident()?);
        }
        if groups.is_empty() {
            return self.err(
                "syntax-part-groups",
                format!("part `{name}` names the row groups it takes (`{name}: paint space`)"),
            );
        }
        self.newline()?;
        Ok(PartDecl { name, groups, span })
    }
}
