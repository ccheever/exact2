//! Generic tree consumers visit only mounted authored rows, never private wrappers.
use super::*;
impl Collection {
    pub(in crate::instance) fn find(
        &self,
        view: ViewId,
        frames: &mut Vec<Frame>,
    ) -> Option<NodesId> {
        for row in &self.mounted {
            frames.push(row.row.frame.clone());
            for child in &row.row.roots {
                let found = match child {
                    Child::Node(n) => n.find(view, frames),
                    Child::Region(r) => r.find(view, frames),
                };
                if found.is_some() {
                    return found;
                }
            }
            frames.pop();
        }
        None
    }
    pub(in crate::instance) fn site(
        &self,
        view: ViewId,
        path: &mut Vec<InstanceStep>,
    ) -> Option<NodesId> {
        for row in &self.mounted {
            path.push(InstanceStep::Row {
                region: self.region,
                key: row.row.key.clone(),
            });
            for child in &row.row.roots {
                let found = match child {
                    Child::Node(n) => n.site(view, path),
                    Child::Region(r) => r.site(view, path),
                };
                if found.is_some() {
                    return found;
                }
            }
            path.pop();
        }
        None
    }
    pub(in crate::instance) fn add_children<'a>(&'a self, stack: &mut Vec<&'a Child>) {
        for row in &self.mounted {
            stack.extend(row.row.roots.iter());
        }
    }
}
impl Tree {
    /// O(live instances) snapshots. No unmounted record/key serialization.
    pub fn collections(&self) -> Vec<CollectionSnapshot> {
        if !self.has_collections {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut stack: Vec<_> = self.children.iter().collect();
        while let Some(child) = stack.pop() {
            match child {
                Child::Node(node) => {
                    if let Some(collection) = &node.collection {
                        out.push(collection.snapshot());
                        collection.add_children(&mut stack);
                    }
                    stack.extend(node.children.iter());
                }
                Child::Region(region) => match &region.active {
                    Active::Arm { roots, .. } => stack.extend(roots.iter()),
                    Active::Rows { rows } => {
                        for row in rows {
                            stack.extend(row.roots.iter());
                        }
                    }
                },
            }
        }
        out.sort_by_key(|c| c.view);
        out
    }
    /// Checked host projection: traversal storage is bounded before reservation;
    /// count every row/collection before allocating any owned snapshot or row.
    pub fn collections_bounded(
        &self,
        max_collections: usize,
        max_rows: usize,
        max_traversal: usize,
        max_json_bytes: usize,
    ) -> Result<Vec<CollectionSnapshot>, &'static str> {
        if !self.has_collections {
            return Ok(Vec::new());
        }
        let mut stack = Vec::new();
        stack
            .try_reserve_exact(max_traversal)
            .map_err(|_| "collection stack allocation")?;
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(max_traversal)
            .map_err(|_| "collection root stack allocation")?;
        let mut counts = (0usize, 0usize);
        // First pass is borrowed only, including each wrapper's first authored root.
        self.visit_collections_bounded(&mut stack, max_traversal, |c| {
            counts.0 = counts.0.checked_add(1).ok_or("collection count overflow")?;
            counts.1 = counts
                .1
                .checked_add(c.mounted.len())
                .ok_or("collection row overflow")?;
            if counts.0 > max_collections || counts.1 > max_rows {
                return Err("collection projection capacity");
            }
            for row in &c.mounted {
                first_root_bounded(&row.row.roots, &mut roots, max_traversal)?;
            }
            Ok(())
        })?;
        // JSON has no source strings. 1024 per row includes two worst-case
        // finite f64 decimal spellings, integer identities, names and punctuation;
        // 1024 per collection includes extent and correction. No snapshot clone
        // precedes this conservative wire admission.
        let wire = counts
            .0
            .checked_add(counts.1)
            .and_then(|n| n.checked_mul(1024))
            .and_then(|n| n.checked_add(2))
            .ok_or("collection wire overflow")?;
        if wire > max_json_bytes {
            return Err("collection wire capacity");
        }
        let mut out = Vec::new();
        out.try_reserve_exact(counts.0)
            .map_err(|_| "collection snapshot allocation")?;
        self.visit_collections_bounded(&mut stack, max_traversal, |c| {
            let mut rows = Vec::new();
            rows.try_reserve_exact(c.mounted.len())
                .map_err(|_| "collection rows allocation")?;
            for row in &c.mounted {
                rows.push(CollectionRow {
                    view: row.wrapper,
                    root: first_root_bounded(&row.row.roots, &mut roots, max_traversal)?,
                    index: row.position,
                    top: c.index.prefix(row.position).unwrap(),
                    height: c.index.height(row.position).unwrap(),
                    epoch: row.epoch,
                    measured: c.index.is_measured(c.index.key(row.position).unwrap()),
                });
            }
            out.push(CollectionSnapshot {
                view: c.view,
                revision: c.revision,
                scroll_sequence: c.geometry.as_ref().map_or(0, |g| g.scroll_sequence),
                count: c.index.len(),
                total_extent: c.index.total_height(),
                rows,
                correction: c.correction,
            });
            Ok(())
        })?;
        out.sort_unstable_by_key(|c| c.view);
        Ok(out)
    }
    fn visit_collections_bounded<'a>(
        &'a self,
        stack: &mut Vec<&'a Child>,
        limit: usize,
        mut visit: impl FnMut(&'a Collection) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        stack.clear();
        push_children_bounded(stack, &self.children, limit)?;
        let mut visited = 0usize;
        while let Some(child) = stack.pop() {
            visited = visited
                .checked_add(1)
                .ok_or("collection traversal overflow")?;
            if visited > limit {
                return Err("collection traversal capacity");
            }
            match child {
                Child::Node(n) => {
                    if let Some(c) = &n.collection {
                        visit(c)?;
                        for row in &c.mounted {
                            push_children_bounded(stack, &row.row.roots, limit)?;
                        }
                    }
                    push_children_bounded(stack, &n.children, limit)?;
                }
                Child::Region(r) => push_active_bounded(stack, &r.active, limit)?,
            }
        }
        Ok(())
    }
    /// Targeted geometry update. Never evaluates collection data or key expressions,
    /// settles resources, or traverses unmounted rows. An edge is returned only
    /// for accepted geometry; the runner dispatches it after committing the ops.
    pub(crate) fn update_collection(
        &mut self,
        u: &mut Update<'_>,
        feedback: CollectionFeedback,
    ) -> Result<(bool, Option<CollectionEdges>), InstanceError> {
        if !self.has_collections {
            self.last_work = u.work;
            return Ok((false, None));
        }
        feedback
            .validate()
            .map_err(|_| invalid("invalid collection feedback"))?;
        let Some(target) = find_collection(&self.children, feedback.view) else {
            self.last_work = u.work;
            return Ok((false, None));
        };
        let Some(by_view) = target.prepare_feedback(&feedback)? else {
            self.last_work = u.work;
            return Ok((false, None));
        };
        let categories = [
            feedback.focus_view.is_some(),
            feedback.interaction_view.is_some(),
        ];
        let released = if categories.into_iter().any(|transfer| transfer) {
            release_other_pins(&mut self.children, u, &[], feedback.view, categories)?
        } else {
            false
        };
        let (changed, edge) = feedback_walk(&mut self.children, u, &[], &feedback, &by_view)?
            .unwrap_or((false, None));
        let (mut live, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut u.ops)
            .into_iter()
            .partition(|op| !matches!(op, Op::DestroyView { .. }));
        live.extend(gone);
        u.ops = live;
        self.last_work = u.work;
        Ok((changed || released, edge))
    }
    pub(crate) fn has_collection(&self, view: ViewId) -> bool {
        find_collection(&self.children, view).is_some()
    }
    /// Publish one fresh set of host measurement identities after a deferred
    /// edge's state change settles. No keys, data or row bodies are evaluated.
    /// Reuses the hosts' bounded feedback scheduling, even for unchanged rows.
    pub(crate) fn wake_collection_edge(&mut self, view: ViewId) -> Result<(), InstanceError> {
        if let Some(collection) = find_collection_mut(&mut self.children, view) {
            for row in &mut collection.mounted {
                row.epoch = advance(&mut collection.next_epoch)?;
            }
            advance(&mut collection.revision)?;
        }
        Ok(())
    }
    /// A refused action did not consume its edge. Retry only on later accepted
    /// host feedback, never by redispatching inside the current call.
    pub(crate) fn rearm_collection_edge(&mut self, view: ViewId, event: EventKind) {
        if let Some(collection) = find_collection_mut(&mut self.children, view) {
            let index = match event {
                EventKind::Reachstart => 0,
                EventKind::Reachend => 1,
                _ => unreachable!("collection edge"),
            };
            collection.edge_armed[index] = true;
        }
    }
    /// Consume the second candidate after the runner established a pure no-op.
    pub(crate) fn take_collection_end(&mut self, view: ViewId) -> bool {
        let Some(collection) = find_collection_mut(&mut self.children, view) else {
            return false;
        };
        if !collection.edge_armed[1] {
            return false;
        }
        collection.edge_armed[1] = false;
        true
    }
}
fn feedback_walk(
    children: &mut [Child],
    u: &mut Update<'_>,
    frames: &[Frame],
    feedback: &CollectionFeedback,
    by_view: &BTreeMap<ViewId, usize>,
) -> Result<Option<(bool, Option<CollectionEdges>)>, InstanceError> {
    for child in children {
        let found = match child {
            Child::Node(node) => {
                if let Some(collection) = &mut node.collection {
                    if collection.view == feedback.view {
                        return collection
                            .feedback(u, frames, feedback.clone(), by_view)
                            .map(Some);
                    }
                    for row in &mut collection.mounted {
                        let mut inner = frames.to_vec();
                        inner.push(row.row.frame.clone());
                        if let Some(result) =
                            feedback_walk(&mut row.row.roots, u, &inner, feedback, by_view)?
                        {
                            return Ok(Some(result));
                        }
                    }
                }
                feedback_walk(&mut node.children, u, frames, feedback, by_view)?
            }
            Child::Region(region) => match &mut region.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.clone());
                    feedback_walk(roots, u, &inner, feedback, by_view)?
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        if let Some(result) =
                            feedback_walk(&mut row.roots, u, &inner, feedback, by_view)?
                        {
                            return Ok(Some(result));
                        }
                    }
                    None
                }
            },
        };
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
}

fn find_collection(children: &[Child], view: ViewId) -> Option<&Collection> {
    let mut stack: Vec<_> = children.iter().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Node(node) => {
                if let Some(collection) = &node.collection {
                    if collection.view == view {
                        return Some(collection);
                    }
                    collection.add_children(&mut stack);
                }
                stack.extend(node.children.iter());
            }
            Child::Region(region) => match &region.active {
                Active::Arm { roots, .. } => stack.extend(roots.iter()),
                Active::Rows { rows } => {
                    for row in rows {
                        stack.extend(row.roots.iter());
                    }
                }
            },
        }
    }
    None
}

fn find_collection_mut(children: &mut [Child], view: ViewId) -> Option<&mut Collection> {
    let mut stack: Vec<_> = children.iter_mut().collect();
    while let Some(child) = stack.pop() {
        match child {
            Child::Node(node) => {
                if let Some(collection) = &mut node.collection {
                    if collection.view == view {
                        return Some(collection);
                    }
                    // Virtual row descendants cannot contain another collection.
                }
                stack.extend(node.children.iter_mut());
            }
            Child::Region(region) => match &mut region.active {
                Active::Arm { roots, .. } => stack.extend(roots.iter_mut()),
                Active::Rows { rows } => {
                    for row in rows {
                        stack.extend(row.roots.iter_mut());
                    }
                }
            },
        }
    }
    None
}

fn release_other_pins(
    children: &mut [Child],
    u: &mut Update<'_>,
    frames: &[Frame],
    target: ViewId,
    categories: [bool; 2],
) -> Result<bool, InstanceError> {
    let mut changed = false;
    for child in children {
        match child {
            Child::Node(node) => {
                if let Some(collection) = &mut node.collection {
                    if collection.view != target {
                        changed |= collection.release_pins(u, frames, categories)?;
                    }
                    // Nested virtual collections are rejected before realization;
                    // releasing a sibling can never retire the addressed owner.
                }
                changed |= release_other_pins(&mut node.children, u, frames, target, categories)?;
            }
            Child::Region(region) => match &mut region.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.clone());
                    changed |= release_other_pins(roots, u, &inner, target, categories)?;
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        changed |=
                            release_other_pins(&mut row.roots, u, &inner, target, categories)?;
                    }
                }
            },
        }
    }
    Ok(changed)
}

/// Check the template, including inactive arms and rows not yet materialized.
/// An explicit false remains an ordinary eager list; a dynamic nested opt-in
/// could become enabled later and is rejected just like an explicit true.
pub(super) fn validate_no_nested(plan: &Plan, region: RegionsId) -> Result<(), InstanceError> {
    let mut stack = sites(plan, None, plan.region(region).arms.iter().next());
    while let Some((_, site)) = stack.pop() {
        match site {
            Site::Node(node) => {
                let row = plan.node(node);
                for binding in row.bindings.iter().map(|id| plan.binding(id)) {
                    if binding.kind == BindingKind::Prop
                        && binding.id == PropId::Virtualized as u16
                        && plan.code(binding.expr)
                            != [
                                exact_plan::Opcode::Bool as u8,
                                0,
                                exact_plan::Opcode::Return as u8,
                            ]
                    {
                        return Err(invalid("nested virtualized collections are not supported"));
                    }
                }
                stack.extend(sites(plan, Some(node), row.arm));
            }
            Site::Region(region) => {
                for arm in plan.region(region).arms.iter() {
                    stack.extend(sites(plan, None, Some(arm)));
                }
            }
        }
    }
    Ok(())
}

/// Inherited typography may change without changing a row-body expression.
pub(in crate::instance) fn invalidate_typography(
    children: &mut [Child],
    u: &mut Update<'_>,
    frames: &[Frame],
) -> Result<(), InstanceError> {
    for child in children {
        match child {
            Child::Node(node) => {
                if let Some(collection) = &mut node.collection {
                    collection.invalidate_measurements(u, frames)?;
                    for row in &mut collection.mounted {
                        let mut inner = frames.to_vec();
                        inner.push(row.row.frame.clone());
                        invalidate_typography(&mut row.row.roots, u, &inner)?;
                    }
                }
                invalidate_typography(&mut node.children, u, frames)?;
            }
            Child::Region(region) => match &mut region.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.clone());
                    invalidate_typography(roots, u, &inner)?;
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        invalidate_typography(&mut row.roots, u, &inner)?;
                    }
                }
            },
        }
    }
    Ok(())
}

impl Tree {
    pub(crate) fn reorder_collection(&self, view: ViewId) -> Option<&Collection> {
        find_collection(&self.children, view)
    }
    pub(crate) fn edit_reorder<T>(
        &mut self,
        view: ViewId,
        u: &mut Update<'_>,
        mut edit: impl FnMut(&mut Collection, &mut Update<'_>, &[Frame]) -> Result<T, InstanceError>,
    ) -> Result<Option<T>, InstanceError> {
        edit_walk(&mut self.children, view, u, &[], &mut edit)
    }
}
fn edit_walk<T>(
    children: &mut [Child],
    view: ViewId,
    u: &mut Update<'_>,
    frames: &[Frame],
    edit: &mut impl FnMut(&mut Collection, &mut Update<'_>, &[Frame]) -> Result<T, InstanceError>,
) -> Result<Option<T>, InstanceError> {
    for child in children {
        let found = match child {
            Child::Node(n) => {
                if let Some(c) = &mut n.collection {
                    if c.view == view {
                        return edit(c, u, frames).map(Some);
                    }
                    // Enabled nested virtual collections are forbidden. Ordinary
                    // row descendants cannot own another private collection.
                }
                edit_walk(&mut n.children, view, u, frames, edit)?
            }
            Child::Region(r) => match &mut r.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.clone());
                    edit_walk(roots, view, u, &inner, edit)?
                }
                Active::Rows { rows } => {
                    let mut found = None;
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        found = edit_walk(&mut row.roots, view, u, &inner, edit)?;
                        if found.is_some() {
                            break;
                        }
                    }
                    found
                }
            },
        };
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
}

fn push_children_bounded<'a>(
    stack: &mut Vec<&'a Child>,
    children: &'a [Child],
    limit: usize,
) -> Result<(), &'static str> {
    if children.len() > limit.saturating_sub(stack.len()) {
        return Err("collection stack capacity");
    }
    stack.extend(children.iter().rev());
    Ok(())
}
fn push_active_bounded<'a>(
    stack: &mut Vec<&'a Child>,
    active: &'a Active,
    limit: usize,
) -> Result<(), &'static str> {
    match active {
        Active::Arm { roots, .. } => push_children_bounded(stack, roots, limit),
        Active::Rows { rows } => {
            for row in rows.iter().rev() {
                push_children_bounded(stack, &row.roots, limit)?;
            }
            Ok(())
        }
    }
}
fn first_root_bounded<'a>(
    children: &'a [Child],
    stack: &mut Vec<&'a Child>,
    limit: usize,
) -> Result<ViewId, &'static str> {
    stack.clear();
    push_children_bounded(stack, children, limit)?;
    let mut visited = 0usize;
    while let Some(child) = stack.pop() {
        visited = visited.checked_add(1).ok_or("collection root overflow")?;
        if visited > limit {
            return Err("collection root capacity");
        }
        match child {
            Child::Node(n) => return Ok(n.view),
            Child::Region(r) => push_active_bounded(stack, &r.active, limit)?,
        }
    }
    Err("collection row has no authored root")
}
