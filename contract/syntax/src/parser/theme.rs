//! `theme`: the app's own colours, one `name="colour"` line an entry
//! (LLP 1095 D4). The values are checked where they lower.

use super::*;

impl Parser {
    /// The file's one `theme` block.
    pub(super) fn theme(&mut self, file: &mut File) -> R<()> {
        let span = self.expect_word("theme")?;
        self.newline()?;
        let entries = self.required_block(span, "theme", |p| {
            let (name, at) = match (p.peek_kind().clone(), p.peek2().clone()) {
                (TokenKind::Ident(n), TokenKind::Punct("=")) => (n, p.next().span),
                (other, _) => {
                    return p.err(
                        "syntax-theme-entry",
                        format!(
                            "a theme line is `name=\"colour\"`: `brand=\"#c8102e\"`, `card=\"secondary-grouped-background\"`; found {}",
                            describe(&other)
                        ),
                    )
                }
            };
            p.next();
            let value = p.expr()?;
            if !matches!(value, Expr::Str(..)) {
                return Err(SyntaxError {
                    id: "syntax-theme-entry",
                    message: format!("`{name}` in `theme` is a string literal, a colour as a row writes one: `{name}=\"light-dark(#d0e2f7, #27394f)\"`"),
                    span: value.span(),
                });
            }
            p.newline()?;
            Ok(Attr {
                name,
                value,
                span: at,
                written: true,
            })
        })?;
        for (i, entry) in entries.iter().enumerate() {
            if let Some(first) = entries[..i].iter().find(|e| e.name == entry.name) {
                return duplicate("theme entry", &entry.name, entry.span, first.span);
            }
        }
        if let Some(first) = &file.theme {
            return duplicate("theme", "theme", span, first.span);
        }
        file.theme = Some(ThemeDecl { entries, span });
        Ok(())
    }
}
