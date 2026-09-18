//! @ref LLP 1038 D2/D3 — indentation is parentage; the slot exists before lifting.

use super::*;

impl Parser {
    pub(super) fn routes_decl(&mut self) -> R<RoutesDecl> {
        let span = self.expect_word("routes")?;
        let (slot, _) = self.ident()?;
        self.newline()?;
        let mut rows = Vec::new();
        self.block(|p| p.route_line(None, &mut rows))?;
        Ok(RoutesDecl { slot, rows, span })
    }

    fn route_line(&mut self, parent: Option<usize>, rows: &mut Vec<RouteDecl>) -> R<()> {
        let span = self.peek().span;
        let tab = self.at_ident("tab");
        if tab {
            self.next();
        }
        let (name, _) = self.ident()?;
        let notfound = name == "notfound" && !tab;
        let pattern = if notfound {
            String::new()
        } else {
            self.str_lit("a route pattern")?
        };
        self.newline()?;
        let index = rows.len();
        rows.push(RouteDecl {
            name,
            pattern,
            parent,
            tab,
            notfound,
            span,
        });
        self.block(|p| p.route_line(Some(index), rows))?;
        Ok(())
    }
}
