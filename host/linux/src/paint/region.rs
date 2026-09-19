//! Flat drawing commands for ONE accepted content publication. This is neither
//! a mutable view tree nor another layout graph. It pins exact paragraphs,
//! palettes, source/link artifacts and charged image owners across candidate
//! changes, including destroyed keys. Shell origin/clip and scroll remain live.
use super::*;
use exact_kernel::{NodeKey, RegionPublication};

const COMMANDS: usize = exact_kernel::region::REGION_NODES * 12;
/// Numeric interaction geometry owned by the same picture as the pixels.
/// Collection limits include the logical end, even when its row is unmounted.
#[derive(Clone, Copy)]
pub(crate) struct ScrollBounds {
    pub axes: (Overflow, Overflow),
    pub max: (f32, f32),
}
impl ScrollBounds {
    fn capture(node: &NodeRef<'_>, kernel: &Kernel, collection_max: Option<f32>) -> Self {
        let (width, height) = content_size(node, kernel);
        Self {
            axes: effective_overflow(node),
            max: (
                (width - node.frame.width).max(0.),
                collection_max.unwrap_or_else(|| (height - node.frame.height).max(0.)),
            ),
        }
    }
    pub(crate) fn clamp(self, offset: (f32, f32)) -> (f32, f32) {
        (
            offset.0.clamp(0., self.max.0),
            offset.1.clamp(0., self.max.1),
        )
    }
}
// Flat enter/leave order, not a second mutable tree. Geometry is never baked
// into a relative coordinate that would later be translated after rasterization.
enum Command {
    Enter(usize),
    Leave(usize),
}
enum Payload {
    Empty,
    Image(Arc<Bitmap>, ObjectFit),
    Text(Rc<Paragraph>, Vec<RunPaint>),
}
struct NodePaint {
    ordinal: usize,
    key: NodeKey,
    id: ViewId,
    paint: BoxPaint,
    payload: Payload,
    opacity: f32,
    clips: bool,
    scroll: Option<(f32, f32)>,
}
pub(super) struct Picture {
    publication: Rc<RegionPublication>,
    commands: Vec<Command>,
    nodes: Vec<NodePaint>,
    scroll: BTreeMap<NodeKey, ScrollBounds>,
    dark: bool,
    scale: u32,
    incarnation: Rc<()>,
    owner: ViewId,
}
impl Picture {
    pub(super) fn belongs_to(&self, region: &crate::content_region::ContentRegionState) -> bool {
        Rc::ptr_eq(&self.incarnation, region.incarnation())
    }
    pub(super) fn publication(&self) -> &Rc<RegionPublication> {
        &self.publication
    }
    pub(super) fn matches(&self, publication: &Rc<RegionPublication>) -> bool {
        Rc::ptr_eq(&self.publication, publication)
    }
    pub(super) fn capture(
        painter: &Painter,
        scene: &Scene<'_>,
        region: &crate::content_region::ContentRegionState,
        publication: &Rc<RegionPublication>,
        collection_limits: &BTreeMap<ViewId, f32>,
    ) -> Result<Rc<Self>, String> {
        if let Some(p) = &painter.region_picture {
            if p.matches(publication)
                && p.dark == painter.dark
                && p.scale == painter.scale.to_bits()
            {
                return Ok(p.clone());
            }
        }
        if publication.frames().len() > exact_kernel::region::REGION_NODES {
            return Err("content mounted node limit".into());
        }
        let mut owners = BTreeMap::new();
        let mut scroll = BTreeMap::new();
        let mut ordinals = BTreeMap::new();
        for (ordinal, f) in publication.frames().iter().enumerate() {
            ordinals.insert(f.node, ordinal);
            let node = scene
                .kernel
                .node_by_key(f.node)
                .ok_or("content node retired before capture")?;
            if (scene.presented)(node.id).moves() {
                return Err("content capture refuses internally transformed presentation".into());
            }
            if node.node_type == NodeType::TextInput {
                return Err(
                    "content capture refuses input controls inside retained content".into(),
                );
            }
            let axes = effective_overflow(&node);
            if axes.0 == Overflow::Scroll || axes.1 == Overflow::Scroll {
                scroll.insert(
                    node.key,
                    ScrollBounds::capture(
                        &node,
                        scene.kernel,
                        collection_limits.get(&node.id).copied(),
                    ),
                );
            }
            if node.node_type != NodeType::Text || node.is_inline_run() {
                continue;
            }
            let artifact = publication
                .paint_artifact(f.node)
                .ok_or("content text lacks final paint offer")?;
            let native = artifact
                .payload::<crate::content_region::NativeText>()
                .ok_or("content text lacks native owner")?;
            if native.paint_context().scale().to_bits() != painter.scale.to_bits() {
                return Err("content native text raster context mismatch".into());
            }
            if node.paragraph_stamp().as_ref() != Some(artifact.request().stamp())
                || !native.matches(artifact.request())
            {
                return Err("content paint/source revision mismatch".into());
            }
            let paragraph = native
                .paragraph()
                .ok_or("intrinsic answer cannot paint final content")?;
            owners.insert(
                node.key,
                (paragraph.clone(), native.palette(painter.dark).to_vec()),
            );
        }
        enum Visit {
            Enter(ViewId),
            Leave(usize),
        }
        let content = scene
            .kernel
            .node_by_key(region.binding().content)
            .ok_or("content root removed")?;
        let mut stack = vec![Visit::Enter(content.id)];
        let mut commands = Vec::new();
        let mut nodes = Vec::new();
        let mut command_cost = 0usize;
        while let Some(visit) = stack.pop() {
            let id = match visit {
                Visit::Leave(i) => {
                    commands.push(Command::Leave(i));
                    continue;
                }
                Visit::Enter(id) => id,
            };
            let node = scene
                .kernel
                .node(id)
                .ok_or("content node removed during capture")?;
            if (scene.hidden)(id)
                || node.is_inline_run()
                || node.style.display == Display::None
                || node.props.str(PropId::SemanticTag) == Some("dialog")
            {
                continue;
            }
            let ordinal = *ordinals
                .get(&node.key)
                .ok_or("content node outside publication")?;
            if nodes.len() == exact_kernel::region::REGION_NODES {
                return Err("content mounted node limit".into());
            }
            let frame = publication.frames()[ordinal].frame;
            let paint = BoxPaint::capture(&node, scene.kernel, painter.dark, frame.width);
            let geometry = paint.geometry(paint_rect(frame, (0., 0.)));
            let axes = effective_overflow(&node);
            let clips = axes.0 != Overflow::Visible || axes.1 != Overflow::Visible;
            let scroll_offset = scroll
                .contains_key(&node.key)
                .then(|| scene.scroll.get(&id).copied().unwrap_or_default());
            let opacity = (scene.presented)(id).opacity.clamp(0., 1.);
            if !opacity.is_finite() {
                return Err("nonfinite content opacity".into());
            }
            let payload = if opacity <= 0. {
                Payload::Empty
            } else {
                match node.node_type {
                    NodeType::Text => {
                        let (p, palette) = owners
                            .get(&node.key)
                            .ok_or("content paragraph owner absent")?;
                        Payload::Text(p.clone(), palette.clone())
                    }
                    NodeType::Image => scene.images.get(&id).map_or(Payload::Empty, |image| {
                        Payload::Image(image.clone(), node.style.object_fit)
                    }),
                    _ => Payload::Empty,
                }
            };
            // Preserve the previous expanded-command budget, even though enter/
            // leave records are smaller. Counting uses the SAME paint emitter.
            let mut cost = 1usize; // hit, including opacity-zero nodes
            if opacity > 0. {
                paint.emit(&geometry, |_, _, _| cost += 1);
                cost += match &payload {
                    Payload::Empty => 0,
                    Payload::Text(..) => 1,
                    Payload::Image(image, fit) => {
                        usize::from(object_fit(image.natural(), *fit, geometry.content).is_some())
                    }
                };
                cost += 2 * usize::from(opacity < 1.)
                    + 2 * usize::from(clips)
                    + 2 * usize::from(scroll_offset.is_some());
            }
            command_cost = command_cost
                .checked_add(cost)
                .ok_or("content command cost overflow")?;
            if command_cost > COMMANDS {
                return Err("content flat paint command limit".into());
            }
            let i = nodes.len();
            nodes.push(NodePaint {
                ordinal,
                key: node.key,
                id,
                paint,
                payload,
                opacity,
                clips,
                scroll: scroll_offset,
            });
            commands.push(Command::Enter(i));
            stack.push(Visit::Leave(i));
            if opacity > 0. {
                stack.extend(node.children().into_iter().rev().map(Visit::Enter));
            }
        }
        Ok(Rc::new(Self {
            publication: publication.clone(),
            commands,
            nodes,
            scroll,
            dark: painter.dark,
            scale: painter.scale.to_bits(),
            incarnation: region.incarnation().clone(),
            owner: scene
                .kernel
                .node_by_key(region.binding().owner)
                .ok_or("content owner removed")?
                .id,
        }))
    }
}
pub(super) struct Published {
    pub incarnation: Rc<()>,
    pub selection: Option<(Rc<RegionPublication>, exact_kernel::Frame)>,
}
impl Painter {
    pub(crate) fn region_scroll_bounds(
        &self,
        region: &crate::content_region::ContentRegionState,
        key: NodeKey,
    ) -> Option<ScrollBounds> {
        self.region_picture
            .as_ref()
            .filter(|p| p.belongs_to(region))?
            .scroll
            .get(&key)
            .copied()
    }
    pub(crate) fn scroll_bounds(
        &self,
        kernel: &Kernel,
        region: Option<&crate::content_region::ContentRegionState>,
        node: &NodeRef<'_>,
        collection_max: Option<f32>,
    ) -> ScrollBounds {
        if let Some(region) = region {
            if let Some(bounds) = self.region_scroll_bounds(region, node.key) {
                return bounds;
            }
            if region.contains(kernel, node.id) {
                // No exact painted key: do not borrow a ready candidate's
                // geometry, including a recycled slot or a new scroll owner.
                return ScrollBounds {
                    axes: (Overflow::Hidden, Overflow::Hidden),
                    max: (0., 0.),
                };
            }
        }
        ScrollBounds::capture(node, kernel, collection_max)
    }
    pub(super) fn validate_region_presentation(
        &self,
        scene: &Scene<'_>,
        region: &crate::content_region::ContentRegionState,
    ) -> Result<(), String> {
        // The first read-only trial supports shell placement and scroll offsets,
        // not animated affine transforms of the region/its containing blocks.
        let mut at = Some(
            scene
                .kernel
                .node_by_key(region.binding().owner)
                .ok_or("content owner removed")?,
        );
        while let Some(node) = at {
            if (scene.presented)(node.id).moves() {
                return Err("content-region trial refuses transformed containing blocks".into());
            }
            at = node.parent.and_then(|id| scene.kernel.node(id));
        }
        Ok(())
    }
    pub(crate) fn published_region(
        &self,
        region: &crate::content_region::ContentRegionState,
    ) -> Result<Option<(Rc<RegionPublication>, exact_kernel::Frame)>, String> {
        let frame = self
            .region_frame
            .as_ref()
            .filter(|f| Rc::ptr_eq(&f.incarnation, region.incarnation()))
            .ok_or("region has no successful native frame")?;
        Ok(frame.selection.clone())
    }
    /// This first region consumer is read-only. Deny action dispatch using the
    /// last successful picture's IDs even when live source/handlers changed or
    /// a failed frame kept old boxes. Scroll routing remains separate.
    pub(crate) fn region_blocks_action(&self, view: ViewId) -> bool {
        self.region_picture
            .as_ref()
            .is_some_and(|p| p.owner == view || p.nodes.iter().any(|n| n.id == view))
    }
}
fn finite_rect(rect: Rect4) -> bool {
    [
        rect.0,
        rect.1,
        rect.2,
        rect.3,
        rect.0 + rect.2,
        rect.1 + rect.3,
    ]
    .into_iter()
    .all(f32::is_finite)
}
struct Resolved {
    geometry: BoxGeometry,
    scroll: Option<(f32, f32)>,
    live_hit: bool,
}
pub(super) struct Replay<'a> {
    pub picture: &'a Picture,
    pub content: NodeKey,
    nodes: Vec<Resolved>,
}
impl<'a> Replay<'a> {
    pub(super) fn prepare(
        picture: &'a Picture,
        scene: &Scene<'_>,
        origin: exact_kernel::Frame,
        content: NodeKey,
        viewport: (f32, f32),
        scale: f32,
    ) -> Result<Self, String> {
        let frames = picture
            .publication
            .projected_frames(origin)
            .map_err(|e| format!("content projection: {e:?}"))?;
        // Ordinary Painter starts at page offset, then adds ancestor scrolls in
        // root-first order. Parent transforms were refused before this call.
        let mut ancestors = Vec::new();
        let mut at = scene
            .kernel
            .node_by_key(content)
            .ok_or("content root removed")?
            .parent;
        while let Some(id) = at {
            let n = scene.kernel.node(id).ok_or("content ancestor removed")?;
            ancestors.push(id);
            at = n.parent;
        }
        let mut offset = scene.page;
        for id in ancestors.into_iter().rev() {
            let n = scene.kernel.node(id).ok_or("content ancestor removed")?;
            let axes = effective_overflow(&n);
            if axes.0 == Overflow::Scroll || axes.1 == Overflow::Scroll {
                let scroll = scene.scroll.get(&id).copied().unwrap_or_default();
                offset = (offset.0 + scroll.0, offset.1 + scroll.1);
            }
        }
        let mut stack = Vec::new();
        let mut nodes = Vec::with_capacity(picture.nodes.len());
        let query_transform = Transform::from_scale(scale, scale).pre_scale(1. / scale, 1. / scale);
        let device_clip = (
            0.,
            0.,
            (viewport.0 * scale).round().max(1.),
            (viewport.1 * scale).round().max(1.),
        );
        for command in &picture.commands {
            match *command {
                Command::Enter(i) => {
                    let n = &picture.nodes[i];
                    let f = &frames[n.ordinal];
                    if f.node != n.key || i != nodes.len() {
                        return Err("content recipe identity mismatch".into());
                    }
                    let geometry = n.paint.geometry(paint_rect(f.frame, offset));
                    let rect = geometry.outer.rect;
                    let c = geometry.content;
                    let mut finite = finite_rect(rect)
                        && finite_rect(c)
                        && n.paint
                            .padding
                            .into_iter()
                            .chain(n.paint.widths)
                            .chain(n.paint.radii)
                            .all(f32::is_finite);
                    n.paint.emit(&geometry, |shape, _, stroke| {
                        finite &= finite_rect(shape.rect)
                            && shape.radii.into_iter().all(f32::is_finite)
                            && stroke.is_none_or(f32::is_finite);
                    });
                    if let Payload::Image(image, fit) = &n.payload {
                        finite &= object_fit(image.natural(), *fit, c).is_none_or(finite_rect);
                    }
                    if !finite {
                        return Err("nonfinite content paint geometry".into());
                    }
                    if let Payload::Text(p, _) = &n.payload {
                        if !p.prepared_ink_supports((c.0, c.1), scale, query_transform, device_clip)
                        {
                            return Err("content-region prepared ink query refused".into());
                        }
                    }
                    let live_hit = scene.kernel.node_by_key(n.key).is_some_and(|node| {
                        picture.publication.paint_artifact(n.key).is_none_or(|a| {
                            node.paragraph_stamp().as_ref() == Some(a.request().stamp())
                        })
                    });
                    let scroll = n.scroll.map(|old| {
                        let current = scene
                            .kernel
                            .node_by_key(n.key)
                            .map(|node| scene.scroll.get(&node.id).copied().unwrap_or_default())
                            .unwrap_or(old);
                        picture.scroll[&n.key].clamp(current)
                    });
                    if scroll.is_some_and(|s| !s.0.is_finite() || !s.1.is_finite()) {
                        return Err("nonfinite content scroll".into());
                    }
                    nodes.push(Resolved {
                        geometry,
                        scroll,
                        live_hit,
                    });
                    stack.push(offset);
                    if n.opacity > 0. {
                        if let Some(s) = scroll {
                            offset = (offset.0 + s.0, offset.1 + s.1);
                        }
                    }
                }
                Command::Leave(_) => {
                    offset = stack.pop().ok_or("content recipe scope mismatch")?;
                }
            }
        }
        if !stack.is_empty() {
            return Err("content recipe scope mismatch".into());
        }
        Ok(Self {
            picture,
            content,
            nodes,
        })
    }
    pub(super) fn paint(
        &self,
        painter: &mut Painter,
        walk: &mut Walk<'_, '_>,
        parent: Transform,
        _offset: (f32, f32),
        outer_clip: Option<Rect4>,
    ) {
        let mut clip = outer_clip;
        let mut clips = Vec::new();
        for command in &self.picture.commands {
            match *command {
                Command::Enter(i) => {
                    let n = &self.picture.nodes[i];
                    let r = &self.nodes[i];
                    let g = &r.geometry;
                    if r.live_hit {
                        walk.boxes.push(PaintedBox {
                            id: n.id,
                            rect: bbox(parent, g.outer.rect),
                            clip,
                            scroll: r.scroll,
                        });
                    }
                    if n.opacity <= 0. {
                        continue;
                    }
                    if n.opacity < 1. {
                        painter.backend.push_opacity(n.opacity);
                    }
                    n.paint.paint(painter.backend.as_mut(), g, parent);
                    match &n.payload {
                        Payload::Empty => {}
                        Payload::Image(image, fit) => {
                            if let Some(dst) = object_fit(image.natural(), *fit, g.content) {
                                painter.backend.image(
                                    image,
                                    dst,
                                    &[Shape::rect(g.content), g.outer],
                                    parent,
                                );
                            }
                        }
                        Payload::Text(p, palette) => {
                            walk.text.insert(n.key, p.clone());
                            painter.backend.text(
                                &mut painter.text.borrow_mut(),
                                p,
                                palette,
                                (g.content.0, g.content.1),
                                parent,
                            );
                        }
                    }
                    if n.clips {
                        painter.backend.push_clip(&g.outer, parent);
                        clips.push(clip);
                        let own = bbox(parent, g.outer.rect);
                        clip = Some(clip.map_or(own, |c| intersect(c, own)));
                    }
                }
                Command::Leave(i) => {
                    let n = &self.picture.nodes[i];
                    if n.opacity <= 0. {
                        continue;
                    }
                    if n.clips {
                        painter.backend.pop_clip();
                        clip = clips.pop().unwrap();
                    }
                    if n.opacity < 1. {
                        painter.backend.pop_opacity();
                    }
                }
            }
        }
    }
}
