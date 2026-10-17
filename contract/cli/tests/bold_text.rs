//! LLP 1115.000 D12.3: Bold Text weights the system font one step, as
//! UIKit's does, where each host measures and paints it. Its own process:
//! the column is the process's.
use exact_kernel::style::metrics::{set_column, Column};
use exact_kernel::{Kernel, StyleId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// LLP 1115.000 D12.3: Bold Text weights the system font one step, as
/// UIKit's does (400 to 600, 600 to 800), an authored weight included, and
/// leaves a family the app names alone; turned off, the weights come back.
#[test]
fn bold_text_weights_the_system_font_and_leaves_a_named_family() {
    let plan = contract::compile(
        "component App\n  view\n    column\n      text testId=\"plain\" \"Plain\"\n      text testId=\"authored\" font-weight=600 \"Authored\"\n      text testId=\"serif\" font-family=\"serif\" \"Serif\"\n      text testId=\"named\" font-family=\"Georgia, serif\" \"Named\"\n      text testId=\"list\" font-family=\"system-ui, sans-serif\" \"List\"\n      text testId=\"para\"\n        text font-weight=500 \"Run\"\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let drawn = |r: &Runner<NoData>, id: &str| {
        let key = r.kernel().find_by_test_id(id)[0];
        r.kernel()
            .node_by_key(key)
            .unwrap()
            .text_style()
            .font_weight
    };
    assert_eq!((drawn(&r, "plain"), drawn(&r, "authored")), (400, 600));
    assert!(r.set_bold_text(true).unwrap().is_some(), "a change commits");
    assert_eq!(drawn(&r, "plain"), 600, "the system font, one step heavier");
    assert_eq!(drawn(&r, "authored"), 800, "an authored weight included");
    assert_eq!(drawn(&r, "serif"), 600, "a generic family is the system's");
    assert_eq!(
        drawn(&r, "named"),
        400,
        "a family the app names is left alone"
    );
    let run = |r: &Runner<NoData>| {
        let key = r.kernel().find_by_test_id("para")[0];
        r.kernel().node_by_key(key).unwrap().text_runs()[0]
            .style
            .font_weight
    };
    assert_eq!(run(&r), 700, "a run in a paragraph, 500 one step heavier");
    assert_eq!(
        drawn(&r, "list"),
        600,
        "a stack whose first is a generic is the system's"
    );
    let sent = {
        let key = r.kernel().find_by_test_id("plain")[0];
        r.kernel()
            .node_by_key(key)
            .unwrap()
            .computed_style(exact_kernel::StyleMask::INHERITED)
    };
    assert!(
        sent.mask.has(StyleId::FontWeight) && sent.font_weight == 600,
        "the rows a host is sent say the stepped weight"
    );
    assert!(
        r.set_bold_text(true).unwrap().is_none(),
        "the same setting commits nothing"
    );
    assert!(r.set_bold_text(false).unwrap().is_some());
    assert_eq!((drawn(&r, "plain"), drawn(&r, "authored")), (400, 600));
    // Through the preferences, with no resource reading them: still a commit.
    let bold = exact_runner::Preferences {
        bold_text: true,
        ..Default::default()
    };
    assert!(
        r.set_preferences(bold).unwrap().is_some(),
        "a change commits"
    );
    assert_eq!(drawn(&r, "plain"), 600);
}

/// What a host paints for itself takes the step as the kernel's text does:
/// a native button's title, the button's weight or none; a Markdown run,
/// whose pieces' bold the host steps by its flag; a font stack a binding
/// names at run time.
#[test]
fn bold_text_reaches_button_titles_markdown_and_runtime_stacks() {
    set_column(Column::Neutral);
    let plan = contract::compile(
        "component App\n  state sys = true\n  view\n    column\n      button appearance=\"auto\" testId=\"plain\"\n        text \"Save\"\n      button appearance=\"auto\" font-weight=600 testId=\"heavy\"\n        text \"Send\"\n      text testId=\"md\" markup=\"markdown\" \"Some **bold**\"\n      text testId=\"bound\" font-family=(sys ? \"system-ui, sans-serif\" : \"'Inter', serif\") \"Bound\"\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let view = |r: &Runner<NoData>, id: &str| {
        let key = r.kernel().find_by_test_id(id)[0];
        r.kernel().node_by_key(key).unwrap().id
    };
    let title =
        |r: &Runner<NoData>, id: &str| r.kernel().button_face_style(view(r, id)).unwrap().title;
    let (plain, heavy) = (
        title(&r, "plain").font_weight,
        title(&r, "heavy").font_weight,
    );
    assert_eq!(heavy, 600);
    r.set_bold_text(true).unwrap();
    let stepped = title(&r, "plain");
    assert_eq!(
        stepped.font_weight,
        exact_kernel::bolder(plain),
        "a title with no weight"
    );
    assert!(
        stepped.mask.has(StyleId::FontWeight),
        "said, so the host paints it"
    );
    assert_eq!(
        title(&r, "heavy").font_weight,
        800,
        "the button's 600, stepped"
    );
    let md = {
        let key = r.kernel().find_by_test_id("md")[0];
        r.kernel().node_by_key(key).unwrap().text_runs()[0].style
    };
    assert!(md.bold_text && md.font_weight == 600, "{md:?}");
    let bound = {
        let key = r.kernel().find_by_test_id("bound")[0];
        r.kernel()
            .node_by_key(key)
            .unwrap()
            .text_style()
            .font_weight
    };
    assert_eq!(bound, 600, "a runtime stack whose first member is generic");
    set_column(Column::Web);
}
