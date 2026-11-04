#!/usr/bin/env bun
/**
 * affected — the test suites a change can affect, for the async lane and for a branch.
 *
 * A suite runs when a change touches one of its inputs, the files it can read. They are found by following, from the
 * suite's roots:
 *   - Cargo's dependencies (`cargo metadata`): the tested package's files and dev-dependencies, and each normal or
 *     build dependency's Rust and manifest, without its test modules;
 *   - imports: JavaScript's and TypeScript's (Rolldown's parser) and Contract's `use … from`;
 *   - what code names in a string: a path (from the code, its package or the root), or a package or an app given by
 *     name (after `-p` or `--app`, or to a call or property that takes one). `references`, `jsSources` and
 *     `rustSource` say which forms are read: templates, format strings, globs, `#[path]`, `include_str!`, and a
 *     build script's declared paths. A Swift test's strings count too.
 * The repository divides into units: each platform's hosts, each app, each package, `bench`, `game` and the
 * documentation; the rest is the shared core. A test or a build script reads what it names. Other code that names an
 * app, a package or another platform's host does so to build or drive what its caller asks for (the CLI builds any app
 * for any host), so in it only a path into one app or package counts; and a module that another unit imports is
 * loaded, not driven: its imports count, and of its names only those its top level reads. `GLOBAL` lists the files
 * every suite reads, and `suites` lists by hand the roots of the XCTests, touch smoke, Apple compiles, Harmony and
 * conformance, from the scripts that run them. Code other than a test names a directory of tests to run the tests
 * there, and the suite that runs them has that directory among its roots, so in such code the name does not count; a
 * file there that it names (a fixture, a script) does. An XCTest suite selected by its own Swift files alone runs only
 * the classes they reach (`xctestClasses`).
 *
 * It reads the source, so a read that nothing names (a walk of the whole repository) escapes it; the async lane runs
 * every step in full at least daily (`scripts/async.mjs`), and `--audit` runs a suite with the units outside its
 * inputs unreadable.
 *
 *   bun scripts/affected.mjs [--base <rev>] [--why <suite>] [--json] [--run] [--audit <suite>]
 *     --base    the commit to compare with (default: the merge base with origin/main); the working tree's changes,
 *               untracked files included, count too
 *     --why     each changed input that selects the suite, with the chain of references that reaches it, and the
 *               XCTest classes it runs alone
 *     --json    the changed files, the suites selected (each with up to five of the files that select it, and the
 *               XCTest classes it runs alone) and the commands, as one object
 *     --run     run the selected Cargo packages' and Bun files' tests (the other suites are listed with their commands)
 *     --audit   run a suite under `sandbox-exec` with the files of every unit outside its inputs unreadable (macOS; a
 *               sandboxed process cannot run a setuid program such as `ps`, which `scripts/test-children.mjs` and
 *               `scripts/verify.mjs` use)
 *   bun scripts/affected.mjs --cargo-tests [--manifest <Cargo.toml>] <package>… [-- <test args>]
 *               (the async lane's) the workspace's tests built as a full run builds them, those packages' run
 */
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, normalize, relative, resolve } from 'node:path';
import { parseAst } from 'rolldown/parseAst';
import { classesReached } from '../host/apple/xctest.mjs';

const ROOT = resolve(import.meta.dir, '..');

/** Files every suite reads: the workspace's manifest and lock, the toolchain pin, Cargo's and Bun's configuration and
 * Bun's manifest and lock. */
export const GLOBAL = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml', 'package.json', 'bun.lock', 'bunfig.toml'];

/** The apps the async lane's conformance steps drive (`host/web-js/conform.mjs`). */
export const WEB_APPS = ['realworld', 'weatherlight', 'completion-storm', 'video-player', 'caltrain', 'typetour', 'carousel', 'sparkline', 'svg-gallery', 'spark', 'markdown-stress', 'reflow', 'textflow', 'canvas-gallery', 'duo-lab', 'update-lab', 'native-fixture', 'photo-editor', 'recorder', 'fieldnotes', 'markdown', 'messages', 'interaction-gallery', 'motion-gallery'];

/** The web builds the async lane's prepare step makes (`dist`, under its dists directory), each named to the steps that
 * read it by an environment variable. A suite of such a step that names the variable reads that app's web build. */
export const LANE_DISTS = [
  ...['EXACT_WEB_DIST', 'EXACT_NO_GPU_WASM_DIST', 'EXACT_MOTION_DIST'].map(name => ({ name, dist: 'caltrain-wasm', app: 'caltrain', steps: ['glue', 'web-ax-test'] })),
  ...['EXACT_WEB_JS_DIST', 'EXACT_NO_GPU_JS_DIST'].map(name => ({ name, dist: 'caltrain-js', app: 'caltrain', steps: ['glue', 'web-ax-test'] })),
  { name: 'EXACT_FIELDNOTES_DIST', dist: 'fieldnotes', app: 'fieldnotes', steps: ['test'] },
];
/** The variables a lane step sets (`LANE_DISTS`), each with the roots of the web build it names. */
export const stepEnv = step => Object.fromEntries(LANE_DISTS.filter(d => d.steps.includes(step))
  .map(d => [d.name, ['js:host/web/build.mjs', `appsrc:apps/${d.app}`, `pkg:${d.app}-web`, 'platform:web', 'pkg:exact-web-js']]));

/** Scripts that check every file of the checkout by design: a suite that runs one reads them all. */
const WHOLE = new Set(['scripts/caps.mjs', 'scripts/metrics.mjs']);

/** The Cargo workspaces whose packages' tests run: the root's, and the two kept apart from it. */
const WORKSPACES = ['Cargo.toml', 'game/Cargo.toml', 'apps/harmony/app/Cargo.toml'];

/** The hosts an app's crate directory serves; the directory is the platform's name. */
const APP_HOSTS = { web: ['web'], apple: ['macos', 'ios'], linux: ['linux'], windows: ['windows'], android: ['android'] };

/**
 * The changed files that select each suite: `[{ suite, because: [file, …], only? }]`, in catalogue order. `changed`
 * is a list of repository paths. An XCTest suite selected by its own Swift files alone runs only the classes they reach
 * (`only`, `xctestClasses`), and is left out when they compile nothing on its platform.
 */
export function select(graph, changed, catalogue = suites(graph.repo)) {
  const global = changed.filter(f => GLOBAL.includes(f));
  const out = [];
  for (const suite of catalogue) {
    const { via } = graph.closure(suite.roots);
    const because = global.length ? global : changed.filter(f => via.has(f));
    if (!because.length) continue;
    const only = suite.kind === 'xctest' ? xctestClasses(graph.repo.root, suite.host, because) : null;
    if (only?.length === 0) continue;
    out.push(only ? { suite, because, only } : { suite, because });
  }
  return out;
}

const XCTESTS = 'host/apple/tests/ExactKitTests/';

/** The test classes of an XCTest suite on `host` that `because`, the changed inputs that select it, can reach
 * (`classesReached` in host/apple/xctest.mjs): null when one of them is not a Swift file directly in the tests'
 * directory (the host's Swift, a fixture, a script, a resource) or can reach any class. */
export function xctestClasses(root, host, because) {
  if (!because.every(f => f.startsWith(XCTESTS) && f.endsWith('.swift') && !f.slice(XCTESTS.length).includes('/'))) return null;
  return classesReached(resolve(root, XCTESTS), host === 'ios' ? 'iOS' : 'macOS', because.map(f => f.slice(XCTESTS.length)));
}

/**
 * Every suite: `{ id, kind, roots, … }`, where `kind` is
 *   `cargo`    a package's `cargo test --tests` (`name`, `manifest`: its workspace's)
 *   `bun`      a Bun test file (`file`)
 *   `app`      an app's `.test.contract` files on a host (`app`, `host`, `files`), as `agent.mjs <host> --test`
 *              runs them
 *   `xctest`   the Swift host tests (`host`: macos or ios), `build.mjs --test`
 *   `touch`    real touches on an iOS simulator, `smoke-touch.mjs --build`
 *   `compile`  the tvOS or watchOS build (`host`), installed on nothing
 *   `harmony`  Harmony's end-to-end tests (`memory-backend-test.mjs`) and its workspace's check
 *   `conform`  the web JS target against the wasm runner and the Linux host, for one app (`app`)
 */
export function suites(repo) {
  const out = [];
  for (const p of repo.packages.values()) {
    if (p.manifest && p.targets.some(t => t.test && !t.kind.includes('custom-build'))) out.push({ id: `cargo:${p.name}`, kind: 'cargo', name: p.name, manifest: p.manifest, roots: [`pkg:${p.name}+dev`] });
  }
  for (const f of repo.files) if (TEST_FILE.test(f) && !repo.absent.has(f) && !/^(?:vendor|snapback4)\//.test(f) && !f.includes('/node_modules/')) out.push({ id: `bun:${f}`, kind: 'bun', file: f, roots: [`test:${f}`] });
  for (const app of repo.apps) {
    const files = repo.under(app).filter(f => dirname(f) === app && f.endsWith('.test.contract') && !repo.absent.has(f));
    if (!files.length) continue;
    for (const [platform, hosts] of Object.entries(APP_HOSTS)) {
      const crate = repo.packageAt.get(`${app}/${platform}`);
      for (const host of crate ? hosts : []) {
        out.push({ id: `app:${app.split('/')[1]}:${host}`, kind: 'app', app: app.split('/')[1], host, files,
          roots: [...appBuild(repo, app, [platform]), `platform:${platform}`, 'js:scripts/agent.mjs', 'js:scripts/agent-test.mjs'] });
      }
    }
  }
  // An Apple build reads the platform's hosts but their tests (the Rust crate's test modules, the Bun files, the
  // XCTests); touch smoke also runs `link.test.mjs` over the app it built. `build.mjs --test` adds the XCTests,
  // links them against Caltrain's archive (its default app) and compiles the fixtures with `contract`; the iOS run
  // also builds native-fixture's plan and modules (`host/apple/build.mjs`, `test`).
  const apple = ['platform:apple', 'js:host/apple/build.mjs', 'pkg:caltrain-apple', 'pkg:contract'];
  const xctest = ['dir:host/apple/tests', 'tests:host/apple/tests', ...apple, 'pkg:exact-canvas-vello', 'dir:scripts/fixtures'];
  out.push({ id: 'xctest:macos', kind: 'xctest', host: 'macos', roots: xctest });
  out.push({ id: 'xctest:ios', kind: 'xctest', host: 'ios', roots: [...xctest, 'appsrc:apps/native-fixture', 'pkg:native-fixture-apple'] });
  const caltrain = appBuild(repo, 'apps/caltrain', ['apple']);
  out.push({ id: 'touch:ios', kind: 'touch', host: 'ios', roots: [...apple, 'js:scripts/smoke-touch.mjs', 'test:host/apple/link.test.mjs', ...caltrain] });
  for (const host of ['tvos', 'watchos']) out.push({ id: `compile:${host}`, kind: 'compile', host, roots: [...apple, ...caltrain] });
  out.push({ id: 'harmony', kind: 'harmony', roots: ['app:apps/harmony', 'js:scripts/memory-backend-test.mjs', 'platform:web', 'pkg:exact-web-js', 'pkg:contract'] });
  for (const app of WEB_APPS) {
    const dir = repo.appNames.get(app);
    if (!dir) continue;
    out.push({ id: `conform:${app}`, kind: 'conform', app, roots: ['js:host/web-js/conform.mjs', ...appBuild(repo, dir, ['web', 'linux']), 'platform:web', 'platform:linux', 'pkg:exact-web-js', 'pkg:exact-linux', 'pkg:contract'] });
  }
  return out;
}

/** What a build of `app` for `platforms` compiles: its sources and every package in its directory but another
 * platform's host crate, since the logic module `app.json` names, its GPU crates and its data crate are built for every
 * host (`scripts/app.mjs`), whether or not the host crate depends on them. */
function appBuild(repo, app, platforms) {
  const others = Object.keys(APP_HOSTS).filter(p => !platforms.includes(p)).map(p => `${app}/${p}`);
  const crates = [...repo.packages.values()].filter(p => p.dir.startsWith(`${app}/`) && !others.some(o => p.dir === o || p.dir.startsWith(`${o}/`)));
  return [`appsrc:${app}`, ...crates.map(p => `pkg:${p.name}`)];
}

const git = (args, cwd = ROOT) => {
  const r = spawnSync('git', args, { cwd, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`git ${args.join(' ')}: ${r.stderr.trim()}`);
  return r.stdout;
};
const JS = /\.(?:mjs|cjs|js|ts|tsx|mts)$/;
const TEST_FILE = /\.test\.(?:mjs|cjs|js|ts|tsx|mts)$/;
/** A platform's own tests, which an app's drive or another platform's build does not read. */
const PLATFORM_TEST = /(?:^|\/)tests?\/|\.test\.\w+$|_tests?\.rs$|Tests\.swift$/;
/** The directories that hold one unit each (an app, a package, a host). */
const CONTAINERS = new Set(['apps', 'examples', 'packages', 'host']);
/** Each platform's host directories: one unit, since a platform's parts read each other's files. The other hosts
 * (`host/render`, `host/raster`, …) serve several platforms and are shared. */
const PLATFORMS = {
  web: ['host/web', 'host/web-js', 'host/web-capabilities', 'host/web-geometry'],
  apple: ['host/apple', 'host/apple-painted', 'host/apple-update'],
  linux: ['host/linux', 'host/linux-update', 'host/ink', 'host/svg-raster'],
  windows: ['host/windows'], android: ['host/android'], terminal: ['host/terminal'],
};
/** A platform's name as a test says it, or as a flag (`--ios`). */
const PLATFORM_WORD = new Map(Object.entries({ web: 'web', ios: 'apple', macos: 'apple', tvos: 'apple', watchos: 'apple', apple: 'apple', linux: 'linux', windows: 'windows', android: 'android', terminal: 'terminal' }));
const platformOf = word => PLATFORM_WORD.get(word.replace(/^--/, ''));
const PLATFORM_OF = new Map(Object.entries(PLATFORMS).flatMap(([name, dirs]) => dirs.map(d => [d, `host:${name}`])));

/**
 * The unit a path belongs to: a platform (`host:apple`), an app or an example (`apps/excord`), a package
 * (`packages/cards`), `bench`, `game` or `docs` (documentation outside an app); null for the shared core.
 */
export function unitOf(path) {
  const parts = path.split('/');
  if (parts[0] === 'host') return parts.length > 1 ? PLATFORM_OF.get(`host/${parts[1]}`) ?? null : 'host';
  if (CONTAINERS.has(parts[0])) return parts.length > 1 ? `${parts[0]}/${parts[1]}` : parts[0];
  if (parts[0] === 'bench' || parts[0] === 'game') return parts[0];
  if (['llp', 'docs', 'issues', 'rules'].includes(parts[0]) || (parts.length === 1 && path.endsWith('.md'))) return 'docs';
  return null;
}

/**
 * The repository as the selection reads it: its files, Cargo's packages, the apps, each source parsed once. `also`
 * adds paths no file is at (a change deleted them; code still names them), read as empty. `metadata` (tests) stands
 * in for `cargo metadata`: `{ <workspace manifest>: <its JSON> }`.
 */
export class Repo {
  #sources = new Map();
  #testFiles = new Map();
  constructor(root = ROOT, { files, metadata, also = [] } = {}) {
    this.root = root;
    const listed = new Set(files ?? git(['ls-files', '-z', '-co', '--exclude-standard'], root).split('\0').filter(Boolean));
    this.absent = new Set(also.filter(f => !listed.has(f)));
    this.files = [...listed, ...this.absent].sort();
    this.fileSet = new Set(this.files);
    this.dirs = new Set();
    for (const f of this.files) for (let d = dirname(f); d !== '.'; d = dirname(d)) this.dirs.add(d);
    /** name → { name, dir, manifest, targets, deps: [{ name, kinds }] } for every package with a path here;
     * `manifest` is its workspace's when it is a member of one */
    this.packages = new Map();
    for (const manifest of WORKSPACES) {
      if (!listed.has(manifest)) continue;
      // The root's packages are resolved (a `[patch]`ed crate is a path package there); the others are read from
      // their manifests alone (`--no-deps`), their path dependencies with them.
      const data = metadata?.[manifest] ?? cargoMetadata(root, manifest, manifest === 'Cargo.toml');
      this.#addWorkspace(data, manifest);
    }
    // A file belongs to the deepest package whose directory holds it.
    this.packageAt = new Map([...this.packages.values()].map(p => [p.dir, p]));
    const dirs = [...this.packageAt.keys()].sort((a, b) => b.length - a.length);
    this.ownerOf = new Map();
    for (const f of this.files) {
      const dir = dirs.find(d => d === '' || f.startsWith(`${d}/`));
      if (dir !== undefined) this.ownerOf.set(f, dir);
    }
    this.apps = new Set(this.files.filter(f => /^(?:apps|examples)\/[^/]+\//.test(f)).map(f => f.split('/').slice(0, 2).join('/')));
    this.appNames = new Map([...this.apps].map(a => [a.split('/')[1], a]));
  }

  #addWorkspace(data, manifest) {
    const members = new Set(data.workspace_members), byId = new Map();
    for (const p of data.packages) {
      const dir = relative(this.root, dirname(p.manifest_path));
      if (p.source !== null || dir.startsWith('..')) continue;
      const known = this.packages.get(p.name);
      const pkg = known ?? { name: p.name, dir, manifest: null, targets: p.targets, deps: [] };
      if (members.has(p.id) && !pkg.manifest) pkg.manifest = manifest;
      this.packages.set(p.name, pkg);
      byId.set(p.id, pkg);
      if (!data.resolve && !known) {
        for (const d of p.dependencies) if (d.path) pkg.deps.push({ name: d.name, kinds: [d.kind ?? 'normal'] });
      }
    }
    for (const node of data.resolve?.nodes ?? []) {
      const pkg = byId.get(node.id);
      for (const d of pkg ? node.deps : []) {
        const dep = byId.get(d.pkg);
        if (dep && !pkg.deps.some(x => x.name === dep.name)) pkg.deps.push({ name: dep.name, kinds: d.dep_kinds.map(k => k.kind ?? 'normal') });
      }
    }
  }

  /** The files under `dir`, or `dir` itself when it is a file. */
  under(dir) {
    if (this.fileSet.has(dir)) return [dir];
    const prefix = `${dir}/`;
    let lo = 0, hi = this.files.length;
    while (lo < hi) { const mid = (lo + hi) >> 1; if (this.files[mid] < prefix) lo = mid + 1; else hi = mid; }
    const out = [];
    for (let i = lo; i < this.files.length && this.files[i].startsWith(prefix); i++) out.push(this.files[i]);
    return out;
  }

  read(file) {
    if (this.absent.has(file)) return '';
    try { return readFileSync(resolve(this.root, file), 'utf8'); } catch { return ''; }
  }

  /** `file`'s sources, parsed once for every graph: JavaScript's `{ imports, literals }`, Rust's and Swift's
   * `{ library, test, mods }` (`rustSource`). */
  sources(file) {
    if (!this.#sources.has(file)) {
      const src = this.read(file);
      this.#sources.set(file, JS.test(file) ? jsSources(src, file) : rustSource(src, { swift: file.endsWith('.swift') }));
    }
    return this.#sources.get(file);
  }

  /** What a Rust file is to its package: `build` (its build script), `test` (only its tests compile it: an integration
   * test, a bench, an example, or a module declared under `#[cfg(test)]` and what is under it) or `library`. */
  rustRole(file, pkg) {
    if (file === join(pkg.dir, 'build.rs')) return 'build';
    return this.#testFilesOf(pkg).has(file) ? 'test' : 'library';
  }

  #testFilesOf(pkg) {
    if (this.#testFiles.has(pkg.dir)) return this.#testFiles.get(pkg.dir);
    const own = this.under(pkg.dir || '.').filter(f => f.endsWith('.rs') && this.ownerOf.get(f) === pkg.dir);
    // The files the modules `f` declares: `mod name;` in lib.rs, main.rs or mod.rs is name.rs or name/mod.rs beside
    // it, in a.rs a/name.rs or a/name/mod.rs, `#[path]`'s beside the declaring file. A test module's directory is all
    // test code (what it declares inside an inline module is under it too); a library module's holds its test modules.
    const declared = (f, keep, sweep) => this.sources(f).mods.filter(keep).flatMap(({ name, path }) => {
      const base = /(?:^|\/)(?:lib|main|mod)\.rs$/.test(f) ? dirname(f) : f.slice(0, -3);
      const at = path ? [normalize(join(dirname(f), path))] : [join(base, `${name}.rs`), join(base, name, 'mod.rs')];
      return [...at.filter(p => this.fileSet.has(p)), ...(path || !sweep ? [] : this.under(join(base, name)).filter(p => p.endsWith('.rs')))];
    });
    const reach = (roots, keep, sweep) => {
      const seen = new Set(roots);
      for (const f of seen) for (const p of declared(f, keep, sweep)) seen.add(p);
      return seen;
    };
    // Test code: integration tests, benches, examples, the modules declared under #[cfg(test)], and what they declare.
    // Test-only is what test code reaches and the library's own modules, from its targets' roots, do not.
    const roots = pkg.targets.filter(t => !t.kind.some(k => ['test', 'bench', 'example', 'custom-build'].includes(k))).map(t => relative(this.root, t.src_path));
    const library = reach(roots.filter(f => this.fileSet.has(f)), m => !m.test, false);
    const tests = reach([...own.filter(f => /^(?:tests|benches|examples)\//.test(pkg.dir ? f.slice(pkg.dir.length + 1) : f)),
      ...own.flatMap(f => declared(f, m => m.test, true))], () => true, true);
    const test = new Set([...tests].filter(f => !library.has(f)));
    this.#testFiles.set(pkg.dir, test);
    return test;
  }

}

function cargoMetadata(root, manifest, resolved) {
  const args = ['metadata', '--format-version', '1', '--offline', '--manifest-path', resolve(root, manifest), ...(resolved ? ['--locked'] : ['--no-deps'])];
  const r = spawnSync('cargo', args, { cwd: root, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`cargo ${args.join(' ')}: ${r.stderr.trim()}`);
  return JSON.parse(r.stdout);
}

/**
 * The reference graph. A node is
 *   `pkg:<name>`      a package built as a dependency: its Rust (its test modules but) and manifest, its dependencies,
 *                     what its code names
 *   `pkg:<name>+dev`  a tested package: every file in its directory, its dev-dependencies, what its tests name too
 *   `test:<file>`     a test file: what it imports and names
 *   `js:<file>`       a module it drives: what that imports and names
 *   `load:<file>`     a module another unit's code imports: what that imports and its top level names
 *   `script:<file>`   a file a test or a build script names (a script it runs); `file:<file>` one other code names
 *   `contract:<file>` a Contract source and what it uses
 *   `tests:<dir>`     test sources under a directory (the Swift tests), each file and what it names
 *   `dir:<dir>`       every file under it
 *   `app:<dir>`       an app's files, packages and the packages its package.json names; `appsrc:<dir>` the same but
 *                     its crates
 *   `platform:<name>` a platform's host directories but their tests
 */
export class Graph {
  #edges = new Map();
  #closures = new Map();
  /** `env`: a variable the suite runs under → the roots of what it names (a build another step made; `stepEnv`). */
  constructor(repo, { env = {} } = {}) { this.repo = repo; this.env = new Map(Object.entries(env)); }

  /** What `node` reaches in one step: `{ files, nodes: Map<node, reason> }`. */
  edges(node) {
    if (!this.#edges.has(node)) this.#edges.set(node, this.#expand(node));
    return this.#edges.get(node);
  }

  /** The files the roots reach (`via`: file → the node that first reached it) and each node reached (`parent`:
   * node → `{ from, why }`). */
  closure(roots) {
    const key = roots.join('\n');
    if (this.#closures.has(key)) return this.#closures.get(key);
    const parent = new Map(roots.map(r => [r, { from: null, why: 'root' }])), via = new Map(), queue = [...roots];
    for (let q = 0; q < queue.length; q++) {
      const node = queue[q], { files, nodes } = this.edges(node);
      for (const f of files) if (!via.has(f)) via.set(f, node);
      for (const [n, why] of nodes) if (!parent.has(n)) { parent.set(n, { from: node, why }); queue.push(n); }
    }
    for (const f of GLOBAL) if (!via.has(f)) via.set(f, null);
    const result = { via, parent };
    this.#closures.set(key, result);
    return result;
  }

  /** How `roots` reach `file`: the chain of nodes, each with why it was reached, from the file back to a root. */
  chain(roots, file) {
    const { via, parent } = this.closure(roots);
    if (!via.has(file)) return null;
    const out = [];
    for (let n = via.get(file); n; n = parent.get(n).from) out.push({ node: n, why: parent.get(n).why });
    return out;
  }

  #expand(node) {
    const repo = this.repo, i = node.indexOf(':'), kind = node.slice(0, i), arg = node.slice(i + 1);
    const files = [], nodes = new Map();
    const add = (n, why) => { if (!nodes.has(n)) nodes.set(n, why); };
    const refer = (file, literals, pkgDir, as) => { for (const [n, why] of references(this, file, literals, pkgDir, as)) add(n, why); };
    if (kind === 'pkg') {
      const [name, dev] = arg.split('+');
      const pkg = repo.packages.get(name);
      if (!pkg) return { files, nodes };
      for (const f of repo.under(pkg.dir || '.')) {
        if (repo.ownerOf.get(f) !== pkg.dir) continue;
        const role = f.endsWith('.rs') ? repo.rustRole(f, pkg) : null;
        if (dev || (role && role !== 'test') || f === join(pkg.dir, 'Cargo.toml')) files.push(f);
        if (!role || (role === 'test' && !dev)) continue;
        const { library, test } = repo.sources(f);
        if (role === 'library') {
          refer(f, library, pkg.dir, 'library');
          if (dev) refer(f, test, pkg.dir, 'test');
        } else refer(f, [...library, ...test], pkg.dir, role);
      }
      for (const d of pkg.deps) if (dev || d.kinds.some(k => k !== 'dev')) add(`pkg:${d.name}`, `${pkg.dir}/Cargo.toml`);
      // A workspace apart from the root's: its manifest and lock (the root's are `GLOBAL`)
      if (pkg.manifest && pkg.manifest !== 'Cargo.toml') files.push(pkg.manifest, join(dirname(pkg.manifest), 'Cargo.lock'));
    } else if (kind === 'test' || kind === 'js' || kind === 'load') {
      files.push(arg);
      const { imports, literals } = repo.sources(arg);
      const home = unitOf(arg);
      for (const spec of imports) {
        const n = importNode(repo, arg, spec);
        if (!n) continue;
        const target = n.startsWith('file:') ? n.slice(5) : null;
        const driven = kind !== 'load' && (kind === 'test' || !target || unitOf(target) === null || unitOf(target) === home);
        add(target && JS.test(target) ? `${driven ? 'js' : 'load'}:${target}` : n, `${arg}: import '${spec}'`);
      }
      refer(arg, kind === 'load' ? literals.filter(l => l.top) : literals, null, kind === 'test' ? 'test' : 'module');
    } else if (kind === 'file' || kind === 'script') {
      if (kind === 'script' && WHOLE.has(arg)) files.push(...repo.files);
      if (JS.test(arg)) add(`${kind === 'script' ? 'js' : 'load'}:${arg}`, arg);
      else if (arg.endsWith('.contract')) add(`contract:${arg}`, arg);
      else files.push(arg);
    } else if (kind === 'contract') {
      files.push(arg);
      for (const m of repo.read(arg).matchAll(/^\s*use\b[^\n]*?\bfrom\s+"([^"]+)"/gm)) {
        const n = importNode(repo, arg, m[1]);
        if (n) add(n.startsWith('file:') && n.endsWith('.contract') ? `contract:${n.slice(5)}` : n, `${arg}: use from "${m[1]}"`);
      }
    } else if (kind === 'tests') {
      for (const f of repo.under(arg)) {
        files.push(f);
        if (/\.(?:swift|rs)$/.test(f)) { const { library, test } = repo.sources(f); refer(f, [...library, ...test], null, 'test'); }
      }
    } else if (kind === 'dir') {
      for (const f of repo.under(arg)) f.endsWith('.contract') ? add(`contract:${f}`, arg) : files.push(f);
    } else if (kind === 'app' || kind === 'appsrc') {
      for (const f of repo.under(arg)) {
        const owner = repo.ownerOf.get(f);
        if (kind === 'appsrc' && owner !== undefined && owner.startsWith(`${arg}/`)) continue;
        f.endsWith('.contract') ? add(`contract:${f}`, arg) : files.push(f);
      }
      if (kind === 'app') for (const p of repo.packages.values()) if (p.dir.startsWith(`${arg}/`)) add(`pkg:${p.name}`, `${p.dir}/Cargo.toml`);
      // The packages its package.json names (`file:../../packages/<name>`)
      let manifest = {};
      try { manifest = JSON.parse(repo.read(`${arg}/package.json`) || '{}'); } catch {}
      for (const spec of Object.values({ ...manifest.dependencies, ...manifest.devDependencies })) {
        const dir = /^file:(.+)$/.exec(spec)?.[1];
        if (dir && repo.dirs.has(normalize(join(arg, dir)))) add(`dir:${normalize(join(arg, dir))}`, `${arg}/package.json: ${spec}`);
      }
    } else if (kind === 'platform') {
      for (const dir of PLATFORMS[arg] ?? []) for (const f of repo.under(dir)) if (!PLATFORM_TEST.test(f)) files.push(f);
    }
    return { files, nodes };
  }
}

/** The node an import specifier (JavaScript's, or Contract's `from`) names, if it is in this repository. */
function importNode(repo, from, spec) {
  if (spec.startsWith('@exact/')) {
    const dir = `packages/${spec.slice(7).split('/')[0]}`;
    return repo.dirs.has(dir) ? `dir:${dir}` : null;
  }
  if (!spec.startsWith('.')) return null;
  const base = normalize(join(dirname(from), spec));
  for (const candidate of [base, ...['.mjs', '.js', '.ts', '.tsx', '.mts', '.cjs', '.json'].map(e => base + e), ...['index.mjs', 'index.js', 'index.ts'].map(e => join(base, e))]) {
    if (repo.fileSet.has(candidate)) return `file:${candidate}`;
  }
  return repo.dirs.has(base) ? `dir:${base}` : null;
}

/** A call that takes a path: a bare word passed to one is a path segment (`join("apps")`, `resolve(ROOT, 'docs')`). */
const PATH_CALL = /^(?:join|resolve|relative|new|from|push|readdirSync|readdir|readFileSync|readFile|existsSync|statSync|lstatSync|cpSync|Glob|file|URL|include_str!|include_bytes!)$/;
/** The expression a bare path segment is joined onto names the repository's root. */
const ROOTED = /(?:root|repo|manifest|workspace|checkout)\w*(?:\(\))?\s*(?:\.|,)?\s*$/i;
/** The expression a bare path segment is joined onto is the code's own directory: `import.meta.dir`, `__dirname`,
 * `dirname(fileURLToPath(import.meta.url))`, a binding named `here`. */
const OWN_DIR = /^(?:import\.meta\.dir(?:name)?|__dirname|here|HERE)$|import\.meta\.url/;
/** A call, property or flag that names an app or a Cargo package. */
const NAME_CALL = /app|crate|package/i;
const NAME_FLAG = new Set(['--app', '-p', '--package', '--bin', '--crate']);

/**
 * The nodes `file`'s string literals name, each with the reason, environment variables the graph knows included. A
 * literal is split into words at whitespace (a command line), and each word is read with the one before it (its first
 * with the literal before); prose (a message, a reason) is skipped. `as` is what the code is: a `test` or a `build`
 * script reads what it names; other code (`module`, `library`) only dispatches to an app, a package or another
 * platform's host that it names, but reads a path into one app or package, and what it includes (`include_str!`).
 *   - A package or an app is named after `--app` or `-p`, or to a call or property that takes one. A hyphenated
 *     package name, which no plain word is, is named wherever it is written. A default (`?? 'caltrain'`) is read by
 *     any code, since a caller that names nothing gets it.
 *   - A test drives a platform it names in a call that runs or drives, or in a list of platforms.
 *   - A bare word joined onto the root is a path from it, and one joined onto the code's own directory (`OWN_DIR`) a
 *     name beside the code's file.
 *   - A literal that says where it starts (`from`: the code's own directory, its package's or the root) is joined
 *     onto that base alone.
 *   - A build script reads the path its `cargo:rerun-if-changed=` names and, by a bare `..`, its package's parent when
 *     that is in an app.
 */
function references(graph, file, literals, pkgDir, as) {
  const repo = graph.repo, out = new Map();
  const add = (n, text) => { if (!out.has(n)) out.set(n, `${file}: "${text}"`); };
  const own = dirname(file), home = unitOf(file), test = as === 'test' || as === 'build', js = JS.test(file);
  const reaches = target => test || unitOf(target) === null || unitOf(target) === home;
  // Join a literal's path onto the base it says it starts at (`from`), else onto each base in turn
  const bases = { own: [own], package: pkgDir === null ? [] : [pkgDir], root: [''] };
  for (const literal of literals) {
    if (!literal.text || literal.text.length > 300) continue;
    const words = literal.text.split(/\s+/).filter(Boolean);
    if (words.length > 1 && (words.length > 16 || /[,;:.`§)…—]\s|\s[(`]/.test(literal.text))) continue;
    if (words.length === 1 && literal.compared) continue;
    words.forEach((word, i) => {
      // Strip `cargo:rerun-if-changed=` to read the path a build script declares (`../app.json`)
      if (as === 'build') word = word.replace(/^cargo::?rerun-if-changed=/, '');
      const prev = i ? words[i - 1] : literal.prev, call = i ? '' : literal.call ?? '';
      const names = NAME_FLAG.has(prev) || NAME_CALL.test(call);
      if (graph.env.has(word)) { for (const n of graph.env.get(word)) add(n, word); return; }
      // A default is no dispatch: it is what a caller that names nothing reads (`resolveApp()` reads Caltrain's app,
      // whose name its crate's shares).
      if (words.length === 1 && literal.fallback && (repo.packages.has(word) || repo.appNames.has(word))) {
        if (repo.packages.has(word)) add(`pkg:${word}`, word);
        if (repo.appNames.has(word)) add(`appsrc:${repo.appNames.get(word)}`, word);
        return;
      }
      // A name both an app's and its crate's (`caltrain`) is the app's after `--app` or to a call that takes an app.
      const app = repo.appNames.get(word), pkg = repo.packages.get(word);
      if (app && names && (!pkg || prev === '--app' || /app/i.test(call))) { if (reaches(app)) add(`app:${app}`, word); return; }
      if (pkg) { if ((names || word.includes('-')) && reaches(pkg.dir)) add(`pkg:${word}`, word); return; }
      if (app) return;
      // (`spawnSync('bun', [agent, 'ios', …])`, `open({ host: 'web' })`, `for (const host of ['web', 'linux'])`; not
      // `judge(…, { host: 'ios' })`, which describes one)
      if (platformOf(word)) {
        if (js && as === 'test' && (literal.drives || /\.mjs$/.test(prev) || prev === '--host')) add(`platform:${platformOf(word)}`, word);
        return;
      }
      if (word.startsWith('@exact/')) { const n = importNode(repo, file, word); if (n && reaches(n.slice(n.indexOf(':') + 1))) add(n, word); return; }
      // A bare word joined onto the root (`repo().join("apps")`, `resolve(ROOT, 'docs')`), or onto the code's own
      // directory (`resolve(import.meta.dir, 'new')`, `join(here, 'caps.mjs')`); not onto a temporary directory.
      const passed = words.length === 1 && PATH_CALL.test(call);
      if (!word.includes('/') && word !== '.' && word !== '..' && passed && OWN_DIR.test(literal.base ?? '')) {
        const near = join(own, word);
        if (repo.fileSet.has(near)) { add(`${test ? 'script' : 'file'}:${near}`, word); return; }
        if (repo.dirs.has(near)) { add(repo.apps.has(near) ? `app:${near}` : `dir:${near}`, word); return; }
      }
      // Read a build script's bare `..` (its package's parent) as the app sources there: an app's crate builds its app
      if (as === 'build' && word === '..' && pkgDir) { const up = dirname(pkgDir); if (repo.apps.has(unitOf(up))) add(`appsrc:${up}`, word); return; }
      if (!word.includes('/') && !(passed && ROOTED.test(literal.receiver ?? ''))) return;
      const path = word.replace(/^\/(?=\.\.?\/)/, '');
      if (path.startsWith('/') || /^[a-z][a-z0-9+.-]*:/i.test(path)) return;
      for (const base of bases[literal.from] ?? [...bases.own, ...bases.package, ...bases.root]) {
        const target = normalize(join(base, path)).replace(/\/$/, '');
        if (target === '.' || target.startsWith('..')) continue;
        // A container, or the code's own directory or one above it, is where its paths start; a build script reads
        // the one above it (an app's crate builds its app).
        if ((!test && CONTAINERS.has(target)) || (as !== 'build' && (own === target || own.startsWith(`${target}/`)))) return;
        // (`tamber tool` serves apps/design-tool, a path into one app; a table of hosts names each platform's, and a
        // tool for games names game/ to make one)
        if (!reaches(target) && !/^(?:apps|examples|packages)\//.test(unitOf(target)) && !/^include_(?:str|bytes)!$/.test(call)) return;
        // Skip a directory of tests named by code other than a test: it names it to run the tests there (`build.mjs
        // --test`), and the suite that runs them has it among its roots. A file there that it names still counts.
        if (!test && repo.dirs.has(target) && PLATFORM_TEST.test(`${target}/`)) return;
        if (repo.fileSet.has(target)) { add(`${test ? 'script' : 'file'}:${target}`, word); return; }
        // (An app's directory is its files to code that reads it, and the whole app, its crates too, to a test.)
        if (repo.dirs.has(target)) { add(repo.apps.has(target) ? `${test ? 'app' : 'appsrc'}:${target}` : `dir:${target}`, word); return; }
      }
    });
  }
  return out;
}

/** `env!("CARGO_MANIFEST_DIR")`, and a `concat!` that starts with it, alone or inside an `include_str!` or
 * `include_bytes!` (whose name it captures) */
const MANIFEST = '\\s*env!\\(\\s*"CARGO_MANIFEST_DIR"\\s*\\)';
const CONCAT_MANIFEST = new RegExp(`(?:(include_(?:str|bytes)!)\\(\\s*)?concat!\\(${MANIFEST}\\s*,\\s*$`);

/**
 * Rust's or Swift's string literals, comments skipped, each `{ text, call, receiver, prev, compared }`: `text` up to a
 * format string's first `{` (Swift's: its first `\(`), or the path after `env!("CARGO_MANIFEST_DIR")` in a `format!`
 * or `concat!`, from the package's directory (`from: 'package'`), `call` the function or macro it is passed to
 * (`join`, `include_str!`), `receiver` what that is called on, `prev` the literal just before it in the same list or
 * call chain (`"-p", "exact-linux"`, `.arg("-p").arg("exact-linux")`), `compared` whether it is compared with
 * (`== "x"`, a match arm). An attribute's string is a note (`#[ignore = "…"]`), unless it is `#[path]`'s or
 * `include_str!`'s. They come as `library` and `test` (inside what is under `#[cfg(test)]`), with `mods`, the modules
 * declared (`mod name;`: `{ name, path, test }`, `path` from `#[path]`, `test` when under `#[cfg(test)]`).
 */
export function rustSource(src, { swift = false } = {}) {
  const found = [], code = src.split('');
  const blank = (from, to) => { for (let k = from; k < to; k++) if (code[k] !== '\n') code[k] = ' '; };
  let last = null;
  const push = (text, at, end) => {
    const before = src.slice(Math.max(0, at - 40), at), after = src.slice(end, end + 8);
    const call = /([A-Za-z_]\w*!?)\(\s*(?:&\s*)?$/.exec(before)?.[1] ?? '';
    const receiver = /([\w!()"]+?)\s*\.\s*[A-Za-z_]\w*\(\s*(?:&\s*)?$/.exec(before)?.[1] ?? /\(\s*([\w!]+)\s*,\s*$/.exec(before)?.[1] ?? '';
    const prev = last && /^\s*\)?\s*(?:,|\.args?\(\s*\[?)\s*\[?\s*$/.test(src.slice(last.end, at)) ? last.text : '';
    const item = { text: cut(text, /\{/), call, receiver, prev, at, compared: /(?:==|!=)\s*$/.test(before) || /^\s*(?:==|!=|=>|\|)/.test(after) };
    // Read a path that starts with the package's directory as the path after it, from there:
    // `format!("{}/../corpus/{name}", env!("CARGO_MANIFEST_DIR"))`, `format!("{dir}/…", dir = env!(…))`,
    // `concat!(env!("CARGO_MANIFEST_DIR"), "/…")`
    const lead = swift ? null : /^\{(\w*)\}\//.exec(text), joined = swift || !text.startsWith('/') ? null : CONCAT_MANIFEST.exec(src.slice(Math.max(0, at - 100), at));
    if (lead && new RegExp(lead[1] ? `^[^;]*?\\b${lead[1]}\\s*=${MANIFEST}` : `^\\s*,${MANIFEST}`).test(src.slice(end, end + 200))) Object.assign(item, { text: cut(`.${text.slice(lead[0].length - 1)}`, /\{/), from: 'package' });
    // Keep the call of an include around the `concat!`: what an include reads counts in any code
    else if (joined) Object.assign(item, { text: `.${text}`, from: 'package', call: joined[1] ?? call });
    const attribute = /#!?\[[^\]]*$/.test(before);
    if (!attribute || /#!?\[\s*path\s*=\s*$/.test(before) || /^include_(?:str|bytes)!$/.test(call)) found.push(item);
    last = { text: item.text, end };
  };
  for (let i = 0; i < src.length;) {
    const c = src[i];
    if (c === '/' && src[i + 1] === '/') { const end = src.indexOf('\n', i); blank(i, end < 0 ? src.length : end); if (end < 0) break; i = end; continue; }
    if (c === '/' && src[i + 1] === '*') {
      const from = i;
      let depth = 1; i += 2;
      while (i < src.length && depth) { if (src.startsWith('/*', i)) { depth++; i += 2; } else if (src.startsWith('*/', i)) { depth--; i += 2; } else i++; }
      blank(from, i);
      continue;
    }
    const raw = !swift && (c === 'r' || c === 'b') && !/\w/.test(src[i - 1] ?? '') ? /^b?r(#*)"/.exec(src.slice(i, i + 40)) : null;
    if (raw) {
      const close = '"' + raw[1], start = i + raw[0].length, end = src.indexOf(close, start);
      if (end < 0) break;
      push(src.slice(start, end), i, end + close.length);
      blank(i, end + close.length);
      i = end + close.length;
      continue;
    }
    if (c === '"') {
      let s = '', j = i + 1, stop = -1;
      for (; j < src.length && src[j] !== '"'; j++) {
        if (src[j] !== '\\') { s += src[j]; continue; }
        const e = src[++j];
        if (swift && e === '(' && stop < 0) stop = s.length;
        if (e === '\n') { while (/\s/.test(src[j + 1] ?? '')) j++; } else s += e === 'n' ? '\n' : e === 't' ? '\t' : e;
      }
      push(stop < 0 ? s : s.slice(0, stop), i, j + 1);
      blank(i, j + 1);
      i = j + 1;
      continue;
    }
    if (c === "'" && !swift) {
      // A char literal ('a', '\n', '\u{1F600}', '"'), else a lifetime or a label ('a, 'static): one quote, skipped.
      const ch = /^'(?:\\u\{[0-9a-fA-F]+\}|\\.|[^\\'\n])'/.exec(src.slice(i, i + 12));
      if (ch) blank(i, i + ch[0].length);
      i += ch ? ch[0].length : 1;
      continue;
    }
    i++;
  }
  // The items under #[cfg(test)], found in the code with its strings and comments blanked: an item's block braces
  // matched, or a declaration to its `;`; on a field, a variant, an arm or an expression, up to the `,` or `;` or the
  // bracket that ends it. And every module declared (`mod name;`), each with its #[path] and whether under
  // #[cfg(test)].
  const masked = code.join(''), tests = [], mods = [];
  const attributesBefore = at => at - /(?:#\[[^\]]*\]\s*)*$/.exec(masked.slice(0, at))[0].length;
  for (const m of masked.matchAll(/(?<![\w!])(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;/g)) {
    const attrs = src.slice(attributesBefore(m.index), m.index);
    mods.push({ name: m[1], path: /#\[\s*path\s*=\s*"([^"]*)"\s*\]/.exec(attrs)?.[1] ?? null, test: /#\[cfg\((?:all\()?test\b/.test(attrs) });
  }
  // (An item, a macro's call, a block or a block expression: it ends at its braces.)
  const ITEM = /^(?:(?:pub(?:\([^)]*\))?\s+)?(?:(?:unsafe|async|const|extern(?:\s+"[^"]*")?)\s+)*(?:fn|mod|impl|struct|enum|union|const|static|use|type|trait|macro_rules!|extern)\b|[\w:]+!\s*[{([]|\{|(?:if|match|for|while|loop|unsafe)\b)/;
  for (const m of masked.matchAll(/#\[cfg\((?:all\()?test\b[^\]]*\]/g)) {
    const start = attributesBefore(m.index);
    let k = m.index + m[0].length;
    k += /^(?:\s*#\[[^\]]*\])*\s*/.exec(masked.slice(k))[0].length;
    const item = ITEM.test(masked.slice(k, k + 80));
    let depth = 0, end = masked.length;
    for (; k < masked.length; k++) {
      const ch = masked[k];
      if ('{[('.includes(ch)) depth++;
      else if ('}])'.includes(ch)) {
        if (--depth < 0) { end = k; break; }
        if (depth === 0 && ch === '}' && item) { end = k + 1; break; }
      } else if (depth === 0 && (ch === ';' || (ch === ',' && !item))) { end = k + 1; break; }
    }
    tests.push([start, end]);
  }
  const inTest = at => tests.some(([a, b]) => at >= a && at < b);
  return { library: found.filter(l => !inTest(l.at)), test: found.filter(l => inTest(l.at)), mods };
}

/** Rust's string literals (`rustSource`), library and test code alike. */
export const rustLiterals = src => { const { library, test } = rustSource(src); return [...library, ...test].sort((a, b) => a.at - b.at); };

const cut = (s, at) => { const m = at.exec(s); return m ? s.slice(0, m.index) : s; };

/** An expression as dotted names (`import.meta.dir`, `app.dir`, `dirname(fileURLToPath(import.meta.url))`), far
 * enough to tell the code's own directory from another's. */
const expression = n => n?.type === 'Identifier' ? n.name : n?.type === 'MetaProperty' ? `${n.meta.name}.${n.property.name}`
  : n?.type === 'MemberExpression' && !n.computed ? `${expression(n.object)}.${n.property.name}`
  : /^(?:Call|New)Expression$/.test(n?.type) ? `${expression(n.callee)}(${n.arguments.map(expression).join(', ')})` : '';

/** The name a call, property or binding gives: `resolveApp(…)`, `x.join(…)`, `{ app: … }`, `crate = …`. */
const nameOf = n => n?.type === 'Identifier' ? n.name : n?.type === 'MemberExpression' ? nameOf(n.property)
  : n?.type === 'Literal' ? String(n.value) : '';

/**
 * A JS or TS file's import specifiers and its other string literals, as `rustSource` gives them, each with `top` (it is
 * outside every function, so read as the module loads) and `drives` (it reaches a call that runs or drives through an
 * argument, an argument's array or property, or sits in a list of platforms): a template's text before its first
 * substitution, or the path after a leading own directory or root (`from`; a name the file binds to a string is a
 * placeholder), and a glob's before its first wildcard. A path a call is given in parts (`resolve(import.meta.dir,
 * '..', 'apps')`) is also given whole. A variable read by name (`process.env.EXACT_WEB_DIST`) is a literal too.
 */
export function jsSources(src, file) {
  const imports = [], literals = [];
  const glob = s => cut(s, /[*?[{]/);
  let ast;
  try { ast = parseAst(src, { lang: /\.tsx$/.test(file) ? 'tsx' : /\.m?ts$/.test(file) ? 'ts' : 'js' }); }
  catch { return { imports, literals: [...src.matchAll(/(['"`])((?:(?!\1)[^\\\n]|\\.)*)\1/g)].map(m => ({ text: glob(m[2]), call: '', prev: '', top: true })) }; }
  const text = n => n?.type === 'Literal' && typeof n.value === 'string' ? n.value : n?.type === 'TemplateLiteral' ? n.quasis[0]?.value?.cooked ?? '' : null;
  // Treat a name the file binds to a string (`const ROOT = '/x/exact2'`) as a placeholder, not a directory
  const bound = new Set(ast.body.flatMap(s => (s.type === 'ExportNamedDeclaration' ? s.declaration : s)?.declarations ?? [])
    .filter(d => d.id?.type === 'Identifier' && typeof d.init?.value === 'string').map(d => d.id.name));
  // Read a template that starts with the code's own directory or the root (`${import.meta.dir}/../assets/`,
  // `${ROOT}docs/scale-measured.md`) as the path after it, from there
  const from = n => {
    if (n?.type !== 'TemplateLiteral' || n.quasis[0]?.value?.cooked || !n.expressions.length) return null;
    const lead = expression(n.expressions[0]);
    const base = bound.has(lead) ? null : OWN_DIR.test(lead) ? 'own' : ROOTED.test(lead) ? 'root' : null;
    return base && { text: `./${(n.quasis[1]?.value?.cooked ?? '').replace(/^\//, '')}`, from: base };
  };
  const isCall = n => n?.type === 'CallExpression' || n?.type === 'NewExpression';
  const isFunction = n => /^(?:FunctionDeclaration|FunctionExpression|ArrowFunctionExpression)$/.test(n?.type);
  const contextOf = (n, parent, grand) => {
    const list = parent?.type === 'ArrayExpression' ? parent.elements : isCall(parent) ? parent.arguments : null;
    const at = list ? list.indexOf(n) : -1;
    const call = isCall(parent) && at >= 0 ? nameOf(parent.callee)
      : parent?.type === 'ArrayExpression' && isCall(grand) ? nameOf(grand.callee)
      : parent?.type === 'Property' && parent.value === n ? nameOf(parent.key)
      : parent?.type === 'AssignmentPattern' ? nameOf(parent.left)
      : parent?.type === 'VariableDeclarator' ? nameOf(parent.id) : '';
    const compared = (parent?.type === 'BinaryExpression' && /^[!=]==?$/.test(parent.operator)) || parent?.type === 'SwitchCase'
      || (isCall(parent) && /^(?:includes|has|startsWith|endsWith|test)$/.test(nameOf(parent.callee)));
    const receiver = at > 0 ? nameOf(list[at - 1]) : parent?.callee?.type === 'MemberExpression' ? nameOf(parent.callee.object) : '';
    const base = at > 0 ? expression(list[at - 1]) : parent?.callee?.type === 'MemberExpression' ? expression(parent.callee.object) : '';
    let drives = parent?.type === 'ArrayExpression' && parent.elements.every(e => text(e) !== null && platformOf(text(e)));
    for (let i = ancestors.length - 1, child = n; !drives && i >= Math.max(0, ancestors.length - 4); child = ancestors[i--]) {
      const a = ancestors[i];
      if (isCall(a)) { drives = a.arguments.includes(child) && DRIVE_CALL.test(nameOf(a.callee)); break; }
      if (!/^(?:ArrayExpression|ObjectExpression|Property|SpreadElement)$/.test(a.type)) break;
    }
    // A value used when the caller gives none: `name ?? 'caltrain'`, `given ? given : 'caltrain'`,
    // `crate = 'caltrain-apple'`
    const fallback = parent?.type === 'AssignmentPattern' || (parent?.type === 'LogicalExpression' && /^(?:\?\?|\|\|)$/.test(parent.operator) && parent.right === n)
      || (parent?.type === 'ConditionalExpression' && parent.test !== n);
    return { call, prev: at > 0 ? text(list[at - 1]) ?? '' : '', compared, receiver, base, drives, fallback };
  };
  const ancestors = [];
  let depth = 0;
  const visit = n => {
    if (!n || typeof n !== 'object') return;
    if (Array.isArray(n)) { for (const x of n) visit(x); return; }
    const parent = ancestors.at(-1), grand = ancestors.at(-2), top = depth === 0;
    if (/^(?:Import|ExportAll|ExportNamed)Declaration$/.test(n.type) && n.source?.value) imports.push(n.source.value);
    else if (n.type === 'ImportExpression' && typeof n.source?.value === 'string') imports.push(n.source.value);
    else if (n.type === 'MemberExpression' && !n.computed && /^[A-Z][A-Z0-9_]+$/.test(n.property?.name ?? '')) literals.push({ text: n.property.name, call: 'env', prev: '', top });
    else if (text(n) !== null) {
      const context = { ...contextOf(n, parent, grand), top }, start = from(n);
      literals.push(start ? { ...context, text: glob(start.text), from: start.from } : { text: glob(text(n)), ...context });
      const args = isCall(parent) ? parent.arguments : [], at = args.indexOf(n);
      let first = at;
      while (first > 0 && text(args[first - 1]) !== null) first--;
      if (at > first && text(args[at + 1]) === null) {
        literals.push({ ...context, text: glob(args.slice(first, at + 1).map(text).join('/')), receiver: first > 0 ? nameOf(args[first - 1]) : '', base: first > 0 ? expression(args[first - 1]) : '' });
      }
    }
    ancestors.push(n);
    if (isFunction(n)) depth++;
    for (const k in n) if (k !== 'type' && k !== 'start' && k !== 'end' && k !== 'range') visit(n[k]);
    if (isFunction(n)) depth--;
    ancestors.pop();
  };
  visit(ast.body);
  return { imports, literals };
}
/** A call that runs or drives something: a platform named in its arguments is one a test drives. */
const DRIVE_CALL = /^(?:spawn|spawnSync|exec|execSync|execFile|execFileSync|run|runApple|open|drive|launch|session|smoke|verify|agent)$/i;

/** The files changed since `base` (default: the merge base with origin/main), the working tree's included. */
export function changedSince(base, root = ROOT) {
  const from = base ?? git(['merge-base', 'HEAD', 'origin/main'], root).trim();
  const tracked = git(['diff', '-z', '--name-only', '--no-renames', from], root).split('\0');
  const untracked = git(['ls-files', '-z', '-o', '--exclude-standard'], root).split('\0');
  return [...new Set([...tracked, ...untracked].filter(Boolean))].sort();
}

/** How each selected suite runs; the Cargo packages of a workspace and the Bun files are one command each. */
export function commands(selected) {
  const out = [];
  const byManifest = new Map();
  for (const { suite } of selected) if (suite.kind === 'cargo') (byManifest.get(suite.manifest) ?? byManifest.set(suite.manifest, []).get(suite.manifest)).push(suite.name);
  for (const [manifest, names] of byManifest) {
    out.push(['cargo', 'test', ...(manifest === 'Cargo.toml' ? [] : ['--manifest-path', manifest]), '--tests', '--no-fail-fast', ...names.flatMap(n => ['-p', n])]);
  }
  const bun = selected.filter(s => s.suite.kind === 'bun').map(s => `./${s.suite.file}`);
  if (bun.length) out.push(['bun', 'test', '--isolate', ...bun]);
  for (const { suite: s, only } of selected) {
    if (s.kind === 'app') for (const f of s.files) out.push(['bun', 'scripts/agent.mjs', s.host, '--app', s.app, '--test', f]);
    else if (s.kind === 'xctest') out.push(['bun', 'host/apple/build.mjs', '--test', ...(s.host === 'ios' ? ['--ios'] : []), ...(only ? ['--only', only.join(',')] : [])]);
    else if (s.kind === 'touch') out.push(['bun', 'scripts/smoke-touch.mjs', '--build']);
    else if (s.kind === 'compile') out.push(['bun', 'host/apple/build.mjs', `--${s.host}`, '--no-install']);
    else if (s.kind === 'harmony') out.push(['bun', 'scripts/memory-backend-test.mjs', 'apps/harmony/app'], ['cargo', 'check', '--manifest-path', 'apps/harmony/app/Cargo.toml', '--all-targets', '--keep-going']);
  }
  const conform = selected.filter(s => s.suite.kind === 'conform').map(s => s.suite.app);
  if (conform.length) out.push(['bun', 'host/web-js/conform.mjs', ...conform, '--synthetic', '--build', '--linux']);
  return out;
}

/**
 * The tests of `packages` (in the workspace of `manifest`), built as `cargo test --workspace --tests` builds them
 * (every member's features unified, so a selection builds nothing a full run does not, and shares the lane's build),
 * then each test binary of those packages run as Cargo runs it: from its package's directory, with Cargo's variables,
 * its library path and a build script's `OUT_DIR` and `cargo:rustc-env`, `args` passed on (`--ignored <test>…`), its
 * start and its failure said in Cargo's words (`Running`, `error: test failed, to rerun pass `-p <package> --lib``).
 * Every binary runs whatever the others did. Returns the exit status: 101 when a build or a test failed, as Cargo's.
 */
export function cargoTests(root, manifest, packages, args = []) {
  const meta = cargoMetadata(root, manifest, false), wanted = new Set(packages);
  const byId = new Map(meta.packages.map(p => [p.id, p]));
  const manifestArgs = manifest === 'Cargo.toml' ? [] : ['--manifest-path', manifest];
  const built = spawnSync('cargo', ['test', ...manifestArgs, '--workspace', '--tests', '--no-fail-fast', '--no-run', '--message-format=json-render-diagnostics'],
    { cwd: root, stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 1 << 30 });
  const binaries = [], linked = new Set(), scripts = new Map(), target = meta.target_directory;
  for (const line of (built.stdout ?? '').split('\n')) {
    if (!line.startsWith('{')) continue;
    const m = JSON.parse(line);
    if (m.reason === 'build-script-executed') {
      scripts.set(m.package_id, { OUT_DIR: m.out_dir, ...Object.fromEntries(m.env ?? []) });
      for (const p of m.linked_paths ?? []) { const dir = p.replace(/^\w+=/, ''); if (dir.startsWith(target)) linked.add(dir); }
    }
    if (m.reason === 'compiler-artifact' && m.executable && m.profile?.test && wanted.has(byId.get(m.package_id)?.name)) binaries.push(m);
  }
  const sysroot = spawnSync('rustc', ['--print', 'sysroot'], { cwd: root, encoding: 'utf8' }).stdout.trim();
  const host = /^host: (\S+)$/m.exec(spawnSync('rustc', ['-vV'], { cwd: root, encoding: 'utf8' }).stdout)?.[1];
  const libraryPath = process.platform === 'darwin' ? 'DYLD_FALLBACK_LIBRARY_PATH' : 'LD_LIBRARY_PATH';
  const fallback = process.platform === 'darwin' ? `${homedir()}/lib:/usr/local/lib:/usr/lib` : '';
  const libraries = [...linked, join(target, 'debug'), join(target, 'debug', 'deps'), join(sysroot, 'lib', 'rustlib', host ?? '', 'lib'), join(sysroot, 'lib'), process.env[libraryPath] ?? fallback].filter(Boolean).join(':');
  const failed = [];
  for (const b of binaries) {
    const p = byId.get(b.package_id), dir = dirname(p.manifest_path), [major, minor, patch] = p.version.split(/[.+-]/);
    // (Cargo names any library `--lib`, whatever its crate types: `cdylib`, `rlib`, `staticlib`)
    const kind = b.target.kind[0], flag = ['bin', 'test', 'bench', 'example'].includes(kind) ? `--${kind} ${b.target.name}` : '--lib';
    // Set the library path, then a build script's variables, then Cargo's own, in the order Cargo's `fill_env` does
    const env = { ...process.env, [libraryPath]: libraries, ...scripts.get(b.package_id), CARGO: join(sysroot, 'bin', 'cargo'), CARGO_MANIFEST_DIR: dir, CARGO_MANIFEST_PATH: p.manifest_path,
      CARGO_PKG_NAME: p.name, CARGO_PKG_VERSION: p.version, CARGO_PKG_VERSION_MAJOR: major, CARGO_PKG_VERSION_MINOR: minor,
      CARGO_PKG_VERSION_PATCH: patch, CARGO_PKG_VERSION_PRE: /-(.*)$/.exec(p.version)?.[1] ?? '', CARGO_PKG_AUTHORS: (p.authors ?? []).join(':'),
      CARGO_PKG_DESCRIPTION: p.description ?? '', CARGO_PKG_HOMEPAGE: p.homepage ?? '', CARGO_PKG_REPOSITORY: p.repository ?? '',
      CARGO_PKG_LICENSE: p.license ?? '', CARGO_PKG_LICENSE_FILE: p.license_file ?? '', CARGO_PKG_RUST_VERSION: p.rust_version ?? '',
      CARGO_PKG_README: p.readme ?? '' };
    const what = kind === 'test' || kind === 'bench' ? relative(dir, b.target.src_path) : `unittests ${relative(dir, b.target.src_path)}`;
    console.log(`     Running ${what} (${relative(root, b.executable)})`);
    const r = spawnSync(b.executable, args, { cwd: dir, env, stdio: 'inherit' });
    if (r.status !== 0) {
      failed.push(`-p ${p.name} ${flag}`);
      console.log(`error: test failed, to rerun pass \`-p ${p.name} ${flag}\``);
    }
  }
  if (failed.length) console.log(`error: ${failed.length} target${failed.length > 1 ? 's' : ''} failed:\n${failed.map(f => `    \`${f}\``).join('\n')}`);
  return built.status !== 0 || failed.length ? 101 : 0;
}

/** A sandbox profile that lets a suite read everything but the files of the units outside its inputs (Cargo's manifests
 * aside, which it reads to load the workspace). */
export function auditProfile(repo, inputs) {
  const units = new Map();
  for (const f of repo.files) {
    const unit = unitOf(f);
    if (!unit || unit === 'host') continue;
    const dir = unit.startsWith('host:') ? PLATFORMS[unit.slice(5)].find(d => f.startsWith(`${d}/`)) : unit;
    if (!units.has(dir)) units.set(dir, false);
    if (inputs.has(f)) units.set(dir, true);
  }
  const denied = [...units].filter(([, read]) => !read).map(([dir]) => dir).sort();
  // A file's contents, not a directory's listing or identity (Cargo lists a member's targets; `realpath` reads a
  // directory).
  const rule = dir => `(deny file-read-data (require-all (subpath ${JSON.stringify(resolve(repo.root, dir))}) (vnode-type REGULAR-FILE) (require-not (regex #"/Cargo\\.toml$"))))`;
  return { denied, profile: ['(version 1)', '(allow default)', ...denied.map(rule)].join('\n') };
}

const quote = a => /^[\w./@:=+-]+$/.test(a) ? a : `'${a.replace(/'/g, `'\\''`)}'`;

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const option = name => argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined;
  if (argv.includes('--help')) {
    console.log('usage: bun scripts/affected.mjs [--base <rev>] [--why <suite>] [--json] [--run] [--audit <suite>]\n       bun scripts/affected.mjs --cargo-tests [--manifest <Cargo.toml>] <package>… [-- <test args>]');
    process.exit(0);
  }
  if (argv[0] === '--cargo-tests') {
    const rest = argv.indexOf('--') < 0 ? argv.slice(1) : argv.slice(1, argv.indexOf('--'));
    const manifest = rest[0] === '--manifest' ? rest[1] : 'Cargo.toml', packages = rest[0] === '--manifest' ? rest.slice(2) : rest;
    // (The workspace is the working directory's, as Cargo's is.)
    process.exit(cargoTests(process.cwd(), manifest, packages, argv.indexOf('--') < 0 ? [] : argv.slice(argv.indexOf('--') + 1)));
  }
  const changed = option('--audit') ? [] : changedSince(option('--base'));
  const repo = new Repo(ROOT, { also: changed }), graph = new Graph(repo), catalogue = suites(repo);
  if (option('--audit')) {
    const suite = catalogue.find(s => s.id === option('--audit'));
    if (!suite) throw new Error(`--audit ${option('--audit')}: no such suite; \`bun scripts/affected.mjs --json --base <rev>\` lists suite ids`);
    const { denied, profile } = auditProfile(repo, new Set(graph.closure(suite.roots).via.keys()));
    const file = resolve(repo.root, 'target/affected-audit.sb');
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, profile);
    let status = 0;
    for (const command of commands([{ suite }])) {
      console.error(`audit ${suite.id}: ${denied.length} units unreadable; ${command.map(quote).join(' ')}`);
      status ||= spawnSync('sandbox-exec', ['-f', file, ...command], { cwd: repo.root, stdio: 'inherit' }).status ?? 1;
    }
    process.exit(status);
  }
  const selected = select(graph, changed, catalogue);
  if (option('--why')) {
    const hit = selected.find(s => s.suite.id === option('--why'));
    if (!hit) {
      const suite = catalogue.find(s => s.id === option('--why'));
      const reached = suite && changed.some(f => graph.closure(suite.roots).via.has(f));
      console.log(`${option('--why')}: not selected (${!suite ? 'no such suite' : reached ? 'the test files that changed compile nothing on its platform' : 'no changed file is among its inputs'})`);
      process.exit(0);
    }
    if (hit.only) console.log(`runs ${hit.only.join(', ')} (--only): the test files that changed reach no other class`);
    for (const file of hit.because) {
      console.log(file);
      for (const { node, why } of graph.chain(hit.suite.roots, file) ?? []) console.log(`  <- ${node}  (${why})`);
    }
    process.exit(0);
  }
  if (argv.includes('--json')) {
    console.log(JSON.stringify({ changed, selected: selected.map(s => ({ id: s.suite.id, because: s.because.slice(0, 5), ...(s.only ? { only: s.only } : {}) })), commands: commands(selected) }, null, 2));
    process.exit(0);
  }
  const counts = kind => `${selected.filter(s => s.suite.kind === kind).length} of ${catalogue.filter(s => s.kind === kind).length}`;
  console.log(`${changed.length} changed files select ${selected.length} of ${catalogue.length} suites: Cargo packages ${counts('cargo')}, Bun files ${counts('bun')}, app tests ${counts('app')}, Apple ${['xctest', 'touch', 'compile'].map(counts).join(' + ')}, Harmony ${counts('harmony')}, conformance ${counts('conform')}`);
  const run = commands(selected);
  for (const c of run) console.log(`  ${c.map(quote).join(' ')}`);
  if (!argv.includes('--run')) process.exit(0);
  let status = 0;
  for (const c of run.filter(c => (c[0] === 'cargo' && c[1] === 'test') || (c[0] === 'bun' && c[1] === 'test'))) {
    console.log(`\n$ ${c.map(quote).join(' ')}`);
    status ||= spawnSync(c[0], c.slice(1), { cwd: repo.root, stdio: 'inherit' }).status ?? 1;
  }
  process.exit(status);
}
