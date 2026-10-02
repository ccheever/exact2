//! Apple paragraph projection: inline kernel identities cross as values, never views.
//! @ref LLP 1044.000 §6 S1
use super::*;

impl<D: DataSource> Host<D> {
    /// An `option` or a run in one (LLP 1069.001 D2): a select's menu item,
    /// which the presenter reads from the kernel (`exact_select_options`),
    /// never a view. A native button's title and image, or a run in its
    /// title, likewise: its face, read through `exact_button_face`
    /// (LLP 1069.011 D5).
    fn option_part(&self, id: ViewId) -> bool {
        let kernel = self.runner.kernel();
        let mut at = kernel.node(id);
        while let Some(node) = at {
            if node.node_type == NodeType::Text && exact_kernel::control::is_option(&node) {
                return true;
            }
            let parent = node.parent.and_then(|p| kernel.node(p));
            if parent.as_ref().is_some_and(|p| {
                exact_kernel::ControlKind::of(p.node_type, p.props)
                    == Some(exact_kernel::ControlKind::Button)
            }) {
                return true;
            }
            if node.node_type != NodeType::Text {
                return false;
            }
            at = parent;
        }
        false
    }

    pub(super) fn paragraph_owner(&self, id: ViewId) -> Option<ViewId> {
        let kernel = self.runner.kernel();
        let mut node = kernel.node(id)?;
        if node.node_type != NodeType::Text {
            return None;
        }
        while node.is_inline_run() {
            node = kernel.node(node.parent?)?;
        }
        Some(node.id)
    }

    // A SetChildren can move a retained identity across the paragraph boundary.
    // Reconcile only that changed subtree; ordinary run updates never walk siblings.
    fn reconcile_projection(&mut self, id: ViewId, batch: &mut Batch) -> bool {
        let owner = self.paragraph_owner(id).filter(|owner| *owner != id);
        let previous = self.inline_runs.get(&id).map(|(owner, _)| *owner);
        if owner == previous {
            return false;
        }
        let events = if let Some((old, events)) = self.inline_runs.remove(&id) {
            self.dirty_paragraphs.insert(old);
            events
        } else {
            self.runner.handlers_of(id)
        };
        if self.mirror.remove(&id).is_some() {
            batch.destroy(id);
        }
        self.create(id, &events, batch);
        self.emit_children(id, batch);
        true
    }

    pub(super) fn stage_native_paragraphs(
        nodes: &[crate::content_region::CandidateNativeNode],
        batch: &mut Batch,
    ) {
        let by_id: BTreeMap<_, _> = nodes.iter().map(|n| (n.header.id, n)).collect();
        for paragraph in nodes
            .iter()
            .filter(|n| n.header.kind == "text" && n.header.inline_owner.is_none())
        {
            let owner = paragraph.header.id;
            let active = !paragraph.mirror.props.contains_key("text");
            let mut stack: Vec<_> = paragraph
                .mirror
                .children
                .iter()
                .rev()
                .map(|id| (*id, owner, active))
                .collect();
            let mut runs = String::from("[");
            let mut first = true;
            while let Some((id, parent, active)) = stack.pop() {
                let Some(node) = by_id.get(&id) else {
                    continue;
                };
                if node.header.inline_owner != Some(owner) {
                    continue;
                }
                let paints = active && node.mirror.props.contains_key("text");
                if !first {
                    runs.push(',');
                }
                first = false;
                Batch::inline_run(
                    &mut runs,
                    id,
                    parent,
                    &node.mirror.props,
                    &node.mirror.style,
                    &node.header.handlers,
                    paints,
                );
                stack.extend(
                    node.mirror
                        .children
                        .iter()
                        .rev()
                        .map(|child| (*child, id, active && !paints)),
                );
            }
            runs.push(']');
            batch.paragraph(owner, &runs);
        }
    }

    pub(super) fn emit_paragraphs(&mut self, batch: &mut Batch) {
        for owner in std::mem::take(&mut self.dirty_paragraphs) {
            let kernel = self.runner.kernel();
            let Some(node) = kernel.node(owner) else {
                continue;
            };
            if node.is_inline_run() || self.native_protected_id(owner) {
                continue;
            }
            let active = node.props.str(PropId::Text).is_none();
            let mut stack: Vec<_> = node
                .children()
                .into_iter()
                .rev()
                .map(|id| (id, active))
                .collect();
            let mut runs = String::from("[");
            let mut first = true;
            while let Some((id, active)) = stack.pop() {
                let Some(run) = kernel.node(id) else {
                    continue;
                };
                // Non-text controls are never absorbed into a paragraph.
                if !run.is_inline_run() {
                    continue;
                }
                let props = props_for(&run);
                // A run painting an inherited `color` that moves (LLP 1062 D5).
                let mut shown = style::Shown::default();
                shown.set(Property::Color, self.paint.runs.get(&id).copied());
                let (style, _) = style::style_json_presented(&run, &kernel.env(), &shown);
                let handlers: Vec<_> = self
                    .inline_runs
                    .get(&id)
                    .into_iter()
                    .flat_map(|(_, events)| events.iter().copied())
                    .filter_map(handler_name)
                    .collect();
                let paints = active && run.props.str(PropId::Text).is_some();
                if !first {
                    runs.push(',');
                }
                first = false;
                Batch::inline_run(
                    &mut runs,
                    id,
                    run.parent.unwrap_or(owner),
                    &props,
                    &style,
                    &handlers,
                    paints,
                );
                stack.extend(
                    run.children()
                        .into_iter()
                        .rev()
                        .map(|child| (child, active && !paints)),
                );
            }
            runs.push(']');
            batch.paragraph(owner, &runs);
        }
    }

    pub(super) fn create(&mut self, id: ViewId, events: &[EventKind], batch: &mut Batch) {
        // @ref LLP 1055 D4 — an SVG element is in its `svg`'s scene, not a view.
        if self.svg.element(self.runner.kernel(), id).is_some() {
            let key = self.runner.kernel().node(id).expect("live").key;
            self.keys.insert(key, id);
            // @ref LLP 1055.000 D17 — the presenter hits it by `pointer-events`.
            self.svg.handlers(id, events.contains(&EventKind::Press));
            return;
        }
        if self.option_part(id) {
            let key = self.runner.kernel().node(id).expect("live").key;
            self.keys.insert(key, id);
            return;
        }
        self.queue_layout(id);
        if let Some(owner) = self.paragraph_owner(id) {
            self.dirty_paragraphs.insert(owner);
            if owner != id {
                let node = self.runner.kernel().node(id).expect("live");
                self.keys.insert(node.key, id);
                self.inline_runs.insert(id, (owner, events.to_vec()));
                return;
            }
        }
        self.track_height_transition(id);
        if self.native_protected_id(id) {
            if let Some(node) = self.runner.kernel().node(id) {
                self.keys.insert(node.key, id);
            }
            return;
        }
        let node = self.runner.kernel().node(id).expect("live");
        let key = node.key;
        // @ref LLP 1056 D10 — a 2D canvas is a plain view: its bitmap is a
        // layer's contents under ordinary children, with no Metal or overlay.
        let kind = if self.runner.is_canvas_2d(id) {
            "canvas2d"
        } else {
            kind_for(&node)
        };
        let props = props_for(&node);
        let env = self.runner.kernel().env();
        let (style, _skipped) = style::style_json_for(&node, &env);
        let handlers: Vec<&str> = events.iter().copied().filter_map(handler_name).collect();
        if handlers.contains(&"heightrelease") {
            self.track_height_handle(id);
        }
        if handlers.contains(&"transformgeometry") || handlers.contains(&"transformrelease") {
            self.transform_drags.insert(
                id,
                key,
                handlers.contains(&"transformgeometry"),
                handlers.contains(&"transformrelease"),
            );
        }
        let pairs: Vec<(&str, String)> =
            props.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        batch.create(id, kind, &pairs, &style, &handlers);
        self.mirror.insert(
            id,
            Mirror {
                props,
                style,
                ..Mirror::default()
            },
        );
        self.keys.insert(key, id);
    }

    pub(super) fn update(&mut self, id: ViewId, batch: &mut Batch) {
        if self.svg.element(self.runner.kernel(), id).is_some() || self.option_part(id) {
            return;
        }
        self.queue_layout(id);
        if self.reconcile_projection(id, batch) {
            return;
        }
        if let Some(owner) = self.paragraph_owner(id) {
            self.dirty_paragraphs.insert(owner);
            if owner != id {
                return;
            }
        }
        self.track_height_transition(id);
        if self.native_protected_id(id) {
            return;
        }
        let node = self.runner.kernel().node(id).expect("live");
        let props = props_for(&node);
        let env = self.runner.kernel().env();
        let (style, _skipped) = style::style_json_for(&node, &env);
        if self.mirror.get(&id).and_then(|m| m.props.get("spellcheck")) != props.get("spellcheck") {
            self.queue_layout_subtree(id);
        }
        let m = self.mirror.entry(id).or_default();
        if props != m.props {
            let set: Vec<(&str, String)> = props
                .iter()
                .filter(|(k, v)| m.props.get(*k) != Some(*v))
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            let clear: Vec<&str> = m
                .props
                .keys()
                .filter(|k| !props.contains_key(*k))
                .map(String::as_str)
                .collect();
            batch.props(id, &set, &clear);
            m.props = props;
        }
        if style != m.style {
            batch.style(id, &style);
            m.style = style;
        }
    }

    pub(super) fn emit_children(&mut self, id: ViewId, batch: &mut Batch) {
        // An `svg`'s and a `g`'s children are its scene's (LLP 1055 D4).
        let node_type = self.runner.kernel().node(id).expect("live").node_type;
        if node_type == NodeType::Svg || node_type.is_svg_element() {
            self.svg.element(self.runner.kernel(), id);
            return;
        }
        // A select's options are its menu's, not views (LLP 1069.001 D2).
        if node_type == NodeType::Control {
            return;
        }
        let children = self.runner.kernel().node(id).expect("live").children();
        if self.mirror.get(&id).is_none_or(|m| m.children != children) {
            // Retained children still inherit from the same parent. Only an
            // arriving subtree needs its descendants' spelling hints revisited;
            // changed ancestor hints already queue their subtree in update().
            let previous: IdSet<_> = self
                .mirror
                .get(&id)
                .into_iter()
                .flat_map(|m| m.children.iter().copied())
                .collect();
            self.queue_layout(id);
            for child in &children {
                // A child this batch creates was queued at its create, and so
                // was each node under it: walking its subtree again from every
                // ancestor made a new row's mount quadratic in its depth.
                if !previous.contains(child) && !batch.creates(*child) {
                    self.queue_layout_subtree(*child);
                }
            }
        }
        for child in &children {
            self.reconcile_projection(*child, batch);
        }
        if self.native_protected_id(id) || self.paragraph_owner(id).is_some() {
            return;
        }
        let mut children = children;
        if self.native_mode() {
            children.retain(|child| {
                !self.native_protected_id(*child) || self.native_selected_id(*child)
            });
        }
        let m = self.mirror.entry(id).or_default();
        if children != m.children {
            batch.children(id, &children);
            m.children = children;
        }
    }

    /// A box whose paint is moving (LLP 1055.000 D6, LLP 1062): its style
    /// with the presented values over its rows, and the style of every
    /// descendant that inherits its `color` — an inline run's through its
    /// paragraph, which paints it (LLP 1062 D5). A leaving view is no
    /// mirror's, and still paints its exit's colours (LLP 1063). When the
    /// motion ends the rows show again, re-sent the same way.
    pub(super) fn present_colors(&mut self, view: ViewId, batch: &mut Batch, inherits: bool) {
        let kernel = self.runner.kernel();
        let Some(node) = kernel.node(view) else {
            return self.restyle_leaving(view, batch);
        };
        let key = motion_node(node.key);
        let shown = self.shown_paint(key);
        let color = shown.get(Property::Color);
        let env = kernel.env();
        let mut restyled = vec![(view, style::style_json_presented(&node, &env, &shown).0)];
        let mut runs = Vec::new();
        if inherits {
            let mut inherited = style::Shown::default();
            inherited.set(Property::Color, color);
            let mut stack = node.children();
            while let Some(child) = stack.pop() {
                let Some(c) = kernel.node(child) else {
                    continue;
                };
                if c.source_of(exact_kernel::StyleId::TextColor) != Some(view) {
                    continue;
                }
                if let Some((owner, _)) = self.inline_runs.get(&child) {
                    runs.push((child, *owner));
                } else {
                    restyled.push((child, style::style_json_presented(&c, &env, &inherited).0));
                }
                stack.extend(c.children());
            }
        }
        for (run, owner) in runs {
            let before = match color {
                Some(c) => self.paint.runs.insert(run, c),
                None => self.paint.runs.remove(&run),
            };
            if before != color {
                self.dirty_paragraphs.insert(owner);
            }
        }
        for (id, style) in restyled {
            if self.svg.touch(self.runner.kernel(), id) {
                continue;
            }
            let Some(m) = self.mirror.get_mut(&id) else {
                continue;
            };
            if m.style != style {
                batch.style(id, &style);
                m.style = style;
            }
        }
        if !self.dirty_paragraphs.is_empty() {
            self.emit_paragraphs(batch);
        }
    }
}
