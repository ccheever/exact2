use super::*;
use crate::grant::{Grant, Operation, Origin};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from("/project")
}

#[test]
fn relative_specifiers_resolve_against_the_importing_module() {
    assert_eq!(
        resolve(&Root::Declared(root()), "./index.js", "./util").unwrap(),
        "./util.js"
    );
    assert_eq!(
        resolve(&Root::Declared(root()), "./a/b.js", "./c").unwrap(),
        "./a/c.js"
    );
    assert_eq!(
        resolve(&Root::Declared(root()), "./a/b.js", "../top").unwrap(),
        "./top.js"
    );
    assert_eq!(
        resolve(&Root::Declared(root()), "./a/b/c.js", "../../x.js").unwrap(),
        "./x.js"
    );
}

/// `node:` and `bun:` are builtin namespaces, not packages. LLP 0059 §6
/// deleted Node's server surface, so the error should say that rather than
/// "not found in node_modules".
/// The JSX transform *injects* a dependency the developer never wrote, and
/// the build walk scans `to_javascript` output for exactly that reason.
/// Scanning the original `.tsx` misses the edge, so `react/jsx-runtime`
/// never gets compiled and `run --precompiled` fails on a graph the build
/// reported as complete — a failure that looks like a fast start when timed.
///
/// Both shapes are covered because the transform emits different ones: a
/// module gets an `import`, a script gets a `require`. A test over only one
/// would leave half the walk unguarded.
#[cfg(feature = "loader")]
#[test]
fn the_jsx_transform_injects_a_dependency_the_source_does_not_contain() {
    // Module form: the injected edge is an import.
    let module_source = "import { useState } from 'react';\nconst el = <div/>;";
    assert_eq!(
        crate::esm::dependencies(module_source, "./a.tsx"),
        vec!["react".to_string()],
        "the source itself imports only react"
    );
    let javascript = to_javascript(module_source, "./a.tsx").expect("strips");
    assert!(
        crate::esm::dependencies(&javascript, "./a.tsx")
            .iter()
            .any(|d| d == "react/jsx-runtime"),
        "the injected import must be visible to the build walk: {javascript}"
    );

    // Script form: no imports, so the transform reaches for require instead.
    let script_source = "const el = <div id=\"a\">hi</div>;";
    assert!(
        crate::esm::dependencies(script_source, "./a.tsx").is_empty(),
        "the source itself depends on nothing"
    );
    let javascript = to_javascript(script_source, "./a.tsx").expect("strips");
    assert!(
        javascript.contains("require(\"react/jsx-runtime\")"),
        "the injected require must be visible to the build walk: {javascript}"
    );
}

#[test]
fn builtin_namespaces_are_refused_by_name() {
    let err = resolve(&Root::Declared(root()), "./index.js", "node:fs").unwrap_err();
    assert!(err.contains("builtin namespace"), "{err}");
    assert!(resolve(&Root::Declared(root()), "./index.js", "bun:test").is_err());
}

/// A URL is not a package and not a builtin, and saying otherwise sends the
/// reader to `node_modules` for something that was never going to be there.
#[test]
fn a_url_specifier_is_refused_as_a_url() {
    let err = resolve(
        &Root::Declared(root()),
        "./index.js",
        "https://evil.example/m.js",
    )
    .unwrap_err();
    assert!(!err.contains("builtin namespace"), "{err}");
    assert!(err.contains("not over"), "{err}");
}

#[test]
fn a_package_that_does_not_exist_is_a_resolution_error() {
    let err = resolve(&Root::Declared(root()), "./index.js", "lodash").unwrap_err();
    assert!(err.contains("cannot resolve"), "{err}");
}

/// Escaping the root is a resolution failure, not an fs.read question.
#[test]
fn escaping_the_project_root_is_refused() {
    assert!(resolve(&Root::Declared(root()), "./index.js", "../secrets").is_err());
    assert!(resolve(&Root::Declared(root()), "./index.js", "../../etc/passwd").is_err());
    assert!(resolve(&Root::Declared(root()), "./a/b.js", "../../../etc/passwd").is_err());
    assert!(resolve(&Root::Declared(root()), "./index.js", "/etc/passwd").is_err());
}

#[test]
fn the_js_extension_is_added_but_not_doubled() {
    assert_eq!(
        resolve(&Root::Declared(root()), "./index.js", "./a").unwrap(),
        "./a.js"
    );
    assert_eq!(
        resolve(&Root::Declared(root()), "./index.js", "./a.js").unwrap(),
        "./a.js"
    );
}

/// A JSON module becomes `module.exports = JSON.parse("…")`: the text is
/// carried as a string literal and parsed at load, never pasted in as an
/// object literal. The text passes through the ESM lowering and the build
/// walk's dependency scan untouched, however much it looks like code.
#[cfg(feature = "loader")]
#[test]
fn a_json_module_is_parsed_not_evaluated() {
    let text = "{\"a\": \"line\\nbreak\"}\n";
    let out = to_javascript(text, "./x.json").unwrap();
    assert_eq!(
        out,
        "module.exports = JSON.parse(\"{\\\"a\\\": \\\"line\\\\nbreak\\\"}\\n\");"
    );
    assert!(
        to_javascript("\"\u{2028}\"", "./x.json")
            .unwrap()
            .contains("\\u2028"),
        "a line terminator inside the literal is escaped"
    );

    let looks_like_code = "{\"k\": \"import a from './b'; require('./c')\"}";
    let javascript = to_javascript(looks_like_code, "./x.json").unwrap();
    assert!(crate::esm::dependencies(&javascript, "./x.json").is_empty());
    assert_eq!(crate::esm::lower(&javascript).unwrap(), javascript);
    assert!(lower_and_wrap(looks_like_code, "./x.json")
        .unwrap()
        .contains("JSON.parse("));
}

#[test]
fn a_wrapped_module_ends_cleanly_after_a_trailing_comment() {
    let wrapped = wrap("exports.x = 1; // trailing comment");
    assert!(wrapped.ends_with("\n})"), "{wrapped}");
    assert!(wrapped.starts_with(
        "(function (module, exports, require, fetch, fs, process, __ibex2_meta, sqlite) {"
    ));
}

#[test]
fn capability_names_are_parameters_and_not_globals() {
    for capability in ["fetch", "fs", "sqlite"] {
        assert!(MODULE_PARAMETERS.contains(&capability));
        assert!(
            !allowed_globals(crate::bindings::Groups::DEFAULT).contains(&capability),
            "{capability} must not be reachable from the global object (LLP 0067 R1)"
        );
    }
}

#[test]
fn group_partition_covers_the_default_global_snapshot_exactly() {
    let mut coverage = vec![0usize; DEFAULT_ADDED_GLOBALS.len()];
    for (_, members) in GLOBAL_PARTITION {
        for &index in *members {
            assert!(
                index < coverage.len(),
                "partition index {index} is out of range"
            );
            coverage[index] += 1;
        }
    }
    assert!(
        coverage.iter().all(|count| *count == 1),
        "each snapshot name must belong to exactly one partition: {coverage:?}"
    );
    assert_eq!(
        allowed_globals(crate::bindings::Groups::DEFAULT),
        DEFAULT_ADDED_GLOBALS
    );
}

#[test]
fn a_module_without_an_entry_gets_the_default_and_the_default_is_nothing() {
    let grants = ModuleGrants::none();
    assert!(grants.for_module("./anything.js").is_empty());
}

#[test]
fn a_manifest_gives_each_module_its_own_authority() {
    let grants = ModuleGrants::parse(
        "[*]\n\
         [./net.js]\n\
         net.fetch https://api.example.com\n\
         [./storage.js]\n\
         fs.read /data\n",
    )
    .unwrap();

    let api = Operation::Fetch {
        origin: Origin::new("https", "api.example.com", 443),
    };
    assert!(grants.for_module("./net.js").permits(&api));
    assert!(!grants.for_module("./storage.js").permits(&api));
    assert!(!grants.for_module("./other.js").permits(&api));

    let data = Operation::FsRead {
        path: "/data/x".into(),
    };
    assert!(grants.for_module("./storage.js").permits(&data));
    assert!(!grants.for_module("./net.js").permits(&data));
}

#[test]
fn a_default_section_applies_only_where_there_is_no_entry() {
    let grants = ModuleGrants::parse(
        "[*]\n\
         net.fetch https://common.example.com\n\
         [./locked.js]\n",
    )
    .unwrap();
    let common = Operation::Fetch {
        origin: Origin::new("https", "common.example.com", 443),
    };
    assert!(grants.for_module("./anything.js").permits(&common));
    assert!(
        !grants.for_module("./locked.js").permits(&common),
        "an explicit empty section must not inherit the default"
    );
}

#[test]
fn a_bad_manifest_is_refused() {
    assert!(ModuleGrants::parse("[./a.js]\nnonsense.cap x\n").is_err());
    assert!(ModuleGrants::parse("[./a.js]\nnet.fetch not-a-url\n").is_err());
}

#[test]
fn grants_compose_from_the_spec_format_the_binding_already_uses() {
    let grants = ModuleGrants::none().with_module(
        "./a.js",
        GrantSet::none().with(Grant::Fetch(Origin::new("https", "x.test", 443))),
    );
    assert!(grants.for_module("./a.js").permits(&Operation::Fetch {
        origin: Origin::new("https", "x.test", 443)
    }));
}

/// A temp project with an in-place install, a look-alike, a scoped pair,
/// a workspace package, and a directory named after a granted package
/// nested inside another package.
fn bound(manifest: &str) -> (ModuleGrants, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "ibex2-bind-{}-{}",
        std::process::id(),
        std::thread::current()
            .name()
            .unwrap_or("t")
            .replace("::", "-")
    ));
    let _ = std::fs::remove_dir_all(&root);
    for dir in [
        "node_modules/react/cjs",
        "node_modules/react-dom",
        "node_modules/@w/ui",
        "node_modules/@w/ui-extra",
        "node_modules/evil/node_modules/react",
        "node_modules/lodash",
        "packages/wsp",
        "src/deep",
    ] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    for file in [
        "node_modules/react/index.js",
        "node_modules/react/cjs/react.production.js",
        "node_modules/react/locked.js",
        "node_modules/react-dom/index.js",
        "node_modules/@w/ui/index.js",
        "node_modules/@w/ui-extra/index.js",
        "node_modules/evil/index.js",
        "node_modules/evil/node_modules/react/index.js",
        "node_modules/lodash/index.js",
        "packages/wsp/index.js",
        "src/a.js",
        "src/deep/b.js",
        "other.js",
    ] {
        std::fs::write(root.join(file), "").unwrap();
    }
    symlink_dir(root.join("packages/wsp"), root.join("node_modules/@w/wsp")).unwrap();
    let mut grants = ModuleGrants::parse(manifest).unwrap();
    grants.bind(&root).unwrap();
    (grants, root)
}

/// `[react]` covers the install `bind` resolved and nothing beside it: not
/// `react-dom`, not a sibling, not the module that imported it — and not
/// a directory named `react` nested inside another package, which is a
/// different install however it is spelt.
#[test]
#[cfg_attr(
    windows,
    ignore = "Windows symlink fixtures require Developer Mode or SeCreateSymbolicLinkPrivilege"
)]
fn a_package_section_covers_its_install_and_nothing_beside_it() {
    let (grants, root) = bound(
        "[*]\n[react]\nnet.fetch https://api.example.com\n[@w/ui]\nfs.read /data\n[@w/wsp]\nfs.read /data\n",
    );
    let api = Operation::Fetch {
        origin: Origin::new("https", "api.example.com", 443),
    };
    assert!(grants
        .for_module("./node_modules/react/index.js")
        .permits(&api));
    assert!(grants
        .for_module("./node_modules/react/cjs/react.production.js")
        .permits(&api));
    assert!(!grants
        .for_module("./node_modules/react-dom/index.js")
        .permits(&api));
    assert!(!grants
        .for_module("./node_modules/evil/index.js")
        .permits(&api));
    assert!(
        !grants
            .for_module("./node_modules/evil/node_modules/react/index.js")
            .permits(&api),
        "a directory named after a granted package, nested in another package, is not that package"
    );
    assert!(!grants.for_module("./index.js").permits(&api));
    let data = Operation::FsRead {
        path: "/data/x".into(),
    };
    assert!(grants
        .for_module("./node_modules/@w/ui/index.js")
        .permits(&data));
    assert!(!grants
        .for_module("./node_modules/@w/ui-extra/index.js")
        .permits(&data));
    assert!(!grants
        .for_module("./node_modules/react/index.js")
        .permits(&data));
    // The workspace package is bound to its real directory.
    assert!(grants.for_module("./packages/wsp/index.js").permits(&data));
    assert!(
        grants
            .bound_packages()
            .any(|(dir, name)| dir == "./packages/wsp/" && name == "@w/wsp"),
        "{:?}",
        grants.bound_packages().collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Most specific wins, and nothing is combined: a file section beats its
/// package's, a package section beats a directory covering it — a
/// workspace package's too, which the first version got backwards — the
/// longest directory beats a shorter one, and `*` is last.
#[test]
#[cfg_attr(
    windows,
    ignore = "Windows symlink fixtures require Developer Mode or SeCreateSymbolicLinkPrivilege"
)]
fn the_most_specific_section_names_a_module_and_nothing_is_combined() {
    let (grants, root) = bound(
        "[*]\nnet.fetch https://star.test\n\
         [./src/]\nnet.fetch https://src.test\n\
         [./src/deep/]\nnet.fetch https://deep.test\n\
         [./node_modules/]\nnet.fetch https://nm.test\n\
         [./packages/]\nnet.fetch https://dir.test\n\
         [react]\nnet.fetch https://react.test\n\
         [@w/wsp]\n\
         [./node_modules/react/locked.js]\n",
    );
    let reaches = |module: &str, host: &str| {
        grants.for_module(module).permits(&Operation::Fetch {
            origin: Origin::new("https", host, 443),
        })
    };
    assert!(reaches("./src/a.js", "src.test") && !reaches("./src/a.js", "star.test"));
    assert!(reaches("./src/deep/b.js", "deep.test") && !reaches("./src/deep/b.js", "src.test"));
    assert!(reaches("./other.js", "star.test"));
    assert!(reaches("./node_modules/react/index.js", "react.test"));
    assert!(
        !reaches("./node_modules/react/index.js", "nm.test"),
        "package beats directory"
    );
    assert!(reaches("./node_modules/lodash/index.js", "nm.test"));
    assert!(
        !reaches("./node_modules/react/locked.js", "react.test"),
        "an explicit empty file section inside a granted package still means nothing"
    );
    assert!(
        !reaches("./packages/wsp/index.js", "dir.test"),
        "an empty package section beats a directory section covering the same workspace tree"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_section_that_names_nothing_is_refused() {
    for bad in [
        "[../x.js]",
        "[/etc/x.js]",
        "[node_modules/react]",
        "[react/]",
        "[@w]",
        "[@w/ui/x]",
        "[./]",
        "[./a/../b.js]",
    ] {
        assert!(
            ModuleGrants::parse(&format!("{bad}\n")).is_err(),
            "{bad} should be refused"
        );
    }
    for good in [
        "[*]",
        "[./x.js]",
        "[./src/]",
        "[react]",
        "[@w/ui]",
        "[lodash.merge]",
    ] {
        assert!(
            ModuleGrants::parse(&format!("{good}\n")).is_ok(),
            "{good} should parse"
        );
    }
}

/// `bind` refuses a package that is not installed, and turns a workspace
/// package — a symlink whose files live in the project's own tree — into
/// the directory section its files actually match.
#[test]
#[cfg_attr(
    windows,
    ignore = "Windows symlink fixtures require Developer Mode or SeCreateSymbolicLinkPrivilege"
)]
fn bind_refuses_the_uninstalled_and_binds_workspace_packages_to_their_directory() {
    let root = std::env::temp_dir().join(format!("ibex2-bind-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("node_modules/react")).unwrap();
    std::fs::write(root.join("node_modules/react/index.js"), "").unwrap();
    std::fs::create_dir_all(root.join("packages/ui")).unwrap();
    std::fs::write(root.join("packages/ui/index.js"), "").unwrap();
    std::fs::create_dir_all(root.join("node_modules/@w")).unwrap();
    symlink_dir(root.join("packages/ui"), root.join("node_modules/@w/ui")).unwrap();

    let api = Operation::Fetch {
        origin: Origin::new("https", "api.example.com", 443),
    };
    let mut grants = ModuleGrants::parse(
        "[react]\nnet.fetch https://api.example.com\n[@w/ui]\nnet.fetch https://api.example.com\n",
    )
    .unwrap();
    assert!(
        !grants.for_module("./packages/ui/index.js").permits(&api),
        "before bind the path does not name the workspace package"
    );
    grants.bind(&root).unwrap();
    assert!(grants.for_module("./packages/ui/index.js").permits(&api));
    assert!(grants
        .for_module("./node_modules/react/index.js")
        .permits(&api));
    assert!(!grants.for_module("./packages/other/index.js").permits(&api));

    let err = ModuleGrants::parse("[nope]\nnet.fetch https://api.example.com\n")
        .unwrap()
        .bind(&root)
        .unwrap_err();
    assert!(
        err.contains("\"nope\"") && err.contains("not installed"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The cache answers as the filesystem does — extension probing, a
/// directory index, a symlinked directory, a symlink out of the root, a
/// case-different spelling — and says what it does not: a file created
/// after its directory was listed is unseen until the loader is set again.
#[test]
#[cfg_attr(
    windows,
    ignore = "Windows symlink fixtures require Developer Mode or SeCreateSymbolicLinkPrivilege"
)]
fn the_resolve_cache_answers_as_the_filesystem_does() {
    let root = std::env::temp_dir().join(format!("ibex2-resolve-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src/dir")).unwrap();
    for file in ["src/a.ts", "src/b.js", "src/dir/index.ts", "index.js"] {
        std::fs::write(root.join(file), "").unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(std::env::current_exe().unwrap(), root.join("src/out.js")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(std::env::current_exe().unwrap(), root.join("src/out.js"))
        .unwrap();
    symlink_dir(root.join("src"), root.join("link")).unwrap();
    let declared = Root::Declared(root.clone());
    let cache = ResolveCache::default();
    let r = |spec: &str, from: &str| resolve_in(&cache, &declared, from, spec);

    assert_eq!(r("./a", "./src/x.js").unwrap(), "./src/a.ts");
    assert_eq!(
        r("./a.js", "./src/x.js").unwrap(),
        "./src/a.ts",
        "the .js -> .ts rewrite"
    );
    assert_eq!(r("./b.js", "./src/x.js").unwrap(), "./src/b.js");
    assert_eq!(r("./dir", "./src/x.js").unwrap(), "./src/dir/index.ts");
    assert_eq!(
        r("./link/a.ts", "./index.js").unwrap(),
        "./src/a.ts",
        "a symlinked directory resolves to its real path"
    );
    let err = r("./out.js", "./src/x.js").unwrap_err();
    assert!(err.contains("outside the project root"), "{err}");
    // A different spelling: settled to the on-disk one where the filesystem
    // folds case, refused where it does not — either way, one file, one name.
    match r("./A.ts", "./src/x.js") {
        Ok(spec) => assert_eq!(spec, "./src/a.ts"),
        Err(err) => assert!(err.contains("is not a file"), "{err}"),
    }
    // The stated limit: a file created after its directory was listed is
    // unseen by this cache — resolution falls to the `.js` spelling a
    // missing file gets, so the error names what was asked — and seen by
    // a fresh one.
    std::fs::write(root.join("src/late.ts"), "").unwrap();
    assert_eq!(r("./late", "./src/x.js").unwrap(), "./src/late.js");
    assert_eq!(
        resolve(&declared, "./src/x.js", "./late").unwrap(),
        "./src/late.ts"
    );
    let _ = std::fs::remove_dir_all(&root);
}
