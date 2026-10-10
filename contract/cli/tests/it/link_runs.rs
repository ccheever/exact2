//! LLP 1115 §3: a run with an `href` in its paragraph is a link, and what
//! its author leaves unsaid of its colour is the platform's link role
//! (`LinkText`: the tint on iOS, `linkColor` on macOS, the browser's own on
//! the web). The author's `color` on the run wins; a block-level `link`
//! keeps inheriting, as the bare node it is.

use exact_kernel::style::roles::role;
use exact_kernel::{Color, ColorValue, Kernel, StyleId, StyleMask};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn color(r: &Runner<NoData>, id: &str) -> ColorValue {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel()
        .node_by_key(key)
        .unwrap()
        .computed_style(StyleMask::of(StyleId::TextColor))
        .text_color
}

#[test]
fn an_href_run_is_link_text_unless_its_author_colours_it() {
    let src = "component App\n  view\n    column color=\"#336699\"\n      text testId=\"p\"\n        text \"Read \"\n        text testId=\"a\" href=\"/about\" \"about\"\n        text testId=\"b\" href=\"/b\" color=\"red\" \"red\"\n        text testId=\"plain\" \" us\"\n      link testId=\"block\" href=\"/x\"\n        text \"Block\"\n";
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let blue = ColorValue::Fixed(Color::rgba(0x33, 0x66, 0x99, 255));
    assert_eq!(color(&r, "a"), ColorValue::Role(role("LinkText").unwrap()));
    assert_eq!(
        color(&r, "b"),
        ColorValue::Fixed(Color::rgba(255, 0, 0, 255))
    );
    assert_eq!(color(&r, "plain"), blue, "a run without an href inherits");
    assert_eq!(color(&r, "block"), blue, "a block-level `link` inherits");
}
