//! LLP 1011.002: `contract/corpus/inline-image.contract`, an image in a
//! paragraph's text, lowered to an `Image` child of its `Text` and placed in
//! the line by the reference measurer.
use exact_kernel::{Kernel, VerticalAlign, INLINE_BOX_TEXT};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
use std::path::Path;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn an_image_in_a_paragraph_is_a_run_between_its_words() {
    let plan = contract::compile(&corpus("inline-image.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
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
        .compute_layout(root, exact_kernel::Offer::definite(400.0, 600.0))
        .unwrap();
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let message = node("message");
    let runs = message.text_runs();
    let texts: Vec<&str> = runs.iter().map(|r| &*r.text).collect();
    assert_eq!(texts, ["hello ", INLINE_BOX_TEXT, " there"]);
    // `1.375em` of 16 px, raised `-0.2em`: lowered 3.2 px.
    let b = runs[1].inline.expect("the emoji is a box");
    assert!((b.height - 22.0).abs() < 1e-3 && (b.width - 22.0).abs() < 1e-3);
    assert!((b.shift + 3.2).abs() < 1e-3, "{b:?}");
    assert_eq!(b.align, VerticalAlign::Baseline);
    let read: Vec<String> = message.read_runs().iter().map(|s| s.to_string()).collect();
    assert_eq!(read.concat(), "hello :wave: there");
    // After "hello " (six glyphs of 9.6 px), its bottom 3.2 px below the
    // baseline, which is 22 - 3.2 px down the line.
    let emoji = node("emoji-b");
    assert!(emoji.is_inline_image());
    assert!(
        (emoji.frame.x - 6.0 * 9.6).abs() < 1e-3,
        "{:?}",
        emoji.frame
    );
    assert!(
        (emoji.frame.y - message.frame.y).abs() < 1e-3,
        "{:?}",
        emoji.frame
    );
    // 18.8 px above the baseline, and the text's 3.84 below it.
    assert!(
        (message.frame.height - 22.64).abs() < 1e-3,
        "{:?}",
        message.frame
    );
    let dot = node("dot").style.clone();
    assert_eq!(dot.rare.vertical_align, VerticalAlign::Middle);
}

#[test]
fn vertical_align_is_an_inline_images_alone() {
    for (attr, id) in [
        ("text \"a\" vertical-align=\"middle\"", "lower-attr-tag"),
        (
            "image \"app:/a.png\" vertical-align=\"middle\"",
            "lower-attr-tag",
        ),
    ] {
        let source = format!("component App\n  view\n    {attr}\n");
        assert_eq!(contract::compile(&source).unwrap_err().id, id, "{source}");
    }
    for value in ["top", "sub", "10%", "nonsense"] {
        let source = format!(
            "component App\n  view\n    text\n      image \"app:/a.png\" vertical-align=\"{value}\"\n"
        );
        assert_eq!(
            contract::compile(&source).unwrap_err().id,
            "lower-css-shorthand",
            "{source}"
        );
    }
    for attr in [
        "width=\"50%\"",
        "min-width=4",
        "max-height=\"2em\"",
        "height=(1 > 0 ? \"10%\" : \"4px\")",
    ] {
        let source =
            format!("component App\n  view\n    text\n      image \"app:/a.png\" {attr}\n");
        assert_eq!(
            contract::compile(&source).unwrap_err().id,
            "lower-attr-value",
            "{source}"
        );
    }
    let columns = "component App\n  view\n    text column-count=2\n      image \"app:/a.png\"\n";
    assert_eq!(contract::compile(columns).unwrap_err().id, "lower-attr-tag");
    // An image is the paragraph's own child, never a nested run's.
    let nested = "component App\n  view\n    text\n      text\n        image \"app:/a.png\"\n";
    assert_eq!(
        contract::compile(nested).unwrap_err().id,
        "lower-leaf-children"
    );
    // A paragraph that holds a string or is a `time` holds no image.
    for parent in ["text \"a string\"", "time datetime=0"] {
        let source = format!("component App\n  view\n    {parent}\n      image \"app:/a.png\"\n");
        assert_eq!(
            contract::compile(&source).unwrap_err().id,
            "lower-leaf-children",
            "{source}"
        );
    }
    // A choice of literals is split into both rows per arm.
    contract::compile(
        "component App\n  state big = false\n  view\n    text\n      image \"app:/a.png\" vertical-align=(big ? \"middle\" : \"-2px\")\n",
    )
    .unwrap();
}
