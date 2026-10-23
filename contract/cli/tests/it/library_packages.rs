//! @ref LLP 1139 D1, D3 (stage 12 E) — the shipped packages, built from
//! their sources in this repository: an app that sets @exact/actions'
//! PostButton's `fill`, `dim` and `pressed` sees each on its look, since a
//! `:disabled` row beats the node's own base row and `:active` is the host's.

use exact_kernel::{Kernel, StyleId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
use std::path::{Path, PathBuf};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A temporary app with the repository's `packages/<name>` copied into its
/// `node_modules/@exact/<name>`.
fn app_with(name: &str, packages: &[&str], design: &str, contract: &str) -> (Dir, PathBuf) {
    let dir =
        Dir(std::env::temp_dir().join(format!("exact-packages-{name}-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir.0);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
    for package in packages {
        let to = dir.0.join("app/node_modules/@exact").join(package);
        std::fs::create_dir_all(&to).unwrap();
        for entry in std::fs::read_dir(repo.join(package)).unwrap() {
            let from = entry.unwrap().path();
            std::fs::copy(&from, to.join(from.file_name().unwrap())).unwrap();
        }
    }
    std::fs::write(dir.0.join("app/app.tamber"), design).unwrap();
    let app = dir.0.join("app/app.contract");
    std::fs::write(&app, contract).unwrap();
    (dir, app)
}

fn background(r: &Runner<NoData>, test_id: &str) -> String {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id)[0];
    format!(
        "{:?}",
        k.node_by_key(key)
            .unwrap()
            .computed(StyleId::BackgroundColor)
    )
}

fn boot(app: &Path) -> Runner<NoData> {
    let plan = contract::compile_path(app).unwrap_or_else(|e| panic!("{e:?}"));
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

const BUTTONS: &str = "use PostButton from \"@exact/actions\"\n\ncomponent App\n  action press\n  view\n    column\n      PostButton(id=\"on\", label=\"Post\", reply=false, disabled=false, press=press)\n      PostButton(id=\"off\", label=\"Post\", reply=false, disabled=true, press=press)\n";

const POST_BUTTON: &str = "@exact/actions/post-button.contract#PostButton";

#[test]
fn an_apps_post_button_fill_dim_and_pressed_reach_its_three_looks() {
    let (_dir, app) = app_with(
        "set",
        &["actions"],
        &format!("tamber app\n\nparams\n  {POST_BUTTON}.fill = #ff0000  scheme=fixed\n  {POST_BUTTON}.dim = #00ff00  scheme=fixed\n  {POST_BUTTON}.pressed = #0000ff  scheme=fixed\n"),
        BUTTONS,
    );
    // The press paint is the host's: the `:active` row names the param's
    // token, which carries the app's value.
    let plan = contract::compile_path(&app).unwrap_or_else(|e| panic!("{e:?}"));
    let pressed = contract_lower::params::token_name(POST_BUTTON, "pressed");
    let row = plan
        .theme
        .iter()
        .find(|t| plan.str(t.name) == pressed)
        .unwrap_or_else(|| panic!("no {pressed} in the plan"));
    assert_eq!(plan.str(row.value), "#0000ffff");
    assert!(
        plan.bindings
            .iter()
            .any(|b| b.id == StyleId::PressBackgroundColor as u16),
        "the :active row, the press's background"
    );
    // The fill on the enabled button, the dim on the disabled one (RGBA).
    let r = boot(&app);
    assert_eq!(
        background(&r, "compose-post-on"),
        "ColorValue(Fixed(Color(4278190335)))",
        "#ff0000"
    );
    assert_eq!(
        background(&r, "compose-post-off"),
        "ColorValue(Fixed(Color(16711935)))",
        "#00ff00, which `:disabled` reads"
    );
}

const ROWS: &str = "use ListRow from \"@exact/lists\"\n\ncomponent App\n  action press\n  view\n    column\n      list appearance=\"auto\" listStyle=\"inset-grouped\" flex=1\n        section\n          ListRow(id=\"plain\", symbol=\"settings\", title=\"Settings\", value=\"\", accessory=\"chevron\", selected=false, destructive=false, press=press)\n          ListRow(id=\"red\", symbol=\"delete\", title=\"Delete\", value=\"\", accessory=\"chevron\", selected=false, destructive=true, press=press)\n";

/// @exact/lists' ListRow (stage 13 C): its symbol takes the tint param,
/// but a destructive row's symbol is the row's red, as its title is.
#[test]
fn a_list_rows_symbol_takes_its_tint_but_a_destructive_rows_is_its_red() {
    let (_dir, app) = app_with("lists", &["lists"], "tamber app\n", ROWS);
    let r = boot(&app);
    let k = r.kernel();
    let node = |test_id: &str| k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap();
    // A symbol is drawn in its `color` (LLP 1081 stage 2): the param's on a
    // plain row, the row's red, inherited, on a destructive one.
    let symbol_tint = |row: &str| k.node(node(row).children()[0]).unwrap().text_color();
    let ink = |row: &str| k.node(node(row).children()[1]).unwrap().text_color();
    assert_ne!(symbol_tint("red"), symbol_tint("plain"), "not the param's");
    assert_eq!(
        symbol_tint("red"),
        ink("red"),
        "the row's red, as its title"
    );
}
