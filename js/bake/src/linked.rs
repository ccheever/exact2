//! A bake made outside Cargo for an Apple link (`exact-js-bake --cargo`,
//! scripts/linked-plan.mjs): the archive's build script takes from it only
//! what Rust compiles, and the link carries the plan, `compat.json` and the
//! module's bytecode in the executable's `__TEXT` (`exact_apple::baked`).

use crate::replace_changed;
use std::path::Path;

/// The files of a bake the archive compiles: all but the plan, the module's
/// bytecode and their receipts, which the link carries.
pub(crate) const COMPILED: [&str; 4] = ["module.rs", "logic.rs", "linked.rs", "host.rs"];

/// The module's bytecode from the section the link added
/// (`exact_apple::baked::bytecode`), for `exact_js::Module::new`, which
/// takes it through `Into<exact_js::Bytecode>`.
pub(crate) const LINKED_BYTECODE: &str = r#"#[allow(dead_code)]
pub const APP_BYTECODE: ExactLinkedBytecode = ExactLinkedBytecode;
#[doc(hidden)]
pub struct ExactLinkedBytecode;
impl From<ExactLinkedBytecode> for ::exact_js::Bytecode {
    fn from(_: ExactLinkedBytecode) -> Self {
        ::exact_js::Bytecode::Linked(::exact_apple::baked::bytecode())
    }
}
"#;

/// The entry's plan and compatibility document, embedded from OUT_DIR.
pub(crate) const EMBEDDED: &str = r#"#[allow(dead_code)]
fn exact_plan() -> &'static [u8] {
    include_bytes!(concat!(env!("OUT_DIR"), "/app.plan"))
}
#[allow(dead_code)]
fn exact_compat() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/compat.json"))
}
"#;

/// The entry's plan and compatibility document, from the sections the link added.
pub(crate) const LINKED: &str = r#"#[allow(dead_code)]
fn exact_plan() -> &'static [u8] {
    ::exact_apple::baked::plan()
}
#[allow(dead_code)]
fn exact_compat() -> &'static str {
    ::exact_apple::baked::compat()
}
"#;

/// The pair over an entry that declares `const PLAN` and `const COMPAT`
/// itself, as `exact new` wrote it before the plan was linked: such an app
/// builds as it did, its plan in the archive, until `bun exact.mjs update`
/// drops the two (`names_its_plan`).
pub(crate) const NAMED: &str = r#"#[allow(dead_code)]
fn exact_plan() -> &'static [u8] {
    PLAN
}
#[allow(dead_code)]
fn exact_compat() -> &'static str {
    COMPAT
}
"#;

/// Whether the crate's `src/lib.rs` declares the plan and compatibility
/// constants itself (see `NAMED`); watched, so `update` dropping them is
/// noticed. Outside a build script, or in a crate with no `src/lib.rs`, no.
pub(crate) fn names_its_plan() -> bool {
    let Some(dir) = std::env::var_os("CARGO_MANIFEST_DIR") else {
        return false;
    };
    let entry = Path::new(&dir).join("src/lib.rs");
    let Ok(source) = std::fs::read_to_string(&entry) else {
        return false;
    };
    println!("cargo:rerun-if-changed={}", entry.display());
    let declares = |name: &str| {
        source
            .lines()
            .any(|line| line.starts_with(&format!("const {name}:")))
    };
    declares("PLAN") && declares("COMPAT")
}

/// `build` over a bake made outside Cargo into `baked`, for OUT_DIR `out`.
pub(crate) fn take_baked(baked: &Path, out: &Path) -> Result<(), String> {
    if names_its_plan() {
        return Err("src/lib.rs declares `const PLAN` and `const COMPAT`, which read a plan this build links instead: run `bun exact.mjs update`, which drops them".into());
    }
    for name in COMPILED {
        let from = baked.join(name);
        println!("cargo:rerun-if-changed={}", from.display());
        let bytes = std::fs::read(&from).map_err(|e| format!("{}: {e}", from.display()))?;
        replace_changed(&out.join(name), &bytes)?;
    }
    replace_changed(&out.join("baked.rs"), LINKED.as_bytes())?;
    // What an earlier bake in Cargo left here would be stale.
    for name in [
        "app.plan",
        "app.plan.map.json",
        "compat.json",
        "artifacts.json",
        "app.module.json",
        "app.js",
        "app.hbc",
    ] {
        match std::fs::remove_file(out.join(name)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
    }
    Ok(())
}
