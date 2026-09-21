//! Apple paragraph projection: inline kernel identities cross as values, never views.
//! @ref LLP 1044.000 §6 S1
use super::*;

impl<D: DataSource> Host<D> {
    fn paragraph_owner(&self, id: ViewId) -> Option<ViewId> {
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
                let (style, _) = style::style_json_for(&run, &kernel.env());
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
        let kind = kind_for(&node);
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
        for child in self.runner.kernel().node(id).expect("live").children() {
            self.reconcile_projection(child, batch);
        }
        if self.native_protected_id(id) || self.paragraph_owner(id).is_some() {
            return;
        }
        let mut children = self.runner.kernel().node(id).expect("live").children();
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
}
