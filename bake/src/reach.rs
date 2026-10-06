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
    let keys_of = |device: &Device| match platform {
        "ios" => device.ios,
        "macos" => device.macos,
        _ => &[],
    };
    for grant in &devices {
        for key in keys_of(grant.device) {
            usage.insert((*key).into(), json!(purposes[grant.purpose]));
        }
        // The platform's signing entitlements: macOS's hardened-runtime ones,
        // iOS's capability ones (LLP 1069.008.000 D2; push's `aps-environment`
        // joins when LLP 1069 §5 #9 lands). `auth.*`'s associated domains are
        // `reach.auth` (below).
        match platform {
            "macos" => entitlements.extend(grant.device.hardened),
            "ios" => entitlements.extend(grant.device.ios_entitlements),
            _ => {}
        }
    }
    // A key a granted row needs beside its own, written with that row's text
    // when no granted row writes it (D1: one HealthKit direction still
    // carries both keys). Only where the row writes keys at all.
    for grant in &devices {
        if let Some(key) = grant
            .device
            .companion
            .filter(|_| !keys_of(grant.device).is_empty())
        {
            usage
                .entry(key)
                .or_insert_with(|| json!(purposes[grant.purpose]));
        }
    }
    entitlements.sort_unstable();
    entitlements.dedup();
    // tvOS bakes the iOS plan; what a row not on TV derives there, the tvOS
    // build drops (D5): every key and entitlement no TV row also needs.
    let tv_omits = if platform == "ios" {
        let on_tv = |key: &str| {
            devices
                .iter()
                .filter(|g| g.device.tvos)
                .any(|g| g.device.ios.contains(&key) || g.device.ios_entitlements.contains(&key))
        };
        let mut omit: Vec<&str> = devices
            .iter()
            .filter(|g| !g.device.tvos)
            .flat_map(|g| {
                g.device
                    .ios
                    .iter()
                    .chain(g.device.ios_entitlements)
                    .copied()
                    .chain(g.device.companion)
            })
            .filter(|key| !on_tv(key))
            .collect();
        omit.sort_unstable();
        omit.dedup();
        omit
    } else {
        Vec::new()
    };
    let auth = auth(&ceiling, platform, compat)?;
    compat.reach = json!({
        "base": base,
        "locales": strings.as_ref().map(|s| s.tables.keys().cloned().collect::<Vec<_>>()),
        "rows": rows,
        "usage": usage,
        "entitlements": entitlements,
        "tvOmits": tv_omits,
        "auth": auth,
    });
    Ok(())
}

/// Refuse grants that do not parse, before anything is derived from them: a
/// host that cannot parse the set holds none of it (the runner's one parse,
/// `exact_runner::grants`), so the build stops instead, every bad line named
/// by its source and its number there. With `rust`, `grants` is `app.ts`'s.
pub(crate) fn parsed(
    app_dir: &Path,
    grants: Option<&str>,
    rust: Option<&str>,
) -> Result<(), String> {
    const RUST: &str = "the Rust source";
    let typescript = rust.is_some() || app_dir.join("app.ts").is_file();
    let sources = [
        (grants, if typescript { "app.ts" } else { RUST }),
        (rust, RUST),
    ];
    let errors: Vec<String> = sources
        .iter()
        .filter_map(|(spec, source)| Some((exact_runner::grants::parse((*spec)?).err()?, source)))
        .flat_map(|(errors, source)| {
            errors
                .into_iter()
                .map(move |e| format!("grant-parse: {source}: {e}"))
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

/// `auth.*` through the same table (LLP 1069.006 D2; LLP 1069.008 D2's
/// `auth` rows): every callback, for the web build's two client-metadata
/// documents (one per `application_type`, ruled); and on Apple the
/// `webcredentials:<host>` associated domain each claimed https callback
/// needs (`ASWebAuthenticationSession.Callback.https`), which the build
/// signs with and the web build's AASA answers. A private-use scheme needs
/// nothing, on purpose. The web refuses `auth.session` in a worker-placed
/// TypeScript source: its popup must open in the press's call stack
/// (after review, item 2), so it fails here, not at the press.
fn auth(ceiling: &str, platform: &str, compat: &Compat) -> Result<Value, String> {
    let callbacks = exact_runner::auth::callbacks(ceiling);
    let sessions = ceiling
        .lines()
        .any(|l| l.trim().starts_with("auth.session "));
    let typescript = compat.inputs["javascriptGrants"]
        .as_str()
        .or(compat.inputs["grantCeiling"].as_str())
        .is_some_and(|g| g.lines().any(|l| l.trim().starts_with("auth.session ")));
    if platform == "web"
        && typescript
        && compat.inputs["typescriptPlacement"].as_str() == Some("worker")
    {
        return Err("grant-auth: `auth.session` in a worker-placed TypeScript source: on the web `openAuthSession` opens its popup in the press's call stack, which a worker is not in (LLP 1069.006); place the source on main (`typescript.placement`)".into());
    }
    let webcredentials: Vec<String> = callbacks
        .iter()
        .filter_map(|c| c.strip_prefix("https://"))
        .filter_map(|rest| rest.split('/').next())
        .filter(|host| {
            !matches!(
                host.split(':').next(),
                Some("localhost" | "127.0.0.1" | "[::1]")
            )
        })
        .map(|host| format!("webcredentials:{host}"))
        .collect();
    Ok(json!({
        "sessions": sessions,
        "callbacks": callbacks,
        "associatedDomains": if matches!(platform, "ios" | "macos") { webcredentials } else { Vec::new() },
    }))
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
        "auth.session" | "auth.callback" => {
            "the host's exact-auth: arm (Apple, web; Linux answers 501)"
        }
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

    #[test]
    fn health_writes_both_keys_signs_on_ios_only_and_leaves_tvos_out() {
        // LLP 1069.008.000 D1, D2, D5.
        let dir = app(&[
            (
                "en",
                r#"{"read": "Reads your runs.", "write": "Saves workouts."}"#,
            ),
            ("fr", r#"{"read": "Lit vos courses."}"#),
        ]);
        let derive_for = |grants: &str, platform: &str| {
            let mut c = compat(grants);
            derive(&dir.0, platform, &mut c).unwrap();
            c.reach
        };
        let (share, update) = (
            "NSHealthShareUsageDescription",
            "NSHealthUpdateUsageDescription",
        );
        let read = json!({"en": "Reads your runs.", "fr": "Lit vos courses."});
        let write = json!({"en": "Saves workouts.", "fr": "Saves workouts."});
        // One direction: the other's key borrows its text, every locale's.
        let ios = derive_for("device.health-read read", "ios");
        assert_eq!(ios["usage"], json!({share: read, update: read}));
        assert_eq!(
            ios["entitlements"],
            json!(["com.apple.developer.healthkit"])
        );
        assert_eq!(
            ios["tvOmits"],
            json!([
                "NSHealthShareUsageDescription",
                "NSHealthUpdateUsageDescription",
                "com.apple.developer.healthkit"
            ])
        );
        assert_eq!(
            ios["rows"].as_array().unwrap().len(),
            1,
            "the reach table lists what was granted"
        );
        let ios = derive_for("device.health-write write", "ios");
        assert_eq!(ios["usage"], json!({share: write, update: write}));
        // Both: each key its own text.
        let both = derive_for("device.health-read read\ndevice.health-write write", "ios");
        assert_eq!(both["usage"], json!({share: read, update: write}));
        // macOS: the keys, never the restricted entitlement.
        let mac = derive_for("device.health-read read", "macos");
        assert_eq!(mac["usage"], json!({share: read, update: read}));
        assert_eq!(mac["entitlements"], json!([]));
        // A row that is on TV leaves nothing to omit, beside Health or not.
        let mixed = derive_for(
            "device.health-read read\ndevice.speech-recognition write",
            "ios",
        );
        assert!(!mixed["tvOmits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k == "NSSpeechRecognitionUsageDescription"));
        assert_eq!(
            derive_for("device.speech-recognition write", "ios")["tvOmits"],
            json!([])
        );
    }

    #[test]
    fn auth_lines_derive_callbacks_and_apple_s_webcredentials() {
        let grants = "auth.session https://bsky.social\nauth.callback social.exact.x:/oauth\nauth.callback https://app.example/.exact/auth/callback\nauth.callback http://127.0.0.1:9/cb";
        let dir = tempdir::Dir::new();
        let mut ios = compat(grants);
        derive(&dir.0, "ios", &mut ios).unwrap();
        assert_eq!(ios.id, "old", "no purpose, no id move");
        assert_eq!(
            ios.reach["auth"]["associatedDomains"],
            json!(["webcredentials:app.example"])
        );
        assert_eq!(ios.reach["auth"]["callbacks"].as_array().unwrap().len(), 3);
        assert!(ios.reach["rows"][0]["enforced"]
            .as_str()
            .unwrap()
            .contains("exact-auth:"));
        let mut web = compat(grants);
        derive(&dir.0, "web", &mut web).unwrap();
        assert_eq!(web.reach["auth"]["associatedDomains"], json!([]));
        web.inputs["typescriptPlacement"] = json!("worker");
        assert!(derive(&dir.0, "web", &mut web)
            .unwrap_err()
            .starts_with("grant-auth: "));
        let mut native = compat(grants);
        native.inputs["typescriptPlacement"] = json!("worker");
        assert!(
            derive(&dir.0, "macos", &mut native).is_ok(),
            "a native worker opens its sheet"
        );
    }
}
