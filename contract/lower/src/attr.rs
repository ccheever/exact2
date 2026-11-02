//! A node's attribute, lowered to what its name targets
//! (`tags::AttrTarget`): a prop, a style row, a handler or a canvas's surface.

use crate::lint::unknown_attr;
use crate::{
    err, expr, fields, lint, menus, native, prop_row, shorthands, style_row, svg, symbols, tags,
    values, FontUse, LowerError, Lowerer,
};
use contract_syntax::{Attr, Expr};
use contract_types::{Scope, Ty};
use exact_kernel::StyleId;
use exact_plan::asm::Asm;
use exact_plan::{BindingsRow, Code, EventKind};

impl Lowerer<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn attr(
        &mut self,
        tag: &str,
        control: Option<&str>,
        a: &Attr,
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
        handlers: &mut Vec<(EventKind, exact_plan::ActionsId, Vec<Code>)>,
        surface: &mut Option<exact_plan::SurfacesId>,
        font: &[FontUse],
    ) -> Result<(), LowerError> {
        match a.name.as_str() {
            "decorations" => return self.bind_decorations(tag, a, scope, locals, bindings),
            "atoms" => return self.bind_atoms(a, scope, locals, bindings),
            "atomFaces" => return self.bind_atom_faces(a, scope, locals, bindings),
            "keep-mounted" => return self.bind_keep_mounted(a, scope, locals, bindings),
            _ => {}
        }
        let Some(mut target) = tags::attr_valued(&a.name, &a.value) else {
            return Err(unknown_attr(tag, a));
        };
        // A reset that writes no row (`tags::attr_valued`).
        if matches!(target, tags::AttrTarget::Styles([])) {
            return Ok(());
        }
        crate::var_tokens::carried(a)?;
        // @ref LLP 1021 §5.3, LLP 1133.025 D11 item 4 — stable menu ids,
        // never localized titles.
        if a.name == "-exact-menu" {
            menus::check_menu_placement(tag, a)?;
        }
        // HTML's global `title` on any element but `head`: advisory text, the
        // platform's tooltip (studio diary R24); `head`'s is the document's.
        if a.name == "title" && tag != "head" {
            target = tags::AttrTarget::Prop(exact_kernel::PropId::Title);
        }
        // @ref LLP 1024 D1 — `load` and `message` are a module's too.
        let module = native::is_module_tag(tag) && a.name != "sandbox";
        if (tag != "iframe"
            && matches!(a.name.as_str(), "sandbox" | "load" | "message")
            && !(tag == "canvas" && a.name == "message")
            && !(tag == "image" && a.name == "load")
            && !module)
            || (!matches!(tag, "iframe" | "video" | "audio") && a.name == "src")
        {
            // HTML's `img src`: an `image`'s source is its argument.
            if tag == "image" && a.name == "src" {
                return err(
                    "lower-attr-tag",
                    "write the source as the image's first argument, `image url`: an `image` takes no `src`",
                    a.span,
                );
            }
            return err(
                "lower-attr-tag",
                format!(
                    "`{}` belongs to {}, not `{tag}`",
                    a.name,
                    match a.name.as_str() {
                        "message" => "`iframe` or `canvas`",
                        "load" => "`iframe`, `image` or a native module",
                        _ => "`iframe`",
                    }
                ),
                a.span,
            );
        }
        // An SVG element's own attribute (a filter primitive's `mode`, `in`,
        // `values`…) does nothing on a box: refused by name rather than kept
        // as a prop no host reads (feed F19: CSS `order` once landed here).
        let svg_tag = svg::is_element(tag) || matches!(tag, "svg" | "text" | "tspan");
        if !svg_tag && !module && svg::svg_only_prop(&a.name) {
            return err(
                "lower-attr-tag",
                format!(
                    "`{}` is an SVG element's attribute; it does nothing on `{tag}`{}",
                    a.name,
                    lint::react_native(&a.name).unwrap_or_default()
                ),
                a.span,
            );
        }
        if let Some(why) = shorthands::text_box_trim_refusal(&a.name, tag, module)
            .or_else(|| fields::placeholder_refusal(&a.name, tag, control))
        {
            return err("lower-attr-tag", why, a.span);
        }
        if a.name == "alt" && tag != "image" {
            return err(
                "lower-attr-tag",
                format!("`alt` belongs to `image`, not `{tag}`; another element's accessible name is `aria-label`"),
                a.span,
            );
        }
        if !svg_tag && !module && a.name == "mask" {
            return err(
                "lower-attr-tag",
                "`mask` masks SVG elements so far; a box takes `mask-image` (a gradient)",
                a.span,
            );
        }
        // @ref LLP 1048.003 D1 — a document's metadata, and nothing else.
        let head_field =
            tags::HEAD_FIELDS.contains(&a.name.as_str()) && !(a.name == "title" && tag != "head");
        if head_field != (tag == "head") {
            return err(
                "lower-attr-tag",
                if head_field {
                    format!("`{}` belongs to `head`, not `{tag}`", a.name)
                } else {
                    format!(
                        "`head` takes only {}; `{}` is not one",
                        tags::HEAD_FIELDS.join(", "),
                        a.name
                    )
                },
                a.span,
            );
        }
        if a.name == "document" && tag != "scroll" {
            return err(
                "lower-attr-tag",
                format!("`document` belongs to `scroll`, not `{tag}`"),
                a.span,
            );
        }
        // The page's HTTP status, known at compile time: a not-found view's
        // 404 or 410, or a view of failed data's 503 (LLP 1048.003 D1, LLP
        // 1048.000 D11).
        if a.name == "status"
            && !matches!(a.value, Expr::Number(n, _) if n == 404.0 || n == 410.0 || n == 503.0)
        {
            return err(
                "lower-attr-value",
                "`status` is 404, 410 or 503, as a literal: the page answers with it",
                a.span,
            );
        }
        // @ref LLP 1064 D6 — a native field shows its value as typed.
        if a.name == "text-transform" && matches!(tag, "input" | "textarea") {
            return err(
                "lower-attr-tag",
                format!("`text-transform` does not apply to `{tag}`: a field shows what was typed on every host (the web's form controls reset it too); transform the value instead"),
                a.span,
            );
        }
        if !matches!(tag, "text" | "input" | "textarea") && a.name == "selectionchange" {
            return err(
                "lower-attr-tag",
                format!("`selectionchange` belongs to `text`: it reports the part of the reader's text selection inside one paragraph, not `{tag}`"),
                a.span,
            );
        }
        // @ref LLP 1045 D3 — `none` or `markdown`, styling a `text`'s or a
        // `textarea`'s own string; another word did nothing (notes #1).
        if a.name == "markup" {
            if !matches!(tag, "text" | "textarea") {
                let message = format!("`markup` belongs to `text` (the reader) or `textarea` (the editor), not `{tag}`");
                return err("lower-attr-tag", message, a.span);
            }
            if matches!(&a.value, Expr::Str(s, _) if s != "markdown" && s != "none") {
                return err(
                    "lower-attr-value",
                    "`markup` is \"markdown\" or \"none\"",
                    a.span,
                );
            }
        }
        match target {
            tags::AttrTarget::Shorthand => self.bind_shorthand(a, scope, locals, font, bindings)?,
            tags::AttrTarget::Flex => {
                for (index, row) in [StyleId::FlexGrow, StyleId::FlexShrink, StyleId::FlexBasis]
                    .into_iter()
                    .enumerate()
                {
                    let value = values::flex_component(&a.value, index)?;
                    let component = Attr { value, ..a.clone() };
                    let (code, ty) = self.typed_code(&component.value, scope, locals)?;
                    if index < 2 && ty.is_string() {
                        return err("lower-attr-type", "a computed `flex` must be a number, a literal CSS shorthand or a choice of literal shorthands; for a computed basis write the longhands, as in `flex-grow=1 flex-shrink=1 flex-basis=w`", a.span);
                    }
                    self.check_style(&component, &[row], &ty, font, scope)?;
                    bindings.push(style_row(row, code, a.written));
                }
            }
            tags::AttrTarget::Styles(rows) => {
                if rows == [StyleId::FontFamily] {
                    if self.family_token(a, scope, locals, bindings)? {
                        return Ok(());
                    }
                    let stacks = self.family_stacks(&a.value)?;
                    let (code, ty) = self.typed_code(&stacks, scope, locals)?;
                    // Literal families are stack numbers, an empty one `none`.
                    let literal = Self::literal_family_arms(&a.value);
                    if ty != Ty::String
                        && !(literal && matches!(ty, Ty::Number | Ty::Option(_)))
                        && !matches!(ty, Ty::Option(ref inner) if **inner == Ty::String)
                    {
                        return err("lower-attr-type", "`font-family` needs a string", a.span);
                    }
                    bindings.push(style_row(StyleId::FontFamily, code, a.written));
                    return Ok(());
                }
                // CSS's one-to-four-value box shorthands: a binding a side.
                if values::four_sided(rows) {
                    let fns = &self.fns;
                    shorthands::computed_list(&a.name, &a.value, &|f| fns.get(f).map(|(_, b)| *b))?;
                    if let Some(sides) = shorthands::sides(&a.name, &a.value)? {
                        for (&row, value) in rows.iter().zip(sides) {
                            let (code, ty) = self.typed_code(&value, scope, locals)?;
                            let side = Attr { value, ..a.clone() };
                            self.check_style(&side, &[row], &ty, font, scope)?;
                            bindings.push(style_row(row, code, a.written));
                        }
                        return Ok(());
                    }
                }
                // @ref LLP 1121 D3 — a duration or easing token is written into its `transition` or
                // `animation` before the codec reads it (the loader wrote a literal's in; this is an
                // expansion's), and a length token into grid tracks.
                let written = rows
                    .iter()
                    .any(|r| {
                        matches!(
                            r,
                            StyleId::Transition
                                | StyleId::Animation
                                | StyleId::ExitAnimation
                                | StyleId::LayoutTransition
                        )
                    })
                    .then(|| {
                        crate::motion::attrs(
                            std::slice::from_ref(a),
                            &crate::theme::motion_tokens(),
                        )
                    })
                    .flatten()
                    .or_else(|| values::grid_tokens(a, rows).map(|g| vec![g]));
                let a = written.as_ref().map_or(a, |m| &m[0]);
                let (code, ty) = self.typed_code(&a.value, scope, locals)?;
                self.check_style(a, rows, &ty, font, scope)?;
                for &row in rows {
                    bindings.push(style_row(row, code, a.written));
                }
            }
            tags::AttrTarget::InvertedBoolProp(prop) => {
                // `not value`, compiled from the value itself.
                let (mut asm, mut depth) = (Asm::new(), locals);
                let ty = expr::compile(self, &mut asm, &a.value, scope, &mut depth)?;
                asm.simple(exact_plan::Opcode::Not);
                let code = self.b.code(asm);
                values::check_prop_value(&a.name, &a.value, a.span, prop, &ty)?;
                bindings.push(prop_row(prop, code, a.written));
            }
            tags::AttrTarget::Prop(prop) => {
                let glass;
                let aria_bool = values::aria_bool_literal(&a.name, prop, &a.value);
                let value = if prop == exact_kernel::PropId::GlassGroup {
                    glass = values::glass_group(&a.value)?;
                    &glass
                } else if prop == exact_kernel::PropId::IntersectMargin {
                    glass = crate::intersect::margin(&a.value, a.span)?;
                    &glass
                } else if prop == exact_kernel::PropId::SwipeDirection {
                    glass = crate::swipe::direction(&a.value, a.span)?;
                    &glass
                } else if prop == exact_kernel::PropId::TouchCount {
                    glass = crate::swipe::count(&a.value, a.span)?;
                    &glass
                } else if let Some(width) = values::split_width(prop, &a.value) {
                    glass = width;
                    &glass
                // @ref LLP 1116.007 D3.1 — a symbol by another spelling is its role.
                } else if let Some(role) = (prop == exact_kernel::PropId::ImageSource)
                    .then(|| symbols::roles(&a.value))
                    .flatten()
                {
                    glass = role;
                    &glass
                } else {
                    aria_bool.as_ref().unwrap_or(&a.value)
                };
                let (mut asm, mut depth) = (Asm::new(), locals);
                let mut ty = expr::compile(self, &mut asm, value, scope, &mut depth)?;
                // @ref LLP 1125 D8 — `midiinputs=[…]` as the prop carries it, joined by newlines.
                if prop == exact_kernel::PropId::MidiInputs
                    && matches!(&ty, Ty::List(t) if matches!(**t, Ty::String | Ty::Unknown))
                {
                    asm.str(self.b.str("\n"));
                    asm.call(exact_plan::Stdlib::Join);
                    ty = Ty::String;
                }
                values::check_prop_value(&a.name, value, a.span, prop, &ty)?;
                // ARIA's word-valued states; a bool is written as `true`/`false`.
                if values::aria_words(prop).is_some_and(|(_, w)| values::aria_takes_bool(w))
                    && ty == Ty::Bool
                {
                    asm.call(exact_plan::Stdlib::ToString);
                }
                // An enumerated attribute whose IDL attribute is a bool takes
                // one, as its words (shop diary F7).
                if let (Ty::Bool, Some((yes, no))) = (&ty, values::bool_words(prop)) {
                    let word = |w: &str| Box::new(Expr::Str(w.into(), a.span));
                    let words = Expr::Ternary(Box::new(value.clone()), word(yes), word(no), a.span);
                    (asm, depth) = (Asm::new(), locals);
                    expr::compile(self, &mut asm, &words, scope, &mut depth)?;
                }
                let code = self.b.code(asm);
                bindings.push(prop_row(prop, code, a.written));
            }
            tags::AttrTarget::Surface => {
                if tag != "canvas" {
                    return err(
                        "lower-surface-tag",
                        format!("`surface` belongs to `canvas`, not `{tag}`"),
                        a.span,
                    );
                }
                let (name, args): (&str, &[Expr]) = match &a.value {
                    Expr::Ident(n, _) => (n, &[]),
                    Expr::Call(n, args, _) => (n, args),
                    _ => {
                        return err(
                            "lower-surface",
                            "a surface is a name or `name(args)`",
                            a.span,
                        )
                    }
                };
                let mut codes = Vec::new();
                for arg in args {
                    let (name, value) = match arg {
                        Expr::NamedArg(name, value, _) => (name.as_str(), value.as_ref()),
                        _ => ("", arg),
                    };
                    codes.push((name, self.expr_code(value, scope, locals)?));
                }
                *surface = Some(self.b.surface(name, &codes));
            }
            tags::AttrTarget::Handler(event) => {
                self.handler(tag, event, control, a, scope, locals, handlers)?;
            }
            tags::AttrTarget::MediaMetadata => self.media_metadata(a, scope, locals, bindings)?,
        }
        Ok(())
    }
}
