//! A package carries a view, and its implementation hears whether the view
//! is displayed and who made it so (LLP 1133.001 D1, LLP 1133.008 D8 items
//! 1 and 2), through `@fixture/swatch`, a package written beside the app as
//! an installed one, whose implementation the test plays as the host's
//! native slot.

use exact_runner::{CallOrigin, Reply, Runner};
use serde_json::{json, Value as Json};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const INTERFACE: &str = r#"{
  "kind": "interface",
  "name": "@fixture/swatch",
  "version": "1.0.0",
  "views": {
    "swatch-box": { "props": { "tint": { "type": "string", "binds": true }, "caption": "option<string>" },
      "events": ["message"], "displayed": true }
  },
  "names": []
}"#;

const APP: &str = r#"component App
  state shown = false
  state hidden = false
  state tint = "red"
  action show
    shown = true
  action unshow
    shown = false
  action hide
    hidden = true
  action recolor
    tint = "blue"
  view
    column
      if shown
        swatch-box tint=tint caption="a" display=(hidden ? "none" : "block") testId="box"
"#;

struct App(PathBuf);
impl App {
    /// The app, depending on `@fixture/swatch` and on each of `others`
    /// (a package name and its `exact.json`).
    fn new(name: &str, app: &str, others: &[(&str, &str)]) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-views-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let mut deps = serde_json::Map::new();
        for (package, declaration) in [("@fixture/swatch", INTERFACE)].iter().chain(others) {
            let at = path.join("node_modules").join(package);
            std::fs::create_dir_all(&at).unwrap();
            let manifest =
                json!({ "name": package, "version": "1.0.0", "exports": { ".": "./exact.json" } });
            std::fs::write(at.join("package.json"), manifest.to_string()).unwrap();
            std::fs::write(at.join("exact.json"), declaration).unwrap();
            deps.insert(package.to_string(), json!("1.0.0"));
        }
        let manifest = json!({ "name": "app", "dependencies": deps });
        std::fs::write(path.join("package.json"), manifest.to_string()).unwrap();
        std::fs::write(path.join("app.contract"), app).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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

type Told = Arc<Mutex<Vec<Json>>>;

/// A runner over the app, every body its native slot is given kept.
fn boot(app: &App) -> (Runner<NoData>, Told) {
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let r = Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let told: Told = Arc::default();
    let seen = told.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            seen.lock()
                .unwrap()
                .push(serde_json::from_slice(&body).unwrap());
            drop(reply);
        })));
    (r, told)
}

/// What the implementation was told since the last call, each report as
/// `displayed <shown> <origin>` or `bound <prop> <origin>`.
fn reports(told: &Told) -> Vec<String> {
    (told.lock().unwrap().drain(..))
        .filter(|b| b["view"].is_string())
        .map(|b| {
            assert_eq!(b["extension"], "swatch");
            assert_eq!(b["tag"], "swatch-box");
            match &b["displayed"] {
                Json::Bool(shown) => format!("displayed {shown} {}", b["origin"].as_str().unwrap()),
                _ => format!(
                    "bound {} {}",
                    b["bound"].as_str().unwrap(),
                    b["origin"].as_str().unwrap()
                ),
            }
        })
        .collect()
}

fn view_id(r: &Runner<NoData>) -> u32 {
    let state = r.extensions_json().unwrap();
    let state: Json = serde_json::from_str(&state).unwrap();
    state["swatch"]["views"][0]["view"].as_u64().unwrap() as u32
}

/// The plan carries the view's row: its tag, the extension that makes it,
/// that it hears its displayed state, and its one binding prop.
#[test]
fn the_plan_names_a_package_s_view_and_its_extension() {
    let app = App::new("plan", APP, &[]);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let row = plan.view_named("swatch-box").expect("a views row");
    assert_eq!(plan.str(row.extension), "swatch");
    assert!(row.displayed);
    assert_eq!(plan.str(row.binds), "tint");
}

/// A view the app's own commit shows is reported displayed with the app's
/// origin; a tool's commit that rebinds and hides it, with the tool's; one
/// that leaves the tree while shown is reported hidden.
#[test]
fn a_view_hears_each_change_with_its_commit_s_origin() {
    let app = App::new("origin", APP, &[]);
    let (mut r, told) = boot(&app);
    r.bind_extensions(&["swatch"], &[]);
    assert!(reports(&told).is_empty(), "nothing is placed yet");

    r.act("show", Vec::new()).unwrap();
    assert_eq!(reports(&told), ["displayed true app", "bound tint app"]);
    let id = view_id(&r);
    assert_eq!(r.view_origin(id, None), Some(CallOrigin::App));

    r.act_from(CallOrigin::Tool, "recolor", Vec::new()).unwrap();
    assert_eq!(reports(&told), ["bound tint tool"]);
    assert_eq!(r.view_origin(id, Some("tint")), Some(CallOrigin::Tool));
    assert_eq!(
        r.view_origin(id, None),
        Some(CallOrigin::App),
        "still shown by the app's commit"
    );

    r.act_from(CallOrigin::Tool, "hide", Vec::new()).unwrap();
    assert_eq!(reports(&told), ["displayed false tool"]);
    assert_eq!(r.view_origin(id, None), None);

    r.act("unshow", Vec::new()).unwrap();
    assert!(
        reports(&told).is_empty(),
        "a hidden view leaves without a report"
    );
}

/// A view a tool's commit shows is reported with the tool's origin, which
/// the runner keeps: a capture start that follows is admitted with it.
#[test]
fn a_view_a_tool_shows_is_reported_with_the_tool_s_origin() {
    let app = App::new("tool", APP, &[]);
    let (mut r, told) = boot(&app);
    r.bind_extensions(&[], &["swatch"]);
    r.act_from(CallOrigin::Tool, "show", Vec::new()).unwrap();
    assert_eq!(reports(&told), ["displayed true tool", "bound tint tool"]);
    assert_eq!(r.view_origin(view_id(&r), None), Some(CallOrigin::Tool));
    r.act("unshow", Vec::new()).unwrap();
    assert_eq!(reports(&told), ["displayed false app"]);
    let state = r.extensions_json().unwrap();
    assert!(state.contains(r#""views":[]"#), "{state}");
}

/// The host binds its implementations after first pixel: a view shown
/// before then is told when its extension is bound, with the kept origin.
#[test]
fn an_extension_bound_late_is_told_its_views_with_their_kept_origins() {
    let app = App::new("late", APP, &[]);
    let (mut r, told) = boot(&app);
    r.act_from(CallOrigin::Tool, "show", Vec::new()).unwrap();
    r.act("recolor", Vec::new()).unwrap();
    assert!(reports(&told).is_empty(), "no implementation is bound");
    r.bind_extensions(&["swatch"], &[]);
    assert_eq!(reports(&told), ["displayed true tool", "bound tint app"]);
}

/// The bake finds a tag in the packages the app depends on: one declared
/// by two packages, or by a package and `app.json` `modules`, is refused
/// naming both; one declared by none is `bake-unknown-module`.
#[test]
fn the_bake_refuses_a_tag_declared_twice_or_by_no_one() {
    let refused = |app: &App| {
        let errors = contract::compile_path_all(&app.0.join("app.contract"), false)
            .err()
            .expect("refused");
        (errors[0].id.clone(), errors[0].message.clone())
    };
    let twice = r#"{ "kind": "interface", "name": "@other/box", "version": "1.0.0",
      "views": { "swatch-box": { "props": {} } }, "names": [] }"#;
    let app = App::new("twice", APP, &[("@other/box", twice)]);
    let (id, message) = refused(&app);
    assert_eq!(id, "bake-view-twice");
    assert!(
        message.contains("@fixture/swatch") && message.contains("@other/box"),
        "{message}"
    );

    let app = App::new("modules", APP, &[]);
    let manifest =
        r#"{ "app": { "id": "com.exact.views", "name": "Views" }, "modules": ["swatch-box"] }"#;
    std::fs::write(app.0.join("app.json"), manifest).unwrap();
    let (id, message) = refused(&app);
    assert_eq!(id, "bake-view-twice");
    assert!(message.contains("app.json"), "{message}");

    let typo = APP.replace("swatch-box tint", "swatch-bx tint");
    let app = App::new("unknown", &typo, &[]);
    let (id, message) = refused(&app);
    assert_eq!(id, "bake-unknown-module");
    assert!(message.contains("did you mean `swatch-box`"), "{message}");
}
