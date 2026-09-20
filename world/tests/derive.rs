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
        .join("scratch/k1")
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
    let positive=compile("use exact_world::*; #[derive(Default,Component)] struct C { n:u32 } #[derive(Default,Args)] struct Options { #[live] on:bool }");
    assert!(
        positive.status.success(),
        "{}",
        String::from_utf8_lossy(&positive.stderr)
    );
    let cases = [
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
    for source in cases {
        let output = compile(source);
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "accepted {source}");
        assert!(
            !error.contains("can't find crate") && !error.contains("unresolved import"),
            "false refusal: {error}"
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}
