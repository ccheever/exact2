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
    /// Explicit pixels into the row wrapper; overrides main-axis alignment.
    pub offset: Option<f64>,
}

/// Correcting reports a request may take before it ends `unconverged`.
const CORRECTIONS: u8 = 6;

/// A request a list is carrying out.
#[derive(Debug, Clone)]
pub(super) struct Target {
    key: Rc<str>,
    align: Align,
    smooth: bool,
    offset: Option<f64>,

    reports: u8,
    /// Consecutive reports that found it aligned, everything to it measured.
    aligned: u8,
    /// A nested request whose outer list waits: its inner part, begun in the
    /// outer row's list once the outer row lands (§3).
    inner: Option<IntoView>,
    /// It has been admitted: when it was made, or where its wait ended.
    moved: bool,
}

/// How a request ended, for `state`.
#[derive(Debug, Clone, PartialEq)]
pub enum IntoViewStatus {
    /// Aligned within half a pixel.
    Done,
    /// Still correcting.
    Pending,
    /// Its corrections (`CORRECTIONS`) did not land it.
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
    /// A nested request's inner target in this list's row `row`, looked up
    /// before either list moves: `None` when this list holds no such row (or
    /// none of its rows holds a list), otherwise whether the row's list
    /// `request.list` holds the key, and why not (§2.2).
    fn inner_target(
        &mut self,
        u: &Update<'_>,
        frames: &[Frame],
        row: &Value,
        request: &IntoView,
    ) -> Result<Option<Result<(), String>>, InstanceError> {
        if !self.nested {
            return Ok(None);
        }
        let Ok(key) = self.resolve(row) else {
            return Ok(None);
        };
        let position = self.index.position(&key).expect("resolved row");
        self.inner_target_at(u, frames, position, request).map(Some)
    }
    /// `inner_target` in the row at `position`, whose list holds rows.
    fn inner_target_at(
        &mut self,
        u: &Update<'_>,
        frames: &[Frame],
        position: usize,
        request: &IntoView,
    ) -> Result<Result<(), String>, InstanceError> {
        // Resolve an unmounted row in a disposable tree before moving either list.
        let mut ids = Ids::default();
        let mut scratch = Update::new(Env { ..u.env }, u.sites, &mut ids);
        scratch.discard = true;
        let mut temporary;
        let row = if let Some(mounted) = self.mounted.iter_mut().find(|m| m.position == position) {
            &mut mounted.row
        } else {
            temporary = self.create_row(&mut scratch, position, frames)?;
            &mut temporary
        };
        let mut inner_frames = frames.to_vec();
        inner_frames.push(row.frame.clone());
        let mut inner = Vec::new();
        lists(
            &mut row.roots,
            (u.env.plan, u.sites),
            &inner_frames,
            &mut inner,
        );
        Ok(
            match inner
                .into_iter()
                .find(|(_, id, _)| id.as_deref() == Some(request.list.as_str()))
            {
                Some((list, _, _)) => list.resolve(&request.key).map(|_| ()),
                None => Err(format!("the row holds no list with id {}", request.list)),
            },
        )
    }
    /// A nested request whose outer list's wait ends, its inner part not yet
    /// begun: its inner target is looked up again before the outer list
    /// moves, since the items may have changed during the wait, and the
    /// request is refused whole if it is gone (§2.6).
    pub(super) fn refuse_if_inner_gone(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
    ) -> Result<(), InstanceError> {
        let Some(request) = self.target.as_ref().and_then(|t| t.inner.clone()) else {
            return Ok(());
        };
        let Some(position) = self.target_row() else {
            return Ok(());
        };
        if let Err(why) = self.inner_target_at(u, frames, position, &request)? {
            u.refused.push((request.list.clone(), why.clone()));
            self.end_into_view(IntoViewStatus::Refused(why));
        }
        Ok(())
    }

    /// The option that applies on this list's axis: `block` on a vertical
    /// one, `inline` on a row list.
    fn align_for(&self, request: &IntoView) -> Align {
        match self.axis {
            ListAxis::Vertical => request.block,
            ListAxis::Horizontal => request.inline,
        }
    }
    /// Insets from the row wrapper to its alignment area: ordinary margins
    /// bring it to the border box, and scroll margins expand that box.
    fn root_margins(&self, u: &Update<'_>, position: usize) -> (f64, f64) {
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
        // As the kernel lays it out, under a terminal's rule (any length, in
        // whole cells); else as the plan binds it.
        let laid = u.cell_margins.map(|margins| margins(root.view));
        let number = |id| match (laid, root.bound_style(u.env.plan, u.sites, id).as_ref()) {
            (Some([t, r, b, l]), _) => match id {
                StyleId::MarginTop => t,
                StyleId::MarginRight => r,
                StyleId::MarginBottom => b,
                _ => l,
            },
            (None, Some(Value::Number(n))) if n.is_finite() => *n,
            _ => 0.0,
        };
        let [top, right, bottom, left] = u
            .scroll_margin
            .map_or([0.0; 4], |resolve| resolve(root.view));
        let (start, end) = match self.axis {
            ListAxis::Vertical => (top, bottom),
            ListAxis::Horizontal => (left, right),
        };
        (number(a) - start, number(b) - end)
    }
    /// The list's `scroll-padding` at each end along its axis, as its
    /// committed style resolves it now: CSSOM View reads it when
    /// `scrollIntoView` runs.
    fn scroll_padding(&self, u: &Update<'_>) -> [f64; 2] {
        let Some(resolve) = u.scroll_padding else {
            return [0.0; 2];
        };
        let [top, right, bottom, left] =
            resolve(self.view).map(|n| if n.is_finite() { n.max(0.0) } else { 0.0 });
        match self.axis {
            ListAxis::Vertical => [top, bottom],
            ListAxis::Horizontal => [left, right],
        }
    }
    /// The offset that aligns row `position` by `align`, from the index,
    /// within the port less its `scroll-padding`, as CSS aligns in the
    /// snapport (@ref LLP 1010 §6.9), and less what the platform covers of it
    /// (`covered`), as UIKit's `scrollToItem(at:at:animated:)` aligns within
    /// the adjusted content insets. It may be negative, down to the padding
    /// before the first row: `scrollTop` 0.
    fn aligned(
        &self,
        u: &Update<'_>,
        position: usize,
        align: Align,
        current: f64,
        offset: Option<f64>,
    ) -> f64 {
        if let Some(offset) = offset {
            let port = self.geometry.as_ref().map_or(0.0, |g| g.port_main);
            return (self.index.prefix(position).unwrap_or(0.0) + offset)
                .clamp(-self.index.leading(), self.index.max_offset(port));
        }
        let (before, after) = self.root_margins(u, position);
        let start = self.index.prefix(position).unwrap_or(0.0) + before;
        let size = (self.index.height(position).unwrap_or(0.0) - before - after).max(0.0);
        let port = self.geometry.as_ref().map_or(0.0, |g| g.port_main);
        let [padding_start, padding_end] = self.scroll_padding(u);
        let (inset_start, inset_end) = (
            padding_start + self.covered[0],
            padding_end + self.covered[1],
        );
        // The snapport, as offsets from the port's start.
        let (low, high) = (inset_start, (port - inset_end).max(inset_start));
        let view = high - low;
        let at = match align {
            Align::Start => start - low,
            Align::Center => start + size / 2.0 - (low + high) / 2.0,
            Align::End => start + size - high,
            // CSSOM View's "nearest": a row that covers the snapport, or
            // fits inside it, stays; else the nearer edge aligns.
            Align::Nearest => {
                let (top, bottom) = (start - current, start + size - current);
                let above = top < low;
                let below = bottom > high;
                if above == below {
                    current
                } else if above == (size <= view) {
                    start - low
                } else {
                    start + size - high
                }
            }
        };
        at.clamp(-self.index.leading(), self.index.max_offset(port))
    }
    /// Start a request for the row keyed `key`: its window is built at the
    /// destination now, and the host is told to move there before it paints.
    /// While the host says the reader's scroll is under way and a jump would
    /// not end it, or where the port is is unknown, it waits instead: nothing
    /// moves, the reader's window stays, and the destination is computed
    /// when the scroll ends or a report says where the port is.
    pub(super) fn begin_into_view(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        key: &str,
        align: Align,
        smooth: bool,
        within: Option<f64>,
    ) -> Result<(), InstanceError> {
        let position = self.index.position(key).expect("resolved");
        self.restored_at = None;
        self.at_end = false;
        // A new request stops the host's animation where the port is (§2.5).
        self.port_unknown |= self.host_animating();
        self.request = self.request.wrapping_add(1).max(1);
        let key = self.index.shared_key(position).expect("resolved").clone();
        self.target = Some(Target {
            key: key.clone(),
            align,
            smooth,
            offset: within,
            reports: 0,
            aligned: 0,
            inner: None,
            moved: false,
        });
        self.into_view_status = Some((key, IntoViewStatus::Pending));
        // One the port already satisfies, where the last report and an
        // anchor's correction since put it, is done and moves nothing, as
        // CSSOM View skips a scroll to where the port already is, judged when
        // it is made: a reader's fling goes on (§2.6). Not while the host's
        // animation leaves the port's position unknown.
        if let Some(g) = self.geometry.as_ref().filter(|_| !self.port_unknown) {
            if self.already_satisfied(u, position, align, within, g.offset) {
                self.end_into_view(IntoViewStatus::Done);
                advance(&mut self.revision)?;
                return Ok(());
            }
        }
        if self.request_waits() {
            // An authored position this list still owed (a kept position, an
            // end followed) is the request's to replace when it lands; an
            // anchor's relative correction still moves with its rows.
            if self.correction.is_some_and(|c| c.from.is_none()) {
                self.correction = None;
            }
            advance(&mut self.revision)?;
            return Ok(());
        }
        self.admit(u, frames, position, align, smooth, within)?;
        if let Some(t) = &mut self.target {
            t.moved = true;
        }
        advance(&mut self.revision)?;
        Ok(())
    }
    /// Align row `position` by `align` from where the port now is, build
    /// the window at that destination, and tell the host to move there.
    fn admit(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        position: usize,
        align: Align,
        smooth: bool,
        within: Option<f64>,
    ) -> Result<(), InstanceError> {
        let current = self
            .geometry
            .as_ref()
            .map_or(self.start_offset, |g| g.offset);
        let offset = self.aligned(u, position, align, current, within);
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
                self.start_offset = offset.max(0.0);
                self.correction = Some(AnchorCorrection {
                    scroll_sequence: 0,
                    offset,
                    from: None,
                    smooth: false,
                });
            }
        }
        self.realize_window_with_row(
            u,
            frames,
            false,
            CollectionFill::default(),
            Some(position),
            false,
        )
    }
    /// Whether the request under way waits for the reader's scroll or for a
    /// report of where the port is (§2.6).
    pub(super) fn request_waits(&self) -> bool {
        self.target.is_some() && (self.scrolling || self.port_unknown)
    }
    /// The host animates a smooth correction, or one is on its way to it:
    /// where the port is, its reports do not say (§11).
    fn host_animating(&self) -> bool {
        self.animating || self.correction.is_some_and(|c| c.smooth)
    }
    /// Whether the list asks the host for its next report: it owes rows
    /// (`pending`), a request has been admitted, or where the port is is
    /// unknown, which nothing else may make the host say. A request that
    /// waits for the reader's scroll rides the scroll's reports.
    pub(super) fn asks_report(&self) -> bool {
        self.pending || self.port_unknown || (self.target.is_some() && !self.request_waits())
    }
    /// No request, or one a report leaves alone: it waits on, the reader
    /// still scrolling. The travel fast path may take such a report.
    pub(super) fn request_untouched_by(&self, fill: &CollectionFill) -> bool {
        self.target.is_none() || (self.scrolling && fill.scrolling && !fill.gesture)
    }
    /// The request ends `status`: nothing it planned moves the port after.
    /// One that ends while the host animates leaves the port where the host
    /// stops it, which only the host's next report says (`port_unknown`).
    /// Whether one was under way.
    fn end_into_view(&mut self, status: IntoViewStatus) -> bool {
        let Some(t) = self.target.take() else {
            return false;
        };
        self.port_unknown |= self.host_animating();
        self.into_view_status = Some((t.key, status));
        true
    }
    /// The request's row left the items: it ends now, where the reader is
    /// (LLP 1070.000 §2.5), not at the next report.
    pub(super) fn cancel_if_row_left(&mut self) {
        if self
            .target
            .as_ref()
            .is_some_and(|t| self.index.position(&t.key).is_none())
        {
            self.end_into_view(IntoViewStatus::Cancelled);
        }
    }
    pub(super) fn target_row(&self) -> Option<usize> {
        self.index.position(&self.target.as_ref()?.key)
    }
    /// The list `id` in this list's mounted row keyed `row`, and the frames
    /// in force there (§3).
    fn inner_list(
        &mut self,
        u: &Update<'_>,
        frames: &[Frame],
        row: &str,
        id: &str,
    ) -> Result<(&mut Collection, Vec<Frame>), String> {
        let Some(mounted) = self
            .mounted
            .iter_mut()
            .find(|m| super::super::ident(&m.row.key, m.row.dup).as_deref() == Some(row))
        else {
            return Err("the outer row did not mount".into());
        };
        let mut inner_frames = frames.to_vec();
        inner_frames.push(mounted.row.frame.clone());
        let mut inner = Vec::new();
        lists(
            &mut mounted.row.roots,
            (u.env.plan, u.sites),
            &inner_frames,
            &mut inner,
        );
        inner
            .into_iter()
            .find(|(_, list, _)| list.as_deref() == Some(id))
            .map(|(list, _, frames)| (list, frames))
            .ok_or_else(|| format!("the row holds no list with id {id}"))
    }
    /// After a nested request's outer part was admitted, its row built at
    /// the destination: the inner list in that row begins the inner part, as
    /// the command begins it in the outer row at rest (§3). Its target was
    /// looked up where the wait ended (`refuse_if_inner_gone`).
    pub(super) fn begin_inner(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
    ) -> Result<(), InstanceError> {
        let Some(t) = self.target.as_mut() else {
            return Ok(());
        };
        let Some(request) = t.inner.take() else {
            return Ok(());
        };
        let row = t.key.clone();
        if let Err(why) = self.begin_inner_part(u, frames, &row, &request)? {
            self.end_into_view(IntoViewStatus::Refused(why));
        }
        Ok(())
    }
    /// A nested request's inner part, its outer row `row` where it should be:
    /// the inner list in that row begins it, as the command begins it at rest
    /// (§3). What the command refuses at rest (§2.2) it refuses here, and why,
    /// journaled as the command's refusal is.
    pub(super) fn begin_inner_part(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        row: &str,
        request: &IntoView,
    ) -> Result<Result<(), String>, InstanceError> {
        let begun = match self.inner_list(u, frames, row, &request.list) {
            Ok((list, frames)) => list.begin_request(u, &frames, request)?,
            Err(why) => Err(why),
        };
        if let Err(why) = &begun {
            u.refused.push((request.list.clone(), why.clone()));
        }
        Ok(begun)
    }
    /// Begin `request` on this list, aligned by the option for its axis; why
    /// not, if the list refuses its key (§2.2).
    fn begin_request(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        request: &IntoView,
    ) -> Result<Result<(), String>, InstanceError> {
        let key = match self.resolve(&request.key) {
            Ok(key) => key,
            Err(why) => return Ok(Err(why)),
        };
        let align = self.align_for(request);
        self.begin_into_view(u, frames, &key, align, request.smooth, request.offset)?;
        Ok(Ok(()))
    }
    /// A report while a request is under way: cancelled if the reader began
    /// a gesture since it was made, or moved the port after it was admitted.
    /// While it waits, the port's motion is the reader's scroll from before
    /// it.
    pub(super) fn follow_into_view(&mut self, fill: CollectionFill) {
        // Once it has moved the port, the reader's own scroll that the host
        // hears ends it where the reader is: its corrections do not fight
        // the reader, who can always stop its motion (§2.5). A host reports
        // the reader's move of the port then as a gesture.
        let moved = self.target.as_ref().is_some_and(|t| t.moved);
        if fill.gesture || (moved && fill.scrolling) {
            self.end_into_view(IntoViewStatus::Cancelled);
        }
    }
    /// After a report's measurements: where the target now aligns. A
    /// request that lands after a wait (`landing`) makes its first move,
    /// uncounted, as `begin_into_view`'s is.
    /// A nested request done there, its outer row already where it should
    /// be, hands back its outer row and inner part, which begins in this
    /// report (`begin_inner_part`).
    pub(super) fn settle_into_view(
        &mut self,
        u: &Update<'_>,
        landing: bool,
    ) -> Option<(Rc<str>, IntoView)> {
        let target = self.target.clone()?;
        let status = self.settle_target(u, &target, landing)?;
        let done = status == IntoViewStatus::Done;
        self.end_into_view(status);
        target
            .inner
            .filter(|_| done)
            .map(|inner| (target.key, inner))
    }
    /// Whether the port already satisfies a request for row `position`
    /// wherever it may stand: where the last report and an anchor's shift
    /// since put it (`at`), and on a row list also where it stands without
    /// that shift, which the web does not make while the reader moves the
    /// list (LLP 1070 H4). Not while a move of the list's own is owed.
    fn already_satisfied(
        &self,
        u: &Update<'_>,
        position: usize,
        align: Align,
        within: Option<f64>,
        at: f64,
    ) -> bool {
        let unshifted = match self.correction {
            Some(AnchorCorrection { from: None, .. }) => return false,
            Some(AnchorCorrection {
                offset,
                from: Some(from),
                ..
            }) if self.axis == ListAxis::Horizontal => at - (offset - from),
            _ => at,
        };
        let holds = |p: f64| {
            self.satisfies_measured(position, p)
                && (self.aligned(u, position, align, p, within) - p).abs() <= 0.5
        };
        holds(at) && (unshifted == at || holds(unshifted))
    }
    /// Whether row `position` lays out where the index puts it, the port at
    /// `at`, for a request it may already satisfy: it mounted and measured,
    /// and no row between the port's start and it mounted unmeasured (laid
    /// out at its real size, an estimate here). A row above the port, once
    /// measured, moves the port with it (the anchor's correction), and a row
    /// not mounted is laid out as the index has it: neither moves the target
    /// in the port, nor do the rows a fill still owes past it.
    fn satisfies_measured(&self, position: usize, at: f64) -> bool {
        let start = self
            .index
            .row_at(at.max(0.0))
            .ok()
            .flatten()
            .unwrap_or(position)
            .min(position);
        self.index.is_measured_at(position)
            && self.mounted.iter().any(|m| m.position == position)
            && !self.mounted.iter().any(|m| {
                (start..position).contains(&m.position) && !self.index.is_measured_at(m.position)
            })
    }
    /// Whether row `position` aligns where the index puts it for completion:
    /// it, and every mounted row up to it, measured, and the window whole.
    fn alignment_measured(&self, position: usize) -> bool {
        // Every mounted row up to it: one above the port, laid out at its
        // real size but still an estimate here, puts the target elsewhere.
        // The rows between a kept row and the window are not mounted and
        // never measured, so the range is the run of mounted rows that
        // reaches the target, and a mounted row before that run counts alone.
        let (mut first, mut joined, mut others) = (position, true, true);
        for m in self.mounted.iter().rev().filter(|m| m.position < position) {
            if joined && m.position + 1 == first {
                first = m.position;
            } else {
                joined = false;
                others &= self.index.is_measured_at(m.position);
            }
        }
        // And not before the window is whole: a row mounted later above it
        // is laid out at its real size before it is measured here.
        others && self.index.range_measured(first..position + 1) && !self.pending
    }
    /// How the request stands after a report, if it ended.
    fn settle_target(
        &mut self,
        u: &Update<'_>,
        target: &Target,
        landing: bool,
    ) -> Option<IntoViewStatus> {
        let Some(position) = self.index.position(&target.key) else {
            return Some(IntoViewStatus::Cancelled);
        };
        let g = self.geometry.as_ref()?;
        // Judged where the host says the port is, not where the runner last
        // asked it to be, as this report's anchor correction leaves it: the
        // index already has the report's measurements.
        let at = g.offset;
        // Where its wait ends it is judged again as one made at rest (§2.6).
        // A nested request's inner part then begins in this report
        // (`settle_into_view`).
        if landing && self.already_satisfied(u, position, target.align, target.offset, at) {
            return Some(IntoViewStatus::Done);
        }
        if self.request_waits() {
            return None;
        }
        // The wait over, it is admitted, which counts as a move whether or
        // not a correction follows (§2.5's rule then ends it)
        if landing {
            if let Some(t) = &mut self.target {
                t.moved = true;
            }
        }
        let desired = self.aligned(u, position, target.align, at, target.offset);
        let satisfied = (desired - at).abs() <= 0.5;
        let measured = self.alignment_measured(position);
        if satisfied {
            // Done when it holds for two reports: filling around it measures
            // rows that can still move it. And not while the host animates
            // there: it reports where the animation is headed, not where the
            // port is (§11).
            let aligned = if measured && !self.animating {
                target.aligned + 1
            } else {
                0
            };
            if let Some(t) = &mut self.target {
                t.aligned = aligned;
            }
            if aligned >= 2 {
                return Some(IntoViewStatus::Done);
            }
            return None;
        }
        if let Some(t) = &mut self.target {
            t.aligned = 0;
        }
        if target.reports >= CORRECTIONS {
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
        // Landing after a wait is the request's first move, uncounted, as
        // `begin_into_view`'s is
        if let Some(t) = self.target.as_mut().filter(|_| !landing) {
            t.reports += 1;
        }
        None
    }
}

/// A collection, its node's `id`, and the frames in force at it.
type Found<'a> = (&'a mut Collection, Option<exact_plan::Str>, Vec<Frame>);

/// The collections under `children`, outside other collections' rows,
/// each with its node's `id` and the frames in force at it.
fn lists<'a>(
    children: &'a mut [Child],
    (plan, sites): (&Plan, &SiteIndex),
    frames: &[Frame],
    out: &mut Vec<Found<'a>>,
) {
    for child in children {
        match child {
            Child::Node(node) => {
                let id = match node.bound_prop(plan, sites, PropId::Id) {
                    Some(ref v @ exact_plan::str_value!()) => v.to_shared_str(),
                    _ => None,
                };
                match crate::instance::rare::list_mut(&mut node.rare) {
                    Some(c) => out.push((c, id, frames.to_vec())),
                    None => lists(&mut node.children, (plan, sites), frames, out),
                }
            }
            Child::Region(region) => match &mut region.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.scope());
                    lists(roots, (plan, sites), &inner, out);
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        lists(&mut row.roots, (plan, sites), &inner, out);
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
                if node.collection().is_some_and(|c| c.view == view) {
                    return node.collection_mut().map(|c| (c, frames.to_vec()));
                }
                if let Some(c) = crate::instance::rare::list_mut(&mut node.rare) {
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
                    inner.push(frame.scope());
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
        if let Some(s) = key.as_str() {
            if let Ok(b) = s.parse::<bool>() {
                if let Ok(found) = self.resolve_exactly(&Value::Bool(b)) {
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
    /// [`crate::Runner::set_list_covered`]: whether `view` is a mounted list.
    pub(crate) fn set_list_covered(&mut self, view: ViewId, covered: [f64; 2]) -> bool {
        let Some((list, _)) = by_view(&mut self.children, view, &[]) else {
            return false;
        };
        list.covered = covered.map(|n| if n.is_finite() { n.max(0.0) } else { 0.0 });
        true
    }

    /// Keyboard traversal already knows a logical row position. Resolve its
    /// identity here, without exporting the unmounted keys to a host.
    pub(crate) fn reveal_collection_index(
        &mut self,
        u: &mut Update<'_>,
        view: ViewId,
        index: usize,
    ) -> Result<bool, InstanceError> {
        let Some((list, frames)) = by_view(&mut self.children, view, &[]) else {
            return Ok(false);
        };
        if list.index.key(index).is_none() {
            return Ok(false);
        }
        // Keyboard focus owns the next position. An earlier authored seek
        // must not pull it back after a resize (including a zero-sized port).
        let cancelled = list.end_into_view(IntoViewStatus::Cancelled);
        if cancelled || list.correction.is_some() {
            list.correction = None;
            advance(&mut list.revision)?;
        }
        if list.mounted.iter().any(|row| row.position == index) {
            return Ok(true);
        }
        // This window is provisional. The host focuses its chosen descendant
        // before reporting geometry; ordinary focus reveal sets the offset.
        list.restored_at = None;
        list.at_end = false;
        list.admit(u, &frames, index, Align::Nearest, false, None)?;
        list.correction = None;
        advance(&mut list.revision)?;
        Ok(true)
    }

    /// Carry out one `scrollIntoView` (LLP 1070.000 §2): resolve the list,
    /// and for an inner one its outer row, and begin the request on each
    /// scroller between the target and the list.
    pub(crate) fn scroll_into_view(
        &mut self,
        u: &mut Update<'_>,
        request: &IntoView,
        continuation: &mut Option<IntoView>,
    ) -> Result<IntoViewStatus, InstanceError> {
        let plan = u.env.plan;
        if request.offset.is_some_and(|n| !n.is_finite()) {
            return Ok(IntoViewStatus::Refused(
                "offset must be a finite number of CSS pixels".into(),
            ));
        }
        if let Some(view) = request.view {
            let Some((list, frames)) = by_view(&mut self.children, view, &[]) else {
                return Ok(IntoViewStatus::Refused(format!(
                    "view {view} is not a mounted virtualized list"
                )));
            };
            return Ok(list
                .begin_request(u, &frames, request)?
                .map_or_else(IntoViewStatus::Refused, |()| IntoViewStatus::Pending));
        }
        let mut found = Vec::new();
        lists(&mut self.children, (plan, u.sites), &[], &mut found);
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
            return Ok(list
                .begin_request(u, &frames, request)?
                .map_or_else(IntoViewStatus::Refused, |()| IntoViewStatus::Pending));
        };
        // An inner list: the outer list whose rows hold lists with this id,
        // and which holds the row.
        // The first whose row holds the inner target; refused before anything
        // moves when none does, for the first reason found.
        let (mut selected, mut why) = (None, None);
        for (outer, _, frames) in found {
            match outer.inner_target(u, &frames, row, request)? {
                Some(Ok(())) => {
                    selected = Some((outer, frames));
                    break;
                }
                Some(Err(e)) => {
                    why.get_or_insert(e);
                }
                None => {}
            }
        }
        let Some((outer, frames)) = selected else {
            return Ok(IntoViewStatus::Refused(why.unwrap_or_else(|| {
                format!(
                    "no virtualized list holds a row keyed {}",
                    super::super::key_text(row).unwrap_or_default()
                )
            })));
        };
        let row_key = outer.resolve(row).expect("resolved");
        let align = outer.align_for(request);
        outer.begin_into_view(u, &frames, &row_key, align, request.smooth, None)?;
        // The outer list waits (§2.6): the inner part begins when the outer
        // row lands (`begin_inner`).
        if outer.request_waits() {
            if let Some(t) = outer.target.as_mut() {
                t.inner = Some(request.clone());
                return Ok(IntoViewStatus::Pending);
            }
        }
        // `inner_target` found the list in the row before the outer list moved
        let list = match outer.inner_list(u, &frames, &row_key, &request.list) {
            Ok((list, _)) => list.view,
            Err(why) => return Ok(IntoViewStatus::Refused(why)),
        };
        // The outer seek may have just created this list. Its styles must
        // compute before the inner alignment reads them, within the same commit.
        *continuation = Some(IntoView {
            view: Some(list),
            row: None,
            ..request.clone()
        });
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
                    if let Some(c) = node.collection() {
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
