#!/usr/bin/env bun
// The smoke drives the resolved app through the agent API (LLP 1012) on the
// web, macOS, iOS, or Linux. Every app gets the generic host fixtures and its
// own app.test.contract; Caltrain's landmarks, interactions, GPU reference,
// and deck run when its fixture root is present. Not a blocking check (it needs Chrome or
// a window server): `bun scripts/smoke.mjs <web|macos|ios|linux> [--shot <png>]`
// --app-only runs the complete selected app drive and its Contract tests,
// not the unrelated bare-plan host fixtures. On the web those fixtures run on
// a second build that links every capability (LLP 1047 D7); --app-only skips
// it too. ios --device selects a phone.
// after `bun host/web/build.mjs` / `bun host/apple/build.mjs [--ios]` /
// `cargo build --release -p caltrain-linux`.
import { spawnSync } from 'node:child_process';
import { createHash, verify } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, readlinkSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { browserDiagnosticNoise, open as openAgent, render, runTests as runAgentTests } from './agent.mjs';
import { resolveApp, withAppFixture } from './app.mjs';
import { DirectoryOrigin, parseWebRoot, webReleasePath, webRootPath } from './origin.mjs';
import { jsTargetBuild, readStaticFile, serveStatic } from '../host/web/serve.mjs';
import { canonicalBytes, publicKeyFromRaw, webRelease } from './deploy.mjs';
import { crop, decodePng, diff, encodePng } from './png.mjs';

const argv = process.argv.slice(2);
const ROOT = resolve(new URL('..', import.meta.url).pathname);
const appName = argv.includes('--app') ? argv[argv.indexOf('--app') + 1] : undefined;
const app = resolveApp(appName);
// Exact Live's job fixture on loopback, for its stream drive (step 16): the
// web build below compiles against it; a native build must have been made
// with the same EXACT_LIVE_JOB_ORIGINS (scripts/smoke-stream.mjs).
let streamFixture = null;
if (app.name === 'exact-live' && ['web', 'macos', 'linux'].includes(argv[0])) {
  const { STREAM_ORIGINS, startStreamFixture } = await import('./smoke-stream.mjs');
  process.env.EXACT_LIVE_JOB_ORIGINS ??= STREAM_ORIGINS;
  streamFixture = await startStreamFixture();
}
let selectedWebDist = null;
let fixtureWebDist = null; // the router sweep's wasm build, linking every capability (LLP 1047 D7)
const device = argv.includes('--device');
const phone = argv.includes('--phone') ? argv[argv.indexOf('--phone') + 1] : undefined;
const open = (options) => openAgent({ device, phone, ...options, app: options.app ?? app.name, webDist: options.webDist ?? selectedWebDist }); // a plan: its JS build (`--plan`)
const runTests = (options) => runAgentTests({ device, phone, ...options, app: options.app ?? app.name, webDist: options.webDist ?? selectedWebDist });

// 0. The transcript form (LLP 1012 §7): the one text rendering of the
// replies, pinned by a fixture — `scripts/fixtures/transcript.json` rendered
// must equal `transcript.txt`. `--record` rewrites the text from the code
// after a deliberate change to the form.
const transcript = () => {
  const sample = JSON.parse(readFileSync(resolve(ROOT, 'scripts/fixtures/transcript.json'), 'utf8'));
  // A sample named `empty` or `dropped` is a `logs` reply; the rest are named by their op.
  return Object.entries(sample).map(([name, reply]) => `--- ${name}\n${render(name === 'empty' || name === 'dropped' ? 'logs' : name, reply)}`).join('\n\n') + '\n';
};
const pinned = resolve(ROOT, 'scripts/fixtures/transcript.txt');
if (argv.includes('--record')) { writeFileSync(pinned, transcript()); console.log(`recorded ${pinned.replace(ROOT + '/', '')}`); process.exit(0); }

const host = argv[0] === 'macos' || argv[0] === 'mac' ? 'macos' : argv[0] === 'web' ? 'web' : argv[0] === 'ios' ? 'ios' : argv[0] === 'linux' ? 'linux' : argv[0] === 'host' ? 'host' : argv[0] === 'host-ios' ? 'host-ios' : argv[0] === 'deploy' ? 'deploy' : argv[0] === 'svg' ? 'svg' : argv[0] === 'canvas' ? 'canvas' : argv[0] === 'motion' ? 'motion' : null;
if (!host) { console.error('usage: bun scripts/smoke.mjs <web|macos|ios|linux|host|host-ios|deploy|svg|canvas|motion> [--app <name>] [--shot <png>] [--hosts linux,macos,ios] | --record'); process.exit(2); }

// The two Apple presenters share one Canvases: children captured through the
// surface, placements (LLP 1014 D2, D5) — what the canvas steps below assert.
const apple = host === 'macos' || host === 'ios';
const shot = argv.includes('--shot') ? argv[argv.indexOf('--shot') + 1] : process.env.EXACT_SHOT;
// --record-canvas rewrites this host's reference picture of the canvas
// fixture (step 10) after a deliberate change to what it shows.
const recordCanvas = argv.includes('--record-canvas');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const failures = [];
const check = (ok, what) => { if (!ok) failures.push(what); return ok; };
const t0 = Date.now();
const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
const box = (l, id) => l.nodes.find((n) => n.testId === id);

// A source-checkout invariant for destructive-looking smokes: names,
// contents, modes, sizes, and mtimes are identical before and after, even
// across the deliberately refused publisher calls.
function treeFingerprint(root) {
  const rows = [];
  const walk = (path, name) => {
    const info = lstatSync(path);
    const meta = `${name}\0${info.mode}\0${info.size}\0${info.mtimeMs}`;
    if (info.isSymbolicLink()) rows.push(`${meta}\0link\0${readlinkSync(path)}`);
    else if (info.isDirectory()) {
      rows.push(`${meta}\0dir`);
      for (const child of readdirSync(path).sort()) {
        if (name === '' && child === '.git') continue;
        walk(resolve(path, child), name ? `${name}/${child}` : child);
      }
    } else if (info.isFile()) rows.push(`${meta}\0file\0${createHash('sha256').update(readFileSync(path)).digest('hex')}`);
    else rows.push(`${meta}\0other`);
  };
  walk(root, '');
  // Read-only Git commands can refresh index cache metadata. The source
  // invariant covers Git's meaning, not timestamps on those cache files.
  for (const args of [['rev-parse', 'HEAD'], ['status', '--porcelain=v1', '--untracked-files=all', '--', '.']]) {
    const state = spawnSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    if (state.status !== 0) throw new Error(`cannot fingerprint source Git state: ${state.stderr}`);
    rows.push(`git ${args[0]}\0${state.stdout}`);
  }
  return createHash('sha256').update(rows.join('\n')).digest('hex');
}
// The app's viewport (step 2): the safe area on a phone, which step 12's
// full-bleed fixture grows by the insets.
let appViewport;

check(transcript() === readFileSync(pinned, 'utf8'), 'the transcript form drifted from scripts/fixtures/transcript.txt (a deliberate change: bun scripts/smoke.mjs --record)');
check(browserDiagnosticNoise('CVDisplayLinkCreateWithCGDisplay failed. CVReturn: -6670'), 'the known headless display-service diagnostic is no longer classified as browser noise');
check(!browserDiagnosticNoise('console.error: exact: failed'), 'page/runtime errors must not be classified as browser noise');
check(browserDiagnosticNoise('[1:2:0927/223530.638588:ERROR:components/page_load_metrics/browser/page_load_metrics_update_dispatcher.cc:179] Invalid first_paint 0.059 s for first_image_paint 0.057 s'), 'Chrome\'s paint-timing bookkeeping is no longer classified as browser noise');
check(!browserDiagnosticNoise('[1:2:0927/223530.286557:ERROR:components/os_crypt/common/keychain_password_mac.mm:102] Keychain lookup failed'), 'a keychain lookup must fail the smoke: the carrier launches Chrome with a mock keychain');

// SVG parity (LLP 1055.000 §5): apps/svg-gallery on each native host against
// Chrome's, fixture by fixture (scripts/svgparity.mjs).
if (host === 'svg') {
  const { svgParity } = await import('./svgparity.mjs');
  const hosts = argv.includes('--hosts') ? argv[argv.indexOf('--hosts') + 1].split(',') : ['linux', ...(process.platform === 'darwin' ? ['macos', 'ios'] : [])];
  await svgParity({ open: (o) => openAgent({ device, phone, ...o }), check, hosts });
  console.log(`svg smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  if (failures.length) { for (const f of failures) console.error('  ' + f); process.exit(1); }
  process.exit(0);
}

// Motion parity (LLP 1011.000, LLP 1055 D7): apps/motion-gallery's animated
// images and keyframes on each native host against Chrome's at fixed clock
// times (scripts/motionparity.mjs).
if (host === 'motion') {
  const { motionParity } = await import('./motionparity.mjs');
  const hosts = argv.includes('--hosts') ? argv[argv.indexOf('--hosts') + 1].split(',') : ['linux', ...(process.platform === 'darwin' ? ['macos', 'ios'] : [])];
  await motionParity({ open: (o) => openAgent({ device, phone, ...o }), check, hosts });
  console.log(`motion smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  for (const f of failures) console.error('  ' + f); process.exit(failures.length ? 1 : 0);
}

// Canvas 2D parity (LLP 1056 §4): apps/canvas-gallery on each native host
// against Chrome's, and the web's recorded path against Chrome's own context
// (scripts/canvasparity.mjs).
if (host === 'canvas') {
  const { canvasParity } = await import('./canvasparity.mjs');
  const hosts = argv.includes('--hosts') ? argv[argv.indexOf('--hosts') + 1].split(',') : ['linux', ...(process.platform === 'darwin' ? ['macos', 'ios'] : [])];
  await canvasParity({ open: (o) => openAgent({ device, phone, ...o }), check, hosts, only: argv.includes('--only') ? argv[argv.indexOf('--only') + 1] : null });
  console.log(`canvas smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  if (failures.length) { for (const f of failures) console.error('  ' + f); process.exit(1); }
  process.exit(0);
}

// The publisher (LLP 1030.000 D3–D5, D7): `exact deploy` driven end to end
// against a directory origin with a throwaway key. The app and the scripts
// run from a disposable tracked checkout; manifest and asset edits never
// touch the developer's source tree. What it holds:
// a dry run prints the table and writes nothing; `--yes` publishes the web
// root (the JS target's build when it takes the app; a browser boots it and
// its deck loads from the immutable release), the blobs, and one signed head
// per stream, and the head verifies
// with the key; repeated bundle bytes stay current, and the origin follows its
// actual public cards (identical cards rewrite nothing); an asset edit
// is `bundle seq 2` naming the asset, with the record's `previous` the old
// head's digest and the old blob kept; a publisher meeting its stream locked
// refuses naming the lock and the head stays, and once the stream is free
// the next publishes seq 3; a retired cohort's stream is `binary` and
// `--only bundle` refuses it while the others publish; a wrong `--snapshot`
// and a dirty tree without `--dirty` refuse.
if (host === 'deploy') {
  const sourceState = treeFingerprint(app.dir);
  try { await withAppFixture(app, async (fixture) => {
  const dir = fixture.run;
  const fixtureRoot = fixture.exactRoot;
  const fixtureApp = fixture.app.dir;
  const origin = resolve(dir, 'origin');
  const keys = resolve(dir, 'keys');
  const deployScript = realpathSync(resolve(fixtureRoot, 'scripts/deploy.mjs'));
  const manifestPath = resolve(fixtureApp, 'app.json');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  const keyId = manifest.deploy?.signing?.key ?? 'exact-smoke';
  // Signing policy belongs only to the captured test app. A developer app
  // without a publishing setup still exercises the same signed delivery.
  manifest.deploy ??= {};
  manifest.deploy.signing ??= { key: keyId, keys: {} };
  manifest.deploy.signing.keys ??= {};
  manifest.deploy.signing.keys[keyId] ??= Buffer.alloc(32).toString('base64');
  writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
  fixture.git(['add', '-A']); fixture.git(['commit', '--allow-empty', '-qm', 'Diagnostic publishing policy']);
  const channel = manifest.deploy?.channel ?? 'prod';
  const assetName = existsSync(resolve(fixtureApp, 'assets')) ? readdirSync(resolve(fixtureApp, 'assets')).find((n) => statSync(resolve(fixtureApp, 'assets', n)).isFile()) : null;
  const assetPath = assetName ? resolve(fixtureApp, 'assets', assetName) : null;
  const asset0 = assetPath ? readFileSync(assetPath) : null;
  const sha = (b) => createHash('sha256').update(b).digest('hex');
  // Cargo build scripts record absolute source paths. Sharing Exact's target
  // with this throwaway checkout would poison the developer's next build
  // with paths that disappear at cleanup, so its build cache is private too.
  const deployEnv = { ...fixture.env, EXACT_SIGNING_KEY_DIR: keys };
  const deploy = (args, expectExit = 0) => {
    const r = spawnSync(process.execPath, [deployScript, ...args], { cwd: fixtureRoot, encoding: 'utf8', env: deployEnv, maxBuffer: 64 * 1024 * 1024 });
    check(r.status === expectExit, `deploy ${args.filter((a) => !a.startsWith('/')).join(' ')} exited ${r.status}, not ${expectExit}: ${r.error?.message ?? ''}\n${(r.stderr + r.stdout).split('\n').slice(-12).join('\n')}`);
    return r;
  };
  const table = (...args) => { const r = deploy([app.name, '--origin', origin, '--json', ...args]); try { return JSON.parse(r.stdout); } catch { check(false, `deploy --json printed no object: ${r.stdout.slice(0, 200)}`); return { rows: [] }; } };
  const streams = (t) => t.rows.filter((r) => r.kind === 'stream');
  const headOf = (id) => { const p = resolve(origin, '.exact', channel, id, 'exact.json'); return existsSync(p) ? readFileSync(p) : null; };
  const mtimes = () => { const out = []; const walk = (d) => { for (const e of readdirSync(d, { withFileTypes: true })) { const p = resolve(d, e.name); if (e.isDirectory()) walk(p); else if (e.name !== '.lock') out.push(`${statSync(p).mtimeMs} ${p}`); } }; walk(origin); return out.sort().join('\n'); };
  try {
    if (!check(!!keyId, `${app.name}/app.json names no deploy.signing.key`)) throw new Error('no key');
    // 1. A throwaway key, and the dry run before the manifest is touched (the table, nothing written).
    const kg = deploy(['keygen', keyId, '--keys', keys, '--json']);
    let publicKey;
    try { publicKey = JSON.parse(kg.stdout).publicKey; }
    catch { throw new Error(`keygen printed no JSON: ${kg.error?.message ?? ''} ${kg.stderr}${kg.stdout}`); }
    check(Buffer.from(publicKey, 'base64').length === 32, `keygen printed ${publicKey}`);
    const dry = table();
    check(dry.dryRun === true && dry.rows[0]?.kind === 'origin' && dry.rows[0].action === 'publish', `the dry run's origin row is ${JSON.stringify(dry.rows[0])}`);
    check(streams(dry).length >= 1 && streams(dry).every((r) => r.action === 'bundle' && r.seq === 1 && r.head === null), `the dry run's stream rows are ${JSON.stringify(streams(dry).map((r) => [r.platform, r.action, r.seq]))}`);
    check(/^[0-9a-f]{40}$/.test(dry.snapshot?.commit ?? ''), `the snapshot is ${JSON.stringify(dry.snapshot)}`);
    check(!existsSync(origin), 'a dry run wrote to the origin');
    const https = deploy([app.name, '--origin', 'https://updates.invalid', '--yes'], 1);
    check(/https origin, read-only/.test(https.stderr + https.stdout), `https --yes did not refuse as read-only: ${https.stderr + https.stdout}`);
    // 2. The manifest names the throwaway key; --yes publishes; every head verifies with it.
    manifest.deploy.signing.keys[keyId] = publicKey;
    writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
    deploy([app.name, '--origin', origin], 1); // dirty without --dirty refuses
    deploy([app.name, '--origin', origin, '--dirty', '--snapshot', 'deadbeef'], 1);
    check(treeFingerprint(app.dir) === sourceState, 'a refused deploy changed the source app checkout');
    const pub1 = table('--yes', '--dirty');
    const ids = streams(pub1).map((r) => r.compatibilityId);
    check(ids.length >= 1 && pub1.published.filter((p) => p.kind === 'stream' && p.action === 'published').length === ids.length && pub1.failed.length === 0, `publish: ${JSON.stringify({ published: pub1.published.map((p) => [p.kind, p.action, p.seq]), failed: pub1.failed })}`);
    check(existsSync(resolve(origin, webRootPath)) && readStaticFile(origin, '/index.html') && readStaticFile(origin, '/exact.json') && readStaticFile(origin, '/app.plan'), 'the atomic web root was published');
    const requests = [];
    const webServer = createServer((req, res) => { requests.push(new URL(req.url, 'http://exact.invalid').pathname); serveStatic(origin, req, res); });
    await new Promise((done) => webServer.listen(0, '127.0.0.1', done));
    let browser;
    try {
      browser = await open({ host: 'web', browser: 'chrome', url: `http://127.0.0.1:${webServer.address().port}/` });
      // A release admits no agent mode (LLP 1069.007 D2): read the page as a browser shows it.
      const page = (expression) => browser.carrier.evaluate(expression);
      check(await page("document.getElementById('exact-root')?.childElementCount > 0"), 'the published web release booted in Chrome');
      if (await page(`!!document.querySelector('[data-testid="open-deck"]')`)) {
        await page(`document.querySelector('[data-testid="open-deck"]').click()`);
        let src = null;
        for (let i = 0; i < 120 && !src; i++) {
          src = await page("(() => { const f = document.querySelector('iframe'); return f?.contentDocument?.getElementById('deck-title') ? f.src : null; })()");
          if (!src) await sleep(25);
        }
        check(src?.includes('/.exact/root/web/releases/'), `the published deck loaded from its immutable release: ${src}`);
      }
      // The program is the JS target's `app.js` when it takes the app, else `app.wasm` (LLP 1071 §7, delivery).
      check(requests.some((p) => /^\/\.exact\/root\/web\/releases\/[0-9a-f]{64}\/app\.(?:js|wasm)$/.test(p))
        && !requests.some((p) => /^\/(assets|deck|shaders)\//.test(p)), `the browser used immutable local resources: ${requests.join(', ')}`);
      const lines = browser.carrier.hostLines;
      check(!lines.some((line) => /exception:|console.error: exact:/.test(line)), `published browser diagnostics: ${lines.join(' | ')}`);
    } finally { if (browser) await browser.close(); await new Promise((done) => webServer.close(done)); }
    const blobs1 = readdirSync(resolve(origin, '.exact/blobs')).length;
    check(blobs1 >= 1, 'blobs were written');
    for (const id of ids) {
      const head = JSON.parse(headOf(id).toString('utf8'));
      const ok = verify(null, canonicalBytes(head), publicKeyFromRaw(Buffer.from(publicKey, 'base64')), Buffer.from(head.signature.ed25519, 'base64'));
      check(ok && head.stream.seq === 1 && head.stream.compatibilityId === id && head.stream.channel === channel && head.signature.keyId === keyId, `the head of ${id.slice(0, 8)} verifies and names its stream`);
      const cards = [head.plan, ...head.assets];
      check(!existsSync(resolve(origin, '.exact', channel, id, 'app.plan'))
        && cards.every((card) => card.url === `../../blobs/${card.sha256}` && existsSync(resolve(origin, '.exact', 'blobs', card.sha256)))
        && readdirSync(resolve(origin, '.exact', channel, id, 'releases')).length === 1,
      `the stream ${id.slice(0, 8)} points at immutable blobs and has one release record`);
    }
    // 3. Bundle bytes stay current. Relocated external Rust packages can
    // produce different wasm bytes; the origin must classify those actual
    // public cards exactly and install only the resulting complete graph.
    const before = mtimes();
    const oldPointer = parseWebRoot(readFileSync(resolve(origin, webRootPath)));
    const oldHeads = ids.map(id => headOf(id));
    const again = table('--yes', '--dirty', '--release', 'r-smoke-repeat');
    const stages = readdirSync(resolve(fixture.app.target, 'deploy')).filter(name => name.startsWith('r-smoke-repeat-'));
    if (stages.length !== 1) throw new Error(`the repeat invocation left ${stages.length} private bakes`);
    const baked = resolve(fixture.app.target, 'deploy', stages[0]);
    const candidate = webRelease(resolve(baked, existsSync(resolve(baked, 'web-js')) ? 'web-js' : 'web'));
    const oldCards = new Map(oldPointer.files.map(card => [card.name, card.sourceSha256]));
    const newCards = new Map(candidate.pointer.files.map(card => [card.name, card.sourceSha256]));
    const expected = { new: [], changed: [], current: [], removed: [] };
    for (const [name, digest] of newCards) expected[!oldCards.has(name) ? 'new' : oldCards.get(name) === digest ? 'current' : 'changed'].push(name);
    for (const name of oldCards.keys()) if (!newCards.has(name)) expected.removed.push(name);
    for (const names of Object.values(expected)) names.sort();
    const action = expected.new.length + expected.changed.length + expected.removed.length ? 'publish' : 'current';
    const row = again.rows.find(r => r.kind === 'origin');
    check(streams(again).every(r => r.action === 'current' && r.seq === 1)
      && ids.every((id, i) => headOf(id).equals(oldHeads[i])), 'identical bundle bytes left every native stream current and unchanged');
    check(row?.action === action && canonicalBytes(row.files).equals(canonicalBytes(expected)), `repeat origin classification: expected ${JSON.stringify({ action, files: expected })}, got ${JSON.stringify(row)}`);
    const adapter = new DirectoryOrigin(origin);
    check((await adapter.get(webRootPath))?.equals(canonicalBytes(candidate.pointer)), 'the published pointer names this invocation’s actual complete public graph');
    for (const file of candidate.files) check((await adapter.get(`${webReleasePath(candidate.pointer.id)}/${file.name}`))?.equals(file.body), `repeat publication readback differs for ${file.name}`);
    const after = mtimes();
    if (action === 'current') check(after === before, 'a byte-identical public graph rewrote an origin file');
    else {
      const oldEntries = new Set(before.split('\n')), newEntries = new Set(after.split('\n'));
      const pathOf = entry => entry.slice(entry.indexOf(' ') + 1);
      const pointerPath = resolve(origin, webRootPath);
      const allowed = new Set([pointerPath, ...candidate.files.map(file => resolve(origin, webReleasePath(candidate.pointer.id), file.name))]);
      check([...oldEntries].every(entry => pathOf(entry) === pointerPath || newEntries.has(entry))
        && [...newEntries].every(entry => oldEntries.has(entry) || allowed.has(pathOf(entry))), 'repeat publication rewrote or added files outside its new graph and pointer');
    }
    // 4. An asset edit: bundle seq 2 naming it; the record's previous is the old head's digest; the old blob is kept.
    if (assetPath) {
      const first = ids[0];
      const prev = sha(headOf(first));
      writeFileSync(assetPath, Buffer.concat([asset0, Buffer.from([0])]));
      const edited = table('--dirty');
      check(streams(edited).every((r) => r.action === 'bundle' && r.seq === 2 && r.changes.some((c) => c.name === `assets/${assetName}` && c.change === 'changed')), `after the asset edit the rows are ${JSON.stringify(streams(edited).map((r) => [r.action, r.seq, r.changes]))}`);
      const pub2 = table('--yes', '--dirty');
      const head2 = JSON.parse(headOf(first).toString('utf8'));
      const record = JSON.parse(readFileSync(resolve(origin, '.exact', channel, first, 'releases', `${pub2.release}.json`), 'utf8'));
      check(head2.stream.seq === 2 && record.previous === prev && record.seq === 2 && readdirSync(resolve(origin, '.exact/blobs')).length === blobs1 + 1, `seq 2: head seq ${head2.stream.seq}, previous ${record.previous === prev}, blobs ${readdirSync(resolve(origin, '.exact/blobs')).length} (was ${blobs1})`);
      writeFileSync(assetPath, asset0);
    }
    // 5. A publisher meets its stream locked (the restored asset makes the
    // bundle new again): it refuses naming the lock and the head stays; then
    // the stream is free and the next one publishes seq 3. The smoke holds the
    // lock itself: two whole publishers never met inside a short hold, since
    // each recompiles its app crates to write its own receipts, and deploys
    // into one Cargo target take turns.
    if (assetPath) {
      const platform = streams(pub1)[0].platform;
      const stream = { channel, compatibilityId: ids[0] };
      const publish = (name, expectExit) => deploy([app.name, '--origin', origin, '--yes', '--dirty', '--platform', platform, '--only', 'bundle', '--release', name], expectExit);
      const seq = () => JSON.parse(headOf(ids[0]).toString('utf8')).stream.seq;
      const held = await new DirectoryOrigin(origin).withLock(stream, async () => publish('r-held', 1));
      const said = held.stderr + held.stdout;
      check(/locked by another publisher/.test(said) && seq() === 2, `the held stream: head seq ${seq()}; the publisher said ${said.split('\n').filter((l) => /locked|failed/.test(l)).join(' | ').slice(0, 300)}`);
      publish('r-free', 0);
      check(seq() === 3, `after the lock was released the head is seq ${seq()}, not 3`);
      check(await new DirectoryOrigin(origin).withLock(stream, async () => true), 'the permanent lock was not released');
    }
    // 6. A retired cohort's stream on the origin: binary in the table; --only bundle refuses it, the others go on.
    const dead = 'deadbeefdeadbeefdeadbeefdeadbeef';
    const deadHead = JSON.parse(headOf(ids[0]).toString('utf8'));
    deadHead.stream.compatibilityId = dead;
    deadHead.stream.seq = 7;
    mkdirSync(resolve(origin, '.exact', channel, dead), { recursive: true });
    writeFileSync(resolve(origin, '.exact', channel, dead, 'exact.json'), JSON.stringify(deadHead) + '\n');
    const retired = table('--dirty', '--only', 'bundle', '--yes');
    const deadRow = streams(retired).find((r) => r.compatibilityId === dead);
    check(deadRow?.action === 'binary' && retired.refused.some((r) => r.compatibilityId === dead) && retired.failed.length === 0 && streams(retired).filter((r) => r.compatibilityId !== dead).every((r) => r.action !== 'binary'), `the retired stream: ${JSON.stringify({ row: deadRow?.action, refused: retired.refused.length, failed: retired.failed, others: streams(retired).filter((r) => r.compatibilityId !== dead).map((r) => r.action) })}`);
    check(retired.notes.some((n) => n.includes(dead) && n.includes('does not verify')), 'the table names the retired stream\'s head as one no binary would take');
  } catch (e) {
    check(false, `the deploy smoke stopped: ${e.message}`);
  }
  }); } catch (error) { check(false, `the deploy fixture stopped: ${error.message}`); }
  finally { check(treeFingerprint(app.dir) === sourceState, 'the deploy smoke changed source app content, metadata, or tree shape'); }
  console.log(`deploy smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  if (failures.length) { for (const f of failures) console.error('  ' + f); process.exit(1); }
  process.exit(0);
}

// The sample host (LLP 1031 D10): a native macOS app that is not Exact's,
// hosting two sessions of the one plan. What the fixture holds, driven
// through the same carrier with each request routed by session label: two
// sessions with overlapping node ids answer apart; operations interleave;
// a command from one session pushes a native screen over the other
// (unmounted, alive) and pops it (remounted) with both intact; a bad
// candidate plan is refused and the running apps kept; a session destroyed
// under the other is refused by name after, and the other still answers.
// The recorder's sample host (LLP 1067.000 Q6): two sessions, two recorders.
if ((host === 'host' || host === 'host-ios') && app.modules.tags.includes('waveform-view')) {
  const { recorderSessions } = await import('./smoke-recorder.mjs');
  const failures = await recorderSessions({ host, open });
  for (const f of failures) console.log('  ' + f);
  console.log(`${host} smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s — two sessions, each its own recorder, one outliving the other`);
  process.exit(failures.length ? 1 : 0);
}
if (host === 'host' || host === 'host-ios') {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-host-'));
  const control = resolve(dir, 'control');
  writeFileSync(control, '');
  const say = (line) => writeFileSync(control, readFileSync(control, 'utf8') + line + '\n');
  const s = await open({ host, browser: 'chrome', session: 'a', env: { EXACT_HOST_CONTROL: control } });
  const hostFailures = [];
  const trace = process.env.EXACT_SMOKE_TRACE ? (m) => console.log(`[${((Date.now() - t0) / 1000).toFixed(1)}s] ${m}`) : () => {};
  const hcheck = (ok, what) => { trace(`${ok ? 'ok' : 'FAIL'} ${what}`); if (!ok) hostFailures.push(what); return ok; };
  const settleHost = async (ms = 300) => { await sleep(ms); };
  try {
    hcheck(Array.isArray(s.sessions) && s.sessions.join(',') === 'a,b', `the host announced sessions ${JSON.stringify(s.sessions)}, not a,b`);
    // 1. Two sessions of one plan, overlapping ids, answering apart.
    const ta = await s.tree();
    s.session = 'b';
    const tb = await s.tree();
    hcheck(byTestId(ta, 'caltrain-main') && byTestId(tb, 'caltrain-main'), 'both sessions show the plan');
    hcheck(ta.roots[0] === tb.roots[0], `node ids overlap across sessions (${ta.roots[0]} vs ${tb.roots[0]}): ids are the runner's, not the process's`);
    const la = await (async () => { s.session = 'a'; return s.layout(); })();
    const lb = await (async () => { s.session = 'b'; return s.layout(); })();
    hcheck(la.viewport.w > 200 && lb.viewport.w > 200, `each session lays out at its pane's width (${la.viewport.w}, ${lb.viewport.w})`);
    // 2. Interleaved operations: a tap on b opens its station picker; a is untouched; a clock on a moves only a.
    await s.tap('change-station');
    await s.clock('settle');
    const tb2 = await s.tree();
    hcheck(byTestId(tb2, 'station-search') != null, 'b opened its station picker');
    s.session = 'a';
    const ta2 = await s.tree();
    hcheck(byTestId(ta2, 'station-search') == null && byTestId(ta2, 'change-station') != null, 'a is untouched by b\'s tap');
    await s.clock('+60000');
    const sa = await s.state();
    s.session = 'b';
    const sb = await s.state();
    hcheck(sa.clock === 60000 && (sb.clock ?? 0) < 60000, `each session has its own clock (a ${sa.clock}, b ${sb.clock})`);
    // 3. A command from b pushes a native screen over a (unmounted, alive), then pops it.
    await s.tap('scheme-dark');
    await s.clock('settle');
    await settleHost();
    s.session = 'a';
    const laGone = await s.layout();
    const taAlive = await s.tree();
    hcheck(laGone.nodes.length === 0, `a is unmounted under the native screen (${laGone.nodes.length} boxes on screen)`);
    hcheck(taAlive.nodes.length === ta2.nodes.length, 'a is alive while unmounted: its tree still answers');
    s.session = 'b';
    await s.tap('scheme-light');
    await s.clock('settle');
    await settleHost();
    s.session = 'a';
    const laBack = await s.layout();
    hcheck(laBack.nodes.length > 0 && laBack.viewport.w > 200, `a remounted and laid out again (${laBack.nodes.length} boxes, ${laBack.viewport.w} wide)`);
    hcheck(byTestId(await s.tree(), 'change-station') != null, 'a kept its state across unmount and remount');
    // 3b. A blur is the session's (LLP 1035.001 D5): b edits its search
    // field with the keyboard up; a is tapped on its own ground, which ends
    // a's editing and nobody else's; b is still editing, its inset intact.
    if (host === 'host-ios') {
      s.session = 'b';
      if (byTestId(await s.tree(), 'station-search')) {
        await s.type('station-search', 'Pa');
        // UIKit's keyboard notification lands asynchronously (LLP 1008 §9): poll for the inset.
        let shown = 0;
        for (let i = 0; i < 40 && !(shown > 0); i++) { await sleep(50); shown = (await s.layout()).env['keyboard-inset-height']; }
        s.session = 'a';
        await s.tap('caltrain-main');
        s.session = 'b';
        const kept = (await s.layout()).env['keyboard-inset-height'];
        const sb3 = await s.state();
        hcheck(shown > 0 && kept === shown && sb3.slots.searchFocused === true, `a's blur reached b: keyboard ${shown} → ${kept}, searchFocused ${sb3.slots.searchFocused}`);
        // Keep b's picker and editor for the refusal and destruction checks.
      }
      s.session = 'a';
    }
    // 4. A bad candidate plan is refused; both keep their running apps.
    const bad = resolve(dir, 'bad.plan');
    writeFileSync(bad, 'not an Exact plan');
    say(`apply ${bad}`);
    await settleHost(500);
    hcheck(byTestId(await s.tree(), 'change-station') != null, 'a kept its app after a refused candidate');
    s.session = 'b';
    hcheck(byTestId(await s.tree(), 'station-search') != null, 'b kept its state after a refused candidate');
    const logs = await s.logs();
    hcheck(logs.host.some((l) => /apply .*refused/.test(l)), `the host reported the refusal: ${logs.host.filter((l) => /apply/.test(l)).join(' | ')}`);
    // 5. Destroy a under b: a's handle is refused by name; b still answers.
    say('destroy a');
    await settleHost(500);
    s.session = 'a';
    let refused = null;
    try { await s.tree(); } catch (e) { refused = e.message; }
    hcheck(refused != null && /no such runtime|destroyed/.test(refused), `a's handle is refused by name after destroy: ${refused}`);
    s.session = 'b';
    const tbAfter = await s.tree();
    hcheck(byTestId(tbAfter, 'station-search') != null, 'b still answers, its state intact, after a was destroyed');
    await s.tap('scheme-light');
    await s.clock('settle');
    const logs2 = await s.logs();
    hcheck(logs2.host.some((l) => /destroyed a/.test(l)) && !logs2.host.some((l) => /exact: /.test(l) && !/unknown command/.test(l)), `no late callback or refusal after destroy: ${logs2.host.filter((l) => /exact:|destroyed/.test(l)).join(' | ')}`);
  } finally {
    await s.close();
    rmSync(dir, { recursive: true, force: true });
  }
  if (hostFailures.length) { console.log(`${host} smoke: ${hostFailures.length} failure(s) in ${((Date.now() - t0) / 1000).toFixed(1)} s`); for (const f of hostFailures) console.log('  ' + f); process.exit(1); }
  console.log(`${host} smoke: ok in ${((Date.now() - t0) / 1000).toFixed(1)} s — two sessions of one plan, interleaved, one pushed under a native screen and back, a bad plan refused, one destroyed under the other`);
  process.exit(0);
}

// Materialize the selected app once, as it ships (the JS target when it takes it, LLP 1071;
// it stages nothing), into this smoke's private dist before its many sessions. Another
// build cannot swap a different app under the run; agent.open proves its identity.
if (host === 'web') {
  const webBuild = mkdtempSync(resolve(tmpdir(), 'exact-smoke-web-'));
  selectedWebDist = resolve(webBuild, 'dist');
  process.on('exit', () => rmSync(webBuild, { recursive: true, force: true }));
  const built = spawnSync(process.execPath, [resolve(ROOT, 'host/web/build.mjs'), app.crate('web')], { cwd: ROOT, stdio: 'inherit', env: { ...process.env, EXACT_WEB_DIST: selectedWebDist } });
  if (built.status !== 0) process.exit(built.status ?? 1);
  if (!argv.includes('--app-only')) { // the router sweep swaps plans into a page: the wasm runner's
    fixtureWebDist = resolve(webBuild, 'fixtures');
    const fixtures = spawnSync(process.execPath, [resolve(ROOT, 'host/web/build.mjs'), app.crate('web'), '--wasm'], { cwd: ROOT, stdio: 'inherit', env: { ...process.env, EXACT_WEB_DIST: fixtureWebDist, EXACT_WEB_LINK: 'all' } });
    if (fixtures.status !== 0) process.exit(fixtures.status ?? 1);
  }
}

const s = await open({ host, browser: 'chrome' });
// A web artifact's staged capabilities (LLP 1047.000 §9): the page boots
// without them, and the first agent call loads inspection before it asks.
const stages = async () => host === 'web' && !jsTargetBuild(selectedWebDist) ? JSON.parse(await s.carrier.evaluate('JSON.stringify(exact.stages())')) : {};
const staged = await stages();
check(Object.values(staged).every((state) => state === 'staged'), `the page booted with a stage already loaded: ${JSON.stringify(staged)}`);
let caltrainFixture = false;
let deckFixture = false;
let appCoversViewport = false;
try {
  // 1. Every resolved app must produce a tree and a rendered root. A
  // `caltrain-main` root admits Caltrain's fixture; once admitted, every
  // landmark its later steps need is required rather than silently skipped.
  let tree = await s.tree();
  check(tree.roots?.length > 0 && tree.nodes.length > 0, `${app.name} produced no live roots`);
  if (staged.inspection) check((await stages()).inspection === 'loaded', 'the first agent call did not load the inspection stage');
  caltrainFixture = !!byTestId(tree, 'caltrain-main');
  deckFixture = !!byTestId(tree, 'deck-toggle');
  appCoversViewport = tree.nodes.find((n) => n.id === tree.roots[0])?.props.viewportFit === 'cover';
  const appHasCanvas = tree.nodes.some((n) => n.type === 'Canvas');
  const countdowns = tree.nodes.filter((n) => n.props.testId?.startsWith('countdown-'));

  // 2. Layout is generic: the app's first root reaches the host. Its viewport
  // also grounds the safe-area fixture below. Caltrain then adds its exact
  // block-width and image-ratio assertions.
  let layout = await s.layout();
  appViewport = layout.viewport;
  check(layout.nodes.some((n) => tree.roots.includes(n.id)), `${app.name}'s root has no rendered layout box`);
  if (caltrainFixture) {
    for (const id of ['station-name', 'board-north', 'board-south', 'change-station', 'logo', 'scheme-dark', 'scheme-light', 'aurora', 'aurora-title', 'deck-toggle', 'material-crt']) {
      check(byTestId(tree, id), `Caltrain fixture missing testId ${id}`);
    }
    check(byTestId(tree, 'station-name')?.props.text === 'Mountain View', 'station name not presented');
    check(countdowns.length > 0, 'no countdowns');
    const root = box(layout, 'caltrain-main');
    check(root?.w === layout.viewport.w, `the root is ${root?.w} wide in a ${layout.viewport.w} viewport`);
    let logo = box(layout, 'logo');
    for (let i = 0; i < 40 && !(logo && Math.round(logo.h) === 36); i++) { await sleep(50); logo = box(await s.layout(), 'logo'); }
    check(logo && Math.round(logo.w) === 96 && Math.round(logo.h) === 36, `the logo is ${logo?.w}×${logo?.h}, not 96×36 from its 320×120 ratio`);
    // 2b. `layout <node>` (LLP 1035.002 D1): the runner's half names where
    // each value came from, the host's half the spaces it has; the explained
    // box is the listing's box; a stale id is refused by name.
    const explained = await s.layout('station-name');
    const n = explained.node;
    const listed = box(layout, 'station-name');
    check(n && n.id === byTestId(tree, 'station-name')?.id && n.style && n.space?.viewport, `layout station-name carries no node detail: ${JSON.stringify(explained.node)}`);
    check(n && ['authored', 'inherited', 'initial'].includes(n.style.text_color?.source), `text_color has no source: ${JSON.stringify(n?.style?.text_color)}`);
    check(n && listed && Math.abs(n.space.viewport.x - listed.x) < 0.01 && Math.abs(n.space.viewport.w - listed.w) < 0.01, `the explained box ${JSON.stringify(n?.space?.viewport)} disagrees with the listing ${JSON.stringify(listed)}`);
    check(await s.op({ op: 'layout', id: 999999 }).then(() => false, (e) => /stale node/.test(e.message)), 'a stale node id was not refused by name');
  }

  // 2a. The iframe parity oracle and Apple arm (@ref LLP 1020 M1/M2): load
  // and message enter the runner, the guest joins tree/input, and the
  // screenshot carries its pixels. Loading is host I/O, so poll it. The
  // deck lab is Caltrain's fixture — a driven app without it (EXACT_APP_DIR)
  // skips these steps instead of dying at the tap; unlike the check()
  // assertions above, a missing tap target throws.
  if ((host === 'web' || apple) && byTestId(tree, 'open-deck')) {
    await s.tap('open-deck');
    await s.clock('settle');
    let frameNode, deckState;
    for (let i = 0; i < 40; i++) {
      tree = await s.tree();
      deckState = await s.state();
      frameNode = byTestId(tree, 'deck-frame');
      if (frameNode?.loading === false && deckState.slots.deckLoaded === true && deckState.slots.deckMessage === 'deck-ready') break;
      await sleep(50);
    }
    check(frameNode?.type === 'WebView' && frameNode.url === '/deck/index.html', `the iframe tree node is ${JSON.stringify(frameNode)}`);
    check(frameNode?.loading === false, `the iframe is still loading: ${JSON.stringify(frameNode)}`);
    check(deckState?.slots.deckLoaded === true, `load did not record its flag: ${JSON.stringify(deckState?.slots)}`);
    check(deckState?.slots.deckMessage === 'deck-ready', `message recorded ${JSON.stringify(deckState?.slots.deckMessage)}, not "deck-ready"`);
    // The same-origin guest joins `tree` as an outline (LLP 1020 D4).
    const guest = frameNode?.guest ?? [];
    check(guest.some((g) => g.testId === 'deck-guest') && guest.some((g) => g.id === 'deck-title'),
      `the guest outline is ${JSON.stringify(guest)}`);
    const deckTmp = mkdtempSync(resolve(tmpdir(), 'exact-deck-'));
    try {
      const deckShot = resolve(deckTmp, 'deck.png');
      await s.screenshot(deckShot);
      check(existsSync(deckShot) && readFileSync(deckShot).length > 0, 'the iframe screenshot was not written');
      const image = decodePng(readFileSync(deckShot));
      const deckLayout = await s.layout();
      const frameBox = box(deckLayout, 'deck-frame');
      if (host === 'web') check(frameBox?.w === 300 && frameBox?.h === 150 && frameBox?.hit === true, `the bare iframe's computed box/hit is ${JSON.stringify(frameBox)}, not a hit-testable 300×150`);
      const scale = image.width / deckLayout.viewport.w;
      const region = crop(image, Math.round(frameBox.x * scale), Math.round(frameBox.y * scale), Math.round(frameBox.w * scale), Math.round(frameBox.h * scale));
      let cyan = 0;
      for (let i = 0; i < region.data.length; i += 4) if (region.data[i] < 190 && region.data[i + 1] > 150 && region.data[i + 2] > 190) cyan++;
      check(cyan > 5, `the iframe screenshot has ${cyan} guest-blue pixels in ${region.width}×${region.height}`);
    } catch (error) {
      failures.push(`the iframe parity oracle stopped: ${error.message}`);
    } finally {
      rmSync(deckTmp, { recursive: true, force: true });
    }
    // Structured-clone data narrows to JSON, with no application protocol
    // intercepted by the shared host (@ref LLP 1020 §9).
    await s.tap('deck-frame', { selector: '#deck-status' });
    for (let i = 0; i < 20; i++) {
      deckState = await s.state();
      if (deckState.slots.deckMessage === '{"protocol":"fixture","request":"status"}') break;
      await sleep(25);
    }
    check(deckState.slots.deckMessage === '{"protocol":"fixture","request":"status"}',
      `structured guest data did not reach the app: ${JSON.stringify(deckState.slots.deckMessage)}`);
    // A tap addressed to the iframe enters its guest in-process. Native
    // delivery is script-dispatched and therefore isTrusted:false (D4).
    await s.tap('deck-frame', { selector: '#deck-title' });
    for (let i = 0; i < 20; i++) {
      deckState = await s.state();
      if (deckState.slots.deckMessage === 'deck-tapped') break;
      await sleep(25);
    }
    check(deckState.slots.deckMessage === 'deck-tapped', `guest tap recorded ${JSON.stringify(deckState.slots.deckMessage)}, not "deck-tapped"`);
    if (host === 'web') {
      // A WindowProxy survives cross-origin navigation. The committed src
      // origin remains the authority: the navigated document must not receive
      // delivery of its messages into the app (@ref LLP 1020 D2).
      await s.tap('deck-frame', { selector: '#deck-navigate' });
      for (let i = 0; i < 40; i++) {
        deckState = await s.state();
        if (deckState.slots.deckLoads >= 2 && deckState.slots.deckMessage === 'attacker-navigating') break;
        await sleep(25);
      }
      check(deckState.slots.deckLoads >= 2, `the navigation probe did not load its cross-origin document: ${JSON.stringify(deckState.slots)}`);
      await sleep(100);
      deckState = await s.state();
      check(deckState.slots.deckMessage === 'attacker-navigating', `a guest message crossed navigation: ${JSON.stringify(deckState.slots.deckMessage)}`);
    }
    await s.tap('deck-back');
    tree = await s.tree();
    check(byTestId(tree, 'home-screen'), 'leaving the iframe did not return home');
  }

  let journal;
  if (caltrainFixture) {
    // 3. The clock: a minute later every countdown still shown is one less —
    // sixty timer fires from one seek, and nothing waited.
    const before = new Map(countdowns.map((n) => [n.props.testId, Number(n.props.text)]));
    await s.clock('+60000');
    tree = await s.tree();
    const after = tree.nodes.filter((n) => n.props.testId?.startsWith('countdown-') && before.has(n.props.testId));
    const wrong = after.filter((n) => Number(n.props.text) !== before.get(n.props.testId) - 1).map((n) => `${n.props.testId} ${before.get(n.props.testId)}→${n.props.text}`);
    check(after.length > 0 && wrong.length === 0, `after +60 s every countdown is one less; not: ${wrong.join(', ') || 'none left'}`);
    let state = await s.state();
    check(state.clock === 60000 && state.slots.nowMs === 1787915400000 + 60000, `state after +60 s: clock ${state.clock}, nowMs ${state.slots.nowMs}`);
    journal = await s.logs();
    // A jump stops only after a timer that sends, for its reply to land
    // before the next fires (LLP 1012 §2); the countdown sends nothing, so
    // the seek is one advance that fires sixty, at the seek's end.
    const fired = journal.lines.map((l) => /^t=(\d+) advance → (\d+) timers? fired/.exec(l)).filter((m) => m && Number(m[1]) > 0 && Number(m[1]) <= 60000);
    const firedSum = fired.reduce((n, m) => n + Number(m[2]), 0);
    check(firedSum === 60 && fired.length === 1 && fired[0][1] === '60000', `the journal does not show sixty timers firing from one seek: ${firedSum} across ${fired.length} advances`);

  // 4. One interaction through the host's real input path: change station,
  // search, pick, home.
  await s.tap('change-station');
  tree = await s.tree();
  check(byTestId(tree, 'stations-screen'), 'tapping Change station did not open the stations screen');
  // 4a. The events beyond press and change (LLP 1005 §3), through the
  // host's own paths: a hover highlights the row under the pointer and
  // leaves with it; typing focuses the field; a key reaches it by name.
  if (host !== 'linux') {
    await s.tap('station-sf', { hover: true });
    state = await s.state();
    tree = await s.tree();
    check(state.slots.hoverOn === true && state.slots.hoverId === 'sf' && byTestId(tree, 'station-hot-sf'), `a hover over station-sf: hoverOn ${state.slots.hoverOn}, hoverId ${JSON.stringify(state.slots.hoverId)}, highlighted ${!!byTestId(tree, 'station-hot-sf')}`);
    await s.tap('station-search', { hover: true });
    state = await s.state();
    check(state.slots.hoverOn === false, `the pointer left station-sf: hoverOn ${state.slots.hoverOn}`);
  }
  await s.type('station-search', 'Palo');
  tree = await s.tree();
  check(byTestId(tree, 'station-search')?.props.value === 'Palo', `typing left the field at ${JSON.stringify(byTestId(tree, 'station-search')?.props.value)}`);
  state = await s.state();
  check(state.slots.query === 'Palo', `the query slot did not hear the change (${JSON.stringify(state.slots.query)})`);
  // 2c. `state`'s host sections (LLP 1035.002 D2) are present on every host
  // (Linux says `unavailable`, never nothing); where the host has a focus,
  // the field just typed into is the editor; on iOS the keyboard comes up
  // (its notification lands asynchronously, so poll). Every reply is
  // tagged with the runner's epoch and incarnation (D3).
  check(state.focus && state.keyboard && state.navigation, `state lacks a host section: ${Object.keys(state).join(', ')}`);
  check(Number.isInteger(state.epoch) && Number.isInteger(state.incarnation), `state is untagged: epoch ${state.epoch}, incarnation ${state.incarnation}`);
  check(Number.isInteger((await s.layout()).epoch), 'the layout reply is untagged');
  if (host !== 'linux') {
    const field = byTestId(tree, 'station-search')?.id;
    check(state.focus.editor === field && state.focus.logical === field, `the typed field is not the focus: ${JSON.stringify(state.focus)}`);
    if (host === 'ios') {
      for (let i = 0; i < 40 && !state.keyboard.visible; i++) { await sleep(50); state = await s.state(); }
      check(state.keyboard.visible === true && state.keyboard.overlap > 0, `the keyboard is not up: ${JSON.stringify(state.keyboard)}`);
    }
    check(state.slots.searchFocused === true, `typing did not focus the field (searchFocused ${state.slots.searchFocused})`);
    await s.type('station-search', { key: 'Enter' });
    state = await s.state();
    tree = await s.tree();
    check(state.slots.lastKey === 'Enter' && byTestId(tree, 'search-hint')?.props.text === 'searching · last key Enter', `Enter at the field: lastKey ${JSON.stringify(state.slots.lastKey)}, hint ${JSON.stringify(byTestId(tree, 'search-hint')?.props.text)}`);
    check(byTestId(tree, 'station-search')?.props.value === 'Palo', `Enter changed the field's text to ${JSON.stringify(byTestId(tree, 'station-search')?.props.value)}`);
  }
  const matches = tree.nodes.filter((n) => n.type === 'Pressable' && n.props.testId?.startsWith('station-'));
  check(matches.length === 1 && matches[0].props.testId === 'station-paloalto', `the search shows ${matches.map((m) => m.props.testId).join(', ') || 'nothing'}, not station-paloalto alone`);
  await s.tap('station-paloalto');
  tree = await s.tree();
  check(byTestId(tree, 'station-name')?.props.text === 'Palo Alto', `after picking Palo Alto the station is ${byTestId(tree, 'station-name')?.props.text}`);
  check(byTestId(tree, 'home-screen'), 'picking a station did not return home');
  // 4c. A held contact on the simulator (LLP 1035.003 §3, candidate 1): a
  // desktop pointer into the Simulator's window, mapped by the driver; a
  // finger down on the title, dragged up while the page is read
  // mid-gesture, held still, lifted. A machine without Accessibility for
  // this terminal reports `unsupported` with the reason — printed, not
  // failed and not faked — so the step stays visibly unverified there.
  if (host === 'ios') {
    const down = await s.tap('station-name', { down: true });
    if (down.delivery === 'unsupported') {
      console.log(`ios contact: unsupported — ${down.reason}`);
    } else {
      await s.pointer('move', { dx: 0, dy: -200, ms: 200 });
      const held = await s.layout();
      const page = held.nodes.find((n) => n.type === 'ScrollView' && n.sy != null);
      check(page && page.sy > 50, `a held drag of 200 up scrolled the page by ${page?.sy} (delivery ${down.delivery}, desktop ${JSON.stringify(down.desktop)})`);
      await s.pointer('hold', { ms: 300 });
      const up = await s.pointer('up');
      check(up.delivery === 'platform' && !s.contact, 'the simulator contact was not released');
      const after = (await s.layout()).nodes.find((n) => n.type === 'ScrollView' && n.sy != null);
      if (after?.sy) await s.tap(after.id, { wheel: [0, -after.sy] });
    }
  }
  // 4b. A held contact (LLP 1035.003 D1) on the AppKit carrier: the button
  // goes down on Change station, leaves it, and is cancelled — on a mouse a
  // release where the pointer is (de13a93f), outside the button, so nothing
  // navigates; down and up in place navigates. Any refusal here is recorded
  // and the rest of the smoke still runs: one pointer error must not throw
  // away every other finding.
  if (host === 'macos') try {
    const down = await s.tap('change-station', { down: true });
    if (check(down.delivery === 'platform' && s.contact, `a contact went down as ${down.delivery}${down.reason ? `: ${down.reason}` : ''}`)) {
      await s.pointer('move', { dx: 0, dy: 300, ms: 50 });
      const cancel = await s.pointer('cancel');
      check(cancel.delivery === 'platform' && !s.contact, `the AppKit cancel did not release the contact (delivery ${cancel.delivery})`);
      tree = await s.tree();
      check(!byTestId(tree, 'stations-screen') && !s.contact, 'a press released outside its button still opened the stations screen');
      await s.tap('change-station', { down: true });
      await s.pointer('hold', { ms: 50 });
      await s.pointer('up');
      tree = await s.tree();
      check(byTestId(tree, 'stations-screen'), 'a contact down and up in place did not press the button');
      await s.tap('station-paloalto');
      tree = await s.tree();
      check(byTestId(tree, 'home-screen'), 'the contact step did not return home');
    }
  } catch (error) {
    check(false, `the AppKit held contact: ${error.message}`);
    if (s.contact) await s.pointer('up').catch(() => {});
    tree = await s.tree();
    if (byTestId(tree, 'stations-screen')) await s.tap('station-paloalto');
  }

  // 5a. A command (LLP 1005 §3): `setScheme` reaches the host as an op and
  // sets its colour scheme; the journal records it and the host reports no
  // error (step 7 reads both). Back to light for the pictures below. Before
  // the scroll below: a platform tap needs its button on screen.
  await s.tap('scheme-dark');
  await s.tap('scheme-light');

  // 5. Scrolling (LLP 1010): a wheel over the content moves it, and exactly
  // one scroll container takes it — the node when it can, else the page.
  layout = await s.layout();
  const nameBefore = box(layout, 'station-name').y;
  const inner0 = layout.nodes.find((n) => n.type === 'ScrollView');
  await s.tap('station-name', { wheel: [0, 300] });
  layout = await s.layout();
  const moved = nameBefore - box(layout, 'station-name').y;
  const inner = layout.nodes.find((n) => n.id === inner0.id);
  const innerMoved = inner.sy > inner0.sy, pageMoved = box(layout, 'caltrain-main').y < 0;
  check(moved === 300, `a wheel of 300 over the content scrolled it by ${moved}`);
  check(innerMoved !== pageMoved, `one scroll container takes a wheel: inner moved ${innerMoved}, page moved ${pageMoved}`);

  }

  // 6. The pixels when asked, and the GPU module where the host renders it
  // (a canvas is on the page; headless Chrome has WebGPU).
  if (shot) console.log(JSON.stringify(await s.screenshot(shot)));
  if (host === 'web' && appHasCanvas) {
    let g = await s.gpuMs();
    for (let i = 0; i < 60 && g == null; i++) { await sleep(50); g = await s.gpuMs(); }
    check(g != null, 'a canvas is on the page but the GPU module did not load (WebGPU unavailable in this Chrome?)');
    if (g != null) console.log(`gpu: module loaded ${g} ms after injection (after a rendering opportunity)`);
  }

  if (caltrainFixture) {
    // 6a. A canvas's children (LLP 1014 D1): laid out by the kernel in the
    // canvas's box and presented over its surface on every host — the aurora's
    // title is the station's name, and its box lies inside the aurora's.
    tree = await s.tree();
    check(byTestId(tree, 'aurora-title')?.props.text === byTestId(tree, 'station-name')?.props.text, `the aurora's title is ${JSON.stringify(byTestId(tree, 'aurora-title')?.props.text)}, not the station's name`);
    layout = await s.layout();
    const aurora = box(layout, 'aurora'), title = box(layout, 'aurora-title');
    const inside = aurora && title && title.x >= aurora.x - 0.5 && title.y >= aurora.y - 0.5 && title.x + title.w <= aurora.x + aurora.w + 0.5 && title.y + title.h <= aurora.y + aurora.h + 0.5;
    check(inside, `the aurora's title ${JSON.stringify(title)} is not inside the aurora ${JSON.stringify(aurora)}`);
  }

  // 7. The journal: a boot, the presses, the change, the timers — no refusal,
  // no host error.
  journal ??= await s.logs();
  const logs = await s.logs();
  const lines = [...journal.lines, ...logs.lines];
  check(lines[0]?.includes('boot: '), 'no boot line in the journal');
  check(journal.dropped === 0 && logs.dropped === 0, 'the journal ring dropped lines in a short run');
  const bad = lines.filter((l) => /refused|poisoned/.test(l));
  check(bad.length === 0, 'refusals in the journal:\n    ' + bad.join('\n    '));
  const hostBad = [...journal.host, ...logs.host].filter((l) => /exact:|error|exception/i.test(l));
  check(hostBad.length === 0, 'the host reported errors:\n    ' + hostBad.join('\n    '));
  console.log(`${host}: boot ${s.boot.toFixed(1)} ms; ${tree.nodes.length} nodes; ${lines.length} journal lines`);
} catch (error) {
  // A driver refusal is one finding; the rest of the smoke still runs.
  failures.push(`the ${app.name} drive stopped: ${error.message}`);
} finally {
  await s.close();
}

// 8. The nested case (LLP 1010, the scroll fixture): a scroll node that
// overflows on a page that overflows. A wheel over the node scrolls it; at
// its edge the wheel chains to the page.
// A paired module client cannot boot unrelated bare plans. --app-only keeps
// the complete app drive and its Contract tests, excluding host-only fixtures.
if (!argv.includes('--app-only')) {
// Launch facts reach a real runner on every carrier, including Linux's t() table.
{
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-place-'));
  const source = resolve(tmp, 'app.contract'), plan = resolve(tmp, 'app.plan');
  mkdirSync(resolve(tmp, 'strings'));
  writeFileSync(resolve(tmp, 'strings/en.json'), '{"greeting":"Hello"}');
  writeFileSync(resolve(tmp, 'strings/fr.json'), '{"greeting":"Bonjour"}');
  writeFileSync(resolve(tmp, 'strings/ar.json'), '{"greeting":"مرحبا"}');
  writeFileSync(source, 'shape Time\n  locale: string\n  resolvedLocale: string\n  timeZone: string\n  seed: number\n  epochAtZero: number\n  utcOffset: number\ncomponent App\n  resource time = exactTime() as shape Time\n  state minute = 0\n  action tick\n    minute = time.epochAtZero + now()\n  task minutes mount\n    every(60000, tick)\n  view\n    column\n      text `${time.locale}|${time.resolvedLocale}|${time.timeZone}|${time.seed}` testId="place"\n      text `${time.epochAtZero}|${time.utcOffset}|${minute}` testId="date"\n      text t("greeting") testId="greeting"\n');
  const compiled = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', source, '-o', plan], {cwd:ROOT, encoding:'utf8'});
  check(compiled.status === 0, 'launch facts fixture compiles: ' + compiled.stderr);
  if (compiled.status === 0) for (const options of [{}, {}, {seed:42, locale:'fr-CA', timeZone:'America/Toronto', epoch:'2026-09-21T14:13:20Z'}, {seed:42, locale:'ar-EG', timeZone:'UTC'}]) {
    const f = await open({host, browser: 'chrome', plan, ...options});
    try {
      const lang = options.locale === 'ar-EG' ? 'ar' : options.locale ? 'fr' : 'en';
      const dir = lang === 'ar' ? 'rtl' : 'ltr';
      const expected = `${options.locale ?? 'en-US'}|${lang}|${options.timeZone ?? 'UTC'}|${options.seed ?? 1}`;
      const tree = await f.tree();
      check(byTestId(tree, 'place')?.props.text === expected, `launch facts: expected ${expected}, got ${byTestId(tree, 'place')?.props.text}`);
      // LLP 1027.000.000 D3: the drive's epoch and its zone's offset; the agent clock moves the date.
      const [epoch, offset] = options.epoch ? [1790000000000, -240] : [1767225600000, 0];
      check(byTestId(tree, 'date')?.props.text === `${epoch}|${offset}|0`, `launch date: expected ${epoch}|${offset}|0, got ${byTestId(tree, 'date')?.props.text}`);
      await f.clock('+60000');
      check(byTestId(await f.tree(), 'date')?.props.text === `${epoch}|${offset}|${epoch + 60000}`, 'the agent clock moves the date');
      check(byTestId(tree, 'greeting')?.props.text === ({en:'Hello', fr:'Bonjour', ar:'مرحبا'}[lang]), 'launch locale selects the translation table');
      const language = (await f.state()).language;
      check(language.lang === lang && language.dir === dir, 'resolved language and direction reach the host');
      if (host === 'web') {
        const attrs = await f.carrier.evaluate('({lang:document.documentElement.lang,dir:document.documentElement.dir})');
        check(attrs.lang === lang && attrs.dir === dir, 'the document element carries lang and dir');
        if (jsTargetBuild(selectedWebDist)) { // a JS page (the plan compiled ahead) reloads, its launch facts in its URL
          await f.carrier.evaluate('location.reload()').catch(() => {});
          for (let i = 0; i < 200 && !(await f.carrier.evaluate("document.readyState === 'complete' && document.getElementById('exact-root')?.dataset.bootMs != null && !!globalThis.exact?.agentSettled").catch(() => false)); i++) await sleep(25);
          await f.carrier.evaluate('exact.ready'); } else await f.carrier.evaluate('fetch("/__plan").then(r => r.arrayBuffer()).then(b => exact.reload(new Uint8Array(b)))');
        check(byTestId(await f.tree(), 'place')?.props.text === expected, 'web reload retains launch facts');
        const attrsAfter = await f.carrier.evaluate('({lang:document.documentElement.lang,dir:document.documentElement.dir})');
        check(attrsAfter.lang === lang && attrsAfter.dir === dir, 'web reload retains lang and dir');
      }
    } catch (error) { check(false, `launch facts fixture: ${error.message}`); }
    finally { await f.close(); }
  }
  rmSync(tmp, {recursive:true, force:true});
  console.log(`${host} launch facts: date, defaults, repeated drive, overrides, translation, lang/dir${host === 'web' ? ', reload' : ''}`);
}

const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
const plan = resolve(tmp, 'scroll.plan');
const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/scroll.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
if (c.status !== 0) failures.push('the scroll fixture did not compile: ' + c.stderr);
else {
  const f = await open({ host, browser: 'chrome', plan });
  try {
    let l = await f.layout();
    check(box(l, 'rows') && box(l, 'root'), 'the fixture did not boot');
    // 8a. A held contact on the web (LLP 1035.003 D1): a finger down on a
    // row, dragged up while the rows are read mid-gesture, held still so
    // there is no fling, then lifted; the wheel steps below start from the
    // top again. Chrome's own touch scrolling, through CDP touch events.
    if (host === 'web') {
      const down = await f.tap('row-1', { down: true });
      check(down.delivery === 'platform', `a contact went down as ${down.delivery}`);
      await f.pointer('move', { dx: 0, dy: -100, ms: 100 });
      const held = box(await f.layout(), 'rows');
      check(held.sy > 50, `a held drag of 100 up scrolled the rows by ${held.sy}`);
      await f.pointer('hold', { ms: 200 });
      const up = await f.pointer('up');
      check(up.delivery === 'platform' && !f.contact, 'the contact was not released');
      await f.tap('rows', { wheel: [0, -box(await f.layout(), 'rows').sy] });
      check(box(await f.layout(), 'rows').sy === 0, 'the rows did not return to the top after the contact');
    }
    await f.tap('row-1', { wheel: [0, 100] });
    l = await f.layout();
    check(box(l, 'rows').sy === 100, `the scroll node took a wheel of 100: scrolled ${box(l, 'rows').sy}`);
    check(box(l, 'root').y === 0, 'the page moved for a wheel the scroll node consumed');
    for (let i = 0; i < 12; i++) await f.tap('rows', { wheel: [0, 400] });
    l = await f.layout();
    const limit = box(l, 'rows').sy;
    // 652 is a parity number: the fixture's rows under each host's text
    // metrics. If it moves on one host, the hosts have parted.
    check(limit === 652, `the scroll node stops at ${limit}, not 652 (the same limit on both hosts)`);
    check(box(l, 'root').y < 0, `a scroll node at its edge (${limit}) did not chain to the page (root at ${box(l, 'root').y})`);
    check(box(l, 'rows').sy === limit, 'the scroll node moved past its edge');
    console.log(`${host} fixture: the scroll node stops at ${limit}, then the page scrolls (root at ${box(l, 'root').y})`);
  } catch (error) {
    // A driver refusal is one finding; the rest of the smoke still runs.
    failures.push(`the scroll fixture stopped: ${error.message}`);
  } finally {
    await f.close();
  }
}
rmSync(tmp, { recursive: true, force: true });

// 9. Children through the surface (LLP 1014, Caltrain's GPU fixture): a button
// and an input inside a canvas whose surface samples its children. Both are
// laid out in the canvas's box; a tap on the button reaches it — on macOS
// through the alpha-0 overlay (D5) — and typing reaches the field; on macOS
// the presenter captures the canvas for the batch and while the field is
// edited (D4 a, d), which agent mode reports in the logs.
if (caltrainFixture) {
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
  const plan = resolve(tmp, 'canvas.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/canvas.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) failures.push('the canvas fixture did not compile: ' + c.stderr);
  else {
    // On Linux the picture is the oracle painter's (tiny-skia; LLP 1015 §2)
    // over the pinned font the driver sets (§5): the same bytes on every
    // machine, so the reference below is exact, not a band — the GPU painter
    // is held to it by `tests/paint.rs`'s band instead.
    const f = await open({ host, browser: 'chrome', plan, env: host === 'linux' ? { EXACT_PAINTER: 'cpu' } : undefined });
    try {
      let t = await f.tree();
      check(byTestId(t, 'sky-zoom') && byTestId(t, 'sky-label'), 'the canvas fixture did not boot');
      const l = await f.layout();
      const sky = box(l, 'sky'), zoom = box(l, 'sky-zoom');
      check(sky && zoom && zoom.y >= sky.y - 0.5 && zoom.y + zoom.h <= sky.y + sky.h + 0.5, `the button ${JSON.stringify(zoom)} is not inside the canvas ${JSON.stringify(sky)}`);
      let logs = '';
      if (apple) {
        // The module loads after the first paint; the first capture puts the
        // overlay at alpha 0 — the tap below must go through it. The first
        // dlopen of a freshly built dylib can take seconds (macOS checks a
        // new binary on its first load), so the poll is long; it stops at
        // the first capture, which is ~200 ms in the usual case.
        for (let i = 0; i < 200 && !/captured/.test(logs); i++) { await sleep(50); logs += JSON.stringify(await f.logs()); }
        check(/captured/.test(logs), 'the canvas fixture was never captured within 10 s (did the surface want its children?)');
      }

      // 10. The readback (LLP 1014 §2 step 3, LLP 1009 D1): the canvas as
      // this host composes it — the sky at clock 0 with its children through
      // it on macOS, over it on the web — cropped from the screenshot by the
      // canvas's box and held against this host's recorded reference,
      // scripts/fixtures/canvas-sky.<host>.png (`--record-canvas` rewrites
      // it). The clock is the agent's, so the sky is the same picture every
      // run; the band is for the GPU's arithmetic (on Linux the picture is
      // the CPU oracle's with the pinned font, and matches to the pixel on
      // any machine). Taken before the tap and the edit: a caret blinks on
      // the wall clock.
      if (host === 'web') { let g = await f.gpuMs(); for (let i = 0; i < 60 && g == null; i++) { await sleep(50); g = await f.gpuMs(); } }
      await sleep(150); // one frame of the surface after its first capture
      const shotPath = resolve(tmp, 'canvas.png');
      await f.screenshot(shotPath, true);
      const image = decodePng(readFileSync(shotPath));
      const scale = image.width / l.viewport.w;
      // iOS's screen also has a bottom safe area; use its reported viewport
      // origin (LLP 1035.002 D4), not all pixels outside the viewport as a title bar.
      const top = host === 'ios' ? Math.round(l.screen.y * scale) : image.height - Math.round(l.viewport.h * scale);
      const region = crop(image, Math.round(sky.x * scale), top + Math.round(sky.y * scale), Math.round(sky.w * scale), Math.round(sky.h * scale));
      // The picture depends on the screen: a 1x display (a Mac on a plain
      // monitor) and an iPad's wider canvas are other pictures, not worse
      // ones. The plain name is the reference at its own size; another
      // size has its own, named by its pixels.
      const plainRef = resolve(ROOT, `scripts/fixtures/canvas-sky.${host}.png`);
      const plainSize = existsSync(plainRef) ? decodePng(readFileSync(plainRef)) : null;
      const reference = !plainSize || (plainSize.width === region.width && plainSize.height === region.height) ? plainRef
        : resolve(ROOT, `scripts/fixtures/canvas-sky.${host}.${region.width}x${region.height}.png`);
      if (recordCanvas) { writeFileSync(reference, encodePng(region)); console.log(`recorded ${reference.replace(ROOT + '/', '')} (${region.width}×${region.height})`); }
      else if (!existsSync(reference)) failures.push(`no reference picture ${reference.replace(ROOT + '/', '')}: bun scripts/smoke.mjs ${host} --record-canvas`);
      else {
        const d = diff(region, decodePng(readFileSync(reference)));
        check(d.differing <= 0.01, `the canvas differs from its reference: ${(d.differing * 100).toFixed(2)}% of pixels beyond the band, mean ${d.mean.toFixed(2)} (${d.size})`);
        console.log(`${host} readback: the canvas matches its reference — ${(d.differing * 100).toFixed(2)}% beyond the band, mean ${d.mean.toFixed(2)} (${d.size})`);
      }

      await f.tap('sky-zoom');
      const st = await f.state();
      check(st.slots.zoom === 2, `a tap on a button inside the canvas did not reach it (zoom ${JSON.stringify(st.slots.zoom)})`);
      await f.type('sky-label', 'aurora');
      t = await f.tree();
      check(byTestId(t, 'sky-label')?.props.value === 'aurora', `typing into an input inside the canvas left it at ${JSON.stringify(byTestId(t, 'sky-label')?.props.value)}`);
      if (apple) {
        await sleep(100);
        logs += JSON.stringify(await f.logs());
        const captures = (logs.match(/canvas \d+: captured/g) ?? []).length;
        check(captures >= 3, `the presenter captured the canvas ${captures} time(s); expected the first, the tap's batch, and the edit`);
        console.log(`${host} fixture: the canvas captured ${captures} times`);
      }
    } catch (error) {
      failures.push(`the canvas fixture stopped: ${error.message}`);
    } finally {
      await f.close();
    }
  }
  rmSync(tmp, { recursive: true, force: true });
}

// 11. The deck and the materials (LLP 1014 §1a, §1b): `Deck` opens a canvas
// whose cards are the kernel's buttons — placed by the surface on macOS, a
// column over the surface on the web and on Linux (D2) — and a tap on a
// card focuses it. Placements settle before each reply; the canvas centre
// initially reaches no card. The agent clock moves springs deterministically.
if (deckFixture) {
    const d = await open({ host, browser: 'chrome' });
  try {
    const reveal = async (id, at) => { const l = await d.layout(), b = box(l, id), mid = b.y + b.h / 2; if (mid < 0 || mid >= l.viewport.h) await d.tap(at, { wheel: [0, mid - l.viewport.h / 2] }); }; // a tap outside the viewport is refused (on a phone: the toggle, the opened deck, then the material buttons above it): a wheel at `at` scrolls `id`'s middle to the viewport's
    await reveal('deck-toggle', 'station-name');
    await d.tap('deck-toggle');
    let st = await d.state();
    check(st.slots.deck === true, `the deck did not open (${JSON.stringify(st.slots.deck)})`);
    await reveal('deck', 'deck-toggle');
    let l = await d.layout();
    const cards = l.nodes.filter((n) => n.testId?.startsWith('card-'));
    check(cards.length >= 2, `the deck has ${cards.length} card(s)`);
    if (apple) {
      const deck = box(l, 'deck');
      const onCanvas = cards.filter((c) => c.x < deck.x + deck.w && c.x + c.w > deck.x);
      check(onCanvas.length > 0 && onCanvas.length < cards.length, `${onCanvas.length} of ${cards.length} cards placed on the canvas (a closed deck shows a few; the rest are off it)`);
      const low = onCanvas.filter((c) => c.y + c.h > deck.y + deck.h / 2);
      check(low.length === 0, `${low.length} card(s) placed in the lower half of a closed deck: ${low.slice(0, 2).map((c) => `${c.testId} y=${c.y} h=${c.h}`).join(', ')}`);
      await d.tap('deck');
      st = await d.state();
      check(st.slots.focus === null, `a tap on the canvas's middle, outside every placed card, focused ${JSON.stringify(st.slots.focus)} — a kernel frame was hit`);
    }
    // The front card: a tap lands on the middle of a card's box as seen, and
    // on a closed deck every card but the first is mostly behind the one in
    // front of it, which is what the tap then reaches (the browser's rule).
    await d.tap(cards[0].testId);
    st = await d.state();
    check(st.slots.focus === cards[0].testId.slice(5), `a tap on ${cards[0].testId} focused ${JSON.stringify(st.slots.focus)}`);
    // One frame of the clock moves the springs, and the reply carries the
    // new placements (LLP 1014.000 §1c: `clock` settles the canvases — the
    // nested deck is read back into the sky's capture before the reply, not
    // when a later redraw happens to ask).
    await d.clock('+100');
    l = await d.layout();
    if (apple) {
      const later = l.nodes.filter((n) => n.testId?.startsWith('card-'));
      const moved = later.filter((c, i) => cards[i] && Math.abs(c.y - cards[i].y) > 1).length;
      check(moved > 0, 'a tenth of a second on, no card moved: the deck was not read back for the clock (placements refresh late)');
    }
    await reveal('material-crt', 'deck'); await d.tap('material-crt');
    st = await d.state();
    check(st.slots.material === 'crt', `the material is ${JSON.stringify(st.slots.material)}`);
    console.log(`${host} deck: ${cards.length} cards, ${cards[0].testId} focused by a tap; material crt`);
  } catch (error) {
    // A driver refusal is one finding; the rest of the smoke still runs.
    failures.push(`the deck fixture stopped: ${error.message}`);
  } finally {
    await d.close();
  }
}

// 11. Motion under the clock (LLP 1012 §2, the motion fixture): three boxes
// scale 1 → 2 — a 250 ms linear transition and a spring on a press, a 500 ms
// linear transition when a timer fires at t = 1000. Nothing plays between
// operations; a seek lands on the curve; `settle` is a fixed point that
// crosses the timer; and one seek across the timer gives what stepping
// across it gives (LLP 1002 D3) — on both hosts, the same numbers.
{
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
  const plan = resolve(tmp, 'motion.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/motion.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) failures.push('the motion fixture did not compile: ' + c.stderr);
  else {
    const w = (l, id) => box(l, id)?.w;
    const near = (a, b, tol = 0.05) => Math.abs(a - b) <= tol;
    const m = await open({ host, browser: 'chrome', plan });
    try {
      await m.tap('toggle');
      // Frozen: the press started two transitions; a wall-clock pause between
      // two reads changes nothing (this sleep tests that nothing moves — it
      // is not a wait for anything).
      const a = await m.layout();
      await sleep(300);
      const b = await m.layout();
      check(w(a, 'linear') === 50 && w(a, 'spring') === 50 && w(b, 'linear') === 50 && w(b, 'spring') === 50, `after a press the boxes sit at local time 0: ${w(a, 'linear')}, ${w(a, 'spring')} then ${w(b, 'linear')}, ${w(b, 'spring')}`);
      await m.clock('+125');
      let l = await m.layout();
      check(w(l, 'linear') === 75, `at 125 ms of a 250 ms linear scale 1→2 the box is ${w(l, 'linear')} wide, not 75`);
      check(near(w(l, 'spring'), 86.55, 0.5), `at 125 ms the spring(180, 12, 1) box is ${w(l, 'spring')} wide (both hosts: 86.55)`);
      check(w(l, 'timed') === 50, `the timer has not fired yet: ${w(l, 'timed')}`);
      await m.clock('+125');
      l = await m.layout();
      check(w(l, 'linear') === 100, `at 250 ms the linear box is ${w(l, 'linear')} wide, not 100`);
      const settled = await m.clock('settle');
      l = await m.layout();
      // Two rounds: the spring settles at 1295.8 ms (the same on both hosts), the
      // seek there crosses the timer at 1000, whose transition ends at 1500.
      check(settled.settled === true && settled.clock === 1500, `settle: ${JSON.stringify(settled)} (expected a fixed point at 1500: the spring's 1295.8, then the timer's transition)`);
      check(w(l, 'spring') === 100 && w(l, 'timed') === 100, `after settle the spring box is ${w(l, 'spring')} and the timer's ${w(l, 'timed')}; both should be 100`);
    } catch (error) {
      failures.push(`the motion fixture stopped: ${error.message}`);
    } finally {
      await m.close();
    }
    // One seek across the timer versus stepping across it: the transition is
    // born at the timer's due time either way.
    const once = await open({ host, browser: 'chrome', plan });
    let oneShot;
    try { await once.clock(1250); oneShot = [w(await once.layout(), 'timed')]; await once.clock(1500); oneShot.push(w(await once.layout(), 'timed')); } finally { await once.close(); }
    const steps = await open({ host, browser: 'chrome', plan });
    let stepwise;
    try { await steps.clock(1000); stepwise = [w(await steps.layout(), 'timed')]; await steps.clock(1250); stepwise.push(w(await steps.layout(), 'timed')); await steps.clock(1500); stepwise.push(w(await steps.layout(), 'timed')); } finally { await steps.close(); }
    check(oneShot[0] === 75 && oneShot[1] === 100, `one seek to 1250 then 1500 across the timer: ${oneShot.join(', ')} (expected 75, 100)`);
    check(stepwise[0] === 50 && stepwise[1] === 75 && stepwise[2] === 100, `stepping 1000, 1250, 1500: ${stepwise.join(', ')} (expected 50, 75, 100)`);
    console.log(`${host} motion: linear 75 at 125 ms, spring in flight, settle a fixed point; across the timer one seek = steps (${oneShot.join('/')} vs ${stepwise.slice(1).join('/')})`);
  }
  rmSync(tmp, { recursive: true, force: true });
}

// 11b. Gesture precedence (LLP 1057.001 §1; the phase-0 exit evidence of LLP
// 1057.000, apparatus approved by Charlie, 2026-09-27): one contact on a
// swipe row inside a panning surface. A horizontal drag is the inner swipe's
// and the pan never fires; a vertical one falls to the pan; a drag that starts
// on the row's button is neither's (rule 3's boundary); a tap presses it. The
// same numbers on every host that can hold a contact; iOS without the desktop
// pointer answers `unsupported`, said so, not faked.
{
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
  const plan = resolve(tmp, 'precedence.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/precedence.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) failures.push('the precedence fixture did not compile: ' + c.stderr);
  else {
    const g = await open({ host, browser: 'chrome', plan });
    try {
      const slots = async () => { const { slots } = await g.state(); return `${slots.replies}/${slots.panned}/${slots.pressed}`; };
      const drag = async (target, at, moves) => {
        const down = await g.tap(target, { down: true, at });
        if (down.delivery === 'unsupported') return false;
        // A carrier may end a contact nothing took (Linux does): no more phases then.
        for (const [dx, dy] of moves) if (g.contact) await g.pointer('move', { dx, dy });
        if (g.contact) await g.pointer('up');
        await g.clock('settle');
        return true;
      };
      if (!await drag('row', [200, 50], [[20, 0], [100, 0]])) {
        console.log(`${host} precedence: unsupported (this carrier holds no contact); unverified here`);
      } else {
        check(await slots() === '1/0/0', `a horizontal drag on the row: replies/panned/pressed ${await slots()}, expected the swipe alone (1/0/0)`);
        await drag('row', [200, 50], [[0, 10], [0, 20]]);
        check(await slots() === '1/30/0', `a vertical drag on the row: ${await slots()}, expected the surface's pan (1/30/0)`);
        await drag('button', [40, 20], [[20, 0], [100, 0]]);
        check(await slots() === '1/30/0', `a drag from the row's button: ${await slots()}, expected neither the swipe nor the pan (1/30/0)`);
        await g.tap('button');
        await g.clock('settle');
        check(await slots() === '1/30/1', `a tap on the button: ${await slots()}, expected its press (1/30/1)`);
        console.log(`${host} precedence: swipe inside pan ${await slots()} (replies/panned/pressed)`);
      }
    } catch (error) {
      failures.push(`the precedence fixture stopped: ${error.message}`);
    } finally {
      await g.close();
    }
  }
  rmSync(tmp, { recursive: true, force: true });
}

// 12. The page's environment (LLP 1008 §9, the insets fixture): a root that
// says `viewport-fit="cover"` is laid out to the whole screen, its content
// kept out of the safe areas by `env(safe-area-inset-*)` lengths — on a
// phone the viewport is the app's (step 2, the safe area) plus the insets.
// A macOS cover plan's full-size-content window reports its titlebar as the
// top safe area; web and Linux report zero. Focusing the input
// at the bottom: on iOS the software keyboard rises, the viewport insets
// itself by the keyboard's height and reveals the field above it, the layout
// viewport untouched — a browser's visual viewport; a tap on the dismiss
// button takes the focus, and the keyboard goes. `layout.env` reports both,
// by the web's `env()` names, on every host.
{
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
  const plan = resolve(tmp, 'insets.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/insets.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) failures.push('the insets fixture did not compile: ' + c.stderr);
  else {
    const f = await open({ host, browser: 'chrome', plan });
    try {
      let l = await f.layout();
      const env = l.env ?? {};
      const names = ['safe-area-inset-top', 'safe-area-inset-right', 'safe-area-inset-bottom', 'safe-area-inset-left', 'keyboard-inset-height'];
      check(names.every((k) => typeof env[k] === 'number'), `layout.env is ${JSON.stringify(l.env)}`); check(['continuous', 'folded'].includes(env['device-posture']) && Number.isInteger(env['horizontal-viewport-segments']) && Number.isInteger(env['vertical-viewport-segments']) && Array.isArray(env['viewport-segments']), `layout.env reports the fold by its four names (LLP 1076 D7): ${JSON.stringify(l.env)}`);
      const [top, right, bottom, left] = names.map((k) => env[k] ?? 0);
      const viewport0 = l.viewport;
      const rootBox = box(l, 'root'), content = box(l, 'content');
      check(rootBox && rootBox.w === l.viewport.w && rootBox.h === l.viewport.h, `a cover root fills the viewport: ${JSON.stringify(rootBox)} in ${JSON.stringify(l.viewport)}`);
      check(content && content.x === left && content.y === top && Math.abs(content.w - (l.viewport.w - left - right)) < 0.01 && Math.abs(content.h - (l.viewport.h - top - bottom)) < 0.01, `the content keeps out of the insets: ${JSON.stringify(content)} for env ${JSON.stringify(env)} in ${JSON.stringify(l.viewport)}`);
      if (host === 'ios') {
        const extraH = appCoversViewport ? 0 : top + bottom, extraW = appCoversViewport ? 0 : left + right;
        check(appViewport && Math.abs(l.viewport.h - (appViewport.h + extraH)) < 0.01 && Math.abs(l.viewport.w - (appViewport.w + extraW)) < 0.01, `a phone cover viewport matches the app's viewport-fit: ${JSON.stringify(l.viewport)} vs ${JSON.stringify(appViewport)}, cover=${appCoversViewport}, insets ${top}/${right}/${bottom}/${left}`);
        check(top > 0 && bottom > 0, `a phone reports its status bar and home indicator: ${top}, ${bottom}`);
      } else if (host === 'macos') {
        // A cover plan's content is its window's whole frame, the titlebar its
        // top inset. Read in this window (a resize to its own size reports the
        // frame): agent windows are spread by pid and AppKit fits each to the
        // screen, so the app's window is not this one's size.
        const w = await f.op({ op: 'tap', resize: [l.viewport.w, l.viewport.h] });
        const [fw, fh] = w.windowFrame ?? [], lh = w.contentLayout?.[1];
        check(Math.abs(l.viewport.w - fw) < 0.01 && Math.abs(l.viewport.h - fh) < 0.01 && Math.abs(top - (fh - lh)) < 0.01, `a macOS cover viewport is its window's frame, the titlebar its top inset: viewport ${JSON.stringify(l.viewport)}, frame ${fw}×${fh}, content layout height ${lh}, top ${top}`);
        check(top > 0 && right === 0 && bottom === 0 && left === 0, `macOS reports only its titlebar safe area: ${JSON.stringify(env)}`);
      } else {
        check(top === 0 && right === 0 && bottom === 0 && left === 0, `no safe area here: ${JSON.stringify(env)}`);
      }
      // The keyboard: typing focuses the field at the bottom.
      const noteBefore = box(l, 'note');
      await f.type('note', 'hi');
      let st = await f.state();
      // (The Linux host's `type` sets the field without focusing it, as step 4a notes.)
      check(st.slots.note === 'hi' && (host === 'linux' || st.slots.focused === true), `typing focused the field and set it: ${JSON.stringify(st.slots)}`);
      let kb = 0;
      for (let i = 0; i < 40; i++) { l = await f.layout(); kb = l.env['keyboard-inset-height']; if (host !== 'ios' || kb > 0) break; await sleep(50); }
      const note = box(l, 'note');
      check(l.viewport.h === viewport0.h && box(l, 'root').h === rootBox.h, `the layout viewport does not change for a keyboard: ${JSON.stringify(l.viewport)}, root ${JSON.stringify(box(l, 'root'))}`);
      if (host === 'ios') {
        check(kb > 100, `the software keyboard rose on the simulator: keyboard-inset-height ${kb} (Simulator › I/O › Keyboard › Connect Hardware Keyboard hides it)`);
        check(note.y + note.h <= l.viewport.h - kb + 0.01 && note.y < noteBefore.y, `the field is revealed above the keyboard: ${JSON.stringify(note)} under a keyboard of ${kb} in ${l.viewport.h}; before ${JSON.stringify(noteBefore)}`);
        console.log(`${host} insets: safe area ${top}/${right}/${bottom}/${left}, the viewport ${l.viewport.w}×${l.viewport.h}; the keyboard ${kb} revealed the field at y ${note.y} (was ${noteBefore.y})`);
      } else {
        check(kb === 0 && note.y === noteBefore.y, `no software keyboard here: keyboard-inset-height ${kb}, the field at ${note.y} (was ${noteBefore.y})`);
        console.log(`${host} insets: env ${JSON.stringify(env)}; the viewport ${l.viewport.w}×${l.viewport.h}; no keyboard`);
      }
      // A finger lands only where the target is seen: the strip at the
      // page's bottom is under the software keyboard, so the tap is refused,
      // having pressed nothing and kept the editor and its keyboard.
      if (host === 'ios') {
        const under = await f.tap('under').then(() => null, (e) => e.message);
        st = await f.state();
        check(/under the software keyboard/.test(under ?? '') && st.slots.under === 0 && st.slots.focused === true && st.keyboard?.visible === true, `a tap under the keyboard is refused and changes nothing: ${under}, ${JSON.stringify(st.slots)}, keyboard ${JSON.stringify(st.keyboard)}`);
      }
      // A press stops at its button, before the enclosing key handler can
      // steal focus. This is the reaction-strip shape used by Messages.
      if (host === 'ios' || host === 'web') {
        await f.tap('retain');
        st = await f.state();
        check(st.slots.presses === 1 && st.slots.focused === true, `the button retained the editor beneath a key handler: ${JSON.stringify(st.slots)}`);
        if (host === 'ios') {
          const input = await f.find('note');
          const observed = await f.op({ op: 'layout', id: input.id });
          check(observed.node?.native?.firstResponder === true, 'the retained editor is still UIKit first responder');
        }
      }
      // Dismiss: the button takes the focus (it has a focus handler), the
      // field blurs, the keyboard goes, and the field is where it was.
      await f.tap('dismiss');
      st = await f.state();
      check(st.slots.focused === false, `the dismiss button took the focus: ${JSON.stringify(st.slots)}`);
      for (let i = 0; i < 40; i++) { l = await f.layout(); if (l.env['keyboard-inset-height'] === 0) break; await sleep(50); }
      check(l.env['keyboard-inset-height'] === 0 && box(l, 'note').y === noteBefore.y, `after the keyboard went the field is back: keyboard ${l.env['keyboard-inset-height']}, the field at ${box(l, 'note').y} (was ${noteBefore.y})`);
    } catch (error) {
      failures.push(`the insets fixture stopped: ${error.message}`);
    } finally {
      await f.close();
    }
  }
  rmSync(tmp, { recursive: true, force: true });
}

// 13. `interactive-widget="resizes-content"` (LLP 1008 §9, the keyboard-bar
// fixture): the layout viewport ends at the keyboard's top, so a bar pinned
// to the bottom of the root rises with it and the bottom safe-area inset is
// the keyboard's — zero — while it is up; the dismiss brings everything back.
// Where no keyboard exists nothing moves.
{
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-smoke-'));
  const plan = resolve(tmp, 'keyboard-bar.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/keyboard-bar.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) failures.push('the keyboard-bar fixture did not compile: ' + c.stderr);
  else {
    const f = await open({ host, browser: 'chrome', plan });
    try {
      let l = await f.layout();
      const viewport0 = l.viewport, bottom0 = l.env['safe-area-inset-bottom'];
      const fact0 = (await f.state()).resources.viewport;
      check(fact0.width === l.viewport.w && fact0.height === l.viewport.h, `boot viewport fact ${JSON.stringify(fact0)} differs from layout ${JSON.stringify(l.viewport)}`);
      const bar0 = box(l, 'bar');
      check(bar0 && Math.abs(bar0.y + bar0.h - (l.viewport.h - bottom0)) < 0.01, `the bar sits on the bottom inset: ${JSON.stringify(bar0)} in ${JSON.stringify(l.viewport)}, inset ${bottom0}`);
      await f.type('note', 'hi');
      let kb = 0;
      for (let i = 0; i < 40; i++) { l = await f.layout(); kb = l.env['keyboard-inset-height']; if (host !== 'ios' || kb > 0) break; await sleep(50); }
      const bar = box(l, 'bar');
      const fact = (await f.state()).resources.viewport;
      check(fact.width === l.viewport.w && fact.height === l.viewport.h, `keyboard viewport fact ${JSON.stringify(fact)} differs from layout ${JSON.stringify(l.viewport)}`);
      console.log(`${host} viewport fact: ${fact0.width}×${fact0.height} → ${fact.width}×${fact.height}; layout ${l.viewport.w}×${l.viewport.h}; keyboard ${kb}`);
      if (host === 'ios') {
        check(kb > 100, `the software keyboard rose: keyboard-inset-height ${kb}`);
        check(Math.abs(l.viewport.h - (viewport0.h - kb)) < 0.01 && box(l, 'root').h === l.viewport.h, `the layout viewport ends at the keyboard: ${JSON.stringify(l.viewport)} (was ${JSON.stringify(viewport0)}, keyboard ${kb}), root ${JSON.stringify(box(l, 'root'))}`);
        check(l.env['safe-area-inset-bottom'] === 0, `the bottom inset is the keyboard's while it is up: ${l.env['safe-area-inset-bottom']}`);
        check(bar && Math.abs(bar.y + bar.h - l.viewport.h) < 0.01 && bar.y < bar0.y, `the bar rides on the keyboard: ${JSON.stringify(bar)} in ${l.viewport.h} (was ${JSON.stringify(bar0)})`);
        console.log(`${host} keyboard bar: the viewport ${viewport0.h} → ${l.viewport.h} under a keyboard of ${kb}; the bar's bottom ${bar0.y + bar0.h} → ${bar.y + bar.h}`);
      } else {
        check(kb === 0 && l.viewport.h === viewport0.h && bar.y === bar0.y, `no software keyboard here: ${JSON.stringify(l.viewport)}, the bar at ${bar.y} (was ${bar0.y})`);
      }
      await f.tap('dismiss');
      for (let i = 0; i < 40; i++) { l = await f.layout(); if (l.env['keyboard-inset-height'] === 0) break; await sleep(50); }
      check(l.viewport.h === viewport0.h && l.env['safe-area-inset-bottom'] === bottom0 && box(l, 'bar').y === bar0.y, `after the keyboard went everything is back: ${JSON.stringify(l.viewport)}, inset ${l.env['safe-area-inset-bottom']}, the bar at ${box(l, 'bar').y} (was ${bar0.y})`);
      // A tap on plain text — nothing focusable, nothing pressable — blurs
      // the field, as a tap on a page's ground does, and the keyboard goes
      // (the web and iOS; the Linux host's `type` never focused).
      if (host !== 'linux') {
        await f.type('note', 'again');
        for (let i = 0; i < 40; i++) { l = await f.layout(); if (host !== 'ios' || l.env['keyboard-inset-height'] > 0) break; await sleep(50); }
        await f.tap('title');
        let st = await f.state();
        for (let i = 0; i < 40; i++) { l = await f.layout(); if (l.env['keyboard-inset-height'] === 0) break; await sleep(50); }
        check(st.slots.focused === false && l.env['keyboard-inset-height'] === 0 && l.viewport.h === viewport0.h, `a tap on the title blurred the field and sent the keyboard away: ${JSON.stringify(st.slots)}, keyboard ${l.env['keyboard-inset-height']}, viewport ${JSON.stringify(l.viewport)}`);
      }
    } catch (error) {
      failures.push(`the keyboard-bar fixture stopped: ${error.message}`);
    } finally {
      await f.close();
    }
  }
  rmSync(tmp, { recursive: true, force: true });
}

}

// Live regions, computed names, and once-per-session autofocus.
if ((host === 'web' || apple || host === 'linux') && !argv.includes('--app-only')) {
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-accessibility-'));
  const plan = resolve(tmp, 'accessibility.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/accessibility.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  check(c.status === 0, 'accessibility fixture compiles: ' + c.stderr);
  if (c.status === 0) {
    const f = await open({host, browser: 'chrome', plan, ...(host === 'macos' ? {env:{EXACT_DEV_PLAN:plan}} : {})});
    try {
      let t = await f.tree();
      check(byTestId(t, 'first')?.focused === true, 'autofocus takes focus after mount');
      check(byTestId(t, 'first')?.accessibleName === 'Increment', 'button name is its text');
      check(byTestId(t, 'toggle')?.props.autofocus === false, 'autofocus=false remains false');
      check(byTestId(t, 'live-count')?.props.accessibilityLive === 'polite' && byTestId(t, 'live-container')?.props.accessibilityLive === 'assertive', 'both live region priorities are in tree');
      await f.tap('first');
      t = await f.tree();
      check(byTestId(t, 'live-count')?.props.text === 'Count 1', 'live text changes through an action');
      await f.tap('other');
      check(byTestId(await f.tree(), 'other')?.focused === true, 'a text update does not steal focus back');
      await f.clock('+1000'); await f.clock('+1000');
      t = await f.tree();
      check(byTestId(t, 'other')?.focused === true, 'remount does not steal focus from Other');
      check((await f.state()).focus.logical === byTestId(t, 'other')?.id, 'state agrees with tree focus');
      if (host === 'macos') {
        const source = resolve(tmp, 'reload.contract');
        writeFileSync(source, readFileSync(resolve(ROOT, 'contract/corpus/accessibility.contract'), 'utf8').replace('text "Other"', 'text "Other reloaded"'));
        const rebuilt = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', source, '-o', plan], {cwd:ROOT, encoding:'utf8'});
        check(rebuilt.status === 0, 'reload fixture compiles: ' + rebuilt.stderr);
        let tree;
        for (let i = 0; i < 100; i++) { tree = await f.tree(); if (byTestId(tree, 'other')?.accessibleName === 'Other reloaded') break; await sleep(20); }
        check(byTestId(tree, 'other')?.accessibleName === 'Other reloaded', 'development plan reloaded in the same session: ' + JSON.stringify(await f.logs()));
        const reloaded = await f.tree();
        check(byTestId(reloaded, 'first')?.focused !== true, 'reload does not steal focus for First');
        check(byTestId(reloaded, 'other')?.focused === true, 'reload keeps the focus on Other, at its place in the tree');
      }
    } catch (error) {
      failures.push(`the accessibility fixture stopped: ${error.message}`);
    } finally { await f.close(); }
  }
  rmSync(tmp, {recursive:true, force:true});
}

// `share` (LLP 1069.003 D6): under the agent no host shows a sheet; the request is held,
// answered by ticket, and its outcome is a journal line. A relative URL is refused by name.
if ((host === 'web' || apple || host === 'linux') && !argv.includes('--app-only')) {
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-share-')), plan = resolve(tmp, 'share.plan');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/share.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  check(c.status === 0, 'share fixture compiles: ' + c.stderr);
  if (c.status === 0) {
    const f = await open({host, browser: 'chrome', plan, ...(host === 'macos' ? {env:{EXACT_DEV_PLAN:plan}} : {})});
    try {
      await f.tap('share-link');
      const held = (await f.state()).pending?.find((p) => p.device?.capability === 'share');
      check(held?.device.args.url === 'https://example.com/post/1' && held.device.args.title === 'A post' && held.device.args.anchor === byTestId(await f.tree(), 'share-link')?.id, `share is held for the agent, anchored to the pressed node: ${JSON.stringify(held)}`);
      const answered = await f.tap(`@${held?.ticket}`, { choice: 'shared' });
      check(answered.delivery === 'substituted' && answered.answered === 'shared', `tap @N shared is substituted: ${JSON.stringify(answered)}`);
      await f.tap('share-relative');
      const lines = (await f.logs()).lines;
      check(lines.some((l) => /^t=\d+ share: shared$/.test(l)) && lines.some((l) => /^t=\d+ share: refused: url is not an absolute/.test(l)), `the journal has the outcomes: ${lines.filter((l) => /share/.test(l)).join(' | ')}`);
    } catch (error) {
      failures.push(`the share fixture stopped: ${error.message}`);
    } finally { await f.close(); }
  }
  rmSync(tmp, {recursive:true, force:true});
}

// The File System Access API's pickers (LLP 1069.010 D2): under the agent each
// is held; `type @N <path>` mints a `doc:` handle that arrives as `change`
// on the named element (one per line under `multiple`); `tap @N cancel`
// fires `cancel`; a second path for a single file is refused by name.
if ((host === 'web' || apple || host === 'linux') && !argv.includes('--app-only')) {
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-pickers-')), plan = resolve(tmp, 'pickers.plan');
  const folder = resolve(tmp, 'notes');
  mkdirSync(folder); writeFileSync(resolve(folder, 'a.md'), '# A\n'); writeFileSync(resolve(folder, 'b.md'), '# B\n');
  const c = spawnSync('cargo', ['run', '-q', '--release', '-p', 'contract', '--', 'build', resolve(ROOT, 'contract/corpus/file-pickers.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  check(c.status === 0, 'pickers fixture compiles: ' + c.stderr);
  if (c.status === 0) {
    const f = await open({host, browser: 'chrome', plan, ...(host === 'macos' ? {env:{EXACT_DEV_PLAN:plan}} : {})});
    const held = async (capability) => (await f.state()).pending?.find((p) => p.device?.capability === capability);
    const text = async (id) => byTestId(await f.tree(), id)?.props.text;
    try {
      await f.tap('open');
      let h = await held('open-file');
      check(h?.device.args.id === 'opened' && h.device.args.multiple === false, `showOpenFilePicker is held: ${JSON.stringify(h)}`);
      let refused = null;
      try { await f.type(`@${h?.ticket}`, `${folder}/a.md\n${folder}/b.md`); } catch (e) { refused = e.message; }
      check(/one path, not 2/.test(refused ?? ''), `two paths for one file are refused: ${refused}`);
      await f.type(`@${h?.ticket}`, `${folder}/a.md`); await f.clock('settle');
      check(/^doc:\/\d+\/a\.md$/.test(await text('opened-value') ?? ''), `the chosen file is a doc: handle: ${await text('opened-value')}`);
      await f.tap('open-many'); h = await held('open-file');
      await f.type(`@${h?.ticket}`, `${folder}/a.md\n${folder}/b.md`); await f.clock('settle');
      check((await text('opened-value'))?.split('\n').length === 2, `multiple: one handle per line: ${JSON.stringify(await text('opened-value'))}`);
      await f.tap('choose-folder'); h = await held('open-directory');
      await f.type(`@${h?.ticket}`, folder); await f.clock('settle');
      check(/^doc:\/\d+\/notes$/.test(await text('folder-value') ?? ''), `showDirectoryPicker: ${await text('folder-value')}`);
      await f.tap('save-as'); h = await held('save-file');
      check(h?.device.args.suggestedName === 'notes.md', `showSaveFilePicker is held with its name: ${JSON.stringify(h)}`);
      await f.type(`@${h?.ticket}`, `${folder}/saved.md`); await f.clock('settle');
      check(/^doc:\/\d+\/saved\.md$/.test(await text('saved-value') ?? ''), `showSaveFilePicker: ${await text('saved-value')}`);
      await f.tap('open'); h = await held('open-file');
      await f.tap(`@${h?.ticket}`, { choice: 'cancel' }); await f.clock('settle');
      check(await text('cancels') === '1', `tap @N cancel fires cancel: ${await text('cancels')}`);
    } catch (error) {
      failures.push(`the pickers fixture stopped: ${error.message}`);
    } finally { await f.close(); }
  }
  rmSync(tmp, {recursive:true, force:true});
}

// 13. The resolved app's own tests (LLP 1017 P7), when it declares them:
// its `test` blocks driven through a fresh session by the same operations.
const appTests = resolve(app.dir, 'app.test.contract');
if (existsSync(appTests)) {
  const t = await runTests({ host, file: appTests });
  for (const r of t.results) for (const f of r.failures) check(false, `test "${r.name}": ${f}`);
  console.log(`${host} tests: ${t.passed} passed, ${t.failed} failed (${app.name}/app.test.contract)`);
}

// 14. Native modules (LLP 1024 D8): the fixture's whole seam, when the app is it.
if (app.modules.tags.includes('exact-fixture') && ['web', 'macos', 'ios'].includes(host)) {
  const { nativeSmoke } = await import('./smoke-native.mjs');
  await nativeSmoke({ host, open, check, webDist: selectedWebDist });
}

// 15. The recorder (LLP 1067.000): one native object, a view and functions.
if (app.modules.tags.includes('waveform-view') && ['web', 'macos', 'ios'].includes(host)) {
  const { recorderSmoke } = await import('./smoke-recorder.mjs');
  await recorderSmoke({ host, open, check });
}

// 17. Signing in (LLP 1069.006): PAR, DPoP and PKCE against the fixture server, held for the agent.
if (app.id === 'com.exact.authfixture' && ['web', 'macos', 'ios'].includes(host)) {
  const { authSmoke } = await import('./smoke-auth.mjs');
  await authSmoke({ host, open, check });
  if (host === 'web') await (await import('./smoke-auth.mjs')).authPopupWeb({ webDist: selectedWebDist, check });
}

// 16. Documents in windows of their own (LLP 1069.010 slice 1), for an app
// whose manifest says `navigate-new`: two documents on the command line
// open two windows, each read through the carrier by its session's label;
// each window's title is its `head`'s; Open Recent lists both. Without `file_handlers` there is nothing to route.
if (host === 'macos' && app.manifest.file_handlers?.length && [app.manifest.launch_handler?.client_mode ?? []].flat().find((m) => m !== 'auto') === 'navigate-new') {
  const docs = [resolve(app.dir, 'README.md'), resolve(ROOT, 'llp/1000-exact2-root.explainer.md')];
  const d = await open({ host, browser: 'chrome', documents: docs });
  try {
    await d.clock('settle');
    const seen = (await d.state()).documents;
    check(seen?.windows?.length === 2, `documents: two windows for two documents, got ${JSON.stringify(seen?.windows)}`);
    for (const [i, w] of (seen?.windows ?? []).entries()) {
      d.session = w.session;
      const tree = await d.tree();
      const head = tree.nodes.find((n) => n.type === 'Head')?.props.headTitle;
      // The field holds the document path the host minted (LLP 1069.010 D1).
      const shown = byTestId(tree, 'open-file')?.props.value;
      check(w.document === docs[i] && /^doc:\/\d+\//.test(shown ?? '') && shown.endsWith('/' + docs[i].split('/').pop()), `documents: session ${w.session} shows ${shown}, not ${docs[i]}`);
      check(head && w.title === head, `documents: window ${w.session} is titled ${JSON.stringify(w.title)}, its head ${JSON.stringify(head)}`);
    }
    // The list is the user's own (AppKit keeps it per bundle id, across
    // runs and checkouts), so another README.md may already be on it; then
    // the menu names each by its folder, as AppKit's does (`RecentMenu`).
    const base = (p) => p.split('/').pop();
    const titles = docs.map((p) => (seen?.recent ?? []).filter((r) => base(r) === base(p)).length > 1 ? `${base(p)} — ${p.split('/').at(-2)}` : base(p));
    check(docs.every((p) => seen?.recent?.includes(p)) && titles.every((n) => seen?.openRecentMenu?.includes(n)), `documents: Open Recent lists ${JSON.stringify(seen?.openRecentMenu)}, not ${JSON.stringify(titles)}`);
    const second = seen?.windows?.[1];
    console.log(`${host} documents: ${seen?.windows?.map((w) => `${w.session} "${w.title}"`).join(', ')}; the second session's first pixel ${second?.firstPixelMs} ms, footprint ${((seen?.footprint - second?.footprintBefore) / 1048576).toFixed(1)} MB since it was asked for`);
  } catch (error) { check(false, `documents: ${error.message}`); } finally { await d.close(); }
}

// 17. Answers that keep coming (LLP 1069.004 slice 2): Exact Live's job
// progress over server-sent events, against the fixture started above.
if (streamFixture) {
  const { streamSmoke } = await import('./smoke-stream.mjs');
  try { await streamSmoke({ host, open, check, fixture: streamFixture }); }
  catch (error) { check(false, `the stream drive stopped: ${error.message}`); }
  finally { await streamFixture.close(); }
}

// The oracle sweep is explicit browser work, never an implicit Cargo pass.
if (host === 'web' && !argv.includes('--app-only')) {
  const sweep = spawnSync('cargo', ['test', '-p', 'exact-web', '--test', 'it', 'navigation::', '--', '--ignored', '--nocapture'], {
    cwd: ROOT, stdio: 'inherit', env: { ...process.env, EXACT_ROUTER_DIST: fixtureWebDist }, // its own plan, on the wasm runner
  });
  check(sweep.status === 0, 'router browser sweep failed');
}

console.log(`${host} smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
if (failures.length) { for (const f of failures) console.error('  ' + f); process.exit(1); }
