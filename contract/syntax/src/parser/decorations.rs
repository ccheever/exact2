//! Named textarea decoration rows (LLP 1114.007).
use super::*;

impl Parser {
    pub(super) fn decoration(&mut self) -> R<StyleDecl> {
        let span = self.expect_word("decoration")?;
        let name = self.named_ident(span)?;
        let owner = format!("decoration {name}");
        let attrs = if matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Eof) {
            self.newline()?;
            self.block(|parser| parser.literal_line(&owner))?
                .into_iter()
                .flatten()
                .collect()
        } else {
            self.literal_line(&owner)?
        };
        unique_attrs(&attrs, &owner)?;
        Ok(StyleDecl { name, attrs, span })
    }

    /// `atom <name> link="scheme:" | link="relative" | prefix="@"`, with an
    /// optional `role` and style, then optionally an indented block of more
    /// rows and the `:hover` and `:checked` state parts (LLP 1045.000 D3);
    /// which rows it takes is the lowerer's.
    pub(super) fn atom(&mut self) -> R<StyleDecl> {
        let span = self.expect_word("atom")?;
        let name = self.named_ident(span)?;
        let owner = format!("atom {name}");
        let mut attrs = self.literal_line(&owner)?;
        self.block(|p| {
            if p.at_state() {
                p.state_part(&format!("`{owner}`"), true, &mut attrs)
            } else {
                attrs.extend(p.literal_line(&owner)?);
                Ok(())
            }
        })?;
        unique_attrs(&attrs, &owner)?;
        Ok(StyleDecl { name, attrs, span })
    }
}
