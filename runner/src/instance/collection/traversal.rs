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
    /// Targeted geometry update. Never evaluates collection data or key expressions,
    /// settles resources, or traverses unmounted rows. False means stale/no change.
    pub fn update_collection(
        &mut self,
        u: &mut Update<'_>,
        feedback: CollectionFeedback,
    ) -> Result<bool, InstanceError> {
        if !self.has_collections {
            self.last_work = u.work;
            return Ok(false);
        }
        feedback
            .validate()
            .map_err(|_| invalid("invalid collection feedback"))?;
        let Some(target) = find_collection(&self.children, feedback.view) else {
            self.last_work = u.work;
            return Ok(false);
        };
        let Some(by_view) = target.prepare_feedback(&feedback)? else {
            self.last_work = u.work;
            return Ok(false);
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
        let changed = feedback_walk(&mut self.children, u, &[], &feedback, &by_view)?
            .unwrap_or(false)
            || released;
        let (mut live, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut u.ops)
            .into_iter()
            .partition(|op| !matches!(op, Op::DestroyView { .. }));
        live.extend(gone);
        u.ops = live;
        self.last_work = u.work;
        Ok(changed)
    }
}
fn feedback_walk(
    children: &mut [Child],
    u: &mut Update<'_>,
    frames: &[Frame],
    feedback: &CollectionFeedback,
    by_view: &BTreeMap<ViewId, usize>,
) -> Result<Option<bool>, InstanceError> {
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
