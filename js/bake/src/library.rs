//! A package's prebuilt dynamic library, linked into the app's crate (LLP 1154 D3 "The library", LLP 1133.001 D9).
//! The app's direct dependency whose `exact.json` declares `native.library` names one `{url, sha256}` per Rust
//! target; the native build fetched the target's file before Cargo ran and laid out its link inputs under
//! `~/.cache/exact/package-libraries/<sha256>/<target>/` (`scripts/package-libraries.mjs`). This build script only
//! points the linker there: `-L` that directory, `-l` the declaration's `native.link`, and a run path where the
//! product carries the library, beside a Linux executable (`$ORIGIN`) and a macOS one run as the Linux host
//! (`@executable_path`). An Apple app's crate is an archive, which links nothing; `host/apple/build.mjs` links the
//! framework into the app and embeds it.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Print the link of the app's package library for Cargo's `TARGET`; nothing when no direct dependency declares one.
pub(crate) fn link(app: &Path) -> Result<(), String> {
    let target = std::env::var("TARGET").map_err(|_| "build requires TARGET".to_string())?;
    let app = app
        .canonicalize()
        .map_err(|e| format!("{}: {e}", app.display()))?;
    let Some((name, declaration_path, native)) = declaring(&app)? else {
        return Ok(());
    };
    println!("cargo:rerun-if-changed={}", declaration_path.display());
    let Some(library) = native.get("library").and_then(Value::as_object) else {
        return Ok(());
    };
    let link = native["link"].as_str().ok_or_else(|| {
        format!("{name}'s exact.json: native.library needs native.link, the library's link name")
    })?;
    let Some(entry) = library.get(&target) else {
        let named: Vec<&str> = library.keys().map(String::as_str).collect();
        return Err(format!(
            "{name} has no native library for {target}: its exact.json native.library names {}. An app that builds for {target} waits for the package to publish one (LLP 1154 D6)",
            if named.is_empty() { "no target".to_string() } else { named.join(", ") }
        ));
    };
    if target.ends_with("-android") {
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
        .join(&target);
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

/// The direct dependency whose `exact.json` declares `native`: its name, that file and the `native` object.
fn declaring(app: &Path) -> Result<Option<(String, PathBuf, Value)>, String> {
    let read = |path: &Path| -> Option<Value> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    };
    let Some(manifest) = app
        .ancestors()
        .map(|dir| dir.join("package.json"))
        .find(|path| path.is_file())
    else {
        return Ok(None);
    };
    let mut names: Vec<String> = (read(&manifest).unwrap_or_default()["dependencies"].as_object())
        .map(|deps| deps.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    let mut found = Vec::new();
    for name in names {
        let Some(root) = app
            .ancestors()
            .map(|dir| dir.join("node_modules").join(&name))
            .find(|root| root.join("package.json").is_file())
        else {
            continue;
        };
        let path = root.join("exact.json");
        if let Some(native) = read(&path).map(|json| json["native"].clone()) {
            if native.is_object() {
                found.push((name, path, native));
            }
        }
    }
    if found.len() > 1 {
        let names: Vec<&str> = found.iter().map(|(name, ..)| name.as_str()).collect();
        return Err(format!(
            "bake-native-twice: {} each declare `native` in exact.json, and an app has one native module; depend on one of them",
            names.join(" and ")
        ));
    }
    Ok(found.pop())
}
