// Deliberately uses the production worker, picture replay and CPU raster. The
// observer records effective backend geometry without changing its operands.
use super::*;
use crate::image::Bitmap;
use crate::paint::{GradientPaint, Shape};
use crate::text::{Paragraph, RunPaint};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use tiny_skia::{Point, Transform};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Call(&'static str, Vec<u32>);
#[derive(Clone, Default)]
struct Observation {
    calls: Vec<Call>,
    placement_diffs: Vec<usize>,
    glyph_diffs: Vec<usize>,
}
type Calls = Rc<RefCell<Observation>>;
struct ObservedRaster {
    raster: Raster,
    calls: Calls,
    reference: Option<Calls>,
}
fn point(at: (f32, f32), ts: Transform) -> [u32; 2] {
    let mut p = Point::from_xy(at.0, at.1);
    ts.map_point(&mut p);
    [p.x.to_bits(), p.y.to_bits()]
}
fn transform_bits(t: Transform) -> [u32; 6] {
    [t.sx, t.kx, t.ky, t.sy, t.tx, t.ty].map(f32::to_bits)
}
impl ObservedRaster {
    fn shape(&self, kind: &'static str, s: &Shape, ts: Transform, extra: &[u32]) {
        let (x, y, w, h) = s.rect;
        let mut bits = point((x, y), ts).to_vec();
        bits.extend(point((x + w, y + h), ts));
        bits.extend(
            s.radii
                .into_iter()
                .flat_map(|(x, y)| [x.to_bits(), y.to_bits()]),
        );
        bits.extend(rect_bits(s.rect));
        bits.extend(transform_bits(ts));
        bits.extend_from_slice(extra);
        self.calls.borrow_mut().calls.push(Call(kind, bits));
    }
    fn mark(&self, kind: &'static str) {
        self.calls.borrow_mut().calls.push(Call(kind, Vec::new()));
    }
}
impl Backend for ObservedRaster {
    fn name(&self) -> &'static str {
        "cpu"
    }
    fn begin(&mut self, w: f32, h: f32, scale: f32) {
        *self.calls.borrow_mut() = Observation::default();
        self.raster.begin(w, h, scale);
    }
    fn fill(&mut self, s: &Shape, c: [u8; 4], t: Transform) {
        self.shape("fill", s, t, &c.map(u32::from));
        self.raster.fill(s, c, t);
    }
    fn fill_gradient(&mut self, s: &Shape, g: &GradientPaint, t: Transform) {
        self.raster.fill_gradient(s, g, t);
    }
    fn fill_border(&mut self, part: &crate::paint::border::BorderFill, t: Transform) {
        let mut bits: Vec<u32> = part.color.map(u32::from).to_vec();
        let ops = part.region.iter().chain(part.clip.iter().flatten());
        let xy: Vec<f32> = ops.flat_map(|op| op.points()).collect();
        bits.extend(xy.chunks(2).flat_map(|p| point((p[0], p[1]), t)));
        self.calls.borrow_mut().calls.push(Call("border", bits));
        self.raster.fill_border(part, t);
    }
    fn image(
        &mut self,
        i: &Arc<Bitmap>,
        dst: Rect4,
        clips: &[Shape],
        t: Transform,
        tint: Option<[u8; 4]>,
    ) {
        self.shape("image", &Shape::rect(dst), t, &[]);
        self.raster.image(i, dst, clips, t, tint);
    }
    fn text(
        &mut self,
        engine: &mut TextEngine,
        p: &Paragraph,
        palette: &[RunPaint],
        at: (f32, f32),
        t: Transform,
    ) {
        let mut raw = point(at, t).to_vec();
        raw.extend([at.0.to_bits(), at.1.to_bits()]);
        raw.extend(transform_bits(t));
        for ink in palette {
            raw.extend(ink.color.map(u32::from));
        }
        let index = self
            .calls
            .borrow()
            .calls
            .iter()
            .filter(|c| c.0 == "text")
            .count();
        if let Some(reference) = &self.reference {
            let expected = reference
                .borrow()
                .calls
                .iter()
                .filter(|c| c.0 == "text")
                .nth(index)
                .cloned()
                .unwrap();
            let e = &expected.1;
            let origin = (f32::from_bits(e[2]), f32::from_bits(e[3]));
            let transform = Transform::from_row(
                f32::from_bits(e[4]),
                f32::from_bits(e[6]),
                f32::from_bits(e[5]),
                f32::from_bits(e[7]),
                f32::from_bits(e[8]),
                f32::from_bits(e[9]),
            );
            // SAME live Paragraph/catalog/palette in both arms. Only raw
            // placement differs; full-scene clipping/RGBA is checked below.
            let mut actual = Raster::new();
            let mut oracle = Raster::new();
            actual.begin(400., 600., 1.);
            oracle.begin(400., 600., 1.);
            actual.text(engine, p, palette, at, t);
            oracle.text(engine, p, palette, origin, transform);
            let actual = actual.finish().unwrap();
            let oracle = oracle.finish().unwrap();
            let different = actual
                .data()
                .iter()
                .zip(oracle.data())
                .filter(|(a, b)| a != b)
                .count();
            let mut glyphs = 0;
            for (i, run) in p.layout_runs().enumerate() {
                for g in run.glyphs {
                    let a = g.physical((at.0, at.1 + p.baselines()[i]), 1.);
                    let b = g.physical((origin.0, origin.1 + p.baselines()[i]), 1.);
                    glyphs += usize::from(a.x != b.x || a.y != b.y || a.cache_key != b.cache_key);
                }
            }
            eprintln!("same-paragraph text{index}: raw={raw:?} expected={e:?} glyph-placement/key-diffs={glyphs} RGBA-different-bytes={different}");
            self.calls.borrow_mut().placement_diffs.push(different);
            self.calls.borrow_mut().glyph_diffs.push(glyphs);
        }
        self.calls.borrow_mut().calls.push(Call("text", raw));
        self.raster.text(engine, p, palette, at, t);
    }
    fn push_clip(&mut self, s: &Shape, t: Transform) {
        self.shape("clip", s, t, &[]);
        self.raster.push_clip(s, t);
    }
    fn pop_clip(&mut self) {
        self.mark("pop-clip");
        self.raster.pop_clip();
    }
    fn push_opacity(&mut self, a: f32) {
        self.calls
            .borrow_mut()
            .calls
            .push(Call("opacity", vec![a.to_bits()]));
        self.raster.push_opacity(a);
    }
    fn pop_opacity(&mut self) {
        self.mark("pop-opacity");
        self.raster.pop_opacity();
    }
    fn pointer(&mut self, x: f32, y: f32) {
        self.raster.pointer(x, y);
    }
    fn finish(&mut self) -> Result<Pixmap, String> {
        self.raster.finish()
    }
}
fn observe(p: &mut Presenter<Empty>, reference: Option<Calls>) -> Calls {
    let calls = Rc::new(RefCell::new(Observation::default()));
    p.brush.replace_backend(Box::new(ObservedRaster {
        raster: Raster::new(),
        calls: calls.clone(),
        reference,
    }));
    calls
}
fn app(fractional: bool) -> String {
    let (x, y, pad, border, next) = if fractional {
        (13.6, 213.6, 0.1, 0.2, 217.3)
    } else {
        (14., 214., 1., 2., 218.)
    };
    let text = "Retained source A has real wrapping and distinct visible lines. ".repeat(24);
    format!(
        r##"component App
  state top = {y}
  state text = "{text}"
  action move
    top = {next}
  action replaceAndMove
    text = "Candidate B must not supply retained source A pixels or hits."
    top = {next}
  view
    column width=400 height=600 padding-left={x} box-sizing="border-box"
      view height=top flex-shrink=0
      view id="owner" width=320 height=200 flex-shrink=0 overflow-x="hidden" overflow-y="hidden"
        view id="content" width="100%" height="100%" display="flex" flex-direction="column"
          scroll testId="port" width="100%" flex=1 min-height=0 overflow-x="hidden"
            column testId="bubble" width="100%" box-sizing="border-box" padding={pad} border-width={border} border-style="solid" border-color="#203040" background-color="#f0e0d0"
              column testId="inner" width="100%" box-sizing="border-box" padding={pad}
                text text testId="paragraph" font-size=16 color="#123456" href="https://source-a.example/"
        text "" id="pending" position="absolute"
      button press=move testId="move" height=24
        text "Move"
      button press=replaceAndMove testId="replace" height=24
        text "Replace"
"##
    )
}
fn fixture(plan: &[u8], region: bool) -> Presenter<Empty> {
    let assets = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain"));
    let (mut p, error) = if region {
        Presenter::boot_with_content_region(
            plan,
            Empty,
            (400., 600.),
            1.,
            assets,
            PainterChoice::Cpu,
            ContentRegionRegistration {
                activate: None,
                owner: "owner",
                content: "content",
                pending: "pending",
            },
        )
    } else {
        Presenter::boot_with(plan, Empty, (400., 600.), 1., assets, PainterChoice::Cpu)
    }
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    if region {
        ready(&mut p);
        p.frame();
    }
    assert!(p.last_frame_succeeded);
    p
}
fn wheel(p: &mut Presenter<Empty>, dy: f32) {
    let port = id(p, "port");
    let at = natural_point(p, port);
    p.wheel_at(at.0, at.1, 0., dy);
    assert_eq!(p.scroll_of(port), (0., dy), "real clamped scroll");
}
fn rect_bits(r: Rect4) -> [u32; 4] {
    [r.0.to_bits(), r.1.to_bits(), r.2.to_bits(), r.3.to_bits()]
}
fn frame_bits(f: exact_kernel::Frame) -> [u32; 4] {
    rect_bits((f.x, f.y, f.width, f.height))
}
fn compare(
    label: &str,
    ordinary: &mut Presenter<Empty>,
    region: &mut Presenter<Empty>,
    a: &Calls,
    b: &Calls,
    retained: bool,
) -> Vec<String> {
    let pa = ordinary.frame();
    let pb = region.frame();
    assert!(ordinary.last_frame_succeeded && region.last_frame_succeeded);
    let a = a.borrow().calls.clone();
    let observed = b.borrow().clone();
    let b = &observed.calls;
    let pixels = pa
        .data()
        .iter()
        .zip(pb.data())
        .filter(|(a, b)| a != b)
        .count();
    let first = a.iter().zip(b.iter()).position(|(a, b)| a != b);
    eprintln!("projection {label}: calls={}/{} first-difference={first:?} RGBA-different-bytes={pixels} dimensions={:?}/{:?}", a.len(), b.len(), (pa.width(), pa.height()), (pb.width(), pb.height()));
    if let Some(i) = first {
        eprintln!("ordinary={:?}\nregion={:?}", a[i], b[i]);
    }
    let mut failures = Vec::new();
    if &a != b {
        failures.push(format!(
            "{label}: raw backend geometry/transform/order differs"
        ));
    }
    if observed.placement_diffs.iter().any(|n| *n != 0) {
        failures.push(format!(
            "{label}: same-Paragraph placement RGBA differ: {:?}",
            observed.placement_diffs
        ));
    }
    if observed.glyph_diffs.iter().any(|n| *n != 0) {
        failures.push(format!(
            "{label}: same-Paragraph physical glyph/key inputs differ: {:?}",
            observed.glyph_diffs
        ));
    }
    if pa.width() != pb.width() || pa.height() != pb.height() || pa.data() != pb.data() {
        failures.push(format!("{label}: real CPU RGBA differ ({pixels} bytes)"));
    }
    let retention = region.host.kernel().region_retention();
    eprintln!("projection {label}: retention={retention:?}");
    assert!(retention.accepted_offers <= 64 && retention.candidate_offers <= 64);
    assert!(retention.total_source_bytes < 16 * 1024);
    // Same authored names, independently booted runtimes: never compare their
    // NodeKeys to one another. Every retained lookup uses its own live key.
    for name in ["port", "bubble", "inner", "paragraph"] {
        let oa = id(ordinary, name);
        let rb = id(region, name);
        let of = ordinary.host.kernel().node(oa).unwrap().frame;
        let key = region.host.kernel().node(rb).unwrap().key;
        let receipt = region.host.content_region().unwrap().receipt().unwrap();
        let exact_kernel::region::RegionSelection::Accepted(publication) = &receipt.selection
        else {
            panic!("accepted A required")
        };
        let projected = publication.frame(key, receipt.origin).unwrap();
        eprintln!(
            "projection {label}/{name}: ordinary={:?} selected={:?} current={}",
            frame_bits(of),
            frame_bits(projected),
            receipt.current
        );
        if frame_bits(of) != frame_bits(projected) {
            failures.push(format!("{label}/{name}: projected frame differs"));
        }
        if retained && name == "paragraph" {
            assert!(
                region.box_of(rb).is_none(),
                "changed source cannot gain a stale hit"
            );
        } else {
            let ob = ordinary.box_of(oa).unwrap();
            let bb = region.box_of(rb).unwrap();
            eprintln!(
                "projection {label}/{name}: hits={:?}/{:?} scroll={:?}/{:?}",
                rect_bits(ob.rect),
                rect_bits(bb.rect),
                ob.scroll,
                bb.scroll
            );
            if rect_bits(ob.rect) != rect_bits(bb.rect) || ob.scroll != bb.scroll {
                failures.push(format!("{label}/{name}: painted hit/scroll differs"));
            }
        }
    }
    let key = region
        .host
        .kernel()
        .node(id(region, "paragraph"))
        .unwrap()
        .key;
    let snapshot = region
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap();
    let ordinary_frame = ordinary
        .host
        .kernel()
        .node(id(ordinary, "paragraph"))
        .unwrap()
        .frame;
    assert_eq!(snapshot.current, !retained);
    if frame_bits(snapshot.frame) != frame_bits(ordinary_frame) {
        failures.push(format!("{label}: text snapshot geometry differs"));
    }
    failures
}
fn current_case(fractional: bool) {
    let _service = crate::content_region::test_service();
    assert_eq!(exact_kernel::region::REGION_OFFERS, 64);
    let plan = contract::compile(&app(fractional)).unwrap().encode();
    let mut ordinary = fixture(&plan, false);
    let mut region = fixture(&plan, true);
    let a = observe(&mut ordinary, None);
    let b = observe(&mut region, Some(a.clone()));
    let mut failures = compare("initial", &mut ordinary, &mut region, &a, &b, false);
    let dy = if fractional { 17.3 } else { 17. };
    wheel(&mut ordinary, dy);
    wheel(&mut region, dy);
    failures.extend(compare(
        "one-wheel",
        &mut ordinary,
        &mut region,
        &a,
        &b,
        false,
    ));
    assert!(failures.is_empty(), "{failures:#?}");
}
#[test]
fn projection_discriminator_integer_control() {
    current_case(false);
}
#[test]
fn projection_discriminator_fractional_border_padding_scroll() {
    current_case(true);
}
#[test]
fn projection_discriminator_retained_a_moves_while_b_is_pending() {
    let _service = crate::content_region::test_service();
    assert_eq!(exact_kernel::region::REGION_OFFERS, 64);
    let plan = contract::compile(&app(true)).unwrap().encode();
    let mut ordinary = fixture(&plan, false);
    let mut region = fixture(&plan, true);
    let a = observe(&mut ordinary, None);
    let b = observe(&mut region, Some(a.clone()));
    wheel(&mut ordinary, 17.3);
    wheel(&mut region, 17.3);
    ordinary.frame();
    region.frame();
    let key = region
        .host
        .kernel()
        .node(id(&region, "paragraph"))
        .unwrap()
        .key;
    let state = region.host.content_region().unwrap();
    let old = state.text_snapshot(key).unwrap();
    let stamp = old.request.stamp().clone();
    let old_paragraph = old.paragraph as *const Paragraph;
    let old_source = old
        .request
        .with_request(|r| r.runs.iter().map(|r| &*r.text).collect::<String>());
    let old_frame = old.frame;
    let gate = crate::content_region::test_hooks::next_text();
    assert!(region
        .host
        .dispatch_at(id(&region, "replace"), Event::Press, 1.)
        .is_none());
    assert!(region.after_commit().is_none());
    gate.entered();
    assert!(ordinary
        .host
        .dispatch_at(id(&ordinary, "move"), Event::Press, 1.)
        .is_none());
    assert!(ordinary.after_commit().is_none());
    let failures = compare(
        "retained-A/new-origin",
        &mut ordinary,
        &mut region,
        &a,
        &b,
        true,
    );
    let old = region
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap();
    assert_eq!(old.request.stamp(), &stamp);
    assert_eq!(old.paragraph as *const Paragraph, old_paragraph);
    assert_eq!(
        old.request
            .with_request(|r| r.runs.iter().map(|r| &*r.text).collect::<String>()),
        old_source
    );
    assert_ne!(
        frame_bits(old.frame),
        frame_bits(old_frame),
        "origin actually moved"
    );
    assert!(!old.current);
    assert_eq!(
        old.runs[0].link.as_ref().unwrap().1.as_ref(),
        "https://source-a.example/"
    );
    drop(gate);
    ready(&mut region);
    region.frame();
    assert!(region.last_frame_succeeded);
    assert!(
        region
            .host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .current
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
