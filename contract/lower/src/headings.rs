//! A heading's text style (LLP 1115 D3, LLP 1115.000 D12.2).

use exact_kernel::StyleId;
use exact_plan::BindingsRow;

/// LLP 1115.000 D12.2's one switch (the lead's provisional ruling,
/// 2026-10-17, for Charlie): a heading whose style is a title, regular on
/// every platform, keeps the heading's bold as an implied row; `false` gives
/// every heading its style's own weight, platform-strict.
pub(crate) const HEADING_BOLD: bool = true;

impl crate::Lowerer<'_> {
    /// @ref LLP 1115 D3 — a heading is the platform's text style for its
    /// level, as a hand-built screen's would be: 1 `title1`, 2 `title2`, 3
    /// `title3`, 4 and on `headline` (ARIA's default level is 2), at the
    /// style's size for the root font size and its emphasized weight (LLP
    /// 1116 D7): bold for a title, whose own weight is regular, as Apple's
    /// emphasized title styles and the web's headings are ([`HEADING_BOLD`]);
    /// `headline`'s own, per platform (semibold on iOS, bold on the Mac), and
    /// the style's leading, which the kernel gives as for a `font` shorthand
    /// (LLP 1115.000 D12.2). A level chosen
    /// between literals (`aria-level=(top ? 1 : 2)`) chooses between their
    /// styles; any other computed level is 2's. Pushed before the author's
    /// rows, so a written `font-size` or `font-weight` wins; an inherited one
    /// is unsaid and does not.
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
            let id = exact_kernel::style::relative::text_style(&format!("-exact-{name}"));
            let weight = exact_kernel::style::relative::text_style_weight(
                id.expect("a schema text style"),
                exact_kernel::style::metrics::Column::Ios,
            );
            // A title, whose own weight is regular, takes the heading's bold
            // (`HEADING_BOLD`); any other style its own, per platform.
            (name, (HEADING_BOLD && weight == 400).then_some(700))
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
        let weight = leaves(level, &|n| match style(n).1 {
            Some(w) => Expr::Number(f64::from(w), span),
            None => Expr::None(span),
        })
        .expect("literal leaves");
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
