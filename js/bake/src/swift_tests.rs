use super::*;
use crate::Scratch;
use std::fs;

#[test]
fn fingerprints_cover_order_contents_missing_inputs_and_toolchain() {
    let dir = Scratch::new(&std::env::temp_dir()).unwrap();
    let a = dir.0.join("a.swift");
    let b = dir.0.join("b.swift");
    fs::write(&a, "first").unwrap();
    fs::write(&b, "second").unwrap();
    let inputs = [a.clone(), b.clone()];
    let original = fingerprint(&inputs, &["swiftc v1", "sdk 1"]).unwrap();
    assert_ne!(
        original,
        fingerprint(&[b.clone(), a.clone()], &["swiftc v1", "sdk 1"]).unwrap()
    );
    assert_ne!(
        original,
        fingerprint(std::slice::from_ref(&a), &["swiftc v1", "sdk 1"]).unwrap()
    );
    assert_ne!(
        original,
        fingerprint(&inputs, &["swiftc v2", "sdk 1"]).unwrap()
    );
    assert_ne!(
        original,
        fingerprint(&inputs, &["swiftc v1", "sdk 2"]).unwrap()
    );
    fs::write(&a, "edited").unwrap();
    fs::File::open(&a)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH))
        .unwrap();
    assert_ne!(
        original,
        fingerprint(&inputs, &["swiftc v1", "sdk 1"]).unwrap()
    );
    fs::remove_file(&b).unwrap();
    assert!(fingerprint(&inputs, &["swiftc v1"])
        .unwrap_err()
        .contains("b.swift"));
    assert_ne!(names("one-app"), names("one_app"));
}

#[cfg(target_os = "macos")]
#[test]
fn swift_archives_rebuild_and_two_apps_link_and_run_together() {
    let dir = Scratch::new(&std::env::temp_dir()).unwrap();
    let source = dir.0.join("Module.swift");
    let extra = dir.0.join("Extra.swift");
    let out = dir.0.join("out");
    fs::write(
        &source,
        r#"
import Foundation
final class Module: ExactNativeModule {
  let grants: String
  init(_ grants: String) { self.grants = grants }
  func call(_ request: [String: Any]) throws -> [String: Any] { ["grants": grants, "revision": 1] }
}
func exactNativeModule(grants: String) -> ExactNativeModule { Module(grants) }
"#,
    )
    .unwrap();
    fs::write(
        &extra,
        "@_cdecl(\"extra_symbol\") public func extraSymbol() -> Int32 { 7 }\n",
    )
    .unwrap();
    let mut tool = toolchain("macosx").unwrap();
    let triple = if cfg!(target_arch = "aarch64") {
        "arm64-apple-macos14.0"
    } else {
        "x86_64-apple-macos14.0"
    };
    let inputs = vec![source.clone(), extra.clone(), bridge()];
    let a = compile(&inputs, &out, triple, "one-app", &tool).unwrap();
    let (_, prefix_a) = names("one-app");
    let stamp = out.join(format!("{prefix_a}.sha256"));
    let first = fs::read(&stamp).unwrap();
    let old = fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH);
    fs::File::open(&a).unwrap().set_times(old).unwrap();
    compile(&inputs, &out, triple, "one-app", &tool).unwrap();
    assert_eq!(
        fs::metadata(&a).unwrap().modified().unwrap(),
        std::time::UNIX_EPOCH,
        "an identical build reuses the archive"
    );
    let fewer = [source.clone(), bridge()];
    compile(&fewer, &out, triple, "one-app", &tool).unwrap();
    assert_ne!(
        first,
        fs::read(&stamp).unwrap(),
        "removing a source rebuilds"
    );
    let removed = fs::read(&a).unwrap();
    fs::File::open(&extra).unwrap().set_times(old).unwrap();
    compile(&inputs, &out, triple, "one-app", &tool).unwrap();
    assert_ne!(
        removed,
        fs::read(&a).unwrap(),
        "an old-dated added source rebuilds"
    );
    fs::remove_file(&extra).unwrap();
    assert!(compile(&inputs, &out, triple, "one-app", &tool)
        .unwrap_err()
        .contains("Extra.swift"));
    let revised = fs::read_to_string(&source)
        .unwrap()
        .replace("\"revision\": 1", "\"revision\": 2");
    fs::write(&source, revised).unwrap();
    fs::File::open(&source).unwrap().set_times(old).unwrap();
    compile(&fewer, &out, triple, "one-app", &tool).unwrap();
    fs::File::open(&a).unwrap().set_times(old).unwrap();
    tool.version.push_str(" (toolchain upgrade)");
    compile(&fewer, &out, triple, "one-app", &tool).unwrap();
    assert_ne!(
        fs::metadata(&a).unwrap().modified().unwrap(),
        std::time::UNIX_EPOCH,
        "a new compiler version rebuilds"
    );
    let b = compile(&fewer, &out, triple, "one_app", &tool).unwrap();
    let (_, prefix_b) = names("one_app");
    let declarations = |p: &str| {
        format!(
            r#"
extern void *{p}_create(const unsigned char *, long);
extern char *{p}_call(void *, const char *, int *);
extern void {p}_free(char *);
extern void {p}_destroy(void *);
"#
        )
    };
    let client = dir.0.join("client.c");
    fs::write(
        &client,
        format!(
            r#"
#include <string.h>
{} {}
int main(void) {{
  void *a = {prefix_a}_create((const unsigned char *)"alpha", 5);
  void *b = {prefix_b}_create((const unsigned char *)"beta", 4);
  int failed_a = 0, failed_b = 0;
  char *ra = {prefix_a}_call(a, "{{}}", &failed_a);
  char *rb = {prefix_b}_call(b, "{{}}", &failed_b);
  int ok = !failed_a && !failed_b && strstr(ra, "alpha") && strstr(rb, "beta") && strstr(ra, "2");
  {prefix_a}_free(ra); {prefix_b}_free(rb);
  {prefix_a}_destroy(a); {prefix_b}_destroy(b);
  return ok ? 0 : 1;
}}
"#,
            declarations(&prefix_a),
            declarations(&prefix_b)
        ),
    )
    .unwrap();
    let object = dir.0.join("client.o");
    let built = Command::new("xcrun")
        .args(["clang", "-c"])
        .arg(&client)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let executable = dir.0.join("client");
    let linked = Command::new(&tool.compiler)
        .args(["-sdk", &tool.sdk, "-target", triple])
        .args([&object, &a, &b])
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    assert!(Command::new(executable).status().unwrap().success());
}
