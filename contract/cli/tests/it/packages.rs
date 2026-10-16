//! LLP 1091 D8/D9: `exact:` built-ins and packages through `node_modules`.

#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

struct Dir(PathBuf);
impl Dir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("exact-pkg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exact_motion_gives_every_file_one_activity_timeline() {
    let dir = Dir::new("builtin");
    dir.write(
        "spinner.contract",
        "use Activity from \"exact:motion\"\nkeyframes spin\n  to rotate=\"360deg\"\ncomponent Spinner\n  view\n    view animation=\"spin 1s infinite\" animation-timeline=Activity\n",
    );
    let root = dir.write(
        "app.contract",
        "use Spinner from \"./spinner.contract\"\nuse Activity from \"exact:motion\"\nkeyframes pulse\n  to opacity=0\ncomponent App\n  view\n    column\n      Spinner()\n      view animation=\"pulse 1s infinite\" animation-timeline=Activity\n",
    );
    let plan = format!("{:?}", contract::compile_path(&root).unwrap());
    assert!(plan.contains("-exact-clock(Activity)"), "{plan}");
    assert!(
        !plan.contains("Activity__"),
        "one declaration, one name: {plan}"
    );
    let bad = dir.write(
        "bad.contract",
        "use Activity from \"exact:moton\"\ncomponent App\n  view\n    text \"x\"\n",
    );
    let e = contract::compile_path(&bad).unwrap_err();
    assert_eq!(e.id, "contract-use-builtin", "{e}");
    assert!(e.message.contains("`exact:motion`"), "{}", e.message);
}

fn library(dir: &Dir, at: &str, package_json: &str) {
    dir.write(&format!("{at}/package.json"), package_json);
    dir.write(
        &format!("{at}/src/card.contract"),
        "use Pad from \"../styles.contract\"\ncomponent Card\n  view\n    column class=Pad testId=\"card\"\n",
    );
    dir.write(
        &format!("{at}/styles.contract"),
        "style Pad\n  padding-top=10\n",
    );
    dir.write(
        &format!("{at}/index.contract"),
        "use Card from \"./src/card.contract\"\n",
    );
}

#[test]
fn a_package_is_found_through_node_modules_and_mapped_through_its_exports() {
    let dir = Dir::new("exports");
    library(
        &dir,
        "app/node_modules/@acme/ui",
        r#"{"name":"@acme/ui","version":"1.2.3","exports":{".":{"contract":"./index.contract"},"./card":"./src/card.contract"}}"#,
    );
    let root = dir.write(
        "app/app.contract",
        "use Card from \"@acme/ui\"\nuse Card as Same from \"@acme/ui/card\"\ncomponent App\n  view\n    column\n      Card()\n      Same()\n",
    );
    contract::compile_path(&root).unwrap();
    // `exports` offers only what it lists.
    dir.write(
        "app/app.contract",
        "use Pad from \"@acme/ui/styles.contract\"\ncomponent App\n  view\n    text \"x\"\n",
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert_eq!(e.id, "contract-use-package", "{e}");
    // Not installed: the refusal says how to add it.
    dir.write(
        "app/app.contract",
        "use Card from \"@acme/nothing\"\ncomponent App\n  view\n    text \"x\"\n",
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert_eq!(e.id, "contract-use-package", "{e}");
    assert!(e.message.contains("bun add @acme/nothing"), "{}", e.message);
    // Declared but not installed (a fresh checkout): the refusal says to install.
    dir.write(
        "app/package.json",
        r#"{"dependencies":{"@acme/nothing":"1.0.0"}}"#,
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert!(
        e.message.contains("bun install --frozen-lockfile"),
        "{}",
        e.message
    );
    std::fs::remove_file(dir.0.join("app/package.json")).unwrap();
    // A workspace's own package, as exact2's root declares `@exact/reading`.
    dir.write(
        "package.json",
        r#"{"workspaces":["libs/*"],"dependencies":{"@acme/nothing":"workspace:*"}}"#,
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert!(
        e.message.contains("is declared in") && e.message.contains("bun install --frozen-lockfile"),
        "{}",
        e.message
    );
    // A bare file name is a mistaken path, not a package.
    dir.write(
        "app/app.contract",
        "use Card from \"card.contract\"\ncomponent App\n  view\n    text \"x\"\n",
    );
    assert_eq!(
        contract::compile_path(&root).unwrap_err().id,
        "contract-use-path"
    );
}

#[cfg(unix)]
#[test]
fn a_linked_local_library_outside_the_app_resolves_and_stays_inside_itself() {
    // `"@me/ui": "file:../ui"` installs a link; the package's files are its
    // own root, outside the app, and its `..` may not leave it.
    let dir = Dir::new("linked");
    library(&dir, "ui", r#"{"name":"@me/ui","version":"0.1.0"}"#);
    std::fs::create_dir_all(dir.0.join("app/node_modules/@me")).unwrap();
    std::os::unix::fs::symlink(dir.0.join("ui"), dir.0.join("app/node_modules/@me/ui")).unwrap();
    let root = dir.write(
        "app/app.contract",
        "use Card from \"@me/ui\"\ncomponent App\n  view\n    Card()\n",
    );
    contract::compile_path(&root).unwrap();
    let graph = contract::source_graph(&root);
    assert!(graph.errors.is_empty(), "{:?}", graph.errors);
    let files: Vec<_> = graph
        .sources
        .iter()
        .map(|s| s.path.strip_prefix(&dir.0).unwrap_or(&s.path).to_path_buf())
        .collect();
    assert_eq!(
        files,
        [
            Path::new("app/app.contract"),
            Path::new("ui/index.contract"),
            Path::new("ui/src/card.contract"),
            Path::new("ui/styles.contract"),
        ]
    );
    match &graph.sources[1].origin {
        contract::Origin::Package {
            name,
            version,
            root,
            manifest,
        } => {
            assert_eq!((name.as_str(), version.as_str()), ("@me/ui", "0.1.0"));
            assert_eq!(root, &dir.0.join("ui"));
            assert_eq!(manifest, &dir.0.join("ui/package.json"));
        }
        other => panic!("{other:?}"),
    }
    dir.write("outside.contract", "style Pad\n  padding-top=1\n");
    dir.write(
        "ui/styles.contract",
        "use Pad from \"../outside.contract\"\n",
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert_eq!(e.id, "contract-use-path", "{e}");
    assert!(
        e.message.contains("leaves the package `@me/ui`"),
        "{}",
        e.message
    );
    // A failed load still reports what it read, for a watcher.
    let graph = contract::source_graph(&root);
    assert_eq!(graph.errors.len(), 1);
    assert_eq!(graph.sources.len(), 4);
}

#[test]
fn an_app_file_may_use_dot_dot_inside_the_app() {
    let dir = Dir::new("dotdot");
    dir.write(
        "lib/row.contract",
        "component Row\n  view\n    text \"r\"\n",
    );
    dir.write(
        "screens/home.contract",
        "use Row from \"../lib/row.contract\"\ncomponent Home\n  view\n    Row()\n",
    );
    let root = dir.write(
        "app.contract",
        "use Home from \"./screens/home.contract\"\ncomponent App\n  view\n    Home()\n",
    );
    contract::compile_path(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn a_file_install_of_linked_files_is_the_library_where_its_files_are() {
    // Bun installs `"file:../ui"` as a directory of per-file links: the
    // package is found where the files really are, as Node's realpath does.
    let dir = Dir::new("file-install");
    library(&dir, "ui", r#"{"name":"@me/ui","version":"0.2.0"}"#);
    let installed = dir.0.join("app/node_modules/@me/ui");
    for file in [
        "package.json",
        "index.contract",
        "styles.contract",
        "src/card.contract",
    ] {
        let at = installed.join(file);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(dir.0.join("ui").join(file), at).unwrap();
    }
    let root = dir.write(
        "app/app.contract",
        "use Card from \"@me/ui\"\ncomponent App\n  view\n    Card()\n",
    );
    contract::compile_path(&root).unwrap();
    let graph = contract::source_graph(&root);
    match &graph.sources[1].origin {
        contract::Origin::Package { root, .. } => assert_eq!(root, &dir.0.join("ui")),
        other => panic!("{other:?}"),
    }
}

struct NoData;
impl exact_runner::DataSource for NoData {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
}

/// Every node under `root`, depth first, as the kernel holds them.
fn under(k: &exact_kernel::Kernel, root: u32) -> Vec<exact_kernel::NodeRef<'_>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let node = k.node(id).unwrap();
        stack.extend(node.children().into_iter().rev());
        out.push(node);
    }
    out
}

/// LLP 1116.000 D3: `exact:content` draws an app's content with the
/// platform's roles, and arranges itself for a phone and a wide window.
#[test]
fn exact_content_draws_a_hero_bars_an_empty_state_and_rows() {
    use exact_kernel::{NodeType, Offer, PropId};
    let dir = Dir::new("content");
    let root = dir.write(
        "app.contract",
        r#"use SummaryCard, StatBar, EmptyState, RowText from "exact:content"
component App
  action go
    let x = 1
  view
    column testId="root" width=900 height=2000
      column testId="narrow" width=300
        SummaryCard(label="Today", value="1,250 ml", detail="of 2,000 ml", progress=some(0.625), symbol=some("symbol:activity"))
      column testId="wide" width=800
        SummaryCard(label="Week", value="9,800 ml", detail="of 14,000 ml", progress=none, symbol=none)
      column testId="bars" width=400
        StatBar(label="Monday", value="1,800 ml", fraction=0.25, tint=some("-exact-system-green"))
        StatBar(label="Over", value="3,000 ml", fraction=1.5, tint=none)
      column testId="empty"
        EmptyState(symbol="symbol:calendar", title="Nothing yet", message="Log a glass.")
          button "Log a glass" press=go testId="first"
      column testId="bare"
        EmptyState(symbol="symbol:calendar", title="Bare", message="No action.")
      column testId="rows"
        RowText(title="Monday", detail="Oct 5", value=some("1,800 ml"))
        RowText(title="Tuesday", detail="", value=none)
"#,
    );
    let plan = contract::bake(contract::compile_path(&root).unwrap(), NoData).unwrap();
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel_mut();
    let at = |k: &exact_kernel::Kernel, t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id;
    let top = at(k, "root");
    k.compute_layout(top, Offer::definite(900.0, 2000.0))
        .unwrap();
    let k = r.kernel();
    let text = |n: &exact_kernel::NodeRef<'_>| n.props.str(PropId::Text).unwrap_or("").to_owned();
    let find = |t: &str, s: &str| {
        under(k, at(k, t))
            .into_iter()
            .find(|n| n.node_type == NodeType::Text && text(n) == s)
            .unwrap_or_else(|| panic!("no text {s:?} under {t}"))
    };
    let count = |t: &str, ty: NodeType| {
        under(k, at(k, t))
            .iter()
            .filter(|n| n.node_type == ty)
            .count()
    };
    let bars = |t: &str| {
        under(k, at(k, t))
            .iter()
            .filter(|n| n.props.str(PropId::AccessibilityRole) == Some("progressbar"))
            .count()
    };
    // The detail sits under the number on a phone, beside it on a wide window.
    let (value, detail) = (
        find("narrow", "1,250 ml").frame,
        find("narrow", "of 2,000 ml").frame,
    );
    assert!(
        detail.y >= value.y + value.height,
        "under: {value:?} {detail:?}"
    );
    let (value, detail) = (
        find("wide", "9,800 ml").frame,
        find("wide", "of 14,000 ml").frame,
    );
    assert!(
        detail.x >= value.x + value.width && detail.y < value.y + value.height,
        "beside: {value:?} {detail:?}"
    );
    assert_eq!(
        find("narrow", "1,250 ml")
            .props
            .str(PropId::AccessibilityLabel),
        Some("Today, 1,250 ml")
    );
    // The fraction is the platform's determinate progress; `none` draws none.
    assert_eq!(bars("narrow"), 1);
    assert_eq!(bars("wide"), 0);
    assert_eq!(count("narrow", NodeType::Image), 1);
    assert_eq!(count("wide", NodeType::Image), 0);
    // A bar is one meter to a reader, its fill the fraction of its track,
    // held to the track.
    let meters: Vec<_> = under(k, at(k, "bars"))
        .into_iter()
        .filter(|n| n.props.str(PropId::AccessibilityRole) == Some("meter"))
        .collect();
    assert_eq!(meters.len(), 2);
    assert_eq!(
        meters[0].props.str(PropId::AccessibilityLabel),
        Some("Monday")
    );
    assert_eq!(
        meters[0].props.str(PropId::AccessibilityValueText),
        Some("1,800 ml")
    );
    for (meter, share) in meters.iter().zip([0.25, 1.0]) {
        let track = k.node(*meter.children().last().unwrap()).unwrap();
        let fill = k.node(track.children()[0]).unwrap();
        assert!(
            (fill.frame.width - track.frame.width * share).abs() < 0.5,
            "{share}: {:?} {:?}",
            fill.frame,
            track.frame
        );
    }
    // The empty state's action is the author's own control, in its slot.
    assert!(!k.find_by_test_id("first").is_empty());
    assert_eq!(
        find("empty", "Nothing yet")
            .props
            .str(PropId::AccessibilityRole),
        Some("heading")
    );
    assert_eq!(count("bare", NodeType::Pressable), 0);
    // A row's detail and trailing value show only when given.
    find("rows", "Oct 5");
    find("rows", "1,800 ml");
    assert_eq!(
        count("rows", NodeType::Text),
        4,
        "Monday, Oct 5, 1,800 ml, Tuesday"
    );
}

/// LLP 1116.001 D2: `RingProgress` is a ring around its value, one
/// progress bar to a reader, and a state is a second colour: short of the
/// goal the accent, a goal met the system green, done against not done
/// green against grey; a `tint` says otherwise.
#[test]
fn exact_content_draws_a_ring_and_state_as_a_second_colour() {
    use exact_kernel::style::roles::role;
    use exact_kernel::{ColorValue, NodeType, Offer, PropId};
    let dir = Dir::new("content-state");
    let root = dir.write(
        "app.contract",
        r#"use SummaryCard, StatBar, RingProgress, DoneMark from "exact:content"
component App
  view
    column testId="root" width=900 height=2000
      column testId="wide" width=400
        RingProgress(label="Focus", value="17:30", fraction=0.3, tint=none)
      row testId="pair" width=370 flex-wrap="wrap" gap=16
        column flex="1 1 160px" testId="left"
          RingProgress(label="Water", value="2,000", fraction=1.2, tint=none)
        column flex="1 1 160px" testId="right"
          RingProgress(label="Tinted", value="0", fraction=0, tint=some("-exact-system-orange"))
      column testId="bars" width=400
        StatBar(label="Under", value="1", fraction=0.5, tint=none)
        StatBar(label="Met", value="2", fraction=1, tint=none)
        StatBar(label="Budget", value="3", fraction=1.2, tint=some("-exact-system-red"))
      column testId="card" width=400
        SummaryCard(label="Habits", value="3 / 3", detail="", progress=some(1), symbol=none)
      row testId="marks"
        DoneMark(caption="Mon", done=true)
        DoneMark(caption="Tue", done=false)
        DoneMark(caption="", done=true)
"#,
    );
    let plan = contract::bake(contract::compile_path(&root).unwrap(), NoData).unwrap();
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel_mut();
    let at = |k: &exact_kernel::Kernel, t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id;
    let top = at(k, "root");
    k.compute_layout(top, Offer::definite(900.0, 2000.0))
        .unwrap();
    let k = r.kernel();
    let named = |n: &str| ColorValue::Role(role(n).unwrap());
    let rings = |t: &str| -> Vec<exact_kernel::NodeRef<'_>> {
        under(k, at(k, t))
            .into_iter()
            .filter(|n| n.props.str(PropId::AccessibilityRole) == Some("progressbar"))
            .collect()
    };
    // One progress bar to a reader: its label, its words, its fraction.
    let ring = &rings("wide")[0];
    assert_eq!(ring.props.str(PropId::AccessibilityLabel), Some("Focus"));
    assert_eq!(
        ring.props.str(PropId::AccessibilityValueText),
        Some("17:30")
    );
    assert_eq!(ring.props.str(PropId::AccessibilityValueMax), Some("1"));
    // A square, 200 across where there is room; two side by side in a
    // phone's row, the ring adding no width of its own.
    assert_eq!((ring.frame.width, ring.frame.height), (200.0, 200.0));
    let (left, right) = (&rings("left")[0], &rings("right")[0]);
    assert!(
        left.frame.width < 200.0 && left.frame.width == left.frame.height,
        "{:?}",
        left.frame
    );
    let (l, r) = (
        k.node(at(k, "left")).unwrap().frame,
        k.node(at(k, "right")).unwrap().frame,
    );
    assert_eq!(l.y, r.y, "side by side: {l:?} {r:?}");
    assert_eq!(
        left.props.str(PropId::AccessibilityValueNow),
        Some("1"),
        "held to 1"
    );
    assert_eq!(right.props.str(PropId::AccessibilityValueNow), Some("0"));
    // The ring's colour is the svg's: the accent, green at the goal, the tint.
    let ink = |t: &str| {
        under(k, at(k, t))
            .into_iter()
            .find(|n| n.node_type == NodeType::Svg)
            .unwrap()
            .style
            .text_color
    };
    assert_eq!(ink("wide"), named("AccentColor"));
    assert_eq!(ink("left"), named("-exact-system-green"));
    assert_eq!(ink("right"), named("-exact-system-orange"));
    // A bar's fill: the accent short of its goal, green at it, the tint.
    let fills: Vec<_> = under(k, at(k, "bars"))
        .into_iter()
        .filter(|n| n.props.str(PropId::AccessibilityRole) == Some("meter"))
        .map(|m| {
            let track = k.node(*m.children().last().unwrap()).unwrap();
            k.node(track.children()[0]).unwrap().style.background_color
        })
        .collect();
    assert_eq!(
        fills,
        [
            Some(named("AccentColor")),
            Some(named("-exact-system-green")),
            Some(named("-exact-system-red"))
        ]
    );
    // A card's progress at its goal is green.
    let card = under(k, at(k, "card"))
        .into_iter()
        .find(|n| n.props.str(PropId::AccessibilityRole) == Some("progressbar"))
        .unwrap();
    assert_eq!(card.style.accent_color, Some(named("-exact-system-green")));
    // Done against not done: one image each, green against grey.
    let marks: Vec<_> = under(k, at(k, "marks"))
        .into_iter()
        .filter(|n| n.props.str(PropId::AccessibilityRole) == Some("img"))
        .collect();
    let names: Vec<_> = marks
        .iter()
        .map(|m| m.props.str(PropId::AccessibilityLabel).unwrap_or(""))
        .collect();
    assert_eq!(names, ["Mon, done", "Tue, not done", "Done"]);
    // The caption is what shows: short, under the mark (LLP 1116.003 D7).
    let captions: Vec<_> = under(k, at(k, "marks"))
        .into_iter()
        .filter(|n| n.node_type == NodeType::Text)
        .map(|n| n.props.str(PropId::Text).unwrap_or("").to_owned())
        .collect();
    assert_eq!(captions, ["Mon", "Tue"]);
    let tints: Vec<_> = marks
        .iter()
        .map(|m| {
            let image = under(k, m.id)
                .into_iter()
                .find(|n| n.node_type == NodeType::Image)
                .unwrap();
            (
                image.props.str(PropId::ImageSource).unwrap().to_owned(),
                image.style.tint_color,
            )
        })
        .collect();
    assert_eq!(
        tints,
        [
            (
                "symbol:select-fill".into(),
                Some(named("-exact-system-green"))
            ),
            ("symbol:circle".into(), Some(named("-exact-tertiary-label"))),
            (
                "symbol:select-fill".into(),
                Some(named("-exact-system-green"))
            ),
        ]
    );
}

/// LLP 1116.003 D4: emphasis is a part. `RoundButton` is the platform's own
/// button as an 80-point circle, filled with its `tint` (the neutral grey
/// for `none`, the hue at low opacity for a light one); `AnswerTile` is a
/// tile whose status is its fill, legible in both schemes: the green under
/// dark ink, the red under white, whatever the app's scheme. Both pass their
/// press, `disabled` and `testId` through.
#[test]
fn exact_content_draws_round_buttons_and_answer_tiles() {
    use exact_kernel::style::roles::role;
    use exact_kernel::{ColorValue, NodeType, Offer, PropId, PropValue};
    let dir = Dir::new("content-controls");
    let root = dir.write(
        "app.contract",
        r#"use RoundButton, AnswerTile from "exact:content"
component App
  state n = 0
  state picked = ""
  action add(k: number)
    n = n + k
  action pick(a: string)
    picked = a
  view
    column testId="root" width=400 height=900
      row justify-content="space-between"
        RoundButton(label="Lap", tint=none, press=add(1), disabled=false, testId="lap")
        RoundButton(label="Stop", tint=some("-exact-system-red"), press=add(10), disabled=false, testId="start")
        RoundButton(label="Reset", tint=none, press=add(100), disabled=true, testId="reset")
      row justify-content="space-between"
        RoundButton(label="Go", tint=some("-exact-system-green"), press=add(1000), disabled=false, testId="go")
        RoundButton(label="Wait", tint=some("-exact-system-yellow"), press=add(10000), disabled=true, testId="wait")
      for a in ["Lima", "Quito", "Bogota", "Caracas"] key=a
        AnswerTile(label=a, status=(picked == "" ? (a == "Caracas" ? "selected" : "idle") : a == "Quito" ? "correct" : a == picked ? "wrong" : "dimmed"), press=pick(a), testId=a)
"#,
    );
    let plan = contract::bake(contract::compile_path(&root).unwrap(), NoData).unwrap();
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let at = |k: &exact_kernel::Kernel, t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id;
    let layout = |r: &mut exact_runner::Runner<NoData>| {
        let k = r.kernel_mut();
        let top = at(k, "root");
        k.compute_layout(top, Offer::definite(400.0, 900.0))
            .unwrap();
    };
    layout(&mut r);
    let named = |n: &str| ColorValue::Role(role(n).unwrap());
    let k = r.kernel();
    // The platform's own button, a circle 80 across, filled with the tint
    // or the neutral grey.
    for (id, style, accent) in [
        ("lap", "gray", None),
        ("start", "filled", Some(named("-exact-system-red"))),
    ] {
        let b = k.node(at(k, id)).unwrap();
        assert_eq!(b.node_type, NodeType::Control, "{id}");
        assert_eq!(
            b.props.get(PropId::ButtonStyle),
            Some(&PropValue::Str(style.into())),
            "{id}"
        );
        assert_eq!(b.style.accent_color, accent, "{id}");
        assert_eq!((b.frame.width, b.frame.height), (80.0, 80.0), "{id}");
    }
    let reset = k.node(at(k, "reset")).unwrap();
    assert_eq!(reset.props.bool(PropId::Disabled), Some(true));
    // LLP 1116.008 D6: a light system hue fails 4.5:1 under the filled
    // button's white ink (green's is 2.2:1) and looks off-platform under black
    // (trivia's teal Start), so it draws the platform's tinted style, the hue at
    // low opacity under the label's own ink (the tertiary label once disabled,
    // as the neutral button's); the other tints keep the platform's filled look.
    for (id, label, hue, disabled, ink) in [
        ("go", "Go", "-exact-system-green", false, "-exact-label"),
        (
            "wait",
            "Wait",
            "-exact-system-yellow",
            true,
            "-exact-tertiary-label",
        ),
    ] {
        let b = k.node(at(k, id)).unwrap();
        assert_eq!(b.node_type, NodeType::Control, "{id}");
        assert_eq!(
            b.props.get(PropId::ButtonStyle),
            Some(&PropValue::Str("tinted".into())),
            "{id}"
        );
        assert_eq!(b.style.accent_color, Some(named(hue)), "{id}");
        assert_eq!((b.frame.width, b.frame.height), (80.0, 80.0), "{id}");
        assert_eq!(b.props.bool(PropId::Disabled), Some(disabled), "{id}");
        assert_eq!(b.child_nodes().next().unwrap().text_runs()[0].text, label);
        assert_eq!(b.style.text_color, named(ink), "{id}");
    }
    assert!(!matches!(
        k.node(at(k, "start")).unwrap().style.text_color,
        ColorValue::LightDark(..)
    ));
    // Its press is the author's action, with its captured arguments.
    let (lap, start) = (at(k, "lap"), at(k, "start"));
    r.dispatch(lap, exact_runner::Event::Press).unwrap();
    r.dispatch(start, exact_runner::Event::Press).unwrap();
    assert_eq!(r.slot("n"), Some(&exact_runner::Value::Number(11.0)));
    // Untold: a neutral fill, a target over 44 points; the selected one
    // carries the accent's outline and says so.
    let k = r.kernel();
    let lima = k.node(at(k, "Lima")).unwrap();
    assert_eq!(
        lima.style.background_color,
        Some(named("-exact-tertiary-fill"))
    );
    assert!(lima.frame.height >= 44.0, "{:?}", lima.frame);
    assert_ne!(lima.props.bool(PropId::Disabled), Some(true));
    let caracas = k.node(at(k, "Caracas")).unwrap();
    assert_eq!(
        caracas.props.str(PropId::AccessibilityLabel),
        Some("Caracas, selected")
    );
    let bogota = at(k, "Bogota");
    r.dispatch(bogota, exact_runner::Event::Press).unwrap();
    assert_eq!(r.slot("picked").and_then(|v| v.as_str()), Some("Bogota"));
    layout(&mut r);
    // Told: green under a light scheme (dark ink), red under a dark one
    // (white ink), the rest dimmed; none of them presses again.
    let k = r.kernel();
    let tile = |t: &str| k.node(at(k, t)).unwrap();
    let told = [
        (
            "Quito",
            "-exact-system-green",
            Some(false),
            "Quito, correct",
        ),
        ("Bogota", "-exact-system-red", Some(true), "Bogota, wrong"),
        ("Lima", "-exact-tertiary-fill", None, "Lima"),
    ];
    for (t, fill, dark, name) in told {
        let n = tile(t);
        assert_eq!(n.style.background_color, Some(named(fill)), "{t}");
        assert_eq!(n.color_scheme_dark(), dark, "{t}");
        assert_eq!(n.props.str(PropId::AccessibilityLabel), Some(name), "{t}");
        assert_eq!(n.props.bool(PropId::Disabled), Some(true), "{t}");
    }
    assert_eq!(tile("Quito").style.text_color, named("-exact-label"));
}

/// LLP 1116.004 D9: a status is a `Badge`, not coloured text: its label in a
/// capsule that keeps its own size at the start of a column and centred in a
/// row, never stretched; its ink legible on its fill in either scheme (dark
/// on a system hue, white on indigo, the platform's own on the accent and on
/// the neutral grey); and a reader hears its text.
#[test]
fn exact_content_draws_a_status_badge() {
    use exact_kernel::style::roles::role;
    use exact_kernel::{ColorValue, NodeType, Offer, PropId};
    let dir = Dir::new("content-badge");
    let root = dir.write(
        "app.contract",
        r#"use Badge from "exact:content"
component App
  view
    column testId="root" width=400 height=400
      column testId="col" width=300
        Badge(label="Locked", tint=none)
      row testId="row" width=380 height=60 gap=8
        text "Garden party" flex=1 min-width=0
        Badge(label="Full", tint=some("-exact-system-orange"))
        Badge(label="New", tint=some("AccentColor"))
        Badge(label="Indigo", tint=some("-exact-system-indigo"))
"#,
    );
    let plan = contract::bake(contract::compile_path(&root).unwrap(), NoData).unwrap();
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel_mut();
    let at = |k: &exact_kernel::Kernel, t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id;
    let top = at(k, "root");
    k.compute_layout(top, Offer::definite(400.0, 400.0))
        .unwrap();
    let k = r.kernel();
    let named = |n: &str| ColorValue::Role(role(n).unwrap());
    // Each capsule (the node right over a text) and its text.
    let capsules = |t: &str| -> Vec<(exact_kernel::NodeRef<'_>, exact_kernel::NodeRef<'_>)> {
        under(k, at(k, t))
            .into_iter()
            .filter_map(|c| {
                // A capsule holds its text, and the accent's tint a layer under it.
                let only = c.children();
                let text = k.node(*only.last()?).unwrap();
                (matches!(only.len(), 1 | 2) && text.node_type == NodeType::Text)
                    .then_some((c, text))
            })
            .collect()
    };
    let col = capsules("col");
    let (locked, label) = &col[0];
    assert_eq!(label.props.str(PropId::Text), Some("Locked"));
    assert!(
        locked.frame.width < 150.0 && locked.frame.x == 0.0,
        "its own width, at the column's start: {:?}",
        locked.frame
    );
    assert_eq!(
        locked.style.background_color,
        Some(named("-exact-tertiary-fill"))
    );
    assert_eq!(
        locked.color_scheme_dark(),
        None,
        "the grey follows the scheme"
    );
    assert_eq!(label.style.text_color, named("-exact-label"));
    let row = capsules("row");
    let words: Vec<&str> = row
        .iter()
        .map(|(_, t)| t.props.str(PropId::Text).unwrap())
        .collect();
    assert_eq!(words, ["Full", "New", "Indigo"]);
    for (c, _) in &row {
        assert!(
            c.frame.height < 40.0 && c.frame.y > 0.0,
            "centred in the row's height, not stretched: {:?}",
            c.frame
        );
    }
    let (full, new, indigo) = (&row[0], &row[1], &row[2]);
    assert_eq!(
        full.0.color_scheme_dark(),
        Some(false),
        "dark ink on orange"
    );
    assert_eq!(full.1.style.text_color, named("-exact-label"));
    assert_eq!(indigo.0.color_scheme_dark(), Some(true), "white on indigo");
    // LLP 1116.007 D3.10: the accent is a tint, not a fill under white ink (1.4:1 on
    // yellow, 3.5:1 on the system blue, against a caption's 4.5:1): the accent at 16% on a
    // layer under the label, which keeps its own ink, so it reads on every accent.
    assert_eq!(new.0.color_scheme_dark(), None);
    assert_eq!(new.1.style.text_color, named("-exact-label"));
    let tint = k.node(new.0.children()[0]).unwrap();
    assert_eq!(tint.style.background_color, Some(named("AccentColor")));
    assert!(
        (tint.style.opacity - 0.16).abs() < 1e-3,
        "{}",
        tint.style.opacity
    );
    assert!(
        new.0
            .style
            .background_color
            .is_none_or(|c| c.resolve(false).a() == 0),
        "the capsule paints nothing of its own: the layer is the tint"
    );
}

/// LLP 1116.007 D3.10: `SummaryCard` cannot know its page, and the secondary
/// background is the grouped page's own grey in the light scheme (the card
/// vanished on unit-converter's grouped page, `check`'s `card` finding). Its
/// surface is a translucent fill, which differs from the plain page and from
/// the grouped page, by more than `check`'s four-step tolerance, in each
/// scheme.
#[test]
fn a_summary_card_shows_on_a_plain_page_and_on_a_grouped_one_in_either_scheme() {
    use exact_kernel::style::roles::role;
    use exact_kernel::ColorValue;
    let dir = Dir::new("card-surface");
    let root = dir.write(
        "app.contract",
        r#"use SummaryCard from "exact:content"
component App
  view
    column testId="root" width=400 height=400
      column testId="holder"
        SummaryCard(label="Today", value="1", detail="of 2", progress=none, symbol=none)
"#,
    );
    let plan = contract::bake(contract::compile_path(&root).unwrap(), NoData).unwrap();
    let r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let holder = k.node_by_key(k.find_by_test_id("holder")[0]).unwrap();
    let card = k.node(holder.children()[0]).unwrap();
    let fill = card.style.background_color.expect("the card has a fill");
    for dark in [false, true] {
        let over = |page: ColorValue| {
            let (f, p) = (fill.resolve(dark), page.resolve(dark));
            let a = f64::from(f.a()) / 255.0;
            let mix = |f: u8, p: u8| f64::from(f) * a + f64::from(p) * (1.0 - a);
            [mix(f.r(), p.r()), mix(f.g(), p.g()), mix(f.b(), p.b())]
        };
        for page in ["-exact-background", "-exact-grouped-background"] {
            let page = ColorValue::Role(role(page).unwrap());
            let (shown, behind) = (over(page), page.resolve(dark));
            let behind = [behind.r(), behind.g(), behind.b()].map(f64::from);
            let apart = (0..3).any(|i| (shown[i] - behind[i]).abs() > 4.0);
            assert!(
                apart,
                "the card {shown:?} is its page {behind:?} (dark: {dark})"
            );
        }
    }
}
