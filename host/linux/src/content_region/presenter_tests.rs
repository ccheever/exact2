//! Actual presenter failure fallback and physical dispatch over retained pixels.
use super::*;
use crate::content_region::ContentRegionRegistration;
use exact_runner::{DataError, Value};
use std::time::Instant;

// Deliberately uses the production worker, picture replay and CPU raster. The
// observer records effective backend geometry without changing its operands.
mod projection_discriminator {
    use super::*;
    use crate::image::Bitmap;
    use crate::paint::Shape;
    use crate::text::{Paragraph, RunPaint};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;
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
            bits.extend(s.radii.map(f32::to_bits));
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
        fn fill_border(&mut self, part: &crate::paint::border::BorderFill, t: Transform) {
            let mut bits: Vec<u32> = part.color.map(u32::from).to_vec();
            let ops = part.region.iter().chain(part.clip.iter().flatten());
            let xy: Vec<f32> = ops.flat_map(|op| op.points()).collect();
            bits.extend(xy.chunks(2).flat_map(|p| point((p[0], p[1]), t)));
            self.calls.borrow_mut().calls.push(Call("border", bits));
            self.raster.fill_border(part, t);
        }
        fn image(&mut self, i: &Arc<Bitmap>, dst: Rect4, clips: &[Shape], t: Transform) {
            self.shape("image", &Shape::rect(dst), t, &[]);
            self.raster.image(i, dst, clips, t);
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
                        glyphs +=
                            usize::from(a.x != b.x || a.y != b.y || a.cache_key != b.cache_key);
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
  action move writes top
    top = {next}
  action replaceAndMove writes text, top
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
}
#[path = "fail_backend.rs"]
mod fail_backend;
struct Empty;
impl DataSource for Empty {
    fn query(&mut self, n: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(n.into()))
    }
}
const APP: &str = r#"component App
  state title = "old picture"
  state link = "https://old.example/"
  state count = 0
  state draft = ""
  state showing = true
  action replace writes title, link
    title = "new source with changed action semantics"
    link = "https://new.example/"
  action replaceAgain writes title, link
    title = "C newest source"
    link = "https://third.example/"
  action hide writes showing
    showing = false
  action activate writes count
    count = title == "old picture" ? 1 : 2
  action edit(value) writes draft
    draft = value
  view
    column width=400 height=500
      button press=replace testId="replace" height=32
        text "replace"
      button press=replaceAgain testId="replace-again" height=20
        text "replace again"
      button press=hide testId="hide" height=20
        text "hide"
      input value=draft change=edit testId="input" height=32
      text `${count}` testId="count" height=24
      when showing
        view id="owner" width=400 height=200 overflow-x="hidden" overflow-y="hidden"
          scroll id="content" testId="scroll" width=400 height=200
            view height=600
              text title href=link press=activate testId="paragraph" font-size=20 color="light-dark(#ff0000,#0000ff)"
          text "Preparing content" id="pending" position="absolute"
"#;
fn id(p: &Presenter<Empty>, name: &str) -> ViewId {
    p.host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id(name)[0])
        .unwrap()
        .id
}
fn ready(p: &mut Presenter<Empty>) {
    let end = Instant::now() + Duration::from_secs(90);
    while !p.host.content_region().unwrap().receipt().unwrap().current {
        assert!(Instant::now() < end, "font/publication watchdog");
        assert!(p.host.content_region().unwrap().refusal().is_none());
        assert!(p.content_region_fd().is_some());
        // Test-only wait. Production watches the fd alongside physical input.
        std::thread::sleep(Duration::from_millis(1));
        assert!(p.poll_content_region().is_none());
    }
}
#[test]
fn non_cpu_region_refuses_before_plan_font_or_device_work() {
    for choice in [PainterChoice::Auto, PainterChoice::Gpu] {
        let result = Presenter::boot_with_content_region(
            &[],
            Empty,
            (400., 500.),
            1.,
            PathBuf::from("/nonexistent"),
            choice,
            ContentRegionRegistration {
                activate: None,
                owner: "o",
                content: "c",
                pending: "p",
            },
        );
        assert!(
            matches!(result, Err(HostError::Painter(ref e)) if e.contains("CPU")),
            "trial must refuse this painter before attempting even plan decoding"
        );
    }
}
#[test]
fn failed_frame_retains_source_hits_and_rejects_live_replacement_actions() {
    let _service = crate::content_region::test_service();
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(APP).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    let input = id(&p, "input");
    p.type_text(input, "external control remains live").unwrap();
    ready(&mut p);
    let first = p.frame();
    assert!(Arc::ptr_eq(&first, p.last_region_frame.as_ref().unwrap()));
    let paragraph = id(&p, "paragraph");
    let key = p.host.kernel().node(paragraph).unwrap().key;
    let b = p.box_of(paragraph).unwrap();
    let point = (b.rect.0 + 2., b.rect.1 + 2.);
    assert_eq!(p.hit(point.0, point.1), Some(paragraph));
    let old_stamp = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap()
        .request
        .stamp()
        .clone();
    let replace = id(&p, "replace");
    assert!(p.host.dispatch_at(replace, Event::Press, 1.).is_none());
    assert!(p.after_commit().is_none());
    ready(&mut p); // B is layout-ready, but has never succeeded in the backend.
    p.brush.replace_backend(Box::new(fail_backend::Failure));
    let failed = p.frame();
    assert!(Arc::ptr_eq(&first, &failed));
    assert!(!p.last_frame_succeeded);
    assert_eq!(first.data(), failed.data());
    let snapshot = p.host.content_region().unwrap().text_snapshot(key).unwrap();
    assert_eq!(snapshot.request.stamp(), &old_stamp);
    assert_eq!(
        snapshot.runs[0].link.as_ref().unwrap().1.as_ref(),
        "https://old.example/"
    );
    assert!(!snapshot.current);
    assert_eq!(p.handler_target(paragraph, EventKind::Press), None);
    assert_eq!(p.press_at(point.0, point.1, 2.), None);
    assert_eq!(
        p.host
            .kernel()
            .node(id(&p, "count"))
            .unwrap()
            .props
            .str(PropId::Text),
        Some("0")
    );
    assert!(
        !p.pointer_down(point.0, point.1, 2.).unwrap(),
        "old hit must not acquire a new content gesture"
    );
    assert!(p.collection_interaction().is_none());
    let port = id(&p, "scroll");
    p.wheel_at(point.0, point.1, 0., 30.);
    assert_eq!(p.scroll_of(port).1, 30., "read-only content still scrolls");
    assert!(p.resize(480., 520.).is_none());
    let resized_failure = p.frame();
    assert!(!Arc::ptr_eq(&first, &resized_failure));
    assert_eq!(
        (resized_failure.width(), resized_failure.height()),
        (480, 520)
    );
    for y in 0..first.height() as usize {
        assert_eq!(
            &resized_failure.data()[y * 480 * 4..][..400 * 4],
            &first.data()[y * 400 * 4..][..400 * 4]
        );
    }
    assert!(resized_failure.data()[500 * 480 * 4..]
        .iter()
        .all(|v| *v == 255));
    p.brush.replace_backend(Box::new(Raster::new()));
    p.frame();
    assert!(p.last_frame_succeeded);
    let snapshot = p.host.content_region().unwrap().text_snapshot(key).unwrap();
    assert_eq!(
        snapshot.runs[0].link.as_ref().unwrap().1.as_ref(),
        "https://new.example/"
    );
    assert!(snapshot.current);
    p.scroll.insert(port, (0., 0.));
    let light = p.frame();
    let shape_calls = p.text.borrow().shape_calls;
    p.brush.dark = true;
    assert!(p.host.content_region_appearance(true).is_none());
    let dark = p.frame();
    assert!(
        p.last_frame_succeeded,
        "appearance-only receipt must not strand reused artifacts"
    );
    assert_ne!(light.data(), dark.data());
    assert_eq!(
        p.text.borrow().shape_calls,
        shape_calls,
        "palette resolution cannot shape text"
    );
    assert!(p.host.content_region().unwrap().publication_painted());
    drop(p);
}

#[test]
fn held_old_source_latest_demand_and_destroy_retire_without_publishing_stale_pixels() {
    let _service = crate::content_region::test_service();
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(APP).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    ready(&mut p);
    let original = p.frame();
    let key = p.host.kernel().node(id(&p, "paragraph")).unwrap().key;
    let original_stamp = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap()
        .request
        .stamp()
        .clone();
    let gate = crate::content_region::test_hooks::next_text();
    assert!(p
        .host
        .dispatch_at(id(&p, "replace"), Event::Press, 1.)
        .is_none());
    assert!(p.after_commit().is_none());
    gate.entered();
    assert_eq!(p.host.content_region().unwrap().work_counts().0, 1);
    assert!(p
        .host
        .dispatch_at(id(&p, "replace-again"), Event::Press, 2.)
        .is_none());
    assert!(p.after_commit().is_none());
    let held = p.frame();
    assert_eq!(
        original.data(),
        held.data(),
        "B/C pending must retain successful A"
    );
    let snapshot = p.host.content_region().unwrap().text_snapshot(key).unwrap();
    assert_eq!(snapshot.request.stamp(), &original_stamp);
    assert!(!snapshot.current);
    let counts = p.host.content_region().unwrap().work_counts();
    assert_eq!(counts.0, 1);
    assert_eq!(counts.1, 1);
    assert_eq!(counts.2, 0);
    drop(gate);
    ready(&mut p);
    p.frame();
    let snapshot = p.host.content_region().unwrap().text_snapshot(key).unwrap();
    assert_eq!(
        snapshot.runs[0].link.as_ref().unwrap().1.as_ref(),
        "https://third.example/"
    );
    assert!(snapshot.current);
    // Now destroy the owner while a real old job is indivisible/parked.
    let gate = crate::content_region::test_hooks::next_text();
    assert!(p
        .host
        .dispatch_at(id(&p, "replace"), Event::Press, 3.)
        .is_none());
    assert!(p.after_commit().is_none());
    gate.entered();
    assert!(p
        .host
        .dispatch_at(id(&p, "hide"), Event::Press, 4.)
        .is_some());
    p.after_commit();
    assert!(
        p.host.content_region().unwrap().refusal().is_some(),
        "destroyed registration must explicitly retire"
    );
    assert!(p.content_region_fd().is_none());
    assert_eq!(
        p.host.content_region().unwrap().work_counts().0,
        1,
        "cancellation cannot refund running native allocations"
    );
    drop(gate);
    let end = Instant::now() + Duration::from_secs(10);
    while p.host.content_region().unwrap().work_counts().0 != 0 {
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    assert!(!p.host.poll_content_region().unwrap());
    assert_eq!(
        p.host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .runs[0]
            .link
            .as_ref()
            .unwrap()
            .1
            .as_ref(),
        "https://third.example/"
    );
}

#[test]
fn exact_scale_prepared_index_refuses_changed_dpr_before_stale_adoption_or_replay() {
    let _service = crate::content_region::test_service();
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(APP).unwrap().encode(),
        Empty,
        (400., 500.),
        1.25,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    ready(&mut p);
    let exact_kernel::RegionSelection::Accepted(a) = &p
        .host
        .content_region()
        .unwrap()
        .receipt()
        .unwrap()
        .selection
    else {
        panic!("current")
    };
    for artifact in a.artifacts() {
        let n = artifact
            .payload::<crate::content_region::NativeText>()
            .unwrap();
        assert_eq!(n.paint_context().scale().to_bits(), 1.25f32.to_bits());
        if let Some(paragraph) = n.paragraph() {
            assert!(
                paragraph.ink_capacity_bytes() > 0,
                "index exists BEFORE first paint"
            );
        }
    }
    let first = p.frame();
    let key = p.host.kernel().node(id(&p, "paragraph")).unwrap().key;
    let stamp = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap()
        .request
        .stamp()
        .clone();
    let gate = crate::content_region::test_hooks::next_text();
    assert!(p
        .host
        .dispatch_at(id(&p, "replace"), Event::Press, 1.)
        .is_none());
    p.after_commit();
    gate.entered();
    // Actual native DPI is fixed at boot today. Emulate a carrier reporting a
    // different scale to exercise the explicit retirement boundary.
    p.brush.scale = 2.;
    let retained = p.frame();
    assert!(
        p.host.content_region().unwrap().refusal().is_some(),
        "DPR must not replay/build a differently scaled index on UI"
    );
    assert_eq!((retained.width(), retained.height()), (800, 1000));
    for y in 0..first.height() as usize {
        assert_eq!(
            &retained.data()[y * 800 * 4..][..500 * 4],
            &first.data()[y * 500 * 4..][..500 * 4]
        );
    }
    assert!(p.content_region_fd().is_none());
    let old = p.host.content_region().unwrap().text_snapshot(key).unwrap();
    assert_eq!(old.request.stamp(), &stamp);
    assert!(!old.current);
    assert!(
        p.boxes.is_empty(),
        "old-DPR hits cannot route at new coordinates"
    );
    drop(gate);
    let end = Instant::now() + Duration::from_secs(10);
    while p.host.content_region().unwrap().work_counts().0 != 0 {
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    assert!(!p.host.poll_content_region().unwrap());
    assert_eq!(
        p.host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .request
            .stamp(),
        &stamp
    );
}

#[test]
fn malformed_region_scale_refuses_before_plan_fonts_or_worker_admission() {
    for scale in [0., -1., f32::NAN, f32::INFINITY] {
        let result = Presenter::boot_with_content_region(
            &[],
            Empty,
            (400., 500.),
            scale,
            PathBuf::from("/nonexistent"),
            PainterChoice::Cpu,
            ContentRegionRegistration {
                activate: None,
                owner: "owner",
                content: "content",
                pending: "pending",
            },
        );
        assert!(
            matches!(result, Err(HostError::Painter(ref e)) if e.contains("content raster context"))
        );
    }
}

#[test]
fn unsupported_region_query_preserves_previous_pixels_and_recovers() {
    let _service = crate::content_region::test_service();
    let app = APP.replace("state showing = true", "state showing = true\n  state ownerScale = 1\n  action collapse writes ownerScale\n    ownerScale = 0\n  action restore writes ownerScale\n    ownerScale = 1")
        .replace("view id=\"owner\"", "view id=\"owner\" scale=ownerScale")
        .replace("button press=hide", "button press=collapse testId=\"collapse\" height=20\n        text \"collapse\"\n      button press=restore testId=\"restore\" height=20\n        text \"restore\"\n      button press=hide");
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(&app).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    ready(&mut p);
    let first = p.frame();
    assert!(p
        .host
        .dispatch_at(id(&p, "collapse"), Event::Press, 1.)
        .is_none());
    p.after_commit();
    let refused = p.frame();
    assert!(
        !p.last_frame_succeeded,
        "singular region query must refuse, not visit every glyph"
    );
    assert_eq!(refused.data(), first.data());
    assert!(p
        .host
        .dispatch_at(id(&p, "restore"), Event::Press, 2.)
        .is_none());
    p.after_commit();
    assert_eq!(p.frame().data(), first.data());
    assert!(p.last_frame_succeeded);
}

#[test]
fn uncertain_ink_coordinates_refuse_full_glyph_fallback_and_recover() {
    let _service = crate::content_region::test_service();
    let app = APP.replace("state showing = true", "state showing = true\n  state inset = 0\n  action move writes inset\n    inset = 20000000\n  action restore writes inset\n    inset = 0")
        .replace("view height=600", "view height=600 padding-top=inset")
        .replace("button press=hide", "button press=move testId=\"move\" height=20\n        text \"move\"\n      button press=restore testId=\"restore\" height=20\n        text \"restore\"\n      button press=hide");
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(&app).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    ready(&mut p);
    let first = p.frame();
    assert!(p
        .host
        .dispatch_at(id(&p, "move"), Event::Press, 1.)
        .is_none());
    p.after_commit();
    ready(&mut p);
    let refused = p.frame();
    assert!(
        !p.last_frame_succeeded,
        "uncertain ink viewport cannot enter full glyph paint"
    );
    assert_eq!(refused.data(), first.data());
    assert!(p
        .host
        .dispatch_at(id(&p, "restore"), Event::Press, 2.)
        .is_none());
    p.after_commit();
    ready(&mut p);
    assert_eq!(p.frame().data(), first.data());
    assert!(p.last_frame_succeeded);
}

// Unlike APP's fixed 600px child, all overflow here comes from real text.
fn natural_scroll_fixture(short: bool) -> Presenter<Empty> {
    let tall = "Natural paragraph line with distinct words and wrap. ".repeat(80);
    let app = format!(
        r#"component App
  state text = "{}"
  state showing = true
  action shorten writes text
    text = "Short paragraph."
  action lengthen writes text
    text = "{tall}"
  action toggle writes showing
    showing = not showing
  view
    column width=400 height=500
      button press=shorten testId="shorten" height=30
        text "Shorten"
      button press=lengthen testId="lengthen" height=30
        text "Lengthen"
      button press=toggle testId="toggle" height=30
        text "Toggle"
      view id="owner" width=400 height=200 overflow-x="hidden" overflow-y="hidden"
        view id="content" width="100%" height="100%" display="flex" flex-direction="column"
          when showing
            scroll testId="natural-scroll" flex=1 min-height=0 width="100%" overflow-x="hidden"
              column width="100%" padding=8 box-sizing="border-box"
                text text testId="natural-text" font-size=20
        text "Preparing" id="pending" position="absolute"
"#,
        if short { "Short paragraph." } else { &tall }
    );
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(&app).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    ready(&mut p);
    p.frame();
    assert!(p.last_frame_succeeded);
    p
}

fn natural_point(p: &mut Presenter<Empty>, port: ViewId) -> (f32, f32) {
    let b = p.box_of(port).unwrap();
    (b.rect.0 + b.rect.2 / 2., b.rect.1 + b.rect.3 / 2.)
}

fn natural_replace(p: &mut Presenter<Empty>, action: &str) {
    let target = id(p, action);
    assert!(p.host.dispatch_at(target, Event::Press, 1.).is_none());
    assert!(p.after_commit().is_none());
    ready(p); // Layout ready is intentionally NOT a native paint publication.
}

#[test]
fn natural_scroll_replay_reports_the_offset_used_for_pixels() {
    let _service = crate::content_region::test_service();
    let mut p = natural_scroll_fixture(false);
    let port = id(&p, "natural-scroll");
    let point = natural_point(&mut p, port);
    let before = p.frame();
    p.wheel_at(point.0, point.1, 0., 30.);
    assert_eq!(p.scroll_of(port).1, 30., "real natural content must scroll");
    let after = p.frame();
    assert_ne!(before.data(), after.data(), "wheel must change real pixels");
    assert_eq!(
        p.box_of(port).unwrap().scroll,
        Some((0., 30.)),
        "replayed hit/agent metadata must describe this painted offset"
    );
}

#[test]
fn natural_scroll_clamp_keeps_painted_a_until_short_b_paints() {
    let _service = crate::content_region::test_service();
    let mut p = natural_scroll_fixture(false);
    let port = id(&p, "natural-scroll");
    let point = natural_point(&mut p, port);
    p.wheel_at(point.0, point.1, 0., 60.);
    let a = p.frame();
    assert_eq!(p.scroll_of(port).1, 60.);
    natural_replace(&mut p, "shorten");
    p.clamp_scroll();
    assert_eq!(
        p.scroll_of(port).1,
        60.,
        "ready B must not shrink the still-painted A scroll extent"
    );
    p.brush.replace_backend(Box::new(fail_backend::Failure));
    assert_eq!(p.frame().data(), a.data());
    assert!(!p.last_frame_succeeded);
    p.clamp_scroll();
    assert_eq!(p.scroll_of(port).1, 60., "failed B must keep A geometry");
    p.brush.replace_backend(Box::new(Raster::new()));
    p.frame();
    assert!(p.last_frame_succeeded);
    assert_eq!(
        p.scroll_of(port).1,
        0.,
        "successful short B clamps its offset"
    );
    assert_eq!(p.box_of(port).unwrap().scroll, Some((0., 0.)));
}

#[test]
fn pending_flip_keeps_acknowledged_a_scroll_and_source_until_b_ack_with_c_live() {
    let _service = crate::content_region::test_service();
    let mut p = natural_scroll_fixture(false);
    let port = id(&p, "natural-scroll");
    let port_key = p.host.kernel().node(port).unwrap().key;
    let text_key = p.host.kernel().node(id(&p, "natural-text")).unwrap().key;
    let point = natural_point(&mut p, port);
    p.wheel_at(point.0, point.1, 0., 60.);
    let a = p.display_frame().unwrap();
    assert!(p.display_complete(&a));
    assert_eq!(p.scroll_of(port).1, 60.);
    let a_request = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(text_key)
        .unwrap()
        .request
        .clone();
    let a_box = p.box_of(port).unwrap();

    natural_replace(&mut p, "shorten");
    assert_eq!(p.host.kernel().node(port).unwrap().key, port_key);
    assert!(p.resize(360., 460.).is_none());
    ready(&mut p);
    let b = p.display_frame().unwrap();
    let b_bytes = b.pixels.data().to_vec();
    assert!(p.last_frame_succeeded);
    assert_ne!(a.pixels.data(), b.pixels.data());
    assert_eq!(
        p.scroll_of(port).1,
        60.,
        "successful but unacknowledged B cannot clamp visible A"
    );
    assert_eq!(p.box_of(port).unwrap().scroll, a_box.scroll);
    assert_eq!(
        p.host
            .content_region()
            .unwrap()
            .text_snapshot(text_key)
            .unwrap()
            .request
            .stamp(),
        a_request.stamp()
    );

    natural_replace(&mut p, "lengthen"); // C is live/ready, never submitted.
    assert!(p.resize(900., 900.).is_none());
    ready(&mut p);
    assert_eq!(p.viewport(), (900., 900.));
    assert_eq!(p.host.kernel().node(port).unwrap().key, port_key);
    let c_stamp = p
        .host
        .kernel()
        .node_by_key(text_key)
        .unwrap()
        .paragraph_stamp()
        .unwrap();
    p.wheel_at(point.0, point.1, 0., 30.);
    assert_eq!(
        p.scroll_of(port).1,
        90.,
        "accept valid input against acknowledged A's extent"
    );
    assert_eq!(
        p.box_of(port).unwrap().scroll,
        Some((0., 60.)),
        "queued offset differs from displayed pixels"
    );
    assert_eq!(
        p.host
            .content_region()
            .unwrap()
            .text_snapshot(text_key)
            .unwrap()
            .request
            .stamp(),
        a_request.stamp()
    );
    assert!(p.display_frame().is_none());
    assert_eq!(
        b.pixels.data(),
        b_bytes,
        "later input cannot mutate submitted B"
    );
    assert!(p.dirty());

    assert!(p.display_complete(&b));
    assert_eq!(
        p.scroll_of(port).1,
        0.,
        "matching B ACK installs B's captured zero extent"
    );
    // Same poll cycle as the ACK: actual input paths must still use B even
    // though C is already dirty and no flip is pending at this instant.
    assert_eq!(p.box_of(port).unwrap().scroll, Some((0., 0.)));
    assert_eq!(p.hit(point.0, point.1), Some(port));
    p.wheel_at(point.0, point.1, 0., 30.);
    assert_eq!(
        p.scroll_of(port).1,
        0.,
        "B's zero extent stays authoritative between ACK and next submit"
    );
    let snapshot = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(text_key)
        .unwrap();
    assert_ne!(snapshot.request.stamp(), a_request.stamp());
    assert_ne!(snapshot.request.stamp(), &c_stamp);
    assert!(!snapshot.current);
    assert!(p.dirty(), "ACK must not overwrite C's dirty state");
    assert_eq!(b.pixels.data(), b_bytes);
}

#[test]
fn natural_scroll_wheel_cannot_use_unpainted_tall_b_extent() {
    let _service = crate::content_region::test_service();
    let mut p = natural_scroll_fixture(true);
    let port = id(&p, "natural-scroll");
    let point = natural_point(&mut p, port);
    let a = p.frame();
    natural_replace(&mut p, "lengthen");
    p.brush.replace_backend(Box::new(fail_backend::Failure));
    assert_eq!(p.frame().data(), a.data());
    p.wheel_at(point.0, point.1, 0., 30.);
    assert_eq!(p.scroll_of(port).1, 0., "painted A has no scroll overflow");
    p.brush.replace_backend(Box::new(Raster::new()));
    p.frame();
    assert!(p.last_frame_succeeded);
    p.wheel_at(point.0, point.1, 0., 30.);
    assert_eq!(p.scroll_of(port).1, 30., "successful B owns its new extent");
}

#[test]
fn natural_scroll_removed_and_recreated_keys_cannot_borrow_painted_a_geometry() {
    let _service = crate::content_region::test_service();
    let mut p = natural_scroll_fixture(false);
    let old_port = id(&p, "natural-scroll");
    let old_key = p.host.kernel().node(old_port).unwrap().key;
    let point = natural_point(&mut p, old_port);
    p.wheel_at(point.0, point.1, 0., 30.);
    p.frame();
    natural_replace(&mut p, "toggle");
    assert!(p.host.kernel().node_by_key(old_key).is_none());
    p.clamp_scroll();
    assert!(!p.scroll.contains_key(&old_port));
    natural_replace(&mut p, "toggle");
    let new_port = id(&p, "natural-scroll");
    let new_key = p.host.kernel().node(new_port).unwrap().key;
    assert_ne!(new_key, old_key);
    let recycled = p.host.kernel().arena().key(old_key.index);
    assert_ne!(recycled, old_key);
    assert!(
        p.host.kernel().node_by_key(recycled).is_some(),
        "old slot was actually reused"
    );
    // A ready replacement's own live geometry is not proof that these pixels
    // have been shown. An offset for an unpainted key must not be retained.
    p.scroll.insert(new_port, (0., 30.));
    p.clamp_scroll();
    assert_eq!(p.scroll_of(new_port).1, 0.);
    p.frame();
    assert!(p.last_frame_succeeded);
    let point = natural_point(&mut p, new_port);
    p.wheel_at(point.0, point.1, 0., 30.);
    assert_eq!(p.scroll_of(new_port).1, 30.);
}

// Source-only retained-action baseline. Shared fixture stays in the existing
// swipe tests; it uses actual Messages handlers and complete MessageBubble.
use super::swipe_tests::retained_actions as actions;

#[test]
fn retained_actions_press_keeps_complete_a_binding_while_body_b_is_parked() {
    let _service = crate::content_region::test_service();
    let mut p = actions::boot(&actions::source(), true, actions::Rows::default());
    let reply = actions::id(&p, "reply-message-0");
    let at = actions::point(&mut p, reply);
    let key = p
        .host
        .kernel()
        .node(actions::id(&p, "body-message-0"))
        .unwrap()
        .key;
    let stamp = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap()
        .request
        .stamp()
        .clone();
    let a = actions::ack(&mut p);
    let _gate = actions::block_body(&mut p, 1.);
    assert_eq!(
        p.press_at(at.0, at.1, 10.),
        Some(reply),
        "actual retained Press dispatch"
    );
    assert_eq!(actions::selected(&p), Value::str("message-0"));
    assert_eq!(
        p.host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .request
            .stamp(),
        &stamp
    );
    assert!(
        !p.host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .current
    );
    // Ordinary controls and composer remain outside the registered worker region.
    let composer = actions::id(&p, "composer");
    p.type_text(composer, "typing with B parked").unwrap();
    assert_eq!(
        p.host.runner().slot("draft"),
        Some(&Value::str("typing with B parked"))
    );
    let outside = actions::id(&p, "outside");
    let at = actions::point(&mut p, outside);
    assert_eq!(p.press_at(at.0, at.1, 11.), Some(outside));
    assert_eq!(actions::selected(&p), Value::str("outside"));
    assert_eq!(p.host.content_region().unwrap().work_counts().0, 1);
    assert!(!a.pixels.data().is_empty());
}

#[test]
fn retained_actions_changed_curry_or_visibility_refuses_before_clock_or_focus() {
    let _service = crate::content_region::test_service();
    for control in ["curry-change", "disable", "hide"] {
        let mut p = actions::boot(&actions::source(), true, actions::Rows::default());
        let reply = actions::id(&p, "reply-message-0");
        let at = actions::point(&mut p, reply);
        let key = p.host.kernel().node(reply).unwrap().key;
        p.focus = Some(actions::id(&p, "composer"));
        let _gate = actions::block_body(&mut p, 1.);
        actions::action(&mut p, control, 2.);
        if control == "curry-change" {
            assert_eq!(
                p.host
                    .kernel()
                    .node(actions::id(&p, "reply-changed-0"))
                    .unwrap()
                    .key,
                key
            );
        }
        let before = actions::clocks(&p);
        let focus = p.focus;
        assert_eq!(p.press_at(at.0, at.1, 100.), None, "{control}");
        assert_eq!(
            actions::clocks(&p),
            before,
            "rejected sample cannot advance clock/dispatch"
        );
        assert_eq!(p.focus, focus);
        assert_eq!(actions::selected(&p), Value::str(""));
    }
}

#[test]
fn retained_actions_down_a_pending_b_live_c_never_rebinds_or_implicitly_paints() {
    let _service = crate::content_region::test_service();
    let mut p = actions::boot(&actions::source(), true, actions::Rows::default());
    let reply = actions::id(&p, "reply-message-0");
    let at = actions::point(&mut p, reply);
    let key = p
        .host
        .kernel()
        .node(actions::id(&p, "body-message-0"))
        .unwrap()
        .key;
    let a_stamp = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap()
        .request
        .stamp()
        .clone();
    actions::action(&mut p, "body-change", 1.);
    actions::ready(&mut p);
    let b = p.display_frame().unwrap();
    assert!(p.last_frame_succeeded);
    let b_bytes = b.pixels.data().to_vec();
    assert_eq!(
        p.host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .request
            .stamp(),
        &a_stamp
    );
    assert!(
        p.pointer_down(at.0, at.1, 2.).unwrap(),
        "DOWN belongs to acknowledged A"
    );
    let gate = actions::block_body(&mut p, 3.);
    actions::action(&mut p, "curry-change", 4.);
    let before = actions::clocks(&p);
    assert!(!p.pointer_up(at.0, at.1, 100.).unwrap());
    assert_eq!(actions::clocks(&p), before);
    assert_eq!(actions::selected(&p), Value::str(""));
    assert_eq!(p.collection_interaction(), None);
    assert!(p.display_complete(&b));
    let b_stamp = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap()
        .request
        .stamp()
        .clone();
    assert_ne!(b_stamp, a_stamp);
    assert!(p.dirty(), "C must survive matching B ACK");
    // The actual post-ACK input path must keep B until the carrier submits C.
    let _ = p.box_of(reply);
    assert_eq!(p.press_at(at.0, at.1, 101.), None);
    assert_eq!(actions::clocks(&p), before);
    assert_eq!(
        p.host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .request
            .stamp(),
        &b_stamp
    );
    assert_eq!(b.pixels.data(), b_bytes);
    assert!(!p.display_complete(&b));
    drop(gate);
    actions::ready(&mut p);
    actions::ack(&mut p);
    let current = actions::id(&p, "reply-changed-0");
    let at = actions::point(&mut p, current);
    assert_eq!(p.press_at(at.0, at.1, 102.), Some(current));
    assert_eq!(actions::selected(&p), Value::str("changed-0"));
    assert_eq!(b.pixels.data(), b_bytes);
}
