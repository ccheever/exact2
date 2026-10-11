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
            let middle = self.press_reach(id, x, y);
            if self.takes_press(id) {
                if !middle.is_some_and(|m| m.is_own(id)) {
                    let found = aim_points(b.rect, (x, y)).into_iter().find(|&(px, py)| {
                        b.contains(px, py)
                            && self.press_reach(id, px, py).is_some_and(|r| r.is_own(id))
                    });
                    let Some((px, py)) = found else {
                        return Err(addressed_refusal(id, middle, true));
                    };
                    avoided = middle;
                    (x, y) = (px, py);
                }
            } else if let Some(inner) = middle.filter(|m| match *m {
                // A link in it, or a node inside it that takes the press;
                // a disabled one presses nothing.
                Reach::Link(n) => n == id || self.is_inside(n, id),
                Reach::Node(n) => n != id && self.is_inside(n, id),
                _ => false,
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
        let avoided = avoided.map_or(String::new(), |d: Reach| {
            let pressing = d.node().map_or("null".into(), |n| n.to_string());
            let mut what = String::new();
            quote(&d.describe(), &mut what);
            format!(",\"avoided\":{{\"pressing\":{pressing},\"what\":{what}}}")
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

    /// Whether a press reaching `id` is taken by `id` itself, as `press_at`
    /// delivers it: its own press, a popover invoker, a form control (a
    /// checkbox, radio, switch, select, file input or range toggles, opens or
    /// moves), a text field (it takes the focus and the caret) or a canvas
    /// that wants input.
    fn takes_press(&self, id: ViewId) -> bool {
        let Some(node) = self.host.kernel().node(id) else {
            return false;
        };
        let invoker = (node.node_type == NodeType::Pressable
            || exact_kernel::ControlKind::of(node.node_type, node.props)
                == Some(exact_kernel::ControlKind::Button))
            && node
                .props
                .str(PropId::Popovertarget)
                .is_some_and(|t| !t.is_empty());
        let control = node.node_type == NodeType::Control
            && !matches!(node.props.str(PropId::Type), Some("button" | "progress"));
        invoker
            || control
            || node.node_type == NodeType::TextInput
            || self.presses_itself(id)
            || self.surfaces.wants_input(id)
    }

    /// What a press at (`x`, `y`) reaches, as `press_at` resolves it, or
    /// `None` where it does not land on `id`: a link run in the paragraph
    /// under it (a Markdown link, an inline run's `href`), else the first
    /// node from the hit one up that takes the press (`takes_press`), or a
    /// disabled one, which stops it.
    fn press_reach(&mut self, id: ViewId, x: f32, y: f32) -> Option<Reach> {
        if !self.lands_in(id, x, y) {
            return None;
        }
        let hit = self.hit(x, y)?;
        let text = self
            .host
            .kernel()
            .node(hit)
            .is_some_and(|n| n.node_type == NodeType::Text);
        if text && self.run_link(hit, x, y).is_some() {
            return Some(Reach::Link(hit));
        }
        let mut at = Some(hit);
        while let Some(n) = at {
            let node = self.host.kernel().node(n)?;
            if node.props.bool(PropId::Disabled) == Some(true) {
                return Some(Reach::Blocked(n));
            }
            if self.takes_press(n) {
                return Some(Reach::Node(n));
            }
            at = self.display.parent(self.host.kernel(), n);
        }
        Some(Reach::Nothing)
    }

    /// Strictly inside `ancestor` in the kernel's tree.
    fn is_inside(&self, node: ViewId, ancestor: ViewId) -> bool {
        let kernel = self.host.kernel();
        let mut at = kernel.node(node).and_then(|n| n.parent);
        while let Some(p) = at {
            if p == ancestor {
                return true;
            }
            at = kernel.node(p).and_then(|n| n.parent);
        }
        false
    }
}

/// What a press at a point reaches (`press_reach`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Reach {
    /// The node that takes it.
    Node(ViewId),
    /// A link run in this paragraph.
    Link(ViewId),
    /// A disabled node on the way up: nothing is pressed.
    Blocked(ViewId),
    Nothing,
}

impl Reach {
    fn node(self) -> Option<ViewId> {
        match self {
            Reach::Node(n) | Reach::Link(n) | Reach::Blocked(n) => Some(n),
            Reach::Nothing => None,
        }
    }
    /// The press is `id`'s own: `id` takes it, or `id` (disabled) stops it.
    fn is_own(self, id: ViewId) -> bool {
        matches!(self, Reach::Node(n) | Reach::Blocked(n) if n == id)
    }
    fn describe(self) -> String {
        match self {
            Reach::Node(n) => format!("#{n}"),
            Reach::Link(n) => format!("a link in #{n}"),
            Reach::Blocked(n) => format!("disabled #{n}"),
            Reach::Nothing => "nothing".into(),
        }
    }
}

/// The points of `rect` a press might land on, nearest `mid` first (ties in
/// reading order): the middles of a grid of cells of about 12 points (at
/// most 32 a side), then of about 3 points (at most 96 a side) with points
/// along its edges 1 point in, every 2 points. A strip of the box narrower
/// than the finer grid away from its edges can still be missed.
fn aim_points(rect: (f32, f32, f32, f32), mid: (f32, f32)) -> Vec<(f32, f32)> {
    let (bx, by, bw, bh) = rect;
    let grid = |cell: f32, most: usize| {
        let (cols, rows) = (
            ((bw / cell).ceil() as usize).clamp(3, most),
            ((bh / cell).ceil() as usize).clamp(3, most),
        );
        (0..rows * cols)
            .map(move |i| {
                let (r, c) = (i / cols, i % cols);
                (
                    bx + (c as f32 + 0.5) * bw / cols as f32,
                    by + (r as f32 + 0.5) * bh / rows as f32,
                )
            })
            .collect::<Vec<_>>()
    };
    let near = |mut points: Vec<(f32, f32)>| {
        points.sort_by(|a, b| {
            (a.0 - mid.0)
                .hypot(a.1 - mid.1)
                .total_cmp(&(b.0 - mid.0).hypot(b.1 - mid.1))
        });
        points
    };
    let mut fine = grid(3., 96);
    if bw > 2. && bh > 2. {
        let (xs, ys) = (((bw / 2.) as usize).min(400), ((bh / 2.) as usize).min(400));
        for i in 0..=xs {
            let x = bx + 1. + (bw - 2.) * i as f32 / xs.max(1) as f32;
            fine.extend([(x, by + 1.), (x, by + bh - 1.)]);
        }
        for i in 0..=ys {
            let y = by + 1. + (bh - 2.) * i as f32 / ys.max(1) as f32;
            fine.extend([(bx + 1., y), (bx + bw - 1., y)]);
        }
    }
    let mut points = near(grid(12., 32));
    points.extend(near(fine));
    points
}

/// A plain tap's refusal (LLP 1012 §1): what it names would not be pressed,
/// `inner` would. The words the Apple hosts and the web use.
fn addressed_refusal(id: ViewId, inner: Option<Reach>, own: bool) -> String {
    let d = inner.map_or("nothing".to_string(), Reach::describe);
    let named = inner
        .and_then(Reach::node)
        .map_or(d.clone(), |n| format!("#{n}"));
    let how = format!(
        "tap {named}, or tap #{id} at <x> <y> (a point in its box) for whatever a finger there reaches"
    );
    if own {
        format!("tap #{id} would press {d} inside it, at its middle; no point of #{id} a finger can reach presses #{id} itself; {how}")
    } else {
        format!("tap #{id} would press {d} inside it; #{id} has no press of its own, and a tap that names it never presses a control inside it; {how}")
    }
}
