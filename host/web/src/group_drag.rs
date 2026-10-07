//! A grouped list's reorder on the web (LLP 1094 D5–D9): the same v3
//! packets, no holds. The page draws the ghost (`group-glue.js`); the
//! runner hides the row, moves the gap in whichever grouped list the ghost's
//! centre is over (op 21, `reorder-preview-into`, its one record naming the
//! target list), steps it by key (op 22), drops on the target (op 17, the
//! same record) and may hold until the move shows. Every reply and each
//! batch's `reorder-state` carry the phase, the ending, the target and the
//! wrapper that holds the row now, where the ghost lands.
use super::*;
use exact_runner::ReorderStep;

/// `{"op":"reorder-group",…}` for a grip: its list's `reorderGroup` (null
/// for none) and whether the host's keys may drive it (D9: no `press`,
/// `key`, `pan` or `pointerdown` of its own).
pub(super) fn group_binding<D: DataSource>(
    runner: &exact_runner::Runner<D>,
    view: ViewId,
    handle: NodeKey,
    binding: Option<ReorderBinding>,
) -> String {
    use exact_kernel::PropId;
    use exact_plan::EventKind;
    let group = binding
        .and_then(|b| runner.kernel().node_by_key(b.list))
        .and_then(|n| {
            n.props
                .str(PropId::ReorderGroup)
                .filter(|g| !g.is_empty())
                .map(str::to_owned)
        });
    let keys = runner.kernel().node_by_key(handle).is_some_and(|n| {
        !runner.handlers_of(n.id).iter().any(|k| {
            matches!(
                k,
                EventKind::Press | EventKind::Key | EventKind::Pan | EventKind::Pointerdown
            )
        })
    });
    let mut out = format!("{{\"op\":\"reorder-group\",\"id\":{view},\"group\":");
    match group {
        Some(g) => crate::batch::quote(&g, &mut out),
        None => out.push_str("null"),
    }
    out.push_str(&format!(",\"keys\":{keys}}}"));
    out
}

impl<D: DataSource> Host<D> {
    pub(super) fn group_input(&mut self, i: &Input) -> Result<String, &'static str> {
        if i.op == 15 {
            if i.token != 0
                || self.reorder_drags.active.is_some()
                || self.runner.reorder_binding(i.binding.handle) != Some(i.binding)
                || self.runner.reorder_geometry(i.binding.list).as_ref() != Some(&i.geometry)
            {
                return Ok(stale());
            }
        } else {
            let Some(a) = self.reorder_drags.active.as_ref().filter(|a| a.grouped) else {
                return Ok(stale());
            };
            if a.binding != i.binding || a.token.serial() != i.token {
                return Ok(stale());
            }
        }
        i.validate(self.springs.now())?;
        self.now_ms = i.now;
        let token = self.reorder_drags.active.as_ref().map(|a| a.token);
        // The record names the target list by its view id (the page holds
        // no other list's key); its geometry is the header's.
        let kernel = self.runner.kernel();
        let target = |i: &Input| {
            let list = kernel.node(i.rows.first()?.key.index)?.key;
            Some(ReorderGeometry {
                list,
                ..i.geometry.clone()
            })
        };
        let failed = |e: exact_runner::RunnerError| {
            let _ = e;
            "reorder refused"
        };
        let (certified, dispatched, receipts) = match i.op {
            15 => {
                let Some(start) = self
                    .runner
                    .begin_group_reorder(i.binding, i.geometry.clone(), i.flags == 1)
                    .map_err(failed)?
                else {
                    return Ok(stale());
                };
                self.reorder_drags.active = Some(Active {
                    binding: i.binding,
                    token: start.token,
                    grouped: true,
                    holds: BTreeMap::new(),
                    terminal: false,
                    captured: false,
                    released: false,
                    velocity: Value::ZERO,
                });
                (true, false, start.receipt.into_iter().collect())
            }
            16 | 21 => {
                let g = if i.op == 21 {
                    target(i).ok_or("reorder-preview-into names its target list")?
                } else {
                    i.geometry.clone()
                };
                let progress = self
                    .runner
                    .preview_reorder_into(token.unwrap(), g.list, g, i.y)
                    .map_err(failed)?;
                match progress {
                    ReorderProgress::Accepted { receipt } => {
                        (true, false, receipt.into_iter().collect())
                    }
                    ReorderProgress::NeedsMeasurement => (false, false, Vec::new()),
                    ReorderProgress::Stale => return Ok(stale()),
                }
            }
            22 => {
                let step = [
                    ReorderStep::Earlier,
                    ReorderStep::Later,
                    ReorderStep::PreviousList,
                    ReorderStep::NextList,
                ][i.flags as usize - 1];
                let receipts = self
                    .runner
                    .reorder_step(token.unwrap(), step)
                    .map_err(failed)?;
                (true, false, receipts)
            }
            17 => {
                let g = target(i).unwrap_or_else(|| i.geometry.clone());
                match self
                    .runner
                    .drop_reorder(token.unwrap(), g)
                    .map_err(failed)?
                {
                    Some(receipt) => (true, true, vec![receipt]),
                    None => (
                        false,
                        false,
                        self.runner
                            .cancel_reorder(token.unwrap())
                            .map_err(failed)?
                            .into_iter()
                            .collect(),
                    ),
                }
            }
            18 => (
                false,
                false,
                self.runner
                    .cancel_reorder(token.unwrap())
                    .map_err(failed)?
                    .into_iter()
                    .collect(),
            ),
            20 => {
                let receipt = self.runner.finish_reorder(token.unwrap()).map_err(failed)?;
                if receipt.is_none() {
                    return Ok(stale());
                }
                self.reorder_drags.active = None;
                let batch = self.reorder_batch(receipt, i.now);
                return Ok(format!("{{\"accepted\":true,\"batch\":{batch}}}"));
            }
            _ => return Ok(stale()),
        };
        let timed: Vec<_> = receipts
            .into_iter()
            .map(|receipt| Timed {
                at_ms: i.now,
                receipt,
            })
            .collect();
        let batch = if timed.is_empty() {
            self.hold_batch(i.now)
        } else {
            self.batch_for(&timed, None)
        };
        Ok(format!(
            "{{\"accepted\":true,\"certified\":{certified},\"dispatched\":{dispatched},{},\"batch\":{batch}}}",
            self.group_fields()
        ))
    }

    /// `"runtime":…,"token":…,"phase":…,"ending":…,"target":…,"row":…`:
    /// the session as the page reads it, by view ids.
    fn group_fields(&self) -> String {
        let Some(a) = self.reorder_drags.active.as_ref() else {
            return "\"phase\":\"finished\"".into();
        };
        let frame = self.runner.reorder_frame(a.token).unwrap_or_default();
        let view = |key: Option<NodeKey>| {
            key.and_then(|k| self.runner.kernel().node_by_key(k))
                .map_or("null".into(), |n| n.id.to_string())
        };
        let phase = if self.runner.reorder_frame(a.token).is_none() {
            "finished"
        } else {
            frame.phase.name()
        };
        format!(
            "\"runtime\":\"{}\",\"token\":\"{}\",\"phase\":\"{phase}\",\"ending\":{},\"target\":{},\"row\":{}",
            self.reorder_drags.runtime,
            a.token.serial(),
            frame
                .ending
                .map_or("null".into(), |e| format!("\"{}\"", e.name())),
            view(frame.target),
            view(frame.row),
        )
    }

    /// The batch's `reorder-state` for a grouped session.
    pub(super) fn group_state(&self) -> String {
        format!(
            "{{\"op\":\"reorder-state\",\"grouped\":true,{}}}",
            self.group_fields()
        )
    }
}
