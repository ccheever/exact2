//! What an Apple archive links (LLP 1047.001 D2, D3; LLP 1047 D8).
//!
//! @ref LLP 1047 D8 (on native the linked set is a compatibility input; the
//! manifest may link capabilities ahead of any plan that uses them)
//! @ref LLP 1047.001 D2 (one composition drives Rust, Swift, admission and
//! compatibility)

use crate::compat::{compatibility_digest, Compat};
use exact_runner::{Capability, Uses};

/// Name what an Apple archive links into `compat`'s inputs (`linked`), from
/// the plan its build script baked into `out`, before the id is taken and
/// everything that binds it (the GPU module's card, the receipts) is written.
///
/// A production build (`host/apple/build.mjs` says `EXACT_APPLE_LINK=plan`)
/// links what the plan uses and what the manifest's `link` names ahead of
/// use; every other build, and a build `build.mjs` does not drive, links
/// every capability (LLP 1047.001 D6). A bundle whose plan uses a capability
/// this binary lacks is then another compatibility id: a binary change.
pub(crate) fn name_into(
    compat: &mut Compat,
    manifest: &contract::Manifest,
    out: &std::path::Path,
) -> Result<(), String> {
    println!("cargo:rerun-if-env-changed=EXACT_APPLE_LINK");
    let uses = if std::env::var("EXACT_APPLE_LINK").is_ok_and(|v| v == "plan") {
        let bytes = std::fs::read(out.join("app.plan")).map_err(|e| {
            format!("the app bake must write OUT_DIR/app.plan before compatibility_id: {e}")
        })?;
        let plan = exact_plan::Plan::decode(&bytes)
            .map_err(|e| format!("the baked plan is invalid: {e:?}"))?;
        // The app's I/O grants link the host's I/O, and its `auth.session`
        // grant the sign-in sheet, whatever the plan says: a data source's
        // requests are the source's, not the plan's.
        let ceiling = compat.inputs["grantCeiling"].as_str().unwrap_or("");
        let granted = [
            (Capability::Io, !exact_runner::io_grants(ceiling).is_empty()),
            (Capability::AuthSession, auth_granted(ceiling)),
        ];
        let uses = ahead(manifest)?
            .into_iter()
            .fold(exact_runner::uses(&plan), Uses::with);
        granted
            .into_iter()
            .filter_map(|(capability, granted)| granted.then_some(capability))
            .fold(uses, Uses::with)
    } else {
        Capability::ALL.into_iter().fold(Uses::NONE, Uses::with)
    };
    let names: Vec<&str> = uses.iter().map(Capability::name).collect();
    compat.inputs["linked"] = serde_json::json!(names);
    compat.id = compatibility_digest(&compat.inputs);
    Ok(())
}

/// Whether a grant ceiling names `auth.session` (LLP 1069.006 D2): what
/// links the sign-in sheet (LLP 1047.001.000 E).
pub fn auth_granted(ceiling: &str) -> bool {
    ceiling
        .lines()
        .any(|line| line.split_whitespace().next() == Some("auth.session"))
}

/// The Apple entry's text for what `compat` says the archive links
/// ([`contract::apple_linked`]); every capability where it says nothing.
pub fn apple_link(compat: &Compat, host: &str) -> String {
    let uses = match compat.inputs["linked"].as_array() {
        Some(names) => names
            .iter()
            .filter_map(|n| n.as_str().and_then(Capability::from_name))
            .fold(Uses::NONE, Uses::with),
        None => Capability::ALL.into_iter().fold(Uses::NONE, Uses::with),
    };
    contract::apple_linked(uses, host)
}

/// The manifest's `link`: capabilities by name, linked before any plan uses
/// them, so a later bundle may (LLP 1047 D8).
pub fn ahead(manifest: &contract::Manifest) -> Result<Vec<Capability>, String> {
    let Some(link) = manifest.json.get("link") else {
        return Ok(Vec::new());
    };
    let names = link
        .as_array()
        .ok_or("app.json: `link` is a list of capability names")?;
    names
        .iter()
        .map(|name| {
            name.as_str()
                .and_then(Capability::from_name)
                .ok_or_else(|| {
                    let known: Vec<&str> = Capability::ALL.iter().map(|c| c.name()).collect();
                    format!("app.json: `link` names {name}, which is not a capability; capabilities: {}", known.join(", "))
                })
        })
        .collect()
}

/// Take the Rust sources of an Apple bake made before Cargo. The build
/// script calls this before baking and returns when it answers true.
/// `files` names generated Rust inputs; a bake must preserve their times
/// when their bytes do not change. Without EXACT_BAKED, bake normally.
///
/// The first Cargo run registers its compiled build-script executable and
/// bootstraps the bake in a child. Later builds run that executable before
/// Cargo, so a Contract edit changes only the artifacts the linker reads.
pub fn take_linked(files: &[&str]) -> Result<bool, String> {
    use sha2::{Digest, Sha256};
    use std::{env, fs, path::PathBuf, process::Command};
    println!("cargo:rerun-if-env-changed=EXACT_BAKED");
    let Some(baked) = env::var_os("EXACT_BAKED").map(PathBuf::from) else {
        return Ok(false);
    };
    if env::var_os("EXACT_RUST_BAKE").is_some() {
        return Ok(false);
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("build requires OUT_DIR")?);
    let executable = env::current_exe().map_err(|e| e.to_string())?;
    let variables: std::collections::BTreeMap<_, _> = env::vars()
        .filter(|(key, _)| {
            key.starts_with("CARGO_CFG_")
                || key.starts_with("CARGO_FEATURE_")
                || matches!(
                    key.as_str(),
                    "TARGET" | "HOST" | "PROFILE" | "OPT_LEVEL" | "DEBUG" | "CARGO_MANIFEST_DIR"
                )
        })
        .collect();
    let mut registration = serde_json::json!({
        "executable": executable, "cwd": env::current_dir().map_err(|e| e.to_string())?,
        "env": variables, "sha256": format!("{:x}", Sha256::digest(fs::read(&executable).map_err(|e| e.to_string())?)),
    });
    fs::create_dir_all(&baked).map_err(|e| e.to_string())?;
    let registry = baked.join("rust-baker.json");
    let mut previous: serde_json::Value = fs::read(&registry)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    if let Some(object) = previous.as_object_mut() {
        object.remove("started");
    }
    if previous != registration
        || !baked.join("compat.json").exists()
        || files.iter().any(|file| !baked.join(file).exists())
    {
        registration["started"] = serde_json::json!(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis());
        let result = Command::new(&executable)
            .env("EXACT_RUST_BAKE", "1")
            .env("OUT_DIR", &baked)
            .output()
            .map_err(|e| e.to_string())?;
        eprint!("{}", String::from_utf8_lossy(&result.stderr));
        if !result.status.success() {
            return Err(format!("Rust bake failed: {}", result.status));
        }
        fs::write(baked.join("rust-output.txt"), &result.stdout).map_err(|e| e.to_string())?;
        fs::write(&registry, registration.to_string()).map_err(|e| e.to_string())?;
    }
    for line in fs::read_to_string(baked.join("rust-output.txt"))
        .map_err(|e| e.to_string())?
        .lines()
    {
        if line.starts_with("cargo:warning=") || line.starts_with("cargo::warning=") {
            println!("{line}");
        }
    }
    for file in files {
        let from = baked.join(file);
        println!("cargo:rerun-if-changed={}", from.display());
        let bytes = fs::read(from).map_err(|e| e.to_string())?;
        let to = out.join(file);
        if fs::read(&to).ok().as_deref() != Some(&bytes) {
            fs::write(to, bytes).map_err(|e| e.to_string())?;
        }
    }
    // A previous embedded bake must not be mistaken for this linked bake.
    for file in [
        "app.plan",
        "compat.json",
        "artifacts.json",
        "app.plan.map.json",
        "app.hbc",
    ] {
        match fs::remove_file(out.join(file)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::ahead;
    use exact_runner::Capability;

    fn manifest(json: serde_json::Value) -> contract::Manifest {
        contract::Manifest {
            json,
            id: "com.example.link".into(),
            name: "Link".into(),
            declared: true,
        }
    }

    #[test]
    fn link_names_capabilities_and_refuses_another_word() {
        assert!(ahead(&manifest(serde_json::json!({}))).unwrap().is_empty());
        assert_eq!(
            ahead(&manifest(
                serde_json::json!({"link": ["markdown", "grouped_lists"]})
            ))
            .unwrap(),
            vec![Capability::Markdown, Capability::GroupedLists]
        );
        let refused = ahead(&manifest(serde_json::json!({"link": ["markdwon"]}))).unwrap_err();
        assert!(
            refused.contains("markdwon") && refused.contains("markdown"),
            "{refused}"
        );
    }

    #[test]
    fn an_auth_session_grant_links_the_sign_in_sheet() {
        assert!(super::auth_granted(
            "net.fetch https://a.test/\nauth.session https://bsky.social"
        ));
        assert!(!super::auth_granted("auth.callback social.exact.x:/oauth"));
        assert!(!super::auth_granted(""));
    }

    /// The manifest schema's `link` names are the roster's, in its order.
    #[test]
    fn the_schema_names_every_capability() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../scripts/app.schema.json")).unwrap();
        let names: Vec<&str> = schema["properties"]["link"]["items"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_str().unwrap())
            .collect();
        let roster: Vec<&str> = Capability::ALL.iter().map(|c| c.name()).collect();
        assert_eq!(names, roster);
    }
}
