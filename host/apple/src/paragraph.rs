//! Apple paragraph projection: inline kernel identities cross as values, never views.
//! @ref LLP 1044.000 §6 S1
use super::*;

impl<D: DataSource> Host<D> {
    /// Every node, parents before children: the order a whole tree's views
    /// are made in.
    pub(super) fn preorder(&self) -> Vec<ViewId> {
        let kernel = self.runner.kernel();
        let mut stack: Vec<ViewId> = self.runner.roots().into_iter().rev().collect();
        let mut order = Vec::new();
        while let Some(id) = stack.pop() {
            order.push(id);
            if let Some(node) = kernel.node(id) {
                let mut children = node.children();
                children.reverse();
                stack.extend(children);
            }
        }
        order
    }

    /// An `option` or a run in one (LLP 1069.001 D2): a select's menu item,
    /// which the presenter reads from the kernel (`exact_select_options`),
    /// never a view. A native button's title and image, or a run in its
    /// title, likewise: its face, read through `exact_press_face`
    /// (LLP 1069.011 D5).
    pub(super) fn option_part(&self, id: ViewId) -> bool {
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

    /// A drawn face's box (a title's avatar, LLP 1075.003 §9.10): a View
    /// with a fill and authored width and height in points. Its points go as
    /// `faceBoxSize` `"WxH"`, because a header the bar replaces is laid out
    /// as `display: none`, so no frame tells the host the size the author
    /// gave it. The test reads the node alone, never its ancestors, and of
    /// its children only that there are some: a box moved into or out of a
    /// header needs no recomputation, and initials beside a status dot are
    /// still a face.
    fn face_box_size(&self, id: ViewId) -> Option<String> {
        let kernel = self.runner.kernel();
        let node = kernel.node(id)?;
        // A childless box is decoration (a dot), not a face.
        if node.node_type != NodeType::View || kernel.arena().children(node.key.index).is_empty() {
            return None;
        }
        let env = kernel.arena().env();
        let (exact_kernel::Dimension::Points(w), exact_kernel::Dimension::Points(h)) = (
            node.style.width.resolve(env),
            node.style.height.resolve(env),
        ) else {
            return None;
        };
        // No `background-color` is `currentcolor`: the text colour fills it.
        let fill = node
            .style
            .background_color
            .unwrap_or_else(|| node.text_color());
        (fill.resolve(false).a() != 0 || fill.resolve(true).a() != 0).then(|| format!("{w}x{h}"))
    }

    /// The paragraph an image in its text belongs to (LLP 1011.002): its
    /// box is one of that paragraph's runs, so a change to the image is a
    /// change to its runs.
    pub(super) fn image_paragraph(&self, id: ViewId) -> Option<ViewId> {
        let node = self.runner.kernel().node(id)?;
        if !node.is_inline_image() {
            return None;
        }
        self.paragraph_owner(node.parent?)
    }

    /// An image in a paragraph's text took a new natural size: its box is a
    /// run of that paragraph, which paints again with it.
    pub(super) fn inline_image_resized(&mut self, view: ViewId, batch: &mut Batch) {
        if let Some(owner) = self.image_paragraph(view) {
            self.dirty_paragraphs.insert(owner);
            self.emit_paragraphs(batch);
        }
    }

    /// Lay out again after `view` took a new natural size.
    pub(super) fn layout_resized(&mut self, view: ViewId, batch: &mut Batch) -> Result<(), String> {
        self.inline_image_resized(view, batch);
        self.layout(batch)
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
                // Non-text controls are never absorbed into a paragraph; an
                // image in its text is a run of its box (LLP 1011.002).
                let inline = run.inline_box();
                if !run.is_inline_run() && inline.is_none() {
                    continue;
                }
                let mut props = props_for(&run);
                if let Some(b) = inline {
                    props.insert(PropId::Text.name(), exact_kernel::INLINE_BOX_TEXT.into());
                    // Its own `visibility`: its style here is its paragraph's.
                    if run
                        .computed_style(exact_kernel::StyleMask::of(
                            exact_kernel::StyleId::Visibility,
                        ))
                        .visibility
                        == exact_kernel::Visibility::Hidden
                    {
                        props.insert("inlineBoxHidden", "true".into());
                    }
                    props.insert(
                        "inlineBox",
                        format!(
                            "{} {} {} {}",
                            style::num(b.width),
                            style::num(b.height),
                            b.align as u8,
                            style::num(b.shift)
                        ),
                    );
                }
                // A run painting an inherited `color` that moves (LLP 1062 D5).
                let mut shown = style::Shown::default();
                shown.set(Property::Color, self.paint.runs.get(&id).copied());
                // An image's box is in its paragraph's font, as the kernel measures it.
                let styled = inline.and(run.parent).and_then(|p| kernel.node(p));
                let (style, _) = style::style_json_inherited(
                    styled.as_ref().unwrap_or(&run),
                    &kernel.env(),
                    &shown,
                );
                let handlers: Vec<_> = self
                    .inline_runs
                    .get(&id)
                    .into_iter()
                    .flat_map(|(_, events)| events.iter().copied())
                    .filter_map(handler_name)
                    .collect();
                let paints = active && (inline.is_some() || run.props.str(PropId::Text).is_some());
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

    /// @ref LLP 1075.003.000 §3.3 — a hatched node this host draws without a
    /// view of its own: its hatch reaches nothing here, journaled once a word.
    fn viewless_hatch(&mut self, id: ViewId, why: &str) {
        let word = self
            .runner
            .kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Hatch).map(str::to_owned));
        if let Some(word) = word.filter(|w| self.hatches.viewless.insert(w.clone())) {
            self.log(&format!(
                "hatch element {word}: {why}; its hatch is not called on this host"
            ));
        }
    }

    pub(super) fn create(&mut self, id: ViewId, events: &[EventKind], batch: &mut Batch) {
        // @ref LLP 1055 D4 — an SVG element is in its `svg`'s scene, not a view.
        if self.svg.element(self.runner.kernel(), id).is_some() {
            let key = self.runner.kernel().node(id).expect("live").key;
            self.keys.insert(key, id);
            self.viewless_hatch(id, "an SVG element is its scene's, not a view");
            // @ref LLP 1055.000 D17 — the presenter hits it by `pointer-events`.
            self.svg.handlers(id, events.contains(&EventKind::Press));
            return;
        }
        if self.option_part(id) {
            batch.controls = true;
            let key = self.runner.kernel().node(id).expect("live").key;
            self.keys.insert(key, id);
            self.viewless_hatch(id, "an option's content is its control's, not a view");
            return;
        }
        self.queue_layout(id);
        if let Some(owner) = self.paragraph_owner(id) {
            self.dirty_paragraphs.insert(owner);
            if owner != id {
                let node = self.runner.kernel().node(id).expect("live");
                self.keys.insert(node.key, id);
                self.inline_runs.insert(id, (owner, events.to_vec()));
                self.viewless_hatch(id, "an inline text run is its paragraph's text, not a view");
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
        let mut props = self.props_of(&node);
        if let Some(size) = self.face_box_size(id) {
            props.insert("faceBoxSize", size);
        }
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
        let pairs: Vec<(&str, String)> = props.iter().map(|(k, v)| (*k, v.clone())).collect();
        batch.create(id, kind, &pairs, &style, &handlers);
        // Kept to compare with the next update, as `update` keeps it.
        let style = self.style_texts.share(style);
        self.mirror.insert(
            id,
            Box::new(Mirror {
                props,
                style,
                ..Mirror::default()
            }),
        );
        self.keys.insert(key, id);
    }

    pub(super) fn update(&mut self, id: ViewId, batch: &mut Batch) {
        if self.svg.element(self.runner.kernel(), id).is_some() {
            return;
        }
        if self.option_part(id) {
            // Its control reads it from the kernel: say so, as no op will.
            batch.controls = true;
            return;
        }
        self.queue_layout(id);
        if self.reconcile_projection(id, batch) {
            return;
        }
        if let Some(owner) = self.image_paragraph(id) {
            self.dirty_paragraphs.insert(owner);
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
        let mut props = self.props_of(&node);
        if let Some(size) = self.face_box_size(id) {
            props.insert("faceBoxSize", size);
        }
        let env = self.runner.kernel().env();
        let (style, _skipped) = style::style_json_for(&node, &env);
        if self.mirror.get(&id).and_then(|m| m.props.get("spellcheck")) != props.get("spellcheck") {
            self.queue_layout_subtree(id);
        }
        let m = self.mirror.entry(id).or_default();
        // What layout derives (`emit_layout`) is layout's to set and clear.
        // Cleared here as unauthored, a growing list's `flexGrows` went and
        // came back in every batch that touched the list: each row of a
        // scrolling list of tasks reconfigured its checkbox for it.
        for key in super::layout::DERIVED_PROPS {
            if let (Some(value), false) = (m.props.get(key), props.contains_key(key)) {
                props.insert(key, value.clone());
            }
        }
        if props != m.props {
            let set: Vec<(&str, String)> = props
                .iter()
                .filter(|(k, v)| m.props.get(*k) != Some(*v))
                .map(|(k, v)| (*k, v.clone()))
                .collect();
            let clear: Vec<&str> = m
                .props
                .keys()
                .filter(|k| !props.contains_key(*k))
                .copied()
                .collect();
            batch.props(id, &set, &clear);
            m.props = props;
        }
        if *m.style != *style {
            batch.style(id, &style);
            m.style = self.style_texts.share(style);
        }
        self.emit_separators_beside(id, batch);
    }

    pub(super) fn emit_children(&mut self, id: ViewId, batch: &mut Batch) {
        // An `svg`'s and a `g`'s children are its scene's (LLP 1055 D4).
        let node_type = self.runner.kernel().node(id).expect("live").node_type;
        if node_type == NodeType::Svg || node_type.is_svg_element() {
            self.svg.element(self.runner.kernel(), id);
            return;
        }
        // A select's options are its menu's, not views (LLP 1069.001 D2). A
        // control this batch creates is configured for its create op; saying
        // so for it too made every list row with a checkbox a batch that
        // configures every control and syncs the navigation chrome.
        if node_type == NodeType::Control {
            if !batch.creates(id) {
                batch.controls = true;
            }
            return;
        }
        let mut children = self.runner.kernel().node(id).expect("live").children();
        if self
            .mirror
            .get(&id)
            .is_none_or(|m| *m.children != *children)
        {
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
        if self.native_protected_id(id) {
            return;
        }
        // A paragraph's views are its images' (LLP 1011.002): its runs are its text.
        if let Some(owner) = self.paragraph_owner(id) {
            let kernel = self.runner.kernel();
            children.retain(|c| kernel.node(*c).is_some_and(|n| n.is_inline_image()));
            let had = self.mirror.get(&id).is_some_and(|m| !m.children.is_empty());
            if owner != id || children.is_empty() && !had {
                return;
            }
        }
        if self.native_mode() {
            children.retain(|child| {
                !self.native_protected_id(*child) || self.native_selected_id(*child)
            });
        }
        let m = self.mirror.entry(id).or_default();
        if *children != *m.children {
            batch.children(id, &children);
            m.children = children.into_boxed_slice();
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
                    restyled.push((child, style::style_json_inherited(&c, &env, &inherited).0));
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
            if *m.style != *style {
                batch.style(id, &style);
                m.style = self.style_texts.share(style);
            }
        }
        if !self.dirty_paragraphs.is_empty() {
            self.emit_paragraphs(batch);
        }
    }
}

/// The platform a hatch word names for this build: tvOS bakes as iOS.
const HATCH_PLATFORM: &str = if cfg!(target_os = "macos") {
    "macos"
} else {
    "ios"
};

/// The hatch words a host keeps: those journaled as having no view here (an
/// inline run's), and those the plan does not give this platform (LLP
/// 1075.003.000.001 §4.3), whose nodes reach Swift unhatched.
#[derive(Default)]
pub(crate) struct HatchWords {
    pub(crate) viewless: std::collections::BTreeSet<String>,
    off: std::collections::BTreeSet<String>,
}

impl HatchWords {
    /// With the words the plan declares that this platform's bit is not set for.
    pub(crate) fn of(plan: &exact_plan::Plan) -> Self {
        let off = plan
            .hatches
            .iter()
            .map(|row| plan.str(row.word))
            .filter(|word| !plan.handles_hatch(word, HATCH_PLATFORM))
            .map(str::to_owned)
            .collect();
        HatchWords {
            viewless: Default::default(),
            off,
        }
    }
}

impl<D: DataSource> Host<D> {
    /// A node's props as Swift gets them: a hatch word the plan does not give
    /// this platform is sent as `hatchOff`, so the node is an ordinary one
    /// there and Swift lists the word as unhandled.
    pub(crate) fn props_of(
        &self,
        node: &NodeRef<'_>,
    ) -> exact_kernel::SortedMap<&'static str, String> {
        let mut props = props_for(node);
        if let Some(line) = super::layout::separator::json(self.runner.kernel(), node.id) {
            props.insert(super::layout::separator::PROP, line);
        }
        if node.props.bool(PropId::DefaultSurface) == Some(true) {
            if let Some((radius, shadow)) = default_surface(exact_kernel::style::metrics::column())
            {
                props.insert("defaultSurfaceRadius", radius);
                props.insert("defaultSurfaceShadow", shadow);
            }
        }
        if !self.hatches.off.is_empty() {
            if let Some(word) = props.get(PropId::Hatch.name()) {
                if self.hatches.off.contains(word) {
                    let word = props.remove(PropId::Hatch.name()).unwrap_or_default();
                    props.insert("hatchOff", word);
                }
            }
        }
        props
    }
}

/// A popover's default surface (`defaultSurface`) as `column` says: its
/// corner (`popover-radius`) and its shadow's offset down, CSS blur and
/// black's alpha (`popover-shadow-`), none where the platform draws its own.
fn default_surface(column: exact_kernel::style::metrics::Column) -> Option<(String, String)> {
    use exact_kernel::generated::metric;
    use exact_kernel::style::metrics::{length_in, number_in};
    let radius = length_in(metric::POPOVER_RADIUS, column, 0.0, 0.0)?;
    let y = length_in(metric::POPOVER_SHADOW_Y, column, 0.0, 0.0)?;
    let blur = length_in(metric::POPOVER_SHADOW_BLUR, column, 0.0, 0.0)?;
    let alpha = number_in(metric::POPOVER_SHADOW_ALPHA, column)?;
    Some((radius.to_string(), format!("{y} {blur} {alpha}")))
}

/// Props by their own names, including derived text advances.
pub(super) fn props_for(node: &NodeRef<'_>) -> exact_kernel::SortedMap<&'static str, String> {
    let mut out = exact_kernel::SortedMap::default();
    for (id, value) in node.props.iter() {
        let text = match value {
            PropValue::Str(_) if id == PropId::Href && node.props.link().is_none() => continue,
            PropValue::Str(s) => s.to_string(),
            PropValue::Bool(b) => b.to_string(),
            PropValue::Int(i) => i.to_string(),
            PropValue::Float(f) if matches!(id, PropId::Datetime | PropId::Start | PropId::End) => {
                f.to_string()
            }
            PropValue::Float(f) => style::num(*f as f32),
        };
        out.insert(id.name(), text);
    }
    // Swift paints the measured paragraph with text-transform applied (LLP 1064 D5).
    if let Some(std::borrow::Cow::Owned(shown)) = node.shown_text() {
        out.insert(PropId::Text.name(), shown);
    }
    style::glass_auto_props(node, &mut out); // LLP 1053.000.000.000 D2
                                             // An image in the text is read in its paragraph's words (LLP 1011.002 D4).
    if node.is_inline_image() {
        out.insert(PropId::AccessibilityElementsHidden.name(), "true".into());
    }
    if let Some(scale) = fit_scale(node) {
        out.insert("fitScale", scale);
    }
    if node.node_type == NodeType::List && node.props.bool(PropId::Virtualized) == Some(true) {
        // The runner preserves collection anchors and follows the end using
        // sequence-checked corrections. Eager native autoscroll would compete.
        out.remove(PropId::ScrollFollowEnd.name());
    }
    if node.node_type == NodeType::TextInput {
        out.remove("spellcheck");
        if let Some(value) = node.spellcheck() {
            out.insert("spellcheck", value.to_string());
        }
    }
    if node.node_type == NodeType::Image {
        if let Some(role) = node
            .props
            .str(PropId::ImageSource)
            .and_then(|s| s.strip_prefix("symbol:"))
        {
            out.insert("symbolName", exact_kernel::apple_symbol_name(role).into());
            // @ref LLP 1121 D10 — an svg-only icon's path, which Apple draws (stage 9 A).
            if let Some((d, view_box, even_odd)) = crate::icon_path::commands(role) {
                out.insert("symbolPath", d);
                out.insert("symbolViewBox", view_box.into());
                // Stage 14 C2: an even-odd icon says so; a nonzero one sends what it did.
                if even_odd {
                    out.insert("symbolFillRule", "evenodd".into());
                }
            }
            // Untinted, it draws in its `color` (LLP 1121 stage 9 B).
            if node.symbol_color_said() {
                out.insert("symbolColorWritten", "1".into());
            }
        }
    }
    out
}

/// The style texts the mirrors hold, one allocation per distinct text: a
/// grid's cells and a list's rows repeat one style hundreds of times
/// (euxy's first screen: 212 texts of 877 bytes, one of them). A text no
/// mirror holds any more leaves at the next sweep, which runs when the
/// table has doubled since the last.
#[derive(Debug, Default)]
pub(crate) struct StyleTexts {
    texts: std::collections::HashSet<std::rc::Rc<str>>,
    sweep_at: usize,
}

impl StyleTexts {
    /// `text` as the one shared allocation of its content.
    pub(crate) fn share(&mut self, text: String) -> std::rc::Rc<str> {
        if let Some(found) = self.texts.get(text.as_str()) {
            return found.clone();
        }
        if self.texts.len() >= self.sweep_at {
            self.texts.retain(|t| std::rc::Rc::strong_count(t) > 1);
            self.sweep_at = (self.texts.len() * 2).max(256);
        }
        let shared: std::rc::Rc<str> = text.into();
        self.texts.insert(shared.clone());
        shared
    }
}

#[cfg(test)]
#[path = "popover_surface_tests.rs"]
mod popover_surface_tests;

#[cfg(test)]
#[path = "icon_fill_rule_tests.rs"]
mod icon_fill_rule_tests;
