use super::{tree::Derived, *};
use crate::{
    arena::NodeArena,
    generated::{Display, NodeType, Overflow, StyleMask, StyleProps},
    style::Dimension,
    CommitReceipt, LayoutError, NodeFlags, TextMeasurer,
};
use std::collections::HashSet;

pub(crate) struct RegionState {
    pub binding: ContentRegion,
    members: HashSet<NodeKey>,
    inherited: StyleProps,
    ticket: Option<RegionTicket>,
    inputs: Option<RegionInputs>,
    shell_catalog: Option<u64>,
    offer: Option<Offer>,
    pub pending: Option<RegionTextRequest>,
    ready: Vec<RegionArtifact>,
    sources: Vec<Arc<RegionTextSource>>,
    accepted: Option<Rc<RegionPublication>>,
}
impl RegionState {
    pub fn new(arena: &NodeArena, binding: ContentRegion) -> Result<Self, LayoutError> {
        validate(arena, binding)?;
        Ok(Self {
            binding,
            members: members(arena, binding.content)?,
            inherited: arena.computed_style(binding.content.index, StyleMask::INHERITED),
            ticket: None,
            inputs: None,
            shell_catalog: None,
            offer: None,
            pending: None,
            ready: Vec::new(),
            sources: Vec::new(),
            accepted: None,
        })
    }
    pub fn retention(&self) -> RegionRetention {
        let mut accepted = HashSet::new();
        let accepted_source_bytes = self.accepted.as_ref().map_or(0, |p| {
            p.artifacts
                .iter()
                .filter_map(|a| {
                    let source = a.request.source();
                    accepted.insert(Arc::as_ptr(source)).then_some(source.bytes)
                })
                .sum()
        });
        let candidate_source_bytes = self.sources.iter().map(|s| s.bytes).sum();
        let shared_source_bytes = self
            .sources
            .iter()
            .filter(|s| accepted.contains(&Arc::as_ptr(s)))
            .map(|s| s.bytes)
            .sum();
        RegionRetention {
            accepted_source_bytes,
            candidate_source_bytes,
            shared_source_bytes,
            total_source_bytes: accepted_source_bytes + candidate_source_bytes
                - shared_source_bytes,
            accepted_offers: self.accepted.as_ref().map_or(0, |p| p.artifacts.len()),
            candidate_offers: self.ready.len() + usize::from(self.pending.is_some()),
        }
    }
    pub fn invalidate(&mut self) {
        self.ticket = None;
        self.pending = None;
        self.ready.clear();
        self.sources.clear();
    }
    pub fn observe(&mut self, arena: &NodeArena, r: &CommitReceipt) -> bool {
        if arena.resolve(self.binding.owner).is_none() {
            return false;
        }
        if validate(arena, self.binding).is_err() {
            self.invalidate();
            return true;
        }
        let inherited = arena.computed_style(self.binding.content.index, StyleMask::INHERITED);
        let changed = r.touched.contains(&self.binding.owner)
            || r.destroyed.iter().any(|k| self.members.contains(k))
            || r.created.iter().chain(&r.touched).any(|k| {
                self.members.contains(k)
                    || arena.resolve(*k).is_some_and(|s| {
                        s == self.binding.content.index
                            || arena.is_ancestor(self.binding.content.index, s)
                    })
            })
            || self.inherited != inherited;
        if changed {
            self.invalidate();
            self.inherited = inherited;
        }
        // Bounded mounted membership, not visited source history. A later
        // over-limit candidate is explicitly refused before construction.
        if let Ok(m) = members(arena, self.binding.content) {
            self.members = m;
        }
        true
    }
    pub fn intrinsic(&mut self, slot: u32) {
        if self.members.iter().any(|k| k.index == slot) {
            self.invalidate()
        }
    }
    pub fn resolve(
        &mut self,
        request: &RegionTextRequest,
        metrics: TextMetrics,
        payload: Rc<dyn Any>,
    ) -> Result<bool, LayoutError> {
        if !self
            .pending
            .as_ref()
            .is_some_and(|p| Arc::ptr_eq(&p.0, &request.0))
        {
            return Ok(false);
        }
        if !metrics.is_valid() {
            return Err(LayoutError::InvalidTextMetrics(
                request.stamp().owner().index,
            ));
        }
        self.ready.push(RegionArtifact {
            request: request.clone(),
            metrics,
            payload,
        });
        self.pending = None;
        Ok(true)
    }
    pub fn compute(
        &mut self,
        arena: &mut NodeArena,
        tree: &mut crate::layout::LayoutTree,
        measurer: &mut dyn TextMeasurer,
        root_offer: (u32, Offer),
        inputs: RegionInputs,
        epoch: u64,
    ) -> Result<RegionLayoutReceipt, LayoutError> {
        let (root, outer) = root_offer;
        validate(arena, self.binding)?;
        if root != self.binding.owner.index && !arena.is_ancestor(root, self.binding.owner.index) {
            return Err(LayoutError::ContentRegion("region belongs to another root"));
        }
        for (dimension, axis) in [
            (arena.style(self.binding.owner.index).width, outer.width),
            (arena.style(self.binding.owner.index).height, outer.height),
        ] {
            if matches!(dimension, Dimension::Percent(_))
                && !matches!(axis,crate::AxisOffer::Definite(n) if n>=0.)
            {
                return Err(LayoutError::ContentRegion(
                    "percent region requires definite outer offer",
                ));
            }
        }
        let b = self.binding;
        // The catalog also identifies ordinary shell text. Invalidate before
        // computing it, including the first registered pass over a previously
        // measured ordinary tree. Keep handles and unchanged-catalog caches.
        if self.shell_catalog != Some(inputs.catalog) {
            for slot in arena.iter_live() {
                if arena.node_type(slot).is_measured_leaf() {
                    if let Some(node) = arena.taffy(slot) {
                        tree.mark_dirty(node);
                    }
                }
            }
            self.shell_catalog = Some(inputs.catalog);
        }
        // Keep the ordinary shell cache and its arena handle map. Only this
        // owner's derived child edge is cut; publication uses the same cut.
        let shell_frames = super::tree::shell(arena, tree, measurer, root, b.owner.index, outer)?;
        let origin = shell_frames
            .iter()
            .find(|f| f.node == b.owner)
            .unwrap()
            .frame;
        let offer = Offer::definite(origin.width, origin.height);
        if self.inputs.is_some_and(|old| old.catalog != inputs.catalog) || self.offer != Some(offer)
        {
            self.invalidate()
        }
        if self.ticket.is_none() {
            self.members = members(arena, b.content)?;
            self.ticket = Some(RegionTicket(Arc::new(())));
            self.inputs = Some(inputs);
            self.offer = Some(offer);
        }
        // Collection feedback epochs can advance on unrelated typing while
        // paragraph/source inputs stay identical. Preserve source work, but a
        // new consumer revision still requires a fresh UI geometry publication.
        self.inputs = Some(inputs);
        let ticket = self.ticket.as_ref().unwrap().clone();
        let already_current = self
            .accepted
            .as_ref()
            .is_some_and(|p| p.ticket == ticket && p.inputs == inputs);
        let mut next_accepted = None;
        if !already_current && self.pending.is_none() {
            let mut candidate = Derived::build(
                arena,
                b.owner.index,
                Some(b.owner.index),
                Some(b.content.index),
                true,
            )?;
            candidate.constrain_owner(arena, b.owner.index, origin);
            let mut latch = Candidate {
                ticket: ticket.clone(),
                ready: &mut self.ready,
                accepted: self.accepted.as_deref(),
                catalog: inputs.catalog,
                sources: &mut self.sources,
                missing: None,
                refused: false,
            };
            let result = candidate.compute(arena, &mut latch, offer);
            // No candidate survives this scope. Pending zeros and *all*
            // descendant engine caches disappear together, even on success.
            result?;
            // Layout offers are insufficient proof for a painter. Pin each
            // paragraph at its final inner width too. Still only the first miss
            // escapes; every poisoned tree is dropped, including this one.
            let mut paints = Vec::new();
            if latch.missing.is_none() && !latch.refused {
                for (slot, width) in candidate.paint_offers(arena) {
                    let mut runs = Vec::new();
                    arena.text_runs(slot, &mut runs);
                    if runs.is_empty() {
                        continue;
                    }
                    let stamp = arena
                        .paragraph_stamp(slot)
                        .ok_or(LayoutError::ContentRegion("paragraph lacks stamp"))?;
                    let request = TextMeasureRequest {
                        runs: &runs,
                        paragraph: Paragraph::from_style(
                            &arena.computed_style(slot, StyleMask::INHERITED),
                        ),
                        width: crate::AxisOffer::Definite(width),
                        height: crate::AxisOffer::MaxContent,
                    };
                    latch.measure_identified(&stamp, &request);
                    if latch.missing.is_some() || latch.refused {
                        break;
                    }
                    let key = TextKey {
                        stamp,
                        offer: Offer {
                            width: request.width,
                            height: request.height,
                        },
                    };
                    let index = latch
                        .ready
                        .iter()
                        .position(|a| a.request.0.key == key)
                        .unwrap();
                    paints.push((arena.key(slot), index));
                }
            }
            if latch.refused {
                return Err(LayoutError::ContentRegion(
                    "exact-offer/source budget exhausted",
                ));
            }
            if let Some(request) = latch.missing {
                self.pending = Some(request);
            } else {
                let mut frames = candidate.frames(
                    arena,
                    b.owner.index,
                    Some(b.owner.index),
                    Some(b.content.index),
                )?;
                frames.retain(|f| f.node != b.owner);
                next_accepted = Some(Rc::new(RegionPublication {
                    ticket: ticket.clone(),
                    inputs,
                    frames,
                    artifacts: self.ready.clone(),
                    paints,
                }));
            }
        }
        let selected = next_accepted.as_ref().or(self.accepted.as_ref());
        let current = selected.is_some_and(|p| p.ticket == ticket && p.inputs == inputs);
        let pending_frames = if selected.is_none() {
            let mut pending = Derived::build(
                arena,
                b.owner.index,
                Some(b.owner.index),
                Some(b.pending.index),
                true,
            )?;
            pending.constrain_owner(arena, b.owner.index, origin);
            pending.compute(arena, measurer, offer)?;
            let mut frames = pending.frames(
                arena,
                b.owner.index,
                Some(b.owner.index),
                Some(b.pending.index),
            )?;
            frames.retain(|f| f.node != b.owner);
            frames
        } else {
            Vec::new()
        };
        // Validate the projection too: finite local coordinates plus a finite
        // origin can still overflow. No arena publication or accepted swap until
        // *all* selected frames, placeholder metrics and candidate work succeed.
        let frames = selected
            .map(|p| p.frames.as_slice())
            .unwrap_or(&pending_frames);
        for f in frames {
            if !(origin.x + f.frame.x).is_finite() || !(origin.y + f.frame.y).is_finite() {
                return Err(LayoutError::ContentRegion("projection overflow"));
            }
        }
        let selection = match selected {
            Some(p) => RegionSelection::Accepted(p.clone()),
            None => RegionSelection::Pending(b.pending),
        };
        let mut changed = Vec::new();
        publish(arena, &shell_frames, Frame::default(), true, &mut changed);
        publish(
            arena,
            frames,
            origin,
            current || selected.is_none(),
            &mut changed,
        );
        if let Some(accepted) = next_accepted {
            self.accepted = Some(accepted);
            self.ready.clear();
            self.sources.clear();
        }
        Ok(RegionLayoutReceipt {
            shell: LayoutReceipt {
                epoch,
                root: arena.key(root),
                changed,
            },
            origin,
            selection,
            current,
        })
    }
}
struct Candidate<'a> {
    ticket: RegionTicket,
    ready: &'a mut Vec<RegionArtifact>,
    accepted: Option<&'a RegionPublication>,
    catalog: u64,
    sources: &'a mut Vec<Arc<RegionTextSource>>,
    missing: Option<RegionTextRequest>,
    refused: bool,
}
impl TextMeasurer for Candidate<'_> {
    fn measure(&mut self, _: &TextMeasureRequest<'_>) -> TextMetrics {
        self.refused = true;
        TextMetrics::default()
    }
    fn measure_identified(
        &mut self,
        stamp: &ParagraphStamp,
        r: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        if self.missing.is_some() || self.refused {
            return TextMetrics::default();
        }
        let key = TextKey {
            stamp: stamp.clone(),
            offer: Offer {
                width: r.width,
                height: r.height,
            },
        };
        if let Some(a) = self.ready.iter().find(|a| a.request.0.key == key) {
            return a.metrics;
        }
        if self.ready.len() == REGION_OFFERS {
            self.refused = true;
            return TextMetrics::default();
        }
        let source = if let Some(source) = self.sources.iter().find(|s| s.stamp == *stamp) {
            source.clone()
        } else {
            let bytes = r
                .runs
                .iter()
                .try_fold(0usize, |n, r| n.checked_add(r.text.len()));
            let retained: usize = self.sources.iter().map(|s| s.bytes).sum();
            if bytes.is_none_or(|n| n > REGION_SOURCE_BYTES.saturating_sub(retained)) {
                self.refused = true;
                return TextMetrics::default();
            }
            // One allocation for a source revision, even while old and new
            // width publications coexist. Canonical metric inputs do not carry
            // a font catalog; only ready answers require equal catalogs.
            let source = self
                .accepted
                .and_then(|p| p.artifacts.iter().find(|a| a.request.stamp() == stamp))
                .map(|a| a.request.source().clone())
                .unwrap_or_else(|| {
                    Arc::new(RegionTextSource {
                        stamp: stamp.clone(),
                        paragraph: r.paragraph,
                        runs: r
                            .runs
                            .iter()
                            .map(|r| (Box::<str>::from(r.text), r.style))
                            .collect(),
                        bytes: bytes.unwrap(),
                    })
                });
            self.sources.push(source.clone());
            source
        };
        let request = RegionTextRequest(Arc::new(RequestData {
            ticket: self.ticket.clone(),
            key,
            catalog: self.catalog,
            source,
        }));
        if let Some(a) = self
            .accepted
            .filter(|p| p.inputs.catalog == self.catalog)
            .and_then(|p| {
                p.artifacts
                    .iter()
                    .find(|a| a.request.0.key == request.0.key)
            })
        {
            let metrics = a.metrics;
            // Import only offers actually requested by the new UI pass, never
            // the entire previous cache or a history of visited widths.
            self.ready.push(RegionArtifact {
                request,
                metrics,
                payload: a.payload.clone(),
            });
            return metrics;
        }
        self.missing = Some(request);
        // Private unpublished containment value. This is never stored as Ready.
        TextMetrics::default()
    }
}
fn members(arena: &NodeArena, key: NodeKey) -> Result<HashSet<NodeKey>, LayoutError> {
    let mut result = HashSet::new();
    let mut stack = vec![key.index];
    while let Some(s) = stack.pop() {
        if result.len() == REGION_NODES {
            return Err(LayoutError::ContentRegion("mounted node limit"));
        }
        result.insert(arena.key(s));
        stack.extend(arena.children(s));
    }
    Ok(result)
}
fn validate(arena: &NodeArena, b: ContentRegion) -> Result<(), LayoutError> {
    let bad = || {
        LayoutError::ContentRegion(
            "requires one attached independent clipped owner and two direct branches",
        )
    };
    for k in [b.owner, b.content, b.pending] {
        if arena.resolve(k).is_none() {
            return Err(bad());
        }
    }
    if b.content == b.pending
        || arena.node_type(b.owner.index) != NodeType::View
        || arena.is_inline_run(b.owner.index)
        || arena.children(b.owner.index).len() != 2
        || arena.parent(b.content.index) != Some(b.owner.index)
        || arena.parent(b.pending.index) != Some(b.owner.index)
    {
        return Err(bad());
    }
    let mut p = Some(b.owner.index);
    let mut attached = false;
    while let Some(s) = p {
        if arena.style(s).display == Display::None {
            return Err(bad());
        }
        // Fixed dimensions do not fix an exported child baseline: cutting the
        // child tree can replace its first baseline with the owner's height.
        // That baseline can propagate through intermediate ancestors. This
        // Flex and Grid both consume it. Refuse participation on the path,
        // even when a
        // particular row currently has too few baseline items to move.
        if let Some(parent) = arena.parent(s) {
            let child = arena.style(s);
            let parent = arena.style(parent);
            if matches!(parent.display, Display::Flex | Display::Grid)
                && (child.align_self == crate::AlignSelf::Baseline
                    || (child.align_self == crate::AlignSelf::Auto
                        && parent.align_items == crate::AlignItems::Baseline))
            {
                return Err(LayoutError::ContentRegion(
                    "baseline-dependent region shell is unsupported",
                ));
            }
        }
        attached |= arena.is_root(s);
        p = arena.parent(s);
    }
    if !attached {
        return Err(bad());
    }
    let s = arena.style(b.owner.index);
    let sized = |v: Dimension| {
        matches!(v,Dimension::Points(x) if x>=0.) || matches!(v,Dimension::Percent(x) if x>=0.)
    };
    // First trial deliberately requires authored sizes on both axes. Flexible
    // outer sizing/intrinsic containment needs a separate eligibility proof.
    if !sized(s.width)
        || !sized(s.height)
        || s.overflow_x != Overflow::Hidden
        || s.overflow_y != Overflow::Hidden
    {
        return Err(bad());
    }
    // Deliberately narrow certificate: percentages only under a direct root.
    // No inference through auto/intrinsic ancestors or generalized containment.
    // Width:auto is allowed only for the ordinary nonabsolute root repair;
    // height:auto is never an independent percentage containing block.
    for (dimension, width) in [(s.width, true), (s.height, false)] {
        if matches!(dimension, Dimension::Percent(_)) {
            let parent = arena.parent(b.owner.index).ok_or_else(bad)?;
            if !arena.is_root(parent) {
                return Err(bad());
            }
            let root = arena.style(parent);
            let dim = if width { root.width } else { root.height };
            let definite = sized(dim)
                || (width
                    && dim == Dimension::Auto
                    && root.position_type != crate::PositionType::Absolute);
            if !definite {
                return Err(bad());
            }
        }
    }
    let style = crate::style::taffy_style(arena, b.owner.index);
    let zero = |x: taffy::style::LengthPercentage| {
        x == taffy::style::LengthPercentage::length(0.)
            || x == taffy::style::LengthPercentage::percent(0.)
    };
    if ![
        style.border.left,
        style.border.right,
        style.border.top,
        style.border.bottom,
        style.padding.left,
        style.padding.right,
        style.padding.top,
        style.padding.bottom,
    ]
    .into_iter()
    .all(zero)
    {
        return Err(bad());
    }
    members(arena, b.content)?;
    members(arena, b.pending)?;
    Ok(())
}
fn publish(
    arena: &mut NodeArena,
    frames: &[RegionFrame],
    origin: Frame,
    current: bool,
    changed: &mut Vec<NodeKey>,
) {
    for f in frames {
        let Some(s) = arena.resolve(f.node) else {
            continue;
        };
        let frame = if arena.is_inline_run(s) {
            Frame::default()
        } else {
            Frame {
                x: origin.x + f.frame.x,
                y: origin.y + f.frame.y,
                ..f.frame
            }
        };
        let moved = !arena.frame(s).bits_eq(frame) || arena.flags(s).has(NodeFlags::CREATED);
        arena.set_frame(s, frame);
        arena.set_content(s, f.content);
        let flags = arena.flags_mut(s);
        if current {
            for clear in [
                NodeFlags::CREATED,
                NodeFlags::STYLE_DIRTY,
                NodeFlags::TEXT_DIRTY,
                NodeFlags::CHILDREN_DIRTY,
            ] {
                flags.remove(clear)
            }
        }
        if moved {
            flags.insert(NodeFlags::GEOMETRY_CHANGED);
            changed.push(f.node)
        } else {
            flags.remove(NodeFlags::GEOMETRY_CHANGED)
        }
    }
}
