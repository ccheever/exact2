//! @ref LLP 1038 D7/D11 — actual Chrome history, the real glue and runner.
use exact_runner::{DataError, DataSource, Value};
use std::path::Path;
use std::process::Command;

#[derive(Default)]
struct Empty;
impl DataSource for Empty {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(vec![]))
    }
}

/// Real Chrome sweep, run by `bun scripts/smoke.mjs web`.
#[test]
#[ignore = "real Chrome sweep: bun scripts/smoke.mjs web"]
fn browser_session_history_and_published_deep_locations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let chrome = std::env::var("CHROME")
        .unwrap_or_else(|_| "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into());
    let dist = std::env::var_os("EXACT_ROUTER_DIST")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("host/web/dist"));
    assert!(
        Path::new(&chrome).exists(),
        "Chrome is required: set CHROME"
    );
    assert!(dist.join("app.wasm").exists(), "build the web dist first");
    // The history fixture retains the corpus's route value, actions and
    // projection. Its data is constant so the Caltrain carrier needs no app
    // sources; the Back counter and refusal are authored Contract state.
    let source = include_str!("../../../contract/corpus/routes.contract")
        .replace("  notfound", "  other \"/other\"\n  notfound")
        .replace("params(nav, \"question\")", "")
        .replace("params(nav, \"post\")", "")
        .replace("params(nav, \"person\")", "")
        .replace(
            "  state initialUrl",
            "  state blurPresses = 0\n  action editorBlur writes blurPresses\n    blurPresses = blurPresses + 1\n  state initialUrl",
        )
        .replace(
            "  state initialUrl",
            "  state redirectLink = false\n  state followEnabled = true\n  state navigatePresses = 0\n  state backEnabled = true\n  state backReplacement = false\n  state backPresses = 0\n  state initialUrl",
        )
        .replace(
            "  action followLink(url: string) writes nav\n    nav = go(nav, url)",
            "  action followLink(url: string) writes nav, navigatePresses\n    navigatePresses = navigatePresses + 1\n    if redirectLink\n      nav = push(nav, \"/other\")\n    else\n      if followEnabled\n        nav = go(nav, url)\n  action redirectNextLink writes redirectLink\n    redirectLink = true\n  action refuseLink writes followEnabled\n    followEnabled = false",
        )
        .replace(
            "  action back writes nav\n    nav = back(nav)",
            "  action back writes nav, backPresses\n    backPresses = backPresses + 1\n    if backReplacement\n      nav = replace(back(nav), \"/?from=back\")\n    else\n      nav = back(nav)\n  action replaceBack writes backReplacement\n    backReplacement = true\n  action refuseBack writes backEnabled\n    backEnabled = false",
        )
        .replace(
            "          button id=\"back\"",
            "          textarea blur=editorBlur testId=`editor-${e.id}` height=32\n          button press=redirectNextLink testId=`redirect-link-${e.id}`\n            text \"Redirect link\"\n          button press=refuseLink testId=`refuse-link-${e.id}`\n            text \"Refuse link\"\n          button press=open(\"//evil.invalid/x\") testId=`open-unknown-${e.id}`\n            text \"Open unknown\"\n          button press=replace(\"//evil.invalid/y\") testId=`replace-unknown-${e.id}`\n            text \"Replace unknown\"\n          button id=\"back\"",
        )
        .replace("button id=\"back\" press=back", "button id=\"back\" disabled=(!backEnabled) press=back")
        .replace(
            "          when e.name == \"home\"",
            "          button press=replaceBack testId=`replace-back-${e.id}`\n            text \"Rewrite Back\"\n          button press=refuseBack testId=`refuse-${e.id}`\n            text \"Refuse Back\"\n          button press=replace(\"/post/43\") testId=`replace-${e.id}`\n            text \"Replace\"\n          button press=go(\"/\") testId=`go-home-${e.id}`\n            text \"Go home\"\n          when e.name == \"home\"",
        );
    let plan = contract::bake(contract::compile(&source).unwrap(), Empty).unwrap();
    let dir = std::env::temp_dir().join(format!("exact-router-browser-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.plan"), plan.encode()).unwrap();
    let no_handler = contract::bake(
        contract::compile(&source.replace(" navigate=followLink", "")).unwrap(),
        Empty,
    )
    .unwrap();
    std::fs::write(dir.join("no-navigate.plan"), no_handler.encode()).unwrap();
    let output = Command::new("bun")
        .arg("host/web/tests/navigation.mjs")
        .env("EXACT_ROUTER_DIST", dist)
        .env("CHROME", chrome)
        .env("EXACT_ROUTER_TEST", &dir)
        .current_dir(root)
        .output()
        .unwrap();
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
