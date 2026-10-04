use super::*;

impl RegionInst {
    pub(super) fn create(
        u: &mut Update<'_>,
        region: RegionsId,
        frames: &[Frame],
    ) -> Result<RegionInst, InstanceError> {
        let mut inst = RegionInst {
            region,
            subject: None,
            active: match u.env.plan.region(region).kind {
                RegionKind::Each => Active::Rows { rows: Vec::new() },
                _ => Active::Arm {
                    arm: None,
                    frame: Frame::default(),
                    roots: Vec::new(),
                },
            },
        };
        inst.update(u, frames, true)?;
        Ok(inst)
    }

    /// Bring the region up to date; whether its roots changed. `fresh`: its
    /// first realization, where nothing has been evaluated yet.
    pub(super) fn update(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        fresh: bool,
    ) -> Result<bool, InstanceError> {
        let plan = u.env.plan;
        let deps = &u.sites.deps;
        let index = self.region.0 as usize;
        let row = plan.region(self.region);
        let subject_stale = fresh || u.stale(&deps.subjects[index]);
        match (&row.kind, &mut self.active) {
            (RegionKind::When, Active::Arm { arm, frame, roots }) => {
                let want = if !subject_stale {
                    *arm
                } else {
                    match u.eval(row.subject, frames)? {
                        Value::Bool(true) => Some(0),
                        Value::Bool(false) if row.arms.len > 1 => Some(1),
                        Value::Bool(false) => None,
                        _ => {
                            return Err(InstanceError::SubjectKind {
                                region: self.region,
                            })
                        }
                    }
                };
                Self::switch(
                    u,
                    self.region,
                    arm,
                    frame,
                    roots,
                    want,
                    Frame::default(),
                    frames,
                )
            }
            (RegionKind::Match, Active::Arm { arm, frame, roots }) => {
                let (want, new_frame) = if !subject_stale {
                    (*arm, frame.clone())
                } else {
                    match u.eval(row.subject, frames)? {
                        Value::Option(Some(v)) => (
                            Some(0),
                            Frame {
                                item: None,
                                bound: Some((*v).clone()),
                                ..Default::default()
                            },
                        ),
                        Value::Option(None) => (
                            if row.arms.len > 1 { Some(1) } else { None },
                            Frame::default(),
                        ),
                        _ => {
                            return Err(InstanceError::SubjectKind {
                                region: self.region,
                            })
                        }
                    }
                };
                Self::switch(u, self.region, arm, frame, roots, want, new_frame, frames)
            }
            (RegionKind::Each, Active::Rows { rows }) => {
                let body = &deps.bodies[index];
                let keys_stale = fresh || u.stale_outside(&deps.keys[index], 1);
                let subject = if subject_stale || keys_stale {
                    Some(u.eval(row.subject, frames)?)
                } else {
                    None
                };
                // The same list object keyed by unchanged inputs is the same keys.
                let rekey = match (&subject, &self.subject) {
                    (None, _) => false,
                    (Some(new), Some(old)) => keys_stale || !crate::compare::same(new, old),
                    (Some(_), None) => true,
                };
                if !rekey {
                    let mut roots = false;
                    for r in rows.iter_mut() {
                        roots |= update_row(u, r, frames, 0, body)?;
                    }
                    return Ok(roots);
                }
                let subject = subject.expect("rekeyed");
                let Value::List(items) = &subject else {
                    return Err(InstanceError::SubjectKind {
                        region: self.region,
                    });
                };
                let arm = row.arms.iter().next();
                // Key every item. A repeated key is the data's error, not the
                // plan's: its later rows get their own identity, in order.
                let mut keyed: Vec<(String, Value, u32, Frame)> = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    u.work.rows_keyed += 1;
                    let frame = Frame {
                        item: Some(item.clone()),
                        index: Some(index),
                        bound: None,
                        ..Default::default()
                    };
                    let inner = with_frame(frames, frame.clone());
                    let key = u.eval(row.key, &inner)?;
                    let key_text = key_text(&key).ok_or(InstanceError::KeyKind {
                        region: self.region,
                    })?;
                    keyed.push((key_text, key, 0, frame));
                }
                // A key's rows after its first are numbered in item order.
                let order = stable_order(keyed.len(), &|i, j| keyed[i].0 < keyed[j].0);
                for pair in order.windows(2) {
                    if keyed[pair[0]].0 == keyed[pair[1]].0 {
                        keyed[pair[1]].2 = keyed[pair[0]].2 + 1;
                    }
                }
                for k in &mut keyed {
                    if k.2 > 0 {
                        k.0 = disambiguate(std::mem::take(&mut k.0), k.2);
                    }
                }
                // Reuse rows by key, create the new, destroy the gone; order follows the items.
                // A linear search per row made an unchanged 10,000-row list
                // quadratic. Reuse the same canonical keys as duplicate checking.
                // Keep old order for destruction, which is observable in receipts.
                let mut old: Vec<Option<Row>> =
                    std::mem::take(rows).into_iter().map(Some).collect();
                // Old rows by identity, sorted; of rows with one identity the
                // last is found, as a map built from them would keep it.
                let idents: Vec<String> = old
                    .iter()
                    .map(|r| {
                        let r = r.as_ref().unwrap();
                        ident(&r.key, r.dup).expect("validated key")
                    })
                    .collect();
                let mut by_key = stable_order(idents.len(), &|i, j| idents[i] < idents[j]);
                by_key.dedup_by(|later, earlier| {
                    let same = idents[*later] == idents[*earlier];
                    if same {
                        *earlier = *later;
                    }
                    same
                });
                let mut roots = keyed.len() != old.len();
                let mut next = Vec::with_capacity(keyed.len());
                for (position, (key_text, key, dup, mut frame)) in keyed.into_iter().enumerate() {
                    let found = by_key
                        .binary_search_by(|&i| idents[i].as_str().cmp(&key_text))
                        .ok()
                        .map(|at| by_key[at]);
                    let existing = found.and_then(|i| old[i].take());
                    frame.region = Some(self.region.0);
                    match existing {
                        Some(mut r) => {
                            roots |= found != Some(position);
                            // Row bodies may distinguish signed zero (`1 / n > 0`):
                            // compare by bits, not by the language's `==`. An
                            // equivalent item keeps its object for nested memos.
                            // A row that moved reads a new position (LLP 1062
                            // D8): every field counts as changed, since the
                            // index is read as the whole frame.
                            let dirty = if r.frame.index != frame.index {
                                !0
                            } else {
                                crate::compare::changed_fields(&r.frame.item, &frame.item)
                            };
                            if dirty == 0 {
                                frame.item = r.frame.item.take();
                            }
                            frame.row = Some(r.slots.clone());
                            r.frame = frame;
                            roots |= update_row(u, &mut r, frames, dirty, body)?;
                            next.push(r);
                        }
                        None => {
                            roots = true;
                            // A new row: its slots start from their initializers,
                            // evaluated here so an initializer may read the item.
                            let slots: RowSlots = Rc::new(RefCell::new(BTreeMap::new()));
                            frame.row = Some(slots.clone());
                            let inner = with_frame(frames, frame.clone());
                            if let Some(arm) = arm {
                                init_owned(u, arm, &slots, &inner)?;
                            }
                            let roots = realize(u, None, arm, &inner)?;
                            if dup > 0 {
                                u.notes.push(repeated(self.region, &key, &key_text));
                            }
                            next.push(Row {
                                key,
                                dup,
                                frame,
                                roots,
                                slots,
                            });
                        }
                    }
                }
                for gone in old.into_iter().flatten() {
                    destroy_all(u, gone.roots);
                }
                *rows = next;
                self.subject = Some(subject);
                Ok(roots)
            }
            _ => Ok(false),
        }
    }

    /// Show arm `want`; whether the roots changed.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn switch(
        u: &mut Update<'_>,
        region: RegionsId,
        arm: &mut Option<usize>,
        frame: &mut Frame,
        roots: &mut Vec<Child>,
        want: Option<usize>,
        new_frame: Frame,
        frames: &[Frame],
    ) -> Result<bool, InstanceError> {
        let plan = u.env.plan;
        if *arm == want {
            // An equivalent binding keeps its object for nested memos; the
            // arm instance, and the slots it owns, stay.
            let dirty = crate::compare::changed_fields(&frame.bound, &new_frame.bound);
            if dirty != 0 {
                frame.bound = new_frame.bound;
            }
            let saved = u.enter(dirty, frame.row.as_ref());
            let result = update_all(u, roots, &with_frame(frames, frame.clone()));
            u.leave(saved);
            return result;
        }
        // The shown arm's instance ends, and with it the slots it owned; a
        // newly shown arm's start from their initializers, evaluated in its
        // frames, after settlement (LLP 1017 P4c).
        let old = std::mem::take(roots);
        destroy_all(u, old);
        *arm = want;
        *frame = new_frame;
        if let Some(i) = want {
            let arm_id = plan.region(region).arms.iter().nth(i);
            if let Some(id) = arm_id.filter(|id| owns_slots(plan, *id)) {
                let slots: RowSlots = Rc::new(RefCell::new(BTreeMap::new()));
                frame.region = Some(region.0);
                frame.row = Some(slots.clone());
                init_owned(u, id, &slots, &with_frame(frames, frame.clone()))?;
            }
            *roots = realize(u, None, arm_id, &with_frame(frames, frame.clone()))?;
        }
        Ok(true)
    }

    pub(super) fn collect_roots(&self, out: &mut Vec<ViewId>) {
        match &self.active {
            Active::Arm { roots, .. } => push_roots(roots, out),
            Active::Rows { rows } => {
                for r in rows {
                    push_roots(&r.roots, out);
                }
            }
        }
    }

    pub(super) fn destroy(self, u: &mut Update<'_>) {
        match self.active {
            Active::Arm { roots, .. } => destroy_all(u, roots),
            Active::Rows { rows } => {
                for r in rows {
                    destroy_all(u, r.roots);
                }
            }
        }
    }
}

/// Whether arm `arm`'s instance owns any slot.
fn owns_slots(plan: &exact_plan::Plan, arm: ArmsId) -> bool {
    plan.slots.iter().any(|s| s.owner == Some(arm))
}

/// Start the slots arm `arm`'s new instance owns from their initializers,
/// in plan order, evaluated in the instance's frames (`frames`, which end
/// with its own), so one may read the row's item or the arm's binding and
/// every earlier slot.
pub(super) fn init_owned(
    u: &mut Update<'_>,
    arm: ArmsId,
    slots: &RowSlots,
    frames: &[Frame],
) -> Result<(), InstanceError> {
    let plan = u.env.plan;
    for (i, s) in plan.slots.iter().enumerate() {
        if s.owner == Some(arm) {
            let v = u.eval(s.init, frames)?;
            if !v.conforms(plan, s.ty) {
                return Err(InstanceError::SlotType {
                    slot: plan.str(s.name).to_string(),
                });
            }
            slots.borrow_mut().insert(i as u32, v);
        }
    }
    Ok(())
}
