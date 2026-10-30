//! What a backend is told when a scroller's kept row changes (`paint/rows.rs`).
use super::*;
use crate::image::Bitmap;
use crate::paint::Shape;
use crate::text::{Paragraph, RunPaint};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia::Transform;

mod measurement;

/// What the backend heard: each row begun, with the row it was said to
/// have been, and each row freed.
#[derive(Default)]
struct Heard {
    begun: Vec<(u32, Option<u32>)>,
    freed: Vec<u32>,
}

/// Keeps rows and draws nothing.
struct Keeps {
    heard: Rc<RefCell<Heard>>,
    row: u32,
}
impl Backend for Keeps {
    fn name(&self) -> &'static str {
        "keeps-rows"
    }
    fn begin(&mut self, _: f32, _: f32, _: f32) {}
    fn fill(&mut self, _: &Shape, _: [u8; 4], _: Transform) {}
    fn fill_gradient(&mut self, _: &Shape, _: &crate::paint::GradientPaint, _: Transform) {}
    fn fill_border(&mut self, _: &crate::paint::border::BorderFill, _: Transform) {}
    fn image(&mut self, _: &Arc<Bitmap>, _: Rect4, _: &[Shape], _: Transform, _: Option<[u8; 4]>) {}
    fn text(
        &mut self,
        _: &mut TextEngine,
        _: &Paragraph,
        _: &[RunPaint],
        _: (f32, f32),
        _: Transform,
    ) {
    }
    fn push_clip(&mut self, _: &Shape, _: Transform) {}
    fn pop_clip(&mut self) {}
    fn push_opacity(&mut self, _: f32) {}
    fn pop_opacity(&mut self) {}
    fn pointer(&mut self, _: f32, _: f32) {}
    fn finish(&mut self) -> Result<Pixmap, String> {
        Pixmap::new(400, 500).ok_or_else(|| "no pixmap".into())
    }
    fn rows(&self) -> bool {
        true
    }
    fn row_begin(&mut self, id: u32, _: (f32, f32), previous: Option<u32>) {
        self.row = id;
        self.heard.borrow_mut().begun.push((id, previous));
    }
    fn row_end(&mut self, _: Rect4) -> u32 {
        self.row
    }
    fn row_free(&mut self, id: u32) {
        self.heard.borrow_mut().freed.push(id);
    }
}

/// A row a commit bound to another item (LLP 1078) is a new row to the
/// backend: not the row it was, whose recording is the other item's (a
/// reader told otherwise draws that in its place until the new one is
/// made), and the old recording is freed. A row that only changed is still
/// the row it was.
#[test]
fn a_renewed_row_is_not_the_row_it_was() {
    let cells: String = (0..6)
        .map(|i| format!("        box height=50 width=200 testId=\"c{i}\"\n"))
        .collect();
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(&format!(
            "component App\n  view\n    scroll testId=\"port\" width=200 height=400\n      column\n{cells}"
        ))
        .unwrap()
        .encode(),
        (),
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let heard = Rc::new(RefCell::new(Heard::default()));
    p.brush.replace_backend(Box::new(Keeps {
        heard: heard.clone(),
        row: 0,
    }));
    let _ = p.frame();
    let first = std::mem::take(&mut *heard.borrow_mut());
    assert_eq!(first.begun.len(), 6, "each cell a kept row");
    assert!(first.begun.iter().all(|(_, previous)| previous.is_none()));
    // () changed: nothing is recorded again.
    let _ = p.frame();
    assert!(heard.borrow().begun.is_empty());
    // One row changed, one was bound to another item.
    let key = |name: &str| p.host.kernel().find_by_test_id(name)[0];
    let (changed, renewed) = (key("c1"), key("c4"));
    p.host.row_dirty.commit(
        &exact_kernel::CommitReceipt {
            touched: vec![changed, renewed],
            renewed: vec![renewed],
            ..Default::default()
        },
        0.0,
    );
    let _ = p.frame();
    let heard = heard.borrow();
    let (was_changed, was_renewed) = (first.begun[1].0, first.begun[4].0);
    assert_eq!(heard.begun.len(), 2);
    assert_eq!(
        heard.begun[0].1,
        Some(was_changed),
        "a changed row is the row it was"
    );
    assert_eq!(heard.begun[1].1, None, "a renewed row is a new one");
    assert!(heard.freed.contains(&was_renewed), "its old recording goes");
}

/// A reader made again holds none of the last one's rows (LLP 1115.001
/// D15.1): the backend that records for it is told every row as a new one,
/// and of none to free, though nothing in the app changed.
#[test]
fn a_new_reader_is_sent_every_row_again() {
    let cells: String = (0..6)
        .map(|i| format!("        box height=50 width=200 testId=\"c{i}\"\n"))
        .collect();
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(&format!(
            "component App\n  view\n    scroll testId=\"port\" width=200 height=400\n      column\n{cells}"
        ))
        .unwrap()
        .encode(),
        (),
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let first = Rc::new(RefCell::new(Heard::default()));
    p.brush.replace_backend(Box::new(Keeps {
        heard: first.clone(),
        row: 0,
    }));
    let _ = p.frame();
    assert_eq!(first.borrow().begun.len(), 6, "each cell a kept row");
    let _ = p.frame();
    assert!(!p.dirty(), "nothing is owed once it has painted");
    let second = Rc::new(RefCell::new(Heard::default()));
    p.reader_replaced(Box::new(Keeps {
        heard: second.clone(),
        row: 0,
    }));
    assert!(p.dirty(), "the new reader is owed a paint");
    let _ = p.frame();
    let second = second.borrow();
    assert_eq!(second.begun.len(), 6, "every row is recorded for it");
    assert!(second.begun.iter().all(|(_, previous)| previous.is_none()));
    assert!(second.freed.is_empty(), "it was sent no row to free");
}

/// A row is recorded again when a layout changes what a node in it
/// publishes beside a frame that stayed: a percentage padding whose
/// containing block narrowed (CSS Box Model 3 §4.1) moves the row's content,
/// which input reads anew, while its frame does not.
#[test]
fn a_row_whose_padding_alone_changed_is_recorded_again() {
    for moved in [false, true] {
        let (mut p, error) = Presenter::boot_with(
            &contract::compile(
                &r##"component App
  state narrow = false
  action shrink
    narrow = true
  view
    column
      box testId="shrink" press=shrink width=40 height=20
      scroll testId="port" width=400 height=300 margin-top=(narrow ? SHIFT : 0)
        column width=(narrow ? 300 : 400)
          box testId="padded" width=200 height=50 box-sizing="border-box" padding-left="5%"
          box testId="plain" width=200 height=50
"##
                .replace("SHIFT", if moved { "10" } else { "0" }),
            )
            .unwrap()
            .encode(),
            (),
            (400., 500.),
            1.,
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let heard = Rc::new(RefCell::new(Heard::default()));
        p.brush.replace_backend(Box::new(Keeps {
            heard: heard.clone(),
            row: 0,
        }));
        let _ = p.frame();
        let first = std::mem::take(&mut *heard.borrow_mut());
        assert_eq!(first.begun.len(), 2, "each box a kept row");
        let k = p.host.kernel();
        let (shrink, padded) = (
            k.find_by_test_id("shrink")[0],
            k.find_by_test_id("padded")[0],
        );
        let frame = k.node_by_key(padded).unwrap().frame;
        let shrink = k.node_by_key(shrink).unwrap().id;
        let reply = crate::agent::handle(&mut p, &format!(r#"{{"op":"tap","id":{shrink}}}"#));
        assert!(!reply.contains("error"), "{reply}");
        let k = p.host.kernel();
        let next = k.node_by_key(padded).unwrap().frame;
        assert_eq!(next.x, frame.x);
        assert_eq!(next.y, frame.y + if moved { 10. } else { 0. });
        assert_eq!((next.width, next.height), (frame.width, frame.height));
        assert_eq!(k.resolved_padding(padded).map(|p| p.0), Some(15.));
        let _ = p.frame();
        let heard = heard.borrow();
        let again: Vec<_> = heard.begun.iter().map(|(_, previous)| *previous).collect();
        assert_eq!(again, [Some(first.begun[0].0)], "the padded row alone");
    }
}

#[test]
fn inline_frame_retained_rows_keep_initial_and_scrolled_viewport_origins() {
    use exact_runner::geometry::InlineGeometry;

    for transform in ["", "scale=1.25 transform-origin=\"0 0\""] {
        let source = format!(
            r#"component App
  view
    column padding-left=30 padding-top=100 {transform}
      scroll testId="port" width=200 height=200
        column
          text height=80
            text "first" testId="first"
          text height=80
            text "second" testId="second"
          box height=400
"#
        );
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(&source),
            (),
            (400., 500.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.frame();
        let id = |name: &str| {
            let k = p.host.kernel();
            k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
        };
        let port = id("port");
        let runs = [id("first"), id("second")];
        let ordinary = runs.map(|id| p.ranges.frame(id));
        assert!(ordinary
            .iter()
            .all(|r| !r.unavailable && r.x >= 30. && r.y >= 100.));
        let heard = Rc::new(RefCell::new(Heard::default()));
        p.reader_replaced(Box::new(Keeps {
            heard: heard.clone(),
            row: 0,
        }));
        p.frame();
        assert_eq!(heard.borrow().begun.len(), 3, "each child is recorded");
        let recorded = runs.map(|id| p.ranges.frame(id));
        heard.borrow_mut().begun.clear();
        p.scroll_to(port, (0., 20.)).unwrap();
        p.frame();
        assert!(
            heard.borrow().begun.is_empty(),
            "scroll replays the recordings"
        );
        let replayed = runs.map(|id| p.ranges.frame(id));
        let shift = if transform.is_empty() { 20. } else { 25. };
        for ((expected, got), moved) in ordinary.into_iter().zip(recorded).zip(replayed) {
            assert!(!got.unavailable && !moved.unavailable);
            assert_eq!(
                (got.x, got.y, got.width, got.height),
                (expected.x, expected.y, expected.width, expected.height),
                "recorded {transform}"
            );
            assert_eq!(
                (moved.x, moved.y, moved.width, moved.height),
                (
                    expected.x,
                    expected.y - shift,
                    expected.width,
                    expected.height
                ),
                "replayed {transform}"
            );
        }
    }
}

#[test]
fn fractional_row_scroll_matches_a_fresh_walk_with_clips_and_hinted_text() {
    let source = r##"component App
  view
    scroll testId="port" width=120 height=80
      column
        view width=120 height=70 clip-path="path('M 5 5 Q 115 0 115 65 L 5 65 Z')" background-color="#2488bb"
          text "Hinted text" font-size=17 color="#ffffff"
        view width=120 height=200 background-color="#44aa33"
"##;
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (140., 100.),
        1.,
        PathBuf::new(),
        PainterChoice::Ink,
    )
    .unwrap();
    p.frame();
    let k = p.host.kernel();
    let port = k.node_by_key(k.find_by_test_id("port")[0]).unwrap().id;
    for offset in [0.25, 0.5, 1., 1.25] {
        p.scroll_to(port, (0., offset)).unwrap();
        let kept = p.frame();
        p.brush.forget_rows();
        p.repaint();
        let fresh = p.frame();
        assert!(kept.data() == fresh.data(), "scroll {offset}");
    }
}
