//! A heading's text style (LLP 1115 D3, LLP 1115.000 D12.2).

use exact_kernel::StyleId;
use exact_plan::BindingsRow;

impl crate::Lowerer<'_> {
    /// @ref LLP 1115 D3 — a heading takes its level's platform text style.
    /// iOS, macOS and Material keep its weight and leading. The web, neutral and
    /// terminal looks add title emphasis through metrics. Authored rows win.
    /// Literal level choices select their styles; other expressions use 2.
    pub(crate) fn heading_style(
        &mut self,
        tag: &str,
        attrs: &[contract_syntax::Attr],
        scope: &crate::Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<(), crate::LowerError> {
        use contract_syntax::Expr;
        let heading = attrs
            .iter()
            .any(|a| a.name == "role" && matches!(&a.value, Expr::Str(v, _) if v == "heading"));
        if tag != "text" || !heading {
            return Ok(());
        }
        let style = |level: f64| {
            let name = match level {
                1.0 => "title1",
                3.0 => "title3",
                n if n >= 4.0 => "headline",
                _ => "title2",
            };
            let weight = match name {
                "title1" | "title2" => Some("-exact-heading-title-weight"),
                "title3" => Some("-exact-heading-small-title-weight"),
                _ => None,
            };
            (name, weight)
        };
        // The level's literal leaves, each mapped; `None` when one is computed.
        fn leaves(e: &Expr, f: &dyn Fn(f64) -> Expr) -> Option<Expr> {
            Some(match e {
                Expr::Number(n, _) => f(*n),
                Expr::Str(v, _) => f(v.trim().parse().ok()?),
                Expr::Ternary(c, yes, no, span) => Expr::Ternary(
                    c.clone(),
                    Box::new(leaves(yes, f)?),
                    Box::new(leaves(no, f)?),
                    *span,
                ),
                _ => return None,
            })
        }
        let level = attrs
            .iter()
            .find(|a| a.name == "aria-level")
            .map(|a| &a.value);
        if level.is_none() {
            // ARIA's default level, said: a host that reads the level (iOS's
            // header-shaped route takes only a heading with one) sees 2.
            let expr = self.expr_code(&Expr::Number(2.0, Default::default()), scope, locals)?;
            let level = exact_kernel::PropId::AccessibilityHeadingLevel;
            bindings.push(crate::prop_row(level, expr, crate::IMPLIED));
        }
        let span = level.map_or_else(Default::default, |l| l.span());
        let two = Expr::Number(2.0, span);
        let level = level
            .filter(|l| leaves(l, &|_| two.clone()).is_some())
            .unwrap_or(&two);
        let size = leaves(level, &|n| {
            Expr::Str(format!("-exact-{}", style(n).0), span)
        })
        .expect("literal leaves");
        let mut weight = leaves(level, &|n| match style(n).1 {
            Some(w) => Expr::Str(w.into(), span),
            None => Expr::None(span),
        })
        .expect("literal leaves");
        // Keep the absence test separate from the heading level: a font's
        // match or let binding must not capture names in that level.
        fn font_absent(font: &Expr) -> Expr {
            let mut out = font.clone();
            match &mut out {
                Expr::None(_) => return Expr::Bool(true, font.span()),
                Expr::Ternary(_, yes, no, _)
                | Expr::Match {
                    some: yes,
                    none: no,
                    ..
                } => {
                    **yes = font_absent(yes);
                    **no = font_absent(no);
                }
                Expr::Let { body, .. } => **body = font_absent(body),
                _ => return Expr::Bool(false, font.span()),
            }
            out
        }
        if let Some(font) = attrs.iter().rev().find(|a| a.name == "font") {
            weight = Expr::Ternary(
                Box::new(font_absent(&font.value)),
                Box::new(weight),
                Box::new(Expr::None(span)),
                span,
            );
        }
        let expr = self.expr_code(&size, scope, locals)?;
        bindings.push(crate::style_row(StyleId::FontSize, expr, crate::IMPLIED));
        // The style is the heading's as a `font` shorthand names one: its
        // weight and leading beat an inherited one (LLP 1115.000 D12.2).
        let marked = self.b.constant(&exact_plan::Value::Bool(true));
        bindings.push(crate::prop_row(
            exact_kernel::PropId::FontShorthand,
            marked,
            crate::IMPLIED,
        ));
        if !matches!(weight, Expr::None(_)) {
            let expr = self.expr_code(&weight, scope, locals)?;
            bindings.push(crate::style_row(StyleId::FontWeight, expr, crate::IMPLIED));
        }
        Ok(())
    }
}
