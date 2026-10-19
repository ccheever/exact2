//! `lower-flex-on-block`: a flex or grid container's property on a box that
//! is `display: block`. Each named case is laid out too: the child stays at
//! the box's left edge, as in a bare `<div>`, and the named repair centres it.

use crate::warnings::warned;
use exact_kernel::{Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};

const ID: &str = "lower-flex-on-block";

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// An app whose box is `boxed`, 100 points wide, around a 20-point child.
fn app(boxed: &str) -> String {
    format!("component App\n  view\n    column width=100\n      {boxed} width=100 height=40\n        view id=\"c\" width=20 height=10\n")
}

fn flagged(boxed: &str) -> Vec<String> {
    let src = app(boxed);
    let (_, warnings) = contract::compile_warned(&src);
    assert_eq!(
        warned(&src).iter().filter(|(id, _)| id == ID).count(),
        warnings.iter().filter(|w| w.id == ID).count()
    );
    warnings
        .into_iter()
        .filter(|w| w.id == ID)
        .map(|w| w.message)
        .collect()
}

fn child_x(boxed: &str) -> f32 {
    let plan = contract::compile(&app(boxed)).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 800.0))
        .unwrap();
    let key = r.kernel().find_by_id("c")[0];
    let view = r.kernel().arena().local_id(key.index);
    r.kernel().node(view).unwrap().frame.x
}

#[test]
fn a_flex_property_on_a_block_is_named_with_its_repair() {
    for (boxed, says) in [
        (
            "view align-items=\"center\"",
            "`align-items` does nothing on this `view`: it is `display: block`",
        ),
        (
            "view align-items=\"center\" justify-content=\"center\" gap=4",
            "`align-items`, `justify-content` and `gap` do nothing on this `view`",
        ),
        (
            "section display=\"block\" align-items=\"center\"",
            "write `display=\"flex\"`",
        ),
        (
            "view grid-template-columns=\"1fr 1fr\" align-items=\"center\"",
            "or `row` in place of `view`",
        ),
        ("scroll align-items=\"center\"", "this `scroll`"),
    ] {
        let messages = flagged(boxed);
        assert_eq!(messages.len(), 1, "{boxed}: {messages:?}");
        assert!(messages[0].contains(says), "{boxed}: {}", messages[0]);
        assert_eq!(child_x(boxed), 0.0, "{boxed}: CSS ignores it");
    }
    let grid = flagged("view grid-template-columns=\"1fr 1fr\"");
    assert!(grid[0].ends_with("write `display=\"grid\"`"), "{grid:?}");
    // The repairs it names lay the child out as the author meant.
    for fixed in [
        "column align-items=\"center\"",
        "view display=\"flex\" flex-direction=\"column\" align-items=\"center\"",
    ] {
        assert!(flagged(fixed).is_empty(), "{fixed}");
        assert_eq!(child_x(fixed), 40.0, "{fixed}");
    }
}

#[test]
fn what_the_walk_cannot_be_sure_of_or_css_gives_a_meaning_is_not_named() {
    for boxed in [
        "row justify-content=\"center\"",
        "view display=\"grid\" justify-items=\"center\"",
        // The children's default `justify-self` (Box Alignment 3), which Chrome draws.
        "view justify-items=\"center\"",
        "view display=(true ? \"flex\" : \"block\") align-items=\"center\"",
        "view display=\"none\" align-items=\"center\"",
        // Box Alignment gives these a meaning on a block container.
        "view align-content=\"center\"",
        // A multi-column box's gaps and `justify-content` lay out its columns.
        "view column-count=2 gap=4 row-gap=4 column-gap=4 justify-content=\"center\"",
        "view columns=\"2\" justify-content=\"center\"",
    ] {
        assert_eq!(flagged(boxed), Vec::<String>::new(), "{boxed}");
    }
}

/// Twenty earlier warnings of a lower rank (a symbol by another spelling
/// than its role, each) do not crowd it out of lowering's cap.
#[test]
fn twenty_lower_warnings_before_it_do_not_crowd_it_out() {
    let images = "      image \"symbol:sf/plus\"\n".repeat(21);
    let src = format!("component App\n  view\n    column\n{images}      view align-items=\"center\"\n        text \"a\"\n");
    let ids: Vec<String> = warned(&src).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids.len(), contract_lower::MAX_WARNINGS, "{ids:?}");
    assert_eq!(ids[0], ID, "{ids:?}");
    assert!(
        ids[1..].iter().all(|id| id == "lower-symbol-alias"),
        "{ids:?}"
    );
}
