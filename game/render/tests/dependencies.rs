#[test]
fn engine_workspace_manifests_never_depend_on_games() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    for member in [
        "", "engine", "derive", "bake", "render", "physics", "audio", "app",
    ] {
        let manifest = std::fs::read_to_string(root.join(member).join("Cargo.toml")).unwrap();
        assert!(!manifest.contains("games/"), "{member} depends on a game");
    }
}

#[test]
fn engine_test_sources_do_not_include_consumer_games() {
    fn scan(path: &std::path::Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scan(&path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                assert!(
                    !text.contains(&["../", "games/"].concat()),
                    "{} includes a game",
                    path.display()
                );
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    for member in [
        "engine", "derive", "bake", "render", "physics", "audio", "app",
    ] {
        for directory in ["src", "tests", "examples"] {
            let path = root.join(member).join(directory);
            if path.is_dir() {
                scan(&path);
            }
        }
    }
}
