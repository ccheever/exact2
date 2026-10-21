//! `color-profile --name src="…" rendering-intent="…"`: CSS Color 5's
//! `@color-profile` on one line (LLP 1100 D3).

use super::*;

impl Parser {
    /// Parses one declaration into the file, refusing a repeated name.
    pub(super) fn color_profile(&mut self, file: &mut File) -> R<()> {
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
        if let Some(first) = file.color_profiles.iter().find(|p| p.name == name) {
            return duplicate("color-profile", &name, span, first.span);
        }
        file.color_profiles
            .push(ColorProfileDecl { name, attrs, span });
        Ok(())
    }
}
