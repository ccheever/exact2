//! A bake run outside Cargo is told its target by `TARGET` alone
//! (`exact-js-bake --cargo`), and a module binding answers `supports` for
//! that target's OS: false on tvOS, which none of `@exact/clipboard`'s
//! implementations names, the declared types on iOS (LLP 1133.006 D7). The
//! binding goes through `tsc` and Rolldown as an app's does.
use exact_js_bake::{bake, Tools};
use std::path::{Path, PathBuf};

const CONTRACT: &str = r#"use ClipboardItem from "@exact/clipboard"

component App
  resource message = message() as shape string
  view
    text message testId="message"
"#;
const SOURCE: &str = r#"
import type { Sources, Answer } from './app.contract.d.ts';
import { ClipboardItem } from '@exact/clipboard';
export const appId = 'test.exact.binding';
export const grants = '';
const sources: Sources = { message: () => `png ${ClipboardItem.supports('image/png')}` };
export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
"#;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
#[test]
fn a_bake_told_only_its_target_answers_supports_for_it() {
    let f =
        Fixture(std::env::temp_dir().join(format!("exact-binding-target-{}", std::process::id())));
    let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/clipboard");
    std::fs::create_dir_all(f.0.join("node_modules/@exact")).unwrap();
    std::os::unix::fs::symlink(
        package.canonicalize().unwrap(),
        f.0.join("node_modules/@exact/clipboard"),
    )
    .unwrap();
    std::fs::write(f.0.join("app.contract"), CONTRACT).unwrap();
    std::fs::write(f.0.join("app.ts"), SOURCE).unwrap();
    assert!(std::env::var_os("CARGO_CFG_TARGET_OS").is_none());
    let script = |target: &str| {
        std::env::set_var("TARGET", target);
        let baked = bake(&f.0, &Tools::default());
        std::env::remove_var("TARGET");
        String::from_utf8(baked.unwrap().script).unwrap()
    };
    let declared = |s: &str| {
        s.contains(r#"["text/plain", "image/png"].includes"#)
            || s.contains(r#"["text/plain","image/png"].includes"#)
    };
    let tvos = script("aarch64-apple-tvos");
    assert!(
        tvos.contains("[].includes(type)") && !declared(&tvos),
        "tvOS:\n{tvos}"
    );
    let ios = script("aarch64-apple-ios-sim");
    assert!(declared(&ios), "iOS:\n{ios}");
}
