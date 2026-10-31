use super::{compatibility_with_trust, Manifest};
use std::path::{Path, PathBuf};

#[test]
fn the_baked_painter_round_trips_per_host_without_changing_bundle_compatibility() {
    let dir = app("painter-config");
    let manifest = Manifest::read(&dir).unwrap();
    for (platform, target, host) in [
        ("windows", "x86_64-pc-windows-msvc", "windows"),
        ("linux", "x86_64-unknown-linux-gnu", "linux"),
        ("linux", "aarch64-apple-darwin", "linux"),
        ("android", "aarch64-linux-android", "android"),
        ("linux", "aarch64-linux-android", "android"),
    ] {
        let build = |m: &Manifest| {
            compatibility_with_trust(&dir, platform, target, m, Some(""), "development").unwrap()
        };
        let default = build(&manifest);
        let mut selected = manifest.clone();
        selected.json["host"] = serde_json::json!({ host: {"painter": "ink"} });
        let selected = build(&selected);
        let json: serde_json::Value = serde_json::from_str(&selected.to_json()).unwrap();
        assert_eq!(json["painter"], "ink", "{platform} on {target}");
        assert_eq!(
            selected.id, default.id,
            "a painter change preserves the cohort"
        );
        let mut unchanged = json;
        unchanged.as_object_mut().unwrap().remove("painter");
        assert_eq!(
            unchanged,
            serde_json::from_str::<serde_json::Value>(&default.to_json()).unwrap()
        );

        let mut other = manifest.clone();
        let other_host = if host == "windows" {
            "linux"
        } else {
            "windows"
        };
        other.json["host"] = serde_json::json!({other_host: {"painter": "ink"}});
        assert_eq!(
            build(&other).to_json(),
            default.to_json(),
            "another platform is unchanged"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn resolved_manifest_is_the_only_reader_dialect() {
    let dir = app("resolved-only");
    for game in [
        serde_json::json!({"crate":"beacons-logic","type":"Beacons"}),
        serde_json::Value::Null,
    ] {
        std::fs::write(
            dir.join("app.json"),
            serde_json::json!({"id":"com.exact.beacons","name":"Beacons","game":game}).to_string(),
        )
        .unwrap();
        assert!(Manifest::read(&dir).is_err());
    }
    std::fs::write(
        dir.join("app.json"),
        r#"{"app":{"id":"com.exact.beacons","name":"Beacons"}}"#,
    )
    .unwrap();
    assert_eq!(Manifest::read(&dir).unwrap().id, "com.exact.beacons");
    std::fs::create_dir_all(dir.join(".shells")).unwrap();
    std::fs::write(
        dir.join(".shells/app.json"),
        r#"{"app":{"id":"com.exact.resolved","name":"Resolved"}}"#,
    )
    .unwrap();
    std::fs::write(dir.join("app.json"), "{}").unwrap();
    assert_eq!(Manifest::read(&dir).unwrap().id, "com.exact.resolved");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn widget_gallery_labels_require_a_new_binary_cohort() {
    let dir = app("widget-label-cohort");
    let mut manifest = Manifest::read(&dir).unwrap();
    manifest.json["widgets"] = serde_json::json!([{"kind":"card","families":["systemSmall"],
        "component":"Card","props":"Props","refresh":"refresh","preview":{}}]);
    let build = |manifest: &Manifest| {
        compatibility_with_trust(
            &dir,
            "ios",
            "aarch64-apple-ios",
            manifest,
            Some(""),
            "development",
        )
        .unwrap()
    };
    let original = build(&manifest);
    for key in ["title", "description"] {
        let mut labelled = manifest.clone();
        labelled.json["widgets"][0][key] = serde_json::json!("Next trains");
        assert_ne!(original.id, build(&labelled).id, "{key} is generated Swift");
    }
    let mut account = manifest.clone();
    account.json["widgets"][0]["requiresSecrets"] = serde_json::json!(true);
    assert_ne!(
        original.id,
        build(&account).id,
        "account privacy is a binary policy"
    );
    let mut watch = manifest.clone();
    watch.json["widgets"][0]["watchFamilies"] = serde_json::json!(["accessoryCorner"]);
    assert_ne!(
        original.id,
        build(&watch).id,
        "watch families are generated Swift"
    );
    manifest.json["widgets"][0]["preview"] = serde_json::json!({"title":"new preview"});
    assert_eq!(
        original.id,
        build(&manifest).id,
        "preview props ride the bundle"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn activity_state_rides_the_bundle_and_capabilities_require_a_binary() {
    let dir = app("activity-cohort");
    let mut m = Manifest::read(&dir).unwrap();
    m.json["live_activities"] =
        serde_json::json!([{"kind":"train","component":"Train","state":"State","preview":{}}]);
    let build = |m: &Manifest| {
        compatibility_with_trust(&dir, "ios", "aarch64-apple-ios", m, Some(""), "development")
            .unwrap()
    };
    let original = build(&m);
    for key in ["title", "description", "push", "frequent"] {
        let mut changed = m.clone();
        changed.json["live_activities"][0][key] = if key == "push" || key == "frequent" {
            serde_json::json!(true)
        } else {
            serde_json::json!("Train")
        };
        assert_ne!(
            original.id,
            build(&changed).id,
            "{key} changes generated Swift or entitlements"
        );
    }
    m.json["live_activities"][0]["state"] = serde_json::json!("NextState");
    m.json["live_activities"][0]["preview"] = serde_json::json!({"title":"New state"});
    assert_eq!(
        original.id,
        build(&m).id,
        "state shapes and preview values ride the bundle"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn list_scroll_offset_requires_a_new_binary_cohort() {
    use sha2::{Digest, Sha256};
    let digest = |list_shape: &str| {
        let mut h = Sha256::new();
        for name in contract_syntax::HOST_COMMANDS {
            let shape = match *name {
                "share" => "3",
                "stopSounds" => "1",
                "playSounds" => "1:4|5|6",
                "playSound" => "6",
                "sendMIDI" => "3",
                "scrollIntoView" => list_shape,
                "saveFile" => "3|4",
                _ => "positional",
            };
            h.update(format!("{name}:{shape}\n"));
        }
        h.finalize()[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let dir = app("scroll-offset-cohort");
    let manifest = Manifest::read(&dir).unwrap();
    let current = compatibility_with_trust(
        &dir,
        "macos",
        "aarch64-apple-darwin",
        &manifest,
        Some(""),
        "development",
    )
    .unwrap();
    let mut previous = current.inputs.clone();
    previous["commands"] = serde_json::json!(digest("4|6"));
    assert_eq!(current.inputs["commands"], digest("4|7"));
    assert_ne!(current.id, super::compatibility_digest(&previous));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mixed_source_ownership_is_hashed_even_when_the_host_ceiling_is_unchanged() {
    let dir = app("mixed-source-grants");
    let manifest = Manifest::read(&dir).unwrap();
    let mut compat = compatibility_with_trust(
        &dir,
        "web",
        "wasm32-unknown-unknown",
        &manifest,
        Some(""),
        "development",
    )
    .unwrap();
    let javascript = "net.fetch https://example.test/";
    let rust = "fs.read app:/data/rust";
    super::source_scopes(&mut compat, javascript, rust);
    assert_eq!(
        compat.inputs["grantCeiling"],
        format!("{rust}\n{javascript}")
    );
    assert_eq!(compat.inputs["javascriptGrants"], javascript);
    assert_eq!(compat.inputs["rustGrants"], rust);
    assert_eq!(compat.inputs["commands"], super::commands_digest());
    let mut without_commands = compat.inputs.clone();
    without_commands.as_object_mut().unwrap().remove("commands");
    assert_ne!(compat.id, super::compatibility_digest(&without_commands));
    let first = compat.id.clone();
    super::source_scopes(&mut compat, &format!("{rust}\n{javascript}"), rust);
    assert_eq!(
        compat.inputs["grantCeiling"],
        format!("{rust}\n{javascript}")
    );
    assert_ne!(
        compat.id, first,
        "a child gaining a sibling's grant changes compatibility"
    );
    let second = compat.id.clone();
    super::source_scopes(&mut compat, &format!("{rust}\n{javascript}"), javascript);
    assert_ne!(
        compat.id, second,
        "Rust grant ownership is hashed independently"
    );
    let spelling = format!("{javascript}\n{rust}\n");
    super::source_scopes(&mut compat, &spelling, rust);
    assert_eq!(
        compat.inputs["grantCeiling"], spelling,
        "subset sources retain the JS receipt spelling like Mixed"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rust_policy_inherits_and_refuses_invalid_configuration() {
    let mut m = Manifest {
        json: serde_json::json!({}),
        id: "com.example.app".into(),
        name: "App".into(),
        declared: true,
    };
    for (platform, dev, prod) in [
        ("web", "browser", "browser"),
        ("ios", "wasm", "wasm"),
        ("macos", "native", "native"),
        ("linux", "native", "native"),
        ("android", "native", "wasm"),
        ("windows", "native", "native"),
    ] {
        assert_eq!(m.rust_mode(platform, true).unwrap(), dev);
        assert_eq!(m.rust_mode(platform, false).unwrap(), prod);
    }
    m.json["rust"] = serde_json::json!({"mode":"off","dev":true,"prod":false,"module":{"package":"app-data"},"platforms":{"ios":{"mode":"wasm","prod":false},"linux":{"prod":true}}});
    assert_eq!(m.rust_mode("ios", true).unwrap(), "wasm");
    assert_eq!(m.rust_mode("ios", false).unwrap(), "off");
    assert_eq!(m.rust_mode("linux", false).unwrap(), "native");
    assert_eq!(m.rust_mode("macos", false).unwrap(), "off");
    m.json["rust"] = serde_json::json!({"prod":false,"platforms":{"ios":true}});
    assert_eq!(m.rust_mode("ios", false).unwrap(), "wasm");
    for policy in [
        serde_json::json!(null),
        serde_json::json!("jit"),
        serde_json::json!({"enabled":false}),
        serde_json::json!({"mode":false}),
        serde_json::json!({"platforms":{"iphone":false}}),
        serde_json::json!({"dev":{"prod":false}}),
        serde_json::json!({"module":{"package":"bad/path"}}),
        serde_json::json!({"module":{}}),
    ] {
        m.json["rust"] = policy;
        assert!(m.rust_mode("ios", true).is_err(), "{}", m.json);
    }
    m.json["rust"] = serde_json::json!("native");
    assert!(m.rust_mode("ios", true).is_err());
    assert!(m.rust_mode("web", true).is_err());
    m.json["rust"] = serde_json::json!({"platforms":{"macos":{"dev":"tiered","prod":"native"},"linux":{"mode":"tiered","prod":false}}});
    assert_eq!(m.rust_mode("macos", true).unwrap(), "tiered");
    assert_eq!(m.rust_mode("macos", false).unwrap(), "native");
    assert_eq!(m.rust_mode("linux", true).unwrap(), "tiered");
    assert_eq!(m.rust_mode("linux", false).unwrap(), "off");
    m.json["rust"] = serde_json::json!("tiered");
    assert!(m.rust_mode("web", true).is_err());
    assert!(m.rust_mode("ios", false).is_err());
    m.json["rust"] = serde_json::json!(false);
    assert_eq!(m.rust_mode("ios", true).unwrap(), "off");
}

/// A minimal app: a data crate with one file, a manifest with one icon
/// and a deploy policy, no shaders.
fn app(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact-compat-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("data/src")).unwrap();
    std::fs::write(dir.join("data/src/lib.rs"), "pub struct App;\n").unwrap();
    std::fs::write(
        dir.join("data/Cargo.toml"),
        "[package]\nname = \"app-data\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.json"),
        r#"{"name":"App","icons":[{"src":"assets/a.png"}],"app":{"id":"com.example.app","name":"App"},"host":{"ios":{"minimumOS":"17.0","backgroundModes":["fetch"]}},"deploy":{"origin":"continuous","bundles":"continuous"}}"#,
    )
    .unwrap();
    dir
}

/// A Rust-only app's grants are read here and nowhere earlier: a set a
/// device would refuse whole refuses the bake, naming each line.
#[test]
fn grants_that_do_not_parse_refuse_the_bake_naming_the_line() {
    let dir = app("unparsed-grants");
    let manifest = Manifest::read(&dir).unwrap();
    let bake = |grants, rust| {
        super::compatibility_id_sources(
            &dir,
            "ios",
            "aarch64-apple-ios",
            &manifest,
            Some(grants),
            rust,
        )
    };
    // Crew's set (the port report of 2026-09-24, F1).
    let crew = "net.fetch https://crew.test\nsecret.keep crewHost\nstorage.kv crew state";
    assert_eq!(
        bake(crew, None).unwrap_err(),
        "grant-parse: the Rust source: line 2: `crewHost` is not a secret name ([a-z0-9._-]{1,64})\n\
         grant-parse: the Rust source: line 3: unexpected `state` after the target"
    );
    // A mixed app: each child's lines, by its own numbers.
    let error = bake("secret.keep jwtToken", Some(crew)).unwrap_err();
    assert!(
        error.starts_with("grant-parse: app.ts: line 1: `jwtToken` is not a secret name")
            && error.contains("\ngrant-parse: the Rust source: line 2: `crewHost`"),
        "{error}"
    );
    // An app with an `app.ts` and no Rust grants declared them there.
    std::fs::write(dir.join("app.ts"), "").unwrap();
    let error = bake(crew, None).unwrap_err();
    assert!(
        error.starts_with("grant-parse: app.ts: line 2: "),
        "{error}"
    );
    std::fs::remove_file(dir.join("app.ts")).unwrap();
    assert!(bake("net.fetch https://crew.test\nsecret.keep crew.host", None).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}

fn id(dir: &Path, platform: &str) -> String {
    let m = Manifest::read(dir).unwrap();
    compatibility_with_trust(
        dir,
        platform,
        "aarch64-apple-ios",
        &m,
        Some(""),
        "development",
    )
    .unwrap()
    .id
}

#[test]
fn the_id_is_stable_and_moves_only_with_identity() {
    let dir = app("stable");
    let first = id(&dir, "ios");
    assert_eq!(first.len(), 32, "{first}");
    assert_eq!(first, id(&dir, "ios"), "stable across computations");
    // Policy is not identity (LLP 1030 D2).
    let manifest = std::fs::read_to_string(dir.join("app.json")).unwrap();
    std::fs::write(
        dir.join("app.json"),
        manifest.replace("\"origin\":\"continuous\"", "\"origin\":\"manual\""),
    )
    .unwrap();
    assert_eq!(first, id(&dir, "ios"), "deploy.origin does not move it");
    // A platform is a cohort of its own.
    assert_ne!(first, id(&dir, "macos"), "platform moves it");
    // An icon a plan may select is identity (D3a).
    let manifest = std::fs::read_to_string(dir.join("app.json")).unwrap();
    std::fs::write(
        dir.join("app.json"),
        manifest.replace(
            "[{\"src\":\"assets/a.png\"}]",
            "[{\"src\":\"assets/a.png\"},{\"src\":\"assets/holiday.png\"}]",
        ),
    )
    .unwrap();
    let with_icon = id(&dir, "ios");
    assert_ne!(first, with_icon, "an added icon moves it");
    // Which GPU artifact owns a surface is identity (LLP 1009 D6); an
    // app with one artifact carries no key at all.
    let manifest = std::fs::read_to_string(dir.join("app.json")).unwrap();
    let m = Manifest::read(&dir).unwrap();
    let inputs = |m: &Manifest| {
        compatibility_with_trust(&dir, "ios", "aarch64-apple-ios", m, Some(""), "development")
            .unwrap()
            .inputs
    };
    assert!(inputs(&m).get("gpuModules").is_none());
    std::fs::write(
        dir.join("app.json"),
        manifest.replacen('{', r#"{"gpu":{"modules":{"world":["world"]}},"#, 1),
    )
    .unwrap();
    let modules = id(&dir, "ios");
    assert_ne!(with_icon, modules, "declaring a module moves it");
    let m = Manifest::read(&dir).unwrap();
    assert_eq!(
        inputs(&m)["gpuModules"],
        serde_json::json!({"world":["world"]})
    );
    std::fs::write(dir.join("app.json"), &manifest).unwrap();
    // A TypeScript module runs on the binary's prelude: its runtime is
    // identity, so a bundle using `exactBodyFrom` never reaches a shell
    // whose prelude would send an empty body (LLP 1108 D6 R2).
    let m = Manifest::read(&dir).unwrap();
    assert!(inputs(&m).get("typescriptRuntime").is_none());
    std::fs::write(dir.join("app.ts"), "export const appId = 'x';\n").unwrap();
    assert_eq!(inputs(&m)["typescriptRuntime"], super::TYPESCRIPT_RUNTIME);
    std::fs::remove_file(dir.join("app.ts")).unwrap();
    // A byte in the data crate is identity.
    std::fs::write(dir.join("data/src/lib.rs"), "pub struct App; // moved\n").unwrap();
    let moved = id(&dir, "ios");
    assert_ne!(with_icon, moved, "a data crate edit moves it");
    // A `data/` of TypeScript modules is no crate: its edits are not identity.
    std::fs::remove_file(dir.join("data/Cargo.toml")).unwrap();
    let typescript = id(&dir, "ios");
    std::fs::write(dir.join("data/src/lib.rs"), "pub struct App; // again\n").unwrap();
    assert_eq!(
        typescript,
        id(&dir, "ios"),
        "a data/ without Cargo.toml is not a crate"
    );
    assert_ne!(typescript, moved);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn protocol_handler_templates_move_apple_compatibility() {
    let dir = app("protocol-identity");
    let manifest = std::fs::read_to_string(dir.join("app.json")).unwrap();
    let original = id(&dir, "macos");
    let undeclared = Manifest::read(&dir).unwrap();
    let inputs = compatibility_with_trust(
        &dir,
        "macos",
        "aarch64-apple-darwin",
        &undeclared,
        Some(""),
        "development",
    )
    .unwrap()
    .inputs;
    assert!(inputs.get("protocolHandlers").is_none());
    let with = |template: &str| {
        std::fs::write(
            dir.join("app.json"),
            manifest.replacen(
                '{',
                &format!(
                    r#"{{"protocol_handlers":[{{"protocol":"exacttest","url":"{template}"}}],"#
                ),
                1,
            ),
        )
        .unwrap();
        id(&dir, "macos")
    };
    let first = with("/open?u=%s");
    assert_ne!(original, first, "registering a scheme needs a new cohort");
    assert_ne!(
        first,
        with("/callback?u=%s"),
        "a template edit needs a new cohort"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn shortcuts_move_a_native_binary_but_not_the_web() {
    let dir = app("shortcuts-identity");
    let manifest = std::fs::read_to_string(dir.join("app.json")).unwrap();
    let (macos, web) = (id(&dir, "macos"), id(&dir, "web"));
    let with = |url: &str| {
        let item = format!(r#"{{"shortcuts":[{{"name":"Add Book","url":"{url}"}}],"#);
        std::fs::write(dir.join("app.json"), manifest.replacen('{', &item, 1)).unwrap();
        (id(&dir, "macos"), id(&dir, "web"))
    };
    let first = with("/add");
    assert_ne!(macos, first.0, "a shortcut is compiled into the binary");
    assert_ne!(first.0, with("/new").0, "so is its url");
    assert_eq!(web, first.1, "the web's manifest.json is the web root's");
    std::fs::remove_dir_all(dir).unwrap();
}

/// A faced tool's signature is compiled into the Apple and Android doors
/// (LLP 1133.023 D8): a change to it moves those binaries' id, never the
/// web's or tvOS's, and an unfaced tool moves none.
#[test]
fn a_faced_tools_signature_moves_the_binaries_with_its_door() {
    let dir = app("faces-identity");
    let manifest = std::fs::read_to_string(dir.join("app.json")).unwrap();
    let ids = |description: &str, faced: bool| {
        let source = format!("component A\n  state n = 0\n  tool add writes \"{description}\" = add\n  action add(by: number)\n    n = n + by\n  view\n    text `${{n}}`\n");
        std::fs::write(dir.join("app.contract"), source).unwrap();
        let face = if faced {
            r#"{"intents":{"tools":{"add":{"title":"Add"}}},"#
        } else {
            "{"
        };
        std::fs::write(dir.join("app.json"), manifest.replacen('{', face, 1)).unwrap();
        let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
        let targets = [
            ("ios", "aarch64-apple-ios"),
            ("android", "aarch64-linux-android"),
            ("web", "wasm32-unknown-unknown"),
            ("ios", "aarch64-apple-tvos"),
        ];
        targets.map(|(platform, target)| {
            let m = Manifest::read(&dir).unwrap();
            let mut c =
                compatibility_with_trust(&dir, platform, target, &m, Some(""), "development")
                    .unwrap();
            super::faces(&mut c, &plan, platform, target);
            c.id
        })
    };
    let bare = ids("Add.", false);
    assert_eq!(
        bare,
        ids("Add more.", false),
        "an unfaced tool is plan data"
    );
    let faced = ids("Add.", true);
    let moved = ids("Add more.", true);
    assert_ne!(
        bare[0], faced[0],
        "a face is compiled into the Apple binary"
    );
    assert_ne!(faced[0], moved[0], "so is the tool's description");
    assert_ne!(faced[1], moved[1], "and into Android's");
    assert_eq!(faced[2], moved[2], "the web registers it at run time");
    assert_eq!(faced[3], moved[3], "tvOS compiles no faces");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_inputs_name_every_field_and_an_undeclared_app_gets_defaults() {
    let dir = app("fields");
    let m = Manifest::read(&dir).unwrap();
    let c = compatibility_with_trust(
        &dir,
        "ios",
        "aarch64-apple-ios",
        &m,
        Some("net.fetch https://x/"),
        "development",
    )
    .unwrap();
    let i = &c.inputs;
    for key in [
        "kernelSchema",
        "formatVersion",
        "formatDigest",
        "abi",
        "executors",
        "rustMode",
        "rustAbi",
        "rustTarget",
        "rustModule",
        "typescriptPlacement",
        "rustPlacement",
        "dataCrate",
        "gpuSurfaces",
        "nativeModules",
        "hatches",
        "icons",
        "capabilities",
        "keys",
        "trust",
        "grantCeiling",
        "platform",
        "arch",
        "minimumOS",
        "store",
        "app",
    ] {
        assert!(i.get(key).is_some(), "missing {key}: {i}");
    }
    assert_eq!(i["abi"]["c"], super::abi_version().unwrap());
    // No `rust.module`: nothing to replace, so no executor (LLP 1047.001 D7).
    assert_eq!(i["rustMode"], "off");
    assert!(i["rustAbi"].is_null());
    assert!(i["rustTarget"].is_null());
    assert!(i["rustModule"].is_null());
    assert_eq!(i["executors"], serde_json::json!(["native"]));
    assert_eq!(i["arch"], "aarch64");
    assert_eq!(i["minimumOS"], "17.0");
    assert_eq!(
        i["capabilities"]["backgroundModes"],
        serde_json::json!(["fetch"])
    );
    assert_eq!(i["capabilities"]["urlSchemes"], serde_json::Value::Null);
    assert_eq!(i["store"]["L"], "A");
    assert_eq!(
        i["store"]["acceptedKinds"],
        serde_json::json!(["plan", "assets"])
    );
    assert_eq!(i["grantCeiling"], "net.fetch https://x/");
    assert!(c.to_json().starts_with("{\"id\":\""));
    // Policy rides beside the id (LLP 1030.000 D4): the channel, its
    // origin, and when a staged bundle applies — `next-launch` unsaid.
    assert!(
        c.to_json().contains(
            "\"delivery\":{\"activate\":\"next-launch\",\"channel\":\"prod\",\"origin\":null}"
        ),
        "{}",
        c.to_json()
    );
    // No manifest: the derived defaults, as scripts/app.mjs derives them.
    std::fs::remove_file(dir.join("app.json")).unwrap();
    let m = Manifest::read(&dir).unwrap();
    assert!(!m.declared);
    assert!(
        m.id.starts_with("com.exact.exact-compat-fields-"),
        "{}",
        m.id
    );
    assert!(m.name.starts_with("Exact-compat-fields-"), "{}", m.name);
    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn an_executor_is_linked_only_for_a_named_module() {
    // LLP 1047.001 D7: iOS defaults to the interpreter (LLP 1029.000),
    // but only an app that names a module has anything for it to run.
    let dir = app("rust-module-gate");
    let mut manifest = Manifest::read(&dir).unwrap();
    for (policy, mode) in [
        (serde_json::json!(true), "off"),
        (serde_json::json!("wasm"), "off"),
        (
            serde_json::json!({"module":{"package":"app-logic"}}),
            "wasm",
        ),
    ] {
        manifest.json["rust"] = policy.clone();
        let receipt = compatibility_with_trust(
            &dir,
            "ios",
            "aarch64-apple-ios",
            &manifest,
            Some(""),
            "development",
        )
        .unwrap();
        assert_eq!(receipt.inputs["rustMode"], mode, "{policy}");
    }
}

#[test]
fn rust_capability_receipts_bind_executor_abi_target_and_module() {
    let dir = app("rust-capabilities");
    let mut manifest = Manifest::read(&dir).unwrap();
    let mut ids = Vec::new();
    for (mode, target, executor) in [
        ("native", Some("aarch64-apple-darwin"), Some("native-dylib")),
        ("wasm", Some("wasm32-unknown-unknown"), Some("wasmi")),
        ("off", None, None),
    ] {
        manifest.json["rust"] = serde_json::json!({"mode":mode,"module":{"package":"app-logic"}});
        let receipt = compatibility_with_trust(
            &dir,
            "macos",
            "aarch64-apple-darwin",
            &manifest,
            Some(""),
            "development",
        )
        .unwrap();
        assert_eq!(receipt.inputs["rustModule"], "app-logic");
        assert_eq!(receipt.inputs["rustTarget"], serde_json::json!(target));
        assert_eq!(
            receipt.inputs["rustAbi"],
            if mode == "off" {
                serde_json::Value::Null
            } else {
                serde_json::json!(super::RUST_ABI)
            }
        );
        let expected = executor.map_or_else(
            || serde_json::json!(["native"]),
            |e| serde_json::json!(["native", e]),
        );
        assert_eq!(receipt.inputs["executors"], expected);
        assert!(
            !ids.contains(&receipt.id),
            "executor capability changes cohort identity"
        );
        ids.push(receipt.id);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_replaceable_rust_module_keeps_its_source_out_of_the_cohort() {
    let dir = app("rust-source");
    std::fs::create_dir_all(dir.join("data/src")).unwrap();
    let mut manifest = Manifest::read(&dir).unwrap();
    let id = |manifest: &Manifest, source: &str| {
        std::fs::write(dir.join("data/src/lib.rs"), source).unwrap();
        let receipt = compatibility_with_trust(
            &dir,
            "macos",
            "aarch64-apple-darwin",
            manifest,
            Some(""),
            "production",
        )
        .unwrap();
        (receipt.id, receipt.inputs["dataCrate"].clone())
    };
    manifest.json["deploy"] = serde_json::json!({"signing":{"key":"k","keys":{"k":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}}});
    manifest.json["rust"] = serde_json::json!({"mode":"native","module":{"package":"app-logic"}});
    let (before, data) = id(&manifest, "pub const A: u8 = 1;");
    assert!(data.is_null(), "{data}");
    assert_eq!(id(&manifest, "pub const A: u8 = 2;").0, before);
    manifest.json["rust"] = serde_json::json!({"mode":"off","module":{"package":"app-logic"}});
    let (off, data) = id(&manifest, "pub const A: u8 = 1;");
    assert!(!data.is_null());
    assert_ne!(id(&manifest, "pub const A: u8 = 2;").0, off);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn web_receipts_name_the_linked_composition_and_refuse_an_unlinked_store() {
    let dir = app("web-zero");
    let mut manifest = Manifest::read(&dir).unwrap();
    manifest.json["deploy"] = serde_json::json!({});
    manifest.json["app"]["origin"] = serde_json::json!("https://updates.example");
    std::fs::write(dir.join("app.ts"), "").unwrap();
    for setting in [None, Some(serde_json::json!("0"))] {
        if let Some(setting) = setting {
            manifest.json["deploy"]["store"] = serde_json::json!({"web":setting});
        }
        let plain =
            compatibility_with_trust(&dir, "web", "wasm32", &manifest, None, "production").unwrap();
        assert_eq!(plain.inputs["store"]["L"], "0");
        // No `rust.module`, so no Rust executor (LLP 1047.001 D7).
        assert_eq!(
            plain.inputs["executors"],
            serde_json::json!(["browser", "native"])
        );
        assert_eq!(
            plain.inputs["store"]["acceptedKinds"],
            serde_json::json!([])
        );
        assert!(plain.inputs["keys"].is_null());
        assert!(plain.inputs["trust"].is_null());
        assert!(plain.inputs["abi"]["storeCodec"].is_null());
        assert_eq!(plain.origin, None);
        assert_eq!(plain.channel, "");
    }
    for setting in [
        serde_json::json!("A"),
        serde_json::json!(null),
        serde_json::json!(0),
    ] {
        manifest.json["deploy"]["store"]["web"] = setting;
        let error = compatibility_with_trust(&dir, "web", "wasm32", &manifest, None, "production")
            .unwrap_err();
        assert!(error.contains("web host links no updater"), "{error}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn production_trust_requires_keys_and_has_a_distinct_cohort() {
    let dir = app("trust");
    let mut manifest = Manifest::read(&dir).unwrap();
    for keys in [
        None,
        Some(serde_json::Value::Null),
        Some(serde_json::json!({})),
        Some(serde_json::json!([])),
        Some(serde_json::json!({"k":"invalid"})),
    ] {
        manifest.json["deploy"]["signing"] = serde_json::json!({});
        if let Some(keys) = keys {
            manifest.json["deploy"]["signing"]["keys"] = keys;
        }
        assert!(
            compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "production")
                .is_err()
        );
    }
    manifest.json["deploy"]["signing"] = serde_json::json!({});
    let dev =
        compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "development").unwrap();
    assert_eq!(dev.inputs["trust"], "development");
    assert!(compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "dev").is_err());
    manifest.json["deploy"]["store"] = serde_json::json!({"linux":"0"});
    assert!(
        compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "production").is_ok()
    );
    manifest.json["app"]["origin"] = serde_json::json!("https://updates.example");
    let zero =
        compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "production").unwrap();
    assert!(zero.inputs["keys"].is_null());
    assert!(zero.inputs["trust"].is_null());
    assert_eq!(
        zero.embedded["trust"], "production",
        "the build's trust, beside the id"
    );
    assert!(zero.inputs["abi"]["storeCodec"].is_null());
    assert_eq!(zero.inputs["store"]["acceptedKinds"], serde_json::json!([]));
    assert!(zero.origin.is_none());
    manifest.json["deploy"]["signing"]["keys"] =
        serde_json::json!({"ignored":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="});
    let changed =
        compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "development").unwrap();
    assert_eq!(
        zero.id, changed.id,
        "a missing updater has no key or trust epoch"
    );
    assert_eq!(changed.embedded["trust"], "development");
    manifest.json["deploy"]["store"] = serde_json::json!({"linux":"A"});
    manifest.json["deploy"]["signing"]["keys"] =
        serde_json::json!({"k":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="});
    let production =
        compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "production").unwrap();
    let development =
        compatibility_with_trust(&dir, "linux", "aarch64", &manifest, None, "development").unwrap();
    assert_eq!(production.inputs["trust"], "production");
    assert_ne!(
        production.id, development.id,
        "unsigned development permission is its own trust epoch"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// LLP 1016.000 D10: a module needs the newest runtime whose option it names,
/// so one that uses nothing newer goes to a binary whose prelude is older.
#[test]
fn a_bundle_needs_the_runtime_of_the_newest_option_it_names() {
    use super::typescript_runtime_of as needs;
    assert_eq!(needs(b"export const answer = () => 'idle';"), 0);
    assert_eq!(needs(b"fetch(u, {exactBodyFrom: p})"), 1);
    assert_eq!(needs(b"fetch(u, {exactBodyTo: p, exactBodyFrom: q})"), 4);
    assert_eq!(needs(b"fetch(u, {signal: AbortSignal.timeout(5e3)})"), 5);
    assert_eq!(
        needs(b"fetch(u, {method:\"POST\",exactUploadProgress:!0,exactStream:m})"),
        5
    );
    // LLP 1133.007 D6a's operations.
    assert_eq!(
        needs(b"const l = await storage.fs.flock(p, {timeout: 0})"),
        6
    );
    assert_eq!(needs(b"(await fs.lstat(p)).isSymbolicLink"), 6);
    assert_eq!(needs(b"return new Response(body, {status: 201})"), 7);
    assert_eq!(needs(b"return new globalThis.Response(body)"), 7);
    assert_eq!(needs(b"return Response.redirect(u, 303)"), 7);
    assert_eq!(needs(b"const copy = (await fetch(u)).clone()"), 7);
    assert_eq!(needs(b"const copy = r.clone.call(r)"), 7);
    assert_eq!(needs(b"const {clone: copy} = r; return copy.call(r)"), 7);
    assert_eq!(needs(b"return (await fetch(u)).statusText"), 7);
    assert_eq!(needs(b"if (r.bodyUsed) return"), 7);
    assert_eq!(needs(b"const c = structuredClone(v)"), 0);
    assert_eq!(
        needs(b"return Response.json({a: 1})"),
        super::TYPESCRIPT_RUNTIME
    );
}
