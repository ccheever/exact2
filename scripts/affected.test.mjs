import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { GLOBAL, Graph, Repo, auditProfile, cargoTests, commands, jsSources, rustLiterals, rustSource, select, suites } from './affected.mjs';
import { classesReached } from '../host/apple/xctest.mjs';

/** A repository of `files` (path → text) and Cargo packages (`{ name, dir, deps }`), with its metadata given. */
function fixture(files, packages) {
  const root = mkdtempSync(resolve(tmpdir(), 'affected-'));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(resolve(root, path)), { recursive: true });
    writeFileSync(resolve(root, path), text);
  }
  const id = name => `path+file://${name}`;
  const metadata = {
    workspace_members: packages.map(p => id(p.name)),
    packages: packages.map(p => ({ id: id(p.name), name: p.name, source: null, manifest_path: resolve(root, p.dir, 'Cargo.toml'), targets: [{ kind: ['lib'], test: true, src_path: resolve(root, p.dir, 'src/lib.rs') }], dependencies: [] })),
    resolve: { nodes: packages.map(p => ({ id: id(p.name), deps: (p.deps ?? []).map(d => ({ pkg: id(d), dep_kinds: [{ kind: null }] })) })) },
  };
  const all = { ...files, ...Object.fromEntries(packages.map(p => [`${p.dir}/Cargo.toml`, ''])), 'Cargo.toml': '' };
  const repo = new Repo(root, { files: Object.keys(all), metadata: { 'Cargo.toml': metadata } });
  return { root, repo, graph: new Graph(repo), done: () => rmSync(root, { recursive: true, force: true }) };
}

/** The Cargo and Bun suites `changed` selects (the fixed ones, the XCTests and the like, read a real checkout's hosts) */
const ids = (graph, changed) => select(graph, changed, suites(graph.repo).filter(s => s.kind === 'cargo' || s.kind === 'bun')).map(s => s.suite.id).sort();

test('Cargo, imports and the names a test gives select a suite; what shared code dispatches to does not', () => {
  const { graph, done } = fixture({
    'kernel/src/lib.rs': 'pub fn layout() {}\n#[cfg(test)]\nmod tests { const APP: &str = include_str!("../../apps/demo/app.contract"); }\n',
    'host/linux/src/lib.rs': '// A library names a host only to build for it.\nconst OTHER: &str = "host/apple/Sources/A.swift";\n',
    'host/linux/tests/it.rs': 'fn fixture() { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/other"); }\n',
    'host/apple/Sources/A.swift': '',
    'host/apple/build.mjs': "export const sources = 'Sources';\n",
    'scripts/app.mjs': "import '../host/apple/build.mjs';\nexport const hosts = ['host/apple', 'host/linux'];\n",
    'scripts/app.test.mjs': "import { hosts } from './app.mjs';\nimport { test } from 'bun:test';\ntest('x', () => resolveApp('demo'));\n",
    'apps/demo/app.contract': 'use Card from "./card.contract"\n',
    'apps/demo/card.contract': '',
    'apps/other/app.contract': '',
    'docs/guide.md': '',
  }, [{ name: 'exact-kernel', dir: 'kernel' }, { name: 'exact-linux', dir: 'host/linux', deps: ['exact-kernel'] }]);
  try {
    expect(ids(graph, ['kernel/src/lib.rs'])).toEqual(['cargo:exact-kernel', 'cargo:exact-linux']);
    // Only the tested package's tests are built: the kernel's include of the app is its own.
    expect(ids(graph, ['apps/demo/card.contract'])).toEqual(['bun:scripts/app.test.mjs', 'cargo:exact-kernel']);
    expect(ids(graph, ['apps/other/app.contract'])).toEqual(['cargo:exact-linux']);
    // The CLI imports the Apple build script, so its file is an input, but not the sources it builds from.
    expect(ids(graph, ['host/apple/build.mjs'])).toEqual(['bun:scripts/app.test.mjs']);
    expect(ids(graph, ['host/apple/Sources/A.swift'])).toEqual([]);
    expect(ids(graph, ['docs/guide.md'])).toEqual([]);
    expect(ids(graph, [GLOBAL[1]])).toEqual(['bun:scripts/app.test.mjs', 'cargo:exact-kernel', 'cargo:exact-linux']);
  } finally { done(); }
});

test('a suite names what it reads by path, flag, call or property, never by prose or an attribute', () => {
  const rust = rustLiterals([
    '#[ignore = "async lane: bun scripts/async.mjs runs it"]',
    'let a = repo().join("apps"); let b = dir.path().join("host");',
    'Command::new("cargo").args(["test", "-p", "exact-linux"]);',
    'if kind == "bench" {} let c = format!("../../apps/{app}/app.contract");',
  ].join('\n'));
  expect(rust.map(l => l.text)).toEqual(['apps', 'host', 'cargo', 'test', '-p', 'exact-linux', 'bench', '../../apps/']);
  expect(rust.find(l => l.text === 'apps').receiver).toBe('repo()');
  expect(rust.find(l => l.text === 'exact-linux').prev).toBe('-p');
  expect(rust.find(l => l.text === 'bench').compared).toBe(true);
  const { literals, imports } = jsSources("import a from './a.mjs'; resolve(import.meta.dir, '..', 'apps'); spawn('bun', ['scripts/agent.mjs', 'web', '--app', 'excord']); const t = `scripts/fixtures/${n}.contract`; process.env.EXACT_WEB_DIST;", 'x.mjs');
  expect(imports).toEqual(['./a.mjs']);
  expect(literals.map(l => l.text)).toEqual(expect.arrayContaining(['../apps', 'excord', 'scripts/fixtures/', 'EXACT_WEB_DIST']));
  expect(literals.find(l => l.text === 'excord').prev).toBe('--app');

  const { graph, done } = fixture({
    'scripts/a.test.mjs': "import { test } from 'bun:test';\ntest('a', () => { throw new Error('see apps/demo/app.contract, then retry: bun scripts/x.mjs'); });\n",
    'scripts/b.test.mjs': "import { test } from 'bun:test';\ntest('b', () => run(['scripts/agent.mjs', 'web', '--app', 'demo']));\n",
    'scripts/c.test.mjs': "import { test } from 'bun:test';\nimport { pick } from './pick.mjs';\ntest('c', () => pick());\n",
    // Shared code's default is read by every caller that names nothing: the app's files and its same-named crate.
    'scripts/pick.mjs': "export const pick = given => given ?? 'demo';\n", 'apps/demo/app.json': '{}',
    'scripts/agent.mjs': '', 'host/web/glue.js': '', 'host/web/tests/own.test.mjs': '',
    'apps/demo/app.contract': '',
  }, [{ name: 'demo', dir: 'apps/demo' }]);
  try {
    // (`--app demo` is the app, crate included, though a crate has its name; its own tests read its directory.)
    expect(ids(graph, ['apps/demo/app.contract'])).toEqual(['bun:scripts/b.test.mjs', 'bun:scripts/c.test.mjs', 'cargo:demo']);
    expect(ids(graph, ['apps/demo/app.json'])).toEqual(['bun:scripts/b.test.mjs', 'bun:scripts/c.test.mjs', 'cargo:demo']);
    // Naming the web platform reads its hosts, not their tests.
    expect(ids(graph, ['host/web/glue.js'])).toEqual(['bun:scripts/b.test.mjs']);
  } finally { done(); }
});

test("a script beside the code is named by its file's name, and one that checks the whole checkout reads all of it", () => {
  const { graph, done } = fixture({
    'scripts/near.test.mjs': "import { test } from 'bun:test';\nimport { join } from 'node:path';\ntest('n', () => spawnSync(process.execPath, [join(import.meta.dir, 'tool.mjs')]));\n",
    'scripts/whole.test.mjs': "import { test } from 'bun:test';\ntest('w', () => spawnSync(process.execPath, [resolve(ROOT, 'scripts/caps.mjs')]));\n",
    'scripts/tool.mjs': '', 'scripts/caps.mjs': '', 'apps/demo/app.contract': '',
  }, []);
  try {
    expect(ids(graph, ['scripts/tool.mjs'])).toEqual(['bun:scripts/near.test.mjs', 'bun:scripts/whole.test.mjs']);
    expect(ids(graph, ['apps/demo/app.contract'])).toEqual(['bun:scripts/whole.test.mjs']);
  } finally { done(); }
});

test("Rust's test modules, #[path] and include_str! are what they say; a deleted file still selects what named it", () => {
  const src = '#[path = "../shared.rs"]\nmod shared;\n#[ignore = "async lane: bun scripts/x.mjs runs it"]\nfn a() {}\n#[cfg(test)]\nmod tests { const F: &str = "fixtures/a.txt"; const B: &str = "}"; }\nconst S: &str = include_str!("shipped.txt");\n#[cfg(test)]\nmod more_tests;\n';
  const { library, test: tests, mods } = rustSource(src);
  expect([library.map(l => l.text), tests.map(l => l.text), mods]).toEqual([['../shared.rs', 'shipped.txt'], ['fixtures/a.txt', '}'], [{ name: 'shared', path: '../shared.rs', test: false }, { name: 'more_tests', path: null, test: true }]]);
  // An attribute on a field ends with the field, and one on a macro's call or a block with its braces, not later.
  for (const before of ['struct S { #[cfg(test)] probe: u32 }', '#[cfg(test)]\nthread_local! { static T: u8 = 0; }', '#[cfg(test)]\n{ let a = 1; }']) {
    const later = rustSource(`${before}\npub fn t() -> &'static str { include_str!("../x.json") }\n`);
    expect([later.library.map(l => l.text), later.test]).toEqual([['../x.json'], []]);
  }
  const { graph, done } = fixture({
    'core/src/lib.rs': 'mod test_steps;\npub mod model;\nmod presenter;\n#[cfg(test)]\nmod more_tests;\n',
    // (A library module's directory holds its own test modules: those stay test code.)
    'core/src/presenter.rs': '#[cfg(test)]\nmod presenter_tests;\n', 'core/src/presenter/presenter_tests.rs': '', 'core/src/test_steps.rs': '', 'core/src/more_tests.rs': 'mod helper;\n',
    'core/src/more_tests/helper.rs': '', 'core/src/model.rs': '', 'core/tests/it.rs': '#[path = "../src/model.rs"]\nmod model;\n',
    'user/src/lib.rs': '', 'scripts/gone.test.mjs': "import { test } from 'bun:test';\ntest('g', () => readFileSync(resolve(ROOT, 'scripts/fixtures/gone.contract')));\n",
  }, [{ name: 'core', dir: 'core' }, { name: 'user', dir: 'user', deps: ['core'] }]);
  try {
    // A module only tests declare is not a dependent's input; one the library declares is, whatever its name.
    expect(ids(graph, ['core/src/more_tests.rs'])).toEqual(['cargo:core']);
    expect(ids(graph, ['core/src/more_tests/helper.rs'])).toEqual(['cargo:core']);
    // (A library module an integration test includes too is still the library's.)
    expect(ids(graph, ['core/src/model.rs'])).toEqual(['cargo:core', 'cargo:user']);
    expect(ids(graph, ['core/src/presenter/presenter_tests.rs'])).toEqual(['cargo:core']);
    expect(ids(graph, ['core/src/test_steps.rs'])).toEqual(['cargo:core', 'cargo:user']);
  } finally { done(); }
  const deleted = fixture({ 'scripts/gone.test.mjs': "import { test } from 'bun:test';\ntest('g', () => readFileSync(resolve(ROOT, 'scripts/fixtures/gone.contract')));\n" }, []);
  try {
    const repo = new Repo(deleted.root, { files: deleted.repo.files, metadata: { 'Cargo.toml': { workspace_members: [], packages: [], resolve: { nodes: [] } } }, also: ['scripts/fixtures/gone.contract'] });
    expect(ids(new Graph(repo), ['scripts/fixtures/gone.contract'])).toEqual(['bun:scripts/gone.test.mjs']);
  } finally { deleted.done(); }
});

test('a path that starts with the code\'s own directory, its package\'s or the root, as a template or a format string, is read from there', () => {
  const { graph, done } = fixture({
    'apps/demo/fixture/media.test.ts': "import { test } from 'bun:test';\ntest('x', () => Bun.file(`${import.meta.dir}/../assets/${'cafe'}.png`));\n",
    'apps/demo/assets/cafe.png': '',
    'scripts/guide.test.mjs': "import { test } from 'bun:test';\nimport { resolve } from 'node:path';\nconst ROOT = resolve(import.meta.dir, '..');\ntest('x', () => [Bun.file(`${ROOT}/docs/guide.md`), Bun.file(`${ROOT}/QUEUE.md`)]);\n",
    'docs/guide.md': '', 'QUEUE.md': '', 'scripts/docs/guide.md': '',
    // Bind `ROOT` to a string, a placeholder that names no checkout
    'scripts/placeholder.test.mjs': "import { test } from 'bun:test';\nconst ROOT = '/x/exact2';\ntest('x', () => describe({ dir: `${ROOT}/docs` }));\n",
    // Bind it to a template instead, which names the checkout and is no placeholder
    'scripts/alias.test.mjs': "import { test } from 'bun:test';\nconst ROOT = `${import.meta.dir}/..`;\ntest('x', () => Bun.file(`${ROOT}/docs/guide.md`));\n",
    'apps/demo/fixture/own.test.ts': "import { test } from 'bun:test';\ntest('x', () => [Bun.file(`${import.meta.dir}/docs/guide.md`), Bun.file(`${import.meta.dir}/QUEUE.md`)]);\n",
    'apps/demo/fixture/docs/guide.md': '',
    'tools/contract/tests/it.rs': 'fn corpus(name: &str) -> String { std::fs::read_to_string(format!("{}/../corpus/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap() }\n'
      + 'fn named() -> String { format!("{dir}/../named/a.txt", dir = env!("CARGO_MANIFEST_DIR")) }\n'
      + 'const SCHEMA: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../tables/schema.json"));\n',
    // Count a library's include of another unit's file through `concat!`, as through a plain path
    'tools/contract/src/lib.rs': 'pub const DATA: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../host/linux/data.txt"));\n',
    'tools/named/a.txt': '', 'tools/tables/schema.json': '', 'host/linux/data.txt': '',
    'tools/corpus/a.contract': '',
    'tools/contract/corpus/a.contract': '',
  }, [{ name: 'contract', dir: 'tools/contract' }]);
  try {
    expect(ids(graph, ['apps/demo/assets/cafe.png'])).toEqual(['bun:apps/demo/fixture/media.test.ts']);
    // Read each from its own base alone (the root's `docs/guide.md`, the test's own), never another's; a placeholder
    // root reads nothing
    expect(ids(graph, ['docs/guide.md'])).toEqual(['bun:scripts/alias.test.mjs', 'bun:scripts/guide.test.mjs']);
    expect(ids(graph, ['scripts/docs/guide.md'])).toEqual([]);
    expect(ids(graph, ['apps/demo/fixture/docs/guide.md'])).toEqual(['bun:apps/demo/fixture/own.test.ts']);
    expect(ids(graph, ['QUEUE.md'])).toEqual(['bun:scripts/guide.test.mjs']);
    // Resolve from the package's directory, not the test's: `tools/corpus`, not the package's own `corpus`
    expect(ids(graph, ['tools/corpus/a.contract'])).toEqual(['cargo:contract']);
    expect(ids(graph, ['tools/named/a.txt'])).toEqual(['cargo:contract']);
    expect(ids(graph, ['tools/tables/schema.json'])).toEqual(['cargo:contract']);
    expect(ids(graph, ['host/linux/data.txt'])).toEqual(['cargo:contract']);
  } finally { done(); }
});

test("an app's tests read every crate its build compiles for their host, and its crates' build scripts read the app", () => {
  const { repo, graph, done } = fixture({
    'apps/demo/app.test.contract': '', 'apps/demo/app.json': '{}', 'apps/demo/app.contract': '',
    'apps/demo/web/src/lib.rs': '', 'apps/demo/apple/src/lib.rs': '', 'apps/demo/logic/src/lib.rs': '',
    'apps/demo/linux/src/lib.rs': '', 'apps/demo/linux/build.rs': 'fn main() { println!("cargo:rerun-if-changed=../app.json"); }\n',
    'apps/demo/data/src/lib.rs': '', 'apps/demo/data/build.rs': 'fn main() { let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".."); }\n',
    // Add a crate one level further in, as Harmony's `app/<host>` is, whose `..` is that part of the app
    'apps/demo/app/shell.contract': '', 'apps/demo/app/web/src/lib.rs': '', 'apps/demo/app/web/build.rs': 'fn main() { let app = std::path::Path::new(".."); }\n',
  }, [...['web', 'apple', 'logic', 'linux', 'data'].map(d => ({ name: `demo-${d}`, dir: `apps/demo/${d}` })), { name: 'demo-app-web', dir: 'apps/demo/app/web' }]);
  try {
    const apps = changed => select(graph, changed, suites(repo).filter(s => s.kind === 'app')).map(s => s.suite.id).sort();
    // Read every crate in the app's directory, a logic crate here, on every host, but not another platform's host crate
    expect(apps(['apps/demo/logic/src/lib.rs'])).toEqual(['app:demo:ios', 'app:demo:linux', 'app:demo:macos', 'app:demo:web']);
    expect(apps(['apps/demo/apple/src/lib.rs'])).toEqual(['app:demo:ios', 'app:demo:macos']);
    expect(ids(graph, ['apps/demo/app.json'])).toEqual(['cargo:demo-data', 'cargo:demo-linux']);
    expect(ids(graph, ['apps/demo/app.contract'])).toEqual(['cargo:demo-data']);
    expect(ids(graph, ['apps/demo/app/shell.contract'])).toEqual(['cargo:demo-app-web', 'cargo:demo-data']);
  } finally { done(); }
});

// What the lane runs a selection's Rust tests by gives each test binary what Cargo gives it.
test('a selected package\'s tests run as Cargo runs them, and a failure is said as Cargo says it', () => {
  const root = mkdtempSync(resolve(tmpdir(), 'affected-cargo-'));
  try {
    const crate = (name, failing) => {
      mkdirSync(resolve(root, name, 'src'), { recursive: true });
      writeFileSync(resolve(root, name, 'Cargo.toml'), `[package]\nname = "${name}"\nversion = "1.2.3-pre"\nedition = "2021"\ndescription = "d"\n`);
      const keep = `let mut v: Vec<_> = std::env::vars().filter(|(k, _)| k.starts_with("CARGO") && !k.starts_with("CARGO_HOME") || k.ends_with("LIBRARY_PATH") || k == "OUT_DIR" || k == "PICKED_SET").collect(); v.sort(); v.push(("cwd".into(), std::env::current_dir().unwrap().display().to_string())); std::fs::write(std::env::var("SEEN").unwrap(), format!("{v:?}")).unwrap();`;
      writeFileSync(resolve(root, name, 'src/lib.rs'), `#[cfg(test)]\nmod t { #[test] fn seen() { ${keep} } #[test] fn sure() { assert!(${!failing}); } }\n`);
    };
    crate('picked', false); crate('broken', true);
    // Give `picked` a build script, whose `OUT_DIR` and variables Cargo gives its tests, but for one of Cargo's own
    // (`CARGO_PKG_DESCRIPTION`), where Cargo's value wins
    writeFileSync(resolve(root, 'picked', 'build.rs'), 'fn main() { println!("cargo:rustc-env=PICKED_SET=1"); println!("cargo:rustc-env=CARGO_PKG_DESCRIPTION=from-script"); }\n');
    writeFileSync(resolve(root, 'Cargo.toml'), '[workspace]\nmembers = ["picked", "broken"]\nresolver = "2"\n');
    copyFileSync(resolve(import.meta.dir, '../rust-toolchain.toml'), resolve(root, 'rust-toolchain.toml'));
    const seen = resolve(root, 'seen'), env = { ...process.env, SEEN: seen };
    const cargo = spawnSync('cargo', ['test', '-q', '-p', 'picked', '--', 'seen'], { cwd: root, env, encoding: 'utf8' });
    expect(cargo.status, cargo.stderr).toBe(0);
    const want = readFileSync(seen, 'utf8');
    const run = spawnSync(process.execPath, [resolve(import.meta.dir, 'affected.mjs'), '--cargo-tests', 'picked', '--', 'seen'], { cwd: root, env, encoding: 'utf8' });
    expect(run.status, run.stderr).toBe(0);
    expect(readFileSync(seen, 'utf8')).toBe(want);
    const failing = spawnSync(process.execPath, [resolve(import.meta.dir, 'affected.mjs'), '--cargo-tests', 'picked', 'broken'], { cwd: root, env, encoding: 'utf8' });
    expect(failing.status).toBe(101);
    expect(failing.stdout).toContain('error: test failed, to rerun pass `-p broken --lib`');
    expect(failing.stdout).not.toContain('`-p picked --lib`');
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 300_000);

test('an environment variable a suite reads stands for the build it names', () => {
  const { repo, done } = fixture({
    'scripts/c.test.mjs': "import { test } from 'bun:test';\ntest('c', () => open(process.env.EXACT_WEB_DIST));\n",
    'apps/demo/app.contract': '',
  }, []);
  try {
    expect(ids(new Graph(repo, { env: { EXACT_WEB_DIST: ['appsrc:apps/demo'] } }), ['apps/demo/app.contract'])).toEqual(['bun:scripts/c.test.mjs']);
    expect(ids(new Graph(repo), ['apps/demo/app.contract'])).toEqual([]);
  } finally { done(); }
});

test("a change to Swift test files alone reaches their classes and those of the files that name them, per platform", () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'affected-xctest-'));
  const files = {
    'A.swift': 'import XCTest\n\nfinal class ATests: XCTestCase {\n    static let shared = 1\n}\n',
    'Uses.swift': 'final class UsesTests: XCTestCase {\n    func testIt() { XCTAssertEqual(ATests.shared, 1) }\n}\n',
    'Hidden.swift': 'private extension BTests {\n    func testHidden() {}\n}\n',
    'AMore.swift': 'extension ATests {\n    func more() {}\n}\n',
    'Base.swift': 'class BaseFixture: XCTestCase {\n    func helper() {}\n}\n',
    'B.swift': '@MainActor\nfinal class BTests: BaseFixture {}\n',
    'Mac.swift': '#if os(macOS)\nfinal class MacTests: XCTestCase {}\n#endif\n',
    'Private.swift': 'final class PTests: XCTestCase {}\nprivate final class Probe: NSObject {}\nprivate func probe() {}\n',
    'Waits.swift': 'extension XCTestCase {\n    func wait() {}\n}\n',
    'Table.swift': 'enum Table {\n    static let rows = [1]\n}\nfinal class TTests: XCTestCase {}\n',
    'Presenter.swift': 'final class Presenter: UIViewController {}\nfinal class UTests: XCTestCase {}\n',
  };
  for (const [name, text] of Object.entries(files)) writeFileSync(resolve(dir, name), text);
  try {
    const reached = (platform, ...changed) => classesReached(dir, platform, changed);
    // A class reaches the classes of each file that names it: one that uses its members, a subclass, an extension
    expect(reached('iOS', 'A.swift')).toEqual(['ATests', 'UsesTests']);
    expect(reached('iOS', 'Uses.swift')).toEqual(['UsesTests']);
    expect(reached('iOS', 'AMore.swift')).toEqual(['ATests', 'UsesTests']);
    expect(reached('iOS', 'Base.swift')).toEqual(['BTests']);
    expect(reached('iOS', 'Mac.swift', 'B.swift')).toEqual(['BTests']);
    expect(reached('macOS', 'Mac.swift')).toEqual(['MacTests']);
    expect(reached('iOS', 'Mac.swift')).toEqual([]);
    // What a file keeps private reaches nothing else; what other files can use (an extension of XCTestCase, a type
    // that is no test class), or a file that is gone, can reach any class
    expect(reached('iOS', 'Private.swift')).toEqual(['PTests']);
    for (const changed of ['Waits.swift', 'Table.swift', 'Presenter.swift', 'Gone.swift']) expect(reached('iOS', 'A.swift', changed)).toBe(null);
    // A file that compiles something yet reaches no class runs every class, whose build shows its error
    expect(reached('iOS', 'Hidden.swift')).toBe(null);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('the commands run each workspace once, the Bun files together and every app test file', () => {
  const run = commands([
    { suite: { kind: 'cargo', name: 'a', manifest: 'Cargo.toml' } }, { suite: { kind: 'cargo', name: 'b', manifest: 'Cargo.toml' } },
    { suite: { kind: 'cargo', name: 'g', manifest: 'game/Cargo.toml' } }, { suite: { kind: 'bun', file: 'x.test.mjs' } },
    { suite: { kind: 'app', app: 'excord', host: 'web', files: ['apps/excord/app.test.contract'] } }, { suite: { kind: 'conform', app: 'caltrain' } },
    { suite: { kind: 'xctest', host: 'macos' } }, { suite: { kind: 'xctest', host: 'ios' }, only: ['ATests', 'BTests'] },
  ]);
  expect(run).toEqual([
    ['cargo', 'test', '--tests', '--no-fail-fast', '-p', 'a', '-p', 'b'],
    ['cargo', 'test', '--manifest-path', 'game/Cargo.toml', '--tests', '--no-fail-fast', '-p', 'g'],
    ['bun', 'test', '--isolate', './x.test.mjs'],
    ['bun', 'scripts/agent.mjs', 'web', '--app', 'excord', '--test', 'apps/excord/app.test.contract'],
    ['bun', 'host/apple/build.mjs', '--test'],
    ['bun', 'host/apple/build.mjs', '--test', '--ios', '--only', 'ATests,BTests'],
    ['bun', 'host/web-js/conform.mjs', 'caltrain', '--synthetic', '--build', '--linux'],
  ]);
});

test('an audit makes every unit outside the inputs unreadable, Cargo manifests aside', () => {
  const { repo, root, done } = fixture({ 'apps/demo/app.contract': '', 'apps/other/app.contract': '', 'host/apple/A.swift': '', 'kernel/lib.rs': '' }, []);
  try {
    const { denied, profile } = auditProfile(repo, new Set(['apps/demo/app.contract']));
    expect(denied).toEqual(['apps/other', 'host/apple']);
    expect(profile).toContain(`(subpath ${JSON.stringify(resolve(root, 'host/apple'))})`);
    expect(profile).toContain('(vnode-type REGULAR-FILE)');
    expect(profile).toContain('Cargo\\.toml');
  } finally { done(); }
});

test("the lane's steps read the dists they are given by variable, and no other step does", async () => {
  const { laneSelection } = await import('./async.mjs');
  const names = "import { test } from 'bun:test';\ntest('d', () => open(process.env.EXACT_WEB_DIST));\n";
  const { root, repo, done } = fixture({ 'scripts/agent.test.mjs': names, 'scripts/elsewhere.test.mjs': names, 'apps/caltrain/app.contract': '' }, []);
  try {
    const picked = laneSelection(['apps/caltrain/app.contract'], { root, repo });
    expect(picked.glue).toEqual(['./scripts/agent.test.mjs']);
    expect(picked.ids).not.toContain('bun:scripts/elsewhere.test.mjs');
  } finally { done(); }
});

test("the lane runs the iOS classes a commit's test files reach, and every class before `--only`", async () => {
  const { laneSelection } = await import('./async.mjs');
  const test = 'host/apple/tests/ExactKitTests/A.swift';
  for (const [build, only] of [["if (args.includes('--only')) {}\n", ['ATests']], ['// (no --only yet)\n', null]]) {
    const { root, repo, done } = fixture({ [test]: 'import XCTest\n\nfinal class ATests: XCTestCase {}\n', 'host/apple/build.mjs': build }, []);
    try {
      const picked = laneSelection([test], { root, repo });
      expect([picked.ios, picked.iosOnly, picked.touch]).toEqual([true, only, false]);
    } finally { done(); }
  }
});

// The checkout itself: a change to one app runs that app's suites, not another's, nor the hosts' own.
test("this checkout's selection keeps an app's change to that app's suites", () => {
  const repo = new Repo(), graph = new Graph(repo);
  const watched = ['app:excord:web', 'app:excord:ios', 'app:caltrain:web', 'xctest:ios', 'xctest:macos', 'touch:ios', 'conform:caltrain', 'cargo:exact-kernel'];
  const catalogue = suites(repo).filter(s => watched.includes(s.id));
  expect(catalogue.map(s => s.id).sort()).toEqual([...watched].sort());
  const picked = changed => select(graph, changed, catalogue).map(s => s.suite.id).sort();
  expect(picked(['apps/excord/render.ts'])).toEqual(['app:excord:ios', 'app:excord:web']);
  expect(picked(['host/apple/Sources/ExactKit/Mac/CaptureMac.swift'])).toEqual(['app:excord:ios', 'touch:ios', 'xctest:ios', 'xctest:macos']);
  // A change to an XCTest file runs the classes it reaches, each on its platform, and no Apple build
  // that does not run them
  const only = changed => Object.fromEntries(select(graph, changed, catalogue).map(s => [s.suite.id, s.only ?? 'every class']));
  expect(only(['host/apple/tests/ExactKitTests/AgentStickyRevealIOSTests.swift'])).toEqual({ 'xctest:ios': ['AgentStickyRevealIOSTests'] });
  expect(only(['host/apple/tests/ExactKitTests/EventWaits.swift'])).toEqual({ 'xctest:ios': 'every class', 'xctest:macos': 'every class' });
  expect(picked(['docs/testing.md'])).toEqual([]);
  expect(picked(['kernel/src/lib.rs'])).toEqual([...watched].sort());
}, 120_000);
