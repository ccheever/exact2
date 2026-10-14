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
    pub(super) nav_place: routes::NavPlace,
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
        let (mut sheet, unmarked) = grouped::split(attrs);
        let attrs = unmarked.as_ref().unwrap_or(attrs);
        let (class_label, mut expanded) = self.class_rows(attrs)?.unzip();
        let button = self.button_appearance(
            tag,
            expanded.as_deref().unwrap_or(&[]),
            attrs,
            children,
            button_context,
            *span,
        )?;
        if button.as_ref().is_some_and(|b| b.native) {
            grouped::native_rows(&mut sheet);
        }
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
        menus::check_menu_remove(expanded, parent_tag.is_none())?;
        values::check_position_area(expanded)?;
        tags::check_exclusion(&t, expanded, self.parent_positioned)?;
        let composed = self.compose_animation(expanded)?;
        let expanded = composed.as_deref().unwrap_or(expanded);
        let lengths =
            svg::coerce_lengths(tag, svg::in_svg(self.svg_depth > 0, parent_tag), expanded);
        let expanded = lengths.as_deref().unwrap_or(expanded);
        self.check_svg(tag, parent_tag, expanded, *span)?;
        shorthands::check_text_box(tag, parent_tag, expanded)?;
        // @ref LLP 1034 §8 — an inline run paints in its paragraph's
        // scheme: no host gives a run an appearance of its own.
        if tag == "text" && parent_tag == Some("text") {
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
        controls::check_progress(tag, expanded, children)?;
        let alert = self.check_menu_shapes(tag, expanded, children, *span)?;
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
                sections = Some(grouped::sections(style, children)?);
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
        let unsplit = self.unsplit(expanded, children, *span);
        let expanded = unsplit.as_deref().unwrap_or(expanded);
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
        // @ref LLP 1038 D8 — only the first root selects navigation.
        if has(&["navigate"])
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
        // @ref LLP 1069.001 (amended 2026-10-07) — an indeterminate
        // `progress` (no `value`, LLP 1116 D8) is busy, as ARIA's
        // `aria-busy`: a bool, which a fixed prop (text) cannot be.
        // @ref LLP 1115.000 D10.2 — a determinate one is full width, as a
        // range is; the indicator keeps its 20 by 20.
        if tag == "progress" {
            if expanded.iter().any(|a| a.name == "value") {
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
                self.errors.push(e);
            }
            if let Some(origins) = &mut origins {
                let origin = if index < sheet.len() {
                    Origin::Tag
                } else if index < class_len {
                    Origin::Class(class_label.clone().expect("class attribute"))
                } else {
                    Origin::Own
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
        if t.node_type == NodeType::TextInput {
            self.field_appearance(tag, expanded, *span, &mut bindings)?;
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
                origins.resize(bindings.len(), Origin::Own);
            }
        }
        if t.node_type == NodeType::NativeView {
            let rest: Vec<&Attr> = expanded
                .iter()
                .filter(|a| native::leftover(tag, a))
                .collect();
            if let Err(e) = self.native_bindings(tag, &rest, scope, locals, &mut bindings) {
                self.errors.push(e);
            }
            if let Some(origins) = &mut origins {
                origins.resize(bindings.len(), Origin::Own);
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
            fn run(c: &Node) -> bool {
                match c {
                    Node::Element { tag, .. } => tag == "text",
                    Node::Each { body, .. } => body.iter().all(run),
                    Node::When {
                        then, otherwise, ..
                    } => then.iter().chain(otherwise).all(run),
                    _ => false,
                }
            }
            if let Some(bad) = children.iter().find(|c| !run(c)) {
                return err(
                    "lower-leaf-children",
                    "`text` may hold only `text` runs",
                    bad.span(),
                );
            }
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
        if let Some(sites) = &mut self.sites {
            debug_assert_eq!(sites.nodes.len(), id.0 as usize);
            sites.nodes.push(sites::node_site(
                *span,
                *instance,
                button,
                &bindings,
                origins.as_deref().expect("site origins"),
            ));
        }
        let children = if let Some(sections) = sections {
            std::borrow::Cow::Owned(sections)
        } else if let Some(face) = face {
            std::borrow::Cow::Owned(face)
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
            nav_place,
        })
    }
}
