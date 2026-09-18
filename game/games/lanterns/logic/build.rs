// Compile independent, edited game implementations for the I3 test binary only.
// The shipped Lanterns source and engine behavior are unchanged.
use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/kinds.rs");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source = fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let source = source.split("#[cfg(test)]").next().unwrap();
    let source = source
        .lines()
        .filter(|l| !l.starts_with("//!") && !l.starts_with("#!"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace(
            "mod kinds;",
            &format!("#[path = {:?}] mod kinds;", root.join("src/kinds.rs")),
        )
        .replace(
            "include_bytes!(\"../../assets/Fox.glb\")",
            &format!("include_bytes!({:?})", root.join("../assets/Fox.glb")),
        );
    let original = fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let types = original
        .split("pub fn scene_types()")
        .nth(1)
        .unwrap()
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    let source = format!("{source}\npub fn scene_types(){types}");
    for (name, replacements) in [
        (
            "physics",
            vec![
                ("6.4", "8.0"),
                ("12.0 * dt", "18.0 * dt"),
                ("-12.0, 0.0", "-18.0, 0.0"),
            ],
        ),
        (
            "clip",
            vec![
                ("\"Run\"", "\"Walk\""),
                (
                    "Animation::looping(\"Survey\")",
                    "Animation::looping(\"Walk\")",
                ),
            ],
        ),
        ("appearance", vec![("glow * 5.0", "glow * 9.0")]),
    ] {
        let mut edited = source.clone();
        for (from, to) in replacements {
            assert!(edited.contains(from), "edit anchor disappeared: {from}");
            edited = edited.replace(from, to);
        }
        fs::write(
            PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(format!("i3_{name}.rs")),
            edited,
        )
        .unwrap();
    }
}
