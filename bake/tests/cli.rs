//! `exact-bake compat`, the compatibility id on the command line (LLP 1030 D3a).
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("exact-bake-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for App {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn help_succeeds_without_reading_or_changing_files() {
    let app = App::new("help");
    for command in [None, Some("compat")] {
        for flag in ["--help", "-h"] {
            let mut process = Command::new(env!("CARGO_BIN_EXE_exact-bake"));
            process
                .args(command)
                .arg(flag)
                .current_dir(Path::new(&app.0));
            let output = process.output().unwrap();
            assert!(output.status.success(), "{command:?} {flag}: {output:?}");
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.starts_with("usage:"), "{text}");
            assert!(text.contains("exact-bake compat"), "{text}");
        }
    }
    assert_eq!(std::fs::read_dir(&app.0).unwrap().count(), 0);
}

#[test]
fn compat_cli_validates_all_options_before_reading_the_manifest() {
    let app = App::new("compat-args");
    let manifest = r#"{"app":{"id":"com.exact.cli-args","name":"CLI args"},"deploy":{"store":{"ios":"0","macos":"0","linux":"0","web":"0"}}}"#;
    app.write("app.json", manifest);
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_exact-bake"))
            .arg("compat")
            .args(args)
            .current_dir(&app.0)
            .env("EXACT_UPDATE_TRUST", "development")
            .env_remove("OUT_DIR")
            .output()
            .unwrap()
    };
    for (platform, target) in [
        ("web", "wasm32-unknown-unknown"),
        ("macos", "aarch64-apple-darwin"),
        ("ios", "aarch64-apple-ios"),
        ("linux", "x86_64-unknown-linux-gnu"),
    ] {
        let result = run(&[".", "--json", "--target", target, "--platform", platform]);
        assert!(result.status.success(), "{result:?}");
        assert!(result.stderr.is_empty());
        let json: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(json["inputs"]["platform"], platform);
        assert_eq!(json["target"], target);
        let plain = run(&[".", "--platform", platform, "--target", target]);
        assert!(plain.status.success(), "{plain:?}");
        assert_eq!(
            String::from_utf8(plain.stdout).unwrap().trim(),
            json["id"].as_str().unwrap()
        );
    }
    let default = run(&[".", "--platform", "web", "--json"]);
    assert!(default.status.success(), "{default:?}");
    let json: Value = serde_json::from_slice(&default.stdout).unwrap();
    assert_eq!(
        json["target"],
        format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
    );
    assert_eq!(
        std::fs::read_to_string(app.0.join("app.json")).unwrap(),
        manifest
    );

    app.write("app.json", "invalid JSON");
    for args in [
        vec![],
        vec![""],
        vec!["."],
        vec![".", "--platform"],
        vec![".", "--platform", "unknown"],
        vec![".", "--platform", "web", "--unknown"],
        vec![".", "--platform", "web", "extra"],
        vec![".", "--platform", "web", "--platform", "ios"],
        vec![".", "--platform", "web", "--target"],
        vec![".", "--platform", "web", "--target", "--json"],
        vec![".", "--platform", "web", "--target", ""],
        vec![".", "--platform", "web", "--target", "a", "--target", "b"],
        vec![".", "--platform", "web", "--json", "--json"],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}: {result:?}");
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).starts_with("usage: exact-bake compat "));
    }
    assert_eq!(
        std::fs::read_to_string(app.0.join("app.json")).unwrap(),
        "invalid JSON"
    );
    assert_eq!(std::fs::read_dir(&app.0).unwrap().count(), 1);
}
