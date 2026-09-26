use super::{tree::Derived, *};
use crate::{
    arena::NodeArena,
    generated::{Display, NodeType, Overflow, StyleMask, StyleProps},
    id::IdSet,
    style::Dimension,
    CommitReceipt, LayoutError, NodeFlags, TextMeasurer,
};

struct Provisional {
    geometry: Rc<RegionGeometry>,
    // Default: ready artifact index. Split: scalar fact index until final adoption.
    paints: Vec<(NodeKey, usize)>,
}
pub(crate) struct RegionState {
    pub binding: ContentRegion,
    pub profile: RegionProfile,
    leases: RegionLeases,
    lease: Option<Arc<()>>,
    members: IdSet<NodeKey>,
    inherited: StyleProps,
    ticket: Option<RegionTicket>,
    inputs: Option<RegionInputs>,
    shell_catalog: Option<u64>,
    offer: Option<Offer>,
    pub pending: Option<RegionTextRequest>,
    ready: Vec<RegionArtifact>,
    facts: Arc<FactSet>,
    provisional: Option<Provisional>,
    accepted: Option<Rc<RegionPublication>>,
}
impl RegionState {
    pub fn new(
        arena: &NodeArena,
        binding: ContentRegion,
        profile: RegionProfile,
        leases: RegionLeases,
    ) -> Result<Self, LayoutError> {
        validate(arena, binding)?;
        Ok(Self {
            binding,
            profile,
            leases,
            lease: None,
            members: members(arena, binding.content)?,
            inherited: arena.computed_style(binding.content.index, StyleMask::INHERITED),
            ticket: None,
            inputs: None,
            shell_catalog: None,
            offer: None,
            pending: None,
            ready: Vec::new(),
            facts: Arc::new(FactSet::default()),
            provisional: None,
            accepted: None,
        })
    }
    pub fn retention(&self) -> RegionRetention {
        let mut accepted = IdSet::default();
        let accepted_source_bytes = self.accepted.as_ref().map_or(0, |p| {
            p.facts
                .sources
                .iter()
                .filter_map(|s| accepted.insert(Arc::as_ptr(s)).then_some(s.bytes))
                .sum()
        });
        let candidate_source_bytes = self.facts.sources.iter().map(|s| s.bytes).sum();
        let shared_source_bytes = self
            .facts
            .sources
            .iter()
            .filter(|s| accepted.contains(&Arc::as_ptr(s)))
            .map(|s| s.bytes)
            .sum();
        let candidate_offers = self.ready.len()
            + usize::from(
                self.pending
                    .as_ref()
                    .is_some_and(|p| p.purpose() != RegionRequestPurpose::Measurement),
            );
        let accepted_offers = self.accepted.as_ref().map_or(0, |p| p.artifacts.len());
        RegionRetention {
            accepted_source_bytes,
            candidate_source_bytes,
            shared_source_bytes,
            total_source_bytes: accepted_source_bytes + candidate_source_bytes
                - shared_source_bytes,
            accepted_offers,
            candidate_offers,
            accepted_facts: self.accepted.as_ref().map_or(0, |p| {
                if self.profile == RegionProfile::SplitFacts {
                    p.facts.entries.len()
                } else {
                    p.artifacts.len()
                }
            }),
            candidate_facts: if self.profile == RegionProfile::SplitFacts {
                self.facts.entries.len()
                    + usize::from(
                        self.pending
                            .as_ref()
                            .is_some_and(|p| p.purpose() == RegionRequestPurpose::Measurement),
                    )
            } else {
                candidate_offers
            },
        }
    }
    pub fn invalidate(&mut self) {
        self.ticket = None;
        self.clear_candidate();
    }
    fn clear_candidate(&mut self) {
        self.pending = None;
        self.ready.clear();
        self.facts = Arc::new(FactSet::default());
        self.provisional = None;
        self.lease = None;
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
        // Purpose and reservation belong to this private Arc, never inferred from tuple equality.
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
        match request.purpose() {
            RegionRequestPurpose::Measurement => {
                let source = self
                    .facts
                    .sources
                    .iter()
                    .position(|s| Arc::ptr_eq(s, request.source()))
                    .expect("pending captured source");
                Arc::get_mut(&mut self.facts)
                    .expect("candidate facts are private")
                    .push(ScalarFact {
                        source: source as u16,
                        offer: request.offer(),
                        metrics,
                    });
                // No native owner or ready artifact is retained by a scalar fact.
                drop(payload);
            }
            RegionRequestPurpose::FinalPaint => {
                let fact = self
                    .facts
                    .find(request.stamp(), request.offer())
                    .expect("final request has an exact fact");
                if !same_metrics(self.facts.entries[fact].metrics, metrics) {
                    return Err(LayoutError::ContentRegion(
                        "final paint metrics differ from exact fact",
                    ));
                }
                if self.ready.capacity() == 0 {
                    self.ready.reserve_exact(SPLIT_PAINTS);
                }
                self.ready.push(RegionArtifact {
                    request: request.clone(),
                    metrics,
                    payload,
                });
            }
            RegionRequestPurpose::RetainedOffer => {
                self.ready.push(RegionArtifact {
                    request: request.clone(),
                    metrics,
                    payload,
                });
            }
        }
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
        // Preflight Auto flex axes before any shell mutation, as in the default path.
        if arena.style(self.binding.owner.index).height == Dimension::Auto
            && ![outer.width, outer.height].into_iter().all(
                |axis| matches!(axis, crate::AxisOffer::Definite(n) if n.is_finite() && n >= 0.),
            )
        {
            return Err(LayoutError::ContentRegion(
                "flex region requires definite outer axes",
            ));
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
        // One common ordinary-shell path, including reservation saturation.
        let shell_frames = super::tree::shell(arena, tree, measurer, root, b.owner.index, outer)?;
        let origin = shell_frames
            .iter()
            .find(|f| f.node == b.owner)
            .unwrap()
            .frame;
        let offer = Offer::definite(origin.width, origin.height);
        if self.inputs.is_some_and(|old| old.catalog != inputs.catalog)
            || self.offer != Some(offer)
            || (self.profile == RegionProfile::SplitFacts
                && self
                    .inputs
                    .is_some_and(|old| old.consumer_revision != inputs.consumer_revision))
        {
            self.invalidate()
        }
        self.inputs = Some(inputs);
        self.offer = Some(offer);
        let already_current = self.accepted.as_ref().is_some_and(|p| {
            self.ticket
                .as_ref()
                .is_some_and(|ticket| p.ticket == *ticket)
                && p.inputs == inputs
        });
        let admitted = if !already_current && self.profile == RegionProfile::SplitFacts {
            if self.lease.is_none() {
                self.lease = self.leases.reserve();
            }
            self.lease.is_some()
        } else {
            true
        };
        if admitted && self.ticket.is_none() {
            self.members = members(arena, b.content)?;
            self.ticket = Some(RegionTicket(Arc::new(())));
        }
        // Default keeps the original consumer-revision republish semantics.
        // Split waits nonfatally when external A+B still own both tokens: no C
        // facts/request/geometry, but shell and retained selection still publish.
        let next_accepted = if admitted && !already_current && self.pending.is_none() {
            self.advance(arena, origin, offer, inputs)?
        } else {
            None
        };
        let selected = next_accepted.as_ref().or(self.accepted.as_ref());
        let current = selected.is_some_and(|p| {
            self.ticket
                .as_ref()
                .is_some_and(|ticket| p.ticket == *ticket)
                && p.inputs == inputs
        });
        let pending_geometry = if selected.is_none() {
            let mut pending = Derived::build(
                arena,
                b.owner.index,
                Some(b.owner.index),
                Some(b.pending.index),
                true,
            )?;
            pending.constrain_owner(arena, b.owner.index, origin);
            pending.compute(arena, measurer, offer)?;
            pending.frames(
                arena,
                b.owner.index,
                Some(b.owner.index),
                Some(b.pending.index),
            )?
        } else {
            RegionGeometry::default()
        };
        // Both policies share exactly one projection/validation/publication barrier.
        let frames = selected
            .map(|p| p.geometry.as_ref())
            .unwrap_or(&pending_geometry)
            .project(origin)?;
        let selection = match selected {
            Some(p) => RegionSelection::Accepted(p.clone()),
            None => RegionSelection::Pending(b.pending),
        };
        let mut changed = Vec::new();
        let updated = shell_frames
            .iter()
            .chain(frames.iter())
            .map(|f| f.node)
            .collect();
        arena.begin_layout_publication(root);
        publish(arena, root, &shell_frames, true, &mut changed);
        publish(
            arena,
            root,
            &frames,
            current || selected.is_none(),
            &mut changed,
        );
        // @ref LLP 1043.000 §3 D4 — resolve only after the selected projection.
        let (flow_changed, flow_skipped) =
            crate::flow::resolve_region(arena, root, &shell_frames, &frames);
        if let Some(accepted) = next_accepted {
            self.accepted = Some(accepted);
            self.clear_candidate();
        }
        Ok(RegionLayoutReceipt {
            shell: LayoutReceipt {
                epoch,
                root: arena.key(root),
                changed,
                updated,
                flow_changed,
                flow_skipped,
            },
            origin,
            selection,
            current,
        })
    }
    fn advance(
        &mut self,
        arena: &NodeArena,
        origin: Frame,
        offer: Offer,
        inputs: RegionInputs,
    ) -> Result<Option<Rc<RegionPublication>>, LayoutError> {
        let b = self.binding;
        let ticket = self.ticket.as_ref().expect("admitted ticket").clone();
        if self.provisional.is_none() {
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
                profile: self.profile,
                lease: self.lease.clone(),
                ready: &mut self.ready,
                facts: Arc::get_mut(&mut self.facts).expect("unpublished candidate facts"),
                accepted: self.accepted.as_deref(),
                catalog: inputs.catalog,
                missing: None,
                refused: None,
            };
            candidate.compute(arena, &mut latch, offer)?;
            let mut paints = Vec::new();
            if latch.missing.is_none() && latch.refused.is_none() {
                if self.profile == RegionProfile::SplitFacts {
                    paints.reserve_exact(SPLIT_PAINTS);
                }
                for (slot, width) in candidate.paint_offers(arena) {
                    let mut runs = Vec::new();
                    arena.text_runs(slot, &mut runs);
                    if runs.is_empty() {
                        continue;
                    }
                    let stamp = arena
                        .paragraph_stamp(slot)
                        .ok_or(LayoutError::ContentRegion("paragraph lacks stamp"))?;
                    let mut paragraph =
                        Paragraph::from_style(&arena.computed_style(slot, StyleMask::INHERITED));
                    paragraph.markup = arena.markup(slot);
                    let request = TextMeasureRequest {
                        exclusions: &[],
                        runs: &runs,
                        paragraph,
                        width: crate::AxisOffer::Definite(width),
                        height: crate::AxisOffer::MaxContent,
                    };
                    latch.measure_identified(&stamp, &request);
                    if latch.missing.is_some() || latch.refused.is_some() {
                        break;
                    }
                    if self.profile == RegionProfile::SplitFacts && paints.len() == SPLIT_PAINTS {
                        return Err(LayoutError::ContentRegion(
                            "split final owner budget exhausted",
                        ));
                    }
                    let key = TextKey {
                        stamp,
                        offer: Offer {
                            width: request.width,
                            height: request.height,
                        },
                    };
                    let index = latch.index(&key).expect("measured final offer");
                    paints.push((arena.key(slot), index));
                }
            }
            if let Some(reason) = latch.refused {
                return Err(LayoutError::ContentRegion(reason));
            }
            if let Some(request) = latch.missing {
                self.pending = Some(request);
                return Ok(None);
            }
            // Only immutable geometry/ordinals survive. All Taffy state drops here.
            self.provisional = Some(Provisional {
                geometry: Rc::new(candidate.frames(
                    arena,
                    b.owner.index,
                    Some(b.owner.index),
                    Some(b.content.index),
                )?),
                paints,
            });
        }
        let provisional = self.provisional.as_ref().unwrap();
        let paints = if self.profile == RegionProfile::SplitFacts {
            if let Some((_, index)) = provisional.paints.get(self.ready.len()) {
                let fact = self.facts.entries[*index];
                let source = self.facts.sources[fact.source as usize].clone();
                self.pending = Some(RegionTextRequest(Arc::new(RequestData {
                    ticket,
                    key: TextKey {
                        stamp: source.stamp.clone(),
                        offer: fact.offer,
                    },
                    catalog: inputs.catalog,
                    source,
                    purpose: RegionRequestPurpose::FinalPaint,
                    _lease: self.lease.clone(),
                })));
                return Ok(None);
            }
            provisional
                .paints
                .iter()
                .enumerate()
                .map(|(i, (key, _))| (*key, i))
                .collect()
        } else {
            provisional.paints.clone()
        };
        Ok(Some(Rc::new(RegionPublication {
            ticket,
            inputs,
            geometry: provisional.geometry.clone(),
            facts: self.facts.clone(),
            _lease: self.lease.clone(),
            artifacts: self.ready.clone(),
            paints,
        })))
    }
}
struct Candidate<'a> {
    ticket: RegionTicket,
    profile: RegionProfile,
    lease: Option<Arc<()>>,
    ready: &'a mut Vec<RegionArtifact>,
    facts: &'a mut FactSet,
    accepted: Option<&'a RegionPublication>,
    catalog: u64,
    missing: Option<RegionTextRequest>,
    refused: Option<&'static str>,
}
impl Candidate<'_> {
    fn index(&self, key: &TextKey) -> Option<usize> {
        if self.profile == RegionProfile::SplitFacts {
            self.facts.find(&key.stamp, key.offer)
        } else {
            self.ready.iter().position(|a| a.request.0.key == *key)
        }
    }
}
impl TextMeasurer for Candidate<'_> {
    fn measure(&mut self, _: &TextMeasureRequest<'_>) -> TextMetrics {
        self.refused = Some("exact-offer/source budget exhausted");
        TextMetrics::default()
    }
    fn measure_identified(
        &mut self,
        stamp: &ParagraphStamp,
        r: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        if self.missing.is_some() || self.refused.is_some() {
            return TextMetrics::default();
        }
        let key = TextKey {
            stamp: stamp.clone(),
            offer: Offer {
                width: r.width,
                height: r.height,
            },
        };
        let split = self.profile == RegionProfile::SplitFacts;
        if let Some(index) = self.index(&key) {
            return if split {
                self.facts.entries[index].metrics
            } else {
                self.ready[index].metrics
            };
        }
        if (split && self.facts.entries.len() == SPLIT_FACTS)
            || (!split && self.ready.len() == REGION_OFFERS)
        {
            self.refused = Some(if split {
                "split scalar fact budget exhausted"
            } else {
                "exact-offer/source budget exhausted"
            });
            return TextMetrics::default();
        }
        let source_index =
            if let Some(i) = self.facts.sources.iter().position(|s| s.stamp == *stamp) {
                i
            } else {
                if split && self.facts.sources.len() == SPLIT_PAINTS {
                    self.refused = Some("split canonical source budget exhausted");
                    return TextMetrics::default();
                }
                let bytes = r
                    .runs
                    .iter()
                    .try_fold(0usize, |n, r| n.checked_add(r.text.len()));
                let retained: usize = self.facts.sources.iter().map(|s| s.bytes).sum();
                if bytes.is_none_or(|n| n > REGION_SOURCE_BYTES.saturating_sub(retained)) {
                    self.refused = Some("exact-offer/source budget exhausted");
                    return TextMetrics::default();
                }
                let source = self
                    .accepted
                    .and_then(|p| p.facts.sources.iter().find(|s| s.stamp == *stamp))
                    .cloned()
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
                if split && self.facts.sources.capacity() == 0 {
                    self.facts.sources.reserve_exact(SPLIT_PAINTS);
                }
                self.facts.sources.push(source);
                self.facts.sources.len() - 1
            };
        let source = self.facts.sources[source_index].clone();
        if let Some(metrics) = self
            .accepted
            .filter(|p| split && p.inputs.catalog == self.catalog)
            .and_then(|p| {
                p.facts
                    .find(stamp, key.offer)
                    .map(|i| p.facts.entries[i].metrics)
            })
        {
            self.facts.push(ScalarFact {
                source: source_index as u16,
                offer: key.offer,
                metrics,
            });
            return metrics;
        }
        let request = RegionTextRequest(Arc::new(RequestData {
            ticket: self.ticket.clone(),
            key,
            catalog: self.catalog,
            source,
            purpose: if split {
                RegionRequestPurpose::Measurement
            } else {
                RegionRequestPurpose::RetainedOffer
            },
            _lease: self.lease.clone(),
        }));
        if let Some(a) = self
            .accepted
            .filter(|p| !split && p.inputs.catalog == self.catalog)
            .and_then(|p| {
                p.artifacts
                    .iter()
                    .find(|a| a.request.0.key == request.0.key)
            })
        {
            let metrics = a.metrics;
            self.ready.push(RegionArtifact {
                request,
                metrics,
                payload: a.payload.clone(),
            });
            return metrics;
        }
        self.missing = Some(request);
        TextMetrics::default()
    }
}
fn members(arena: &NodeArena, key: NodeKey) -> Result<IdSet<NodeKey>, LayoutError> {
    let mut result = IdSet::default();
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
    // Keep the explicit-size path unchanged. Auto height has only the narrow
    // direct-root column certificate below, never generic intrinsic sizing.
    if !sized(s.width)
        || !(sized(s.height) || flex_height_independent(arena, b.owner.index))
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
    if !s.unpadded(arena.env()) {
        return Err(bad());
    }
    members(arena, b.content)?;
    members(arena, b.pending)?;
    Ok(())
}

/// Only a clipped, zero-basis flex item in a definite, unwrapped root column.
/// Its own contribution to main-axis sizing is numeric, not a descendant's
/// min-content size; explicit cross-axis width cannot request intrinsic width.
/// The ordinary baseline, branch, visibility and zero border/padding checks
/// remain in validate. No new derived tree or retained measurement is needed.
fn flex_height_independent(arena: &NodeArena, owner: u32) -> bool {
    let Some(parent) = arena.parent(owner) else {
        return false;
    };
    if !arena.is_root(parent) {
        return false;
    }
    let root = arena.style(parent);
    let s = arena.style(owner);
    let definite = |d| {
        matches!(d, Dimension::Points(n) | Dimension::Percent(n)
        if n.is_finite() && n >= 0.)
    };
    let zero = |d| matches!(d, Dimension::Points(n) | Dimension::Percent(n) if n == 0.);
    let limit = |d| d == Dimension::Auto || definite(d);
    root.display == Display::Flex
        && root.flex_direction == crate::FlexDirection::Column
        && root.flex_wrap == crate::FlexWrap::Nowrap
        && definite(root.width)
        && definite(root.height)
        && s.height == Dimension::Auto
        && definite(s.width)
        && s.position_type == crate::PositionType::Relative
        && s.flex_grow == 1.
        && s.flex_shrink == 1.
        && zero(s.flex_basis)
        && definite(s.min_height)
        && limit(s.max_height)
        // With clipping and a definite cross size, Auto here is zero, not an
        // automatic main-axis content minimum. Aspect transfer is not admitted.
        && limit(s.min_width)
        && limit(s.max_width)
        && s.aspect_ratio.preferred().is_none()
        && [s.margin_top, s.margin_right, s.margin_bottom, s.margin_left]
            .into_iter().all(zero)
        && [s.top, s.right, s.bottom, s.left]
            .into_iter().all(|d| d == Dimension::Auto || zero(d))
}
fn publish(
    arena: &mut NodeArena,
    root: u32,
    frames: &[RegionFrame],
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
            f.frame
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
            arena.mark_geometry_changed(s, root);
            changed.push(f.node)
        } else {
            flags.remove(NodeFlags::GEOMETRY_CHANGED)
        }
    }
}
