//! `contract tamber gen` (LLP 1121 D12, stage 5, commit 4): every language's
//! generated names over the one table, from the design's IR.

use exact_kernel::{ColorValue, Kernel};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Place, Runner};
use std::path::{Path, PathBuf};
use std::process::Command;

/// What `contract tamber gen rust` writes for [`DESIGN`], kept as the test's
/// own file (an app's never is): it compiles here, and the generator still
/// writes it byte for byte.
#[path = "tamber_gen_golden.rs"]
mod golden;

const DESIGN: &str = "tamber shelf\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  brand = #c8102e\n  surface.card = light-dark(#ffffff, #000000)\n    when dark-style=dim: light-dark(#ffffff, #161e27)\n  CanvasText = #101010\n";

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn app(name: &str, design: &str, view: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-gen-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), view).unwrap();
    dir
}

fn gen(target: &str, dir: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["tamber", "gen", target])
        .arg(dir)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
    )
}

/// The generated files go in the app's `.exact/tamber/`, the same bytes for
/// an unchanged design (and not written again), the Rust as the golden.
#[test]
fn the_generated_files_are_byte_stable_in_the_apps_exact_directory() {
    let dir = app("stable", DESIGN, "component App\n  view\n    column\n");
    let (ok, said) = gen("rust", &dir);
    assert!(ok, "{said}");
    let rust = std::fs::read_to_string(dir.join(".exact/tamber/tamber.rs")).unwrap();
    assert_eq!(rust, include_str!("tamber_gen_golden.rs"));
    let (ok, said) = gen("ts", &dir);
    assert!(ok, "{said}");
    let ts = std::fs::read_to_string(dir.join(".exact/tamber/tamber.ts")).unwrap();
    let css = std::fs::read_to_string(dir.join(".exact/tamber/tamber.css")).unwrap();
    assert!(
        ts.contains(r#"export type ColorToken = "brand" | "surface.card" | "CanvasText";"#),
        "{ts}"
    );
    assert!(ts.contains(r#"export const surfaceCard = { name: "surface.card", css: ["light-dark(#ffffffff, #000000ff)", "light-dark(#ffffffff, #161e27ff)"] } as const;"#), "{ts}");
    assert!(css.contains(r#"[data-tamber="dark-style=dim"]{--brand:#c8102eff;--surface-card:light-dark(#ffffffff, #161e27ff);"#), "{css}");
    // Again: nothing written, nothing said.
    let (ok, said) = gen("ts", &dir);
    assert!(ok && said.is_empty(), "{said}");
    assert_eq!(
        std::fs::read_to_string(dir.join(".exact/tamber/tamber.ts")).unwrap(),
        ts
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Rust constants compile (the golden module above) and resolve through
/// the session's binding core, as the design and the place say.
#[test]
fn the_rust_constants_resolve_through_the_binding_core() {
    let dir = app(
        "rust",
        DESIGN,
        "component App\n  view\n    column background-color=\"surface.card\"\n      text \"a\" color=\"brand\"\n      column -exact-mode=\"dark-style=dim\" background-color=\"surface.card\" testId=\"dim\"\n",
    );
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let mut r = Runner::boot(
        contract::bake(plan, NoData).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let dim = {
        let k = r.kernel();
        Place::Node(k.node_by_key(k.find_by_test_id("dim")[0]).unwrap().id)
    };
    let parsed = |t: &str| {
        ColorValue::parse_light_dark(t)
            .or_else(|| exact_kernel::Color::parse(t).map(ColorValue::Fixed))
    };
    let mut t = r.tamber();
    assert_eq!(t.get(golden::BRAND, Place::Root), parsed("#c8102e"));
    assert_eq!(
        t.get(golden::SURFACE_CARD, Place::Root),
        parsed("light-dark(#ffffff, #000000)")
    );
    assert_eq!(
        t.get(golden::SURFACE_CARD, dim),
        parsed("light-dark(#ffffff, #161e27)")
    );
    assert_eq!(t.get(golden::CANVAS_TEXT, Place::Root), parsed("#101010"));
}

/// Names that collide after mapping are refused, naming both; so is a name
/// no binding can take.
#[test]
fn a_collision_after_mapping_is_refused() {
    let dir = app(
        "collide",
        "tamber t\n\ntheme\n  surface.card = #ffffff\n  surface-card = #000000\n",
        "component App\n  view\n    column\n",
    );
    let (ok, said) = gen("ts", &dir);
    assert!(!ok);
    assert!(
        said.contains("`surface.card` and `surface-card` are both `surfaceCard` in camelCase"),
        "{said}"
    );
    assert!(
        !dir.join(".exact/tamber/tamber.ts").exists(),
        "nothing written"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let dir = app(
        "reserved",
        "tamber t\n\ntheme\n  default = #ffffff\n",
        "component App\n  view\n    column\n",
    );
    let (ok, said) = gen("rust", &dir);
    assert!(
        !ok && said.contains("`default` is `default` in camelCase"),
        "{said}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `contract types`' declarations name the design's colour tokens as a
/// union, so a data module's token is typed (the `tsc` check is the async
/// lane's, beside the other declaration checks).
#[test]
fn the_types_carry_a_colour_token_union() {
    let dir = app(
        "types",
        DESIGN,
        "component App\n  view\n    column background-color=\"surface.card\"\n      text \"a\" color=\"brand\"\n",
    );
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let types = contract::typescript(&plan).unwrap();
    assert!(
        types.contains(r#"export type ColorToken = "brand" | "surface.card" | "CanvasText";"#),
        "{types}"
    );
}

/// A design's `exports` bounds the generated names, as it bounds what a
/// plan keeps for bindings (D7): every constant resolves.
#[test]
fn exports_bound_the_generated_names() {
    let dir = app(
        "exports",
        "tamber t\n\nexports\n  badge.ink\n\ntheme\n  brand = #c8102e\n  badge.ink = #123456\n",
        "component App\n  view\n    column\n",
    );
    let (ok, said) = gen("rust", &dir);
    assert!(ok, "{said}");
    let rust = std::fs::read_to_string(dir.join(".exact/tamber/tamber.rs")).unwrap();
    assert!(rust.contains("pub const BADGE_INK: Token<Color>"), "{rust}");
    assert!(!rust.contains("BRAND"), "{rust}");
    let _ = std::fs::remove_dir_all(&dir);
}
