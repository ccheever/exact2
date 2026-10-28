//! What this app can reach: the grant ceiling, with each `device.*` line's
//! purpose read from the strings tables and every host's spelling derived
//! from the runner's one table.
//!
//! @ref LLP 1069.008 D1 (a missing purpose key refuses the bake, every one
//! in one run) / D4 (the derived `Info.plist` keys and entitlements) / D5
//! (`grantPurposes` in the compatibility id) / D7 (the reach table, with an
//! "enforced by" column) / D9 (runtime exceptions, each scoped to one helper
//! executable)
//!
//! The result rides beside the id in `compat.json` as `reach`, never in it:
//! `host/apple/build.mjs` writes the plists from it, `exact release` signs
//! with its entitlements, and the deploy dry run and the install page print
//! its rows. The purposes' digest is what enters the id, so editing a purpose
//! or its translation is a binary change and never a silent bundle.

use crate::compat::{canonical, compatibility_digest, Compat};
use exact_runner::device::{device_grants, Device};
use exact_runner::hardened::runtime_grants;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

/// Fill `compat.reach` and, for an app with device grants, the
/// `grantPurposes` input (moving the id). Refuses a malformed device line and
/// a purpose key the base strings table lacks, each reported.
pub(crate) fn derive(
    app_dir: &Path,
    platform: &str,
    plan: Option<&exact_plan::Plan>,
    compat: &mut Compat,
) -> Result<(), String> {
    let Some(ceiling) = compat.inputs["grantCeiling"].as_str().map(str::to_owned) else {
        return Ok(());
    };
    let (devices, exceptions) = match (device_grants(&ceiling), runtime_grants(&ceiling)) {
        (Ok(devices), Ok(exceptions)) => (devices, exceptions),
        (devices, exceptions) => {
            let device = devices.err().unwrap_or_default().into_iter();
            let runtime = exceptions.err().unwrap_or_default().into_iter();
            return Err(device
                .map(|e| format!("grant-device: {e}"))
                .chain(runtime.map(|e| format!("grant-runtime: {e}")))
                .collect::<Vec<_>>()
                .join("\n"));
        }
    };
    privacy_policy(app_dir, platform, &devices)?;
    // No call transport exists yet, so a call is a test call, and App Review
    // reads one with no real calls beside it as no intended use of the
    // `audio` mode (2.5.4): it stays in development builds (LLP 1133.033
    // §13 question 4).
    if compat.embedded["trust"] == "production" && devices.iter().any(|g| g.device.name == "calls")
    {
        return Err("grant-device: `device.calls` stays out of production builds until exact2 has a call transport: a call is a test call until then (LLP 1133.033 §13 question 4); bake with EXACT_UPDATE_TRUST=development, or leave the line out".into());
    }
    let strings = if devices.iter().all(|g| !g.device.purpose) {
        None
    } else {
        contract::strings_tables(app_dir)?
    };
    let mut errors = Vec::new();
    // Key → locale → text, a translation lacking a key falling back to the
    // base as `t` does.
    let mut purposes: BTreeMap<&str, BTreeMap<String, String>> = BTreeMap::new();
    for grant in devices.iter().filter(|g| g.device.purpose) {
        let base = strings
            .as_ref()
            .and_then(|s| s.base_table().get(grant.purpose));
        let (Some(strings), Some(base)) = (strings.as_ref(), base) else {
            let table = strings.as_ref().map_or_else(
                || "the app has no strings/ directory".to_string(),
                |s| format!("strings/{}.json has no \"{}\": add `\"{}\": \"…\"`, the text the person is shown when asked", s.base, grant.purpose, grant.purpose),
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
            let exception = exceptions.iter().find(|g| {
                line.split_whitespace().collect::<Vec<_>>()
                    == [&*format!("runtime.{}", g.exception.name), g.target]
            });
            let enforced = match (device, exception) {
                (Some(g), _) => device_enforcement(g.device, declared(line)),
                (_, Some(g)) => format!(
                    "hardened runtime (a macOS release signs {} with {}); declared by {}",
                    g.target,
                    g.exception.entitlement,
                    declared(line)
                ),
                _ => family_enforcement(line).to_string(),
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
    // An entitlement whose value is no boolean (LLP 1133.035 D16 item 2).
    let mut values = Map::new();
    let mut flags: Vec<&str> = Vec::new();
    // iOS's background modes and task identifiers (LLP 1133.017 D14).
    let (mut modes, mut tasks): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
    // Android's permissions: the Canvas host on Android bakes as Linux.
    let mut android: Vec<&str> = Vec::new();
    let mut features: Vec<&str> = Vec::new();
    // The reader's foreground services the lines declare, each with every
    // type its lines give it and the base locale's purpose text of the first
    // line naming it, its notification's title and channel name (LLP
    // 1133.033 D2, D4).
    let mut services = Map::new();
    let keys_of = |device: &Device| match platform {
        "ios" => device.ios,
        "macos" => device.macos,
        _ => &[],
    };
    // A key two granted rows write takes the text of the row whose own
    // prompt it is, the one listing it first (LLP 1133.019 D4: on macOS both
    // photo lines write `NSPhotoLibraryUsageDescription`, which takes
    // `device.photos`'s purpose), never whichever line came last; `shared`
    // names the line each such key took its text from.
    let mut writers: BTreeMap<&str, (usize, &str)> = BTreeMap::new();
    let mut shared = Map::new();
    for grant in &devices {
        for (at, key) in keys_of(grant.device).iter().enumerate() {
            if let Some(&(first, by)) = writers.get(key) {
                let line = if first <= at { by } else { grant.device.name };
                shared.insert((*key).into(), json!(format!("device.{line}")));
                if first <= at {
                    continue;
                }
            }
            writers.insert(key, (at, grant.device.name));
            usage.insert((*key).into(), json!(purposes[grant.purpose]));
        }
        // The platform's signing entitlements: macOS's hardened-runtime ones,
        // iOS's capability ones (LLP 1069.008.000 D2; push's `aps-environment`
        // joins when LLP 1069 §5 #9 lands). `auth.*`'s associated domains are
        // `reach.auth` (below).
        match platform {
            "macos" => entitlements.extend(grant.device.hardened),
            "ios" => {
                entitlements.extend(grant.device.ios_entitlements);
                for (name, value) in grant.device.ios_entitlement_values {
                    values.insert((*name).into(), json!(value));
                }
                flags.extend(grant.device.ios_flags);
                modes.extend(grant.device.ios_modes);
                tasks.extend(grant.device.ios_tasks);
            }
            "linux" => {
                android.extend(grant.device.android);
                features.extend(grant.device.android_features);
                for pair in grant.device.android_services {
                    let (kind, kind_type) = pair.split_once(':').unwrap_or((pair, ""));
                    let service = services
                        .entry(kind)
                        .or_insert_with(|| json!({ "purpose": text(grant.purpose), "types": [] }));
                    let types = service["types"].as_array_mut().expect("a service's types");
                    if !types.iter().any(|t| t == kind_type) {
                        types.push(json!(kind_type));
                    }
                }
            }
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
    // A usage key the app's use of a module name derives (`derives`, LLP
    // 1133.026 D19 item 4: `NSMotionUsageDescription` for a pedometer or an
    // altimeter name), written with its grant line's purpose; a line not on
    // TV drops it there.
    let mut derived_off_tv: Vec<String> = Vec::new();
    if let (Some(plan), "ios") = (plan, platform) {
        let text = contract::native::app_text(app_dir);
        for (line, key) in contract::native::derived(plan, &text, "ios") {
            let name = line.strip_prefix("device.").unwrap_or(&line);
            let Some(grant) = devices.iter().find(|g| g.device.name == name) else {
                continue;
            };
            if grant.device.purpose && !usage.contains_key(&key) {
                usage.insert(key.clone(), json!(purposes[grant.purpose]));
                if !grant.device.tvos {
                    derived_off_tv.push(key);
                }
            }
        }
    }
    // A runtime exception (D9) is macOS's alone, signed onto the one helper
    // it names and never joining the app's own entitlements.
    let mut helpers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for grant in exceptions.iter().filter(|_| platform == "macos") {
        helpers
            .entry(grant.target)
            .or_default()
            .push(grant.exception.entitlement);
    }
    for keys in helpers.values_mut() {
        keys.sort_unstable();
    }
    entitlements.sort_unstable();
    entitlements.dedup();
    for list in [&mut modes, &mut tasks] {
        list.sort_unstable();
        list.dedup();
    }
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
        omit.extend(derived_off_tv.iter().map(String::as_str));
        omit.sort_unstable();
        omit.dedup();
        omit
    } else {
        Vec::new()
    };
    let (bonjour, local_source, multicast) = local_network_reach(&ceiling);
    if multicast && platform == "linux" {
        android.push("android.permission.CHANGE_WIFI_MULTICAST_STATE");
    }
    android.sort_unstable();
    android.dedup();
    let auth = auth(&ceiling, platform, compat)?;
    let payments = payment_lines(&ceiling)?;
    compat.reach = json!({
        "base": base,
        "locales": strings.as_ref().map(|s| s.tables.keys().cloned().collect::<Vec<_>>()),
        "rows": rows,
        "usage": usage,
        "flags": flags,
        "android": android,
        "entitlements": entitlements,
        "helperEntitlements": helpers,
        "tvOmits": tv_omits,
        "auth": auth,
        "payments": payments,
    });
    if !modes.is_empty() {
        compat.reach["backgroundModes"] = json!(modes);
        compat.reach["tasks"] = json!(tasks);
    }
    if !shared.is_empty() {
        compat.reach["sharedKeys"] = Value::Object(shared);
    }
    if !services.is_empty() {
        compat.reach["androidServices"] = Value::Object(services);
    }
    if !values.is_empty() {
        compat.reach["entitlementValues"] = Value::Object(values);
    }
    features.sort_unstable();
    features.dedup();
    if !features.is_empty() {
        compat.reach["androidFeatures"] = json!(features);
    }
    // Apple's: tvOS keeps both, since it bakes the iOS plan and its
    // `URLSession` is under App Transport Security as iOS's is.
    if matches!(platform, "ios" | "macos") {
        if !bonjour.is_empty() {
            compat.reach["bonjour"] = json!(bonjour);
        }
        if local_source {
            compat.reach["localNetworking"] = json!(true);
        }
    }
    Ok(())
}

/// An app granting Health names its privacy policy (`privacy_policy`, LLP
/// 1133.026 D4): HealthKit requires one, and Health Connect shows its
/// permission screen only beside one. Refused where Android bakes (as
/// `linux`); a warning on iOS, where App Store review asks for it.
fn privacy_policy(
    app_dir: &Path,
    platform: &str,
    devices: &[exact_runner::device::DeviceGrant<'_>],
) -> Result<(), String> {
    let health = devices.iter().any(|g| g.device.name.starts_with("health-"));
    if !health || !matches!(platform, "linux" | "ios") {
        return Ok(());
    }
    let manifest: Value = std::fs::read_to_string(app_dir.join("app.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if (manifest["privacy_policy"].as_str()).is_some_and(|u| u.starts_with("https://")) {
        return Ok(());
    }
    let why = "grant-privacy-policy: an app granting `device.health-read` or `device.health-write` names its privacy policy in app.json, `\"privacy_policy\": \"https://…\"` (LLP 1133.026 D4)";
    if platform == "linux" {
        return Err(why.into());
    }
    if std::env::var_os("OUT_DIR").is_some() {
        println!("cargo:warning={why}");
    }
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
    production: bool,
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
        .chain(presence(app_dir, &sources))
        .chain(local_network(&sources, production))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

/// A presence line's refusals (LLP 1133.010 D4, D13): a name whose sources
/// keep it under two policies (`grant-policy-conflict`); a presence line in
/// an app that grants no `device.local-auth`, which Face ID will not run
/// without (`grant-missing`); and one in an app that does not depend on
/// `@exact/local-auth`, whose implementation shows the sheet.
fn presence(app_dir: &Path, sources: &[(Option<&str>, &str); 2]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut lines: BTreeMap<String, (String, &str)> = BTreeMap::new();
    let mut device = false;
    for (spec, source) in sources {
        let Some(Ok(set)) = spec.map(exact_runner::grants::parse) else {
            continue;
        };
        device |= set
            .lines()
            .iter()
            .any(|l| l.starts_with("device.local-auth "));
        for (name, policy) in set.presence() {
            let words = policy.line_words();
            match lines.get(name) {
                Some((other, at)) if *other != words => errors.push(format!(
                    "grant-policy-conflict: `secret.keep {name}` is `{other}` in {at} and `{words}` in {source}; a name is kept under one policy"
                )),
                Some(_) => {}
                None => {
                    lines.insert(name.to_owned(), (words, source));
                }
            }
        }
    }
    let Some((name, (words, _))) = lines.iter().next() else {
        return errors;
    };
    if !device {
        errors.push(format!(
            "grant-missing: `secret.keep {name} {words}` needs `device.local-auth <purposeKey>` beside it: the strings key whose text says why the app asks for Face ID"
        ));
    }
    let manifest = std::fs::read_to_string(app_dir.join("package.json")).unwrap_or_default();
    let depends = serde_json::from_str::<Value>(&manifest)
        .is_ok_and(|m| !m["dependencies"]["@exact/local-auth"].is_null());
    if !depends {
        errors.push(format!(
            "grant-missing: `secret.keep {name} {words}` needs `\"@exact/local-auth\"` in package.json's dependencies, whose implementation shows the check: `bun scripts/exact.mjs add local-auth {}`",
            app_dir.display()
        ));
    }
    errors
}

/// The local network's lines (LLP 1133.036 D5, D7): a `net.dns-sd` type
/// RFC 6335 does not allow (`grant-parse`); `net.dns-sd` or a
/// `local-network` source without `device.local-network`, since on Apple a
/// browse or a reach under them can show the Local Network prompt
/// (`grant-missing`); and in a production build the `development` word,
/// which only a development build's bake writes and which admits loopback.
fn local_network(sources: &[(Option<&str>, &str); 2], production: bool) -> Vec<String> {
    use exact_runner::grants::{is_service_type, DEVELOPMENT, LOCAL_NETWORK};
    let mut errors = Vec::new();
    let (mut device, mut needs) = (false, None);
    for (spec, source) in sources {
        let Some(Ok(set)) = spec.map(exact_runner::grants::parse) else {
            continue;
        };
        for line in set.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words[..] {
                ["device.local-network", ..] => device = true,
                ["net.dns-sd", kind] if !is_service_type(kind) => errors.push(format!(
                    "grant-parse: {source}: `{line}`: a DNS-SD service type is `_<name>._tcp` or `_<name>._udp`, the name 1 to 15 letters, digits and hyphens with a letter among them (RFC 6335 §5.1)"
                )),
                ["net.dns-sd", ..] => needs = needs.or(Some(*line)),
                ["net.fetch" | "net.websocket", LOCAL_NETWORK, ..] => {
                    needs = needs.or(Some(*line));
                    if production && words.get(2) == Some(&DEVELOPMENT) {
                        errors.push(format!(
                            "grant-parse: {source}: `{line}`: a production build admits no loopback by this line; write `{} {LOCAL_NETWORK}`, and name a loopback origin the app serves itself on",
                            words[0]
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    if let (Some(line), false) = (needs, device) {
        errors.push(format!(
            "grant-missing: `{line}` needs `device.local-network <purposeKey>` beside it: on Apple a browse or a reach under it can show the Local Network prompt, with that strings key's text"
        ));
    }
    errors
}

/// What the local network's lines derive (LLP 1133.036 D7): each
/// `net.dns-sd` type, for `NSBonjourServices` on Apple; whether a
/// `local-network` source lets `URLSession` load `http:` from local hosts
/// (`NSAllowsLocalNetworking`); and on Android, with a type, the permission
/// `WifiManager.MulticastLock` needs where the system does not manage it.
fn local_network_reach(ceiling: &str) -> (Vec<String>, bool, bool) {
    let lines = || {
        ceiling
            .lines()
            .map(|l| l.split_whitespace().collect::<Vec<_>>())
    };
    let mut types: Vec<String> = lines()
        .filter_map(|w| (w.len() == 2 && w[0] == "net.dns-sd").then(|| w[1].to_string()))
        .collect();
    types.sort_unstable();
    types.dedup();
    let source = lines().any(|w| {
        matches!(
            w[..],
            [
                "net.fetch" | "net.websocket",
                exact_runner::grants::LOCAL_NETWORK,
                ..
            ]
        )
    });
    let multicast = !types.is_empty();
    (types, source, multicast)
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
    // `@exact/sign-in`'s lines open the same popup (LLP 1133.014 D5).
    let typescript = compat.inputs["javascriptGrants"]
        .as_str()
        .or(compat.inputs["grantCeiling"].as_str())
        .is_some_and(|g| {
            g.lines().any(|l| {
                let l = l.trim();
                ["auth.session ", "auth.apple ", "auth.google "]
                    .iter()
                    .any(|p| l.starts_with(p))
            })
        });
    if platform == "web"
        && typescript
        && compat.inputs["typescriptPlacement"].as_str() == Some("worker")
    {
        return Err("grant-auth: `auth.session`, `auth.apple` or `auth.google` in a worker-placed TypeScript source: on the web the sign-in popup opens in the press's call stack, which a worker is not in (LLP 1069.006, LLP 1133.014 D5); place the source on main (`typescript.placement`)".into());
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
    let passkeys = passkey_rp_ids(ceiling)?;
    let mut webcredentials = webcredentials;
    for rp in passkeys.iter().filter(|rp| *rp != "localhost") {
        let domain = format!("webcredentials:{rp}");
        if !webcredentials.contains(&domain) {
            webcredentials.push(domain);
        }
    }
    Ok(json!({
        "sessions": sessions,
        "callbacks": callbacks,
        "passkeys": passkeys,
        "signIn": sign_in(ceiling, platform)?,
        "associatedDomains": if matches!(platform, "ios" | "macos") { webcredentials } else { Vec::new() },
    }))
}

/// What `auth.apple <client id>` and `auth.google <client id>` derive (LLP
/// 1133.014 D8): the clients, the audiences a token from this target
/// carries, which the app's server must accept, and the build's notes: on
/// the web each provider's web client registers the server's return route;
/// on Apple a build with Google and no Apple prints App Review 4.8, never a
/// refusal. Apple's native audience is the bundle ID, which the Apple build
/// prints beside these.
fn sign_in(ceiling: &str, platform: &str) -> Result<Value, String> {
    let mut apple: Vec<String> = Vec::new();
    let mut google: Vec<String> = Vec::new();
    for line in ceiling.lines().map(str::trim) {
        let mut words = line.split_whitespace();
        let into = match words.next() {
            Some("auth.apple") => &mut apple,
            Some("auth.google") => &mut google,
            _ => continue,
        };
        let (Some(id), None) = (words.next(), words.next()) else {
            return Err(format!("grant-auth: `{line}`: `auth.apple` and `auth.google` take one client id each (LLP 1133.014 D8)"));
        };
        if !into.iter().any(|i| i == id) {
            into.push(id.to_owned());
        }
    }
    let mut notes: Vec<String> = Vec::new();
    if platform == "web" && !(apple.is_empty() && google.is_empty()) {
        notes.push("sign-in: each provider's web client registers the app server's return route (`web.redirectURI`) as its redirect URI, never the page (LLP 1133.014 D3)".into());
    }
    if matches!(platform, "ios" | "macos") && apple.is_empty() && !google.is_empty() {
        notes.push("sign-in: App Review 4.8: an app whose primary account is a third-party login also offers an equivalent that limits data to name and email, keeps the email private if the person asks, and does not track; Sign in with Apple meets it. Exceptions: the company's own accounts, education and enterprise apps, a client for a specific third-party service".into());
    }
    let audiences = match platform {
        "web" => json!({ "apple": apple, "google": google }),
        "android" => json!({ "google": google }),
        _ => json!({ "apple": ["<the bundle ID>"], "google": google }),
    };
    Ok(json!({ "apple": apple, "google": google, "audiences": audiences, "notes": notes }))
}

/// The RP IDs `auth.passkey <rp id>` grants (LLP 1133.011 D6): each a
/// domain (`example.com`, `login.example.com`) or `localhost`, which only the
/// web admits. A scheme, port, path, IP address or uppercase letter refuses,
/// since WebAuthn compares the RP ID with the origin's effective domain as
/// written (§5.1.3 step 7).
fn passkey_rp_ids(ceiling: &str) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    for line in ceiling.lines().map(str::trim) {
        let mut words = line.split_whitespace();
        if words.next() != Some("auth.passkey") {
            continue;
        }
        let (Some(rp), None) = (words.next(), words.next()) else {
            return Err(format!("grant-auth: `{line}`: `auth.passkey` takes one RP ID, a domain such as `example.com`, or `localhost` (LLP 1133.011 D6)"));
        };
        let label = |l: &str| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        };
        let labels: Vec<&str> = rp.split('.').collect();
        let numeric = labels
            .last()
            .is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()));
        if rp != "localhost" && (labels.len() < 2 || numeric || !labels.iter().all(|l| label(l))) {
            return Err(format!("grant-auth: `{line}`: an RP ID is a domain in lowercase with no scheme, port or path (`example.com`, `login.example.com`), or `localhost`; an IP address is not one (LLP 1133.011 D6)"));
        }
        if !ids.iter().any(|i| i == rp) {
            ids.push(rp.to_owned());
        }
    }
    Ok(ids)
}

/// The wallets `payment.*` grants (LLP 1133.034 D13): each Apple Pay
/// merchant ID (`payment.apple-pay merchant.com.example.app`), which the
/// Apple build lists in `com.apple.developer.in-app-payments`, and whether
/// Google Pay is granted (`payment.google-pay`, no target), which derives
/// Android's `wallet.api.enabled` meta-data and the callbacks service.
fn payment_lines(ceiling: &str) -> Result<Value, String> {
    let mut merchants: Vec<String> = Vec::new();
    let mut google = false;
    for line in ceiling.lines().map(str::trim) {
        let mut words = line.split_whitespace();
        match (words.next(), words.next(), words.next()) {
            (Some("payment.google-pay"), None, _) => google = true,
            (Some("payment.google-pay"), Some(_), _) => {
                return Err(format!("grant-payment: `{line}`: `payment.google-pay` names no target (LLP 1133.034 D13)"))
            }
            (Some("payment.apple-pay"), Some(id), None)
                if id.starts_with("merchant.")
                    && id.split('.').all(|l| {
                        !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    }) =>
            {
                if !merchants.iter().any(|m| m == id) {
                    merchants.push(id.to_owned());
                }
            }
            (Some("payment.apple-pay"), _, _) => {
                return Err(format!("grant-payment: `{line}`: `payment.apple-pay` takes one merchant ID, `merchant.` and a reverse domain, such as `merchant.com.example.app` (LLP 1133.034 D13)"))
            }
            _ => {}
        }
    }
    Ok(json!({ "merchantIds": merchants, "googlePay": google }))
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
        "auth.passkey" => {
            "the runner's admission (the RP ID); the platform's association check (Apple, Android)"
        }
        "doc.keep" => "the host's kept documents (macOS; Linux and the web keep nothing yet)",
        "payment.apple-pay" | "payment.google-pay" => {
            "the runner's admission (the merchant ID); the wallet's own merchant check (Apple, Google)"
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
            painter: None,
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
        let error = derive(&dir.0, "ios", None, &mut c).unwrap_err();
        assert_eq!(error.lines().count(), 2, "{error}");
        assert!(
            error.lines().all(|l| l.starts_with("grant-purpose: ")),
            "{error}"
        );
        assert!(error.contains("strings/en.json has no \"purpose.where\""));
        let bare = tempdir::Dir::new();
        let error = derive(&bare.0, "ios", None, &mut compat("device.microphone p")).unwrap_err();
        assert!(error.contains("no strings/ directory"), "{error}");
        let error = derive(&bare.0, "ios", None, &mut compat("device.lidar p")).unwrap_err();
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
            derive(&dir.0, "macos", None, &mut c).unwrap();
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
        derive(&dir.0, "macos", None, &mut c).unwrap();
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
        derive(&dir.0, "ios", None, &mut c).unwrap();
        assert_eq!(c.id, "old");
        assert!(c.inputs.get("grantPurposes").is_none());
        assert_eq!(c.reach["usage"], json!({}));
        let mut ios = compat("device.speech-recognition p");
        let dir = app(&[("en", r#"{"p": "Transcribes."}"#)]);
        derive(&dir.0, "ios", None, &mut ios).unwrap();
        assert_eq!(
            ios.reach["usage"],
            json!({"NSSpeechRecognitionUsageDescription": {"en": "Transcribes."}})
        );
        assert_eq!(ios.reach["entitlements"], json!([]));
    }

    #[test]
    fn background_lines_derive_ios_modes_and_the_task_identifiers() {
        // LLP 1133.017 D14: `fetch` and the refresh identifier for a periodic
        // line, `processing` and two processing identifiers too for a sync
        // one, each once; the Mac needs none.
        let dir = app(&[("en", "{}")]);
        let mut periodic = compat("device.periodic-background-sync");
        derive(&dir.0, "ios", None, &mut periodic).unwrap();
        assert_eq!(periodic.reach["backgroundModes"], json!(["fetch"]));
        assert_eq!(periodic.reach["tasks"], json!(["refresh"]));
        let grants = "device.background-sync\ndevice.periodic-background-sync";
        let mut ios = compat(grants);
        derive(&dir.0, "ios", None, &mut ios).unwrap();
        assert_eq!(ios.reach["backgroundModes"], json!(["fetch", "processing"]));
        assert_eq!(
            ios.reach["tasks"],
            json!(["processing.network", "processing.network-power", "refresh"])
        );
        assert_eq!(ios.reach["usage"], json!({}));
        let mut mac = compat(grants);
        derive(&dir.0, "macos", None, &mut mac).unwrap();
        assert!(mac.reach.get("backgroundModes").is_none());
    }

    /// LLP 1133.033 D3: one line derives iOS's `audio` mode with the key the
    /// capture reads, and Android's normal permissions with the `capture`
    /// service named by its purpose's text.
    #[test]
    fn the_background_capture_line_derives_audio_and_the_capture_service() {
        let dir = app(&[("en", r#"{"mic": "Records.", "bg": "Recording"}"#)]);
        let grants = "device.microphone mic\ndevice.microphone-background bg";
        let mut ios = compat(grants);
        derive(&dir.0, "ios", None, &mut ios).unwrap();
        assert_eq!(ios.reach["backgroundModes"], json!(["audio"]));
        assert_eq!(ios.reach["flags"], json!(["ExactCapturesInBackground"]));
        let mut android = compat(grants);
        derive(&dir.0, "linux", None, &mut android).unwrap();
        assert_eq!(
            android.reach["android"],
            json!([
                "android.permission.FOREGROUND_SERVICE",
                "android.permission.FOREGROUND_SERVICE_MICROPHONE",
                "android.permission.RECORD_AUDIO"
            ])
        );
        assert_eq!(
            android.reach["androidServices"],
            json!({"capture": {"purpose": "Recording", "types": ["microphone"]}})
        );
        let mut mac = compat(grants);
        derive(&dir.0, "macos", None, &mut mac).unwrap();
        assert!(
            mac.reach.get("backgroundModes").is_none()
                && mac.reach.get("androidServices").is_none()
        );
        let mut alone = compat("device.microphone-background bg");
        let refused = derive(&dir.0, "ios", None, &mut alone).unwrap_err();
        assert!(refused.contains("grant-needs"), "{refused}");
    }

    #[test]
    fn calls_stay_out_of_a_production_build() {
        let dir = app(&[("en", r#"{"mic": "Records.", "calls": "Test calls."}"#)]);
        let grants = "device.microphone mic\ndevice.calls calls";
        let mut development = compat(grants);
        derive(&dir.0, "ios", None, &mut development).unwrap();
        assert_eq!(
            development.reach["backgroundModes"],
            json!(["audio", "voip"])
        );
        let mut android = compat(grants);
        derive(&dir.0, "linux", None, &mut android).unwrap();
        assert_eq!(
            android.reach["androidServices"],
            json!({"call": {"purpose": "Test calls.", "types": ["phoneCall", "microphone"]}})
        );
        let mut production = compat(grants);
        production.embedded = json!({ "trust": "production" });
        let refused = derive(&dir.0, "ios", None, &mut production).unwrap_err();
        assert!(refused.contains("§13 question 4"), "{refused}");
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
            derive(&dir.0, platform, None, &mut c).unwrap();
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
    fn health_on_android_names_a_privacy_policy() {
        // LLP 1133.026 D4.
        let dir = app(&[("en", r#"{"read": "Reads your steps."}"#)]);
        let mut android = compat("device.health-read read");
        let refused = derive(&dir.0, "linux", None, &mut android).unwrap_err();
        assert!(refused.starts_with("grant-privacy-policy"), "{refused}");
        let mut ios = compat("device.health-read read");
        derive(&dir.0, "ios", None, &mut ios).unwrap();
        let named = r#"{"name": "Steps", "app": {"id": "a.b", "name": "Steps"}, "privacy_policy": "https://example.com/privacy"}"#;
        std::fs::write(dir.0.join("app.json"), named).unwrap();
        let mut android = compat("device.health-read read");
        derive(&dir.0, "linux", None, &mut android).unwrap();
    }

    #[test]
    fn both_photo_lines_write_the_read_key_once_with_the_read_purpose() {
        // LLP 1133.019 D4.
        let dir = app(&[(
            "en",
            r#"{"recents": "Shows recent photos.", "save": "Saves edits."}"#,
        )]);
        let derive_for = |grants: &str, platform: &str| {
            let mut c = compat(grants);
            derive(&dir.0, platform, None, &mut c).unwrap();
            c.reach
        };
        let (read, add) = (
            "NSPhotoLibraryUsageDescription",
            "NSPhotoLibraryAddUsageDescription",
        );
        let recents = json!({"en": "Shows recent photos."});
        let save = json!({"en": "Saves edits."});
        for grants in [
            "device.photos-add save\ndevice.photos recents",
            "device.photos recents\ndevice.photos-add save",
        ] {
            let mac = derive_for(grants, "macos");
            assert_eq!(mac["usage"], json!({read: recents, add: save}));
            assert_eq!(mac["sharedKeys"], json!({read: "device.photos"}));
            let ios = derive_for(grants, "ios");
            assert_eq!(ios["usage"], json!({read: recents, add: save}));
            assert_eq!(
                ios["flags"],
                json!(["PHPhotoLibraryPreventAutomaticLimitedAccessAlert"])
            );
        }
        // The add line alone on macOS: the read key with the add's text.
        let mac = derive_for("device.photos-add save", "macos");
        assert_eq!(mac["usage"], json!({add: save, read: save}));
        assert_eq!(
            mac["entitlements"],
            json!(["com.apple.security.personal-information.photos-library"])
        );
        // Android: adding asks nothing; reading is a permission per API range,
        // the one that ends at 32 with its end.
        assert_eq!(
            derive_for("device.photos-add save", "linux")["android"],
            json!([])
        );
        assert_eq!(
            derive_for("device.photos-add save\ndevice.photos recents", "linux")["android"],
            json!([
                "android.permission.ACCESS_MEDIA_LOCATION",
                "android.permission.READ_EXTERNAL_STORAGE maxSdkVersion=32",
                "android.permission.READ_MEDIA_IMAGES",
                "android.permission.READ_MEDIA_VISUAL_USER_SELECTED",
            ])
        );
    }

    #[test]
    fn local_network_writes_its_purpose_on_apple_hosts_only() {
        let dir = app(&[
            ("en", r#"{"lan": "Finds servers nearby."}"#),
            ("fr", r#"{"lan": "Trouve les serveurs proches."}"#),
        ]);
        let text = json!({"en": "Finds servers nearby.", "fr": "Trouve les serveurs proches."});
        for platform in ["ios", "macos"] {
            let mut c = compat("device.local-network lan");
            derive(&dir.0, platform, None, &mut c).unwrap();
            assert_eq!(
                c.reach["usage"],
                json!({"NSLocalNetworkUsageDescription": text})
            );
            assert_eq!(c.reach["entitlements"], json!([]));
            assert_ne!(c.id, "old", "the purpose joins the compatibility id");
        }
        for platform in ["web", "linux"] {
            let mut c = compat("device.local-network lan");
            derive(&dir.0, platform, None, &mut c).unwrap();
            assert_eq!(c.reach["usage"], json!({}));
            assert_eq!(c.reach["android"], json!([]));
        }
    }

    #[test]
    fn nfc_signs_its_formats_array_on_ios_and_leaves_tvos_out() {
        // LLP 1133.035 D15, D16 item 2.
        let dir = app(&[("en", r#"{"nfc": "Reads the tags on your books."}"#)]);
        let derive_for = |platform: &str| {
            let mut c = compat("device.nfc nfc");
            derive(&dir.0, platform, None, &mut c).unwrap();
            c.reach
        };
        let formats = "com.apple.developer.nfc.readersession.formats";
        let ios = derive_for("ios");
        assert_eq!(
            ios["usage"],
            json!({"NFCReaderUsageDescription": {"en": "Reads the tags on your books."}})
        );
        assert_eq!(ios["entitlements"], json!([formats]));
        assert_eq!(ios["entitlementValues"], json!({formats: ["TAG"]}));
        assert_eq!(
            ios["tvOmits"],
            json!(["NFCReaderUsageDescription", formats])
        );
        // Android's bake derives as `linux`.
        assert_eq!(
            derive_for("linux")["android"],
            json!(["android.permission.NFC"])
        );
        let mac = derive_for("macos");
        assert_eq!(mac["entitlements"], json!([]));
        assert!(mac.get("entitlementValues").is_none());
    }

    #[test]
    fn a_presence_line_needs_its_permission_line_its_module_and_one_policy() {
        let dir = app(&[("en", r#"{"purpose.lock": "Unlocks your session."}"#)]);
        let line = "secret.keep token user-presence optional";
        let refused = parsed(&dir.0, Some(line), None, false).unwrap_err();
        assert!(
            refused.contains("grant-missing: `secret.keep token user-presence optional` needs `device.local-auth"),
            "{refused}"
        );
        assert!(refused.contains("\"@exact/local-auth\""), "{refused}");
        std::fs::write(
            dir.0.join("package.json"),
            r#"{"dependencies":{"@exact/local-auth":"file:../packages/local-auth"}}"#,
        )
        .unwrap();
        let both = format!("{line}\ndevice.local-auth purpose.lock");
        parsed(&dir.0, Some(&both), None, false).unwrap();
        let rust = "secret.keep token biometry-any\ndevice.local-auth purpose.lock";
        let conflict = parsed(&dir.0, Some(&both), Some(rust), false).unwrap_err();
        assert!(
            conflict.starts_with("grant-policy-conflict: "),
            "{conflict}"
        );
        let unknown = parsed(&dir.0, Some("secret.keep token face-id"), None, false).unwrap_err();
        assert!(unknown.contains("grant-policy-unknown"), "{unknown}");
        // Face ID's usage key on iOS, nothing on macOS, the biometric permission on Android.
        let text = json!({"en": "Unlocks your session."});
        let mut ios = compat(&both);
        derive(&dir.0, "ios", None, &mut ios).unwrap();
        assert_eq!(
            ios.reach["usage"],
            json!({"NSFaceIDUsageDescription": text})
        );
        let mut mac = compat(&both);
        derive(&dir.0, "macos", None, &mut mac).unwrap();
        assert_eq!(mac.reach["usage"], json!({}));
        let mut android = compat(&both);
        derive(&dir.0, "linux", None, &mut android).unwrap();
        assert_eq!(
            android.reach["android"],
            json!(["android.permission.USE_BIOMETRIC"])
        );
    }

    #[test]
    fn the_local_network_s_lines_need_their_permission_and_derive_apple_s_and_android_s_keys() {
        // LLP 1133.036 D5, D7.
        let dir = app(&[("en", r#"{"lan": "Finds your lights."}"#)]);
        let lines = "net.dns-sd _wled._tcp\nnet.fetch local-network";
        let refused = parsed(&dir.0, Some(lines), None, false).unwrap_err();
        assert!(
            refused
                .starts_with("grant-missing: `net.dns-sd _wled._tcp` needs `device.local-network"),
            "{refused}"
        );
        let bad = parsed(
            &dir.0,
            Some("net.dns-sd _wled._sctp\ndevice.local-network lan"),
            None,
            false,
        );
        let bad = bad.unwrap_err();
        assert!(
            bad.starts_with("grant-parse: the Rust source: `net.dns-sd _wled._sctp`"),
            "{bad}"
        );
        let grants = format!("{lines}\ndevice.local-network lan");
        parsed(&dir.0, Some(&grants), None, false).unwrap();
        let development = format!("{grants}\nnet.websocket local-network development");
        parsed(&dir.0, Some(&development), None, false).unwrap();
        let production = parsed(&dir.0, Some(&development), None, true).unwrap_err();
        assert!(
            production.contains("a production build admits no loopback"),
            "{production}"
        );
        for platform in ["ios", "macos"] {
            let mut c = compat(&grants);
            derive(&dir.0, platform, None, &mut c).unwrap();
            assert_eq!(c.reach["bonjour"], json!(["_wled._tcp"]), "{platform}");
            assert_eq!(c.reach["localNetworking"], json!(true), "{platform}");
        }
        let mut android = compat(&grants);
        derive(&dir.0, "linux", None, &mut android).unwrap();
        assert_eq!(
            android.reach["android"],
            json!(["android.permission.CHANGE_WIFI_MULTICAST_STATE"])
        );
        assert!(android.reach["bonjour"].is_null());
        let mut fetch_only = compat("net.fetch local-network\ndevice.local-network lan");
        derive(&dir.0, "ios", None, &mut fetch_only).unwrap();
        assert!(fetch_only.reach["bonjour"].is_null());
        assert_eq!(fetch_only.reach["localNetworking"], json!(true));
    }

    #[test]
    fn runtime_exceptions_sign_their_own_executable_on_macos_only() {
        // LLP 1069.008 D9.
        let grants = "runtime.allow-jit Helpers/node/bin/node\nruntime.disable-library-validation Helpers/node/bin/node\nruntime.allow-unsigned-executable-memory Helpers/tool/run\nnet.fetch https://x/";
        let dir = tempdir::Dir::new();
        let mut mac = compat(grants);
        derive(&dir.0, "macos", None, &mut mac).unwrap();
        assert_eq!(mac.id, "old", "no purpose, no id move");
        assert_eq!(mac.reach["entitlements"], json!([]), "never the app's");
        assert_eq!(
            mac.reach["helperEntitlements"],
            json!({
                "Helpers/node/bin/node": [
                    "com.apple.security.cs.allow-jit",
                    "com.apple.security.cs.disable-library-validation"
                ],
                "Helpers/tool/run": ["com.apple.security.cs.allow-unsigned-executable-memory"]
            })
        );
        assert!(mac.reach["rows"][0]["enforced"]
            .as_str()
            .unwrap()
            .starts_with("hardened runtime (a macOS release signs Helpers/node/bin/node with com.apple.security.cs.allow-jit)"));
        let mut ios = compat(grants);
        derive(&dir.0, "ios", None, &mut ios).unwrap();
        assert_eq!(ios.reach["entitlements"], json!([]));
        assert_eq!(ios.reach["helperEntitlements"], json!({}));
        let error = derive(
            &dir.0,
            "macos",
            None,
            &mut compat("runtime.allow-jit MacOS/Exact\ndevice.lidar p\nruntime.allow-jit app"),
        )
        .unwrap_err();
        assert_eq!(error.lines().count(), 3, "{error}");
        assert!(error
            .lines()
            .skip(1)
            .all(|l| l.starts_with("grant-runtime: ")));
        assert!(error.contains("only helper executables are admitted"));
    }

    #[test]
    fn auth_lines_derive_callbacks_and_apple_s_webcredentials() {
        let grants = "auth.session https://bsky.social\nauth.callback social.exact.x:/oauth\nauth.callback https://app.example/.exact/auth/callback\nauth.callback http://127.0.0.1:9/cb";
        let dir = tempdir::Dir::new();
        let mut ios = compat(grants);
        derive(&dir.0, "ios", None, &mut ios).unwrap();
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
        derive(&dir.0, "web", None, &mut web).unwrap();
        assert_eq!(web.reach["auth"]["associatedDomains"], json!([]));
        let passkeys = format!(
            "{grants}\nauth.passkey app.example\nauth.passkey login.test\nauth.passkey localhost"
        );
        let mut ios = compat(&passkeys);
        derive(&dir.0, "ios", None, &mut ios).unwrap();
        assert_eq!(
            ios.reach["auth"]["associatedDomains"],
            json!(["webcredentials:app.example", "webcredentials:login.test"])
        );
        assert_eq!(
            ios.reach["auth"]["passkeys"],
            json!(["app.example", "login.test", "localhost"])
        );
        for bad in [
            "https://a.test",
            "a.test:443",
            "127.0.0.1",
            "A.test",
            "test",
            "a.test b",
        ] {
            let error = derive(
                &dir.0,
                "ios",
                None,
                &mut compat(&format!("auth.passkey {bad}")),
            )
            .unwrap_err();
            assert!(error.starts_with("grant-auth: "), "{bad}: {error}");
        }
        web.inputs["typescriptPlacement"] = json!("worker");
        assert!(derive(&dir.0, "web", None, &mut web)
            .unwrap_err()
            .starts_with("grant-auth: "));
        let mut native = compat(grants);
        native.inputs["typescriptPlacement"] = json!("worker");
        assert!(
            derive(&dir.0, "macos", None, &mut native).is_ok(),
            "a native worker opens its sheet"
        );
    }
}
