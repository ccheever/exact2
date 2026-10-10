//! A scroller's kept rows (`paint/rows.rs`) under a backend that keeps
//! them: what it is told of a row recorded again. The one such backend is
//! the Android recorder, which this host does not compile elsewhere.
use super::*;
use crate::image::Bitmap;
use crate::paint::Shape;
use crate::text::{Paragraph, RunPaint};
use exact_runner::{DataError, Value};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia::Transform;

#[derive(Default)]
struct Nothing;
impl DataSource for Nothing {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

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
        Nothing,
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
    // Nothing changed: nothing is recorded again.
    let _ = p.frame();
    assert!(heard.borrow().begun.is_empty());
    // One row changed, one was bound to another item.
    let key = |name: &str| p.host.kernel().find_by_test_id(name)[0];
    let (changed, renewed) = (key("c1"), key("c4"));
    p.host.row_dirty.commit(&exact_kernel::CommitReceipt {
        touched: vec![changed, renewed],
        renewed: vec![renewed],
        ..Default::default()
    });
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
