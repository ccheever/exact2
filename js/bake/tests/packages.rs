//! Package code in a data module (LLP 1027 D5, "Package code"): what the
//! bundle reaches of an installed package is captured, runs alike in every
//! producer, and what no executor has is refused by package and file.
use exact_js::Paired;
use exact_js_bake::{bake, Tools};
use exact_kernel::Kernel;
use exact_runner::{Runner, Value};
use serde_json::Value as Json;
use std::path::{Path, PathBuf};

const CONTRACT: &str = r#"
component App
  state count = 0
  resource message = message(count) as shape string
  view
    column
      text message testId="message"
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("exact-packages-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        let app = Self(path);
        app.put("app.contract", CONTRACT);
        app.put(
            "app.json",
            r#"{"name":"Logic","app":{"id":"test.exact.logic","name":"Logic"}}"#,
        );
        app
    }
    fn put(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    /// `app.ts` answering `message` with `expression` (of `count`).
    fn answer(&self, imports: &str, expression: &str) {
        self.put(
            "app.ts",
            &format!("import type {{ Sources, Answer }} from './app.contract.d.ts';\n{imports}\nexport const appId = 'test.exact.logic';\nexport const grants = '';\nconst sources: Sources = {{ message: ([count]) => {expression} }};\nexport const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);\n"),
        );
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn web_build(app: &Path) -> Result<(), String> {
    let out = app.join("web-out");
    let _ = std::fs::remove_dir_all(&out);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let built = std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
        .arg(root.join("host/web-js/build.mjs"))
        .args(["logic", "--render", "none", "--out"])
        .arg(&out)
        .env("EXACT_APP_DIR", app)
        .current_dir(&root)
        .output()
        .unwrap();
    if built.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&built.stderr).into_owned())
    }
}

/// The web build, then (with Hermes) the resident and one-shot producers.
fn every(app: &App) -> Vec<Result<(), String>> {
    let mut results = vec![web_build(&app.0)];
    if exact_js::ENGINE_LINKED {
        let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
        results.push(producer.bake(&app.0, None).map(|_| ()));
        results.push(bake(&app.0, &Tools::default()).map(|_| ()));
    }
    results
}

fn message(app: &App) -> Value {
    let baked = bake(&app.0, &Tools::default()).unwrap();
    let paired = Paired::decode(
        &baked.receipt,
        &baked.plan,
        baked.bytecode.clone(),
        "test.exact.logic",
        "",
    )
    .unwrap();
    let runner = Runner::boot(
        paired.plan,
        paired.module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    runner.resource("message").unwrap().clone()
}

/// ESM and CommonJS packages (`exports` with a `browser` condition, `main`,
/// a `browser` field's `false`, `.mjs`/`.cjs`, JSON, a nested dependency) and
/// the app's own `.js` bundle from the capture alike in every producer; the
/// bake records each captured file by its digest and is reproducible.
#[test]
fn installed_packages_and_app_javascript_bundle_from_the_capture() {
    let app = App::new("bundle");
    app.put(
        "node_modules/esm/package.json",
        r#"{"name":"esm","version":"2.0.0","type":"module","exports":{".":{"browser":"./web.js","default":"./node.js"},"./extra":"./extra.mjs"}}"#,
    );
    app.put("node_modules/esm/web.js", "export const where = 'web';\n");
    app.put(
        "node_modules/esm/node.js",
        "import { readFileSync } from 'node:fs';\nexport const where = readFileSync;\n",
    );
    app.put("node_modules/esm/extra.mjs", "export default 'extra';\n");
    app.put(
        "node_modules/cjs/package.json",
        r#"{"name":"cjs","version":"1.5.4","main":"./lib/index.js","browser":{"fs":false}}"#,
    );
    app.put(
        "node_modules/cjs/lib/index.js",
        "const fs = require('fs');\nconst table = require('./table.json');\nconst pad = require('pad');\nconst twice = require('./twice.cjs');\nexports.encode = (n) => pad(String(twice(n) + table.offset)) + (typeof fs === 'object' ? 'fs' : '');\n",
    );
    app.put("node_modules/cjs/lib/table.json", r#"{"offset":100}"#);
    app.put(
        "node_modules/cjs/lib/twice.cjs",
        "module.exports = (n) => n * 2;\n",
    );
    app.put(
        "node_modules/cjs/node_modules/pad/package.json",
        r#"{"name":"pad","version":"0.1.0"}"#,
    );
    // Tested before use: a package that runs without the DOM is not refused.
    app.put(
        "node_modules/cjs/node_modules/pad/index.js",
        "module.exports = (s) => (typeof window !== 'undefined' ? window.name : '') + '<' + s + '>';\n",
    );
    app.put(
        "packages.d.ts",
        "declare module 'esm' { export const where: string; }\ndeclare module 'esm/extra' { const extra: string; export default extra; }\ndeclare module 'cjs' { export function encode(n: number): string; }\n",
    );
    app.put(
        "src/helper.js",
        "export const joined = (...parts) => parts.join(' ');\n",
    );
    app.answer(
        "import { where } from 'esm';\nimport extra from 'esm/extra';\nimport { encode } from 'cjs';\nimport { joined } from './src/helper.js';",
        "joined(where, extra, encode(count))",
    );
    for result in every(&app) {
        result.unwrap();
    }
    let record: Json = serde_json::from_slice(
        &std::fs::read(app.0.join("web-out/.gen/typescript/__exact_packages.json")).unwrap(),
    )
    .unwrap();
    let files: Vec<(String, String)> = record["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                format!("{}/{}", f[0].as_str().unwrap(), f[1].as_str().unwrap()),
                f[3].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(
        files
            .iter()
            .any(|(path, sha)| path == "cjs@1.5.4/lib/twice.cjs" && sha.len() == 64),
        "{files:?}"
    );
    assert!(files.iter().any(|(path, _)| path == "pad@0.1.0/index.js"));
    assert!(
        !files.iter().any(|(path, _)| path.ends_with("node.js")),
        "only the files the bundle reaches are captured: {files:?}"
    );
    if !exact_js::ENGINE_LINKED {
        return;
    }
    assert_eq!(message(&app), Value::str("web extra <100>fs"));
    let first = bake(&app.0, &Tools::default()).unwrap();
    let again = bake(&app.0, &Tools::default()).unwrap();
    assert_eq!(
        (&first.bytecode, &first.receipt),
        (&again.bytecode, &again.receipt)
    );
    let receipt: Json = serde_json::from_str(&first.receipt).unwrap();
    for key in ["esm@2.0.0", "cjs@1.5.4", "pad@0.1.0"] {
        assert_eq!(
            receipt["packages"][key].as_str().map(str::len),
            Some(64),
            "{key}: {receipt}"
        );
    }
    // A captured file changed is another bake.
    app.put(
        "node_modules/cjs/lib/twice.cjs",
        "module.exports = (n) => n * 3;\n",
    );
    let changed = bake(&app.0, &Tools::default()).unwrap();
    let changed: Json = serde_json::from_str(&changed.receipt).unwrap();
    assert_ne!(
        changed["packages"]["cjs@1.5.4"],
        receipt["packages"]["cjs@1.5.4"]
    );
    assert_eq!(
        changed["packages"]["pad@0.1.0"],
        receipt["packages"]["pad@0.1.0"]
    );
}

/// What no executor has is refused by every producer, naming the package,
/// its version and the file: a Node built-in, a native addon, a dependency
/// not installed, the DOM, Node's globals, and a computed `require`.
#[test]
fn package_code_no_executor_runs_is_refused_by_package_and_file() {
    let app = App::new("refused");
    app.put(
        "packages.d.ts",
        "declare module 'bad' { const bad: string; export default bad; }\n",
    );
    let cases = [
        (
            "const fs = require('fs');\nmodule.exports = 'x';\n",
            "bad@3.1.0 (index.js) imports Node's built-in 'fs'",
        ),
        (
            "module.exports = require('./build/Release/bad.node');\n",
            "imports bad@3.1.0, a native addon",
        ),
        (
            "module.exports = require('missing');\n",
            "bad@3.1.0 (index.js) imports 'missing', which is not installed",
        ),
        (
            "module.exports = document.title;\n",
            "bad@3.1.0 (index.js:1:18) uses the DOM's 'document'",
        ),
        (
            "module.exports = Buffer.from('x').toString();\n",
            "bad@3.1.0 (index.js:1:18) uses Node's 'Buffer'",
        ),
        (
            "const name = 'x';\nmodule.exports = require('./' + name);\n",
            "bad@3.1.0 (index.js:2:18) loads a module by a computed name",
        ),
    ];
    app.put(
        "node_modules/bad/package.json",
        r#"{"name":"bad","version":"3.1.0"}"#,
    );
    app.put("node_modules/bad/build/Release/bad.node", "not JavaScript");
    let ran = app.0.join("ran");
    app.answer("import bad from 'bad';", "bad + count");
    for (code, why) in cases {
        // Its code says so if it ever runs (the web build's Bun import of app.ts).
        app.put(
            "node_modules/bad/index.js",
            &format!(
                "{code}globalThis.process?.getBuiltinModule('fs').writeFileSync({:?}, 'ran');\n",
                ran.to_str().unwrap()
            ),
        );
        for result in every(&app) {
            let error = result.expect_err(why);
            assert!(error.contains(why), "{why}: {error}");
        }
        assert!(!ran.exists(), "a refused package's code never runs: {why}");
    }
    // A package that is a native addon by its build file.
    app.put("node_modules/bad/index.js", "module.exports = 'x';\n");
    app.put("node_modules/bad/binding.gyp", "{}");
    for result in every(&app) {
        let error = result.expect_err("binding.gyp");
        assert!(
            error.contains("imports bad@3.1.0, a native addon"),
            "{error}"
        );
    }
}
