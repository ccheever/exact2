//! A retiring row rebound to the item the window needs (LLP 1078), as a
//! recycling list rebinds a recycled cell: its views, their kernel nodes,
//! layout and text stay, and only the bindings whose values differ for the
//! new item are written. What a rebind cannot make fresh refuses it.
//!
//! The rebound row must be indistinguishable from a fresh mount. Its
//! bindings are evaluated whole against the new item (a full update of the
//! row, so every binding equals what a create would emit), its own slots are
//! initialized anew, its wrapper takes the new key, position and measurement
//! epoch, and every view in it is named in the commit's
//! [`exact_kernel::CommitReceipt::renewed`]: motion forgets it and starts it
//! as new (animations from their start, no transition from the old item's
//! values), and a host drops what it keeps by view (a scroll offset, a
//! picture of another source, a press).
//!
//! Refused, statically by site: a node of a kind that holds its own state
//! (an editor, a control, a native, canvas, web or video view, a virtualized
//! list), a node with a row a rebind cannot restate (`autofocus`, a bound
//! scroll offset, a drag handle, an exit animation, a layout transition, a
//! timeline) and a row whose body holds slots below the row's own. A row
//! holding an inner virtualized list is refused by its instance. Only a host
//! that resets what it keeps by view opts in
//! ([`crate::Runner::set_row_reuse`]).
use super::*;
use exact_kernel::StyleId;

/// What a collection may rebind: the sites of its row body that refuse.
#[derive(Debug, Default)]
pub(super) struct Reuse {
    refused: std::collections::HashSet<NodesId>,
}

const REFUSED_TYPES: &[NodeType] = &[
    NodeType::List,
    NodeType::TextInput,
    NodeType::Control,
    NodeType::NativeView,
    NodeType::Canvas,
    NodeType::WebView,
    NodeType::Video,
    NodeType::Head,
];

const REFUSED_PROPS: &[PropId] = &[
    PropId::Autofocus,
    PropId::ScrollTop,
    PropId::ScrollLeft,
    PropId::HeightDragFor,
    PropId::TransformDragFor,
    PropId::BackgroundMaterial,
    PropId::GlassGroup,
    PropId::Popover,
    PropId::RetainFocus,
    PropId::Refreshing,
];

const REFUSED_STYLES: &[StyleId] = &[
    StyleId::ExitAnimation,
    StyleId::LayoutTransition,
    StyleId::DragTimeline,
    StyleId::AnimationTimeline,
    StyleId::AnimationRange,
    StyleId::TimelineScope,
];

/// Whether a rebind could carry state of this site's into another item's row.
fn site_refused(plan: &Plan, node: NodesId) -> bool {
    let row = plan.node(node);
    if row.surface.is_some()
        || NodeType::from_wire(row.node_type).is_none_or(|t| REFUSED_TYPES.contains(&t))
    {
        return true;
    }
    row.bindings
        .iter()
        .map(|b| plan.binding(b))
        .any(|b| match b.kind {
            BindingKind::Prop => REFUSED_PROPS.iter().any(|p| *p as u16 == b.id),
            BindingKind::Style => REFUSED_STYLES.iter().any(|s| *s as u16 == b.id),
        })
}

impl Reuse {
    /// The row body under `region`'s arm, walked once: `None` when no row of
    /// it can be rebound (slots below the row's own).
    pub(super) fn new(plan: &Plan, sites: &SiteIndex, region: RegionsId) -> Option<Rc<Self>> {
        let mut this = Self::default();
        let mut regions = Vec::new();
        let mut stack: Vec<Site> = sites
            .children(None, plan.region(region).arms.iter().next())
            .iter()
            .map(|(_, s)| *s)
            .collect();
        while let Some(site) = stack.pop() {
            match site {
                Site::Node(node) => {
                    if site_refused(plan, node) {
                        this.refused.insert(node);
                    }
                    let arm = plan.node(node).arm;
                    stack.extend(sites.children(Some(node), arm).iter().map(|(_, s)| *s));
                }
                Site::Region(inner) => {
                    regions.push(inner);
                    for arm in plan.region(inner).arms.iter() {
                        stack.extend(sites.children(None, Some(arm)).iter().map(|(_, s)| *s));
                    }
                }
            }
        }
        if plan.slots.iter().any(|s| {
            s.owner
                .is_some_and(|o| regions.contains(&plan.arm(o).region))
        }) {
            return None;
        }
        Some(Rc::new(this))
    }

    /// Whether this mounted row holds nothing a rebind refuses.
    pub(super) fn admits(&self, roots: &[Child]) -> bool {
        roots.iter().all(|c| match c {
            Child::Node(n) => {
                n.collection.is_none()
                    && !self.refused.contains(&n.node)
                    && self.admits(&n.children)
            }
            Child::Region(r) => match &r.active {
                Active::Arm { roots, .. } => self.admits(roots),
                Active::Rows { rows } => rows.iter().all(|row| self.admits(&row.roots)),
            },
        })
    }
}

/// Retiring rows a travelling list keeps mounted for the rows it needs next.
pub(super) const HOLD: usize = 4;

/// Which way an item would likely steer its row's `when` and `match` arms:
/// per field, whether an option is present, a flag set, a list empty.
fn steer(item: Option<&Value>) -> u64 {
    let Some(Value::Record(fields)) = item else {
        return 0;
    };
    let mut bits = 0u64;
    for (i, field) in fields.iter().take(64).enumerate() {
        let on = match field {
            Value::Option(o) => o.is_some(),
            Value::Bool(b) => *b,
            Value::List(l) => !l.is_empty(),
            _ => false,
        };
        bits |= u64::from(on) << i;
    }
    bits
}

/// The spare to rebind to `item`: the latest whose item steers its arms as
/// `item` does, so the fewest views are destroyed and built, else the latest.
pub(super) fn take_spare(spares: &mut Vec<Mounted>, item: &Value) -> Option<Mounted> {
    let want = steer(Some(item));
    let at = spares
        .iter()
        .rposition(|m| steer(m.row.frame.item.as_ref()) == want)
        .or(spares.len().checked_sub(1))?;
    Some(spares.remove(at))
}

/// Every view under `children`, preorder.
fn views(children: &[Child], out: &mut Vec<ViewId>) {
    for c in children {
        match c {
            Child::Node(n) => {
                out.push(n.view);
                views(&n.children, out);
            }
            Child::Region(r) => match &r.active {
                Active::Arm { roots, .. } => views(roots, out),
                Active::Rows { rows } => rows.iter().for_each(|row| views(&row.roots, out)),
            },
        }
    }
}

impl Collection {
    /// Mount the rows the window needs and nothing mounted: each rebound
    /// from a retiring row that admits it, else from a row `kept` past the
    /// window (farthest first; a limited report keeps them), else built. The
    /// kept ones left are returned.
    ///
    /// With `hold` (the port: a list that travels, its rows rebindable), the
    /// retiring rows left over stay mounted where they are, the [`HOLD`]
    /// nearest the port: rows enter a moving window at one edge and leave
    /// it at the other in different reports, so a report that only retired
    /// would destroy the row the next one builds (crypto at 6,000 px/s
    /// rebound 14 rows of 72). The next row the window needs takes the
    /// farthest of them, and a turn back finds them as they were. The rest,
    /// and every one without `hold`, are destroyed.
    ///
    /// A slice that leaves `more` rows of its window for the next holds as
    /// many of them, however far: they are those rows. Travel of more than a
    /// viewport between passes (a window that leads, a pass each sixth step
    /// at 24,000 dp/s) takes the rows behind it two viewports past the port
    /// before a pass sees them, so they all retire in the first slice, which
    /// builds only its limit: the rest were destroyed there, and the slices
    /// after it built their rows from nothing (easy at 24,000 dp/s: 135
    /// builds in 661 mounts, against 18 without a lead).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_needed(
        &mut self,
        u: &mut Update<'_>,
        needed: Vec<(usize, String)>,
        mut retiring: Vec<Mounted>,
        mut kept: Vec<(f64, String, Mounted)>,
        hold: Option<(f64, f64)>,
        more: usize,
        frames: &[Frame],
    ) -> Result<Vec<(f64, String, Mounted)>, InstanceError> {
        let mut gone = Vec::new();
        let mut spares = Vec::new();
        // Nearest the port first: a rebind takes the last.
        let far = |c: &Self, m: &Mounted| {
            hold.map_or(0.0, |(top, end)| c.distance(m.position, top, end).1)
        };
        if hold.is_some() {
            retiring.sort_by(|a, b| far(self, a).total_cmp(&far(self, b)));
        }
        for mounted in retiring {
            if (!needed.is_empty() || hold.is_some()) && self.spare(&mounted) {
                spares.push(mounted);
            } else {
                gone.push(mounted.wrapper);
            }
        }
        let reusing = self.reusing(u);
        for (position, text) in needed {
            let spare = take_spare(&mut spares, &self.items[position]).or_else(|| {
                let at = kept.iter().position(|(_, _, m)| reusing && self.spare(m))?;
                Some(kept.remove(at).2)
            });
            let mounted = match spare {
                Some(spare) => self.rebind(u, spare, position, &text, frames)?,
                None => self.build_row(u, position, &text, frames)?,
            };
            self.settle_mounted(u, mounted, &text)?;
        }
        let reach = hold.map_or(0.0, |(top, end)| FAR_VIEWPORTS * (end - top));
        let held = spares
            .iter()
            .take(HOLD)
            .take_while(|m| hold.is_some() && far(self, m) <= reach)
            .count();
        let held = if hold.is_some() {
            held.max(more.min(spares.len()))
        } else {
            held
        };
        for mut mounted in spares.drain(..held) {
            mounted.held = true;
            let text = super::super::ident(&mounted.row.key, mounted.row.dup).expect("validated");
            self.settle_mounted(u, mounted, &text)?;
        }
        gone.extend(spares.into_iter().map(|m| m.wrapper));
        for id in gone {
            u.ops.push(Op::DestroyView { id });
        }
        Ok(kept)
    }
    /// Whether retiring rows of this list may be rebound in this update.
    pub(super) fn reusing(&self, u: &Update<'_>) -> bool {
        self.reuse.is_some() && u.reuse && !u.discard
    }

    /// Whether `mounted`, retiring, may be rebound to another item.
    pub(super) fn spare(&self, mounted: &Mounted) -> bool {
        self.reuse
            .as_ref()
            .is_some_and(|r| r.admits(&mounted.row.roots))
    }

    /// Rebind a retiring row to the item at `position` (key `text`): as
    /// [`Self::create_row`] would build it, from the row it was.
    pub(super) fn rebind(
        &mut self,
        u: &mut Update<'_>,
        mut mounted: Mounted,
        position: usize,
        text: &str,
        frames: &[Frame],
    ) -> Result<Mounted, InstanceError> {
        let plan = u.env.plan;
        let token = self.index.invalidate_row(text).map_err(index_error)?;
        let slots: RowSlots = Rc::new(RefCell::new(BTreeMap::new()));
        let row = &mut mounted.row;
        row.frame = Frame {
            item: Some(self.items[position].clone()),
            index: Some(position),
            bound: None,
            region: Some(self.region.0),
            row: Some(slots.clone()),
        };
        let inner = with_frame(frames, row.frame.clone());
        let arm = plan.region(self.region).arms.iter().next();
        for (i, s) in plan.slots.iter().enumerate() {
            if s.owner.is_some() && s.owner == arm {
                let value = u.eval(s.init, &inner)?;
                if !value.conforms(plan, s.ty) {
                    return Err(InstanceError::SlotType {
                        slot: plan.str(s.name).to_string(),
                    });
                }
                slots.borrow_mut().insert(i as u32, value);
            }
        }
        row.slots = slots;
        row.key = self.keys[position].clone();
        row.dup = self.dups.get(&position).copied().unwrap_or(0);
        if row.dup > 0 {
            u.notes.push(super::super::repeated(
                self.region,
                &self.keys[position],
                text,
            ));
        }
        // Every binding of the row, evaluated against the new item: what a
        // create emits, less what the row already shows.
        let full = std::mem::replace(&mut u.full, true);
        let body = &u.sites.deps.bodies[self.region.0 as usize];
        let updated = update_row(u, row, frames, !0, body);
        u.full = full;
        updated?;
        views::rekey(u, mounted.wrapper, text);
        mounted.position = position;
        mounted.token = token;
        mounted.epoch = advance(&mut self.next_epoch)?;
        mounted.preview_target = None;
        mounted.preview_hidden = false;
        mounted.held = false;
        mounted.published = (usize::MAX, usize::MAX);
        u.renewed.push(mounted.wrapper);
        views(&mounted.row.roots, &mut u.renewed);
        self.adopt_nested(u, &mut mounted.row, text, frames)?;
        self.mounted_shown(u, &mut mounted);
        u.work.rows_rebound += 1;
        Ok(mounted)
    }
}
