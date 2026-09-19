//! Compile failures assert the intended message, rather than accepting any rustc error.
use std::{fs, process::Command};

#[test]
fn kind_diagnostics() {
    let exe = std::env::current_exe().unwrap();
    let deps = exe.parent().unwrap();
    let library = fs::read_dir(deps)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("libexact_game-")
                && p.extension().is_some_and(|e| e == "rlib")
        })
        .max_by_key(|p| fs::metadata(p).unwrap().modified().unwrap())
        .unwrap();
    let scratch = deps
        .parent()
        .unwrap()
        .join(format!("kind-diagnostics-{}", std::process::id()));
    fs::create_dir_all(&scratch).unwrap();
    let cases = [
        ("#[derive(Kind)] struct Bad { a: Transform, b: Transform }", "Bad: duplicate component type Transform"),
        ("#[derive(Kind)] struct Bad { a: Transform, b: Option<Transform> }", "duplicate component type Transform"),
        ("#[derive(Kind)] struct Bad { #[read(foo)] a: Transform }", "expected #[read] on a Kind field, without arguments"),
        ("#[derive(Kind)] #[read] struct Bad { a: Transform }", "expected #[read] on a Kind field, without arguments"),
        ("#[derive(Kind)] struct Bad { r#id: Transform }", "Bad: id is reserved"),
        ("type Maybe<T> = Option<T>; #[derive(Kind)] struct Bad { a: Maybe<Transform> }", "optional Kind aliases are unsupported: spell Option<Component> directly"),
        ("#[derive(Kind)] struct Bad(Transform);", "Bad: Kind does not support tuple structs"),
        ("#[derive(Kind)] struct Bad;", "Bad: Kind does not support unit structs"),
        ("#[derive(Kind)] struct Bad<T> { a: T }", "Bad: Kind does not support generic structs"),
        ("#[derive(Kind)] struct Bad {}", "Kind requires 1..=8 component fields (maximum 8)"),
        ("#[derive(Kind)] struct Bad { a: Transform,b: Transform,c: Transform,d: Transform,e: Transform,f: Transform,g: Transform,h: Transform,i: Transform }", "maximum 8"),
        ("#[derive(Kind)] struct Bad { #[data(skip)] a: Transform }", "Kind fields cannot use data(skip)"),
        ("#[derive(Kind)] struct Bad { a: Option<Transform> }", "at least one required component"),
        ("#[derive(Kind)] struct Bad { a: Option<Option<Transform>> }", "not empty or nested Option"),
        ("#[derive(Kind)] struct Bad { #[child(foo)] a: Transform }", "expected #[child"),
        ("#[derive(Kind)] struct Bad { a: u32 }", "expected a Component"),
        ("#[derive(Kind)] struct A { a: Transform } #[derive(Kind)] struct B { b: Material } fn wrong(w: &World, id: Id<B>) { w.row::<A>(id); }", "expected `Id<A>`, found `Id<B>`"),
    ];
    let mut failures = Vec::new();
    for (i, (source, expected)) in cases.iter().enumerate() {
        let file = scratch.join(format!("case_{i}.rs"));
        fs::write(&file, format!("use exact_game::*; {source} fn main() {{}}")).unwrap();
        let output = Command::new("rustc")
            .arg("--edition=2021")
            .arg("--emit=metadata")
            .arg("--extern")
            .arg(format!("exact_game={}", library.display()))
            .arg("-L")
            .arg(format!("dependency={}", deps.display()))
            .arg("--out-dir")
            .arg(&scratch)
            .arg(&file)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() || !stderr.contains(expected) {
            failures.push(format!("case {i}: expected {expected}\n{stderr}"));
        }
    }
    fs::remove_dir_all(scratch).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
