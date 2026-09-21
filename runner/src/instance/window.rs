//! Fixed and measured-height lists: O(N) keys/data, O(window) instances. LLP 1010 §6.
use super::heights::Heights;
use super::*;
use crate::ListViewport;
use exact_kernel::{PropId, PropValue};

#[derive(Debug)]
pub(super) struct ListWindow {
    pub content: ViewId,
    owner: ViewId,
    height: f64,
    measured: bool,
    width: Option<f64>,
    heights: Heights,
    top: f64,
    port: f64,
    origin: f64,
    items: Rc<Vec<Value>>,
    keys: Vec<Value>,
    positions: BTreeMap<String, usize>,
    pins: Vec<String>,
    rendered: Vec<usize>,
    extent: f64,
    requested_top: Option<f64>,
    /// What the last report created, and whether its budget left work over.
    created: usize,
    pending: bool,
    /// Beside each mounted row, in the region's order: its index, and the
    /// offset and count its wrapper was last told. A report names a row by
    /// number, not by formatting its key again, and leaves alone a wrapper
    /// that would be told what it already has — a scroll reports many times a
    /// second and nearly every mounted row is where it was.
    placed: Vec<(usize, f64, usize)>,
}

fn style(u: &mut Update<'_>, view: ViewId, rows: &[(&str, Value)]) -> Result<(), InstanceError> {
    let mut patch = StyleProps::default();
    for (name, value) in rows {
        let id = exact_kernel::StyleId::from_name(name).expect("schema style");
        bridge::set_style(&mut patch, id as u16, value, u.env.plan.stacks.len())
            .map_err(InstanceError::Bridge)?;
    }
    u.ops.push(Op::SetStyle {
        id: view,
        patch: Box::new(patch),
    });
    Ok(())
}

impl NodeInst {
    pub(super) fn bound_prop(&self, plan: &Plan, prop: PropId) -> Option<&Value> {
        plan.node(self.node)
            .bindings
            .iter()
            .enumerate()
            .find_map(|(i, b)| {
                let b = plan.binding(b);
                (b.kind == BindingKind::Prop && b.id == prop as u16)
                    .then(|| self.last[i].as_ref())
                    .flatten()
            })
    }

    pub(super) fn prepare_list(
        &mut self,
        plan: &Plan,
        old_top: Option<Value>,
    ) -> Result<(), InstanceError> {
        if plan.node(self.node).node_type != NodeType::List as u8 {
            return Ok(());
        }
        let fixed = self.bound_prop(plan, PropId::ItemHeight).cloned();
        let estimated = self.bound_prop(plan, PropId::EstimatedItemHeight).cloned();
        let top = self.bound_prop(plan, PropId::ScrollTop).cloned();
        if let [Child::Region(region)] = self.children.as_mut_slice() {
            if let Some(window) = &mut region.window {
                let height = if window.measured { estimated } else { fixed };
                if height != Some(Value::Number(window.height)) {
                    return Err(InstanceError::List(
                        "row height declaration is fixed for a mounted list",
                    ));
                }
                if top != old_top {
                    if let Some(Value::Number(n)) = top {
                        window.requested_top = Some(n);
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn list_children(
        &self,
        u: &mut Update<'_>,
        frames: &[Frame],
    ) -> Result<Vec<Child>, InstanceError> {
        let fixed = self.bound_prop(u.env.plan, PropId::ItemHeight);
        let estimated = self.bound_prop(u.env.plan, PropId::EstimatedItemHeight);
        if fixed.is_some() == estimated.is_some() {
            return Err(InstanceError::List(
                "list requires exactly one row height declaration",
            ));
        }
        let measured = estimated.is_some();
        let Some(Value::Number(height)) = fixed.or(estimated) else {
            return Err(InstanceError::List("list requires a numeric row height"));
        };
        if !height.is_finite() || *height <= 0.0 || *height > f32::MAX as f64 {
            return Err(InstanceError::List(
                "item-height must be positive and finite",
            ));
        }
        let sites = u
            .sites
            .children(Some(self.node), u.env.plan.node(self.node).arm);
        let [(_, Site::Region(region))] = sites else {
            return Err(InstanceError::List("list requires one direct each"));
        };
        if u.env.plan.region(*region).kind != RegionKind::Each {
            return Err(InstanceError::List("list requires one direct each"));
        }
        let content = u.ids.fresh();
        u.ops.push(Op::CreateView {
            id: content,
            node_type: NodeType::View,
        });
        style(
            u,
            content,
            &[
                ("position_type", Value::str("relative")),
                ("width", Value::str("100%")),
                ("flex_shrink", Value::Number(0.0)),
            ],
        )?;
        let top = match self.bound_prop(u.env.plan, PropId::ScrollTop) {
            Some(Value::Number(n)) if n.is_finite() => n.max(0.0),
            _ => 0.0,
        };
        let mut region = RegionInst {
            region: *region,
            active: Active::Rows { rows: Vec::new() },
            memo: None,
            body_memo: None,
            window: Some(Box::new(ListWindow {
                content,
                owner: self.view,
                height: *height,
                measured,
                width: None,
                heights: Heights::default(),
                top,
                // Bootstrap one viewport row; the host supplies the actual
                // scrollport before paint, never a guessed screen height.
                port: *height,
                origin: 0.0,
                items: Rc::new(Vec::new()),
                keys: Vec::new(),
                positions: BTreeMap::new(),
                pins: Vec::new(),
                rendered: Vec::new(),
                extent: -1.0,
                requested_top: None,
                created: 0,
                pending: false,
                placed: Vec::new(),
            })),
        };
        region.update(u, frames)?;
        Ok(vec![Child::Region(region)])
    }
}

impl ListWindow {
    pub(super) fn index(&self, key: &str) -> Option<usize> {
        self.positions.get(key).copied()
    }
    pub(super) fn len(&self) -> usize {
        self.items.len()
    }
    pub(super) fn status(&self) -> crate::ListStatus {
        crate::ListStatus {
            top: self.top + self.origin,
            created: self.created,
            pending: self.pending,
        }
    }
    pub(super) fn row(
        &self,
        u: &mut Update<'_>,
        region: RegionsId,
        index: usize,
        frames: &[Frame],
    ) -> Result<Row, InstanceError> {
        Row::create(
            u,
            region,
            self.keys[index].clone(),
            self.items[index].clone(),
            frames,
        )
    }
    fn measure(
        &mut self,
        u: &mut Update<'_>,
        active: &Active,
        geometry: ListViewport<'_>,
    ) -> Result<(), InstanceError> {
        if !self.measured {
            return Ok(());
        }
        let anchor = self.heights.locate(self.top);
        let inset = self.top - self.heights.offset(anchor);
        let mut changed = self.width != Some(geometry.width);
        if changed {
            self.heights = Heights::new(vec![self.height; self.items.len()]);
            self.width = Some(geometry.width);
        }
        if let Active::Rows { rows } = active {
            let indices: BTreeMap<_, _> = rows
                .iter()
                .filter_map(|row| Some((row.wrapper?, *self.positions.get(&key_text(&row.key)?)?)))
                .collect();
            for (wrapper, height) in geometry.rows {
                if let Some(index) = indices.get(wrapper) {
                    changed |= self.heights.set(*index, *height);
                }
            }
        }
        if !changed {
            return Ok(());
        }
        let extent = self.heights.offset(self.items.len());
        if !extent.is_finite() || extent > f32::MAX as f64 {
            return Err(InstanceError::List(
                "list extent exceeds layout coordinates",
            ));
        }
        let top = (self.heights.offset(anchor)
            + if anchor < self.items.len() {
                inset.min(self.heights.value(anchor))
            } else {
                0.0
            })
        .clamp(0.0, (extent - self.port).max(0.0));
        if top != self.top {
            self.top = top;
            u.ops.push(Op::SetProp {
                id: self.owner,
                prop: PropId::ScrollTop,
                value: PropValue::Float(top + self.origin),
            });
        }
        if self.extent != extent {
            style(u, self.content, &[("height", Value::Number(extent))])?;
            self.extent = extent;
        }
        // Reposition surviving wrappers without evaluating their row bodies.
        self.rendered.clear();
        Ok(())
    }

    pub(super) fn replace(
        &mut self,
        u: &mut Update<'_>,
        active: &mut Active,
        region: RegionsId,
        items: Rc<Vec<Value>>,
        frames: &[Frame],
    ) -> Result<(), InstanceError> {
        let mut keys = Vec::with_capacity(items.len());
        let mut positions = BTreeMap::new();
        for (index, item) in items.iter().enumerate() {
            let mut inner = frames.to_vec();
            inner.push(Frame {
                item: Some(item.clone()),
                ..Frame::default()
            });
            let key = u.eval(u.env.plan.region(region).key, &inner)?;
            let text = key_text(&key).ok_or(InstanceError::KeyKind { region })?;
            if positions.insert(text, index).is_some() {
                return Err(InstanceError::DuplicateKey { region });
            }
            keys.push(key);
        }
        // Keep the reading key at its pixel offset. Deleted anchors fall
        // forward, then backward. This O(N) work occurs on app/data updates,
        // never on host-only scrolling.
        let anchor = self.heights.locate(self.top.max(0.0));
        let inset = self.top - self.heights.offset(anchor);
        let heights = Heights::new(
            keys.iter()
                .enumerate()
                .map(|(i, key)| {
                    self.positions
                        .get(&key_text(key).unwrap())
                        .filter(|old| self.items[**old] == items[i])
                        .map_or(self.height, |old| self.heights.value(*old))
                })
                .collect(),
        );
        let extent = heights.offset(items.len());
        if !extent.is_finite() || extent > f32::MAX as f64 {
            return Err(InstanceError::List(
                "list extent exceeds layout coordinates",
            ));
        }
        let old_top = self.top;
        if let Some(top) = self.requested_top.take() {
            self.top = (top - self.origin).max(0.0);
        } else if !self.keys.is_empty() {
            let next = (anchor.min(self.keys.len())..self.keys.len())
                .chain((0..anchor.min(self.keys.len())).rev())
                .find_map(|i| {
                    positions
                        .get(&key_text(&self.keys[i]).unwrap())
                        .map(|j| (*j, i))
                });
            self.top = next.map_or(0.0, |(j, i)| {
                heights.offset(j)
                    + if i == anchor {
                        inset.min(heights.value(j))
                    } else {
                        0.0
                    }
            });
        }
        self.top = self.top.clamp(0.0, (extent - self.port).max(0.0));
        self.heights = heights;
        self.items = items;
        self.keys = keys;
        self.positions = positions;
        self.pins.retain(|key| self.positions.contains_key(key));
        if self.top != old_top {
            u.ops.push(Op::SetProp {
                id: self.owner,
                prop: PropId::ScrollTop,
                value: PropValue::Float(self.top + self.origin),
            });
        }
        if self.extent != extent {
            style(u, self.content, &[("height", Value::Number(extent))])?;
            self.extent = extent;
        }
        // The records moved: number the mounted rows again by their keys. A
        // row whose key is gone has no number and `render` retires it.
        if let Active::Rows { rows } = &*active {
            for (row, placed) in rows.iter().zip(self.placed.iter_mut()) {
                placed.0 = key_text(&row.key)
                    .and_then(|key| self.positions.get(&key).copied())
                    .unwrap_or(usize::MAX);
            }
        }
        self.render(u, active, region, frames, true, None, false)
    }

    // Row creation and retirement have independent admission policies.
    #[allow(clippy::too_many_arguments)]
    fn render(
        &mut self,
        u: &mut Update<'_>,
        active: &mut Active,
        region: RegionsId,
        frames: &[Frame],
        refresh: bool,
        create_limit: Option<usize>,
        defer_retirement: bool,
    ) -> Result<(), InstanceError> {
        let count = self.items.len();
        let start = self.heights.locate((self.top - self.port).max(0.0));
        let bottom = (self.top + 2.0 * self.port).max(0.0);
        let last = self.heights.locate(bottom);
        let end = last + usize::from(self.heights.offset(last) < bottom);
        let mut wanted: Vec<usize> = if self.port > 0.0 {
            (start.min(count)..end.min(count)).collect()
        } else {
            Vec::new()
        };
        wanted.extend(
            self.pins
                .iter()
                .filter_map(|key| self.positions.get(key).copied()),
        );
        wanted.sort_unstable();
        wanted.dedup();
        let Active::Rows { rows } = active else {
            unreachable!()
        };
        self.created = 0;
        self.pending = false;
        debug_assert_eq!(rows.len(), self.placed.len());
        if let Some(limit) = create_limit {
            self.ration(&mut wanted, limit, defer_retirement);
        }
        // Unbudgeted hosts keep retirement with the next creation pass, except
        // when viewport or interaction changes require obsolete rows to leave.
        if create_limit.is_none()
            && !refresh
            && defer_retirement
            && !wanted.is_empty()
            && wanted
                .iter()
                .all(|index| self.rendered.binary_search(index).is_ok())
        {
            return Ok(());
        }
        // Measurement invalidates `rendered` without retiring mounted rows.
        if !refresh && wanted == self.rendered && rows.len() == wanted.len() {
            return Ok(());
        }
        let mut gone = Vec::new();
        let mut old: BTreeMap<usize, (Row, f64, usize)> = BTreeMap::new();
        for (row, (index, top, of)) in std::mem::take(rows)
            .into_iter()
            .zip(std::mem::take(&mut self.placed))
        {
            if index == usize::MAX {
                gone.push(row);
            } else {
                old.insert(index, (row, top, of));
            }
        }
        for index in &wanted {
            let key = self.keys[*index].clone();
            let mut frame = Frame {
                item: Some(self.items[*index].clone()),
                region: Some(region.0),
                ..Frame::default()
            };
            let top = self.heights.offset(*index);
            let mut row = if let Some((mut row, was, of)) = old.remove(index) {
                frame.row = Some(row.slots.clone());
                row.frame = frame.clone();
                let mut inner = frames.to_vec();
                inner.push(frame);
                if refresh {
                    update_all(u, &mut row.roots, &inner)?;
                } else if was == top && of == count {
                    // Where it was, as many as there were: nothing to say.
                    rows.push(row);
                    self.placed.push((*index, top, count));
                    continue;
                }
                row
            } else {
                self.created += 1;
                Row::create(u, region, key, self.items[*index].clone(), frames)?
            };
            let wrapper = match row.wrapper {
                Some(id) => id,
                None => {
                    let id = u.ids.fresh();
                    u.ops.push(Op::CreateView {
                        id,
                        node_type: NodeType::View,
                    });
                    u.ops.push(Op::SetChildren {
                        id,
                        children: roots_of(&row.roots),
                    });
                    u.ops.push(Op::SetProp {
                        id,
                        prop: PropId::AccessibilityRole,
                        value: PropValue::Str("listitem".into()),
                    });
                    row.wrapper = Some(id);
                    id
                }
            };
            // The wrapper supplies a fixed box or a naturally measured box. Authored descendants
            // retain their CSS, event handlers and keyed identity.
            style(
                u,
                wrapper,
                &[
                    ("position_type", Value::str("absolute")),
                    ("top", Value::Number(top)),
                    ("left", Value::Number(0.0)),
                    ("width", Value::str("100%")),
                    (
                        "height",
                        if self.measured {
                            Value::str("auto")
                        } else {
                            Value::Number(self.height)
                        },
                    ),
                    // A fixed row clips what does not fit its declared height.
                    // A measured row is as tall as its content, the list clips
                    // the inline axis, and absolute positioning already makes
                    // the wrapper a formatting context: a clip there is only a
                    // clipping group per row for the compositor to carry.
                    (
                        "overflow_x",
                        Value::str(if self.measured { "visible" } else { "hidden" }),
                    ),
                    (
                        "overflow_y",
                        Value::str(if self.measured { "visible" } else { "hidden" }),
                    ),
                ],
            )?;
            u.ops.push(Op::SetProp {
                id: wrapper,
                prop: PropId::ListItemKey,
                value: PropValue::Str(key_text(&row.key).unwrap()),
            });
            u.ops.push(Op::SetProp {
                id: wrapper,
                prop: PropId::AccessibilityPosInSet,
                value: PropValue::Int(*index as i64 + 1),
            });
            u.ops.push(Op::SetProp {
                id: wrapper,
                prop: PropId::AccessibilitySetSize,
                value: PropValue::Int(count as i64),
            });
            // A row's conditional roots may change without changing its key.
            u.ops.push(Op::SetChildren {
                id: wrapper,
                children: roots_of(&row.roots),
            });
            rows.push(row);
            self.placed.push((*index, top, count));
        }
        u.ops.push(Op::SetChildren {
            id: self.content,
            children: rows.iter().map(|r| r.wrapper.unwrap()).collect(),
        });
        for row in old.into_values().map(|(row, ..)| row).chain(gone) {
            u.ops.push(Op::DestroyView {
                id: row.wrapper.unwrap(),
            });
        }
        self.rendered = wanted;
        Ok(())
    }
}

impl ListWindow {
    /// Hold a report to its budget: every row the scrollport shows, the rows
    /// already mounted, and at most `limit` more, nearest the scrollport
    /// first. Rows that have left the window go with a report that also
    /// creates, or once nothing is left to create — never in a report of
    /// their own, which was 45% of a scroll's reports and bought nothing
    /// (LLP 1044 F7) — and at most `2 * limit` of them at a time.
    fn ration(&mut self, wanted: &mut Vec<usize>, limit: usize, defer_retirement: bool) {
        let mounted: std::collections::BTreeSet<usize> = self
            .placed
            .iter()
            .map(|placed| placed.0)
            .filter(|index| *index != usize::MAX)
            .collect();
        let count = self.items.len();
        let first = self.heights.locate(self.top.max(0.0)).min(count);
        let bottom = (self.top + self.port).max(0.0);
        let last = self.heights.locate(bottom);
        let last = (last + usize::from(self.heights.offset(last) < bottom)).min(count);
        let pinned: Vec<usize> = self
            .pins
            .iter()
            .filter_map(|key| self.positions.get(key).copied())
            .collect();
        let owed = |i: &usize| (first..last).contains(i) || pinned.contains(i);
        let mut missing: Vec<usize> = wanted
            .iter()
            .copied()
            .filter(|i| !mounted.contains(i) && !owed(i))
            .collect();
        missing.sort_by_key(|i| {
            if *i < first {
                first - *i
            } else {
                (*i + 1).saturating_sub(last)
            }
        });
        let deferred: std::collections::BTreeSet<usize> =
            missing.iter().skip(limit).copied().collect();
        wanted.retain(|i| !deferred.contains(i));
        let creating = wanted.iter().any(|i| !mounted.contains(i));
        let mut leaving: Vec<usize> = mounted
            .iter()
            .copied()
            .filter(|i| wanted.binary_search(i).is_err())
            .collect();
        // Farthest from the scrollport first; the rest stay mounted a report longer.
        leaving.sort_by_key(|i| std::cmp::Reverse(first.abs_diff(*i)));
        let retire = if !defer_retirement {
            leaving.len()
        } else if creating || deferred.is_empty() {
            (2 * limit).max(4)
        } else {
            0
        };
        let kept: Vec<usize> = leaving.iter().skip(retire).copied().collect();
        self.pending = !deferred.is_empty() || !kept.is_empty();
        wanted.extend(kept);
        wanted.sort_unstable();
    }
}

impl Row {
    fn create(
        u: &mut Update<'_>,
        region: RegionsId,
        key: Value,
        item: Value,
        frames: &[Frame],
    ) -> Result<Self, InstanceError> {
        let plan = u.env.plan;
        let slots: RowSlots = Rc::new(RefCell::new(BTreeMap::new()));
        let frame = Frame {
            item: Some(item),
            region: Some(region.0),
            row: Some(slots.clone()),
            ..Frame::default()
        };
        let mut inner = frames.to_vec();
        inner.push(frame.clone());
        for (i, slot) in plan.slots.iter().enumerate() {
            if slot.owner == Some(region) {
                let value = u.eval(slot.init, &inner)?;
                if !value.conforms(plan, slot.ty) {
                    return Err(InstanceError::SlotType {
                        slot: plan.str(slot.name).into(),
                    });
                }
                slots.borrow_mut().insert(i as u32, value);
            }
        }
        Ok(Self {
            wrapper: None,
            key,
            frame,
            roots: realize(u, None, plan.region(region).arms.iter().next(), &inner)?,
            slots,
        })
    }
}

impl Tree {
    /// Change only one list's window from host geometry, without resource
    /// settlement, key evaluation or an application action.
    pub fn update_list(
        &mut self,
        u: &mut Update<'_>,
        view: ViewId,
        geometry: ListViewport<'_>,
        create_limit: Option<usize>,
    ) -> Result<(), InstanceError> {
        fn walk(
            children: &mut [Child],
            frames: &[Frame],
            u: &mut Update<'_>,
            view: ViewId,
            geometry: ListViewport<'_>,
            create_limit: Option<usize>,
        ) -> Result<bool, InstanceError> {
            for child in children {
                match child {
                    Child::Node(node) if node.view == view => {
                        let [Child::Region(region)] = node.children.as_mut_slice() else {
                            return Err(InstanceError::List("not a windowed list"));
                        };
                        let window = region
                            .window
                            .as_mut()
                            .ok_or(InstanceError::List("not a windowed list"))?;
                        // LLP 1010 §6: a released pin or changed viewport must
                        // release rows now, even when no new row is needed.
                        let defer_retirement = geometry.height > 0.0
                            && window.port == geometry.height
                            && window.origin == geometry.origin
                            && (!window.measured || window.width == Some(geometry.width));
                        let old_pins = window.pins.clone();
                        window.top = (geometry.top - geometry.origin).max(0.0);
                        window.port = geometry.height;
                        window.origin = geometry.origin;
                        window.measure(u, &region.active, geometry)?;
                        window.pins.clear();
                        if let Active::Rows { rows } = &region.active {
                            for row in rows {
                                let contains = |pin| {
                                    pin != 0
                                        && (row.wrapper == Some(pin)
                                            || row.roots.iter().any(|c| match c {
                                                Child::Node(n) => {
                                                    n.find::<false>(pin, &mut Vec::new()).is_some()
                                                }
                                                Child::Region(r) => {
                                                    r.find::<false>(pin, &mut Vec::new()).is_some()
                                                }
                                            }))
                                };
                                if geometry.pins.iter().copied().any(contains) {
                                    window.pins.push(key_text(&row.key).unwrap());
                                }
                            }
                        }
                        let defer_retirement = defer_retirement && old_pins == window.pins;
                        window.render(
                            u,
                            &mut region.active,
                            region.region,
                            frames,
                            false,
                            create_limit,
                            defer_retirement,
                        )?;
                        return Ok(true);
                    }
                    Child::Node(n) => {
                        if walk(&mut n.children, frames, u, view, geometry, create_limit)? {
                            return Ok(true);
                        }
                    }
                    Child::Region(r) => match &mut r.active {
                        Active::Arm { roots, frame, .. } => {
                            let mut inner = frames.to_vec();
                            inner.push(frame.clone());
                            if walk(roots, &inner, u, view, geometry, create_limit)? {
                                return Ok(true);
                            }
                        }
                        Active::Rows { rows } => {
                            for row in rows {
                                let mut inner = frames.to_vec();
                                inner.push(row.frame.clone());
                                if walk(&mut row.roots, &inner, u, view, geometry, create_limit)? {
                                    return Ok(true);
                                }
                            }
                        }
                    },
                }
            }
            Ok(false)
        }
        if !walk(&mut self.children, &[], u, view, geometry, create_limit)? {
            return Err(InstanceError::List("unknown list"));
        }
        // Reconcile all final child lists before retiring old subtrees.
        let (mut live, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut u.ops)
            .into_iter()
            .partition(|op| !matches!(op, Op::DestroyView { .. }));
        live.extend(gone);
        u.ops = live;
        Ok(())
    }
}
