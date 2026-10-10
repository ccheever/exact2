//! The agent's `tap` (LLP 1012 §1): a press through the path a pointer
//! takes, and a tap that names a node presses that node, never a control
//! inside it (`AgentAddressedTap.swift` on Apple, `scripts/agent-aim.mjs` on
//! the web).
use super::*;

impl<D: DataSource> Presenter<D> {
    /// The agent's `tap`: a press at the node's center through the same
    /// path a pointer takes.
    pub fn tap(&mut self, id: ViewId) -> Result<String, String> {
        self.tap_at(id, None)
    }

    /// The agent's `tap`, at `at` (a point in the node's box, from its top
    /// left: whatever a press there reaches) or plain. A plain tap presses
    /// what it names (LLP 1012 §1; `AgentAddressedTap.swift` on Apple): a
    /// node with a press of its own is pressed where a press reaches it — its
    /// center, else the point of its box nearest the center that does — or
    /// refused; one without never presses a control inside it.
    pub fn tap_at(&mut self, id: ViewId, at: Option<(f32, f32)>) -> Result<String, String> {
        self.boxes();
        if self.host.route_visibility(id).1 || self.placement_hidden(id) {
            return Err(format!("view {id} is hidden or inert"));
        }
        if crate::navigation::popover_invoker(self.host.kernel(), id) {
            return Err(crate::navigation::POPOVER_UNSUPPORTED.into());
        }
        let b = self
            .box_of(id)
            .ok_or_else(|| format!("no view {id} on screen"))?;
        let (mut x, mut y) = match at {
            Some((ax, ay)) => {
                if !(ax.is_finite()
                    && ay.is_finite()
                    && ax >= 0.
                    && ay >= 0.
                    && ax < b.rect.2
                    && ay < b.rect.3)
                {
                    return Err(format!(
                        "tap #{id} at: ({}, {}) is outside its {}x{} box",
                        num(r2(ax)),
                        num(r2(ay)),
                        num(r2(b.rect.2)),
                        num(r2(b.rect.3))
                    ));
                }
                (b.rect.0 + ax, b.rect.1 + ay)
            }
            None => crate::paint::tap_point(self.host.kernel(), &b).unwrap_or_else(|| b.center()),
        };
        if !self.lands_in(id, x, y) {
            return Err(format!(
                "view {id} is covered or not hit at its projected center"
            ));
        }
        let now = self.host.now();
        let actual = self.hit(x, y).and_then(|hit| {
            self.control_target(hit)
                .or_else(|| self.handler_target(hit, EventKind::Press))
        });
        let mut avoided = None;
        if at.is_none() {
            let middle = self.press_stop(id, x, y);
            if self.presses_itself(id) {
                if middle != Some(id) {
                    let (bx, by, bw, bh) = b.rect;
                    let (cols, rows) = (
                        ((bw / 12.).ceil() as usize).clamp(3, 32),
                        ((bh / 12.).ceil() as usize).clamp(3, 32),
                    );
                    let mut points: Vec<(f32, f32, f32, usize)> = (0..rows * cols)
                        .map(|i| {
                            let (r, c) = (i / cols, i % cols);
                            let px = bx + (c as f32 + 0.5) * bw / cols as f32;
                            let py = by + (r as f32 + 0.5) * bh / rows as f32;
                            (px, py, (px - x).hypot(py - y), i)
                        })
                        .collect();
                    points.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.3.cmp(&b.3)));
                    let found = points.into_iter().find(|&(px, py, ..)| {
                        b.contains(px, py) && self.press_stop(id, px, py) == Some(id)
                    });
                    let Some((px, py, ..)) = found else {
                        return Err(addressed_refusal(id, middle, true));
                    };
                    avoided = middle;
                    (x, y) = (px, py);
                }
            } else if let Some(inner) = middle.filter(|m| {
                // A disabled node inside it stops the press: nothing is pressed.
                *m != id
                    && self
                        .host
                        .kernel()
                        .node(*m)
                        .is_none_or(|node| node.props.bool(PropId::Disabled) != Some(true))
            }) {
                return Err(addressed_refusal(id, Some(inner), false));
            }
        }
        let actual = if avoided.is_some() {
            self.hit(x, y).and_then(|hit| {
                self.control_target(hit)
                    .or_else(|| self.handler_target(hit, EventKind::Press))
            })
        } else {
            actual
        };
        if let Some(actual) = actual.filter(|actual| {
            at.is_none()
                && *actual != id
                && !self.drawn_in(*actual, id)
                && self
                    .control_target(id)
                    .or_else(|| self.handler_target(id, EventKind::Press))
                    != Some(*actual)
        }) {
            return Err(format!(
                "view {id} activates view {actual} at its projected center"
            ));
        }
        let mark = self.hatch_pointer_mark(crate::hatches::Phase::Down, x, y);
        let activated = self.press_at(x, y, now);
        self.hatch_tapped(mark, x, y);
        if actual.is_some() && activated.is_none() {
            return Err(format!("view {id} did not accept activation"));
        }
        let tapped = activated.unwrap_or(id);
        let avoided = avoided.map_or(String::new(), |d| {
            format!(",\"avoided\":{{\"pressing\":{d},\"what\":\"#{d}\"}}")
        });
        Ok(format!(
            "{{\"tapped\":{tapped},\"at\":[{},{}]{avoided}}}",
            num(r2(x)),
            num(r2(y))
        ))
    }

    /// Whether a press at (`x`, `y`) lands on `id`: the hit, or a node it
    /// is in, is `id`.
    fn lands_in(&mut self, id: ViewId, x: f32, y: f32) -> bool {
        let mut hit = self.hit(x, y);
        while hit.is_some() && hit != Some(id) {
            hit = hit.and_then(|n| self.host.kernel().node(n).and_then(|n| n.parent));
        }
        hit == Some(id)
    }

    /// Whether `id` has a press of its own: a `press` handler, a link or a
    /// surface control's `action` — disabled or not (a disabled one presses
    /// nothing, and a tap that names it presses nothing inside it).
    fn presses_itself(&self, id: ViewId) -> bool {
        let kernel = self.host.kernel();
        kernel.node(id).is_some_and(|n| {
            n.props.str(PropId::Action).is_some() || n.props.str(PropId::Href).is_some()
        }) || self
            .host
            .runner()
            .handlers_of(id)
            .contains(&EventKind::Press)
    }

    /// The first node from what a press at (`x`, `y`) hits up to `id` that
    /// stops it — one with a press of its own, or a disabled one — or `id`
    /// when none below it does; `None` where the press does not land on `id`.
    fn press_stop(&mut self, id: ViewId, x: f32, y: f32) -> Option<ViewId> {
        if !self.lands_in(id, x, y) {
            return None;
        }
        let mut at = self.hit(x, y);
        while let Some(n) = at.filter(|n| *n != id) {
            let disabled = self
                .host
                .kernel()
                .node(n)
                .is_some_and(|node| node.props.bool(PropId::Disabled) == Some(true));
            if disabled || self.presses_itself(n) {
                return Some(n);
            }
            at = self.host.kernel().node(n).and_then(|node| node.parent);
        }
        Some(id)
    }
}

/// A plain tap's refusal (LLP 1012 §1): what it names would not be pressed,
/// `inner` would. The words the Apple hosts and the web use.
fn addressed_refusal(id: ViewId, inner: Option<ViewId>, own: bool) -> String {
    let d = inner.map_or("nothing".to_string(), |d| format!("#{d}"));
    let how = format!(
        "tap {d}, or tap #{id} at <x> <y> (a point in its box) for whatever a finger there reaches"
    );
    if own {
        format!("tap #{id} would press {d} inside it, at its middle; no point of #{id} a finger can reach presses #{id} itself; {how}")
    } else {
        format!("tap #{id} would press {d} inside it; #{id} has no press of its own, and a tap that names it never presses a control inside it; {how}")
    }
}
