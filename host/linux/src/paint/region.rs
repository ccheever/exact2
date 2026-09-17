//! Flat drawing commands for ONE accepted content publication. This is neither
//! a mutable view tree nor another layout graph. It pins exact paragraphs,
//! palettes, source/link artifacts and charged image owners across candidate
//! changes, including destroyed keys. Shell origin/clip and scroll remain live.
use super::*;
use exact_kernel::{NodeKey, RegionPublication};
use std::cell::RefCell;

const COMMANDS: usize = exact_kernel::region::REGION_NODES * 12;
enum Command {
    Fill(Shape, [u8; 4], Transform),
    Stroke(Shape, f32, [u8; 4], Transform),
    Image(Arc<Bitmap>, Rect4, Vec<Shape>, Transform),
    Text(Rc<Paragraph>, Vec<RunPaint>, (f32, f32), Transform),
    PushClip(Shape, Transform),
    PopClip,
    PushOpacity(f32),
    PopOpacity,
    Scroll(NodeKey, (f32, f32)),
    EndScroll,
    Hit(NodeKey, PaintedBox),
}
struct Commands {
    list: Vec<Command>,
    error: Option<&'static str>,
    paragraphs: BTreeMap<usize, Rc<Paragraph>>,
}
#[derive(Clone)]
pub(super) struct Capture(Rc<RefCell<Commands>>);
impl Capture {
    fn add(&self, command: Command) {
        let mut s = self.0.borrow_mut();
        if s.list.len() < COMMANDS && s.error.is_none() {
            s.list.push(command);
        } else if s.error.is_none() {
            s.error = Some("content flat paint command limit");
        }
    }
    pub(super) fn hit(&self, key: NodeKey, b: PaintedBox) {
        self.add(Command::Hit(key, b));
    }
    pub(super) fn scroll(&self, key: NodeKey, offset: (f32, f32)) {
        self.add(Command::Scroll(key, offset));
    }
    pub(super) fn end_scroll(&self) {
        self.add(Command::EndScroll);
    }
}
impl Backend for Capture {
    fn name(&self) -> &'static str {
        "content-capture"
    }
    fn begin(&mut self, _: f32, _: f32, _: f32) {}
    fn fill(&mut self, s: &Shape, c: [u8; 4], t: Transform) {
        self.add(Command::Fill(*s, c, t));
    }
    fn stroke(&mut self, s: &Shape, w: f32, c: [u8; 4], t: Transform) {
        self.add(Command::Stroke(*s, w, c, t));
    }
    fn image(&mut self, i: &Arc<Bitmap>, dst: Rect4, c: &[Shape], t: Transform) {
        self.add(Command::Image(i.clone(), dst, c.to_vec(), t));
    }
    fn text(
        &mut self,
        _: &mut TextEngine,
        p: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        t: Transform,
    ) {
        let paragraph = self
            .0
            .borrow()
            .paragraphs
            .get(&(p as *const Paragraph as usize))
            .cloned();
        if let Some(p) = paragraph {
            self.add(Command::Text(p, palette.to_vec(), origin, t));
        } else {
            self.0.borrow_mut().error = Some("content painter received an unowned paragraph");
        }
    }
    fn push_clip(&mut self, s: &Shape, t: Transform) {
        self.add(Command::PushClip(*s, t));
    }
    fn pop_clip(&mut self) {
        self.add(Command::PopClip);
    }
    fn push_opacity(&mut self, a: f32) {
        self.add(Command::PushOpacity(a));
    }
    fn pop_opacity(&mut self) {
        self.add(Command::PopOpacity);
    }
    fn pointer(&mut self, _: f32, _: f32) {
        self.0.borrow_mut().error = Some("pointer inside content snapshot");
    }
    fn finish(&mut self) -> Result<Pixmap, String> {
        Err("content capture has no pixel surface".into())
    }
}
pub(super) struct Picture {
    publication: Rc<RegionPublication>,
    commands: Vec<Command>,
    paragraphs: BTreeMap<NodeKey, Rc<Paragraph>>,
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
    ) -> Result<Rc<Self>, String> {
        if let Some(p) = &painter.region_picture {
            if p.matches(publication)
                && p.dark == painter.dark
                && p.scale == painter.scale.to_bits()
            {
                return Ok(p.clone());
            }
        }
        let mut owners = BTreeMap::new();
        // This first consumer is static read-only Markdown. General transformed
        // children/inputs require their own coherent interaction snapshot.
        for f in publication.frames() {
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
            let p = native
                .paragraph()
                .ok_or("intrinsic answer cannot paint final content")?;
            owners.insert(Rc::as_ptr(p) as usize, p.clone());
        }
        let capture = Capture(Rc::new(RefCell::new(Commands {
            list: Vec::new(),
            error: None,
            paragraphs: owners,
        })));
        let mut recorder = Painter::new(
            painter.text.clone(),
            painter.scale,
            Box::new(capture.clone()),
        );
        recorder.dark = painter.dark;
        let mut walk = Walk {
            scene,
            boxes: Vec::new(),
            text: BTreeMap::new(),
            skip: None,
            region: Some(publication),
            capture: Some(&capture),
            replay: None,
            region_error: None,
        };
        let content = scene
            .kernel
            .node_by_key(region.binding().content)
            .ok_or("content root removed")?;
        let origin = region.receipt().ok_or("content layout absent")?.origin;
        recorder.node(
            &mut walk,
            content.id,
            Transform::identity(),
            (origin.x, origin.y),
            None,
        );
        let mut state = capture.0.borrow_mut();
        if let Some(e) = state.error {
            return Err(e.into());
        }
        Ok(Rc::new(Self {
            publication: publication.clone(),
            commands: std::mem::take(&mut state.list),
            paragraphs: walk.text,
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
        self.region_picture.as_ref().is_some_and(|p| {
            p.owner == view
                || p.commands
                    .iter()
                    .any(|c| matches!(c,Command::Hit(_,b) if b.id==view))
        })
    }
}
pub(super) struct Replay<'a> {
    pub picture: &'a Picture,
    pub origin: exact_kernel::Frame,
    pub content: NodeKey,
    pub viewport: (f32, f32),
}
impl Replay<'_> {
    fn queries_supported(&self, painter: &Painter, walk: &Walk<'_, '_>, origin: Transform) -> bool {
        // A full-device viewport conservatively contains every native clip.
        // This pass visits flat commands, never glyphs or source text, and runs
        // before any retained text reaches the ordinary fallback-capable backend.
        let scale = painter.scale;
        let clip = (
            0.,
            0.,
            (self.viewport.0 * scale).round().max(1.),
            (self.viewport.1 * scale).round().max(1.),
        );
        let mut scroll = (0., 0.);
        let mut stack = Vec::new();
        for command in &self.picture.commands {
            match command {
                Command::Scroll(key, old) => {
                    stack.push(scroll);
                    let off = scroll_offset(walk, *key, *old);
                    scroll = (scroll.0 + off.0, scroll.1 + off.1);
                }
                Command::EndScroll => scroll = stack.pop().unwrap(),
                Command::Text(paragraph, _, at, transform) => {
                    let transform = Transform::from_scale(scale, scale)
                        .pre_concat(
                            origin
                                .pre_translate(-scroll.0, -scroll.1)
                                .pre_concat(*transform),
                        )
                        .pre_scale(1. / scale, 1. / scale);
                    if !paragraph.prepared_ink_supports(*at, scale, transform, clip) {
                        return false;
                    }
                }
                _ => {}
            }
        }
        true
    }
    pub(super) fn paint(
        &self,
        painter: &mut Painter,
        walk: &mut Walk<'_, '_>,
        parent: Transform,
        offset: (f32, f32),
        outer_clip: Option<Rect4>,
    ) {
        let origin = parent.pre_translate(self.origin.x - offset.0, self.origin.y - offset.1);
        if !self.queries_supported(painter, walk, origin) {
            walk.region_error = Some("content-region prepared ink query refused");
            return;
        }
        let mut scroll = (0., 0.);
        let mut scroll_stack = Vec::new();
        let mut clips = Vec::new();
        let mut clip = outer_clip;
        for op in &self.picture.commands {
            let at = origin.pre_translate(-scroll.0, -scroll.1);
            match op {
                Command::Fill(s, c, t) => painter.backend.fill(s, *c, at.pre_concat(*t)),
                Command::Stroke(s, w, c, t) => painter.backend.stroke(s, *w, *c, at.pre_concat(*t)),
                Command::Image(i, d, c, t) => painter.backend.image(i, *d, c, at.pre_concat(*t)),
                Command::Text(p, palette, o, t) => painter.backend.text(
                    &mut painter.text.borrow_mut(),
                    p,
                    palette,
                    *o,
                    at.pre_concat(*t),
                ),
                Command::PushClip(s, t) => {
                    painter.backend.push_clip(s, at.pre_concat(*t));
                    clips.push(clip);
                    let own = bbox(at.pre_concat(*t), s.rect);
                    clip = Some(clip.map_or(own, |c| intersect(c, own)));
                }
                Command::PopClip => {
                    painter.backend.pop_clip();
                    clip = clips.pop().unwrap();
                }
                Command::PushOpacity(a) => painter.backend.push_opacity(*a),
                Command::PopOpacity => painter.backend.pop_opacity(),
                Command::Scroll(key, old) => {
                    scroll_stack.push(scroll);
                    let off = scroll_offset(walk, *key, *old);
                    scroll = (scroll.0 + off.0, scroll.1 + off.1);
                }
                Command::EndScroll => {
                    scroll = scroll_stack.pop().unwrap();
                }
                Command::Hit(key, b) => {
                    // Never route a retained box to a recycled ViewId. A source
                    // changed in the live paragraph also retires its old hit.
                    let live = walk.scene.kernel.node_by_key(*key).is_some_and(|n| {
                        self.picture
                            .publication
                            .paint_artifact(*key)
                            .is_none_or(|a| {
                                n.paragraph_stamp().as_ref() == Some(a.request().stamp())
                            })
                    });
                    if live {
                        walk.boxes.push(PaintedBox {
                            rect: bbox(at, b.rect),
                            clip,
                            ..*b
                        });
                    }
                }
            }
        }
        for (key, p) in &self.picture.paragraphs {
            walk.text.insert(*key, p.clone());
        }
    }
}

fn scroll_offset(walk: &Walk<'_, '_>, key: NodeKey, old: (f32, f32)) -> (f32, f32) {
    walk.scene
        .kernel
        .node_by_key(key)
        .and_then(|n| walk.scene.scroll.get(&n.id))
        .copied()
        .unwrap_or(old)
}
