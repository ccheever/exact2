use std::{fs, process::Command};
#[test]
fn derives_accept_supported_shapes_and_refuse_invalid_syntax() {
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let lib = fs::read_dir(&deps)
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("libexact_world-")
                && e.path().extension().is_some_and(|e| e == "rlib")
        })
        .max_by_key(|e| e.metadata().unwrap().modified().unwrap())
        .unwrap()
        .path();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let scratch = root
        .parent()
        .unwrap()
        .join("scratch/KL")
        .join(format!("derive-{}", std::process::id()));
    fs::create_dir_all(&scratch).unwrap();
    let compile = |source: &str| {
        let file = scratch.join("case.rs");
        fs::write(&file, source).unwrap();
        Command::new("rustc")
            .args([
                "--edition=2021",
                "--crate-type=lib",
                "--emit=metadata",
                "--crate-name=derive_case",
            ])
            .arg("--extern")
            .arg(format!("exact_world={}", lib.display()))
            .arg("-L")
            .arg(format!("dependency={}", deps.display()))
            .arg("-o")
            .arg(scratch.join("case.rmeta"))
            .arg(file)
            .output()
            .unwrap()
    };
    let positive=compile("use exact_world::*; #[derive(Default,Component)] struct C { n:u32 } #[derive(Default,Args)] struct Options { #[live] on:bool, count:std::primitive::u32, label:std::string::String }");
    assert!(
        positive.status.success(),
        "{}",
        String::from_utf8_lossy(&positive.stderr)
    );
    let raw = compile(
        r#"use exact_world::*;
use exact_world::storage::query::Fetch;
#[derive(Default, Component)] struct C(u32);
fn escape(w: &World) -> RefMut<'_, C> {
    let state = <&mut C as Query>::prepare(w, &mut [None; 8]).unwrap();
    unsafe { state.owned(0) }
}"#,
    );
    assert!(
        !raw.status.success(),
        "raw query construction escaped the private boundary"
    );
    assert!(
        String::from_utf8_lossy(&raw.stderr).contains("private"),
        "{}",
        String::from_utf8_lossy(&raw.stderr)
    );
    let raw = compile(
        r#"use exact_world::*;
fn escape<Q: Query>(w: &World) -> Q::Item<'static> {
    let state = Q::prepare(w, &mut [None; 8]).unwrap();
    unsafe { state.fetch(0) }
}"#,
    );
    assert!(
        !raw.status.success(),
        "raw fetch escaped through an associated bound"
    );
    assert!(
        String::from_utf8_lossy(&raw.stderr).contains("fetch"),
        "{}",
        String::from_utf8_lossy(&raw.stderr)
    );
    let missing_default = compile(
        r#"use exact_world::*;
struct NoDefault;
#[derive(Data)] struct Record { #[data(skip)] omitted: NoDefault }
impl Default for Record { fn default() -> Self { Self { omitted: NoDefault } } }
"#,
    );
    let error = String::from_utf8_lossy(&missing_default.stderr);
    assert!(!missing_default.status.success());
    assert!(
        error.contains("NoDefault") && error.contains("Data fields require Default"),
        "{error}"
    );
    // Presence is unsafe initialization evidence, never caller-replaceable.
    let mask = compile(
        r#"use exact_world::*;
#[derive(Default, Component)] struct Item(String);
fn replace(w: &World) {
    let pages = w.pages::<Item>();
    let mut page = pages.iter().next().unwrap();
    page.mask = &[u64::MAX];
}"#,
    );
    assert!(
        !mask.status.success(),
        "Page presence was publicly replaceable"
    );
    assert!(String::from_utf8_lossy(&mask.stderr).contains("private"));
    let static_fields = compile(
        r#"use exact_world::*;
#[derive(Default, Data)] struct Text { text: std::borrow::Cow<'static, str>, #[data(skip)] marker: std::borrow::Cow<'static, str> }
"#,
    );
    assert!(
        static_fields.status.success(),
        "{}",
        String::from_utf8_lossy(&static_fields.stderr)
    );
    let custom_key = compile(
        r#"use exact_world::*;
#[derive(Default, Data, PartialEq, Eq, PartialOrd, Ord)] struct Key(String, u32);
impl AsRef<str> for Key { fn as_ref(&self) -> &str { &self.0 } }
impl From<&str> for Key { fn from(s: &str) -> Self { Self(s.into(), 0) } }
fn save() { bin::to_vec(&std::collections::BTreeMap::from([(Key("x".into(), 7), 1u32)])).unwrap(); }
"#,
    );
    assert!(!custom_key.status.success());
    assert!(String::from_utf8_lossy(&custom_key.stderr).contains("CanonicalKey"));
    let cases = [
        r###"fn main() {
use exact_world::{World, Component};
#[derive(Default, Component)] struct Count(u32);
World::new(60, 0).resource::<Count>();
}"###,
        r###"fn main() {
use exact_world::{World, Component};
#[derive(Default, Component)] struct Item(u32);
let mut world = World::new(60, 0);
world.spawn((Item::default(),)).unwrap();
let mut query = world.query::<&mut Item>();
let row = query.iter().next().unwrap().1;
drop(query);
row.0 = 1;
}"###,
        r###"fn main() {
use exact_world::{World, Component};
#[derive(Default, Component)] struct Item(u32);
let world = World::new(60, 0);
let mut query = world.query::<&mut Item>();
let row = query.iter().next().unwrap().1;
let again = query.iter().next().unwrap().1;
row.0 = again.0;
}"###,
        r###"fn main() {
use exact_world::{World, Component};
#[derive(Default, Component)] struct Item(u32);
let mut world = World::new(60, 0);
world.spawn((Item::default(),)).unwrap();
let pages = world.pages::<Item>();
let view = pages.iter().next().unwrap();
drop(pages);
let runs = view.runs().collect::<Vec<_>>();
}"###,
        r###"use exact_world::Data;
#[derive(Default, Data)]
struct Mutable { value: std::cell::Cell<u32> }"###,
        r###"use exact_world::Data;
#[derive(Default, Data)]
struct Generic<T>(T);"###,
        r###"use exact_world::Data;
#[derive(Default, Data)]
struct Borrowed<'a> { text: &'a str }"###,
        r###"use exact_world::Data;
#[derive(Default, Data)]
struct PlatformSized { count: usize }"###,
        r###"use exact_world::Data;
#[derive(Default, Data)]
struct Unordered { entries: std::collections::HashMap<String, u32> }"###,
        r###"use exact_world::Data;
#[derive(Default, Data)]
#[data(skip)]
struct TypeAttribute { score: u32 }"###,
        r###"use exact_world::Data;
#[derive(Default, Data)]
enum VariantAttribute { #[default] #[data(skip)] A }"###,
        r###"use exact_world::Data;
#[derive(Default, Data)]
struct UnknownAttribute { #[data(typo)] score: u32 }"###,
        r###"use exact_world::Data;
#[derive(Default, Data)]
enum Discriminants { #[default] A = 1, B = 2 }"###,
        r###"use exact_world::Args;
#[derive(Default, Args)]
struct Options { #[live(typo)] volume: f32 }"###,
        r###"use exact_world::Args;
#[derive(Default, Args)]
struct Options { #[live = true] volume: f32 }"###,
        r###"use exact_world::Args;
#[derive(Default, Args)]
#[live]
struct Options { volume: f32 }"###,
        r###"use exact_world::Args;
#[derive(Default, Args)]
#[live(typo)]
struct Options { volume: f32 }"###,
        r###"use exact_world::Args;
#[derive(Default, Args)]
#[live = true]
struct Options { volume: f32 }"###,
    ];
    // Trait refusals are the public boundary: outside crates cannot add Data
    // for usize/Cell/HashMap because both trait and type would be foreign.
    let diagnostics = [
        "Resource",
        "cannot move out of `query`",
        "cannot borrow `query` as mutable",
        "cannot move out of `pages`",
        "Data",
        "does not support generics or lifetimes",
        "does not support generics or lifetimes",
        "Data",
        "Data",
        "data attribute is only meaningful on a field",
        "data attribute is only meaningful on a field",
        "unknown data attribute",
        "explicit enum discriminants",
        "without arguments",
        "without arguments",
        "attribute is only meaningful on a field",
        "attribute is only meaningful on a field",
        "attribute is only meaningful on a field",
    ];
    for (source, diagnostic) in cases.into_iter().zip(diagnostics) {
        let output = compile(source);
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "accepted {source}");
        assert!(error.contains(diagnostic), "expected {diagnostic}: {error}");
        assert!(
            !error.contains("can't find crate") && !error.contains("unresolved import"),
            "false refusal: {error}"
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}
