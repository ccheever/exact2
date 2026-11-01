//! Element lowering keeps its large per-tag temporaries off the recursive view path.

use super::*;

pub(super) struct ElementStep<'n> {
    pub(super) id: NodesId,
    pub(super) children: std::borrow::Cow<'n, [Node]>,
    pub(super) tag: &'n str,
    pub(super) enters: bool,
    pub(super) positioned: bool,
    pub(super) button_context: Option<&'static str>,
    pub(super) popover_child: bool,
    pub(super) bounded: bool,
    pub(super) horizontal_flex: Option<bool>,
    pub(super) nav_place: routes::NavPlace,
    /// How many element levels below this one a list's rows are (LLP 1121
    /// D8's `lower-state-target`): 1 a cell list's, 3 a grouped list's
    /// (its sections' group's); 0 none.
    pub(super) rows: u8,
}

impl Lowerer<'_> {
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    pub(super) fn prepare_element<'n>(
        &mut self,
        n: &'n Node,
        parent: Option<NodesId>,
        arm: Option<ArmsId>,
        order: u32,
        scope: &Scope,
        locals: u16,
        parent_tag: Option<&str>,
    ) -> Result<ElementStep<'n>, LowerError> {
        let Node::Element {
            tag,
            positional,
            attrs,
            children,
            span,
            instance,
        } = n
        else {
            unreachable!("prepare_element only receives elements")
        };
        let Some(t) = tags::tag(tag).or_else(|| native::tag(tag)) else {
            return Err(unknown_tag(tag, *span));
        };
        // Two layout refusals the compiler can make without measuring
        // (LLP 1017 P1c; the measured ones are bake's). Conservative:
        // only the case nothing on the path can bound is refused.
        // `class=` expands its style's rows first; the node's own
        // attribute of the same name replaces the style's (LLP 1017 P6).
        // @ref LLP 1084 D7 — a grouped list's sheet, under its classes.
        let button_context = grouped::button_context(attrs);
        let separated = grouped::row_separator(attrs);
        let acting = grouped::actions::said(attrs);
        let (mut sheet, unmarked) = grouped::split(attrs);
        let attrs = unmarked.as_ref().unwrap_or(attrs);
        // A `delete=` no list row expanded does nothing (`lower-delete-row`).
        let kept = swipe_rows::stray(attrs);
        let attrs = kept.as_ref().unwrap_or(attrs);
        let (class_label, mut expanded) = self.class_rows(attrs)?.unzip();
        // The class's rows by their places: a rewrite below (an animation's
        // longhands composed, a grouped list's rows inserted) moves rows
        // across `class_len`, never their spans (LLP 1122 D6's address).
        let class_spans: std::collections::BTreeSet<contract_syntax::Span> = expanded
            .iter()
            .flatten()
            .filter(|a| a.written)
            .map(|a| a.span)
            .collect();
        let button = self.button_appearance(
            tag,
            expanded.as_deref().unwrap_or(&[]),
            attrs,
            children,
            button_context,
            *span,
        )?;
        let native_button = button.as_ref().is_some_and(|b| b.native);
        let painted_group =
            self.painted_radio_group(attrs, expanded.as_deref().unwrap_or(&[]), children, *span)?;
        // A `button` part as the platform draws it (LLP 1139 D6).
        if let Some(b) = button.as_ref() {
            self.button_part(*span, b.native, b.bare_reason.as_deref());
        }
        if native_button {
            let margins = grouped::margins_said(attrs)
                || expanded.as_deref().is_some_and(grouped::margins_said);
            grouped::native_rows(&mut sheet, margins);
        }
        // @ref LLP 1084 D7 — the platform's separator under a grouped row; a
        // native button row is the platform's control, under none.
        let separated = separated && !native_button;
        let class_len = expanded.as_ref().map_or(0, Vec::len) + sheet.len();
        let expanded = match &mut expanded {
            Some(rows) => {
                rows.splice(0..0, sheet.iter().cloned());
                rows.extend(attrs.iter().filter(|a| a.name != "class").cloned());
                rows.as_slice()
            }
            None if !sheet.is_empty() => {
                expanded = Some([sheet.clone(), attrs.to_vec()].concat());
                expanded.as_deref().expect("just set")
            }
            None => attrs.as_slice(),
        };
        let face = self.button_context == Some("a native button");
        let inked = crate::symbols::symbol_ink(
            tag,
            positional,
            &expanded[sheet.len()..],
            face,
            !self.fns.contains_key("startsWith")
                && !contract_types::records::is_record_call("startsWith", &self.types.shapes),
        );
        crate::popover_surface::check_authored(expanded)?;
        let surfaced = crate::popover_surface::implied(expanded, *span);
        let added: Vec<Attr> = inked
            .into_iter()
            .chain(painted_group)
            .chain(surfaced)
            .collect();
        let expanded = match added.is_empty() {
            false => std::borrow::Cow::Owned([expanded, &added].concat()),
            true => std::borrow::Cow::Borrowed(expanded),
        };
        let expanded = &*expanded;
        decorations::check_field(tag, expanded)?;
        atoms::check_field(tag, expanded)?;
        values::check_paragraph_dirs(tag, expanded)?;
        menus::check_menu_remove(expanded, parent_tag.is_none())?;
        menus::check_menu_keys(expanded, parent_tag.is_none())?;
        values::check_position_area(expanded)?;
        menus::check_invoker(tag, expanded)?;
        crate::alert_parts::check_authored(expanded)?;
        tags::check_exclusion(&t, expanded, self.parent_positioned)?;
        // @ref LLP 1121 D3 — a motion token an expansion carried in (a
        // component's prop, a sheet) is written in before the longhands
        // compose with their shorthand.
        let timed = crate::motion::attrs(expanded, &crate::theme::motion_tokens());
        let expanded = timed.as_deref().unwrap_or(expanded);
        let composed = self.compose_animation(expanded)?;
        let expanded = composed.as_deref().unwrap_or(expanded);
        let lengths =
            svg::coerce_lengths(tag, svg::in_svg(self.svg_depth > 0, parent_tag), expanded);
        let expanded = lengths.as_deref().unwrap_or(expanded);
        // A box's `transform` as the rows it equals (LLP 1055.000 D5's
        // deferral keeps the rest), before `check_svg` reads the rows.
        let boxed = crate::box_transform::lower(
            tag,
            svg::in_svg(self.svg_depth > 0, parent_tag),
            expanded,
        )?;
        let expanded = boxed.as_deref().unwrap_or(expanded);
        self.check_svg(tag, parent_tag, expanded, *span)?;
        shorthands::check_text_box(tag, parent_tag, expanded)?;
        // @ref LLP 1011.002 D3 — the one box `vertical-align` moves.
        let inline_image = tag == "image" && matches!(parent_tag, Some("text"));
        if inline_image {
            // Its box takes lengths alone, not the sizes it would resolve
            // against a containing block (LLP 1011.002 §"Not built").
            // A literal percentage, or one a choice may take.
            fn percent(e: &contract_syntax::Expr) -> bool {
                use contract_syntax::Expr;
                match e {
                    Expr::Str(t, _) => t.contains('%'),
                    Expr::Ternary(_, yes, no, _) => percent(yes) || percent(no),
                    Expr::Match { some, none, .. } => percent(some) || percent(none),
                    Expr::Let { body, .. } => percent(body),
                    _ => false,
                }
            }
            if let Some(a) = expanded.iter().find(|a| {
                matches!(
                    a.name.as_str(),
                    "min-width" | "max-width" | "min-height" | "max-height"
                ) || matches!(a.name.as_str(), "width" | "height") && percent(&a.value)
            }) {
                return err(
                    "lower-attr-value",
                    format!("`{}` on an `image` in a paragraph's text: its box takes a length `width` and `height`, no percentage and no min or max size", a.name),
                    a.span,
                );
            }
        }
        if let Some(a) = expanded.iter().find(|a| a.name == "vertical-align") {
            if !inline_image && !matches!(tag.as_str(), "th" | "td") {
                return err(
                    "lower-attr-tag",
                    format!(
                        "`vertical-align` aligns an `image` in a paragraph's text or a table cell's content, not `{tag}`"
                    ),
                    a.span,
                );
            }
        }
        // @ref LLP 1034 §8 — an inline run paints in its paragraph's
        // scheme: no host gives a run an appearance of its own.
        if matches!(tag.as_str(), "text" | "time") && matches!(parent_tag, Some("text" | "time")) {
            if let Some(a) = expanded.iter().find(|a| a.name == "color-scheme") {
                return Err(LowerError {
                            id: "lower-attr-tag",
                            message: "`color-scheme` on an inline `text` run: a run paints in its paragraph's scheme (LLP 1034 §8); set it on the paragraph or a box above".into(),
                            span: a.span,
                        });
            }
        }
        // @ref LLP 1055.000 D4 — an `svg` inside an `svg` is a viewport.
        // Inside an `svg`, `svg` is a viewport and `text` is SVG text
        // (LLP 1055.000 D4, D11).
        let t = match tag.as_str() {
            "svg" if svg::in_svg(self.svg_depth > 0, parent_tag) => tags::Tag {
                node_type: NodeType::SvgViewport,
                ..t
            },
            "text" if svg::in_svg(self.svg_depth > 0, parent_tag) => tags::Tag {
                node_type: NodeType::SvgText,
                ..t
            },
            _ => t,
        };
        // @ref LLP 1069.001 D1 — `input`'s `type` is a literal: a text
        // type is a text field, `checkbox` a form control.
        let canonical_type = controls::canonical_type_attrs(tag, expanded);
        let expanded = canonical_type.as_deref().unwrap_or(expanded);
        let control = controls::control(tag, expanded, button.as_ref().is_some_and(|b| b.native))?;
        let t = control.map_or(t.clone(), |kind| controls::tag(kind, t.clone()));
        let face = (control == Some("button"))
            .then(|| grouped::unsheet(children))
            .flatten();
        let authored_children = children;
        let children: &[Node] = face.as_deref().unwrap_or(authored_children.as_slice());
        controls::check_nesting(tag, parent_tag, *span)?;
        crate::tables::check(tag, parent_tag, self.in_dl_group, expanded, children, *span)?;
        controls::check_progress(tag, expanded, children)?;
        crate::live_time::check(tag, expanded, *span)?;
        let alert = self.check_menu_shapes(tag, expanded, children, *span)?;
        let parted = self.alert_parts(tag, expanded, children, alert, *span, *instance);
        let children: &[Node] = parted.as_deref().unwrap_or(children);
        let closedby = menus::implied_closedby(tag, expanded, *span, alert);
        let expanded = closedby.as_deref().unwrap_or(expanded);
        let numeric = controls::range_attrs(tag, control, expanded);
        let expanded = numeric.as_deref().unwrap_or(expanded);
        // @ref LLP 1084 D1, D3 — a grouped list's sheet, before its
        // author's rows, and its sections' shape.
        let grouped = grouped::style(tag, expanded)?;
        let listed;
        let mut sections = None;
        let (expanded, children) = match grouped {
            Some(style) => {
                listed = [grouped::list_rows(style, *span), expanded.to_vec()].concat();
                sections = Some(grouped::sections(style, children, &|a| self.class_rows(a))?);
                (
                    listed.as_slice(),
                    sections.as_deref().expect("sections just set"),
                )
            }
            None => (expanded, children),
        };
        tags::validate_list(tag, expanded, *span)?;
        self.check_collection(tag, expanded, children, *span, scope)?;
        // @ref LLP 1115.000 D16 — whether this list's rows are cells.
        let cells = cells::rows(tag, expanded, children, grouped.is_some());
        let grouped_list = grouped.is_some();
        // A cell's row is no state part's target (LLP 1121 D8).
        // @ref LLP 1116.020 D1 — a cell's `delete=`, its swipe.
        let swiped = cells.then(|| swipe_rows::cells(children)).flatten();
        // A row list is a flex item of its column like any carousel;
        // CSS's own fix keeps its spacers' extent from widening that
        // column: `min-width: 0`, unless the author set one (LLP 1070
        // §3.1).
        let row_list = (tag == "list"
            && collection::row_list(expanded)
            && expanded
                .iter()
                .any(|a| a.name == "virtualized" && matches!(a.value, Expr::Bool(true, _)))
            && !expanded.iter().any(|a| a.name == "min-width"))
        .then(|| {
            let mut attrs = expanded.to_vec();
            attrs.push(contract_syntax::Attr {
                name: "min-width".into(),
                value: Expr::Number(0.0, *span),
                span: *span,
                written: IMPLIED,
            });
            attrs
        });
        let expanded = row_list.as_deref().unwrap_or(expanded);
        media::check_session(tag, expanded)?;
        crate::frames::check(tag, positional, expanded)?;
        let audio = media::audio_rows(tag, expanded, *span)?;
        let expanded = audio.as_deref().unwrap_or(expanded);
        // @ref LLP 1074 T1 — a box that contains its absolutely positioned
        // descendants on every host is lowered `position: relative`.
        let in_svg = svg::in_svg(self.svg_depth > 0, parent_tag);
        let relative = contain::positioned(
            &t,
            expanded,
            in_svg,
            *span,
            self.host_transforms.contains(&(*span, *instance)),
            !self.may_hold_absolute(children),
        )?;
        let expanded = relative.as_deref().unwrap_or(expanded);
        let first = parent_tag.is_none() && arm.is_none() && order == 0;
        let back = self.nav_place.unnamed(first, expanded, *span);
        let expanded = back.as_deref().unwrap_or(expanded);
        values::check_glass_group(&t, expanded)?;
        let has = |names: &[&str]| expanded.iter().any(|a| names.contains(&a.name.as_str()));
        let parent_stacks = !matches!(parent_tag, Some("row") | Some("canvas"));
        let clips_y = expanded.iter().any(|a| {
            a.name == "overflow-y" && matches!(&a.value, Expr::Str(v, _) if v == "hidden")
        });
        if tag == "scroll"
            && !clips_y
            && parent_stacks
            && !has(&["height", "max-height"])
            && !expanded
                .iter()
                .any(|a| a.name == "flex" && values::flex_bounds(&a.value))
            && !values::shrinking_scroll(expanded, self.parent_bounded_column)
        {
            return err(
                        "lower-scroll-unbounded",
                        "`scroll` has no `height`, `max-height`, or `flex`, and its parent stacks it top to bottom, so it will grow with its content and never scroll",
                        *span,
                    );
        }
        controls::check_zero_size(tag, expanded, children, *span)?;
        // @ref LLP 1116.007 D5 — a route that scrolls a grouped list is on
        // the grouped background, unless its author said otherwise.
        let grouped_page = self.nav_place.grouped_page(tag, expanded, children, *span);
        let expanded = grouped_page.as_deref().unwrap_or(expanded);
        // @ref LLP 1116.009 D3 — and a sheet is never transparent.
        let sheet_page = self.nav_place.sheet_page(expanded, *span);
        let expanded = sheet_page.as_deref().unwrap_or(expanded);
        let nav_place = self.nav_place.enter(tag, expanded, children, *span)?;
        // @ref LLP 1038 D8 — only the first root selects navigation; an
        // iframe's `navigate` is its guest's (LLP 1020.002 D2).
        if tag != "iframe"
            && has(&["navigate"])
            && (parent_tag.is_some()
                || arm.is_some()
                || order != 0
                || !has(&["navigationKey"])
                || !has(&["navigationBack"]))
        {
            return err(
                "lower-navigate-root",
                "`navigate` belongs to the navigation root (the first root, with navigationKey)",
                *span,
            );
        }
        // @ref LLP 1121 D8 — the state parts, folded or recorded.
        let target = if control.is_some() || t.node_type == NodeType::Control {
            Some("a native control")
        } else if self.row_depth == 1 {
            Some("a list's row, whose press is the cell's highlight")
        } else {
            None
        };
        // What the node takes unwritten, a state's base where its author
        // writes none: its tag's fixed rows (an `hr`'s separator colour),
        // then a link's colour, which is pushed after them.
        let mut implied: Vec<(&str, Expr)> = t
            .fixed_styles
            .iter()
            .filter_map(|(id, v)| {
                Some((
                    crate::states::row_name(*id)?,
                    Expr::Str(v.to_string(), *span),
                ))
            })
            .collect();
        implied.extend(
            self.link_colour(tag, parent_tag, expanded, children)
                .map(|e| ("color", e)),
        );
        // A run inside a paragraph has no box of its own on every host: its
        // hover, focus and press are its paragraph's (theme2's 08:13 ruling;
        // D7's press rows paint no run on Apple or Linux either).
        // (A run is a text node under a text node, as the kernel has it: under
        // `text`, `time` or `option` alike.)
        if parent_tag
            .and_then(tags::tag)
            .is_some_and(|p| p.node_type == NodeType::Text)
            && t.node_type == NodeType::Text
        {
            let host = |a: &&Attr| {
                contract_syntax::state::state_of(&a.name)
                    .is_some_and(|(s, _)| matches!(s, "active" | "hover" | "focus-visible"))
            };
            if let Some(a) = expanded.iter().find(host) {
                return err(
                    "lower-state-target",
                    "a state part on a run inside a paragraph: a run's press, hover and focus are its paragraph's; put the part on the paragraph or make the run a box (LLP 1121 D8)",
                    a.span,
                );
            }
        }
        // The press's absolute opacity is lowering's, from `:active`.
        if let Some(a) = expanded
            .iter()
            .find(|a| a.name == crate::states::PRESS_OPACITY_TO)
        {
            return err(
                "lower-unknown-attr",
                format!("unknown attribute `{}`: the press's opacity is `opacity` in an `:active` part (LLP 1121 D8), or `-exact-press-opacity`, a share of the node's own (LLP 1061 D7)", a.name),
                a.span,
            );
        }
        let states = self.fold_states(expanded, target, &implied, class_len, scope, locals)?;
        let expanded = states.as_ref().map_or(expanded, |s| s.attrs.as_slice());
        let mut bindings: Vec<BindingsRow> = Vec::new();
        let mut handlers: Vec<(EventKind, exact_plan::ActionsId, Vec<Code>)> = Vec::new();
        let mut surface: Option<exact_plan::SurfacesId> = None;
        for (style, value) in t.fixed_styles {
            bindings.push(style_row(*style, self.fixed(true, value), IMPLIED));
        }
        for (prop, value) in t.fixed_props {
            bindings.push(prop_row(*prop, self.fixed(false, value), IMPLIED));
        }
        if cells {
            let rows = self.b.constant(&Value::Bool(true));
            bindings.push(prop_row(exact_kernel::PropId::ListRows, rows, IMPLIED));
        }
        if let Some(acting) = acting {
            let said = self.b.constant(&Value::Bool(acting));
            bindings.push(prop_row(exact_kernel::PropId::GroupedAction, said, IMPLIED));
        }
        if separated {
            let on = self.b.constant(&Value::Bool(true));
            bindings.push(prop_row(
                exact_kernel::PropId::GroupedRowSeparator,
                on,
                IMPLIED,
            ));
        }
        // @ref LLP 1115.000 D6 — the author's own disabled look: no default dim.
        let styled = states
            .as_ref()
            .map_or_else(|| disabled_look::styled(expanded), |s| s.disabled_look);
        if styled {
            let styled = self.b.constant(&Value::Bool(true));
            bindings.push(prop_row(
                exact_kernel::PropId::DisabledStyled,
                styled,
                IMPLIED,
            ));
        }
        self.link_color(
            tag,
            parent_tag,
            expanded,
            children,
            scope,
            locals,
            &mut bindings,
        );
        self.heading_style(tag, expanded, scope, locals, &mut bindings)?;
        if tag == "time" && expanded.iter().any(|a| a.name == "format" && matches!(&a.value, Expr::Str(v, _) if matches!(v.as_str(), "relative" | "timer" | "countdown"))) {
            bindings.push(style_row(exact_kernel::StyleId::FontVariantNumeric, self.fixed(true, "tabular-nums"), IMPLIED));
        }

        // @ref LLP 1069.001 (amended 2026-10-07) — an indeterminate
        // `progress` (no `value`, LLP 1116 D8) is busy, as ARIA's
        // `aria-busy`: a bool, which a fixed prop (text) cannot be.
        // @ref LLP 1115.000 D10.2 — a determinate one is full width, as a
        // range is; the indicator keeps its 20 by 20.
        if tag == "progress" {
            if expanded
                .iter()
                .any(|a| matches!(a.name.as_str(), "value" | "start"))
            {
                for (style, value) in controls::FULL_WIDTH {
                    bindings.push(style_row(*style, self.fixed(true, value), IMPLIED));
                }
            } else {
                let busy = self.b.constant(&Value::Bool(true));
                bindings.push(prop_row(
                    exact_kernel::PropId::AccessibilityBusy,
                    busy,
                    IMPLIED,
                ));
            }
        }
        // @ref LLP 1048.003 D4 — the page scrolls where this does.
        let positional = match positional.as_slice() {
            [word] if contract_syntax::is_scroll_document(tag, word) => {
                if let Some(bound) = attrs.iter().find(|a| a.name == "document") {
                    return err(
                        "lower-attr-tag",
                        "`scroll` takes `document` or `document=(…)`, not both",
                        bound.span,
                    );
                }
                let document = self.b.constant(&Value::Bool(true));
                bindings.push(prop_row(
                    exact_kernel::PropId::ScrollDocument,
                    document,
                    true,
                ));
                &[][..]
            }
            // @ref LLP 1069.001 D1, LLP 1069.002 D1 — HTML's `switch`
            // and `multiple`.
            [word] if self.control_word(tag, word, control, &mut bindings)? => &[][..],
            all => all,
        };
        if let Some(first) = positional.first() {
            let Some(prop) = t.positional else {
                return err(
                    "lower-positional",
                    if tag == "scroll" {
                        "`scroll` takes one word, `document`: `scroll document`".to_owned()
                    } else {
                        format!("`{tag}` takes no positional argument")
                    },
                    first.span(),
                );
            };
            // @ref LLP 1116.007 D3.1 — a symbol by another spelling is its role.
            let role = (prop == exact_kernel::PropId::ImageSource)
                .then(|| crate::symbols::roles(first))
                .flatten();
            let first = role.as_ref().unwrap_or(first);
            let (code, ty) = self.typed_code(first, scope, locals)?;
            values::check_prop_value(tag, first, first.span(), prop, &ty)?;
            bindings.push(prop_row(prop, code, true));
        }
        if positional.len() > 1 {
            return err(
                "lower-positional",
                format!("`{tag}` takes at most one positional argument"),
                positional[1].span(),
            );
        }
        let mut origins = self
            .sites
            .as_ref()
            .map(|_| vec![Origin::Tag; bindings.len()]);
        let font = self.font_use(expanded)?;
        // @ref LLP 1024 D1 — a module tag's own attribute named like
        // a row its box never uses is refused, not bound to nothing;
        // a class's rows are the style's, never a prop. By name over
        // the authored list: `expanded` is rewritten above (an
        // animation shorthand and its longhands compose), so a
        // position in it does not say whose an attribute is.
        let own: Vec<&Attr> = attrs.iter().filter(|a| a.name != "class").collect();
        let refused: Vec<&str> = own
            .iter()
            .filter(|a| native::refused(tag, a).is_some())
            .map(|a| a.name.as_str())
            .collect();
        for a in &own {
            self.errors.extend(native::refused(tag, a));
        }
        let flex = tags::flex_container(tag, expanded);
        self.element_children = !children.is_empty();
        for (index, a) in expanded.iter().enumerate() {
            if let Some(e) = flex.and_then(|f| tags::multicol_on_flex(f, a).err()) {
                self.errors.push(e);
                continue;
            }
            if native::leftover(tag, a)
                || refused.contains(&a.name.as_str())
                || dataset::word(&a.name).is_some()
            {
                continue;
            }
            if let Err(e) = self.attr(
                tag,
                contract_syntax::payload_control(tag, attrs),
                a,
                scope,
                locals,
                &mut bindings,
                &mut handlers,
                &mut surface,
                &font,
            ) {
                self.errors.push(native::name_the_fix(n, e));
            }
            if let Some(origins) = &mut origins {
                // A folded state part's (LLP 1121 D8): its class's where
                // the class wrote every state row in it.
                let (class_len, folded) = match &states {
                    Some(s) => (
                        s.class_len,
                        (index >= s.base_len).then(|| s.by_class[index - s.base_len]),
                    ),
                    None => (class_len, None),
                };
                let class = class_label.is_some();
                let pressed = states.as_ref().is_some_and(|s| index >= s.pressed);
                // Where an author wrote it (LLP 1122 D6; the source map
                // checks the name at the place against the source). A
                // conditional class's row is either style's, and a folded
                // `:disabled` or `:checked` row is a choice over the base
                // and its part: both say nowhere.
                // A written row's origin is its place's: a class's row is
                // one of the class's attributes, whatever a rewrite (an
                // animation composed, a grouped list's rows inserted) did
                // to its position, a state part's row included. A row the
                // compiler wrote goes by position as before.
                let positioned = class
                    && (folded == Some(true)
                        || folded.is_none() && index >= sheet.len() && index < class_len);
                let (in_class, sheet_row) = match a.written {
                    true => (class && class_spans.contains(&a.span), false),
                    false => (positioned, folded.is_none() && index < sheet.len()),
                };
                let written = (a.written && folded.is_none()).then(|| crate::Written {
                    span: a.span,
                    name: a.name.clone(),
                });
                let origin = if in_class {
                    let label = class_label.clone().expect("class attribute");
                    let written = written.filter(|_| !label.contains('/'));
                    Origin::Class(label, written)
                } else if sheet_row {
                    Origin::Tag
                } else {
                    Origin::Own(written)
                };
                // A folded `:disabled` or `:checked` row (LLP 1122's queue
                // item (2)): its base's attribute where an author wrote it,
                // and each part folded over it, each the class's by its span.
                let fold = (states.as_ref())
                    .filter(|s| index >= s.base_len && index < s.pressed)
                    .map(|s| &s.folds[index - s.base_len]);
                let origin = match fold {
                    Some(fold) => {
                        let class_of = |span: &Span| {
                            class_label.as_ref().filter(|_| class_spans.contains(span))
                        };
                        let base = fold.base.as_ref().map(|(span, name)| {
                            let w = crate::Written {
                                span: *span,
                                name: name.clone(),
                            };
                            Box::new(match class_of(span) {
                                Some(label) => Origin::Class(
                                    label.clone(),
                                    (!label.contains('/')).then_some(w),
                                ),
                                None => Origin::Own(Some(w)),
                            })
                        });
                        let parts = (fold.parts.iter())
                            .map(|(state, span)| Origin::State {
                                of: Box::new(match class_of(span) {
                                    Some(label) => Origin::Class(label.clone(), None),
                                    None => Origin::Own(None),
                                }),
                                state,
                                span: *span,
                            })
                            .collect();
                        Origin::Folded { base, parts }
                    }
                    None => origin,
                };
                // `:active`'s, lowered to the press paint: where it is written.
                let origin = match pressed {
                    true => Origin::State {
                        of: Box::new(origin),
                        state: "active",
                        span: a.span,
                    },
                    false => origin,
                };
                origins.resize(bindings.len(), origin);
            }
        }
        // @ref LLP 1116.006 D2 — no box where no split shows it (the web shell shows it).
        if routes::split_placeholder(expanded).is_some() {
            let none = self.fixed(true, "none");
            bindings.push(style_row(StyleId::Display, none, IMPLIED));
            if let Some(origins) = &mut origins {
                origins.resize(bindings.len(), Origin::Tag);
            }
        }
        if button.as_ref().is_some_and(|b| !b.native)
            && !bindings
                .iter()
                .any(|b| b.kind == BindingKind::Style && b.id == StyleId::Appearance as u16)
        {
            let none = self.fixed(true, "none");
            bindings.push(style_row(StyleId::Appearance, none, IMPLIED));
            if let Some(origins) = &mut origins {
                origins.resize(bindings.len(), Origin::Tag);
            }
        }
        let mut native_font = button.as_ref().map(|b| b.native);
        if t.node_type == NodeType::TextInput {
            native_font = Some(self.field_appearance(tag, expanded, *span, &mut bindings)?);
            if let Some(origins) = &mut origins {
                origins.resize(bindings.len(), Origin::Tag);
            }
        } else if let Some(a) = crate::fields::textfield_elsewhere(expanded) {
            // @ref LLP 1116.007 D4
            return err(
                "lower-appearance",
                "`appearance=\"textfield\"` is for a text field (an `input`, a `textarea`): it drops a number input's stepper and changes nothing else. Write `\"auto\"` or `\"none\"` here",
                a.span,
            );
        }
        // @ref LLP 1057.003 C6 — a transform drag needs both halves:
        // the page's geometry and the release. A handle with one is
        // never admitted by any host, so it would sit inert, unsaid.
        let has = |k: EventKind| handlers.iter().any(|(kind, ..)| *kind == k);
        let (geometry, release) = (
            has(EventKind::Transformgeometry),
            has(EventKind::Transformrelease),
        );
        if geometry != release {
            let (present, missing) = if geometry {
                ("transformgeometry", "transformrelease")
            } else {
                ("transformrelease", "transformgeometry")
            };
            let span = expanded
                .iter()
                .find(|a| a.name == present)
                .map_or(*span, |a| a.span);
            self.errors.extend(
                        err::<()>(
                            "lower-transform-drag-handlers",
                            format!("`{present}` without `{missing}`: a transform drag needs both (the page's geometry and the release), or it never starts; add `{missing}=`"),
                            span,
                        )
                        .err(),
                    );
        }
        // @ref LLP 1069.001 D8 — rows a control derives.
        if control.is_some() && controls::derived_rows(&mut bindings) {
            if let Some(origins) = &mut origins {
                origins.resize(bindings.len(), Origin::Own(None));
            }
        }
        if t.node_type == NodeType::NativeView {
            let rest: Vec<&Attr> = expanded
                .iter()
                .filter(|a| native::leftover(tag, a))
                .collect();
            let made =
                self.native_bindings(tag, &rest, scope, locals, &mut bindings, &mut handlers);
            if let Err(e) = made {
                self.errors.push(e);
            }
            if let Some(origins) = &mut origins {
                origins.resize(bindings.len(), Origin::Own(None));
            }
        }
        self.data_row(tag, expanded, scope, locals, &mut bindings, &mut origins);
        // Two bindings for one row — a style's and the node's own, a
        // tag's fixed row and an attribute — the last one wins.
        if bindings.len() > 1 {
            let mut seen: BTreeMap<(u8, u16), usize> = BTreeMap::new();
            let mut unique = 0;
            for index in 0..bindings.len() {
                let b = &bindings[index];
                let target = match seen.get(&(b.kind as u8, b.id)) {
                    Some(&i) => i,
                    None => {
                        seen.insert((b.kind as u8, b.id), unique);
                        unique += 1;
                        unique - 1
                    }
                };
                // Targets are in the consumed prefix: unread rows stay intact.
                if target != index {
                    bindings.swap(target, index);
                    if let Some(origins) = &mut origins {
                        origins.swap(target, index);
                    }
                }
            }
            bindings.truncate(unique);
            if let Some(origins) = &mut origins {
                origins.truncate(unique);
            }
        }
        let handler_refs: Vec<(EventKind, exact_plan::ActionsId, &[Code])> = handlers
            .iter()
            .map(|(e, a, c)| (*e, *a, c.as_slice()))
            .collect();
        if !children.is_empty() && !t.node_type.can_hold_children() {
            return err(
                "lower-leaf-children",
                format!("`{tag}` cannot hold children"),
                children[0].span(),
            );
        }
        if t.node_type.is_text_leaf() {
            // Regions have no node of their own: a dynamic Markdown
            // paragraph's each/when still produces only text runs.
            // An image in the text is the paragraph's own child (LLP
            // 1011.002), never a run's: its box is placed in the paragraph.
            fn run(c: &Node, images: bool) -> bool {
                match c {
                    Node::Element { tag, .. } => {
                        matches!(tag.as_str(), "text" | "time") || images && tag == "image"
                    }
                    Node::Each { body, .. } => body.iter().all(|c| run(c, images)),
                    Node::When {
                        then, otherwise, ..
                    } => then.iter().chain(otherwise).all(|c| run(c, images)),
                    _ => false,
                }
            }
            // A `text` that holds runs, not a string or a `time` value.
            let paragraph = !matches!(parent_tag, Some("text" | "time"))
                && tag == "text"
                && positional.is_empty()
                && !expanded.iter().any(|a| a.name == "text");
            // Columns lay a paragraph's lines out apart, which an image's
            // place is not read from (LLP 1011.002, "Not built").
            fn has_image(c: &Node) -> bool {
                match c {
                    Node::Element { tag, .. } => tag == "image",
                    Node::Each { body, .. } => body.iter().any(has_image),
                    Node::When {
                        then, otherwise, ..
                    } => then.iter().chain(otherwise).any(has_image),
                    _ => false,
                }
            }
            let columns = expanded
                .iter()
                .find(|a| matches!(a.name.as_str(), "columns" | "column-count" | "column-width"));
            if let (Some(a), true) = (columns, children.iter().any(has_image)) {
                return err(
                    "lower-attr-tag",
                    format!(
                        "`{}`: a multi-column `text` holds no `image` in its text",
                        a.name
                    ),
                    a.span,
                );
            }
            if let Some(bad) = children.iter().find(|c| !run(c, paragraph)) {
                return err(
                    "lower-leaf-children",
                    if paragraph {
                        "a text paragraph holds `text` or `time` runs and `image`s"
                    } else {
                        "this holds `text` or `time` runs; an `image` is the child of a `text` paragraph that holds runs alone"
                    },
                    bad.span(),
                );
            }
        }
        if let Some(styles) = &mut self.text_styles {
            let mut style = warnings::TextStyle {
                native: native_font,
                parent_horizontal_flex: self.parent_horizontal_flex,
                font: expanded
                    .iter()
                    .rev()
                    .find(|a| matches!(a.name.as_str(), "font" | "font-size"))
                    .cloned(),
                layout: expanded
                    .iter()
                    .filter(|a| {
                        matches!(
                            a.name.as_str(),
                            "width"
                                | "min-width"
                                | "max-width"
                                | "display"
                                | "position"
                                | "flex"
                                | "flex-direction"
                                | "flex-basis"
                                | "flex-grow"
                                | "popover"
                        )
                    })
                    .cloned()
                    .collect(),
            };
            styles
                .entry((*span, *instance, tag.clone()))
                .and_modify(|known| {
                    // A shared slot can have different parents but the same own style.
                    if let Some(previous) = known.as_mut() {
                        if previous.parent_horizontal_flex != style.parent_horizontal_flex {
                            previous.parent_horizontal_flex = None;
                            style.parent_horizontal_flex = None;
                        }
                    }
                    if known.as_ref() != Some(&style) {
                        *known = None;
                    }
                })
                .or_insert(Some(style));
        }
        // @ref LLP 1121 D8 — its host-known states' looks, each row lowered
        // as the node's own literal rows are (before the node is made: a
        // refusal leaves no node behind).
        let (mut look_rows, mut look_sites) = (Vec::new(), Vec::new());
        for (state, attrs) in states.as_ref().map_or(&[][..], |s| s.looks.as_slice()) {
            let mut bindings = Vec::new();
            // Each row's origin, for the map: its part's place, the class's
            // (by its span, as a base row's: a rewrite moves rows across
            // `class_len`, never their spans) or the node's.
            let mut origins = Vec::new();
            let named = match state.as_str() {
                "hover" => "hover",
                _ => "focus-visible",
            };
            for a in attrs {
                let (mut handlers, mut surface) = (Vec::new(), None);
                self.attr(
                    tag,
                    None,
                    a,
                    scope,
                    locals,
                    &mut bindings,
                    &mut handlers,
                    &mut surface,
                    &font,
                )?;
                if self.sites.is_none() {
                    continue;
                }
                let of = match &class_label {
                    Some(label) if class_spans.contains(&a.span) => {
                        Origin::Class(label.clone(), None)
                    }
                    _ => Origin::Own(None),
                };
                origins.resize(
                    bindings.len(),
                    Origin::State {
                        of: Box::new(of),
                        state: named,
                        span: a.span,
                    },
                );
            }
            if self.sites.is_some() {
                look_sites.push((named, sites::style_rows(&bindings, &origins)));
            }
            let state = match state.as_str() {
                "hover" => 1,
                _ => 2,
            };
            look_rows.push((state, bindings));
        }
        let id = self.b.node(
            t.node_type as u8,
            parent,
            arm,
            order,
            &bindings,
            &handler_refs,
            surface,
        );
        if !look_rows.is_empty() {
            self.b.set_looks(id, &look_rows);
        }
        if let Some(sites) = &mut self.sites {
            debug_assert_eq!(sites.nodes.len(), id.0 as usize);
            let mut site = sites::node_site(
                *span,
                *instance,
                parent.map(|p| p.0).or(self.site_parent),
                button,
                &bindings,
                origins.as_deref().expect("site origins"),
                look_sites,
            );
            site.part = self
                .part_names
                .iter()
                .find(|(node, _)| node == span)
                .map(|(_, part)| part.clone());
            site.library = self
                .library_nodes
                .iter()
                .find(|(node, _)| node == span)
                .map(|(_, identity)| identity.clone());
            sites.nodes.push(site);
        }
        let children = if let Some(sections) = sections {
            std::borrow::Cow::Owned(sections)
        } else if let Some(rows) = swiped {
            std::borrow::Cow::Owned(rows)
        } else if let Some(face) = face {
            std::borrow::Cow::Owned(face)
        } else if let Some(parts) = parted {
            std::borrow::Cow::Owned(parts)
        } else if let Some(bodies) =
            crate::tables::implied_bodies(tag, authored_children, *span, *instance)
        {
            std::borrow::Cow::Owned(bodies)
        } else {
            std::borrow::Cow::Borrowed(authored_children.as_slice())
        };
        Ok(ElementStep {
            id,
            children,
            tag,
            enters: tag == "svg",
            positioned: parent_tag.is_none()
                || expanded
                    .iter()
                    .rev()
                    .find(|a| a.name == "position")
                    .is_some_and(|a| !matches!(&a.value, Expr::Str(v, _) if v == "static"))
                || t.fixed_styles
                    .iter()
                    .any(|(id, v)| *id == StyleId::PositionType && *v != "static"),
            button_context: controls::button_context(tag, control, self.popover_child),
            popover_child: expanded.iter().any(|a| a.name == "popover"),
            bounded: values::bounded_column(tag, expanded, self.parent_bounded_column),
            horizontal_flex: if attrs
                .iter()
                .any(|a| a.name == "class" && !matches!(a.value, Expr::Ident(..)))
            {
                None
            } else {
                warnings::horizontal_flex(t.fixed_styles, expanded)
            },
            nav_place,
            // A row that says `delete=` is its swipe's `scroll` (swipe_rows.rs):
            // the author's row is the scroll's content, a row still.
            rows: if grouped_list {
                3
            } else if cells
                || self.row_depth == 1 && expanded.iter().any(|a| a.name == "swipeTrailing")
            {
                1
            } else {
                0
            },
        })
    }
}
