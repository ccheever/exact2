//! `color-profile --name src="…" rendering-intent="…"`: CSS Color 5's
//! `@color-profile` on one line (LLP 1100 D3).

use super::*;

impl Parser {
    pub(super) fn color_profile(&mut self) -> R<ColorProfileDecl> {
        let span = self.expect_word("color-profile")?;
        let dashed = self.eat_punct("-") && self.eat_punct("-");
        let name = match (dashed, self.peek_kind().clone()) {
            (true, TokenKind::Ident(n)) => {
                self.next();
                format!("--{n}")
            }
            (_, other) => {
                return self.err(
                    "syntax-color-profile-name",
                    format!(
                        "a color profile's name is a dashed ident, as CSS's: `--brand`, found {}",
                        describe(&other)
                    ),
                )
            }
        };
        let attrs = self.literal_line(&format!("color-profile {name}"))?;
        unique_attrs(&attrs, &format!("color-profile {name}"))?;
        Ok(ColorProfileDecl { name, attrs, span })
    }
}
