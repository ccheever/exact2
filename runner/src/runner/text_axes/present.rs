//! What the font-axis engine presents: keyframes that move a font row,
//! each frame's values, and a row's return to rest.
use super::*;
use exact_motion::{Animation, Animations, Keyframes};

impl<D: DataSource> Runner<D> {
    /// Give the engine a view's animations of font rows, and the values
    /// they play over (CSS Animations 1 §3: the underlying value fills a
    /// keyframe the rule leaves out). An axis the view's own settings do
    /// not name plays over the value its first keyframe gives it.
    pub(super) fn animate_text_axes(
        &mut self,
        view: ViewId,
        make: fn() -> Engine,
        dirty: &mut BTreeSet<(ViewId, Row)>,
    ) {
        let Some(node) = self.kernel.node(view) else {
            return;
        };
        // CSS runs no animation under `display: none` (CSS Animations 1 §3).
        let row = if self.hidden(view) {
            Animations::default()
        } else {
            self.kernel.animation_row(&node)
        };
        let font =
            |a: &Animation| (a.keyframes.properties().into_iter()).any(|p| Row::of(p).is_some());
        let own = Animations(row.0.into_iter().filter(font).collect());
        let properties: Vec<Property> = own.properties();
        let mut tags: Vec<u32> = Vec::new();
        for a in &own.0 {
            for frame in a.keyframes.0.iter() {
                for tag in frame.axis_tags() {
                    if !tags.contains(&tag) {
                        tags.push(tag);
                    }
                }
            }
        }
        let targets: Vec<(Row, Shown)> = ROWS
            .into_iter()
            .map(|row| (row, self.target_of(view, row, false)))
            .collect();
        let inherits: Vec<bool> = ROWS
            .into_iter()
            .map(|row| {
                let ours = (self.text_axes.moving.get(&(view, row))).is_some_and(|m| m.inherited);
                !node.style.mask.has(row.id()) || ours
            })
            .collect();
        let old = self.text_axes.keyframed.remove(&view).unwrap_or_default();
        let engine = self
            .text_axes
            .engine
            .get_or_insert_with(|| Box::new(make()));
        let _ = engine.advance((self.now_ms / 1000.0).max(engine.now()));
        let _ = engine.set_animations(key(view), &own);
        for (i, (row, target)) in targets.iter().enumerate() {
            let moved = match (row, target) {
                (Row::Axes, Shown::Axes(list)) => {
                    for tag in old.iter().filter(|t| !tags.contains(t)) {
                        let _ = engine.set_animations(axis_key(view, *tag), &Animations::NONE);
                    }
                    for &tag in &tags {
                        let node = axis_key(view, tag);
                        let _ = engine.set_animations(node, &one_axis(&own, tag));
                        if engine.value(node, row.property()).is_none() {
                            let first = || {
                                (own.0.iter().flat_map(|a| a.keyframes.0.iter()))
                                    .flat_map(|f| f.values.iter())
                                    .find(|(p, v)| {
                                        *p == Property::FontVariationSettings && v.axis_tag() == tag
                                    })
                                    .map_or(0.0, |(_, v)| v.x)
                            };
                            let at = list.get(&tag.to_be_bytes()).map_or_else(first, f64::from);
                            let _ = engine.observe_settled(Change {
                                node,
                                property: row.property(),
                                value: Value::axis(tag, at),
                                velocity: None,
                            });
                        }
                    }
                    !tags.is_empty() || !old.is_empty()
                }
                (_, Shown::Number(n)) => {
                    let p = row.property();
                    let had = engine.value(key(view), p).is_some();
                    if properties.contains(&p) && !had {
                        let _ = engine.observe_settled(Change {
                            node: key(view),
                            property: p,
                            value: Value::scalar(*n),
                            velocity: None,
                        });
                    }
                    properties.contains(&p) || had
                }
                _ => false,
            };
            if moved {
                self.text_axes.moving.entry((view, *row)).or_insert(Moving {
                    target: target.clone(),
                    inherited: inherits[i],
                });
                dirty.insert((view, *row));
            }
        }
        if !tags.is_empty() {
            self.text_axes.keyframed.insert(view, tags);
        }
    }

    /// Whether the view or an ancestor is `display: none`.
    fn hidden(&self, view: ViewId) -> bool {
        let mut cur = self.kernel.node(view);
        while let Some(node) = cur {
            if node.style.display == exact_kernel::Display::None {
                return true;
            }
            cur = node.parent.and_then(|p| self.kernel.node(p));
        }
        false
    }

    /// The value presented now for a moving row, and whether it is at rest:
    /// no transition runs and no animation will play on it.
    fn shown_text_axis(&self, view: ViewId, row: Row) -> Option<(Shown, bool)> {
        let engine = self.text_axes.engine.as_ref()?;
        let moving = self.text_axes.moving.get(&(view, row))?;
        let p = row.property();
        let now = engine.now();
        let busy = |node: u64, underlying: Value| {
            engine.is_active(node, p)
                || engine.animated(node, p, underlying).is_some()
                || engine.animation_plays(node).iter().any(|play| {
                    play.animation.keyframes.properties().contains(&p)
                        && (play.hold.is_some()
                            || play.pending.is_some()
                            || play.local(now) <= play.animation.end_time())
                })
        };
        match &moving.target {
            Shown::Number(target) => {
                let node = key(view);
                let value = engine.sampled_value(node, p).map_or(*target, |v| v.x);
                Some((Shown::Number(value), !busy(node, Value::scalar(value))))
            }
            Shown::Axes(list) => {
                let mut axes: Vec<(u32, f32)> = Vec::new();
                let mut rest = true;
                for (tag, value) in axis_values(list) {
                    let node = axis_key(view, tag);
                    let shown = engine.sampled_value(node, p).unwrap_or(value);
                    rest &= !busy(node, shown);
                    axes.push((tag, shown.x as f32));
                }
                let keyframed = self.text_axes.keyframed.get(&view);
                for &tag in keyframed.into_iter().flatten() {
                    if list.get(&tag.to_be_bytes()).is_some() {
                        continue;
                    }
                    let node = axis_key(view, tag);
                    let Some(under) = engine.value(node, p) else {
                        continue;
                    };
                    rest &= !busy(node, under);
                    if let Some(v) = engine.animated(node, p, under) {
                        axes.push((tag, v.x as f32));
                    }
                }
                Some((Shown::Axes(FontVariations::from_axes(&axes)), rest))
            }
        }
    }

    /// The ops that present `rows` now; a row at rest shows its target, or
    /// inherits again, and the engine forgets it.
    pub(super) fn present_text_axes(&mut self, rows: BTreeSet<(ViewId, Row)>) -> Vec<Op> {
        let mut ops: Vec<Op> = Vec::new();
        for (view, row) in rows {
            let Some(node) = self.kernel.node(view) else {
                continue;
            };
            let Some((shown, rest)) = self.shown_text_axis(view, row) else {
                continue;
            };
            let holds = node.style.mask.has(row.id()).then(|| read(node.style, row));
            if rest {
                let moving = self.text_axes.moving.remove(&(view, row)).expect("moving");
                self.rest_text_axis(view, row, &moving.target);
                if moving.inherited {
                    if holds.is_some() {
                        ops.push(Op::ClearStyle {
                            id: view,
                            mask: mask_of(row),
                            written: false,
                        });
                    }
                    continue;
                }
            }
            if holds.as_ref() == Some(&shown) {
                continue;
            }
            let at =
                (ops.iter()).position(|op| matches!(op, Op::SetStyle { id, .. } if *id == view));
            let at = at.unwrap_or_else(|| {
                ops.push(Op::SetStyle {
                    id: view,
                    patch: Box::default(),
                });
                ops.len() - 1
            });
            if let Op::SetStyle { patch, .. } = &mut ops[at] {
                write(patch, row, &shown);
            }
        }
        ops
    }

    /// Forget a row's engine values once it rests.
    fn rest_text_axis(&mut self, view: ViewId, row: Row, target: &Shown) {
        let Some(engine) = self.text_axes.engine.as_mut() else {
            return;
        };
        let p = row.property();
        match target {
            Shown::Number(_) => {
                engine.remove_property(key(view), p);
            }
            Shown::Axes(list) => {
                let keyframed = self.text_axes.keyframed.get(&view);
                for tag in list
                    .axes()
                    .iter()
                    .map(|(t, _)| t)
                    .chain(keyframed.into_iter().flatten())
                {
                    engine.remove_property(axis_key(view, *tag), p);
                }
            }
        }
    }

    /// Each moving font row at the clock, one commit for all of them.
    pub(in crate::runner) fn tick_text_axes(
        &mut self,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(engine) = self.text_axes.engine.as_mut() else {
            return Ok(None);
        };
        let now = (self.now_ms / 1000.0).max(engine.now());
        let _ = engine.advance(now);
        let mut rows = BTreeSet::new();
        for shown in engine.frame() {
            let view = shown.node as u32;
            let row = match (shown.node >> 32, Row::of(shown.property)) {
                (0, Some(row)) => row,
                (_, Some(Row::Axes)) => Row::Axes,
                _ => continue,
            };
            if self.text_axes.moving.contains_key(&(view, row)) {
                rows.insert((view, row));
            }
        }
        let ops = self.present_text_axes(rows);
        if ops.is_empty() {
            return Ok(None);
        }
        self.batch += 1;
        Ok(Some(self.kernel.apply(0, self.batch, &ops)?))
    }
}

/// The animations of one variation axis: each entry that moves it, its
/// keyframes cut to that axis's value.
fn one_axis(animations: &Animations, tag: u32) -> Animations {
    let entries = animations.0.iter().filter_map(|a| {
        let frames: Vec<_> = (a.keyframes.0.iter())
            .filter_map(|f| {
                let value = f
                    .values
                    .iter()
                    .find(|(p, v)| *p == Property::FontVariationSettings && v.axis_tag() == tag)?;
                let mut frame = f.clone();
                frame.values = vec![*value];
                frame.dark.clear();
                Some(frame)
            })
            .collect();
        (!frames.is_empty()).then(|| Animation {
            keyframes: Keyframes(frames.into()),
            ..a.clone()
        })
    });
    Animations(entries.collect())
}
