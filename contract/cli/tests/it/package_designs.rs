//! LLP 1121 D11 "Packages" (stage 11 A): a package's design, `<name>.tamber`
//! beside its entry, its tokens under its namespace, set by the app's design
//! by full name; what a package may not declare, refused.

use std::path::{Path, PathBuf};

struct Dir(PathBuf);
impl Dir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("exact-pkgdesign-{name}-{}", std::process::id()));
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

/// A package `@acme/<ns>` under the app's node_modules: a `Card` whose view
/// reads `<ns>.card` (and `<ns>.gap`), with `design` as its `<ns>.tamber`.
fn package(dir: &Dir, ns: &str, design: Option<&str>) {
    let at = format!("app/node_modules/@acme/{ns}");
    dir.write(
        &format!("{at}/package.json"),
        &format!(
            r#"{{"name":"@acme/{ns}","version":"1.0.0","exports":{{".":"./index.contract"}}}}"#
        ),
    );
    let card = ns[..1].to_uppercase() + &ns[1..];
    dir.write(
        &format!("{at}/index.contract"),
        &format!("component {card}Card\n  view\n    column background-color=\"{ns}.card\" padding-top=\"{ns}.gap\" testId=\"{ns}\"\n      text \"x\"\n"),
    );
    if let Some(design) = design {
        dir.write(&format!("{at}/{ns}.tamber"), design);
    }
}

fn app(dir: &Dir, uses: &[&str], design: Option<&str>) -> PathBuf {
    if let Some(design) = design {
        dir.write("app/app.tamber", design);
    }
    let lines: String = uses
        .iter()
        .map(|ns| {
            format!(
                "use {c}Card from \"@acme/{ns}\"\n",
                c = ns[..1].to_uppercase() + &ns[1..]
            )
        })
        .collect();
    let views: String = uses
        .iter()
        .map(|ns| format!("      {}Card()\n", ns[..1].to_uppercase() + &ns[1..]))
        .collect();
    dir.write(
        "app/app.contract",
        &format!("{lines}component App\n  view\n    column\n{views}"),
    )
}

const READING: &str = "tamber reading\n\ntheme\n  reading.card = #112233\n  reading.gap = 12px\n  reading.unused = #445566\n";

fn plan(root: &Path) -> String {
    format!(
        "{:?}",
        contract::compile_path(root).unwrap_or_else(|e| panic!("{e}"))
    )
}

fn refused(root: &Path) -> contract::CompileError {
    contract::compile_path(root).expect_err("refused")
}

#[test]
fn a_package_design_styles_its_views_and_the_app_sets_its_tokens_by_full_name() {
    let dir = Dir::new("styles");
    package(&dir, "reading", Some(READING));
    let root = app(&dir, &["reading"], None);
    let built = plan(&root);
    assert!(built.contains("reading.card"), "{built}");
    // D7's retention: a token no view names is not kept.
    assert!(!built.contains("reading.unused"), "{built}");
    // The app's design sets one by full name: its value is the token's.
    let overridden = app(
        &dir,
        &["reading"],
        Some("tamber app\n\ntheme\n  reading.card = #abcdef\n"),
    );
    let again = plan(&overridden);
    assert_ne!(built, again, "the app's value moves the package's view");
    // A package's design is a source of the app's (a watcher rebuilds on it).
    let graph = contract::source_graph(&root);
    assert!(
        graph
            .sources
            .iter()
            .any(|s| s.path.ends_with("reading.tamber")),
        "{:?}",
        graph.sources
    );
}

#[test]
fn a_package_design_is_named_for_its_package_and_its_tokens_under_it() {
    let dir = Dir::new("names");
    package(
        &dir,
        "reading",
        Some("tamber books\n\ntheme\n  reading.card = #112233\n  reading.gap = 12px\n"),
    );
    let e = refused(&app(&dir, &["reading"], None));
    assert_eq!(e.id, "tamber-package-name", "{e}");
    assert!(e.message.contains("`tamber reading`"), "{e}");
    package(&dir, "reading", Some("tamber reading\n\ntheme\n  reading.card = #112233\n  reading.gap = 12px\n  ink = #000000\n"));
    let e = refused(&app(&dir, &["reading"], None));
    assert_eq!(e.id, "tamber-package-name", "{e}");
    assert!(
        e.message.contains("`ink`") && e.message.contains("`reading.…`"),
        "{e}"
    );
    assert!(
        e.file
            .as_ref()
            .is_some_and(|f| f.to_string_lossy().ends_with("reading.tamber")),
        "{e:?}"
    );
}

#[test]
fn axes_dials_params_lines_and_exports_are_not_a_packages() {
    let dir = Dir::new("blocks");
    let cases = [
        ("axes\n  density = compact | roomy\n\n", "an axis"),
        ("dials\n  warmth: number = 0 in 0..1\n\n", "a dial"),
        ("exports\n  reading.card\n\n", "an `exports` line"),
        ("params\n  Card.fill = #123456\n\n", "a `params` line"),
    ];
    for (block, said) in cases {
        package(
            &dir,
            "reading",
            Some(&format!(
                "tamber reading\n\n{block}theme\n  reading.card = #112233\n  reading.gap = 12px\n"
            )),
        );
        let e = refused(&app(&dir, &["reading"], None));
        assert_eq!(e.id, "tamber-package-block", "{block}: {e}");
        assert!(e.message.contains(said), "{block}: {e}");
    }
    // A component's params are a package's (stage 12 B1;
    // `tamber_package_params.rs`), and so are its icons (stage 13,
    // `package_icons.rs`): `component … params` and `icons` build.
    package(
        &dir,
        "reading",
        Some("tamber reading\n\ncomponent MapCard\n  params\n    pin: color = #112233\n\nicons\n  reading.book = icon(sf \"book\")\n\ntheme\n  reading.card = #112233\n  reading.gap = 12px\n"),
    );
    plan(&app(&dir, &["reading"], None));
}

#[test]
fn a_package_tokens_when_names_only_the_apps_axes() {
    let dir = Dir::new("whens");
    let design = "tamber reading\n\ntheme\n  reading.card = #112233\n    when density=compact: #000000\n  reading.gap = 12px\n";
    package(&dir, "reading", Some(design));
    let e = refused(&app(&dir, &["reading"], None));
    assert_eq!(e.id, "tamber-unknown-axis", "{e}");
    assert!(e.message.contains("`density` is no declared axis"), "{e}");
    // Declared by the app, it varies by it.
    let root = app(
        &dir,
        &["reading"],
        Some("tamber app\n\naxes\n  density = roomy | compact\n"),
    );
    plan(&root);
    // Or the app sets the token, whole: the package's `when` lines go with it.
    plan(&app(
        &dir,
        &["reading"],
        Some("tamber app\n\ntheme\n  reading.card = #abcdef\n"),
    ));
}

#[test]
fn one_name_from_two_files_is_refused_unless_the_app_sets_it() {
    let dir = Dir::new("duplicate");
    // Twice in one package: the fold's refusal, at its line in the package's file.
    package(&dir, "reading", Some("tamber reading\n\ntheme\n  reading.card = #112233\n  reading.gap = 12px\n  reading.card = #332211\n"));
    let e = refused(&app(&dir, &["reading"], None));
    assert_eq!(e.id, "tamber-duplicate", "{e}");
    assert!(
        e.file
            .as_ref()
            .is_some_and(|f| f.to_string_lossy().ends_with("reading.tamber")),
        "{e:?}"
    );
    assert_eq!(e.span.line, 6, "{e:?}");
    // A line its message names is the file's too.
    assert!(e.message.contains("(line 4)"), "{e}");
    // The app and a package: the app's line is the token, whole.
    plan(&app(
        &dir,
        &["reading"],
        Some("tamber app\n\ntheme\n  reading.card = #abcdef\n"),
    ));
    package(&dir, "reading", Some(READING));
    // A second package defining a `reading.` name: refused by its namespace first.
    package(&dir, "notes", Some("tamber notes\n\ntheme\n  notes.card = #010101\n  notes.gap = 4px\n  reading.card = #020202\n"));
    let e = refused(&app(&dir, &["reading", "notes"], None));
    assert_eq!(e.id, "tamber-package-name", "{e}");
    // Two packages that would take one namespace are refused, naming both.
    dir.write(
        "app/node_modules/@other/reading/package.json",
        r#"{"name":"@other/reading","version":"1.0.0","exports":{".":"./index.contract"}}"#,
    );
    dir.write(
        "app/node_modules/@other/reading/index.contract",
        "component OtherCard\n  view\n    text \"y\"\n",
    );
    dir.write(
        "app/node_modules/@other/reading/reading.tamber",
        "tamber reading\n\ntheme\n  reading.card = #030303\n",
    );
    dir.write("app/app.contract", "use ReadingCard from \"@acme/reading\"\nuse OtherCard from \"@other/reading\"\ncomponent App\n  view\n    column\n      ReadingCard()\n      OtherCard()\n");
    let e = refused(&dir.0.join("app/app.contract"));
    assert_eq!(e.id, "tamber-package-name", "{e}");
    assert!(
        e.message.contains("`@acme/reading`") && e.message.contains("`@other/reading`"),
        "{e}"
    );
}

#[test]
fn a_package_token_names_its_own_tokens_roles_and_base_names_only() {
    let dir = Dir::new("references");
    package(
        &dir,
        "reading",
        Some("tamber reading\n\ntheme\n  reading.card = brand\n  reading.gap = 12px\n"),
    );
    let e = refused(&app(
        &dir,
        &["reading"],
        Some("tamber app\n\ntheme\n  brand = #123456\n"),
    ));
    assert_eq!(e.id, "tamber-unknown-token", "{e}");
    assert!(
        e.message
            .contains("names `brand`, a token that isn't the package `@acme/reading`'s"),
        "{e}"
    );
    // Its own package's token, and a role, are its to name; a CSS function
    // or a colour's hex digits are no token's name, whatever the app names.
    package(&dir, "reading", Some("tamber reading\n\ntheme\n  reading.ink = #00face\n  reading.card = light-dark(reading.ink, #102030)\n  reading.rule = -exact-label\n  reading.gap = 12px\n"));
    plan(&app(
        &dir,
        &["reading"],
        Some("tamber app\n\ntheme\n  brand = #123456\n  face = #654321\n"),
    ));
    // An app token under the package's name that the package doesn't define is no name of the package's.
    package(
        &dir,
        "reading",
        Some("tamber reading\n\ntheme\n  reading.card = reading.extra\n  reading.gap = 12px\n"),
    );
    let e = refused(&app(
        &dir,
        &["reading"],
        Some("tamber app\n\ntheme\n  reading.extra = #123456\n"),
    ));
    assert_eq!(e.id, "tamber-unknown-token", "{e}");
}

#[test]
fn what_a_package_token_names_is_judged_as_the_fold_resolves_it() {
    let dir = Dir::new("resolved");
    // A family token's word is a font's name alone, and the app's token once the app declares one.
    package(&dir, "reading", Some("tamber reading\n\ntheme\n  reading.card = #112233\n  reading.gap = 12px\n  reading.face: family = appFace\n"));
    plan(&app(&dir, &["reading"], None));
    let e = refused(&app(
        &dir,
        &["reading"],
        Some("tamber app\n\ntheme\n  appFace: family = serif\n"),
    ));
    assert_eq!(e.id, "tamber-unknown-token", "{e}");
    assert!(e.message.contains("names `appFace`"), "{e}");
    // A base name the app sets is every package's to name (the lead's ruling 4).
    package(
        &dir,
        "reading",
        Some("tamber reading\n\ntheme\n  reading.card = #112233\n  reading.gap = space.md\n"),
    );
    plan(&app(
        &dir,
        &["reading"],
        Some("tamber app\n\ntheme\n  space.md = 12px\n"),
    ));
}

#[test]
fn a_package_may_not_name_a_base_name_nor_reach_outside_itself() {
    let dir = Dir::new("outside");
    // A package named `space` defining a base name.
    package(
        &dir,
        "space",
        Some(
            "tamber space\n\ntheme\n  space.card = #112233\n  space.gap = 1px\n  space.md = 12px\n",
        ),
    );
    let e = refused(&app(&dir, &["space"], None));
    assert_eq!(e.id, "tamber-package-name", "{e}");
    assert!(e.message.contains("`space.md`"), "{e}");
    // A `"tamber"` path out of the package.
    let dir = Dir::new("outside2");
    package(&dir, "reading", None);
    dir.write("app/node_modules/@acme/reading/package.json", r#"{"name":"@acme/reading","version":"1.0.0","exports":{".":"./index.contract"},"tamber":"../elsewhere.tamber"}"#);
    dir.write("app/node_modules/@acme/elsewhere.tamber", READING);
    let e = refused(&app(&dir, &["reading"], None));
    assert_eq!(e.id, "tamber-unreadable", "{e}");
    assert!(e.message.contains("outside the package"), "{e}");
}

#[test]
fn a_package_installed_under_another_name_keeps_its_own() {
    let dir = Dir::new("alias");
    dir.write(
        "app/node_modules/books/package.json",
        r#"{"name":"@acme/reading","version":"1.0.0","exports":{".":"./index.contract"}}"#,
    );
    dir.write("app/node_modules/books/index.contract", "component ReadingCard\n  view\n    column background-color=\"reading.card\" testId=\"reading\"\n      text \"x\"\n");
    dir.write("app/node_modules/books/reading.tamber", READING);
    let root = dir.write(
        "app/app.contract",
        "use ReadingCard from \"books\"\ncomponent App\n  view\n    column\n      ReadingCard()\n",
    );
    assert!(plan(&root).contains("reading.card"));
}

#[test]
fn an_app_with_no_design_of_its_own_still_reads_its_packages() {
    let dir = Dir::new("alone");
    package(&dir, "reading", Some(READING));
    // No app.tamber, no inline theme.
    let built = plan(&app(&dir, &["reading"], None));
    assert!(built.contains("reading.card"), "{built}");
    // A package without a design reads none (its view's names are unknown, as before).
    let bare = Dir::new("bare");
    package(&bare, "reading", None);
    assert!(contract::compile_path(&app(&bare, &["reading"], None)).is_err());
}

#[test]
fn an_app_with_code_beside_it_keeps_every_package_token() {
    // D7: an app whose code can name a token outside its views keeps every
    // token, its packages' too, with or without a design of its own.
    let dir = Dir::new("bindable");
    package(&dir, "reading", Some(READING));
    dir.write("app/data/Cargo.toml", "[package]\nname = \"data\"\n");
    let built = plan(&app(&dir, &["reading"], None));
    assert!(built.contains("reading.unused"), "{built}");
}

#[test]
fn a_package_token_names_no_dial_no_app_pair_and_no_other_packages_token() {
    let dir = Dir::new("edges");
    // A dial, named directly.
    package(
        &dir,
        "reading",
        Some("tamber reading\n\ntheme\n  reading.card = accent\n  reading.gap = 12px\n"),
    );
    let e = refused(&app(
        &dir,
        &["reading"],
        Some("tamber app\n\ndials\n  accent: color = #1083fe\n\ntheme\n  brand = accent\n"),
    ));
    assert_eq!(e.id, "tamber-unknown-token", "{e}");
    assert!(e.message.contains("the dial `accent`"), "{e}");
    // An app token as its pair.
    package(
        &dir,
        "reading",
        Some(
            "tamber reading\n\ntheme\n  reading.card = #112233  pair=brand\n  reading.gap = 12px\n",
        ),
    );
    let e = refused(&app(
        &dir,
        &["reading"],
        Some("tamber app\n\ntheme\n  brand = #ffffff\n"),
    ));
    assert_eq!(e.id, "tamber-unknown-token", "{e}");
    assert!(e.message.contains("`brand`"), "{e}");
    // Another package's token.
    package(&dir, "reading", Some(READING));
    package(
        &dir,
        "notes",
        Some("tamber notes\n\ntheme\n  notes.card = reading.card\n  notes.gap = 4px\n"),
    );
    let e = refused(&app(&dir, &["reading", "notes"], None));
    assert_eq!(e.id, "tamber-unknown-token", "{e}");
    assert!(
        e.message
            .contains("`reading.card`, a token that isn't the package `@acme/notes`'s"),
        "{e}"
    );
}

#[test]
fn a_package_vendored_inside_another_brings_its_design() {
    let dir = Dir::new("vendored");
    dir.write(
        "app/node_modules/@acme/shelf/package.json",
        r#"{"name":"@acme/shelf","version":"1.0.0","exports":{".":"./index.contract"}}"#,
    );
    dir.write("app/node_modules/@acme/shelf/index.contract", "use ReadingCard from \"./vendor/reading/index.contract\"\ncomponent ShelfCard\n  view\n    column\n      ReadingCard()\n");
    dir.write(
        "app/node_modules/@acme/shelf/vendor/reading/package.json",
        r#"{"name":"@acme/reading","version":"1.0.0","exports":{".":"./index.contract"}}"#,
    );
    dir.write("app/node_modules/@acme/shelf/vendor/reading/index.contract", "component ReadingCard\n  view\n    column background-color=\"reading.card\" testId=\"reading\"\n      text \"x\"\n");
    dir.write(
        "app/node_modules/@acme/shelf/vendor/reading/reading.tamber",
        READING,
    );
    let root = dir.write("app/app.contract", "use ShelfCard from \"@acme/shelf\"\ncomponent App\n  view\n    column\n      ShelfCard()\n");
    assert!(plan(&root).contains("reading.card"));
    let graph = contract::source_graph(&root);
    assert!(
        graph
            .consulted
            .iter()
            .any(|p| p.ends_with("vendor/reading/package.json")),
        "{:?}",
        graph.consulted
    );
}

#[test]
fn a_dials_name_in_a_package_value_is_a_read_only_where_the_fold_reads_the_dial() {
    // A CSS function or a colour's hex digits spelled like an app dial read no dial.
    let dir = Dir::new("dialwords");
    package(&dir, "reading", Some("tamber reading\n\ntheme\n  reading.card = rgb(17 34 51)\n  reading.ink = #00face\n  reading.gap = 12px\n"));
    plan(&app(&dir, &["reading"], Some("tamber app\n\ndials\n  rgb: color = #123456\n  face: color = #654321\n\ntheme\n  brand = light-dark(rgb, face)\n")));
}
