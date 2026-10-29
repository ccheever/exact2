//! LLP 1011.002: an image in a paragraph's text takes its box in the line
//! Parley breaks, the glyphs after it start past it, and the kernel places
//! the image where the box went.
use crate::presenter::{PainterChoice, Presenter};

fn boot(view: &str) -> Presenter<()> {
    let source = format!("component App\n  view\n    column align-items=\"start\"\n{view}");
    let plan = contract::compile(&source).unwrap();
    let (mut presenter, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    // The middle-alignment assertion below uses DejaVu Sans's x-height.
    let text = presenter.text().clone();
    *text.borrow_mut() = crate::text::TextEngine::from_font_files(
        &[std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/fixtures/fonts/assets"
        ))],
        "DejaVu Sans",
    );
    let kernel = presenter
        .host()
        .kernel()
        .rehydrate(Box::new(crate::text::Measurer(text)));
    *presenter.host_mut().runner_mut().kernel_mut() = kernel;
    presenter.resize(300.0, 300.0);
    presenter.frame();
    presenter
}

fn frame(p: &Presenter<()>, id: &str) -> exact_kernel::Frame {
    let kernel = p.host().kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(id)[0])
        .unwrap()
        .frame
}

#[test]
fn an_image_takes_its_box_between_the_words_and_the_line_grows_to_hold_it() {
    let plain = boot("      text \"ab\" font-size=16 testId=\"ab\"\n      text \"abcd\" font-size=16 testId=\"abcd\"\n");
    let p = boot(concat!(
        "      text font-size=16 testId=\"p\"\n",
        "        text \"ab\"\n",
        "        image \"app:/x.png\" width=24 height=40 testId=\"img\"\n",
        "        text \"cd\"\n",
    ));
    let (ab, abcd, line) = (
        frame(&plain, "ab"),
        frame(&plain, "abcd"),
        frame(&plain, "abcd").height,
    );
    let (para, img) = (frame(&p, "p"), frame(&p, "img"));
    // After "ab", 24 wide; "cd" after it, so the paragraph is that much wider.
    assert!((img.x - para.x - ab.width).abs() <= 1.0, "{img:?} {ab:?}");
    assert!(
        (para.width - abcd.width - 24.0).abs() <= 1.0,
        "{para:?} {abcd:?}"
    );
    assert_eq!((img.width, img.height), (24.0, 40.0));
    // `baseline`: its bottom on the baseline, so the line is 40 px above it
    // and the text's descent below: taller than a line of text.
    assert!(para.height > 40.0 && para.height < 40.0 + line, "{para:?}");
    assert!((img.y - para.y).abs() < 0.5, "{img:?} {para:?}");
}

#[test]
fn middle_and_a_length_place_the_image_against_the_baseline() {
    let at = |align: &str| {
        let p = boot(&format!(
            "      text font-size=16 testId=\"p\"\n        text \"ab\"\n        image \"app:/x.png\" width=8 height=8 vertical-align=\"{align}\" testId=\"img\"\n        text \"cd\"\n"
        ));
        (frame(&p, "img").y - frame(&p, "p").y, frame(&p, "p").height)
    };
    let (baseline, plain) = at("baseline");
    let (middle, _) = at("middle");
    let (raised, height) = at("4px");
    // A small box keeps the line's height. `middle` puts its middle half
    // the x-height above the baseline: an 8 px box is raised by half of
    // what the x-height (8 to 9 px at 16 px) exceeds it by. A length raises
    // it by that much.
    assert!(
        middle < baseline && baseline - middle < 0.5,
        "{middle} {baseline}"
    );
    assert!(
        (baseline - raised - 4.0).abs() < 0.01,
        "{raised} {baseline}"
    );
    assert_eq!(height, plain);
}
