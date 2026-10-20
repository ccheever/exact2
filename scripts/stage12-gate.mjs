// The stage 12 gate (LLP 1139 §5, "The gate"; ~/x2theme/iosfix-s12e-plan.md,
// theme2's 06:46 approval): each line written PASS, FAIL, WAITS-ON (naming
// what it waits on) or NOT-RUN (a line the selection left out) to one file,
// with its evidence. A line whose parts aren't built yet says WAITS-ON; none
// is passed over.
//
//   bun scripts/stage12-gate.mjs [--only <line> …] [--out <file>] [--copy <clone copy>] [--before <its tag's worktree>] [--baseline <dir>] [--host <h> …] [--app <android reader app>] [--release <release identity JSON>]
//   bun scripts/stage12-gate.mjs baseline --copy <clone copy> --out <dir> [--host <h> …]
//
// `baseline` takes the consumer's "before" (LLP 1139 §5: baselines taken
// before the change, with their provenance): the clone copy's fixture build
// (signed out, synthetic data, no network: its data/fixture.ts) on each of
// the clone's own targets among the required hosts, light and dark, every
// screen in SCREENS, plus the copy's plan bytes. The copy is a git clone of
// the clone, never the clone itself; its exact.mjs names this checkout or
// another by EXACT2. EXACT_SIM names the simulator.
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir, release } from 'node:os';
import { join, resolve } from 'node:path';
import { decodePng } from './png.mjs';
import { baseLength, cellsOf, drive as scaleDrive, overridesOf, pageTop } from './scale-check.mjs';
import { webDist } from './app.mjs';
import { jsTargetBuild } from '../host/web/serve.mjs';
import { placements } from './library-gate.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
/** Every per-host line's required hosts (LLP 1139 §5). */
export const HOSTS = ['web', 'web-js', 'ios', 'macos', 'linux', 'android', 'terminal'];
/** The consumer's own targets among them: the pixel line's hosts. */
export const CLONE_HOSTS = ['web', 'ios', 'macos', 'linux'];
export const SCHEMES = ['light', 'dark'];
/** The agent clock's zero for every drive: the fixture's posts read "3h". */
export const EPOCH = '2026-10-01T15:00:00Z';
const SIZE = '420x900';

/** The fixture build's screens: each a name and the agent ops reaching it
 * from launch (fixed taps, no scrolling: a virtualized list draws its first
 * screen, so the fixture leads each screen with what it shows). */
export const SCREENS = [
  ['discover', []],
  ['author-ada', ['tap avatar-0:at://did:plc:fixture0/app.bsky.feed.post/fx0']],
  ['author-bo', ['tap avatar-1:at://did:plc:fixture1/app.bsky.feed.post/fx4']],
  ['thread', ['tap post-0:at://did:plc:fixture0/app.bsky.feed.post/fx0']],
  // The composer (the Post button): signed in through the form as the
  // fixture's made-up account, on the drive's reset store (theme2 09:19 (c)).
  // The password first: on Linux a tap after typing in a password field is
  // refused (QUEUE). Then a second of real time, for iOS's keyboard, which
  // is the system's and doesn't follow the agent's clock.
  ['composer', ['tap bottomBarSignInBtn', 'type loginPasswordInput fixture-password', 'type loginUsernameInput me.fixture.invalid', 'tap loginNextButton', 'tap compose', 'clock +1500 real']],
];

/** A screen's allowlisted rectangles, read from its layout after the shot
 * (theme2 12:06): the composer's text field caret, which blinks on the
 * platform's own clock, the field's leading 4 px over its first line. */
export const ALLOW = {
  composer: { target: 'compose-text', reason: 'the app\'s own text field\'s caret, blinking on the platform\'s clock (theme2 12:06)', rect: ({ x, y, h }) => [x, y, 4, Math.min(h, 40)].map(Math.round) },
};

/** The allowlist rectangles a drive's output gives for `screen`. */
export function allowOf(screen, out) {
  const a = ALLOW[screen];
  if (!a) return [];
  const line = out.split('\n').findLast(l => l.startsWith('{') && l.includes(`"testId":"${a.target}"`));
  if (!line) return null;
  const reply = JSON.parse(line), node = reply.node;
  // In points: the copy's agent shoots every host at one pixel a point.
  const caret = { rect: a.rect(node.space.viewport), reason: a.reason };
  // The system keyboard, where one is up (iOS): the platform's material,
  // drawn by the OS, a few levels apart between runs, in no library box.
  const kb = reply.env?.['keyboard-inset-height'] ?? 0;
  const view = reply.viewport;
  // The layout's viewport ends where the keyboard begins (iOS: 546 of 874).
  return kb > 0 && view ? [caret, { rect: [0, Math.round(view.h), Math.round(view.w), Math.round(kb)], reason: 'the system keyboard: the OS draws its material, a few levels apart between runs, over no library component' }] : [caret];
}

/** What the fixture says in the app's log for a request it refuses: a shot
 * whose drive logged it fails (nothing may reach the network). */
export const REFUSED = 'FIXTURE-REFUSED';

const pass = (...evidence) => ({ status: 'PASS', evidence });
const fail = (...evidence) => ({ status: 'FAIL', evidence });
const waits = (on, ...evidence) => ({ status: `WAITS-ON ${on}`, evidence });
const notRun = (...evidence) => ({ status: 'NOT-RUN', evidence });

function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { cwd: ROOT, encoding: 'utf8', timeout: 1_800_000, maxBuffer: 1 << 26, ...opts });
  return { ok: r.status === 0 && !r.error, out: `${r.stdout ?? ''}${r.stderr ?? ''}`, error: r.error };
}

const sha = buf => createHash('sha256').update(buf).digest('hex');

/** The agent ops for one screen, light or dark: launched, the clock settled,
 * the scheme preferred, the screen reached, each step settled and a second
 * on (the tab bar's indicator follows a scheme change a frame late, which
 * settling alone left in either place), shot, and the app's log read. */
export function screenOps(screen, scheme, png) {
  const [, taps] = SCREENS.find(([n]) => n === screen);
  const settle = ['clock settle', 'clock +1000', 'clock settle'];
  // A second of real time first: the web loads its font in real time, and a
  // header measured before it lands keeps that height (one run in thirteen
  // drew the profile's header 2 px short).
  return ['clock +1000 real', 'clock settle', `prefer prefers-color-scheme ${scheme}`, ...settle, ...taps.flatMap(t => [t, ...settle]), `screenshot ${png}`, 'logs',
    ...(ALLOW[screen] ? [`layout ${ALLOW[screen].target}`] : [])];
}

/** One host's drive of the copy: every screen, both schemes, each its own
 * launch on a reset store (so no screen follows another's state). */
// The simulator's status bar, pinned for an iOS drive (its clock and
// signal are the machine's otherwise, in every shot).
export const STATUS_BAR = ['--time', '9:41', '--dataNetwork', 'wifi', '--wifiMode', 'active', '--wifiBars', '3', '--cellularMode', 'notSupported', '--batteryState', 'charged', '--batteryLevel', '100'];

function drive(copy, host, dir, env = process.env) {
  const rows = [];
  const sim = host === 'ios' ? process.env.EXACT_SIM : null;
  if (sim) run('xcrun', ['simctl', 'status_bar', sim, 'override', ...STATUS_BAR]);
  try {
    rows.push(...driveScreens(copy, host, dir, env));
  } finally {
    if (sim) run('xcrun', ['simctl', 'status_bar', sim, 'clear']);
  }
  return rows;
}

function driveScreens(copy, host, dir, env) {
  const rows = [];
  for (const scheme of SCHEMES) {
    for (const [screen] of SCREENS) {
      const png = join(dir, `${host}-${scheme}-${screen}.png`);
      const args = [join(copy, 'exact.mjs'), 'agent', host, '--json', '--storage', 'stage12-gate', '--reset-storage', '--epoch', EPOCH,
        ...(host === 'ios' || host === 'macos' ? [] : ['--size', SIZE]), ...(host === 'ios' && process.env.EXACT_SIM ? ['--sim', process.env.EXACT_SIM] : []),
        ...screenOps(screen, scheme, png)];
      const r = run(process.execPath, args, { cwd: copy, env });
      const refused = [...new Set(r.out.match(new RegExp(`${REFUSED} [^"\\s]+`, 'g')) ?? [])];
      const said = refused.length ? `the fixture refused: ${refused.join(', ')}` : r.ok ? '' : r.out.trim().split('\n').slice(-2).join(' / ').slice(0, 300);
      const allow = r.ok ? allowOf(screen, r.out) : [];
      rows.push({ host, scheme, screen, png, ok: r.ok && existsSync(png) && !refused.length && allow !== null, sha: existsSync(png) ? sha(readFileSync(png)) : null, allow: allow ?? [], said: allow === null ? `no layout for ${ALLOW[screen].target}` : said });
    }
  }
  return rows;
}

/** What a baseline was taken on (LLP 1139 §5: commit, OS, device). */
function provenance(copy) {
  const git = (cwd, ...a) => run('git', a, { cwd }).out.trim();
  const xcode = run('xcodebuild', ['-version']).out.trim().split('\n').join(' ');
  const sim = process.env.EXACT_SIM ? run('xcrun', ['simctl', 'list', 'devices', '-j']).out : '';
  let device = null;
  try {
    for (const [runtime, list] of Object.entries(JSON.parse(sim).devices)) for (const d of list) if (d.udid === process.env.EXACT_SIM) device = { name: d.name, runtime };
  } catch { /* no simulator named */ }
  // The SDK the copy builds with: its exact.mjs's EXACT2 (the environment's,
  // else its default), not this script's checkout.
  const named = /const EXACT2 = resolve\(import\.meta\.dir, process\.env\.EXACT2 \?\? "([^"]+)"\)/.exec(readFileSync(join(copy, 'exact.mjs'), 'utf8'))?.[1];
  const sdk = process.env.EXACT2 ? resolve(process.env.EXACT2) : named ? resolve(copy, named) : null;
  return {
    gate: git(ROOT, 'rev-parse', 'HEAD'), sdk, exact2: sdk ? git(sdk, 'rev-parse', 'HEAD') : null, sdkDirty: sdk ? git(sdk, 'status', '--porcelain', '--untracked-files=no') !== '' : null,
    copy: git(copy, 'rev-parse', 'HEAD'), copyDirty: git(copy, 'status', '--porcelain') !== '',
    os: `darwin ${release()}`, macos: run('sw_vers', ['-productVersion']).out.trim(), xcode, simulator: device, statusBar: STATUS_BAR.join(' '), epoch: EPOCH, size: { web: SIZE, linux: SIZE, ios: 'the simulator\'s screen', macos: 'its own window' }, schemes: SCHEMES,
    taken: new Date().toISOString(),
  };
}

/** The copy's plan bytes (its app.contract, compiled by its exact2). */
function planBytes(copy, dir) {
  const plan = join(dir, 'app.plan');
  const r = run(process.execPath, [join(copy, 'exact.mjs'), 'contract', 'build', 'app.contract', '-o', plan], { cwd: copy });
  return r.ok && existsSync(plan) ? { plan, sha: sha(readFileSync(plan)), bytes: readFileSync(plan).length } : { plan: null, said: r.out.trim().split('\n').slice(-3).join(' / ') };
}

export function baseline(copy, out, hosts = CLONE_HOSTS, env = process.env) {
  mkdirSync(out, { recursive: true });
  const rows = hosts.flatMap(h => drive(copy, h, out, env));
  const record = { provenance: provenance(copy), plan: planBytes(copy, out), shots: rows };
  writeFileSync(join(out, 'baseline.json'), `${JSON.stringify(record, null, 1)}\n`);
  return record;
}

/** The web's floor (theme2's stage 12 E ruling): Chromium draws the edge of
 * an avatar's round clip one of two ways per launch, a level or two apart
 * (598 px, 501 at 1 and 97 at 2, in about four launches of ten, whatever
 * the raster flags), so an anti-aliased EDGE pixel may differ by up to
 * `floor` there; any other pixel, a flat area's, compares strictly, so a
 * change of one level there still fails. An edge pixel is one a 4-neighbour
 * differs from by more than `edge` in some channel, in the before shot. */
export const WEB_NOISE = { floor: 2, edge: 32 };

/** Pixels differing between two screenshots outside `allow` (rectangles
 * [x, y, w, h], each with a reason): any channel apart, or more than
 * `noise.floor` apart on a before-shot edge pixel (`noise.edge`):
 * {differ, total, size}. A size change is every pixel. */
export function pixelDiff(a, b, allow = [], noise = null) {
  const A = decodePng(a), B = decodePng(b);
  if (A.width !== B.width || A.height !== B.height) return { differ: A.width * A.height, total: A.width * A.height, size: [A.width, A.height, B.width, B.height] };
  const inside = (x, y) => allow.some(([rx, ry, rw, rh]) => x >= rx && x < rx + rw && y >= ry && y < ry + rh);
  let differ = 0;
  for (let y = 0; y < A.height; y++) {
    for (let x = 0; x < A.width; x++) {
      const i = (y * A.width + x) * 4;
      const d = apart(A.data, i, B.data, i);
      if (d === 0 || inside(x, y)) continue;
      if (noise && d <= noise.floor && edgeAt(A, x, y, noise.edge)) continue;
      differ++;
    }
  }
  return { differ, total: A.width * A.height };
}

/** Two baseline records shot for shot (each host, scheme and screen):
 * differing pixels outside both sides' allowlisted rectangles, strict zero
 * (theme2 12:06: no floor). A shot missing or failed on either side fails. */
export function comparePixels(before, after) {
  const key = s => `${s.host} ${s.scheme} ${s.screen}`;
  const later = new Map(after.shots.map(s => [key(s), s]));
  return before.shots.map(b => {
    const a = later.get(key(b));
    if (!b.ok || !a?.ok) return { shot: key(b), ok: false, said: !a ? 'no after shot' : `not shot: ${(b.ok ? a : b).said}` };
    const allow = [...b.allow ?? [], ...a.allow ?? []];
    const d = pixelDiff(readFileSync(b.png), readFileSync(a.png), allow.map(r => r.rect), b.host === 'web' ? WEB_NOISE : null);
    return { shot: key(b), ok: d.differ === 0, differ: d.differ, total: d.total, allow };
  });
}

/** The largest channel difference between pixel `i` of `p` and `j` of `q`. */
const apart = (p, i, q, j) => Math.max(Math.abs(p[i] - q[j]), Math.abs(p[i + 1] - q[j + 1]), Math.abs(p[i + 2] - q[j + 2]), Math.abs(p[i + 3] - q[j + 3]));

/** Whether (x, y) is an anti-aliased edge: a 4-neighbour more than `edge` apart. */
function edgeAt(img, x, y, edge) {
  const i = (y * img.width + x) * 4;
  return [[1, 0], [-1, 0], [0, 1], [0, -1]].some(([dx, dy]) => {
    const nx = x + dx, ny = y + dy;
    return nx >= 0 && ny >= 0 && nx < img.width && ny < img.height && apart(img.data, i, img.data, (ny * img.width + nx) * 4) > edge;
  });
}

/** The tag the baseline was taken at, in the copy. */
export const BASELINE_TAG = 's12e-baseline';

/** The files `before` differs from the baseline's tag by: only the lock a
 * newer SDK needs (`exact.mjs lock`) may differ. */
export function beforeDrift(before) {
  return run('git', ['diff', '--name-only', BASELINE_TAG, 'HEAD'], { cwd: before }).out.split('\n').filter(f => f && f !== 'Cargo.lock');
}

/** A copy's tracked changes not committed (staged or not): a build reads
 * the working tree, so a side must be its commit. */
export const uncommitted = copy => run('git', ['status', '--porcelain', '--untracked-files=no'], { cwd: copy }).out.split('\n').filter(Boolean);

/** The library components a copy's app uses (its Tamber IR's `library`,
 * compiled by its own SDK), by identity. */
function usesOf(copy, dir, env) {
  mkdirSync(dir, { recursive: true });
  const ir = join(dir, 'tamber.json');
  const r = run(process.execPath, [join(copy, 'exact.mjs'), 'contract', 'tamber', 'ir', '.', '-o', ir], { cwd: copy, env });
  if (!r.ok || !existsSync(ir)) return null;
  return (JSON.parse(readFileSync(ir, 'utf8')).library ?? []).map(c => c.identity);
}

/** What the adoption must use: the four components from the two packages. */
export const ADOPTED = ['@exact/cards/post-card.contract#PostCard', '@exact/cards/at-card.contract#AtCard', '@exact/actions/action-row.contract#ActionRow', '@exact/actions/post-button.contract#PostButton'];

/** One side's builds, on every host, into a target of its own; iOS onto the
 * simulator, which both sides share (one app id), so a side is shot right
 * after its own build. */
function buildSide(copy, side) {
  const env = { ...process.env, CARGO_TARGET_DIR: join(ROOT, 'target', `stage12-gate-${side}`), CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? '4' };
  for (const verb of [['web-build'], ['linux'], ['mac'], ['ios', ...(process.env.EXACT_SIM ? ['--sim', process.env.EXACT_SIM] : [])]]) {
    const r = run(process.execPath, [join(copy, 'exact.mjs'), ...verb], { cwd: copy, env });
    if (!r.ok) return `${side}: ${verb[0]} failed: ${r.out.trim().split('\n').slice(-2).join(' / ').slice(0, 300)}`;
  }
  return null;
}

/** Before (the baseline's tag, its lock refreshed for this SDK) against
 * after (the adoption), each built and shot on this SDK, so only the
 * adoption differs (theme2 13:40). */
export function consumerPixels(opts) {
  const drift = beforeDrift(opts.before);
  if (drift.length) return fail(`--before differs from ${BASELINE_TAG} beyond its lock: ${drift.join(', ')}`);
  for (const [side, copy] of [['--before', opts.before], ['--copy', opts.copy]]) {
    const dirty = uncommitted(copy);
    if (dirty.length) return fail(`${side} has changes not committed: ${dirty.join(', ').slice(0, 300)}`);
  }
  // The adoption is the packages': after uses all four library
  // components, before none.
  const uses = Object.fromEntries([['before', opts.before], ['after', opts.copy]].map(([side, copy]) => [side, usesOf(copy, join(ROOT, 'target', 'stage12-gate', `${side}-ir`), { ...process.env, CARGO_TARGET_DIR: join(ROOT, 'target', `stage12-gate-${side}`) })]));
  if (!uses.before || !uses.after) return fail(`a copy's design did not read (contract tamber ir): ${['before', 'after'].filter(s => !uses[s]).join(', ')}`);
  const missing = ADOPTED.filter(id => !uses.after.includes(id));
  if (missing.length) return fail(`--copy does not use ${missing.join(', ')}: not the adoption`);
  if (uses.before.length) return fail(`--before already uses library components (${uses.before.join(', ')}): not the baseline`);
  // Conform on the clone: its own check (every diagnostic of its build,
  // and its data module's tests), on the adoption.
  const check = run('sh', [join(opts.copy, 'tools/check.sh')], { cwd: opts.copy, env: { ...process.env, CARGO_TARGET_DIR: join(ROOT, 'target', 'stage12-gate-after') } });
  const checkSaid = check.out.trim().split('\n').at(-1) ?? '';
  const shots = {};
  for (const [side, copy] of [['before', opts.before], ['after', opts.copy]]) {
    const said = buildSide(copy, side);
    if (said) return fail(said);
    const env = { ...process.env, CARGO_TARGET_DIR: join(ROOT, 'target', `stage12-gate-${side}`) };
    shots[side] = baseline(copy, join(ROOT, 'target', 'stage12-gate', side), CLONE_HOSTS, env);
  }
  const r = comparePixels(shots.before, shots.after);
  const bad = r.filter(x => !x.ok);
  const p = shots.after.provenance;
  const evidence = [`before ${shots.before.provenance.copy.slice(0, 7)} (${BASELINE_TAG}, lock for the SDK), after ${p.copy.slice(0, 7)}, SDK ${p.exact2?.slice(0, 10)}: ${r.length} shots, ${r.length - bad.length} identical outside their allowlists`,
    ...bad.map(x => `  ${x.shot}: ${x.differ ?? x.said}${x.differ ? ' px' : ''}`),
    `the adoption uses ${ADOPTED.length} of ${ADOPTED.length} library components (${ADOPTED.map(i => i.split('#')[1]).join(', ')}); before uses none`,
    `conform on the clone (its tools/check.sh: its build's diagnostics and its data tests): ${check.ok && checkSaid === 'ok' ? 'clean' : `NOT clean: ${check.out.trim().split('\n').slice(-3).join(' / ').slice(0, 300)}`}`];
  return bad.length || !r.length || !check.ok || checkSaid !== 'ok' ? fail(...evidence) : pass(...evidence);
}

/** The library lines' required hosts (LLP 1139 §5): the web's JS target
 * and its wasm target, iOS, macOS, Linux, Android's reader and the
 * terminal. A required host missing from a run fails the line. */
export const SLOT_HOSTS = ['web-js', 'web-wasm', 'linux', 'macos', 'ios', 'android', 'terminal'];
/** Checked where a runner is available, else recorded as skipped. */
export const OPTIONAL_HOSTS = ['windows', 'tvos'];
const skipped = OPTIONAL_HOSTS.map(h => `  ${h}: skipped (no runner on this machine)`);

/** The web's wasm build of the agent's app (`bun host/web/build.mjs
 * <app> --wasm`, into this dist: EXACT_WASM_DIST, or the gate's own). */
const wasmDist = () => resolve(process.env.EXACT_WASM_DIST ?? join(ROOT, 'target', 'stage12-gate', 'web-wasm-dist'));

/** scale-check's drive on `host`, the web's two targets told apart: each
 * from a dist built for it, refused when the dist is the other's. */
export function drivePlan(host, plan, png, steps = [], app = null) {
  if (host === 'web-js') {
    if (!jsTargetBuild(webDist())) throw new Error(`the web dist ${webDist()} is not a JS-target build (bun host/web/build.mjs <app>)`);
    return scaleDrive('web', plan, png, steps, app);
  }
  if (host === 'web-wasm') {
    const dist = wasmDist();
    if (!existsSync(join(dist, '.exact-build.json')) || jsTargetBuild(dist)) throw new Error(`${dist} is not a wasm build (EXACT_WEB_DIST=${dist} bun host/web/build.mjs <app> --wasm)`);
    const was = process.env.EXACT_WEB_DIST;
    process.env.EXACT_WEB_DIST = dist;
    try { return scaleDrive('web', plan, png, steps, app); } finally { if (was === undefined) delete process.env.EXACT_WEB_DIST; else process.env.EXACT_WEB_DIST = was; }
  }
  return scaleDrive(host, plan, png, steps, app);
}
const LIBRARY_GATE = join(ROOT, 'apps/library-gate');

/** apps/library-gate's plan, compiled from a copy without its app.json (a
 * host takes a plan of its own app's id, or of none), inside the checkout so
 * `@exact/*` resolves through its node_modules, as stage 11's gate does. */
function libraryPlan(dir) {
  const app = join(dir, 'app');
  mkdirSync(app, { recursive: true });
  for (const f of ['app.contract', 'app.tamber']) writeFileSync(join(app, f), readFileSync(join(LIBRARY_GATE, f)));
  const plan = join(dir, 'library-gate.plan');
  const r = run('cargo', ['run', '-q', '-p', 'contract', '--', 'build', join(app, 'app.contract'), '-o', plan]);
  return compiled(r, plan);
}

/** The last compile's errors, which a line that needed the plan says. */
let compileSaid = '';
const compiled = (r, plan) => {
  if (r.ok) return plan;
  compileSaid = r.out.split('\n').filter(l => / error |^error|\[[a-z-]+\]/.test(l) && !/warning/.test(l)).slice(0, 3).join(' / ').slice(0, 400) || r.out.trim().split('\n').slice(-3).join(' / ').slice(0, 400);
  return null;
};

/** The ids of library-gate's slotted cells (each a marker): its PostCards'. */
export const markerIds = () => placements(readFileSync(join(LIBRARY_GATE, 'app.contract'), 'utf8'), ['PostCard']).map(p => p.id);

/** Differing pixels inside `rect` (layout points, scaled to the shots, `top`
 * pixels down: a host's bars above the viewport). */
export function rectDiffers(a, b, rect, viewportW, top = 0) {
  const A = typeof a.width === 'number' ? a : decodePng(a), B = typeof b.width === 'number' ? b : decodePng(b);
  if (A.width !== B.width || A.height !== B.height) return -1;
  const k = A.width / viewportW;
  let n = 0;
  for (let y = Math.max(0, Math.floor(rect.y * k) + top); y < Math.min(A.height, Math.ceil((rect.y + rect.h) * k) + top); y++) {
    for (let x = Math.floor(rect.x * k); x < Math.ceil((rect.x + rect.w) * k); x++) {
      const i = (y * A.width + x) * 4;
      if (A.data[i] !== B.data[i] || A.data[i + 1] !== B.data[i + 1] || A.data[i + 2] !== B.data[i + 2]) n++;
    }
  }
  return n;
}

/** Why a drive failed: the first line past "the drive failed" that says. */
export const whyOf = e => {
  const lines = String(e?.message ?? e).split('\n');
  return (lines.slice(1).find(l => /stale|error|unavailable|refused|not on the screen|no view|failed/i.test(l)) ?? lines[0]).trim().slice(0, 240);
};

/** A rect laid out, non-zero and inside the viewport. */
const shown = (n, v) => n && n.w > 0 && n.h > 0 && n.x >= 0 && n.y >= 0 && n.x + n.w <= v.w + 0.5 && n.y + n.h <= v.h + 0.5;

/** Where the viewport starts in a shot that holds a host's bars too (the
 * Android reader's status bar, which the layout's insets don't name), in
 * the shot's pixels: library-gate's top mark (`gate-top`, its own colour),
 * found as scale-check finds a page's top, from neither marker's pixels;
 * 0 for a shot of the viewport. Throws when a taller shot shows no mark. */
export function viewportTop(img, layout) {
  const k = img.width / layout.viewport.w;
  if (img.height <= layout.viewport.h * k + 2) return 0;
  const top = pageTop(img, layout, { background: '#ffffff', boxes: [{ testId: 'gate-top', fill: '#ff00ff' }] }, k);
  if (!(top > 0)) throw new Error('the shot is taller than the viewport and its top mark (gate-top) was not found');
  return top;
}

/** The marker's rect in a layout, and why it doesn't count. */
const markerIn = (layout, id) => {
  const n = layout.nodes.find(x => x.testId === `marker-${id}`);
  return { n, why: shown(n, layout.viewport) ? '' : `not laid out inside the viewport (${JSON.stringify(n && { x: n.x, y: n.y, w: n.w, h: n.h })})` };
};
const sameRect = (a, b) => ['x', 'y', 'w', 'h'].every(k => Math.abs(a[k] - b[k]) <= 0.5);

/** The slot rendered, on `host`: every marker shown with each glyph, its
 * rect laid out inside the viewport and the same in both renders (each
 * render's own layout), and the two shots apart inside it (the terminal:
 * its cells in the two text shots). */
export function slotOn(host, plan, dir, app = 'unsaid') {
  const ids = markerIds();
  if (!ids.length) return [{ host, id: '(none)', ok: false, why: 'apps/library-gate places no slotted cell' }];
  if (host === 'terminal') {
    const a = join(dir, 'terminal-a.txt'), b = join(dir, 'terminal-b.txt');
    let layout;
    try { layout = drivePlan('terminal', plan, null, ['tap marker-toggle', 'layout', `screenshot ${a}`, 'tap marker-toggle', `screenshot ${b}`]); } catch (e) { return ids.map(id => ({ host, id, ok: false, why: whyOf(e) })); }
    const first = layout.earlier?.[0];
    if (!first) return ids.map(id => ({ host, id, ok: false, why: 'the first render\'s layout was not read' }));
    const [ta, tb] = [a, b].map(f => readFileSync(f, 'utf8').split('\n'));
    return ids.map(id => {
      const [A, B] = [markerIn(first, id), markerIn(layout, id)];
      if (A.why || B.why) return { host, id, ok: false, why: A.why || B.why };
      const [ca, c] = [A.n.cells, B.n.cells];
      if (!c || c.w <= 0 || c.h <= 0) return { host, id, ok: false, why: 'no cells laid out' };
      if (!ca || !sameRect(ca, c)) return { host, id, ok: false, why: `its cells moved between the renders (${JSON.stringify(ca)} then ${JSON.stringify(c)})` };
      const cut = t => Array.from({ length: c.h }, (_, r) => [...(t[c.y + r] ?? '')].slice(c.x, c.x + c.w).join(''));
      const differ = cut(ta).join('\n') !== cut(tb).join('\n');
      return { host, id, ok: differ, why: differ ? '' : 'its cells read the same with both glyphs', cells: c };
    });
  }
  return ids.map(id => {
    const a = join(dir, `${host}-${id}-a.png`), b = join(dir, `${host}-${id}-b.png`);
    let layout;
    try { layout = drivePlan(host, plan, b, [`tap marker-${id}`, 'layout', `screenshot ${a}`, `tap marker-${id}`], app); } catch (e) { return { host, id, ok: false, why: whyOf(e) }; }
    const first = layout.earlier?.[0];
    if (!first) return { host, id, ok: false, why: 'the first render\'s layout was not read' };
    const [A, B] = [markerIn(first, id), markerIn(layout, id)];
    if (A.why || B.why) return { host, id, ok: false, why: A.why || B.why };
    if (!sameRect(A.n, B.n)) return { host, id, ok: false, why: 'its rect moved between the renders' };
    try {
      const [IA, IB] = [a, b].map(f => decodePng(readFileSync(f)));
      const [topA, topB] = [viewportTop(IA, first), viewportTop(IB, layout)];
      if (Math.abs(topA - topB) > 0.5) return { host, id, ok: false, why: `the viewport starts at ${topA} and ${topB} in the two shots` };
      const d = rectDiffers(IA, IB, B.n, layout.viewport.w, Math.round(topB));
      return { host, id, ok: d > 0, why: d > 0 ? '' : d < 0 ? 'the two shots differ in size' : 'its pixels are the same with both glyphs', px: d };
    } catch (e) { return { host, id, ok: false, why: String(e?.message ?? e).slice(0, 240) }; }
  });
}

export function slotRendered(opts) {
  const dir = join(ROOT, 'target', 'stage12-gate', 'slot');
  const plan = libraryPlan(dir);
  if (!plan) return fail('apps/library-gate did not compile', compileSaid);
  const hosts = opts.hosts.length ? opts.hosts.filter(h => SLOT_HOSTS.includes(h)) : SLOT_HOSTS;
  const rows = hosts.flatMap(h => slotOn(h, plan, dir, opts.app));
  const missing = SLOT_HOSTS.filter(h => !hosts.includes(h));
  const bad = rows.filter(r => !r.ok);
  const evidence = hosts.map(h => `${h}: ${rows.filter(r => r.host === h && r.ok).length} of ${rows.filter(r => r.host === h).length} markers rendered with both glyphs`)
    .concat(bad.map(r => `  ${r.host} ${r.id}: ${r.why}`), missing.map(h => `  ${h}: a required host not run`), skipped);
  return bad.length || missing.length || !rows.length ? fail(...evidence) : pass(...evidence);
}

/** The restyle line's cases (LLP 1139 §5, "One app value restyles the
 * library"): a base name, a package token and a param, each set in
 * apps/library-gate's design and then removed. Each `reads` is a length
 * measured on the layout (`at`, in points) and what it is: `is` the value
 * itself, or `twice` / `once` its change from the base drive (a box that
 * grows on both sides, or moves). `axis` says which way a terminal cell
 * rounds it (none for a size). */
export const RESTYLE = [
  {
    name: 'space.md', what: 'a base name', set: '24px', unset: { op: 'remove-token', name: 'space.md' },
    reads: [
      { who: 'PostCard', what: 'its end padding (cards.pad-end)', is: true, axis: 'w', at: L => right(L, 'post-pc-base') - right(L, 'postcard-content-pc-base') },
      // Between its counted buttons' row and save's: three equal columns
      // (flex 1 each), so the first row ends a column past like's.
      { who: 'ActionRow', what: 'its gap (param gap, actions.row-gap)', is: true, axis: 'w', at: L => box(L, 'bookmark-ar-base').x - (2 * box(L, 'like-ar-base').x - box(L, 'repost-ar-base').x) },
      { who: 'AtCard', what: 'its padding (cards.link-pad), the visit button\'s move', once: true, axis: 'w', at: L => box(L, 'atcard-visit-at-base').x - box(L, 'atcard-at-base').x },
      { who: 'PostButton', what: 'its side padding (actions.post-pad), its width', twice: true, axis: 'w', at: L => box(L, 'compose-post-pb-base').w },
    ],
  },
  {
    name: 'cards.pad-start', what: 'a package token', set: '20px', unset: { op: 'remove-token', name: 'cards.pad-start' },
    reads: [{ who: 'PostCard', what: 'its start padding', is: true, axis: 'w', at: L => box(L, 'postcard-avatar-pc-base').x - box(L, 'post-pc-base').x }],
  },
  {
    // A library component's param by its full name (LLP 1139 D8).
    name: '@exact/actions/post-button.contract#PostButton.height', param: true, what: 'a param', set: '40px', unset: { op: 'unset-param', name: '@exact/actions/post-button.contract#PostButton.height' },
    // A size, not a space: the terminal keeps it in points (only spaces
    // are whole cells, LLP 1121 stage 11 (c)).
    reads: [{ who: 'PostButton', what: 'its height', is: true, axis: null, at: L => box(L, 'compose-post-pb-base').h }],
  },
];

const box = (L, id) => {
  const n = L.nodes.find(x => x.testId === id);
  if (!n) throw new Error(`${id} is not in the layout`);
  return n;
};
const right = (L, id) => { const n = box(L, id); return n.x + n.w; };

/** A design server for `dir` (`contract tamber serve --stdio`, as the dev
 * loops run it), asked one request at a time. A request it doesn't answer
 * in `deadline` ms, an answer it can't tie to a request, or the server's
 * exit or failure ends every request still waiting, and every later one,
 * with an error. */
function designServer(dir, deadline = 300_000) {
  // Its own process group, so closing it ends cargo's child too, which
  // holds the design's lock.
  const child = spawn('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'serve', '--stdio', dir], { cwd: ROOT, stdio: ['pipe', 'pipe', 'pipe'], detached: true });
  const waiting = new Map();
  let buf = '', next = 0, err = '', closed = null;
  const end = why => {
    closed ??= why;
    for (const done of waiting.values()) done({ error: { message: closed } });
    waiting.clear();
  };
  child.stderr.on('data', d => { err = (err + d).slice(-2000); });
  child.stdout.on('data', d => {
    buf += d;
    for (let i; (i = buf.indexOf('\n')) >= 0;) {
      const line = buf.slice(0, i);
      buf = buf.slice(i + 1);
      let reply;
      try { reply = JSON.parse(line); } catch { continue; }
      // A line with no id is an event (a write's `changed`), no answer.
      if (!reply || typeof reply !== 'object' || !('id' in reply)) continue;
      const done = waiting.get(reply.id);
      if (!done) { end(`the server answered no request: ${line.slice(0, 300)}`); return; }
      waiting.delete(reply.id);
      done(reply);
    }
  });
  child.on('error', e => end(`the server did not start: ${e.message}`));
  child.stdin.on('error', e => end(`the server's input failed: ${e.message}`));
  child.on('exit', code => end(`the server exited (${code}): ${err.trim().slice(-300)}`));
  return {
    ask: request => new Promise(done => {
      if (closed) return done({ error: { message: closed } });
      const id = ++next;
      const timer = setTimeout(() => { if (waiting.delete(id)) done({ error: { message: `no answer in ${deadline / 1000} s` } }); }, deadline);
      waiting.set(id, reply => { clearTimeout(timer); done(reply); });
      child.stdin.write(`${JSON.stringify({ ...request, id })}\n`);
    }),
    close: () => {
      closed ??= 'closed';
      try { process.kill(-child.pid, 'SIGTERM'); } catch { child.kill(); }
    },
  };
}

/** library-gate's app (no app.json, as libraryPlan) in `dir`, its design
 * `design` (the gate's own when null), and its plan; null when it fails. */
function restylePlan(dir, design = null) {
  const app = join(dir, 'app');
  mkdirSync(app, { recursive: true });
  writeFileSync(join(app, 'app.contract'), readFileSync(join(LIBRARY_GATE, 'app.contract')));
  writeFileSync(join(app, 'app.tamber'), design ?? readFileSync(join(LIBRARY_GATE, 'app.tamber')));
  const plan = join(dir, 'library-gate.plan');
  const r = run('cargo', ['run', '-q', '-p', 'contract', '--', 'build', join(app, 'app.contract'), '-o', plan]);
  return compiled(r, plan);
}

/** Each case through the design server on a copy of library-gate: `propose`
 * the line (its class), write it (`transaction`), `propose` its removal
 * (its class), and undo the write, which must leave the design's bytes as
 * they were. The designs: the line set, and after removal. */
export async function restyleWrites(dir) {
  const work = join(dir, 'server');
  restylePlan(work);
  const design = join(work, 'app', 'app.tamber'), original = readFileSync(design, 'utf8');
  const server = designServer(join(work, 'app'));
  const client = 'agent:stage12-gate';
  try {
    const hello = await server.ask({ method: 'hello', client });
    if (!hello.ok) throw new Error(`hello: ${hello.error?.message}`);
    let files = hello.ok.files;
    const out = [];
    for (const c of RESTYLE) {
      const set = [c.param ? { op: 'set-param', name: c.name, value: c.set } : { op: 'set-token', name: c.name, value: c.set }];
      const said = (r, what) => { if (!r.ok) throw new Error(`${c.name}: ${what}: ${r.error?.message ?? JSON.stringify(r.error)}`); return r.ok; };
      const proposed = said(await server.ask({ method: 'propose', ops: set, files }), 'propose');
      const written = said(await server.ask({ method: 'transaction', client, ops: set, files }), 'transaction');
      files = written.files;
      const withLine = readFileSync(design, 'utf8');
      const removal = said(await server.ask({ method: 'propose', ops: [c.unset], files }), 'propose its removal');
      const undone = said(await server.ask({ method: 'undo', client, inverse: written.inverse }), 'undo');
      files = undone.files;
      const after = readFileSync(design, 'utf8');
      out.push({ name: c.name, classes: { set: proposed.class, written: written.class, removal: removal.class, undo: undone.class }, withLine, after, restored: after === original, diff: written.diff });
    }
    return out;
  } finally {
    server.close();
  }
}

/** What `read` should measure on `host` for value `v` (points), from the
 * base drive's `base`: the terminal lays a length out in whole cells. */
export function expected(read, v, v0, base, cell) {
  const k = cell && read.axis ? (read.axis === 'h' ? cell.h : cell.w) : 0;
  const q = x => (k ? cellsOf(x, k) * k : x);
  if (read.is) return q(v);
  return base + (read.twice ? 2 : 1) * (q(v) - q(v0));
}

/** The restyle line on `hosts`: library-gate's base drive, then each case's
 * set and removed designs, measured against `scale::column_for`'s value for
 * the host (scale-check's `baseLength` over kernel/tables/schema.json and
 * the design's own px tokens). */
export async function restyle(opts) {
  const dir = join(ROOT, 'target', 'stage12-gate', 'restyle');
  let writes;
  try { writes = await restyleWrites(dir); } catch (e) { return fail(`the design server: ${String(e?.message ?? e).slice(0, 300)}`); }
  const plans = { base: restylePlan(join(dir, 'base')) };
  for (const w of writes) {
    const at = w.name.replace(/[^\w.-]+/g, '-');
    plans[`${w.name} set`] = restylePlan(join(dir, `${at}-set`), w.withLine);
    plans[`${w.name} removed`] = restylePlan(join(dir, `${at}-removed`), w.after);
  }
  const broken = Object.entries(plans).filter(([, p]) => !p).map(([k]) => k);
  if (broken.length) return fail(`library-gate did not compile: ${broken.join(', ')}`, compileSaid);
  const schema = JSON.parse(readFileSync(join(ROOT, 'kernel/tables/schema.json'), 'utf8'));
  const irFile = join(dir, 'base', 'tamber.json');
  if (!run('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'ir', join(dir, 'base', 'app'), '-o', irFile]).ok) return fail('library-gate\'s design did not read (contract tamber ir)');
  const ir = JSON.parse(readFileSync(irFile, 'utf8'));
  const overrides = overridesOf(ir, []);
  // A name's length on a host: a px token's, or the base name a package
  // token names (cards.pad-start is space.sm), or a base name's; a param's
  // is its default's.
  const lengthOf = (name, host, cell) => {
    const t = ir.tokens?.find(x => x.name === name);
    const values = [...new Set(t?.values ?? [])];
    if (t && !Object.hasOwn(overrides, name) && values.length === 1) return baseLength(schema, overrides, values[0], host, cell);
    return baseLength(schema, overrides, name, host, cell);
  };
  const paramOf = c => ir.components?.flatMap(x => x.params ?? []).find(p => p.name === c.name);
  const hosts = opts.hosts.length ? opts.hosts.filter(h => SLOT_HOSTS.includes(h)) : SLOT_HOSTS;
  const rows = [];
  for (const host of hosts) {
    const L = {};
    try {
      for (const [k, plan] of Object.entries(plans)) L[k] = drivePlan(host, plan, join(dir, `${host}-${k.replace(/\W+/g, '-')}.png`), [], opts.app);
    } catch (e) { rows.push({ host, ok: false, why: whyOf(e) }); continue; }
    const cell = L.base.cell ?? null;
    for (const c of RESTYLE) {
      // The web's two targets read one column (the neutral look).
      const v0 = lengthOf(c.param ? paramOf(c)?.default : c.name, host.startsWith('web-') ? 'web' : host, cell);
      const v1 = Number(/^(\d+(?:\.\d+)?)px$/.exec(c.set)[1]);
      for (const read of c.reads) {
        const row = { host, case: c.name, who: read.who, what: read.what, ok: true, why: [] };
        try {
          const b = read.at(L.base), s = read.at(L[`${c.name} set`]), r = read.at(L[`${c.name} removed`]);
          const want0 = read.is ? expected(read, v0, v0, b, cell) : b, want1 = expected(read, v1, v0, b, cell);
          Object.assign(row, { before: b, set: s, removed: r, want: [want0, want1] });
          if (v0 === undefined) row.why.push(`no value of ${c.name} for ${host}`);
          const unread = Object.entries({ before: b, set: s, removed: r, expected: want0, 'expected set': want1 }).filter(([, x]) => !Number.isFinite(x)).map(([k]) => k);
          if (unread.length) throw new Error(`not a number: ${unread.join(', ')}`);
          if (Math.abs(b - want0) > 0.5) row.why.push(`before ${b}, expected ${want0}`);
          if (Math.abs(s - want1) > 0.5) row.why.push(`set ${s}, expected ${want1}`);
          if (Math.abs(r - b) > 0.5) row.why.push(`removed ${r}, not back to ${b}`);
        } catch (e) { row.why.push(String(e?.message ?? e)); }
        row.ok = !row.why.length;
        row.why = row.why.join('; ');
        rows.push(row);
      }
    }
  }
  const missing = SLOT_HOSTS.filter(h => !hosts.includes(h));
  const bad = rows.filter(r => !r.ok);
  const n = x => (Number.isFinite(x) ? Math.round(x * 100) / 100 : x);
  const evidence = [
    ...writes.map(w => `${w.name} (${RESTYLE.find(c => c.name === w.name).what}): propose ${w.classes.set}, written ${w.classes.written}, its removal proposed ${w.classes.removal}, undone ${w.classes.undo}; the design's bytes ${w.restored ? 'restored' : 'NOT restored'}`),
    ...rows.filter(r => r.case).map(r => `${r.ok ? 'ok  ' : 'FAIL'} ${r.host} ${r.case}: ${r.who} ${r.what}: before ${n(r.before)}, set ${n(r.set)}, removed ${n(r.removed)} (expected ${n(r.want?.[0])}, ${n(r.want?.[1])})${r.why ? `: ${r.why}` : ''}`),
    ...rows.filter(r => !r.case).map(r => `FAIL ${r.host}: ${r.why}`),
    ...missing.map(h => `  ${h}: a required host not run`),
    ...skipped,
  ];
  return bad.length || missing.length || !rows.length || writes.some(w => !w.restored) ? fail(...evidence) : pass(...evidence);
}

/** Sources that fail to build at both ends of every run, named: apps/expose
 * reads `./local`, which a checkout lacks (stage 11's gate, the same). */
export const FAIL_ALIKE = ['apps/expose/app.contract'];

/** `--release`'s comparison (~/x2ios/rel12/release-groups.py, as stage
 * 11's: every app's plan and the press and motion corpus built at the
 * parent and the end of each run of stage 12 E commits on main): PASS when
 * every run compares every app now in apps/ and the corpus, each built at
 * both ends and byte-identical, but apps/library-gate's own (the gate app
 * those commits make and change) and FAIL_ALIKE's. */
export function releaseIdentity(file, sources = releaseSources()) {
  if (!file || !existsSync(file)) return { status: 'WAITS', on: 'the release identity run (--release <its JSON>)', evidence: [] };
  const rows = JSON.parse(readFileSync(file, 'utf8'));
  if (!Array.isArray(rows) || !rows.length) return { status: 'FAIL', evidence: ['release identity: no comparisons'] };
  const own = r => r.source === 'apps/library-gate/app.contract';
  const alike = r => r.status === 'both failed' && FAIL_ALIKE.includes(r.source);
  const count = st => rows.filter(r => r.status === st).length;
  const unnamed = rows.filter(r => r.status !== 'identical' && !own(r) && !alike(r));
  const runs = [...new Set(rows.map(r => r.run))];
  const short = runs.flatMap(run => sources.filter(src => !rows.some(r => r.run === run && r.source === src)).map(src => `  NOT COMPARED ${src} over ${run}`));
  const evidence = [`release identity: ${rows.length} comparisons over ${runs.length} runs of stage 12 E commits; ${count('identical')} identical, ${rows.filter(own).length} apps/library-gate's own, ${rows.filter(alike).length} failing alike (${FAIL_ALIKE.join(', ')})`,
    ...unnamed.map(r => `  ${r.status === 'DIFFERS' ? 'MOVED' : r.status === 'both failed' ? 'FAILED AT BOTH ENDS' : 'BUILD FAILED'} ${r.source} over ${r.run} (${r.commits?.[0]})`), ...short];
  return { status: unnamed.length || short.length ? 'FAIL' : 'PASS', evidence };
}

/** What release identity must compare: every app now in apps/ (library-gate
 * aside: its own) and the corpus release-groups.py builds. */
export function releaseSources() {
  const apps = run('git', ['ls-files', 'apps/*/app.contract']).out.trim().split('\n').filter(f => f.split('/').length === 3 && f !== 'apps/library-gate/app.contract');
  return [...apps, 'host/web-js/conformance/press.contract', 'host/web-js/conformance/press-paint.contract', 'contract/corpus/motion-feel.contract'];
}

/** No regressions: conform (--synthetic --strict --linux) clean, the apps
 * on @exact/reading built unchanged (release identity covers them, as every
 * app), and release identity over stage 12 E's commits. */
export function noRegressions(opts) {
  const work = join(ROOT, 'target', 'stage12-gate');
  const conform = run(process.execPath, [join(ROOT, 'host/web-js/conform.mjs'), '--synthetic', '--strict', '--linux', '--build', '--jobs', '1', '--wasm-root', join(work, 'wasm'), '--out', join(work, 'conform')], { timeout: 7_200_000 });
  const said = conform.out.split('\n');
  const summary = said.find(l => /failures across/.test(l)) ?? `no summary${conform.error ? ` (${conform.error.code ?? conform.error.message})` : ''}: ${conform.out.trim().split('\n').slice(-3).join(' / ').slice(0, 400)}`;
  const failed = said.filter(l => /^[\w-]+: [1-9]\d* failures?$/.test(l));
  const readers = run('git', ['grep', '-l', '@exact/reading', '--', 'apps/*/app.contract']).out.trim().split('\n').filter(Boolean);
  const evidence = [`conform --synthetic --strict --linux: ${summary}`, ...failed.map(l => `  ${l}`), `@exact/reading's users: ${readers.join(', ') || 'none'} (in release identity)`];
  const release = releaseIdentity(opts.release);
  evidence.push(...release.evidence);
  if (!conform.ok || release.status !== 'PASS') return release.status === 'WAITS' && conform.ok ? waits(release.on, ...evidence) : fail(...evidence);
  return pass(...evidence);
}

/** The gate's lines, in LLP 1139 §5's order. */
export function lines(opts) {
  return [
    ['consumer-pixels', 'The clone\'s post card, AtCard, action row and Post button come from the two packages with no visual change (conform clean; zero differing pixels outside the allowlist, web, iOS, macOS, Linux, light and dark)', () => {
      if (!opts.baseline || !existsSync(join(opts.baseline, 'baseline.json'))) return waits('the baseline (`baseline --copy … --out …`, taken before the change)');
      if (!opts.copy || !opts.before) return waits('the clone copy\'s adoption (--copy) and its before (--before, the baseline\'s tag)');
      return consumerPixels(opts);
    }],
    ['restyle', 'Setting space.md, cards.pad-start or a param moves every library component reading it; removing the line returns it (per host, measured on apps/library-gate; live or rebuild from the design server)', () => restyle(opts)],
    ['anatomy', 'C\'s compiler tests: every anatomy, slot, library-literal, library-platform and library-control refusal with its span', () => {
      // C's own suites (reload1's stage 12 C), run by name.
      const r = run('cargo', ['test', '-q', '-p', 'contract', '--test', 'it', '--', 'anatomy::', 'anatomy_slot::', 'library_rules::']);
      const counts = [...r.out.matchAll(/test result: (\w+)\. (\d+) passed; (\d+) failed/g)];
      const passed = counts.reduce((n, m) => n + Number(m[2]), 0), failed = counts.reduce((n, m) => n + Number(m[3]), 0);
      const said = `contract tests anatomy::, anatomy_slot::, library_rules::: ${passed} passed, ${failed} failed`;
      return r.ok && passed > 0 && failed === 0 ? pass(said) : fail(said, r.out.trim().split('\n').slice(-4).join(' / ').slice(0, 400));
    }],
    ['slot-rendered', 'Every variant cell of apps/library-gate renders its slot\'s marker, both glyphs, on every required host', () => slotRendered(opts)],
    ['slot-check-time', 'The slot check (check_slots) on the clone with both library packages takes under about 10 ms a check (the lead, 10:15Z; rt1 measures it)', () =>
      waits('stage 12 C (check_slots), the packages and the adoption, then rt1\'s measurement')],
    ['no-forks', 'lower-library-platform clean over both packages', () => {
      // apps/library-gate uses every export, so its build reads both packages
      // whole; lower-library-platform is an error, so a clean build is clean.
      mkdirSync(join(ROOT, 'target', 'stage12-gate'), { recursive: true });
      const r = run('cargo', ['run', '-q', '-p', 'contract', '--', 'build', join(LIBRARY_GATE, 'app.contract'), '-o', join(ROOT, 'target', 'stage12-gate', 'no-forks.plan')]);
      const forks = r.out.split('\n').filter(l => l.includes('lower-library-platform'));
      const said = `apps/library-gate built: ${r.ok ? 'yes' : 'NO'}; lower-library-platform: ${forks.length ? forks.length : 'none'}`;
      return r.ok && !forks.length ? pass(said) : fail(said, ...forks.slice(0, 5), ...(r.ok ? [] : [r.out.split('\n').filter(l => /error|\[/.test(l)).slice(0, 3).join(' / ').slice(0, 400)]));
    }],
    ['no-regressions', 'Release output unchanged for apps using no library component; conform clean; @exact/reading\'s users unchanged', () => noRegressions(opts)],
  ];
}

export function parse(argv) {
  const opts = { mode: 'gate', only: [], hosts: [], app: 'unsaid', out: join(homedir(), 'x2theme/stage12-gate-run.txt'), copy: null, before: null, baseline: null, release: null };
  const args = [...argv];
  if (args[0] === 'baseline') { opts.mode = 'baseline'; opts.out = null; args.shift(); }
  for (let i = 0; i < args.length; i += 2) {
    const [flag, v] = [args[i], args[i + 1]];
    if (!['--only', '--out', '--copy', '--before', '--baseline', '--host', '--app', '--release'].includes(flag)) return `unknown argument ${flag}`;
    if (v === undefined || v.startsWith('--')) return `${flag} needs a value`;
    if (flag === '--only') opts.only.push(v); else if (flag === '--host') opts.hosts.push(v); else opts[flag.slice(2)] = v;
  }
  if (opts.mode === 'baseline' && (!opts.copy || !opts.out)) return 'baseline needs --copy and --out';
  const allowed = opts.mode === 'baseline' ? CLONE_HOSTS : SLOT_HOSTS;
  const unknownHost = opts.hosts.find(h => !allowed.includes(h));
  if (unknownHost) return `--host ${unknownHost}: one of ${allowed.join(', ')}`;
  const ids = lines(opts).map(([id]) => id);
  const missing = opts.only.find(o => !ids.includes(o));
  if (missing) return `--only ${missing}: one of ${ids.join(', ')}`;
  return opts;
}

if (import.meta.main) {
  const opts = parse(process.argv.slice(2));
  if (typeof opts === 'string') { console.error(`${opts}\nusage: bun scripts/stage12-gate.mjs [--only <line> …] [--out <file>] [--copy <dir>] [--baseline <dir>] [--release <json>] | baseline --copy <dir> --out <dir> [--host <h> …]`); process.exit(2); }
  if (opts.mode === 'baseline') {
    const r = baseline(resolve(opts.copy), resolve(opts.out), opts.hosts.length ? opts.hosts : CLONE_HOSTS);
    const bad = r.shots.filter(s => !s.ok);
    console.log(`baseline: ${r.shots.length - bad.length} of ${r.shots.length} screens; plan ${r.plan.sha ?? `FAILED (${r.plan.said})`}; ${join(opts.out, 'baseline.json')}`);
    for (const s of bad) console.log(`  FAILED ${s.host} ${s.scheme} ${s.screen}: ${s.said}`);
    process.exit(bad.length || !r.plan.sha ? 1 : 0);
  }
  const head = run('git', ['rev-parse', '--short', 'HEAD']).out.trim();
  const text = [`# Stage 12 gate run (LLP 1139 §5), ${new Date().toISOString()}, ${head}; ${opts.only.length ? `lines ${opts.only.join(', ')}` : 'every line'}`, ''];
  for (const [id, title, check] of lines(opts)) {
    let r;
    try { r = !opts.only.length || opts.only.includes(id) ? await check() : notRun('not selected'); } catch (e) { r = fail(`the line stopped: ${String(e?.stack ?? e).split('\n').slice(0, 3).join(' / ').slice(0, 400)}`); }
    text.push(`${r.status.padEnd(r.status.startsWith('WAITS') ? 0 : 5)}  ${id}: ${title}`, ...r.evidence.map(e => `    ${e}`), '');
    console.log(`${r.status}  ${id}`);
  }
  writeFileSync(opts.out, `${text.join('\n')}\n`);
  console.log(`written to ${opts.out}`);
}
