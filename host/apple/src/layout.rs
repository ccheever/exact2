//! Layout receipts survive silent list settling until the presenter sees them.
use super::*;
#[cfg(test)]
#[path = "advance_tests.rs"]
mod advance_tests;
#[path = "closed.rs"]
pub(super) mod closed;
#[path = "separator.rs"]
pub(super) mod separator;

/// The props `emit_layout` derives from a node's layout, not its author's.
pub(super) const DERIVED_PROPS: [&str; 3] = ["fitScale", "flexGrows", "textOffset"];

impl<D: DataSource> Host<D> {
    /// Configure support, admit one batch, or run the frame tasks after admission.
    pub fn timer_waits(&mut self, kinds: u8, cutoff: f64, mode: u32) -> String {
        let kinds = exact_runner::WaitKinds::from_bits(kinds);
        if mode == 0 {
            self.runner.hold_waits(kinds);
            let mut batch = Batch::new();
            self.state_onto(&mut batch);
            return self.write(batch, None);
        }
        if mode == 2 {
            let a = self.runner.complete_frame_tasks(cutoff);
            return self.advanced(a);
        }
        let admitted = self.runner.release_waits(kinds, cutoff);
        let continuing = admitted.continuing;
        let mut out = self.advanced(admitted.advanced);
        if continuing {
            out.pop();
            out.push_str(",\"waitContinue\":true}");
        }
        out
    }

    /// Writes `batch` for the presenter; every batch with ops is written
    /// here. From a failed layout until one settles, whatever turn made the
    /// batch, its layout geometry is taken out, including what an earlier
    /// pass staged in it, and the settled layout sends it again. The failed
    /// layout's own batch also loses its motion samples; later batches keep
    /// theirs, as nothing would send them again.
    pub(super) fn write(&mut self, mut batch: Batch, error: Option<&str>) -> String {
        if batch.layout_provisional {
            self.withhold_layout();
        }
        if self.layout_withheld {
            batch.withhold(!batch.layout_provisional);
        }
        batch.facts = self.facts_moved();
        batch.landing = self.runner.data_timer_landing();
        batch.wakelock = self.wakelock_moved();
        batch.orientationlock = self.orientation_moved();
        // A package view's displayed state, by the same rule, now the
        // presenter's covers are set (LLP 1133.008 D8 item 2).
        self.runner.report_views();
        batch.finish(
            self.runner.timer_due_ms(),
            !self.engine.quiescent(),
            self.runner.now_ms(),
            error,
        )
    }

    /// An empty batch, which has no geometry to hold: a refusal's.
    pub(crate) fn refused(&self, error: Option<String>) -> String {
        let mut batch = Batch::new();
        self.state_onto(&mut batch);
        let (timers, motion) = (self.runner.timer_due_ms(), !self.engine.quiescent());
        batch.finish(timers, motion, self.runner.now_ms(), error.as_deref())
    }

    pub(super) fn state_onto(&self, batch: &mut Batch) {
        batch.spatial = self.engine.spatial();
        batch.frames = self.runner.wants_frames();
        if self.runner.uses_native_timers() {
            batch.native_timers = Some(self.runner.native_timers());
        }
        if self.runner.plan().timers.iter().any(|t| t.wait != 0) {
            batch.waits = Some(self.runner.waits());
        }
        batch.seq = self.runner.seq_range();
        batch.alerts = self.runner.take_alerts();
        batch.journal = self.runner.take_journal();
        batch.canvas_frames(self.runner.canvas_wants_frame());
        self.canvas_owed_onto(batch);
    }

    fn withhold_layout(&mut self) {
        // These geometry rows will be withheld. Force a settled retry to
        // publish them even if the kernel's last guessed boxes compare equal.
        self.layout_withheld = true;
        for m in self.mirror.values_mut() {
            m.frame = None;
        }
        self.pending_layout.extend(
            self.mirror
                .keys()
                .filter_map(|id| self.runner.kernel().node(*id).map(|n| n.key)),
        );
    }

    pub(super) fn record_layout(&mut self, receipt: &exact_kernel::LayoutReceipt) {
        self.pending_layout
            .extend(receipt.updated.iter().chain(&receipt.flow_changed).copied());
    }

    pub(super) fn queue_layout(&mut self, id: ViewId) {
        if let Some(node) = self.runner.kernel().node(id) {
            if !node.is_inline_run() {
                self.pending_layout.insert(node.key);
            }
        }
    }

    // Logical ancestry can change an editor's inherited spelling hint without
    // moving it. Visit that affected subtree only on a topology/hint change.
    pub(super) fn queue_layout_subtree(&mut self, id: ViewId) {
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            if let Some(node) = self.runner.kernel().node(id) {
                if !node.is_inline_run() {
                    self.pending_layout.insert(node.key);
                }
                stack.extend(node.children());
            }
        }
    }
    /// Lay every root out under the viewport and emit the parent-relative
    /// frames and scroll content sizes that changed. One that fails is
    /// owed (`layout_owed`) until one succeeds.
    pub(super) fn layout(&mut self, batch: &mut Batch) -> Result<(), String> {
        let laid = self.layout_pass(batch);
        self.layout_owed = laid.is_err();
        laid
    }

    fn layout_pass(&mut self, batch: &mut Batch) -> Result<(), String> {
        if let Some((callback, ctx, installed)) = self.font_hook.as_mut() {
            if self.runner.runtime_font_count() != *installed {
                let runtime = self.runner.runtime_font_stacks();
                crate::measure::install_fonts(self.runner.plan(), &runtime, *callback, *ctx);
                *installed = runtime.len();
            }
        }
        #[cfg(test)]
        {
            self.layout_calls += 1;
        }
        self.sync_height_owner()?;
        self.sync_height_transitions()?;
        self.collect_height_samples()?;
        self.height_presented.clear();
        let epoch = self.runner.kernel().epoch();
        self.height_presented
            .extend(
                self.height_sampling
                    .iter()
                    .map(|(node, px)| exact_kernel::PresentedHeight {
                        node: *node,
                        px: *px,
                        epoch,
                    }),
            );
        let (w, h) = self.viewport;
        for root in self.runner.roots() {
            // A miss has filled its cache on main before returning. Re-run
            // silently, collecting both receipts so a final unchanged frame
            // still replaces the provisional geometry the presenter never saw.
            for attempt in 0..3 {
                let before = self.runner.kernel().provisional_layouts();
                if self.content_region.is_some() {
                    self.region_layout(root, Offer::definite(w, h), batch)?;
                } else {
                    self.runner.count_kernel_layout();
                    let receipt = self
                        .runner
                        .kernel_mut()
                        .compute_layout_presented(
                            root,
                            Offer::definite(w, h),
                            &self.height_presented,
                        )
                        .map_err(|e| format!("layout: {e:?}"))?;
                    self.runner.report_flow_skipped(&receipt.flow_skipped);
                    self.runner
                        .report_fragment_skipped(&receipt.fragment_skipped);
                    self.runner.moved(&receipt.changed);
                    self.record_layout(&receipt);
                }
                batch.layout_provisional = self.runner.kernel().provisional_layouts() != before;
                if !batch.layout_provisional {
                    break;
                }
                if attempt == 2 {
                    return Err("layout: field chrome remained provisional after three passes; batch refused before presentation".into());
                }
            }
        }
        self.height_projection.clear();
        self.height_projection
            .extend_from_slice(&self.height_sampling);
        self.emit_layout(batch)
    }

    /// @ref LLP 1083 D3 — each sticky node's constraint, when it changed.
    /// It follows its parent's and its scroller's boxes as well as its own,
    /// so every sticky node is read again after a layout; there are few.
    fn emit_sticky(&mut self, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        let keys = kernel.sticky_nodes();
        if keys.is_empty() && self.stickies.is_empty() {
            return;
        }
        let mut now = IdMap::default();
        for key in keys {
            let Some(node) = kernel.node_by_key(key) else {
                continue;
            };
            if let Some(c) = kernel.sticky_constraint(key) {
                if self.layout_withheld || self.stickies.get(&node.id) != Some(&c) {
                    batch.sticky(node.id, Some(&c));
                }
                now.insert(node.id, c);
            }
        }
        for id in self.stickies.keys() {
            if !now.contains_key(id) && kernel.node(*id).is_some() {
                batch.sticky(*id, None);
            }
        }
        self.stickies = now;
    }

    /// @ref LLP 1093 D7 — each box's fragments and each container's columns,
    /// when they changed: the presenter's copy of the kernel's record.
    fn emit_fragments(&mut self, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        let keys = kernel.fragmented();
        if keys.is_empty() && self.fragments.is_empty() {
            return;
        }
        let mut now = IdMap::default();
        for key in keys {
            let Some(node) = kernel.node_by_key(key) else {
                continue;
            };
            let record = crate::batch::fragments_json(kernel.fragments(key), kernel.columns(key));
            if self.layout_withheld || self.fragments.get(&node.id) != Some(&record) {
                batch.fragments(node.id, &record);
            }
            now.insert(node.id, record);
        }
        for id in self.fragments.keys() {
            if !now.contains_key(id) && kernel.node(*id).is_some() {
                batch.fragments(*id, "");
            }
        }
        self.fragments = now;
    }

    /// LLP 1083.000 D4: publish ranks independently of geometry, including zero.
    pub(super) fn emit_ranks(&mut self, batch: &mut Batch) {
        for (id, placed) in self.runner.kernel().paint_order() {
            if self.ranks.insert(id, placed.rank) != Some(placed.rank) {
                batch.rank(id, placed.rank);
            }
        }
    }

    /// The parent-relative frames and scroll content sizes that changed since
    /// the presenter last heard them.
    fn emit_layout(&mut self, batch: &mut Batch) -> Result<(), String> {
        self.judge_list_moves();
        // Preserve publication order without a tree insertion for each touch.
        let mut pending: Vec<_> = std::mem::take(&mut self.pending_layout)
            .into_iter()
            .collect();
        pending.sort_unstable();
        // What this pass laid out anew, and which of those moved, for the
        // fit-content routes above them (`emit_fitted`).
        let mut visited: Vec<ViewId> = Vec::new();
        let mut moved: IdSet<ViewId> = IdSet::default();
        for key in pending {
            let Some(node) = self.runner.kernel().node_by_key(key) else {
                continue;
            };
            let id = node.id;
            let native_protected = self.native_protected_id(id);
            let kernel = self.runner.kernel();
            let Some(node) = kernel.node(id) else {
                continue;
            };
            if node.is_inline_run() {
                continue;
            }
            let m = self.mirror.entry(id).or_default();
            // Silent list settling already updated kernel flow. Compare final
            // shapes with what the presenter saw, just like frames; destroyed
            // views drop this state with their mirror, and [] clears old ink.
            // Region-owned views still receive flow invalidation even when
            // their frames come from the selected native artifact below.
            if self.layout_withheld || *m.flow != *node.flow_shapes() {
                batch.flow(id, node.flow_shapes());
                m.flow = node.flow_shapes().into();
            }
            if native_protected {
                continue;
            }
            // A subtree a native container gave a containing block is laid
            // out in that block's space, not its parent's (LLP 1115.002
            // D8.2): its root's frame as published, sent again when it
            // takes or loses the block, with the block's used size.
            let block = kernel.host_block_size(id);
            let parent = node.parent.filter(|_| block.is_none());
            let parent = parent.and_then(|p| kernel.node(p)).map(|p| p.frame);
            let rel = relative(node.frame, parent);
            // A sheet sized to its route's content reads that extent too
            // (LLP 1075.003 §9.11). A route that scrolls itself keeps its
            // scroll extent; any other (one that only clips too) is measured
            // laid out alone (`emit_fitted`) whenever its subtree changes.
            let overflow = style::effective_overflow(&node);
            let fits = node
                .props
                .str(PropId::NavigationDetent)
                .is_some_and(|d| d.split(' ').any(|w| w == "fit-content"));
            let scrolls = [overflow.0, overflow.1]
                .iter()
                .any(|o| matches!(o, Overflow::Scroll | Overflow::Auto));
            if fits && !scrolls {
                if !self.fit_routes.iter().any(|(r, _)| *r == id) {
                    self.fit_routes.push((id, None));
                }
            } else {
                self.fit_routes.retain(|(r, _)| *r != id);
            }
            visited.push(id);
            let content = (!(fits && !scrolls)
                && overflow != (Overflow::Visible, Overflow::Visible))
                .then(|| content_size(&node, kernel));
            // An ancestor hint may change without touching the editor. Pass
            // its effective value through native containment, or clear it to
            // restore the platform default when the last declaration disappears.
            if node.node_type == NodeType::TextInput {
                let spelling = node.spellcheck().map(|value| value.to_string());
                if m.props.get("spellcheck") != spelling.as_ref() {
                    if let Some(value) = spelling {
                        batch.props(id, &[("spellcheck", value.clone())], &[]);
                        m.props.insert("spellcheck", value);
                    } else {
                        batch.props(id, &[], &["spellcheck"]);
                        m.props.remove("spellcheck");
                    }
                }
            }
            // A fitted paragraph's scale moves with its layout (LLP 1116.001 D1.4).
            let fit = fit_scale(&node);
            if m.props.get("fitScale") != fit.as_ref() {
                match fit {
                    Some(scale) => {
                        batch.props(id, &[("fitScale", scale.clone())], &[]);
                        m.props.insert("fitScale", scale);
                    }
                    None => {
                        batch.props(id, &[], &["fitScale"]);
                        m.props.remove("fitScale");
                    }
                }
            }
            self.emit_separators(id, batch);
            let kernel = self.runner.kernel();
            let Some(node) = kernel.node(id) else {
                continue;
            };
            let m = self.mirror.entry(id).or_default();
            // LLP 1131 D2: a scroller, or a box holding one, takes what its
            // neighbours leave, which the iOS host asks before drawing the
            // scroller under them: an item of a column flex container
            // growing by 1 or more with no `max-height`, which takes every
            // point of the free space (CSS Flexbox §9.7; a factor under 1
            // takes only that share).
            let column = node.parent.and_then(|p| kernel.node(p)).is_some_and(|p| {
                p.style.display == exact_kernel::Display::Flex
                    && matches!(
                        p.style.flex_direction,
                        exact_kernel::FlexDirection::Column
                            | exact_kernel::FlexDirection::ColumnReverse
                    )
            });
            let grows = (column
                && node.style.flex_grow >= 1.0
                && !node.style.mask.has(exact_kernel::StyleId::MaxHeight))
            .then(|| "true".to_string());
            if m.props.get("flexGrows") != grows.as_ref() {
                match grows {
                    Some(value) => {
                        batch.props(id, &[("flexGrows", value.clone())], &[]);
                        m.props.insert("flexGrows", value);
                    }
                    None => {
                        batch.props(id, &[], &["flexGrows"]);
                        m.props.remove("flexGrows");
                    }
                }
            }
            // Where a flex text's lines go down its content box (CSS's
            // anonymous flex item), which its paragraph is laid out from.
            let offset = kernel.text_line_offset(&node);
            let offset = (offset > 0.0).then(|| crate::style::num(offset));
            if m.props.get("textOffset") != offset.as_ref() {
                match offset {
                    Some(value) => {
                        batch.props(id, &[("textOffset", value.clone())], &[]);
                        m.props.insert("textOffset", value);
                    }
                    None => {
                        batch.props(id, &[], &["textOffset"]);
                        m.props.remove("textOffset");
                    }
                }
            }
            let field_content = published_content_box(&node, kernel);
            let field_cell = node.is_cell_field();
            if self.layout_withheld
                || m.field_content.as_deref() != field_content.as_ref()
                || m.field_cell != field_cell
            {
                m.field_content = field_content.map(Box::new);
                m.field_cell = field_cell;
                batch.field_content(id, field_content, field_cell);
            }
            let framed = m.frame != Some(rel);
            if framed || m.block.is_some() != block.is_some() {
                moved.insert(id);
                m.frame = Some(rel);
                batch.frame(id, rel.0, rel.1, rel.2, rel.3);
            }
            let implied = kernel.host_block_implied(id);
            if m.block != block || m.implied != implied || (framed && block.is_some()) {
                let had = m.block.is_some();
                (m.block, m.implied) = (block, implied);
                match block {
                    Some(size) => batch.block(id, size, (rel.0, rel.1)),
                    None if had => batch.unblock(id),
                    None => {}
                }
            }
            if let Some(c) = content {
                if self.layout_withheld || m.content != Some(c) {
                    m.content = Some(c);
                    batch.content(id, c.0, c.1);
                }
            }
            // @ref LLP 1063 — a moved box plays from where it was.
            if !self.presence.snap {
                self.observe_layout(key, batch);
            }
        }
        self.emit_fitted(&visited, &moved, batch);
        self.emit_extents(!visited.is_empty(), batch);
        self.snap_layout(batch);
        self.emit_sticky(batch);
        self.emit_fragments(batch);
        // A withheld batch held the scenes it placed: sent again
        if self.layout_withheld {
            self.svg.resend();
            self.svg.emit(self.runner.kernel(), &self.engine, batch);
        }
        self.layout_withheld = false;
        self.emit_ranks(batch);
        self.emit_collections(batch)
    }

    /// An advance whose commits changed no node ([`Host::commit_tree`]): no
    /// layout unless a held height moved, but what layout keeps besides
    /// boxes still follows the commits. A commit can change a list's data
    /// without a node (an item's field no mounted row shows, an item past
    /// the window): its metadata, which the presenter checks scroll and
    /// measurement feedback against, is published, and its generation is
    /// what the next layout judges row moves from (`judge_list_moves`, as
    /// layout would have; its boxes queue nothing here). A whole layout
    /// runs instead while one is owed (the last failed or did not run) or
    /// withheld, while boxes wait in `pending_layout`, for fonts installed
    /// since the last, and in a native content region, whose lists' next
    /// snapshot only layout prepares (`native_prepare_candidate`).
    pub(super) fn inert_layout(
        &mut self,
        batch: &mut Batch,
        committed: bool,
    ) -> Result<(), String> {
        let fonts = self
            .font_hook
            .as_ref()
            .is_some_and(|(_, _, installed)| self.runner.runtime_font_count() != *installed);
        let owed = self.layout_owed || self.layout_withheld || !self.pending_layout.is_empty();
        if owed || fonts || (committed && self.native_mode()) {
            return self.layout(batch);
        }
        if !self.height_layout_if_needed(batch)? && committed {
            self.judge_list_moves();
            self.emit_collections(batch)?;
        }
        Ok(())
    }

    /// The lists' metadata, when it moved. Layout and receipt work may change
    /// the live window; motion-only ticks and stale feedback never traverse
    /// the tree to collect it.
    fn emit_collections(&mut self, batch: &mut Batch) -> Result<(), String> {
        let collections = if self.native_mode() {
            self.collection_heads = None;
            self.native_collections_json()?
        } else {
            // Revisions cover rows, extent and corrections. A newer feedback
            // sequence alone needs no snapshot or serialization.
            let mut heads = self.runner.collections_shallow();
            for head in &mut heads {
                head.scroll_sequence = 0;
            }
            if self.collection_heads.as_ref() == Some(&heads) {
                return Ok(());
            }
            self.collection_heads = Some(heads);
            self.runner.collections_json()
        };
        if collections != self.collections_json {
            batch.collections(&collections);
            self.collections_json = collections;
        }
        Ok(())
    }
}

impl<D: DataSource> Host<D> {
    /// Each root that does not clip publishes its `document_extent` as its
    /// content, which the presenter's document scrolls: measured whenever
    /// anything was laid out anew, since a box deep below a root may have
    /// moved without the root.
    fn emit_extents(&mut self, laid_out: bool, batch: &mut Batch) {
        if !laid_out && !self.layout_withheld {
            return;
        }
        let kernel = self.runner.kernel();
        let extents: Vec<_> = kernel
            .roots()
            .into_iter()
            .filter(|id| !self.native_protected_id(*id))
            .filter_map(|id| kernel.node(id))
            .filter(|n| {
                n.props.str(PropId::SemanticTag) != Some("dialog")
                    && n.props.str(PropId::Popover).is_none()
                    && style::effective_overflow(n) == (Overflow::Visible, Overflow::Visible)
            })
            .map(|n| (n.id, document_extent(&n, kernel)))
            .collect();
        for (id, c) in extents {
            let m = self.mirror.entry(id).or_default();
            if self.layout_withheld || m.content != Some(c) {
                m.content = Some(c);
                batch.content(id, c.0, c.1);
            }
        }
    }

    /// A `fit-content` route's extent (LLP 1075.003 §9.11): the height it
    /// asks for laid out alone at its width, its own height left to its
    /// content (`Kernel::fit_content_height`), so the sheet it sizes never
    /// feeds back into it. A bottom cover is the sheet's safe area, which
    /// UIKit adds below the detent itself: not counted.
    ///
    /// Measured when a node under the route was laid out anew (a row added
    /// need not move the route's box or extent), or when its width,
    /// resolved padding, border or cover changed (a block route's
    /// percentage padding moves nothing it publishes); else the last
    /// measure stands. A scroller of a fixed length height that did not
    /// move holds what changes inside it, as it does in the trial.
    fn emit_fitted(&mut self, visited: &[ViewId], moved: &IdSet<ViewId>, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        self.fit_routes.retain(|(id, _)| kernel.node(*id).is_some());
        if self.fit_routes.is_empty() {
            return;
        }
        let mut refit: Vec<ViewId> = Vec::new();
        for &id in visited {
            let mut at = Some(id);
            while let Some(node) = at.and_then(|a| kernel.node(a)) {
                if self.fit_routes.iter().any(|(r, _)| *r == node.id) && !refit.contains(&node.id) {
                    refit.push(node.id);
                }
                let overflow = style::effective_overflow(&node);
                let scrolls = [overflow.0, overflow.1]
                    .iter()
                    .any(|o| matches!(o, Overflow::Scroll | Overflow::Auto));
                if scrolls
                    && matches!(node.style.height, exact_kernel::Dimension::Points(_))
                    && !moved.contains(&node.id)
                {
                    break;
                }
                at = node.parent;
            }
        }
        for i in 0..self.fit_routes.len() {
            let (id, last) = self.fit_routes[i];
            let kernel = self.runner.kernel_mut();
            let Some(node) = kernel.node(id) else {
                continue;
            };
            let key = node.key;
            // The cover's bottom as layout pads it: none where the route's
            // bottom-edge scroller takes it as UIKit's inset.
            let cover = kernel.cover_bottom(key);
            let (pl, pt, pr, pb) = kernel.layout_padding(key).unwrap_or_default();
            let (bl, bt, br, bb) = kernel.resolved_border(key).unwrap_or_default();
            let sig = [node.frame.width, pl, pt, pr, pb, bl, bt, br, bb, cover];
            if !self.layout_withheld && !refit.contains(&id) && last == Some(sig) {
                continue;
            }
            self.runner.count_kernel_layout();
            let Some(height) = self.runner.kernel_mut().fit_content_height(key) else {
                continue;
            };
            self.fit_routes[i].1 = Some(sig);
            let c = (sig[0], (height - cover).max(0.0));
            let m = self.mirror.entry(id).or_default();
            // The detent reads the height alone: a route that does not
            // scroll has no use for a new width.
            if self.layout_withheld || m.content.map(|c| c.1) != Some(c.1) {
                m.content = Some(c);
                batch.content(id, c.0, c.1);
            }
        }
    }
}

/// Publish native editor geometry, or the CSS content box of a box with
/// padding or a border and the padding layout resolved: what the presenter
/// places content and insets by, so a sheet or a failed layout holds it with
/// the frame (a style row is never held). A box with neither is its own
/// content box and sends none, unless it is a native button whose author
/// wrote a padding: which sides the author wrote is geometry too, held with
/// the values, as a written `0` and an unsaid side inset its title apart.
pub(super) fn published_content_box(
    node: &NodeRef<'_>,
    kernel: &Kernel,
) -> Option<crate::batch::ContentBox> {
    if let Some(rect) = node.field_content_rect() {
        return Some(crate::batch::ContentBox {
            rect,
            padding: None,
        });
    }
    let (x, y, width, height) = exact_kernel::svg::scene::content_box(kernel, node);
    let f = node.frame;
    let (left, top, right, bottom) = kernel.resolved_padding(node.key).unwrap_or_default();
    let button = exact_kernel::ControlKind::of(node.node_type, node.props)
        == Some(exact_kernel::ControlKind::Button);
    // A list's row inset is a side the host places the title by too (LLP 1138).
    let row = button && kernel.takes_row_inset(node.key);
    let side = |id: exact_kernel::StyleId, value: f32, inset: bool| {
        (!button || node.style.mask.has(id) || inset).then_some(value)
    };
    let padding = [
        side(exact_kernel::StyleId::PaddingTop, top, false),
        side(exact_kernel::StyleId::PaddingRight, right, row),
        side(exact_kernel::StyleId::PaddingBottom, bottom, false),
        side(exact_kernel::StyleId::PaddingLeft, left, row),
    ];
    let written = button && padding.iter().any(Option::is_some);
    (written || x != 0.0 || y != 0.0 || width != f.width || height != f.height).then_some(
        crate::batch::ContentBox {
            rect: Frame {
                x,
                y,
                width,
                height,
            },
            padding: Some(padding),
        },
    )
}

/// Natural scrollable overflow, including padding and descendants. The
/// presenter applies the CSS client-size minimum against its actual viewport;
/// flooring here loses the extent a native container needs under its own insets.
pub(super) fn content_size(node: &NodeRef<'_>, kernel: &Kernel) -> (f32, f32) {
    // Taffy's block containers do not always count end-edge padding in
    // `content_size` (its flex containers do); CSS's `scrollHeight` does.
    // Floor with the direct children's extent plus the end padding, as the
    // last layout resolved it (a percentage is of the containing block).
    let (_, _, pad_right, pad_bottom) = kernel.layout_padding(node.key).unwrap_or_default();
    // A scroller UIKit insets: its end padding holds those insets, which
    // UIKit adds back (LLP 1116.000 D1.2), as `node.content` leaves them out.
    let pad_bottom = match kernel.arena().cover(node.key.index) {
        Some(exact_kernel::HostCover::Inset([top, bottom])) => pad_bottom - top - bottom,
        _ => pad_bottom,
    };
    let (mut w, mut h) = node.content;
    for child in node.children() {
        // A child in a containing block of its own is in another space.
        let own = |c: &NodeRef<'_>| kernel.host_block(c.id).is_some();
        if let Some(c) = kernel.node(child).filter(|c| !own(c)) {
            // An absolutely positioned box counts by its border box alone:
            // the end padding follows in-flow content (CSS Overflow 3 §2.2).
            // A popover lays out absolute too (the kernel's top layer), unless sticky.
            let (right, bottom) = if c.layout_position() == exact_kernel::PositionType::Absolute {
                (0.0, 0.0)
            } else {
                (pad_right, pad_bottom)
            };
            w = w.max(c.frame.x - node.frame.x + c.frame.width + right);
            h = h.max(c.frame.y - node.frame.y + c.frame.height + bottom);
        }
    }
    (w, h)
}

/// What a root that does not clip adds to the document: its border box and
/// every laid-out box below it, down to a box that clips or scrolls (whose
/// own box counts, not what it holds), as a browser's viewport scrolls the
/// root element's overflow (CSS Overflow 3 §2.2). The root's end padding is
/// not added: the viewport is the scroll container, not the root. A box in
/// another containing block, the top layer, an `svg`'s inside and the head
/// are elsewhere. Boxes are where layout put them: a transform below the root
/// is not projected (the presenter projects the root's own).
pub(super) fn document_extent(root: &NodeRef<'_>, kernel: &Kernel) -> (f32, f32) {
    let (x0, y0) = (root.frame.x, root.frame.y);
    let (mut w, mut h) = (root.frame.width, root.frame.height);
    let mut below = root.children();
    while let Some(id) = below.pop() {
        let Some(c) = kernel.node(id) else { continue };
        let elsewhere = kernel.host_block(id).is_some()
            || c.node_type.is_svg_element()
            || c.node_type.is_metadata()
            || c.props.str(PropId::SemanticTag) == Some("dialog")
            || c.props.str(PropId::Popover).is_some();
        if elsewhere {
            continue;
        }
        // An empty box adds nothing, as Chromium's rectangle union skips one; what it holds may.
        if c.frame.width > 0.0 && c.frame.height > 0.0 {
            w = w.max(c.frame.x - x0 + c.frame.width);
            h = h.max(c.frame.y - y0 + c.frame.height);
        }
        if style::effective_overflow(&c) == (Overflow::Visible, Overflow::Visible) {
            below.extend(c.children());
        }
    }
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::{Op, PropId};
    use exact_runner::{DataError, Value};

    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
    }
    fn fixture() -> Host<NoData> {
        let plan = contract::compile(r#"component App
  view
    box testId="root"
      box testId="port" width=200 height=80 overflow-x="hidden" overflow-y="scroll" spellcheck="false"
        text "short" testId="text"
        input testId="field" value="abc"
      text "unrelated" testId="other"
"#).unwrap().encode();
        Host::boot(
            &plan,
            NoData,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            900.,
            700.,
        )
        .unwrap()
        .0
    }
    #[test]
    fn measured_layout_counts_actual_entries_and_stops_when_disabled() {
        let mut h = fixture();
        h.runner.measure(true, true);
        h.runner.measure_kernel_layouts();
        let read = |h: &Host<NoData>| {
            let reply = exact_runner::perf::reply(&h.runner, "{}");
            serde_json::from_str::<serde_json::Value>(&reply).unwrap()["work"]["counts"]
                ["kernelLayouts"]
                .as_u64()
                .unwrap()
        };
        let before = read(&h);
        h.layout(&mut Batch::new()).unwrap();
        assert_eq!(read(&h), before + h.runner.roots().len() as u64);
        let before_trial = read(&h);
        let root = h.runner.roots()[0];
        let _ = h.measure_block(
            root,
            exact_kernel::HostBlock {
                width: exact_kernel::BlockAxis::Is(300.0),
                height: exact_kernel::BlockAxis::AtMost(300.0),
            },
        );
        assert_eq!(
            read(&h),
            before_trial + 1,
            "trial layouts also enter the kernel"
        );
        h.runner.measure(false, false);
        h.layout(&mut Batch::new()).unwrap();
        assert!(exact_runner::perf::reply(&h.runner, "{}").contains("measures no work"));
    }

    fn id(host: &Host<NoData>, name: &str) -> ViewId {
        let k = host.runner.kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    }
    #[test]
    fn ranks_follow_paint_facts_without_geometry_and_clear_to_zero() {
        use exact_kernel::{StyleId, StyleProps, StyleValue};
        fn opacity(id: ViewId, value: f64) -> Op {
            let mut patch = StyleProps::default();
            patch
                .set_dynamic(StyleId::Opacity, &StyleValue::Number(value))
                .unwrap();
            Op::SetStyle {
                id,
                patch: Box::new(patch),
            }
        }
        let mut host = fixture();
        let root = id(&host, "root");
        let other = id(&host, "other");
        assert_eq!(host.ranks[&other], 0);
        let frame = host.mirror[&other].frame;
        let mut apply = |ops: &[Op]| {
            let receipt = host.runner.kernel_mut().apply(0, 99, ops).unwrap();
            host.commit(&[Timed { at_ms: 0., receipt }], None)
        };
        let raised = apply(&[opacity(other, 0.5)]);
        assert!(
            raised.contains(&format!("{{\"op\":\"rank\",\"id\":{other},\"rank\":1}}")),
            "{raised}"
        );
        let cleared = apply(&[opacity(other, 1.)]);
        assert!(
            cleared.contains(&format!("{{\"op\":\"rank\",\"id\":{other},\"rank\":0}}")),
            "{cleared}"
        );
        assert_eq!(host.mirror[&other].frame, frame);
        let mut unchanged = Batch::new();
        host.layout(&mut unchanged).unwrap();
        assert!(!unchanged
            .finish(None, false, 0., None)
            .contains("\"op\":\"rank\""));
        assert_eq!(host.ranks[&root], 1);
        let receipt = host
            .runner
            .kernel_mut()
            .apply(0, 100, &[Op::DestroyView { id: other }])
            .unwrap();
        host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert!(!host.ranks.contains_key(&other));
    }

    #[test]
    fn ranks_follow_props_children_and_new_roots() {
        let mut host = fixture();
        let root = id(&host, "root");
        let port = id(&host, "port");
        let other = id(&host, "other");
        let receipt = host
            .runner
            .kernel_mut()
            .apply(
                0,
                99,
                &[Op::SetProp {
                    id: other,
                    prop: PropId::BackgroundMaterial,
                    value: "thin".into(),
                }],
            )
            .unwrap();
        let batch = host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert!(batch.contains(&format!("{{\"op\":\"rank\",\"id\":{other},\"rank\":1}}")));
        let receipt = host
            .runner
            .kernel_mut()
            .apply(
                0,
                100,
                &[
                    Op::SetChildren {
                        id: root,
                        children: vec![port],
                    },
                    Op::AttachRoot { id: other },
                ],
            )
            .unwrap();
        host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert_eq!(host.ranks[&other], 1);
        // A plain box moved out of the tree then made a root changes rank
        // even though it has no authored style or geometry change.
        let receipt = host
            .runner
            .kernel_mut()
            .apply(
                0,
                101,
                &[
                    Op::CreateView {
                        id: 999,
                        node_type: NodeType::View,
                    },
                    Op::SetChildren {
                        id: root,
                        children: vec![port, 999],
                    },
                ],
            )
            .unwrap();
        host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert_eq!(host.ranks[&999], 0);
        let receipt = host
            .runner
            .kernel_mut()
            .apply(
                0,
                102,
                &[
                    Op::SetChildren {
                        id: root,
                        children: vec![port],
                    },
                    Op::AttachRoot { id: 999 },
                ],
            )
            .unwrap();
        let batch = host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert!(batch.contains("{\"op\":\"rank\",\"id\":999,\"rank\":1}"));
    }

    #[test]
    fn ancestor_spelling_change_reaches_an_unmoved_editor() {
        let mut host = fixture();
        let port = id(&host, "port");
        let field = id(&host, "field");
        let frame = host.mirror[&field].frame;
        assert_eq!(
            host.mirror[&field]
                .props
                .get("spellcheck")
                .map(String::as_str),
            Some("false")
        );
        host.runner
            .kernel_mut()
            .apply(
                0,
                2,
                &[Op::SetProp {
                    id: port,
                    prop: PropId::Spellcheck,
                    value: "true".into(),
                }],
            )
            .unwrap();
        let mut batch = Batch::new();
        host.update(port, &mut batch);
        host.layout(&mut batch).unwrap();
        assert_eq!(
            host.mirror[&field]
                .props
                .get("spellcheck")
                .map(String::as_str),
            Some("true")
        );
        assert_eq!(host.mirror[&field].frame, frame);
    }

    #[test]
    fn a_new_sibling_does_not_requeue_retained_subtrees_but_a_move_does() {
        let mut host = fixture();
        let root = id(&host, "root");
        let port = id(&host, "port");
        let field = id(&host, "field");
        let field_key = host.runner.kernel().node(field).unwrap().key;
        let added = 999;
        let mut children = host.runner.kernel().node(root).unwrap().children();
        children.push(added);
        host.runner
            .kernel_mut()
            .apply(
                0,
                2,
                &[
                    Op::CreateView {
                        id: added,
                        node_type: NodeType::View,
                    },
                    Op::SetProp {
                        id: added,
                        prop: PropId::Spellcheck,
                        value: "true".into(),
                    },
                    Op::SetChildren { id: root, children },
                ],
            )
            .unwrap();
        let mut batch = Batch::new();
        host.create(added, &[], &mut batch);
        host.emit_children(root, &mut batch);
        assert!(!host.pending_layout.contains(&field_key));
        host.layout(&mut batch).unwrap();

        host.runner
            .kernel_mut()
            .apply(
                0,
                3,
                &[Op::SetChildren {
                    id: added,
                    children: vec![field],
                }],
            )
            .unwrap();
        host.emit_children(port, &mut batch);
        host.emit_children(added, &mut batch);
        assert!(host.pending_layout.contains(&field_key));
        host.layout(&mut batch).unwrap();
        assert_eq!(
            host.mirror[&field]
                .props
                .get("spellcheck")
                .map(String::as_str),
            Some("true")
        );
    }

    /// LLP 1116: a padding that resolves only at layout reaches the
    /// presenter as the kernel's content box, which a host reading the
    /// authored rows could not compute: on a bare field, which places its
    /// editor there, and on a box, whose paragraph or image goes there. A
    /// resize that moves no frame still republishes it. A padding in points
    /// is published too, the one source a sheet or a failed layout holds; a
    /// box with no padding or border is its own content box and sends none.
    #[test]
    fn padding_resolved_at_layout_publishes_the_content_box() {
        use exact_kernel::{StyleId, StyleProps, StyleValue};
        let plan = contract::compile(
            r#"component App
  view
    box testId="holder"
      box testId="sized" box-sizing="border-box" width=100 height=50 padding-left="10%"
      input testId="bare" appearance="none" value="abc" padding-top="min(10%, 20px)" border-width=2 border-style="solid"
      box testId="pct" padding-top="10%"
        text "inside"
      box testId="points" padding=5
"#,
        )
        .unwrap()
        .encode();
        let (mut host, boot) = Host::boot(
            &plan,
            NoData,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.,
            700.,
        )
        .unwrap();
        let rects = |batch: &str| -> std::collections::HashMap<u64, serde_json::Value> {
            let json: serde_json::Value = serde_json::from_str(batch).unwrap();
            json["ops"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|op| op["op"] == "fieldContent")
                .map(|op| (op["id"].as_u64().unwrap(), op["rect"].clone()))
                .collect()
        };
        let [bare, pct, sized, points, holder] =
            ["bare", "pct", "sized", "points", "holder"].map(|name| u64::from(id(&host, name)));
        let first = rects(&boot);
        assert_eq!(first[&bare][1], 22.0, "border 2 + min(40, 20): {boot}");
        assert_eq!(first[&pct][1], 40.0, "10% of 400: {boot}");
        assert_eq!(first[&sized][0], 40.0, "10% of 400: {boot}");
        assert_eq!(first[&points][0], 5.0, "{boot}");
        assert!(!first.contains_key(&holder), "no padding, no box: {boot}");

        let sized_frame = host.mirror[&(sized as ViewId)].frame;
        let resized = host.resize(300., 700.);
        let second = rects(&resized);
        assert_eq!(host.mirror[&(sized as ViewId)].frame, sized_frame);
        assert_eq!(
            second[&sized][0], 30.0,
            "unmoved, still republished: {resized}"
        );
        assert_eq!(second[&pct][1], 30.0, "{resized}");
        assert_eq!(
            host.mirror[&(bare as ViewId)]
                .field_content
                .as_deref()
                .map(|c| c.rect.y),
            Some(22.0),
            "min(30, 20) is still 20"
        );

        let mut patch = StyleProps::default();
        patch
            .set_dynamic(StyleId::PaddingTop, &StyleValue::Number(30.))
            .unwrap();
        let receipt = host
            .runner
            .kernel_mut()
            .apply(
                0,
                99,
                &[Op::SetStyle {
                    id: pct as ViewId,
                    patch: Box::new(patch),
                }],
            )
            .unwrap();
        let cleared = host.commit(&[Timed { at_ms: 0., receipt }], None);
        assert!(
            !rects(&cleared).contains_key(&pct),
            "30 points is the box 10% of 300 made: {cleared}"
        );
    }

    /// A list growing in a column has the `flexGrows` its layout gives it,
    /// and a commit that changes its rows leaves it: emitting the list's
    /// props kept it, and no batch clears it to set it again.
    #[test]
    fn a_rows_change_leaves_a_growing_lists_derived_props() {
        let plan = contract::compile(
            r#"component App
  state n = 2
  action more
    n = n + 1
  view
    column testId="root" display="flex" flex-direction="column" height=400
      button testId="more" press=more
        text "More"
      list testId="list" flex=1 min-height=0
        for i in range(n) key=i
          text `${i}`
"#,
        )
        .unwrap()
        .encode();
        let (mut h, boot) = Host::boot(
            &plan,
            NoData,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            400.,
            400.,
        )
        .unwrap();
        let list = id(&h, "list");
        let grows = |batch: &str| {
            let v: serde_json::Value = serde_json::from_str(batch).unwrap();
            (v["ops"].as_array().cloned().unwrap_or_default().into_iter())
                .filter(|op| op["id"] == list && op["op"] == "props")
                .map(|op| (op["set"]["flexGrows"].clone(), op["clear"].to_string()))
                .collect::<Vec<_>>()
        };
        assert!(boot.contains("\"flexGrows\":\"true\""), "{boot}");
        let more = id(&h, "more");
        let batch = h.dispatch_at(more, exact_runner::Event::Press, h.engine().now() * 1000.);
        assert!(batch.contains("\"op\":\"children\""), "{batch}");
        assert!(
            grows(&batch)
                .iter()
                .all(|(set, clear)| set.is_null() && !clear.contains("flexGrows")),
            "{:?}",
            grows(&batch)
        );
    }
}
