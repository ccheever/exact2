//! LLP 1091 D8/D9: `exact:` built-ins and packages through `node_modules`.

#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

struct Dir(PathBuf);
impl Dir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("exact-pkg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exact_motion_gives_every_file_one_activity_timeline() {
    let dir = Dir::new("builtin");
    dir.write(
        "spinner.contract",
        "use Activity from \"exact:motion\"\nkeyframes spin\n  to rotate=\"360deg\"\ncomponent Spinner\n  view\n    view animation=\"spin 1s infinite\" animation-timeline=Activity\n",
    );
    let root = dir.write(
        "app.contract",
        "use Spinner from \"./spinner.contract\"\nuse Activity from \"exact:motion\"\nkeyframes pulse\n  to opacity=0\ncomponent App\n  view\n    column\n      Spinner()\n      view animation=\"pulse 1s infinite\" animation-timeline=Activity\n",
    );
    let plan = format!("{:?}", contract::compile_path(&root).unwrap());
    assert!(plan.contains("clock(Activity)"), "{plan}");
    assert!(
        !plan.contains("Activity__"),
        "one declaration, one name: {plan}"
    );
    let bad = dir.write(
        "bad.contract",
        "use Activity from \"exact:moton\"\ncomponent App\n  view\n    text \"x\"\n",
    );
    let e = contract::compile_path(&bad).unwrap_err();
    assert_eq!(e.id, "contract-use-builtin", "{e}");
    assert!(e.message.contains("`exact:motion`"), "{}", e.message);
}

fn library(dir: &Dir, at: &str, package_json: &str) {
    dir.write(&format!("{at}/package.json"), package_json);
    dir.write(
        &format!("{at}/src/card.contract"),
        "use Pad from \"../styles.contract\"\ncomponent Card\n  view\n    column class=Pad testId=\"card\"\n",
    );
    dir.write(
        &format!("{at}/styles.contract"),
        "style Pad\n  padding-top=10\n",
    );
    dir.write(
        &format!("{at}/index.contract"),
        "use Card from \"./src/card.contract\"\n",
    );
}

#[test]
fn a_package_is_found_through_node_modules_and_mapped_through_its_exports() {
    let dir = Dir::new("exports");
    library(
        &dir,
        "app/node_modules/@acme/ui",
        r#"{"name":"@acme/ui","version":"1.2.3","exports":{".":{"contract":"./index.contract"},"./card":"./src/card.contract"}}"#,
    );
    let root = dir.write(
        "app/app.contract",
        "use Card from \"@acme/ui\"\nuse Card as Same from \"@acme/ui/card\"\ncomponent App\n  view\n    column\n      Card()\n      Same()\n",
    );
    contract::compile_path(&root).unwrap();
    // `exports` offers only what it lists.
    dir.write(
        "app/app.contract",
        "use Pad from \"@acme/ui/styles.contract\"\ncomponent App\n  view\n    text \"x\"\n",
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert_eq!(e.id, "contract-use-package", "{e}");
    // Not installed: the refusal says how to add it.
    dir.write(
        "app/app.contract",
        "use Card from \"@acme/nothing\"\ncomponent App\n  view\n    text \"x\"\n",
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert_eq!(e.id, "contract-use-package", "{e}");
    assert!(e.message.contains("bun add @acme/nothing"), "{}", e.message);
    // Declared but not installed (a fresh checkout): the refusal says to install.
    dir.write(
        "app/package.json",
        r#"{"dependencies":{"@acme/nothing":"1.0.0"}}"#,
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert!(
        e.message.contains("bun install --frozen-lockfile"),
        "{}",
        e.message
    );
    std::fs::remove_file(dir.0.join("app/package.json")).unwrap();
    // A workspace's own package, as exact2's root declares `@exact/reading`.
    dir.write(
        "package.json",
        r#"{"workspaces":["libs/*"],"dependencies":{"@acme/nothing":"workspace:*"}}"#,
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert!(
        e.message.contains("is declared in") && e.message.contains("bun install --frozen-lockfile"),
        "{}",
        e.message
    );
    // A bare file name is a mistaken path, not a package.
    dir.write(
        "app/app.contract",
        "use Card from \"card.contract\"\ncomponent App\n  view\n    text \"x\"\n",
    );
    assert_eq!(
        contract::compile_path(&root).unwrap_err().id,
        "contract-use-path"
    );
}

#[cfg(unix)]
#[test]
fn a_linked_local_library_outside_the_app_resolves_and_stays_inside_itself() {
    // `"@me/ui": "file:../ui"` installs a link; the package's files are its
    // own root, outside the app, and its `..` may not leave it.
    let dir = Dir::new("linked");
    library(&dir, "ui", r#"{"name":"@me/ui","version":"0.1.0"}"#);
    std::fs::create_dir_all(dir.0.join("app/node_modules/@me")).unwrap();
    std::os::unix::fs::symlink(dir.0.join("ui"), dir.0.join("app/node_modules/@me/ui")).unwrap();
    let root = dir.write(
        "app/app.contract",
        "use Card from \"@me/ui\"\ncomponent App\n  view\n    Card()\n",
    );
    contract::compile_path(&root).unwrap();
    let graph = contract::source_graph(&root);
    assert!(graph.errors.is_empty(), "{:?}", graph.errors);
    let files: Vec<_> = graph
        .sources
        .iter()
        .map(|s| s.path.strip_prefix(&dir.0).unwrap_or(&s.path).to_path_buf())
        .collect();
    assert_eq!(
        files,
        [
            Path::new("app/app.contract"),
            Path::new("ui/index.contract"),
            Path::new("ui/src/card.contract"),
            Path::new("ui/styles.contract"),
        ]
    );
    match &graph.sources[1].origin {
        contract::Origin::Package {
            name,
            version,
            root,
            manifest,
        } => {
            assert_eq!((name.as_str(), version.as_str()), ("@me/ui", "0.1.0"));
            assert_eq!(root, &dir.0.join("ui"));
            assert_eq!(manifest, &dir.0.join("ui/package.json"));
        }
        other => panic!("{other:?}"),
    }
    dir.write("outside.contract", "style Pad\n  padding-top=1\n");
    dir.write(
        "ui/styles.contract",
        "use Pad from \"../outside.contract\"\n",
    );
    let e = contract::compile_path(&root).unwrap_err();
    assert_eq!(e.id, "contract-use-path", "{e}");
    assert!(
        e.message.contains("leaves the package `@me/ui`"),
        "{}",
        e.message
    );
    // A failed load still reports what it read, for a watcher.
    let graph = contract::source_graph(&root);
    assert_eq!(graph.errors.len(), 1);
    assert_eq!(graph.sources.len(), 4);
}

#[test]
fn an_app_file_may_use_dot_dot_inside_the_app() {
    let dir = Dir::new("dotdot");
    dir.write(
        "lib/row.contract",
        "component Row\n  view\n    text \"r\"\n",
    );
    dir.write(
        "screens/home.contract",
        "use Row from \"../lib/row.contract\"\ncomponent Home\n  view\n    Row()\n",
    );
    let root = dir.write(
        "app.contract",
        "use Home from \"./screens/home.contract\"\ncomponent App\n  view\n    Home()\n",
    );
    contract::compile_path(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn a_file_install_of_linked_files_is_the_library_where_its_files_are() {
    // Bun installs `"file:../ui"` as a directory of per-file links: the
    // package is found where the files really are, as Node's realpath does.
    let dir = Dir::new("file-install");
    library(&dir, "ui", r#"{"name":"@me/ui","version":"0.2.0"}"#);
    let installed = dir.0.join("app/node_modules/@me/ui");
    for file in [
        "package.json",
        "index.contract",
        "styles.contract",
        "src/card.contract",
    ] {
        let at = installed.join(file);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(dir.0.join("ui").join(file), at).unwrap();
    }
    let root = dir.write(
        "app/app.contract",
        "use Card from \"@me/ui\"\ncomponent App\n  view\n    Card()\n",
    );
    contract::compile_path(&root).unwrap();
    let graph = contract::source_graph(&root);
    match &graph.sources[1].origin {
        contract::Origin::Package { root, .. } => assert_eq!(root, &dir.0.join("ui")),
        other => panic!("{other:?}"),
    }
}
