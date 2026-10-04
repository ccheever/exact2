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
        std::os::unix::fs::symlink(app.0.join("fonts"), app.0.join("more")).unwrap();
        assert!(sources(&app.0).unwrap_err().contains("source links"));
        std::fs::remove_file(app.0.join("more")).unwrap();
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
    let names: Vec<_> = captured
        .keys()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "app.json",
            "app.ts",
            "core/deep/rules.json",
            "core/model.ts"
        ]
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
    ] {
        std::fs::write(app.join("app.json"), bad).unwrap();
        assert!(
            sources(&app).unwrap_err().contains("typescript.sources"),
            "{bad}"
        );
    }
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
