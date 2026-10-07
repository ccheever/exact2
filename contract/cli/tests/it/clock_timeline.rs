//! `timeline Name` and `animation-timeline=Name` (LLP 1055.002 D1, D2): the
//! row reads `-exact-clock(Name)`, its animations stay on the clock, and two
//! declarations of one name meet as a refusal, never as one shared phase.

use exact_kernel::{Kernel, StyleId};
use exact_plan::{Plan, Value as PlanValue};
use exact_runner::{DataError, DataSource, Runner};
use std::path::PathBuf;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[PlanValue]) -> Result<PlanValue, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SOURCE: &str = "timeline Pending

keyframes pending
  from opacity=0.4
  to opacity=1

style Waiting
  animation=\"pending 800ms ease-in-out infinite alternate\"
  animation-timeline=Pending

component App
  view
    column
      text \"Unlocking\" testId=\"lock\" animation=\"pending 800ms ease-in-out infinite alternate\" animation-timeline=Pending
      text \"Starting\" testId=\"engine\" class=Waiting
      text \"Alone\" testId=\"alone\" animation=\"pending 800ms infinite\"
";

fn clock(r: &Runner<NoData>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap();
    assert!(node.style.mask.has(StyleId::Animation) || test_id == "alone");
    node.style
        .rare
        .animation_timeline
        .clock()
        .map(str::to_owned)
}

#[test]
fn a_named_clock_timeline_reaches_the_row_and_keeps_an_infinite_animation() {
    let plan = contract::compile(SOURCE).unwrap();
    let r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(clock(&r, "lock").as_deref(), Some("Pending"));
    assert_eq!(clock(&r, "engine").as_deref(), Some("Pending"));
    assert_eq!(clock(&r, "alone"), None);
    // The kernel tells the engine each node's clock before its animations.
    let k = r.kernel();
    let mut sync = exact_kernel::MotionSync::default();
    for id in ["lock", "engine"] {
        k.motion_sync_node(k.find_by_test_id(id)[0], &mut sync);
    }
    let clocks: Vec<_> = sync.clocks.iter().map(|(_, c)| c.as_deref()).collect();
    assert_eq!(clocks, [Some("Pending"), Some("Pending")]);
}

#[test]
fn a_style_naming_no_timeline_is_refused() {
    let e = contract::compile(
        "style S\n  animation-timeline=Nope\ncomponent App\n  view\n    text \"x\" class=S\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "contract-timeline-unknown");
}

#[test]
fn a_timeline_declared_twice_in_one_file_is_refused() {
    let e = contract::compile("timeline A\ntimeline A\ncomponent App\n  view\n    text \"x\"\n")
        .unwrap_err();
    assert_eq!(e.id, "syntax-duplicate-declaration");
}

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("exact-clock-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
}

#[test]
fn a_timeline_is_imported_by_name_and_two_files_of_one_name_are_two_timelines() {
    let app = App::new("imports");
    let spinner = "timeline Pending\nkeyframes p\n  to opacity=0\ncomponent Spinner\n  view\n    text \"…\" animation=\"p 1s infinite\" animation-timeline=Pending\n";
    app.write("lib/spinner.contract", spinner);
    let one = app.write(
        "one.contract",
        "use Spinner, Pending from \"./lib/spinner.contract\"\nkeyframes q\n  to opacity=0\ncomponent App\n  view\n    column\n      Spinner()\n      text \"x\" animation=\"q 1s infinite\" animation-timeline=Pending\n",
    );
    let plan = format!("{:?}", contract::compile_path(&one).unwrap());
    assert!(plan.contains("-exact-clock(Pending)"), "{plan}");
    assert!(!plan.contains("Pending__"), "{plan}");
    // The app's own `Pending` and `p` are not the spinner's: each file's
    // names mean its own declarations, and the spinner's are renamed where
    // they are written (LLP 1091 D4/D5).
    let two = app.write(
        "two.contract",
        "use Spinner from \"./lib/spinner.contract\"\ntimeline Pending\nkeyframes p\n  to opacity=1\ncomponent App\n  view\n    column\n      Spinner()\n      text \"x\" animation=\"p 1s\" animation-timeline=Pending\n",
    );
    let plan = format!("{:?}", contract::compile_path(&two).unwrap());
    for name in [
        "-exact-clock(Pending)",
        "-exact-clock(Pending__spinner)",
        "p__spinner 1s infinite",
    ] {
        assert!(plan.contains(name), "{name}: {plan}");
    }
    // A file sees no timeline it does not name.
    let three = app.write(
        "three.contract",
        "use Spinner from \"./lib/spinner.contract\"\ncomponent App\n  view\n    column\n      Spinner()\n      text \"x\" animation=\"p 1s\"\n",
    );
    let e = contract::compile_path(&three).unwrap_err();
    assert_eq!(e.id, "contract-use-missing", "{e:?}");
    let _ = std::fs::remove_dir_all(&app.0);
}

#[test]
fn a_local_binding_of_the_name_shadows_the_timeline() {
    let src = "timeline Pending\nkeyframes p\n  to opacity=0\ncomponent App\n  state Pending = \"--drag\"\n  view\n    column\n      text \"x\" testId=\"x\" animation=\"p 1s\" animation-timeline=Pending\n";
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("x")[0]).unwrap();
    let t = &node.style.rare.animation_timeline;
    assert_eq!((t.clock(), t.name()), (None, Some("--drag")));
}

#[test]
fn every_entry_point_sees_the_clock() {
    let app = App::new("entries");
    let src = "timeline Pending\nkeyframes p\n  to opacity=0\ncomponent App\n  view\n    text \"x\" animation=\"p 1s infinite\" animation-timeline=Pending\n";
    let path = app.write("app.contract", src);
    contract::lean::lean(src, "App").unwrap();
    contract::lean::lean_path(&path, "App").unwrap();
    contract::symbols_json(&path, None).unwrap();
    let _ = std::fs::remove_dir_all(&app.0);
}
