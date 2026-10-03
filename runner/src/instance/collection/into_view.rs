//! `scrollIntoView` on a virtualized list (LLP 1070.000): a row by its key,
//! aligned as `Element.scrollIntoView()` aligns it, its window built at the
//! destination before any host moves, corrected until it lands.
use super::*;

/// How a row aligns in a port along one axis, CSS's `ScrollLogicalPosition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Its start at the port's start (the web's `block` default).
    Start,
    /// Its middle at the port's middle.
    Center,
    /// Its end at the port's end.
    End,
    /// The least movement that shows it whole (the web's `inline` default).
    Nearest,
}
impl Align {
    /// CSS's name, or none.
    pub fn parse(name: &str) -> Option<Align> {
        Some(match name {
            "start" => Align::Start,
            "center" => Align::Center,
            "end" => Align::End,
            "nearest" => Align::Nearest,
            _ => return None,
        })
    }
}

/// One request, as the command or the agent's `tap … into` states it.
#[derive(Debug, Clone, PartialEq)]
pub struct IntoView {
    /// The virtualized list's `id`.
    pub list: String,
    /// The row's key.
    pub key: Value,
    /// Alignment in vertical ports.
    pub block: Align,
    /// Alignment in horizontal ports.
    pub inline: Align,
    /// For an inner list, its outer row's key.
    pub row: Option<Value>,
    /// The list by its view instead of its `id`: the agent's `tap <list>
    /// into <key>`, whose target is a mounted list.
    pub view: Option<ViewId>,
    /// `behavior="smooth"` (LLP 1070.000 §6.2): the host animates to the
    /// destination; the window is built there at once, as for `auto`.
    pub smooth: bool,
}

/// Correcting reports a request may take before it ends `unconverged`.
const CORRECTIONS: u8 = 6;

/// A request a list is carrying out.
#[derive(Debug, Clone)]
pub(super) struct Target {
    key: Rc<str>,
    align: Align,
    smooth: bool,

    reports: u8,
    /// Consecutive reports that said the port was travelling.
    travelling: u8,
    /// Consecutive reports that found it aligned, everything to it measured.
    aligned: u8,
}

/// How a request ended, for `state`.
#[derive(Debug, Clone, PartialEq)]
pub enum IntoViewStatus {
    /// Aligned within half a pixel.
    Done,
    /// Still correcting.
    Pending,
    /// Four corrections did not land it.
    Unconverged,
    /// The reader scrolled, a newer request came, or the key left.
    Cancelled,
    /// Refused before anything moved.
    Refused(String),
}
impl IntoViewStatus {
    pub(crate) fn json(&self, out: &mut String) {
        let name = match self {
            IntoViewStatus::Done => "done",
            IntoViewStatus::Pending => "pending",
            IntoViewStatus::Unconverged => "unconverged",
            IntoViewStatus::Cancelled => "cancelled",
            IntoViewStatus::Refused(why) => {
                out.push_str("\"refused: ");
                out.push_str(&why.replace('"', "'"));
                out.push('"');
                return;
            }
        };
        out.push('"');
        out.push_str(name);
        out.push('"');
    }
}

impl Collection {
    /// The option that applies on this list's axis: `block` on a vertical
    /// one, `inline` on a row list.
    fn align_for(&self, request: &IntoView) -> Align {
        match self.axis {
            ListAxis::Vertical => request.block,
            ListAxis::Horizontal => request.inline,
        }
    }
    /// The row root's margins along this list's axis, when it is mounted:
    /// its wrapper encloses them, and the web aligns the element's border
    /// box, inside them.
    fn root_margins(&self, plan: &Plan, position: usize) -> (f64, f64) {
        use exact_kernel::StyleId;
        let Some(Child::Node(root)) = self
            .mounted
            .iter()
            .find(|m| m.position == position)
            .and_then(|m| m.row.roots.first())
        else {
            return (0.0, 0.0);
        };
        let [a, b] = match self.axis {
            ListAxis::Vertical => [StyleId::MarginTop, StyleId::MarginBottom],
            ListAxis::Horizontal => [StyleId::MarginLeft, StyleId::MarginRight],
        };
        let number = |id| match root.bound_style(plan, id) {
            Some(Value::Number(n)) if n.is_finite() => *n,
            _ => 0.0,
        };
        (number(a), number(b))
    }
    /// The offset that aligns row `position` by `align`, from the index.
    fn aligned(&self, plan: &Plan, position: usize, align: Align, current: f64) -> f64 {
        let (before, after) = self.root_margins(plan, position);
        let start = self.index.prefix(position).unwrap_or(0.0) + before;
        let size = (self.index.height(position).unwrap_or(0.0) - before - after).max(0.0);
        let port = self.geometry.as_ref().map_or(0.0, |g| g.port_main);
        let at = match align {
            Align::Start => start,
            Align::Center => start + size / 2.0 - port / 2.0,
            Align::End => start + size - port,
            Align::Nearest if start < current => start,
            Align::Nearest if start + size > current + port => {
                if size > port {
                    start
                } else {
                    start + size - port
                }
            }
            Align::Nearest => current,
        };
        at.clamp(0.0, (self.index.total_height() - port).max(0.0))
    }
    /// Start a request for the row keyed `key`: its window is built at the
    /// destination now, and the host is told to move there before it paints.
    pub(super) fn begin_into_view(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        key: &str,
        align: Align,
        smooth: bool,
    ) -> Result<(), InstanceError> {
        let position = self.index.position(key).expect("resolved");
        let current = self
            .geometry
            .as_ref()
            .map_or(self.start_offset, |g| g.offset);
        let offset = self.aligned(u.env.plan, position, align, current);
        self.restored_at = None;
        self.at_end = false;
        self.target = Some(Target {
            key: self.index.shared_key(position).expect("resolved").clone(),
            align,
            smooth,
            reports: 0,
            travelling: 0,
            aligned: 0,
        });
        self.into_view_status = Some((
            self.index.shared_key(position).expect("resolved").clone(),
            IntoViewStatus::Pending,
        ));
        match &mut self.geometry {
            Some(g) => {
                g.offset = offset;
                self.correction = Some(AnchorCorrection {
                    scroll_sequence: g.scroll_sequence,
                    offset,
                    from: None,
                    smooth,
                });
            }
            None => {
                self.start_offset = offset;
                self.correction = Some(AnchorCorrection {
                    scroll_sequence: 0,
                    offset,
                    from: None,
                    smooth: false,
                });
            }
        }
        self.realize_window(u, frames, false, CollectionFill::default())?;
        advance(&mut self.revision)?;
        Ok(())
    }
    /// A report while a request is under way: cancelled if the reader moved
    /// the port; otherwise the destination from what is now measured, and a
    /// correction when the port is not there. The status when it ended.
    pub(super) fn follow_into_view(&mut self, fill: CollectionFill) -> Option<IntoViewStatus> {
        let target = self.target.as_mut()?;
        // The reader's own motion: travel a host samples from a drag, a
        // fling or a wheel, in two reports running. (One report's velocity
        // can be the browser clamping the port when the window's extent
        // changed; the host's sequence moves for that too.)
        target.travelling = if fill.velocity != 0.0 {
            target.travelling + 1
        } else {
            0
        };
        if target.travelling >= 2 {
            let key = target.key.clone();
            self.target = None;
            self.into_view_status = Some((key, IntoViewStatus::Cancelled));
            return Some(IntoViewStatus::Cancelled);
        }
        None
    }
    /// After a report's measurements: where the target now aligns.
    pub(super) fn settle_into_view(
        &mut self,
        plan: &Plan,
        reported: f64,
    ) -> Option<IntoViewStatus> {
        let target = self.target.clone()?;
        let ended = self.settle_target(plan, &target, reported);
        if let Some(status) = &ended {
            self.into_view_status = Some((target.key, status.clone()));
        }
        ended
    }
    fn settle_target(
        &mut self,
        plan: &Plan,
        target: &Target,
        reported: f64,
    ) -> Option<IntoViewStatus> {
        let Some(position) = self.index.position(&target.key) else {
            self.target = None;
            return Some(IntoViewStatus::Cancelled);
        };
        let g = self.geometry.as_ref()?;
        // Judged where the host says the port is, not where the runner
        // last asked it to be.
        let desired = self.aligned(plan, position, target.align, reported);
        // Every mounted row up to it: one above the port, laid out at its
        // real size but still an estimate here, puts the target elsewhere.
        let first = self
            .mounted
            .first()
            .map_or(position, |m| m.position)
            .min(position);
        let measured = self.index.range_measured(first..position + 1);
        if (desired - reported).abs() <= 0.5 {
            // Done when it holds for two reports: filling around it measures
            // rows that can still move it.
            // And not before the window is whole: a row mounted later above
            // it is laid out at its real size before it is measured here.
            let aligned = if measured && !self.pending {
                target.aligned + 1
            } else {
                0
            };
            if let Some(t) = &mut self.target {
                t.aligned = aligned;
            }
            if aligned >= 2 {
                self.target = None;
                return Some(IntoViewStatus::Done);
            }
            return None;
        }
        if let Some(t) = &mut self.target {
            t.aligned = 0;
        }
        if target.reports >= CORRECTIONS {
            self.target = None;
            return Some(IntoViewStatus::Unconverged);
        }
        let sequence = g.scroll_sequence;
        if let Some(g) = &mut self.geometry {
            g.offset = desired;
        }
        self.correction = Some(AnchorCorrection {
            scroll_sequence: sequence,
            offset: desired,
            from: None,
            smooth: target.smooth,
        });
        if let Some(t) = &mut self.target {
            t.reports += 1;
        }
        None
    }
}

/// A collection, its node's `id`, and the frames in force at it.
type Found<'a> = (&'a mut Collection, Option<exact_plan::Str>, Vec<Frame>);

/// The collections under `children`, outside other collections' rows,
/// each with its node's `id` and the frames in force at it.
fn lists<'a>(children: &'a mut [Child], plan: &Plan, frames: &[Frame], out: &mut Vec<Found<'a>>) {
    for child in children {
        match child {
            Child::Node(node) => {
                let id = match node.bound_prop(plan, PropId::Id) {
                    Some(v @ exact_plan::str_value!()) => v.to_shared_str(),
                    _ => None,
                };
                match &mut node.collection {
                    Some(c) => out.push((c, id, frames.to_vec())),
                    None => lists(&mut node.children, plan, frames, out),
                }
            }
            Child::Region(region) => match &mut region.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.clone());
                    lists(roots, plan, &inner, out);
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        lists(&mut row.roots, plan, &inner, out);
                    }
                }
            },
        }
    }
}

/// The mounted collection whose view is `view`, rows of mounted collections
/// included (the agent names a mounted inner list by its view), with the
/// frames in force at it.
fn by_view<'a>(
    children: &'a mut [Child],
    view: ViewId,
    frames: &[Frame],
) -> Option<(&'a mut Collection, Vec<Frame>)> {
    for child in children {
        match child {
            Child::Node(node) => {
                if node.collection.as_ref().is_some_and(|c| c.view == view) {
                    return node.collection.as_deref_mut().map(|c| (c, frames.to_vec()));
                }
                if let Some(c) = &mut node.collection {
                    for m in &mut c.mounted {
                        let mut inner = frames.to_vec();
                        inner.push(m.row.frame.clone());
                        if let Some(found) = by_view(&mut m.row.roots, view, &inner) {
                            return Some(found);
                        }
                    }
                    continue;
                }
                if let Some(found) = by_view(&mut node.children, view, frames) {
                    return Some(found);
                }
            }
            Child::Region(region) => match &mut region.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.clone());
                    if let Some(found) = by_view(roots, view, &inner) {
                        return Some(found);
                    }
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        if let Some(found) = by_view(&mut row.roots, view, &inner) {
                            return Some(found);
                        }
                    }
                }
            },
        }
    }
    None
}

impl Collection {
    /// A key's row identity here, or why it is refused (LLP 1070.000 §2.2).
    fn resolve(&self, key: &Value) -> Result<String, String> {
        // The agent types a key as text; a list keyed by numbers reads it as one.
        if let Some(s) = key.as_str() {
            if let Ok(n) = s.parse::<f64>() {
                if let Ok(found) = self.resolve_exactly(&Value::Number(n)) {
                    return Ok(found);
                }
            }
        }
        self.resolve_exactly(key)
    }
    fn resolve_exactly(&self, key: &Value) -> Result<String, String> {
        let text = super::super::key_text(key).ok_or("the key is not a string, number or bool")?;
        if self.index.position(&text).is_none() {
            return Err(format!("no row keyed {text} in the list"));
        }
        if self
            .index
            .position(&super::super::disambiguate(text.clone(), 1))
            .is_some()
        {
            return Err(format!("the list holds {text} more than once"));
        }
        Ok(text)
    }
}

impl Tree {
    /// Carry out one `scrollIntoView` (LLP 1070.000 §2): resolve the list,
    /// and for an inner one its outer row, and begin the request on each
    /// scroller between the target and the list.
    pub(crate) fn scroll_into_view(
        &mut self,
        u: &mut Update<'_>,
        request: &IntoView,
    ) -> Result<IntoViewStatus, InstanceError> {
        let plan = u.env.plan;
        if let Some(view) = request.view {
            let Some((list, frames)) = by_view(&mut self.children, view, &[]) else {
                return Ok(IntoViewStatus::Refused(format!(
                    "view {view} is not a mounted virtualized list"
                )));
            };
            let key = match list.resolve(&request.key) {
                Ok(key) => key,
                Err(why) => return Ok(IntoViewStatus::Refused(why)),
            };
            let align = list.align_for(request);
            list.begin_into_view(u, &frames, &key, align, request.smooth)?;
            return Ok(IntoViewStatus::Pending);
        }
        let mut found = Vec::new();
        lists(&mut self.children, plan, &[], &mut found);
        let Some(row) = &request.row else {
            let Some((list, _, frames)) = found
                .into_iter()
                .find(|(_, id, _)| id.as_deref() == Some(request.list.as_str()))
            else {
                return Ok(IntoViewStatus::Refused(format!(
                    "no virtualized list has id {}",
                    request.list
                )));
            };
            let key = match list.resolve(&request.key) {
                Ok(key) => key,
                Err(why) => return Ok(IntoViewStatus::Refused(why)),
            };
            let align = list.align_for(request);
            list.begin_into_view(u, &frames, &key, align, request.smooth)?;
            return Ok(IntoViewStatus::Pending);
        };
        // An inner list: the outer list whose rows hold lists with this id,
        // and which holds the row.
        let Some((outer, _, frames)) = found
            .into_iter()
            .find(|(c, _, _)| c.nested && c.resolve(row).is_ok())
        else {
            return Ok(IntoViewStatus::Refused(format!(
                "no virtualized list holds a row keyed {}",
                super::super::key_text(row).unwrap_or_default()
            )));
        };
        let row_key = outer.resolve(row).expect("resolved");
        let align = outer.align_for(request);
        outer.begin_into_view(u, &frames, &row_key, align, request.smooth)?;
        let Some(mounted) = outer.mounted.iter_mut().find(|m| {
            super::super::ident(&m.row.key, m.row.dup).as_deref() == Some(row_key.as_str())
        }) else {
            return Ok(IntoViewStatus::Refused(
                "the outer row did not mount".into(),
            ));
        };
        let mut inner_frames = frames.clone();
        inner_frames.push(mounted.row.frame.clone());
        let mut inner = Vec::new();
        lists(&mut mounted.row.roots, plan, &inner_frames, &mut inner);
        let Some((list, _, frames)) = inner
            .into_iter()
            .find(|(_, id, _)| id.as_deref() == Some(request.list.as_str()))
        else {
            return Ok(IntoViewStatus::Refused(format!(
                "the row holds no list with id {}",
                request.list
            )));
        };
        let key = match list.resolve(&request.key) {
            Ok(key) => key,
            Err(why) => return Ok(IntoViewStatus::Refused(why)),
        };
        let align = list.align_for(request);
        list.begin_into_view(u, &frames, &key, align, request.smooth)?;
        Ok(IntoViewStatus::Pending)
    }
}

impl Tree {
    /// Every list's latest request and how it ended, for `state`.
    pub fn into_view_json(&self) -> String {
        let mut out = String::from("[");
        let mut stack: Vec<_> = self.children.iter().collect();
        while let Some(child) = stack.pop() {
            match child {
                Child::Node(node) => {
                    if let Some(c) = &node.collection {
                        if let Some((key, status)) = &c.into_view_status {
                            if out.len() > 1 {
                                out.push(',');
                            }
                            out.push_str(&format!("{{\"list\":{},\"key\":", c.view));
                            crate::agent::quote(key, &mut out);
                            out.push_str(",\"status\":");
                            status.json(&mut out);
                            out.push('}');
                        }
                        c.add_children(&mut stack);
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
        out.push(']');
        out
    }
}
