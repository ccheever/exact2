//! A package's prebuilt dynamic library, linked into the app's crate (LLP 1154 D3 "The library", LLP 1133.001 D9).
//! The package that fills the app's native slot (`contract::native::native_slot`, which chooses it and refuses two)
//! names one `{url, sha256}` per Rust target in its `exact.json` `native.library`; the native build fetched and
//! checked the target's file before Cargo ran and laid out its link inputs under
//! `~/.cache/exact/package-libraries/<sha256>/<target>/<link>/` (`scripts/package-libraries.mjs`). This build
//! script only points the linker there: `-L` that directory, `-l` the declaration's `native.link`, and a run path
//! where the product carries the library, beside a Linux executable (`$ORIGIN`) and a Mac one run as the Linux host
//! (`@executable_path`). An Apple app's crate is an archive, for which Cargo records these and links nothing;
//! `host/apple/build.mjs` links the framework into the app and embeds it. A web bake links no native library.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Print the link of the app's package library for Cargo's `TARGET`; nothing for a web bake or when the package that
/// fills the native slot declares none.
pub(crate) fn link(app: &Path, platform: &str) -> Result<(), String> {
    let target = std::env::var("TARGET").map_err(|_| "build requires TARGET".to_string())?;
    if platform == "web" || target.starts_with("wasm32-") {
        return Ok(());
    }
    let selection = contract::native::native_slot(app)?;
    for file in &selection.watched {
        println!("cargo:rerun-if-changed={}", file.display());
    }
    let Some(slot) = selection.slot else {
        return Ok(());
    };
    let (name, native) = (&slot.package, &slot.native);
    let Some(library) = native.get("library").and_then(Value::as_object) else {
        return Ok(());
    };
    // The name is printed into `cargo:` lines, so it is held to the characters a library name has.
    let link = native["link"]
        .as_str()
        .filter(|link| {
            !link.is_empty()
                && !link.starts_with(['.', '-', '+'])
                && link
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'+' | b'-'))
        })
        .ok_or_else(|| {
            format!("{name}'s exact.json: native.link names the library (`lib<link>.so`, `<link>.framework`) in letters, digits and `_.+-`")
        })?;
    let Some(entry) = library.get(&target) else {
        let named: Vec<&str> = library.keys().map(String::as_str).collect();
        return Err(format!(
            "{name} has no native library for {target}: its exact.json native.library names {}. An app that builds for {target} waits for the package to publish one (LLP 1154 D6)",
            if named.is_empty() {
                "no target".to_string()
            } else {
                named.join(", ")
            }
        ));
    };
    if target.contains("-android") {
        return Err(format!(
            "{name} declares a native library for {target}, and exact2 links a package's library only for Apple and Linux targets; Android's APK carries none yet (LLP 1154 D6)"
        ));
    }
    let sha256 = entry["sha256"]
        .as_str()
        .filter(|hex| {
            hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
        .ok_or_else(|| {
            format!(
                "{name}'s exact.json: native.library.{target}.sha256 is 64 lowercase hex digits"
            )
        })?;
    let home = std::env::var_os("HOME").ok_or_else(|| "build requires HOME".to_string())?;
    let view = PathBuf::from(home)
        .join(".cache/exact/package-libraries")
        .join(sha256)
        .join(&target)
        .join(link);
    let apple = target.contains("-apple-");
    let placed = view.join(if apple {
        format!("lib{link}.dylib")
    } else {
        format!("lib{link}.so")
    });
    if !placed.exists() {
        return Err(format!(
            "{name}'s library for {target} is not fetched ({}): the native build fetches it before Cargo runs (`bun exact.mjs linux`, `bun exact.mjs mac`, `bun exact.mjs ios`)",
            placed.display()
        ));
    }
    println!("cargo:rustc-link-search=native={}", view.display());
    println!("cargo:rustc-link-lib=dylib={link}");
    let origin = if apple { "@executable_path" } else { "$ORIGIN" };
    println!("cargo:rustc-link-arg=-Wl,-rpath,{origin}");
    Ok(())
}
