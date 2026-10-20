//! @ref LLP 1139 D0, D4 (stage 12 C2) — an exported library component's
//! slot: `children` placed once, unconditionally, never forwarded; and no
//! node of the slot's chain (the view's root down to the node holding it)
//! hiding the fill, by a row of any class branch or state layer, an
//! attribute, or a token's value in any tuple. A component of an app, or a
//! package's that isn't a library (not private, no design), is not held to
//! it.

use contract::CompileError;
use std::path::{Path, PathBuf};

struct Dir(PathBuf);
impl Dir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("exact-slot-{name}-{}", std::process::id()));
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

const DESIGN: &str =
    "tamber lib\n\ntheme\n  lib.pad = 4px\n  lib.op = 1\n    when density=tight: 0\n";
const APP_DESIGN: &str = "tamber app\n\naxes\n  density = roomy | tight\n";

/// The errors of an app that uses `Card` from a package `@acme/lib` whose
/// entry is `card` (a `Card` component's source); `private` and `design`
/// say whether it is a library.
fn build(name: &str, card: &str, private: bool, design: bool) -> Vec<CompileError> {
    let dir = Dir::new(name);
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        &format!(
            r#"{{"name":"@acme/lib","version":"1.0.0","private":{private},"exports":{{".":"./index.contract"}}}}"#
        ),
    );
    dir.write(&format!("{at}/index.contract"), card);
    if design {
        dir.write(&format!("{at}/lib.tamber"), DESIGN);
    }
    dir.write("app/app.tamber", APP_DESIGN);
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
    );
    let src = std::fs::read_to_string(&app).unwrap();
    match contract::compile_path_source_all(Path::new(&app), &src, false) {
        Ok(_) => Vec::new(),
        Err(all) => all,
    }
}

fn card(body: &str) -> String {
    format!("component Card\n  slot\n  view\n{body}")
}

fn with<'e>(all: &'e [CompileError], id: &str) -> Vec<&'e CompileError> {
    all.iter().filter(|e| e.id == id).collect()
}

#[test]
fn a_library_slot_placed_once_and_shown_compiles() {
    let all = build(
        "ok",
        &card("    column padding=\"lib.pad\" opacity=1\n      children\n"),
        true,
        true,
    );
    assert!(all.is_empty(), "{all:?}");
}

#[test]
fn the_slot_is_placed_once_unconditionally_never_forwarded() {
    let id = "lower-anatomy-slot-placement";
    for (name, body, said) in [
        ("none", "    column\n      text \"a\"\n", "places no `children`"),
        ("twice", "    column\n      children\n      children\n", "again"),
        ("if", "    column\n      if true\n        children\n", "inside an `if`"),
        (
            "for",
            "    column\n      for x in [\"a\"] key=x\n        children\n",
            "inside a `for`",
        ),
        (
            "match",
            "    column\n      match none\n        case some(x)\n          children\n        case none\n          text \"a\"\n",
            "inside a `match`",
        ),
        (
            "forwarded",
            "    column\n      Box()\n        children\n\ncomponent Box\n  slot\n  view\n    column\n      children\n",
            "another component's slot",
        ),
    ] {
        let all = build(&format!("place-{name}"), &card(body), true, true);
        let got = with(&all, id);
        assert!(
            got.iter().any(|e| e.message.contains(said)),
            "{name}: {all:?}"
        );
    }
}

#[test]
fn no_node_of_the_chain_hides_the_fill() {
    let id = "lower-anatomy-slot";
    for (name, row) in [
        ("display", "display=\"none\""),
        ("visibility", "visibility=\"hidden\""),
        ("opacity", "opacity=0"),
        ("width", "width=\"0px\""),
        ("height", "height=0"),
        ("max-width", "max-width=\"0px\""),
        ("max-height", "max-height=0"),
        ("scale", "scale=0"),
        ("font-size", "font-size=\"0px\""),
        ("line-height", "line-height=0"),
        ("hidden", "hidden=true"),
        ("inert", "inert=true"),
        ("aria-hidden", "aria-hidden=\"true\""),
        ("popover", "popover=\"auto\""),
    ] {
        // On the root of the chain, and on the node holding `children`.
        for (at, body) in [
            (
                "root",
                format!("    column {row}\n      column\n        children\n"),
            ),
            (
                "holder",
                format!("    column\n      column {row}\n        children\n"),
            ),
        ] {
            let all = build(&format!("deny-{name}-{at}"), &card(&body), true, true);
            assert!(!with(&all, id).is_empty(), "{name} on the {at}: {all:?}");
        }
    }
    // A node off the chain may hide itself.
    let all = build(
        "off-chain",
        &card("    column\n      text \"x\" opacity=0\n      column\n        children\n"),
        true,
        true,
    );
    assert!(with(&all, id).is_empty(), "{all:?}");
}

#[test]
fn a_class_branch_a_state_layer_and_a_token_tuple_count() {
    let id = "lower-anatomy-slot";
    // A conditional class's other branch.
    let all = build(
        "class",
        &(card("    column class=(true ? Shown : Gone)\n      children\n")
            + "\nstyle Shown\n  opacity = \"1\"\n\nstyle Gone\n  display = \"none\"\n"),
        true,
        true,
    );
    assert!(!with(&all, id).is_empty(), "{all:?}");
    // A state layer.
    let all = build(
        "state",
        &(card("    button class=Dim appearance=\"none\"\n      children\n")
            + "\nstyle Dim\n  :disabled\n    opacity = \"0\"\n"),
        true,
        true,
    );
    assert!(!with(&all, id).is_empty(), "{all:?}");
    // A token that is 0 in one tuple (density=tight).
    let all = build(
        "token",
        &card("    column opacity=\"lib.op\"\n      children\n"),
        true,
        true,
    );
    let got = with(&all, id);
    assert!(got.iter().any(|e| e.message.contains("lib.op")), "{all:?}");
    // A value the check can't fold.
    let all = build(
        "computed",
        &card("    column opacity=(true ? 1 : 0.5) width=(\"1\" + \"0px\")\n      children\n"),
        true,
        true,
    );
    let got = with(&all, id);
    assert!(
        got.iter().any(|e| e.message.contains("can't fold")),
        "{all:?}"
    );
}

#[test]
fn only_an_exported_library_component_is_held_to_it() {
    let hidden = card("    column opacity=0\n      children\n");
    // Not private, or no design: not a library.
    for (name, private, design) in [("public", false, true), ("nodesign", true, false)] {
        let all = build(name, &hidden, private, design);
        assert!(
            with(&all, "lower-anatomy-slot").is_empty(),
            "{name}: {all:?}"
        );
    }
    // A library's internal helper, which its entry neither declares nor re-exports.
    let dir = Dir::new("helper");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        &format!("{at}/index.contract"),
        "use Inner from \"./inner.contract\"\n\ncomponent Card\n  view\n    Inner()\n",
    );
    dir.write(&format!("{at}/inner.contract"), "component Inner\n  view\n    Hidden()\n      text \"a\"\n\ncomponent Hidden\n  slot\n  view\n    column opacity=0\n      children\n");
    dir.write(&format!("{at}/lib.tamber"), DESIGN);
    dir.write("app/app.tamber", APP_DESIGN);
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n",
    );
    let src = std::fs::read_to_string(&app).unwrap();
    let all = match contract::compile_path_source_all(Path::new(&app), &src, false) {
        Ok(_) => Vec::new(),
        Err(all) => all,
    };
    assert!(with(&all, "lower-anatomy-slot").is_empty(), "{all:?}");
}

/// A component the entry re-exports (`use` from another file of the
/// package) is an exported library component; `font`'s size and an SVG
/// element's collapsing `transform` on the chain count too.
#[test]
fn a_re_export_font_and_svg_transform_count() {
    let dir = Dir::new("reexport");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        &format!("{at}/index.contract"),
        "use Card from \"./card.contract\"\n",
    );
    dir.write(
        &format!("{at}/card.contract"),
        "component Card\n  slot\n  view\n    column font=\"0px/1 system-ui\"\n      children\n",
    );
    dir.write(&format!("{at}/lib.tamber"), DESIGN);
    dir.write("app/app.tamber", APP_DESIGN);
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
    );
    let src = std::fs::read_to_string(&app).unwrap();
    let all = match contract::compile_path_source_all(Path::new(&app), &src, false) {
        Ok(_) => Vec::new(),
        Err(all) => all,
    };
    let got = with(&all, "lower-anatomy-slot");
    assert!(
        got.iter().any(|e| e.message.contains("font-size")),
        "{all:?}"
    );
    assert!(contract_lower::anatomy::slot::denied("transform", "scale(1, 0)").is_some());
    assert!(contract_lower::anatomy::slot::denied("transform", "matrix(1 2 2 4 0 0)").is_some());
    assert!(contract_lower::anatomy::slot::denied("transform", "scale(2)").is_none());
}

/// The Tamber IR file records an exported library component's slot chain:
/// each node's deny-list rows with their values as written, what the design
/// server asks the one checker of on a candidate fold (LLP 1139 D4, D5).
#[test]
fn the_ir_records_the_slot_chain() {
    let dir = Dir::new("ir");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        &format!("{at}/index.contract"),
        &card("    column opacity=\"lib.op\" padding=\"lib.pad\"\n      column hidden=false\n        children\n"),
    );
    dir.write(&format!("{at}/lib.tamber"), DESIGN);
    dir.write("app/app.tamber", APP_DESIGN);
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
    );
    let params = contract::tamber_params::params_of(&app);
    let design = exact_tamber::parse(APP_DESIGN).unwrap();
    let ir = contract::tamber_params::library(&params, Some(&design));
    assert_eq!(ir[0]["identity"], "@acme/lib/index.contract#Card", "{ir}");
    assert_eq!(
        ir[0]["chain"],
        serde_json::json!([
            {"tag": "column", "rows": [["opacity", "lib.op"]], "tokens": ["lib.op"]},
            {"tag": "column", "rows": [["hidden", "false"]], "tokens": []},
        ]),
        "{ir}"
    );
}

/// Round 1 (Astra, Grok): a param's folded value; a base name in every
/// column (the terminal's `radius.sm` is 0); a math function the kernel
/// folds to 0; `0em`; the node's own row over its class's in one layer; and
/// the one checker is the whole rule (`font`, attributes, transforms, a
/// computation).
#[test]
fn round_1_values() {
    let id = "lower-anatomy-slot";
    let param = "component Card\n  slot\n  params\n    fade: number = \"lib.op\"\n  view\n    column opacity=\"param.fade\"\n      children\n";
    let all = build("param", param, true, true);
    assert!(
        with(&all, id).iter().any(|e| e.message.contains("vanish")),
        "{all:?}"
    );
    for (name, row) in [
        ("base", "width=\"radius.sm\""),
        ("calc", "width=\"calc(0px)\""),
        ("min", "max-height=\"min(0px, 10px)\""),
        ("em", "height=\"0em\""),
        ("leading", "font=\"16px/0 system-ui\""),
    ] {
        let all = build(
            &format!("value-{name}"),
            &card(&format!("    column {row}\n      children\n")),
            true,
            true,
        );
        assert!(!with(&all, id).is_empty(), "{name}: {all:?}");
    }
    // The node's own row wins in its layer: no refusal.
    let all = build(
        "override",
        &(card("    column class=Gone display=\"flex\"\n      children\n")
            + "\nstyle Gone\n  display = \"none\"\n"),
        true,
        true,
    );
    assert!(with(&all, id).is_empty(), "{all:?}");
    use contract_lower::anatomy::slot::{denied, UNFOLDED};
    for (row, value) in [
        ("font", "0px/1 system-ui"),
        ("hidden", "true"),
        ("aria-hidden", "true"),
        ("popover", "auto"),
        ("transform", "scaleX(0)"),
        ("transform", "translate(1,1), SCALE(0)"),
        ("opacity", UNFOLDED),
        ("width", "lib.gap"),
    ] {
        assert!(denied(row, value).is_some(), "{row}: {value}");
    }
    for (row, value) in [
        ("hidden", "false"),
        ("width", "auto"),
        ("transform", "scale(2)"),
        ("opacity", "1e-9"),
    ] {
        assert!(denied(row, value).is_none(), "{row}: {value}");
    }
}

/// Round 1 (Astra, Grok): an export through a barrel and a rename, one an
/// app reaches by a subpath past the entry, and a package nested inside a
/// library (its own `node_modules`), which is not the library's.
#[test]
fn round_1_exports() {
    let dir = Dir::new("barrel");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract","./card.contract":"./card.contract"}}"#,
    );
    dir.write(&format!("{at}/index.contract"), "use Fancy from \"./barrel.contract\"\nuse Other from \"./node_modules/@acme/inner/index.contract\"\n");
    dir.write(
        &format!("{at}/barrel.contract"),
        "use Card as Fancy from \"./card.contract\"\n",
    );
    dir.write(
        &format!("{at}/card.contract"),
        "component Card\n  slot\n  view\n    column opacity=0\n      children\n",
    );
    dir.write(
        &format!("{at}/node_modules/@acme/inner/package.json"),
        r#"{"name":"@acme/inner","version":"1.0.0","exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        &format!("{at}/node_modules/@acme/inner/index.contract"),
        "component Other\n  slot\n  view\n    column opacity=0\n      children\n",
    );
    dir.write(&format!("{at}/lib.tamber"), DESIGN);
    dir.write("app/app.tamber", APP_DESIGN);
    // Reached by a subpath, past the entry.
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib/card.contract\"\nuse Other from \"@acme/lib\"\n\ncomponent App\n  view\n    column\n      Card()\n        text \"a\"\n      Other()\n        text \"b\"\n",
    );
    let src = std::fs::read_to_string(&app).unwrap();
    let all = match contract::compile_path_source_all(Path::new(&app), &src, false) {
        Ok(_) => Vec::new(),
        Err(all) => all,
    };
    let slot = with(&all, "lower-anatomy-slot");
    assert!(slot.iter().any(|e| e.message.contains("`Card`")), "{all:?}");
    assert!(
        !slot.iter().any(|e| e.message.contains("`Other`")),
        "{all:?}"
    );
}

/// Round 2 (Astra, Grok): a base name the design sets is its token (its
/// `0px` refused, its `8px` not the scale's terminal `0`); SVG `text` on
/// the chain inside an `svg` has its `transform` checked; a determinant
/// that overflows is no pass; a text style is a font size; an unknown
/// `display` is unreadable; an export through the package's own name and
/// from an entry under `src/`.
#[test]
fn round_2_edges() {
    let id = "lower-anatomy-slot";
    let dir_build = |name: &str, card: &str, app_design: &str| {
        let dir = Dir::new(name);
        let at = "app/node_modules/@acme/lib";
        dir.write(
            &format!("{at}/package.json"),
            r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
        );
        dir.write(&format!("{at}/index.contract"), card);
        dir.write(&format!("{at}/lib.tamber"), DESIGN);
        dir.write("app/app.tamber", app_design);
        let app = dir.write(
            "app/app.contract",
            "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
        );
        let src = std::fs::read_to_string(&app).unwrap();
        match contract::compile_path_source_all(Path::new(&app), &src, false) {
            Ok(_) => Vec::new(),
            Err(all) => all,
        }
    };
    let all = dir_build(
        "set-zero",
        &card("    column width=\"space.md\"\n      children\n"),
        "tamber app\n\naxes\n  density = roomy | tight\n\ntheme\n  space.md = 0px\n",
    );
    assert!(!with(&all, id).is_empty(), "{all:?}");
    let all = dir_build(
        "set-radius",
        &card("    column width=\"radius.sm\"\n      children\n"),
        "tamber app\n\naxes\n  density = roomy | tight\n\ntheme\n  radius.sm = 8px\n",
    );
    assert!(with(&all, id).is_empty(), "{all:?}");
    let all = dir_build(
        "svg-text",
        &card("    svg viewBox=\"0 0 10 10\" width=10 height=10\n      text transform=\"scale(0)\"\n        children\n"),
        APP_DESIGN,
    );
    assert!(
        with(&all, id)
            .iter()
            .any(|e| e.message.contains("transform")),
        "{all:?}"
    );
    use contract_lower::anatomy::slot::denied;
    assert!(denied("transform", "matrix(1e20 1e20 1e20 1e20 0 0)").is_some());
    assert!(denied("font", "-apple-system-body").is_none());
    assert!(denied("font-size", "-EXACT-BODY").is_none());
    assert!(denied("display", "flexbox").is_some());
    assert!(denied("display", "inline-flex").is_none());
    // Exports: the package's own name with a subpath, and an entry under
    // `src/` whose `use` is relative to it.
    let dir = Dir::new("self-import");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./src/index.contract","./src/card.contract":"./src/card.contract"}}"#,
    );
    dir.write(
        &format!("{at}/src/index.contract"),
        "use Card from \"@acme/lib/src/card.contract\"\nuse Other from \"./other.contract\"\n",
    );
    dir.write(
        &format!("{at}/src/card.contract"),
        "component Card\n  slot\n  view\n    column opacity=0\n      children\n",
    );
    dir.write(
        &format!("{at}/src/other.contract"),
        "component Other\n  slot\n  view\n    column opacity=0\n      children\n",
    );
    dir.write(&format!("{at}/src/lib.tamber"), DESIGN);
    dir.write("app/app.tamber", APP_DESIGN);
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib/src/card.contract\"\nuse Other from \"@acme/lib\"\n\ncomponent App\n  view\n    column\n      Card()\n        text \"a\"\n      Other()\n        text \"b\"\n",
    );
    let src = std::fs::read_to_string(&app).unwrap();
    let all = match contract::compile_path_source_all(Path::new(&app), &src, false) {
        Ok(_) => Vec::new(),
        Err(all) => all,
    };
    let slot = with(&all, id);
    assert!(slot.iter().any(|e| e.message.contains("`Card`")), "{all:?}");
    assert!(
        slot.iter().any(|e| e.message.contains("`Other`")),
        "{all:?}"
    );
}

/// The one checker as stage 12 D's server calls it (rt1's shape): chains
/// read once from the IR, a candidate fold asked in-process, the refusal in
/// the compiler's words at the token's line, only the chains a touched
/// token reaches.
#[test]
fn check_slots_judges_a_candidate_fold() {
    use contract_lower::anatomy::slot::{check_slots, SlotChains};
    let dir = Dir::new("check-slots");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        &format!("{at}/index.contract"),
        &card("    column opacity=\"lib.op\"\n      children\n"),
    );
    dir.write(
        &format!("{at}/lib.tamber"),
        "tamber lib\n\ntheme\n  lib.pad = 4px\n  lib.op = 1\n",
    );
    dir.write("app/app.tamber", "tamber app\n");
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
    );
    let params = contract::tamber_params::params_of(&app);
    let design = exact_tamber::parse("tamber app\n").unwrap();
    let chains = SlotChains::from_ir(&contract::tamber_params::library(&params, Some(&design)));
    let vocab = contract_lower::tamber_vocab();
    let fold = |text: &str| {
        let d = contract_lower::with_package_designs(
            Some(exact_tamber::parse(text).unwrap()),
            &params.packages,
        )
        .unwrap();
        exact_tamber::check_design(d, &vocab).unwrap()
    };
    // The package's own value: fine.
    assert!(check_slots(&fold("tamber app\n"), None, &chains, &[]).is_empty());
    // A write that makes it 0: refused at the app's line, in the compiler's words.
    let broke = fold("tamber app\n\ntheme\n  lib.op = 0\n");
    let got = check_slots(&broke, None, &chains, &["lib.op"]);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].id, "lower-anatomy-slot");
    assert_eq!((got[0].line, got[0].col), (4, 3), "{got:?}");
    assert!(got[0].message.contains("slot's chain in `Card`"), "{got:?}");
    // A write touching no chain's token checks none.
    assert!(check_slots(&broke, None, &chains, &["lib.pad"]).is_empty());
}

/// theme2's D2 condition: `touched` is closed over references, so a write
/// to a base name reaches a chain whose row reads a token over it.
#[test]
fn check_slots_touched_reaches_through_aliases() {
    use contract_lower::anatomy::slot::{check_slots, SlotChains};
    let dir = Dir::new("check-slots-alias");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        &format!("{at}/index.contract"),
        &card("    column width=\"lib.w\"\n      children\n"),
    );
    dir.write(
        &format!("{at}/lib.tamber"),
        "tamber lib\n\ntheme\n  lib.w = calc(space.md * 2)\n",
    );
    dir.write("app/app.tamber", "tamber app\n");
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
    );
    let params = contract::tamber_params::params_of(&app);
    let design = exact_tamber::parse("tamber app\n").unwrap();
    let ir = contract::tamber_params::library(&params, Some(&design));
    // The IR lists what the row reaches, the base name among it.
    assert_eq!(
        ir[0]["chain"][0]["tokens"],
        serde_json::json!(["lib.w", "space.md"]),
        "{ir}"
    );
    let chains = SlotChains::from_ir(&ir);
    let vocab = contract_lower::tamber_vocab();
    let fold = |text: &str| {
        let d = contract_lower::with_package_designs(
            Some(exact_tamber::parse(text).unwrap()),
            &params.packages,
        )
        .unwrap();
        exact_tamber::check_design(d, &vocab).unwrap()
    };
    let broke = fold("tamber app\n\ntheme\n  space.md = 0px\n");
    let got = check_slots(&broke, None, &chains, &["space.md"]);
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(got[0].message.contains("lib.w"), "{got:?}");
    // Said at the app's line the package token reaches (rt1's ask), not
    // the design's start.
    assert_eq!((got[0].line, got[0].col), (4, 3), "{got:?}");
}

/// A chain row naming a base name itself, written with spaces: indexed and
/// listed by the name, and judged by the scale's columns once the design
/// leaves it unset (a write that removes the app's line).
#[test]
fn check_slots_reads_a_bare_base_name() {
    use contract_lower::anatomy::slot::{check_slots, SlotChains};
    let dir = Dir::new("check-slots-base");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        &format!("{at}/index.contract"),
        &card("    column width=\" radius.sm \"\n      children\n"),
    );
    dir.write(
        &format!("{at}/lib.tamber"),
        "tamber lib\n\ntheme\n  lib.pad = 4px\n",
    );
    let set = "tamber app\n\ntheme\n  radius.sm = 8px\n";
    dir.write("app/app.tamber", set);
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
    );
    let params = contract::tamber_params::params_of(&app);
    let ir = contract::tamber_params::library(&params, Some(&exact_tamber::parse(set).unwrap()));
    assert_eq!(
        ir[0]["chain"][0]["tokens"],
        serde_json::json!(["radius.sm"]),
        "{ir}"
    );
    let chains = SlotChains::from_ir(&ir);
    let vocab = contract_lower::tamber_vocab();
    let fold = |text: &str| {
        let d = contract_lower::with_package_designs(
            Some(exact_tamber::parse(text).unwrap()),
            &params.packages,
        )
        .unwrap();
        exact_tamber::check_design(d, &vocab).unwrap()
    };
    assert!(check_slots(&fold(set), None, &chains, &["radius.sm"]).is_empty());
    // The line removed: the scale's own `radius.sm`, 0 in a column.
    let got = check_slots(&fold("tamber app\n"), None, &chains, &["radius.sm"]);
    assert_eq!(got.len(), 1, "{got:?}");
    // Said at the removed override's line in the fold the write replaces
    // (the lead, from rt1's D2 review), never the design's start.
    let before = fold(set);
    let got = check_slots(
        &fold("tamber app\n"),
        Some(&before),
        &chains,
        &["radius.sm"],
    );
    assert_eq!((got[0].line, got[0].col), (4, 3), "{got:?}");
    let got = check_slots(
        &fold("tamber app\n\ntheme\n  radius.sm = 0px\n"),
        None,
        &chains,
        &["radius.sm"],
    );
    assert_eq!(got.len(), 1, "{got:?}");
}

/// A removed override reached through a package token, and through an
/// expression over it, is said at its former line too.
#[test]
fn check_slots_says_a_removed_override_at_its_line() {
    use contract_lower::anatomy::slot::{check_slots, SlotChains};
    for (name, row, lib) in [
        ("alias", "lib.w", "  lib.w = radius.sm\n"),
        ("expr", "calc(radius.sm * 2)", ""),
    ] {
        let dir = Dir::new(&format!("check-slots-removed-{name}"));
        let at = "app/node_modules/@acme/lib";
        dir.write(
            &format!("{at}/package.json"),
            r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
        );
        dir.write(
            &format!("{at}/index.contract"),
            &card(&format!("    column width=\"{row}\"\n      children\n")),
        );
        dir.write(
            &format!("{at}/lib.tamber"),
            &format!("tamber lib\n\ntheme\n  lib.pad = 4px\n{lib}"),
        );
        let set = "tamber app\n\ntheme\n  radius.sm = 8px\n";
        dir.write("app/app.tamber", set);
        let app = dir.write(
            "app/app.contract",
            "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
        );
        let params = contract::tamber_params::params_of(&app);
        let ir =
            contract::tamber_params::library(&params, Some(&exact_tamber::parse(set).unwrap()));
        let chains = SlotChains::from_ir(&ir);
        let vocab = contract_lower::tamber_vocab();
        let fold = |text: &str| {
            let d = contract_lower::with_package_designs(
                Some(exact_tamber::parse(text).unwrap()),
                &params.packages,
            )
            .unwrap();
            exact_tamber::check_design(d, &vocab).unwrap()
        };
        let before = fold(set);
        let got = check_slots(
            &fold("tamber app\n"),
            Some(&before),
            &chains,
            &["radius.sm"],
        );
        assert_eq!(got.len(), 1, "{name}: {got:?}");
        assert_eq!((got[0].line, got[0].col), (4, 3), "{name}: {got:?}");
    }
}

/// A chain row whose value is an expression over a package token is one
/// the compiler can't fold, and refuses (theme2's D4: the check folds or
/// refuses); `check_slots` judges it the same, so the server and the build
/// agree (rt1's D2 round 1), and the IR lists the token so a write to it
/// reaches the chain.
#[test]
fn check_slots_judges_an_expression_as_the_compiler_does() {
    use contract_lower::anatomy::slot::{check_slots, SlotChains};
    let dir = Dir::new("check-slots-expr");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        &format!("{at}/index.contract"),
        &card("    column width=\"calc(lib.w * 2)\"\n      children\n"),
    );
    dir.write(
        &format!("{at}/lib.tamber"),
        "tamber lib\n\ntheme\n  lib.w = 10px\n",
    );
    dir.write("app/app.tamber", "tamber app\n");
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
    );
    let params = contract::tamber_params::params_of(&app);
    let ir = contract::tamber_params::library(
        &params,
        Some(&exact_tamber::parse("tamber app\n").unwrap()),
    );
    assert_eq!(
        ir[0]["chain"][0]["tokens"],
        serde_json::json!(["lib.w"]),
        "{ir}"
    );
    let chains = SlotChains::from_ir(&ir);
    let vocab = contract_lower::tamber_vocab();
    let fold = |text: &str| {
        let d = contract_lower::with_package_designs(
            Some(exact_tamber::parse(text).unwrap()),
            &params.packages,
        )
        .unwrap();
        exact_tamber::check_design(d, &vocab).unwrap()
    };
    // The build refuses it, and so does the checker, in the same words.
    let src = std::fs::read_to_string(&app).unwrap();
    let built = contract::compile_path_source_all(std::path::Path::new(&app), &src, false)
        .err()
        .unwrap_or_default();
    let compiler: Vec<_> = built
        .iter()
        .filter(|e| e.id == "lower-anatomy-slot")
        .collect();
    assert_eq!(compiler.len(), 1, "{built:?}");
    let got = check_slots(&fold("tamber app\n"), None, &chains, &["lib.w"]);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].message, compiler[0].message, "{got:?}");
    let got = check_slots(
        &fold("tamber app\n\ntheme\n  lib.w = 4px\n"),
        None,
        &chains,
        &["lib.w"],
    );
    assert_eq!((got[0].line, got[0].col), (4, 3), "{got:?}");
}

/// The server's check and the build agree, id and words byte for byte
/// (theme2's re-confirm conditions, from rt1's D2 reviews): a write that
/// removes an app-only token a library row names, and an expression row
/// whose token a write makes `0px`.
#[test]
fn check_slots_and_the_build_refuse_in_the_same_words() {
    use contract_lower::anatomy::slot::{check_slots, SlotChains};
    for (name, row, set, broke) in [
        (
            "removed",
            "app.w",
            "tamber app\n\ntheme\n  app.w = 10px\n",
            "tamber app\n",
        ),
        (
            "expr",
            "calc(lib.w * 2)",
            "tamber app\n",
            "tamber app\n\ntheme\n  lib.w = 0px\n",
        ),
    ] {
        let dir = Dir::new(&format!("check-slots-same-{name}"));
        let at = "app/node_modules/@acme/lib";
        dir.write(
            &format!("{at}/package.json"),
            r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
        );
        dir.write(
            &format!("{at}/index.contract"),
            &card(&format!("    column width=\"{row}\"\n      children\n")),
        );
        dir.write(
            &format!("{at}/lib.tamber"),
            "tamber lib\n\ntheme\n  lib.w = 10px\n",
        );
        dir.write("app/app.tamber", set);
        let app = dir.write(
            "app/app.contract",
            "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"fill\"\n",
        );
        let params = contract::tamber_params::params_of(&app);
        let ir =
            contract::tamber_params::library(&params, Some(&exact_tamber::parse(set).unwrap()));
        let chains = SlotChains::from_ir(&ir);
        let vocab = contract_lower::tamber_vocab();
        let fold = |text: &str| {
            let d = contract_lower::with_package_designs(
                Some(exact_tamber::parse(text).unwrap()),
                &params.packages,
            )
            .unwrap();
            exact_tamber::check_design(d, &vocab).unwrap()
        };
        let before = fold(set);
        let server = check_slots(&fold(broke), Some(&before), &chains, &[]);
        // The build of the design the write leaves.
        dir.write("app/app.tamber", broke);
        let src = std::fs::read_to_string(&app).unwrap();
        let built = contract::compile_path_source_all(std::path::Path::new(&app), &src, false)
            .err()
            .unwrap_or_default();
        let build: Vec<_> = built
            .iter()
            .filter(|e| e.id == "lower-anatomy-slot")
            .map(|e| (e.id.to_string(), e.message.clone()))
            .collect();
        let server: Vec<_> = server
            .iter()
            .map(|d| (d.id.to_string(), d.message.clone()))
            .collect();
        assert_eq!(build.len(), 1, "{name}: {built:?}");
        assert_eq!(server, build, "{name}");
    }
}

/// LLP 1139 D5, for stage D's selection report (rt1): each source-map node
/// of an exported library component's own view names its declaring
/// identity; the slot's fill, the caller's, does not.
#[test]
fn the_source_map_names_a_library_nodes_identity() {
    let dir = Dir::new("map-library");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        &format!("{at}/index.contract"),
        &card("    column testId=\"card\"\n      children\n"),
    );
    dir.write(&format!("{at}/lib.tamber"), DESIGN);
    dir.write("app/app.tamber", APP_DESIGN);
    let app = dir.write(
        "app/app.contract",
        "use Card as Box from \"@acme/lib\"\n\ncomponent App\n  view\n    Box()\n      text \"fill\" testId=\"fill\"\n",
    );
    let src = std::fs::read_to_string(&app).unwrap();
    let (plan, map) = contract::compile_path_source_mapped(&app, &src).unwrap();
    let json: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let libs: Vec<Option<&str>> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["library"].as_str())
        .collect();
    assert!(
        libs.contains(&Some("@acme/lib/index.contract#Card")),
        "{libs:?}"
    );
    assert!(libs.contains(&None), "the fill is the caller's: {libs:?}");
}

/// Round 3 (Astra, Grok): a re-export through the package's own name and
/// its `exports` mapping; and a class's `font` naming a type token whose
/// leading is 0 stays checked whatever longhands the node sets.
#[test]
fn round_3_edges() {
    let dir = Dir::new("exports-map");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract","./card":"./src/card.contract"}}"#,
    );
    dir.write(
        &format!("{at}/index.contract"),
        "use Card from \"@acme/lib/card\"\n",
    );
    dir.write(
        &format!("{at}/src/card.contract"),
        "component Card\n  slot\n  view\n    column opacity=0\n      children\n",
    );
    dir.write(&format!("{at}/lib.tamber"), DESIGN);
    dir.write("app/app.tamber", APP_DESIGN);
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"a\"\n",
    );
    let src = std::fs::read_to_string(&app).unwrap();
    let all = match contract::compile_path_source_all(Path::new(&app), &src, false) {
        Ok(_) => Vec::new(),
        Err(all) => all,
    };
    assert!(!with(&all, "lower-anatomy-slot").is_empty(), "{all:?}");
    // The type token's leading.
    let dir = Dir::new("font-token");
    let at = "app/node_modules/@acme/lib";
    dir.write(
        &format!("{at}/package.json"),
        r#"{"name":"@acme/lib","version":"1.0.0","private":true,"exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        &format!("{at}/index.contract"),
        "component Card\n  slot\n  view\n    text class=Body font-size=\"16px\" font-style=\"normal\"\n      children\n\nstyle Body\n  font = \"lib.body\"\n",
    );
    dir.write(
        &format!("{at}/lib.tamber"),
        "tamber lib\n\ntheme\n  lib.body: type = italic 16px/0 system-ui\n",
    );
    dir.write("app/app.tamber", "tamber app\n");
    let app = dir.write(
        "app/app.contract",
        "use Card from \"@acme/lib\"\n\ncomponent App\n  view\n    Card()\n      text \"a\"\n",
    );
    let src = std::fs::read_to_string(&app).unwrap();
    let all = match contract::compile_path_source_all(Path::new(&app), &src, false) {
        Ok(_) => Vec::new(),
        Err(all) => all,
    };
    assert!(
        with(&all, "lower-anatomy-slot")
            .iter()
            .any(|e| e.message.contains("line-height")),
        "{all:?}"
    );
}
