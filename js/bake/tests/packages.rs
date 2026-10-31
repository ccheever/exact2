//! Package code in a data module (LLP 1027 D5, "Package code"): what the
//! bundle reaches of an installed package is captured, runs alike in every
//! producer, and what no executor has is refused by package and file.
use exact_js::Paired;
use exact_js_bake::{bake, Tools};
use exact_kernel::Kernel;
use exact_runner::{Runner, Value};
use serde_json::Value as Json;
use sha2::{Digest, Sha256};
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
        // `js` builds the server bundle too, which replays the capture under Bun.
        .args(["logic", "--render", "js", "--out"])
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
    let receipt: Json = serde_json::from_str(&baked.receipt).unwrap();
    let paired = Paired::decode(
        &baked.receipt,
        &baked.plan,
        baked.bytecode.clone(),
        "test.exact.logic",
        receipt["grants"].as_str().unwrap(),
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

/// A side effect on this machine, written as no static check sees it: it
/// runs only if something executes the file in the build's own process.
fn sentinel(ran: &Path) -> String {
    format!(
        "globalThis['pro' + 'cess']?.getBuiltinModule?.('fs')?.writeFileSync({:?}, 'ran');\n",
        ran.to_str().unwrap()
    )
}

/// The web build's record of the captured package code.
fn record(app: &App) -> Json {
    serde_json::from_slice(
        &std::fs::read(app.0.join("web-out/.gen/typescript/__exact_packages.json")).unwrap(),
    )
    .unwrap()
}

/// ESM and CommonJS packages (`exports` with a `browser` condition, `main`,
/// a `browser` field's `false`, `.mjs`/`.cjs`, JSON, a nested dependency) and
/// the app's own `.js` bundle from the capture alike in every producer: the
/// page and Hermes run the `browser` file, the unselected Node entry never
/// runs, each captured file's recorded digest is its bytes, the receipt
/// carries the record's digests, and a bake is reproducible.
#[test]
fn installed_packages_and_app_javascript_bundle_from_the_capture() {
    let app = App::new("bundle");
    let ran = app.0.join("ran");
    app.put(
        "node_modules/esm/package.json",
        r#"{"name":"esm","version":"2.0.0","type":"module","exports":{".":{"browser":"./web.js","default":"./node.js"},"./extra":"./extra.mjs"}}"#,
    );
    app.put(
        "node_modules/esm/web.js",
        "export const where = 'web-build';\n",
    );
    app.put(
        "node_modules/esm/node.js",
        &format!("{}export const where = 'node-build';\n", sentinel(&ran)),
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
    // A dominating test: a package that runs without the DOM is not refused.
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
    assert!(!ran.exists(), "the unselected Node entry never runs");
    let page: String = std::fs::read_dir(app.0.join("web-out"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "js"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    assert!(
        page.contains("web-build") && !page.contains("node-build"),
        "the page bundles the browser file"
    );
    let record = record(&app);
    let files = record["files"].as_array().unwrap();
    for file in files {
        let bytes = std::fs::read(file[2].as_str().unwrap()).unwrap();
        assert_eq!(
            file[3].as_str().unwrap(),
            format!("{:x}", Sha256::digest(&bytes)),
            "{file}"
        );
    }
    let names: Vec<String> = files
        .iter()
        .map(|f| format!("{}/{}", f[0].as_str().unwrap(), f[1].as_str().unwrap()))
        .collect();
    assert!(
        names.contains(&"cjs@1.5.4/lib/twice.cjs".into())
            && names.contains(&"pad@0.1.0/index.js".into()),
        "{names:?}"
    );
    assert!(
        !names.iter().any(|n| n.ends_with("node.js")),
        "only what the bundle reaches is captured: {names:?}"
    );
    let installed = record["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p[0] == "pad@0.1.0")
        .unwrap()[2]
        .clone();
    assert!(
        installed[0]
            .as_str()
            .unwrap()
            .ends_with("node_modules/cjs/node_modules/pad"),
        "{installed}"
    );
    if !exact_js::ENGINE_LINKED {
        return;
    }
    assert_eq!(message(&app), Value::str("web-build extra <100>fs"));
    let first = bake(&app.0, &Tools::default()).unwrap();
    let again = bake(&app.0, &Tools::default()).unwrap();
    assert_eq!(
        (&first.bytecode, &first.receipt),
        (&again.bytecode, &again.receipt)
    );
    let receipt: Json = serde_json::from_str(&first.receipt).unwrap();
    assert_eq!(
        receipt["packages"], record["digests"],
        "the receipt carries the capture's digests"
    );
    // A captured file changed is another bake.
    app.put(
        "node_modules/cjs/lib/twice.cjs",
        "module.exports = (n) => n * 3;\n",
    );
    let changed: Json =
        serde_json::from_str(&bake(&app.0, &Tools::default()).unwrap().receipt).unwrap();
    assert_ne!(
        changed["packages"]["cjs@1.5.4"],
        receipt["packages"]["cjs@1.5.4"]
    );
    assert_eq!(
        changed["packages"]["pad@0.1.0"],
        receipt["packages"]["pad@0.1.0"]
    );
}

/// An install that is a link (as Bun's and pnpm's isolated layouts make
/// them) is recorded where it is installed, and retargeting it to another
/// copy of the same version is another bake; links swapped between two
/// packages change the record too.
#[cfg(unix)]
#[test]
fn a_retargeted_install_link_is_another_bake() {
    let app = App::new("links");
    for (dir, word) in [
        ("node_modules/.store/a1", "first"),
        ("node_modules/.store/a2", "second"),
        ("node_modules/.store/b", "bee"),
    ] {
        let name = if dir.ends_with("/b") { "b" } else { "a" };
        app.put(
            &format!("{dir}/package.json"),
            &format!(r#"{{"name":"{name}","version":"1.0.0","main":"index.js"}}"#),
        );
        app.put(
            &format!("{dir}/index.js"),
            &format!("module.exports = '{word}';\n"),
        );
    }
    std::fs::create_dir_all(app.0.join("node_modules")).unwrap();
    let link = |name: &str, to: &str| {
        let path = app.0.join("node_modules").join(name);
        let _ = std::fs::remove_file(&path);
        std::os::unix::fs::symlink(app.0.join(to), path).unwrap();
    };
    link("a", "node_modules/.store/a1");
    link("b", "node_modules/.store/b");
    app.put("packages.d.ts", "declare module 'a' { const a: string; export default a; }\ndeclare module 'b' { const b: string; export default b; }\n");
    app.answer("import a from 'a';\nimport b from 'b';", "a + b + count");
    web_build(&app.0).unwrap();
    let installed = record(&app)["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p[0] == "a@1.0.0")
        .unwrap()[2]
        .clone();
    assert_eq!(
        installed[0].as_str().unwrap(),
        app.0
            .canonicalize()
            .unwrap()
            .join("node_modules/a")
            .to_str()
            .unwrap(),
        "recorded as installed, not as stored"
    );
    let digests = record(&app)["digests"].clone();
    link("a", "node_modules/.store/a2");
    web_build(&app.0).unwrap();
    assert_ne!(
        record(&app)["digests"]["a@1.0.0"],
        digests["a@1.0.0"],
        "a retargeted link is another capture"
    );
    if exact_js::ENGINE_LINKED {
        assert_eq!(message(&app), Value::str("secondbee0"));
    }
    // The same files reached by each other's names: the resolutions are captured too.
    let digests = record(&app)["digests"].clone();
    link("a", "node_modules/.store/b");
    link("b", "node_modules/.store/a2");
    web_build(&app.0).unwrap();
    let swapped = record(&app)["digests"].clone();
    assert!(swapped["a@1.0.0"] != digests["a@1.0.0"] && swapped["b@1.0.0"] != digests["b@1.0.0"]);
}

/// What no executor has is refused by every producer, naming the package,
/// its version and the file: a Node built-in, a native addon, a dependency
/// not installed, the DOM, Node's globals, and a computed `require`. Nothing
/// of a package runs in the build's process, refused or not.
#[test]
fn package_code_no_executor_runs_is_refused_by_package_and_file() {
    let app = App::new("refused");
    app.put("packages.d.ts", "declare module 'bad' { const bad: string; export default bad; }\ndeclare module 'absent' { const a: string; export default a; }\n");
    // Each after the sentinel line, so a position is on line 2.
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
            "bad@3.1.0 (index.js:2:18) uses the DOM's 'document'",
        ),
        (
            "module.exports = Buffer.from('x').toString();\n",
            "bad@3.1.0 (index.js:2:18) uses Node's 'Buffer'",
        ),
        (
            "const name = 'x';\nmodule.exports = require('./' + name);\n",
            "bad@3.1.0 (index.js:3:18) loads a module by a computed name",
        ),
        // An unrelated test of the name, or a parameter of that name in another function, guards nothing.
        (
            "typeof document;\nmodule.exports = document.title;\n",
            "bad@3.1.0 (index.js:3:18) uses the DOM's 'document'",
        ),
        (
            "function f(document) { return document; }\nmodule.exports = () => document.title;\n",
            "bad@3.1.0 (index.js:3:24) uses the DOM's 'document'",
        ),
        // Nor does a `try`, or reaching it through the global object.
        (
            "try { module.exports = document.cookie; } catch (e) {}\n",
            "bad@3.1.0 (index.js:2:24) uses the DOM's 'document'",
        ),
        (
            "module.exports = () => globalThis.localStorage.getItem('k');\n",
            "bad@3.1.0 (index.js:2:24) uses the DOM's 'localStorage'",
        ),
        // The page's other names for itself, and code from a string.
        (
            "module.exports = () => globalThis.top.document.cookie;\n",
            "bad@3.1.0 (index.js:2:24) uses the DOM's 'top'",
        ),
        (
            "module.exports = Function('return 1');\n",
            "bad@3.1.0 (index.js:2:18) loads code from a string ('Function')",
        ),
        (
            "module.exports = (0, eval)('1');\n",
            "bad@3.1.0 (index.js:2:22) loads code from a string ('eval')",
        ),
        (
            "module.exports = (name) => import(name);\n",
            "bad@3.1.0 (index.js:2:28) loads code from a string ('import()')",
        ),
        // A setting is app.ts's to read (LLP 1153 D4), not a package function's.
        (
            "export default () => import.meta.env.API_BASE_URL;\n",
            "bad@3.1.0 (index.js:2:22) reads import.meta.env, which only app.ts may",
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
        app.put(
            "node_modules/bad/index.js",
            &format!("{}{code}", sentinel(&ran)),
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
    std::fs::remove_file(app.0.join("node_modules/bad/binding.gyp")).unwrap();
    // A direct dependency the app declares but nobody installed.
    app.answer("import absent from 'absent';", "absent + count");
    for result in every(&app) {
        let error = result.expect_err("absent");
        assert!(
            error.contains("app.ts imports 'absent', which is not installed"),
            "{error}"
        );
    }
    // The app's own JavaScript is held to the same.
    app.put(
        "src/helper.js",
        "export const encode = (s) => Buffer.from(s).toString('hex');\n",
    );
    app.answer(
        "import { encode } from './src/helper.js';",
        "encode('a') + count",
    );
    for result in every(&app) {
        let error = result.expect_err("helper");
        assert!(
            error.contains("src/helper.js:1:30 uses Node's 'Buffer'"),
            "{error}"
        );
    }
    // A package's identity names a place in the capture: one that would leave it is refused, writing nothing.
    app.put(
        "node_modules/bad/package.json",
        r#"{"name":"bad","version":"../../../../escaped"}"#,
    );
    app.put("node_modules/bad/index.js", "module.exports = 'x';\n");
    app.answer("import bad from 'bad';", "bad + count");
    for result in every(&app) {
        let error = result.expect_err("identity");
        assert!(
            error.contains("not an npm package name and version"),
            "{error}"
        );
    }
    assert!(!std::env::temp_dir().join("escaped").exists() && !app.0.join("escaped").exists());
}

/// A use the static check cannot see (an alias of the global object) finds
/// the DOM absent on Hermes, as the web's module bindings make it on the page.
#[test]
fn an_aliased_dom_global_is_absent_where_the_module_runs() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let app = App::new("alias");
    app.put(
        "node_modules/sly/package.json",
        r#"{"name":"sly","version":"1.0.0"}"#,
    );
    app.put("node_modules/sly/index.js", "const g = globalThis, key = ['docu', 'ment'].join('');\nmodule.exports = () => typeof g[key];\n");
    app.put(
        "packages.d.ts",
        "declare module 'sly' { const sly: () => string; export default sly; }\n",
    );
    app.answer("import sly from 'sly';", "sly() + count");
    for result in every(&app) {
        result.unwrap();
    }
    assert_eq!(message(&app), Value::str("undefined0"));
}

/// No build runs app or package code to learn `appId` and `grants`: they are
/// read from app.ts's syntax, and one the build cannot read so is refused by
/// every producer, naming the fix. A `paths` alias that looks like a package
/// resolves in the capture and in both of the web's later bundles.
#[test]
fn app_facts_are_read_without_running_the_module_and_aliases_replay() {
    let app = App::new("facts");
    app.put("src/helper.ts", "export const helper = 'aliased';\n");
    app.put(
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@app/*":["./src/*"]}}}"#,
    );
    app.put(
        "app.ts",
        "import type { Sources, Answer } from './app.contract.d.ts';\nimport { helper } from '@app/helper';\nconst origin = 'http://127.0.0.1:4400';\nexport const appId = 'test.exact.logic';\nexport const grants = [`net.fetch ${origin}`, 'secret.keep logic.key'].join('\\n');\nconst sources: Sources = { message: ([count]) => helper + count };\nexport const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);\n",
    );
    for result in every(&app) {
        result.unwrap();
    }
    if exact_js::ENGINE_LINKED {
        assert_eq!(message(&app), Value::str("aliased0"));
    }
    let code = std::fs::read_to_string(app.0.join("app.ts")).unwrap();
    app.put(
        "app.ts",
        &code.replace(
            "export const grants = [",
            "const made = () => 'x';\nexport const grants = made() + [",
        ),
    );
    for result in every(&app) {
        let error = result.expect_err("computed grants");
        assert!(
            error.contains("grants must be a constant the build reads without running the module"),
            "{error}"
        );
    }
}

/// The web's later bundles resolve only what the capture recorded: a
/// recorded import (an absolute one too) to its captured file, the host's
/// injected module as itself, and anything else is refused.
#[test]
fn replay_resolves_recorded_imports_and_refuses_the_rest() {
    let app = App::new("replay");
    let stage = app.0.join("stage");
    std::fs::create_dir_all(stage.join("node_modules/.packages/p@1.0.0")).unwrap();
    std::fs::write(stage.join("app.ts"), "").unwrap();
    std::fs::write(stage.join("node_modules/.packages/p@1.0.0/index.js"), "").unwrap();
    std::fs::write(
        stage.join("__exact_packages.json"),
        r#"{"packages":[],"files":[],"digests":{},"edges":[["app.ts","/abs/p/index.js","import","node_modules/.packages/p@1.0.0/index.js"],["app.ts","./local","import","app.ts"]]}"#,
    )
    .unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let checked = exact_bake::bun()
        .args([
            "-e",
            r#"
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { resolve } from 'node:path';
const [config, stage] = process.argv.slice(1);
const { replayCaptured } = await import(config);
const real = realpathSync(stage), app = resolve(real, 'app.ts');
const { rolldown } = replayCaptured(stage, ['/host/ts-fetch.js']);
assert.equal(rolldown.resolveId('/abs/p/index.js', app, { kind: 'import-statement' }), resolve(real, 'node_modules/.packages/p@1.0.0/index.js'));
assert.equal(rolldown.resolveId('./local', app, { kind: 'import-statement' }), app);
assert.equal(rolldown.resolveId('/host/ts-fetch.js', app, { kind: 'import-statement' }), null);
assert.equal(rolldown.resolveId('./elsewhere.js', '/outside/file.js', {}), null);
for (const source of ['/abs/other.js', './unrecorded', 'pkg'])
  assert.throws(() => rolldown.resolveId(source, app, { kind: 'import-statement' }), /which the checked capture did not resolve/, source);
"#,
        ])
        .arg(root.join("js/bake/src/typescript.mjs"))
        .arg(&stage)
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
}
