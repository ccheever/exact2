use super::{sources, Scratch};

#[test]
fn mixed_bake_reads_rust_identity_and_own_grants_without_executing_it() {
    use contract::DataSource;
    struct Rust;
    impl DataSource for Rust {
        fn app_id(&self) -> &str {
            "test.mixed"
        }
        fn grants(&self) -> &str {
            "fs.read app:/data/rust"
        }
        fn query(
            &mut self,
            _: &str,
            _: &[exact_plan::Value],
        ) -> Result<exact_plan::Value, exact_runner::DataError> {
            panic!("metadata collection must not query the Rust source")
        }
        fn activate(&mut self) -> Result<(), exact_runner::DataError> {
            panic!("metadata collection must not activate the Rust source")
        }
    }
    let javascript =
        serde_json::json!({"appId":"test.mixed","grants":"net.fetch https://example.test/"});
    assert_eq!(
        super::mixed_grants(&javascript, Some(&Rust)).unwrap(),
        Some("fs.read app:/data/rust")
    );
    assert!(super::mixed_grants(&serde_json::json!({"appId":"wrong"}), Some(&Rust)).is_err());
}

#[test]
fn captures_fonts_and_complete_static_trees_without_following_links() {
    let app = Scratch::new(&std::env::temp_dir()).unwrap();
    let inputs = [
        ("app.ts", &b"export const appId = 'test.assets';"[..]),
        ("fonts/face.ttf", &b"font bytes"[..]),
        ("fonts/face.otf", &b"other font bytes"[..]),
        ("assets/icons/menu.svg", &b"<svg/>"[..]),
        ("assets/icons/raster/menu.png", &b"\x89PNG\x00\xff"[..]),
        ("assets/data.bin", &b"\x00\x01\xfe\xff"[..]),
        ("deck/index.html", &b"<!doctype html>"[..]),
        ("gpu/shaders/paint.wgsl", &b"@fragment fn paint() {}"[..]),
    ];
    for (name, bytes) in inputs {
        let path = app.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    std::fs::write(app.0.join("private.bin"), b"not an app asset").unwrap();
    // The driver's test file is no build's input (trivia F8).
    std::fs::write(app.0.join("app.test.contract"), b"test \"t\"\n").unwrap();
    // Nor is the app's `.exact/`: an agent's evidence and runtime files (Depot).
    std::fs::create_dir_all(app.0.join(".exact/evidence")).unwrap();
    std::fs::write(app.0.join(".exact/evidence/results.json"), b"{}").unwrap();
    // Nor a web build in `dist` and what it keeps beside it (`web-build --out dist`), each a tree of
    // what the capture would otherwise take; a source whose name only starts so is still one.
    for name in [
        "dist",
        "dist.gen/0123",
        "dist.leases/0123",
        "dist.previous",
        "dist.stage-a1b2c3",
    ] {
        std::fs::create_dir_all(app.0.join(name)).unwrap();
        std::fs::write(app.0.join(name).join("app.js"), b"built").unwrap();
    }
    for name in ["dist.gen.reaped", "dist.lock"] {
        std::fs::write(app.0.join(name), b"0").unwrap();
    }
    assert!(super::beside::beside_dist("dist.gen") && !super::beside::beside_dist("dist.stage-"));
    assert!(!super::beside::beside_dist("dist.config.ts"));
    let captured = sources(&app.0).unwrap();
    assert_eq!(captured.len(), inputs.len());
    for (name, bytes) in inputs {
        assert_eq!(captured[std::path::Path::new(name)], bytes);
    }
    std::fs::write(app.0.join("assets/icons/menu.svg"), b"<svg>changed</svg>").unwrap();
    assert_ne!(sources(&app.0).unwrap(), captured);
    std::fs::remove_file(app.0.join("assets/data.bin")).unwrap();
    assert!(!sources(&app.0)
        .unwrap()
        .contains_key(std::path::Path::new("assets/data.bin")));
    #[cfg(unix)]
    {
        // A link the capture never reads is no input (CLAUDE.md beside an
        // app's AGENTS.md); one it would read, or a directory, is refused.
        std::fs::write(app.0.join("AGENTS.md"), b"notes").unwrap();
        std::os::unix::fs::symlink("AGENTS.md", app.0.join("CLAUDE.md")).unwrap();
        let unchanged = sources(&app.0).unwrap();
        assert!(!unchanged.contains_key(std::path::Path::new("CLAUDE.md")));
        // Anything else is refused: a directory, a script no capture lists
        // (the web bundler would follow it), a document inside an asset tree.
        for (target, link) in [
            ("fonts", "more"),
            ("private.bin", "helper.js"),
            ("AGENTS.md", "assets/notes.md"),
        ] {
            std::os::unix::fs::symlink(app.0.join(target), app.0.join(link)).unwrap();
            assert!(
                sources(&app.0).unwrap_err().contains("source links"),
                "{link}"
            );
            std::fs::remove_file(app.0.join(link)).unwrap();
        }
        std::os::unix::fs::symlink(app.0.join("private.bin"), app.0.join("assets/link.bin"))
            .unwrap();
        assert!(sources(&app.0).unwrap_err().contains("source links"));
    }
}

#[test]
fn a_mounted_directory_is_captured_beside_app_ts_and_nothing_else_of_it() {
    let parent = Scratch::new(&std::env::temp_dir()).unwrap();
    let app = parent.0.join("app");
    let core = parent.0.join("shared/core");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::create_dir_all(core.join("deep")).unwrap();
    std::fs::write(app.join("app.ts"), "import { x } from './core/model';").unwrap();
    std::fs::write(
        app.join("app.json"),
        r#"{"typescript":{"sources":{"core":"../shared/core"}}}"#,
    )
    .unwrap();
    std::fs::write(core.join("model.ts"), "export const x = 1;").unwrap();
    std::fs::write(core.join("deep/rules.json"), "{}").unwrap();
    std::fs::write(core.join("notes.md"), "not TypeScript").unwrap();
    let captured = sources(&app).unwrap();
    let names: Vec<_> = captured.keys().map(|p| p.as_path()).collect();
    assert_eq!(
        names,
        [
            "app.json",
            "app.ts",
            "core/deep/rules.json",
            "core/model.ts"
        ]
        .map(std::path::Path::new)
    );
    // Where each came from, for Cargo's rerun lines.
    let root = app.canonicalize().unwrap();
    let mounted = super::mounts(&root).unwrap();
    assert_eq!(
        super::origin(&root, &mounted, std::path::Path::new("core/model.ts")),
        core.canonicalize().unwrap().join("model.ts")
    );
    // An editor's link of that name is skipped; a real directory is refused.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&core, app.join("core")).unwrap();
        assert_eq!(sources(&app).unwrap(), captured);
        std::fs::remove_file(app.join("core")).unwrap();
    }
    std::fs::create_dir(app.join("core")).unwrap();
    assert!(sources(&app)
        .unwrap_err()
        .contains("mounted from typescript.sources"));
    std::fs::remove_dir(app.join("core")).unwrap();
    // A mount must be outside the app, and not a reserved name.
    for bad in [
        r#"{"typescript":{"sources":{"core":"."}}}"#,
        r#"{"typescript":{"sources":{"assets":"../shared/core"}}}"#,
        r#"{"typescript":{"sources":{"core":"/etc"}}}"#,
        r#"{"typescript":{"sources":{"core":"../missing"}}}"#,
        r#"{"typescript":{"sources":{"core":"../shared/core","shared":"../shared"}}}"#,
    ] {
        std::fs::write(app.join("app.json"), bad).unwrap();
        assert!(
            sources(&app).unwrap_err().contains("typescript.sources"),
            "{bad}"
        );
    }
    // A missing one says where it was resolved from.
    std::fs::write(
        app.join("app.json"),
        r#"{"typescript":{"sources":{"core":"../missing"}}}"#,
    )
    .unwrap();
    assert!(sources(&app)
        .unwrap_err()
        .contains("../missing, resolved from the app's directory"));
}

#[test]
fn tsconfig_paths_reach_the_type_checker_and_bundler_without_hermes() {
    let app = Scratch::new(&std::env::temp_dir()).unwrap();
    std::fs::create_dir_all(app.0.join("lib")).unwrap();
    std::fs::write(app.0.join("lib/word.ts"), "export const word = 'mapped';").unwrap();
    std::fs::write(
        app.0.join("__exact_entry.ts"),
        "export { word } from '@/word';",
    )
    .unwrap();
    std::fs::write(
        app.0.join("tsconfig.json"),
        r#"{
        // The first candidate is absent; TypeScript tries the next one.
        "compilerOptions": {"baseUrl":"lib", "paths":{"@/*":["absent/*", "*"]}},
    }"#,
    )
    .unwrap();
    super::compile_once(&app.0, &super::Tools::default()).unwrap();
    assert!(std::fs::read_to_string(app.0.join("app.js"))
        .unwrap()
        .contains("mapped"));
}

#[test]
fn captured_module_guard_uses_real_files_and_path_components() {
    let parent = Scratch::new(&std::env::temp_dir()).unwrap();
    let stage = parent.0.join("captured café space");
    let sibling = parent.0.join("captured café space-other");
    std::fs::create_dir_all(stage.join("nested")).unwrap();
    std::fs::create_dir_all(&sibling).unwrap();
    std::fs::write(stage.join("nested/file.ts"), "export const n = 1;").unwrap();
    std::fs::write(sibling.join("outside.ts"), "export const n = 2;").unwrap();
    std::fs::write(stage.join("__exact_config.mjs"), super::resident::CONFIG).unwrap();
    std::fs::write(stage.join("__exact_binding.mjs"), super::resident::BINDING).unwrap();
    std::fs::write(stage.join("__exact_env.mjs"), super::resident::ENV).unwrap();
    let checked = exact_bake::bun()
        .args([
            "--input-type=module",
            "-e",
            r#"
import assert from 'node:assert/strict';
import { realpathSync, symlinkSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const stage=process.argv[1], sibling=stage+'-other';
const { assertCapturedModule: check } = await import(pathToFileURL(resolve(stage,'__exact_config.mjs')).href);
const file=resolve(stage,'nested/file.ts');
check(stage,file);
check(realpathSync(stage),file);
check(stage,realpathSync(file));
if(process.platform==='win32') {
  check(stage,file.replaceAll('\\','/'));
  check(stage,file.toUpperCase());
  // Synthetic canonical names model a case-sensitive parent's two siblings.
  // This exercises the exact reconstruction guard, not NTFS case mode.
  const native=realpathSync.native;
  try {
    realpathSync.native=path=>path===file ? native(file).replace('captured café space','CAPTURED café space') : native(path);
    assert.throws(()=>check(stage,file),/module outside captured app/);
  } finally { realpathSync.native=native; }
}
for(const bad of [
  'nested/file.ts', '\0virtual', resolve(stage,'missing.ts'), stage,
  resolve(stage,'nested'), resolve(sibling,'outside.ts'),
  resolve(stage,'..',sibling.split(/[\\/]/).at(-1),'outside.ts'),
]) assert.throws(()=>check(stage,bad),String(bad));
const link=resolve(stage,'escape');
let linked=true;
try { symlinkSync(sibling,link,process.platform==='win32' ? 'junction' : 'dir'); }
catch(error) {
  if(!['EPERM','EACCES'].includes(error.code))throw error;
  linked=false;
  console.error('outside-link fixture unavailable: '+error.code);
}
if(linked) {
  assert.throws(()=>check(stage,resolve(link,'outside.ts')),/module outside captured app/);
  console.log('captured module guard: real nested file accepted; outside link refused');
}
"#,
        ])
        .arg(&stage)
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    println!(
        "{}{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
}

#[test]
fn native_resource_trees_are_not_captured_as_typescript_or_assets() {
    let app = Scratch::new(&std::env::temp_dir()).unwrap();
    std::fs::create_dir_all(app.0.join("server/@scope")).unwrap();
    std::fs::write(
        app.0.join("app.json"),
        r#"{"host":{"macos":{"resources":[{"from":"server","to":"Resources/server"}]}}}"#,
    )
    .unwrap();
    std::fs::write(app.0.join("server/@scope/large.json"), vec![b' '; 65 << 20]).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("large.json", app.0.join("server/@scope/link")).unwrap();
    let captured = sources(&app.0).unwrap();
    assert_eq!(captured.len(), 1);
    assert!(captured.contains_key(std::path::Path::new("app.json")));
}

/// A Cargo bake reruns on each package's installed paths and on each
/// captured file where it was read: a package reached through a file link
/// has no installed path, and its files are still watched.
#[test]
fn cargo_watches_captured_package_files_and_installed_paths() {
    let stage = Scratch::new(&std::env::temp_dir()).unwrap();
    std::fs::write(
        stage.0.join("__exact_packages.json"),
        r#"{"packages":[["a@1.0.0","/real/a",["/app/node_modules/a"]],["b@1.0.0","/real/b",[]]],"files":[["b@1.0.0","extra.js","/real/b/extra.js","x"]],"edges":[],"digests":{"a@1.0.0":"d"}}"#,
    )
    .unwrap();
    let code = super::package_code(&stage.0).unwrap();
    for path in ["/app/node_modules/a", "/real/b/extra.js"] {
        assert!(
            code.watched.contains(&std::path::PathBuf::from(path)),
            "{path}"
        );
    }
    assert_eq!(code.packages["a@1.0.0"], "d");
}

/// A bake made outside Cargo gives the archive only what Rust compiles: its
/// entry reads the plan the link carries, and a plan an earlier bake in
/// Cargo left is gone; a file whose bytes did not change keeps its time.
#[test]
fn a_linked_bake_gives_the_archive_what_rust_compiles() {
    let baked = Scratch::new(&std::env::temp_dir()).unwrap();
    let out = Scratch::new(&std::env::temp_dir()).unwrap();
    for name in super::COMPILED {
        std::fs::write(baked.0.join(name), name).unwrap();
    }
    std::fs::write(baked.0.join("app.plan"), "plan").unwrap();
    std::fs::write(out.0.join("app.plan"), "stale").unwrap();
    std::fs::write(out.0.join("module.rs"), "module.rs").unwrap();
    let kept = std::fs::metadata(out.0.join("module.rs"))
        .unwrap()
        .modified()
        .unwrap();
    super::take_baked(&baked.0, &out.0).unwrap();
    for name in super::COMPILED {
        assert_eq!(std::fs::read_to_string(out.0.join(name)).unwrap(), name);
    }
    let modified = std::fs::metadata(out.0.join("module.rs"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(modified, kept);
    assert_eq!(
        std::fs::read_to_string(out.0.join("baked.rs")).unwrap(),
        super::LINKED
    );
    assert!(!out.0.join("app.plan").exists());
}

/// A linked bake's `module.rs` names neither the bytecode's bytes nor their
/// hash, so an edit to the TypeScript alone compiles no Rust; the embedded
/// one includes the bytecode and says its hash.
#[test]
fn a_linked_module_leaves_its_bytecode_to_the_link() {
    let meta = serde_json::json!({"appId": "a", "grants": "", "module": {"sha256": "beef"}});
    let linked = super::module_constants(&meta, false, "ios", true, None).unwrap();
    assert!(
        !linked.contains("beef") && !linked.contains("app.hbc"),
        "{linked}"
    );
    assert!(
        linked.contains("::exact_apple::baked::bytecode()"),
        "{linked}"
    );
    let embedded = super::module_constants(&meta, false, "ios", false, None).unwrap();
    assert!(
        embedded.contains("REVISION: &str = \"beef\"") && embedded.contains("/app.hbc"),
        "{embedded}"
    );
}

/// LLP 1154 D3: the one direct dependency whose `exact.json`, at its root,
/// declares `native` fills the slot, whatever its `exports` "." names (here
/// its TypeScript driver). On a native platform `module.rs` joins its shim
/// by an absolute `#[path]` and writes the factory; the web gets neither; a
/// second such dependency refuses the bake, naming both. Every package file
/// the choice read is watched, and one it cannot read refuses the bake.
#[test]
fn a_package_declaring_native_writes_the_factory_and_a_second_refuses() {
    let app = Scratch::new(&std::env::temp_dir()).unwrap();
    let package = |name: &str, declaration: &str| {
        let dir = app.0.join("node_modules").join(name);
        std::fs::create_dir_all(dir.join("native")).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"exports": {".": "./driver.js"}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("exact.json"), declaration).unwrap();
        std::fs::write(dir.join("native/lib.rs"), "").unwrap();
        dir
    };
    let vault = package(
        "@fixture/vault",
        r#"{"native": {"rust": "native/lib.rs", "class": "Vault", "library": {}}, "web": {"assets": []}}"#,
    );
    package("@fixture/plain", r#"{"kind": "interface", "names": []}"#);
    let manifest = |deps: &str| {
        std::fs::write(
            app.0.join("package.json"),
            format!(r#"{{"dependencies": {{{deps}}}}}"#),
        )
        .unwrap()
    };
    manifest(r#""@fixture/vault": "1.0.0", "@fixture/plain": "1.0.0""#);
    let selection = contract::native::native_slot(&app.0).unwrap();
    // Every declaration read is watched, the one without `native` too: adding
    // it there asks for the choice again, and so for `bake-native-twice`.
    let plain = app.0.join("node_modules/@fixture/plain");
    for file in [
        app.0.join("package.json"),
        plain.join("package.json"),
        plain.join("exact.json"),
        vault.join("exact.json"),
    ] {
        let file = std::fs::canonicalize(file).unwrap();
        assert!(
            selection.watched.contains(&file),
            "{} is not watched: {:?}",
            file.display(),
            selection.watched
        );
    }
    let slot = selection.slot.unwrap();
    assert_eq!(
        (slot.package.as_str(), slot.class.as_str()),
        ("@fixture/vault", "Vault")
    );
    let root = std::fs::canonicalize(vault.join("native/lib.rs")).unwrap();
    assert_eq!(slot.root, root);
    // The link reads the rest of `native` from the same declaration.
    assert!(slot.native["library"].is_object(), "{:?}", slot.native);
    let meta = serde_json::json!({"appId": "a", "grants": "", "module": {"sha256": "beef"}});
    let linux = super::module_constants(&meta, false, "linux", false, Some(&slot)).unwrap();
    assert!(
        linux.contains(&format!("#[path = {:?}]\n", root.display().to_string()))
            && linux.contains("mod exact_native_shim;")
            && linux.contains("pub fn exact_native(_grants: &str) -> Box<dyn exact_js::NativeModule> {\n    Box::new(exact_native_shim::Vault::new(APP))\n}"),
        "{linux}"
    );
    let web = super::module_constants(&meta, false, "web", false, Some(&slot)).unwrap();
    assert!(!web.contains("exact_native"), "{web}");

    package(
        "second",
        r#"{"native": {"rust": "native/lib.rs", "class": "Second"}}"#,
    );
    manifest(r#""@fixture/vault": "1.0.0", "second": "1.0.0""#);
    let refused = contract::native::native_slot(&app.0).unwrap_err();
    assert!(
        refused.starts_with("bake-native-twice: @fixture/vault and second each declare"),
        "{refused}"
    );

    // What the choice cannot read it refuses, as it cannot watch a missing file.
    manifest(r#""@fixture/vault": "1.0.0", "absent": "1.0.0""#);
    let refused = contract::native::native_slot(&app.0).unwrap_err();
    assert!(
        refused.starts_with("bake-native-missing: the app depends on absent, which is not installed: run bun install"),
        "{refused}"
    );
    // A dependency with no exact.json declares nothing, and its directory is
    // watched, where the file would appear.
    let bare = package("bare", "{}");
    std::fs::remove_file(bare.join("exact.json")).unwrap();
    manifest(r#""@fixture/vault": "1.0.0", "bare": "1.0.0""#);
    let selection = contract::native::native_slot(&app.0).unwrap();
    assert_eq!(selection.slot.unwrap().package, "@fixture/vault");
    let bare = std::fs::canonicalize(&bare).unwrap();
    assert!(selection.watched.contains(&bare), "{:?}", selection.watched);
    // One whose package.json exports an exact.json that is not there is refused.
    let exporting = package("exporting", "{}");
    std::fs::write(
        exporting.join("package.json"),
        r#"{"exports": {".": "./exact.json"}}"#,
    )
    .unwrap();
    std::fs::remove_file(exporting.join("exact.json")).unwrap();
    manifest(r#""@fixture/vault": "1.0.0", "exporting": "1.0.0""#);
    let refused = contract::native::native_slot(&app.0).unwrap_err();
    assert!(
        refused.starts_with("bake-native-declaration: exporting's package.json exports ")
            && refused.ends_with("exact.json, which does not exist"),
        "{refused}"
    );
    package("unread", r#"{"native": "#);
    manifest(r#""@fixture/vault": "1.0.0", "unread": "1.0.0""#);
    let refused = contract::native::native_slot(&app.0).unwrap_err();
    assert!(
        refused.starts_with("bake-native-declaration: unread's exact.json (")
            && refused.contains("is not JSON: "),
        "{refused}"
    );
}
