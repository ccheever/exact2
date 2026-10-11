//! A presenter's paint that misses the text cache walks it at most once.
use super::cache::trim_walks;
use crate::presenter::{PainterChoice, Presenter};
use exact_runner::{DataError, DataSource, Value};

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const PARAGRAPHS: usize = 120;

fn fixture() -> Presenter<NoData> {
    let mut source = String::from("component App\n  view\n    column width=\"100%\"\n");
    for i in 0..PARAGRAPHS {
        source.push_str(&format!(
            "      text \"Paragraph {i}: enough words that a narrower width wraps it.\"\n"
        ));
    }
    let plan = contract::compile(&source).unwrap();
    let (presenter, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 3000.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    presenter
}

/// md-mixed4m's revisit pass: the kernel answers a width it laid out before
/// from its own cache, so every paragraph misses in paint. Each miss used to
/// walk every identity to evict (16,865 walks a resize); the frame now walks
/// at most once, at its end.
#[test]
fn a_paint_that_misses_every_paragraph_walks_the_cache_at_most_once() {
    let mut p = fixture();
    for width in [300.0, 200.0, 250.0] {
        assert!(p.resize(width, 3000.0).is_none());
        p.frame();
    }
    // Nothing the cache owns survives: what the 250 frame holds is pinned,
    // every other width is gone.
    p.text().borrow_mut().paragraphs.clear();
    assert!(p.resize(300.0, 3000.0).is_none());
    let (layouts, walks) = {
        let engine = p.text().borrow();
        (engine.layout_calls, trim_walks())
    };
    p.frame();
    let missed = p.text().borrow().layout_calls - layouts;
    assert!(
        missed >= PARAGRAPHS,
        "the revisit's paint misses every paragraph ({missed} layouts)"
    );
    let walked = trim_walks() - walks;
    assert!(
        walked <= 1,
        "{missed} misses walked the cache {walked} times"
    );
    // Residency is still bounded: the frame's maintenance trims what is over.
    let engine = p.text().borrow();
    let residency = engine.residency();
    assert!(residency.cold_policy_bytes <= residency.cold_target_bytes);
    assert!(residency.identities <= PARAGRAPHS + super::cache::COLD_IDENTITIES);
}
