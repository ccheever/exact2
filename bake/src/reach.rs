//! What this app can reach: the grant ceiling, with each `device.*` line's
//! purpose read from the strings tables and every host's spelling derived
//! from the runner's one table.
//!
//! @ref LLP 1069.008 D1 (a missing purpose key refuses the bake, every one
//! in one run) / D4 (the derived `Info.plist` keys and entitlements) / D5
//! (`grantPurposes` in the compatibility id) / D7 (the reach table, with an
//! "enforced by" column)
//!
//! The result rides beside the id in `compat.json` as `reach`, never in it:
//! `host/apple/build.mjs` writes the plists from it, `exact release` signs
//! with its entitlements, and the deploy dry run and the install page print
//! its rows. The purposes' digest is what enters the id, so editing a purpose
//! or its translation is a binary change and never a silent bundle.

use crate::compat::{canonical, compatibility_digest, Compat};
use exact_runner::device::{device_grants, Device};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

/// Fill `compat.reach` and, for an app with device grants, the
/// `grantPurposes` input (moving the id). Refuses a malformed device line and
/// a purpose key the base strings table lacks, each reported.
pub(crate) fn derive(app_dir: &Path, platform: &str, compat: &mut Compat) -> Result<(), String> {
    let Some(ceiling) = compat.inputs["grantCeiling"].as_str().map(str::to_owned) else {
        return Ok(());
    };
    let devices = device_grants(&ceiling).map_err(|all| {
        all.iter()
            .map(|e| format!("grant-device: {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let strings = if devices.is_empty() {
        None
    } else {
        contract::strings_tables(app_dir)?
    };
    let mut errors = Vec::new();
    // Key → locale → text, a translation lacking a key falling back to the
    // base as `t` does.
    let mut purposes: BTreeMap<&str, BTreeMap<String, String>> = BTreeMap::new();
    for grant in &devices {
        let base = strings
            .as_ref()
            .and_then(|s| s.base_table().get(grant.purpose));
        let (Some(strings), Some(base)) = (strings.as_ref(), base) else {
            let table = strings.as_ref().map_or_else(
                || "the app has no strings/ directory".to_string(),
                |s| format!("strings/{}.json has no \"{}\"", s.base, grant.purpose),
            );
            errors.push(format!(
                "grant-purpose: `device.{} {}` names its purpose by strings key (LLP 1060), and {table}",
                grant.device.name, grant.purpose
            ));
            continue;
        };
        let texts = strings
            .tables
            .iter()
            .map(|(locale, table)| {
                let text = table.get(grant.purpose).unwrap_or(base);
                (locale.clone(), text.clone())
            })
            .collect();
        purposes.insert(grant.purpose, texts);
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    if !purposes.is_empty() {
        let mut canon = String::new();
        canonical(&json!(purposes), &mut canon);
        let digest = Sha256::digest(canon.as_bytes());
        let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
        compat.inputs["grantPurposes"] = Value::String(hex);
        compat.id = compatibility_digest(&compat.inputs);
    }
    let base = strings.as_ref().map(|s| s.base.clone());
    let text =
        |key: &str| -> Option<&String> { base.as_ref().and_then(|b| purposes.get(key)?.get(b)) };
    // A mixed app names each child's grants; otherwise the one source is
    // `app.ts` when there is one, else the Rust data crate's `fn grants()`.
    let typescript = app_dir.join("app.ts").is_file();
    let declared = |line: &str| {
        let has = |field: &str| {
            compat.inputs[field]
                .as_str()
                .is_some_and(|g| g.lines().any(|l| l.trim() == line))
        };
        if compat.inputs["javascriptGrants"].is_null() {
            return if typescript {
                "app.ts"
            } else {
                "the Rust source"
            };
        }
        match (has("javascriptGrants"), has("rustGrants")) {
            (true, true) => "app.ts and the Rust source",
            (true, false) => "app.ts",
            _ => "the Rust source",
        }
    };
    let rows: Vec<Value> = ceiling
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|line| {
            let device = devices.iter().find(|g| {
                line.split_whitespace().next() == Some(&format!("device.{}", g.device.name))
            });
            let enforced = match device {
                Some(g) => device_enforcement(g.device, declared(line)),
                None => family_enforcement(line).to_string(),
            };
            json!({
                "grant": line,
                "purpose": device.and_then(|g| text(g.purpose)),
                "enforced": enforced,
            })
        })
        .collect();
    // The host's own spelling: usage keys with every locale's text, and the
    // entitlements the platform's signature must carry.
    let mut usage = Map::new();
    let mut entitlements = Vec::new();
    for grant in &devices {
        let keys = match platform {
            "ios" => grant.device.ios,
            "macos" => grant.device.macos,
            _ => &[],
        };
        for key in keys {
            usage.insert((*key).into(), json!(purposes[grant.purpose]));
        }
        // iOS needs none today; push's `aps-environment` joins when LLP 1069
        // §5 #9 lands, and `auth.*`'s associated domains with LLP 1069.006.
        if platform == "macos" {
            entitlements.extend(grant.device.hardened);
        }
    }
    entitlements.sort_unstable();
    entitlements.dedup();
    compat.reach = json!({
        "base": base,
        "locales": strings.as_ref().map(|s| s.tables.keys().cloned().collect::<Vec<_>>()),
        "rows": rows,
        "usage": usage,
        "entitlements": entitlements,
    });
    Ok(())
}

/// Who enforces a device line: the OS's prompt (from the key the build
/// writes), the served web's policy, and which source declared it. No
/// first-party device capability ships yet, so no row claims the runtime.
fn device_enforcement(device: &Device, declared: &str) -> String {
    let mut by = Vec::new();
    if !device.ios.is_empty() || device.name == "notifications" {
        by.push("OS prompt (iOS, macOS)".to_string());
    }
    if device.web.is_some() {
        by.push("Permissions-Policy (served web; a static dist sends none)".into());
    }
    by.push(format!("declared by {declared}"));
    by.join("; ")
}

/// Who enforces every other grant line.
fn family_enforcement(line: &str) -> &'static str {
    match line.split_whitespace().next().unwrap_or_default() {
        "net.fetch" => "runtime (all hosts); CSP (served web)",
        "surface.read" | "surface.write" => "presenter",
        // `auth.*` (LLP 1069.006) joins here: the host's `exact-auth:` arm.
        _ => "runtime",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compat(grants: &str) -> Compat {
        Compat {
            id: "old".into(),
            inputs: json!({"grantCeiling": grants, "javascriptGrants": grants, "rustGrants": ""}),
            channel: String::new(),
            origin: None,
            activate: "next-launch".into(),
            target: "t".into(),
            embedded: Value::Null,
            reach: Value::Null,
        }
    }

    fn app(tables: &[(&str, &str)]) -> tempdir::Dir {
        let dir = tempdir::Dir::new();
        std::fs::create_dir(dir.0.join("strings")).unwrap();
        for (locale, json) in tables {
            std::fs::write(dir.0.join(format!("strings/{locale}.json")), json).unwrap();
        }
        dir
    }

    mod tempdir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new() -> Dir {
                use std::sync::atomic::{AtomicU32, Ordering};
                static N: AtomicU32 = AtomicU32::new(0);
                let n = N.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("exact-bake-reach-{}-{n}", std::process::id()));
                let _ = std::fs::remove_dir_all(&path);
                std::fs::create_dir_all(&path).unwrap();
                Dir(path)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn a_missing_purpose_refuses_every_one_in_one_run() {
        let dir = app(&[("en", r#"{"purpose.mic": "Records."}"#)]);
        let mut c = compat("device.microphone purpose.mic\ndevice.geolocation purpose.where\ndevice.camera purpose.eye");
        let error = derive(&dir.0, "ios", &mut c).unwrap_err();
        assert_eq!(error.lines().count(), 2, "{error}");
        assert!(
            error.lines().all(|l| l.starts_with("grant-purpose: ")),
            "{error}"
        );
        assert!(error.contains("strings/en.json has no \"purpose.where\""));
        let bare = tempdir::Dir::new();
        let error = derive(&bare.0, "ios", &mut compat("device.microphone p")).unwrap_err();
        assert!(error.contains("no strings/ directory"), "{error}");
        let error = derive(&bare.0, "ios", &mut compat("device.lidar p")).unwrap_err();
        assert!(error.starts_with("grant-device: "), "{error}");
    }

    #[test]
    fn the_purposes_digest_moves_with_any_locale_s_text() {
        let digest = |fr: &str| {
            let dir = app(&[
                ("en", r#"{"purpose.mic": "Records.", "title": "T"}"#),
                ("fr", fr),
            ]);
            let mut c = compat("net.fetch https://x/\ndevice.microphone purpose.mic");
            derive(&dir.0, "macos", &mut c).unwrap();
            (
                c.inputs["grantPurposes"].as_str().unwrap().to_string(),
                c.id,
                c.reach,
            )
        };
        let (a, id, reach) = digest(r#"{"purpose.mic": "Enregistre."}"#);
        let (b, _, _) = digest(r#"{"purpose.mic": "Enregistre !"}"#);
        let (fallback, _, _) = digest(r#"{"title": "T"}"#);
        assert_ne!(a, b);
        assert_ne!(a, fallback);
        assert_ne!(id, "old");
        // A string no grant names moves nothing.
        let dir = app(&[
            ("en", r#"{"purpose.mic": "Records.", "title": "Other"}"#),
            ("fr", r#"{"purpose.mic": "Enregistre."}"#),
        ]);
        let mut c = compat("net.fetch https://x/\ndevice.microphone purpose.mic");
        derive(&dir.0, "macos", &mut c).unwrap();
        assert_eq!(c.inputs["grantPurposes"], a);
        assert_eq!(
            reach["usage"]["NSMicrophoneUsageDescription"],
            json!({"en": "Records.", "fr": "Enregistre."})
        );
        assert_eq!(
            reach["entitlements"],
            json!(["com.apple.security.device.audio-input"])
        );
        assert_eq!(reach["locales"], json!(["en", "fr"]));
        assert_eq!(
            reach["rows"][0]["enforced"],
            "runtime (all hosts); CSP (served web)"
        );
        assert_eq!(reach["rows"][1]["purpose"], "Records.");
        assert!(reach["rows"][1]["enforced"]
            .as_str()
            .unwrap()
            .ends_with("declared by app.ts"));
    }

    #[test]
    fn an_app_without_device_grants_keeps_its_id() {
        let dir = tempdir::Dir::new();
        let mut c = compat("net.fetch https://x/");
        derive(&dir.0, "ios", &mut c).unwrap();
        assert_eq!(c.id, "old");
        assert!(c.inputs.get("grantPurposes").is_none());
        assert_eq!(c.reach["usage"], json!({}));
        let mut ios = compat("device.speech-recognition p");
        let dir = app(&[("en", r#"{"p": "Transcribes."}"#)]);
        derive(&dir.0, "ios", &mut ios).unwrap();
        assert_eq!(
            ios.reach["usage"],
            json!({"NSSpeechRecognitionUsageDescription": {"en": "Transcribes."}})
        );
        assert_eq!(ios.reach["entitlements"], json!([]));
    }
}
