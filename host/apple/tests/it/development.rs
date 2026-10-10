//! The development client's two local entries (LLP 1030.000 §7): the
//! opening link — admitted only for the dev server a build was made against,
//! with that build's random token — and the local plan/module pair.

use std::path::Path;
use std::process::Command;

fn bun(script: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new("bun")
        .current_dir(root)
        .args(["--input-type=module", "-e", script])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[test]
#[ignore = "async lane: launches Bun; bun scripts/async.mjs runs it"]
fn development_opening_metadata_is_app_specific_and_opt_in() {
    bun(r#"
import assert from 'node:assert/strict';
import {mkdirSync, mkdtempSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {developmentURLScheme} from './scripts/app.mjs';
import {appleArtifacts, developmentAdmission, developmentLinks, infoPlist, macInfoPlist} from './host/apple/build.mjs';
// Moved beside the simulator and device helpers (28a2ee81d).
import {developmentLaunchEnvironment} from './host/apple/devices.mjs';
import {developmentOpenPage} from './host/web/serve.mjs';
const dir = mkdtempSync(join(tmpdir(), 'exact-development-link-'));
try {
const app = {id:'test.one', dir, target:join(dir, 'target'), displayName:'<img src=x onerror=alert(1)>', crate:()=>'<script>bad</script>', manifest:{host:{ios:{urlSchemes:['existing']}}}};
const scheme = developmentURLScheme(app.id);
assert.match(scheme, /^exact2-[0-9a-f]{32}$/);
assert.notEqual(scheme, developmentURLScheme('test.two'));
// A build given a dev server admits that origin alone, with a token of its own.
const admission = developmentAdmission(app, 'http://192.168.1.20:8765/deep?q=1');
assert.deepEqual([admission.scheme, admission.origins], [scheme, ['http://192.168.1.20:8765']]);
assert.match(admission.token, /^[0-9a-f]{64}$/);
assert.notEqual(developmentAdmission(app, 'http://192.168.1.20:8765/').token, admission.token);
for (const locator of [undefined, '', '/tmp/app.plan', 'file:///tmp/app.plan', 'javascript:alert(1)']) assert.equal(developmentAdmission(app, locator), null);
for (const make of [d=>infoPlist(app,false,{development:d}),d=>macInfoPlist(app,{development:d})]) {
  const on = make(admission), off = make(null);
  for (const text of [scheme, 'ExactDevelopmentURLScheme', 'ExactDevelopmentOrigins', 'http://192.168.1.20:8765', 'ExactDevelopmentToken', admission.token]) assert.ok(on.includes(text), text);
  assert.ok(!off.includes(scheme)); assert.ok(!off.includes('ExactDevelopment'));
}
assert.ok(infoPlist(app).includes('existing'));
// --url is a launch locator or a client bundle's dev server, never alone.
for (const flag of ['--run', '--bundle', '--ios', '--device']) assert.equal(developmentLaunchEnvironment([flag, '--url', 'http://192.168.1.20:8765'], {}).EXACT_DEV_PLAN, 'http://192.168.1.20:8765/');
assert.throws(() => developmentLaunchEnvironment(['--url', 'http://192.168.1.20:8765'], {}));
// The dev server's links come from the placed clients' receipts: that build's
// token, only for the origin it admits.
const resources = resolve(appleArtifacts(app, {destination:'macos', trust:'development'}).bundle, 'Contents/Resources');
mkdirSync(resources, {recursive:true});
writeFileSync(join(resources, 'receipt.json'), JSON.stringify({development:admission}));
const page = 'http://192.168.1.20:8765/deep?q=1#x';
assert.deepEqual(developmentLinks(app, page), [{destination:'macos', href:`${scheme}://open?url=${encodeURIComponent(page)}&token=${admission.token}`}]);
assert.deepEqual(developmentLinks(app, 'http://192.168.1.21:8765/'), []);
writeFileSync(join(resources, 'receipt.json'), JSON.stringify({development:null}));
assert.deepEqual(developmentLinks(app, page), []);
const html = developmentOpenPage(app);
assert.ok(!html.includes(app.displayName)); assert.ok(html.includes('&lt;img'));
assert.ok(!html.includes('<script>bad')); assert.ok(html.includes(scheme + '://open?url='));
assert.ok(html.includes('cannot detect whether a client is installed'));
} finally { rmSync(dir, {recursive:true, force:true}); }
"#);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "async lane: launches Bun and the Swift compiler; bun scripts/async.mjs runs it"]
fn swift_development_link_admits_only_its_dev_server_and_token() {
    // What build.mjs bakes and the link a dev server offers for that build,
    // read here exactly as a development bundle's Info.plist is.
    let produced = bun(r#"
import {mkdirSync, mkdtempSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {appleArtifacts, developmentAdmission, developmentLinks, macInfoPlist} from './host/apple/build.mjs';
const dir = mkdtempSync(join(tmpdir(), 'exact-development-link-'));
try {
  const app = {id:'test.one', dir, target:join(dir, 'target'), displayName:'One', manifest:{}};
  const admission = developmentAdmission(app, 'http://192.168.1.20:8765/');
  const resources = resolve(appleArtifacts(app, {destination:'macos', trust:'development'}).bundle, 'Contents/Resources');
  mkdirSync(resources, {recursive:true});
  writeFileSync(join(resources, 'receipt.json'), JSON.stringify({development:admission}));
  const page = 'http://192.168.1.20:8765/deep?q=x%26y#frag';
  process.stdout.write(JSON.stringify({plist:macInfoPlist(app, {development:admission}), link:developmentLinks(app, page)[0].href, page}));
} finally { rmSync(dir, {recursive:true, force:true}); }
"#);
    // Compile the actual Foundation-only opening code, not a parallel parser.
    let source = include_str!("../../Sources/ExactKit/PlanURL.swift")
        .split("public enum ExactDevelopmentLink")
        .nth(1)
        .unwrap()
        .split("/// One bounded HTTP rung.")
        .next()
        .unwrap();
    let checks = r#"
final class ExactApp {
    static let shared = ExactApp()
    func connect(_ url: String) { preconditionFailure("must not connect in a non-development bundle") }
}
let produced = try! JSONSerialization.jsonObject(with: Data(PRODUCED.utf8)) as! [String: String]
let info = try! PropertyListSerialization.propertyList(from: Data(produced["plist"]!.utf8), format: nil) as! [String: Any]
let scheme = info["ExactDevelopmentURLScheme"] as! String
let token = info["ExactDevelopmentToken"] as! String
let baked = info["ExactDevelopmentOrigins"] as! [String]
precondition(((info["CFBundleURLTypes"] as! [[String: Any]])[0]["CFBundleURLSchemes"] as! [String]) == [scheme])
precondition(ExactDevelopmentLink.page(URL(string: produced["link"]!)!, scheme: scheme, origins: baked, token: token)?.absoluteString == produced["page"]!)
let origins = baked + ["https://x.test"]
func admits(_ link: URL) -> String? { ExactDevelopmentLink.page(link, scheme: scheme, origins: origins, token: token)?.absoluteString }
func link(_ url: String?, _ offered: String?, extra: [URLQueryItem] = []) -> URL {
    var c = URLComponents(string: scheme + "://open")!
    c.queryItems = (url.map { [URLQueryItem(name: "url", value: $0)] } ?? []) + (offered.map { [URLQueryItem(name: "token", value: $0)] } ?? []) + extra
    return c.url!
}
// The announced origins with the token, in either order, default ports equal.
precondition(admits(link("http://192.168.1.20:8765/a?q=x%26y#hello", token)) == "http://192.168.1.20:8765/a?q=x%26y#hello")
precondition(admits(link("https://x.test:443/", token)) == "https://x.test:443/")
precondition(admits(URL(string: scheme + "://open?token=" + token + "&url=http%3A%2F%2F192.168.1.20%3A8765%2F")!) == "http://192.168.1.20:8765/")
// Any other origin, even with the token: host, port or scheme.
for other in ["https://evil.example/", "http://192.168.1.21:8765/", "http://192.168.1.20:8766/", "https://192.168.1.20:8765/", "http://x.test/", "https://x.test.evil.example/"] {
    precondition(admits(link(other, token)) == nil, other)
}
// A missing, wrong, short, uppercase, empty or repeated token.
for bad in [link("https://x.test/", nil), link("https://x.test/", String(repeating: "f", count: 64)),
            link("https://x.test/", String(token.dropLast())), link("https://x.test/", token.uppercased()), link("https://x.test/", ""),
            link("https://x.test/", token, extra: [URLQueryItem(name: "token", value: token)])] {
    precondition(admits(bad) == nil, bad.absoluteString)
}
// A build baked without a valid token or any origin admits nothing.
precondition(ExactDevelopmentLink.page(link("https://x.test/", token), scheme: scheme, origins: origins, token: "") == nil)
precondition(ExactDevelopmentLink.page(link("https://x.test/", ""), scheme: scheme, origins: origins, token: "") == nil)
precondition(ExactDevelopmentLink.page(link("https://x.test/", token), scheme: scheme, origins: [], token: token) == nil)
// Malformed links, each with the right token and an admitted origin.
for bad in ["other://open?url=https://x.test&token=T", "S://else?url=https://x.test&token=T",
 "S://open?url=file:///tmp/secret&token=T", "S://open?url=javascript:alert(1)&token=T",
 "S://open?url=https://u:p@x.test&token=T", "S://open?url=https://x.test&url=https://y.test&token=T",
 "S://open?url=https://x.test&other=1&token=T", "S://open?url=https://x.test&token=T#extra",
 "S://open/path?url=https://x.test&token=T", "S://u@open?url=https://x.test&token=T",
 "S://open:3?url=https://x.test&token=T", "S://open", "S://open?url=&token=T", "S://open?token=T"] {
 let spelled = bad.replacingOccurrences(of: "S://", with: scheme + "://").replacingOccurrences(of: "=T", with: "=" + token)
 precondition(admits(URL(string: spelled)!) == nil, spelled)
}
precondition(admits(URL(string: scheme + "://open?url=https://x.test&token=" + token)!) == "https://x.test")
precondition(admits(URL(string: scheme + "://open?token=" + token + "&url=https://x.test/" + String(repeating: "a", count: 8192))!) == nil)
// Outside a development bundle nothing is claimed, so nothing connects.
precondition(!ExactDevelopmentLink.claims(link("https://x.test/", token)) && !ExactDevelopmentLink.open(link("https://x.test/", token)))
"#
    .replace("PRODUCED", &format!("##\"{produced}\"##"));
    let result = Command::new("swift")
        .args([
            "-swift-version",
            "5",
            "-e",
            &format!("import Foundation\npublic enum ExactDevelopmentLink{source}\n{checks}"),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "async lane: launches the Swift compiler; bun scripts/async.mjs runs it"]
fn local_development_plan_loads_the_pair_and_retries_partial_writes() {
    let source = include_str!("../../Sources/ExactKit/PlanURL.swift")
        .split("/// Explicit development-only opening action")
        .next()
        .unwrap();
    let checks = r#"
struct ExactModule { let receipt: Data; let bytecode: Data }
struct ExactGeneration { let plan: Data; let assets: String; let module: ExactModule }
public final class ExactApp {
    let resolver = "assets"
    var mode = "wasm"
    var rustPolicy: (mode: String, target: String) { (mode, "test-target") }
    var plain = 0
    var generations: [ExactGeneration] = []
    func apply(_ bytes: Data, label: String) -> Bool { plain += 1; return true }
    func applyGeneration(_ g: ExactGeneration, label: String, commit: () -> Bool) -> Bool {
        precondition(commit()); generations.append(g); return true
    }
}
let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: directory) }
func write(_ name: String, _ value: String) throws {
    try Data(value.utf8).write(to: directory.appendingPathComponent(name), options: .atomic)
}
let app = ExactApp(), candidate = ExactDevelopmentPlan(directory.appendingPathComponent("app.plan").path)
try write("app.plan", "plan")
precondition(!candidate.hasModule && candidate.apply(to: app) && app.plain == 1)
let bare = candidate.revision
try write("app.module.json", "receipt")
let partial = candidate.revision
precondition(partial != bare && candidate.hasModule)
precondition(!candidate.apply(to: app) && app.plain == 1 && app.generations.isEmpty)
try write("app.hbc", "bytecode")
precondition(candidate.revision != partial && candidate.apply(to: app))
let pair = app.generations.last!
precondition(pair.plan == Data("plan".utf8) && pair.module.receipt == Data("receipt".utf8))
precondition(pair.module.bytecode == Data("bytecode".utf8))
let first = candidate.revision
try write("app.hbc", "replacement")
precondition(candidate.revision != first && candidate.apply(to: app))
precondition(app.generations.last!.module.bytecode == Data("replacement".utf8))
try FileManager.default.removeItem(at: directory.appendingPathComponent("app.module.json"))
precondition(!candidate.apply(to: app) && app.plain == 1 && app.generations.count == 2)
try write("app.module.json", String(repeating: "x", count: (1 << 20) + 1))
precondition(!candidate.apply(to: app) && app.generations.count == 2)
try FileManager.default.removeItem(at: directory.appendingPathComponent("app.module.json"))
try FileManager.default.removeItem(at: directory.appendingPathComponent("app.hbc"))
try FileManager.default.createDirectory(at: directory.appendingPathComponent("rust/wasm"), withIntermediateDirectories: true)
try write("rust/wasm/app.module.json", "{\"module\":{\"file\":\"app.module.wasm\"}}")
let partialRust = candidate.revision
precondition(candidate.hasModule && !candidate.apply(to: app))
try write("rust/wasm/app.module.wasm", "wasm")
precondition(candidate.revision != partialRust && candidate.apply(to: app))
precondition(app.generations.last!.module.bytecode == Data("wasm".utf8))
let rustOnly = app.generations.count; try write("app.module.json", "{\"kind\":\"javascript\",\"plan\":{\"file\":\"app.plan\"}}")
precondition(!candidate.apply(to: app) && app.generations.count == rustOnly)
try write("app.hbc", "hbc")
precondition(candidate.apply(to: app))
let mixed = app.generations.last!.module
let receipt = try JSONSerialization.jsonObject(with: mixed.receipt) as! [String: Any]
precondition(receipt["kind"] as? String == "mixed" && receipt["version"] as? Int == 1)
precondition(receipt["javascriptBytes"] as? Int == 3 && mixed.bytecode == Data("hbcwasm".utf8))
precondition((receipt["javascript"] as? [String: Any])?["kind"] as? String == "javascript")
precondition(((receipt["rust"] as? [String: Any])?["module"] as? [String: Any])?["file"] as? String == "app.module.wasm")
let both = app.generations.count; try FileManager.default.removeItem(at: directory.appendingPathComponent("rust/wasm/app.module.wasm"))
precondition(!candidate.apply(to: app) && app.generations.count == both)
try write("rust/wasm/app.module.wasm", "wasm"); try write("app.module.json", "not JSON")
precondition(!candidate.apply(to: app) && app.generations.count == both)
app.mode = "off"; precondition(candidate.apply(to: app) && app.generations.last!.module.bytecode == Data("hbc".utf8))
"#;
    let result = Command::new("swift")
        .args(["-swift-version", "5", "-e", &format!("{source}\n{checks}")])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "async lane: launches Bun; bun scripts/async.mjs runs it"]
fn a_build_is_stamped_with_its_time_commits_and_kind() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new("bun")
        .current_dir(root)
        .args(["test", "./host/apple/buildinfo.test.mjs"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
