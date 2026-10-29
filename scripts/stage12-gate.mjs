// The stage 12 gate (LLP 1139 §5, "The gate"; ~/x2theme/iosfix-s12e-plan.md,
// theme2's 06:46 approval): each line written PASS, FAIL, WAITS-ON (naming
// what it waits on) or NOT-RUN (a line the selection left out) to one file,
// with its evidence. A line whose parts aren't built yet says WAITS-ON; none
// is passed over.
//
//   bun scripts/stage12-gate.mjs [--only <line> …] [--screen <name> …] [--out <file>] [--copy <clone copy>] [--before <its tag's worktree>] [--baseline <dir>] [--host <h> …] [--app <android reader app>] [--release <release identity JSON>]
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
import { closeSync, cpSync, existsSync, mkdirSync, openSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { decodePng } from './png.mjs';
import { bindingShape, bindingsCheck, namedCrops, namedOf, namedReport, pixelDiff, unionNamed } from './stage12-named.mjs';
import { s15b2Stage } from './stage12-s15b2.mjs';
import { baseLength, cellsOf, drive as scaleDrive, overridesOf, pageTop } from './scale-check.mjs';
import { webDist } from './app.mjs';
import { jsTargetBuild } from '../host/web/serve.mjs';
import { placements } from './library-gate.mjs';
import { KEYBOARD_UP, keyboardRedrive } from './stage12-keyboard.mjs';
export { KEYBOARD_UP, keyboardInset, keyboardRedrive } from './stage12-keyboard.mjs';
import { caseOps, caseReads, cases as exactCases, inspectDrive, locate, matches, apart as standsApart, same, COMPONENTS, EMPTY, readsOf } from './s13c-restyle.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
/** The consumer's own targets among the required hosts (LLP 1139 §5): the
 * pixel line's. `web` is the clone's own web build (the JS target),
 * `web-wasm` its wasm build, into a dist of its own. */
export const CLONE_HOSTS = ['web', 'web-wasm', 'ios', 'macos', 'linux'];
/** A side's wasm web dist, in its own target. */
const cloneWasmDist = env => join(env.CARGO_TARGET_DIR ?? join(ROOT, 'target'), 'web-wasm-dist');
/** And its JS build's, so the clone's web build never lands in the dist the
 * library-gate lines drive (EXACT_WEB_DIST, Caltrain's JS build). */
const cloneJsDist = env => join(env.CARGO_TARGET_DIR ?? join(ROOT, 'target'), 'web-js-dist');
const cloneWebEnv = (env, wasm) => ({ ...env, EXACT_WEB_DIST: wasm ? cloneWasmDist(env) : cloneJsDist(env) });
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

/** The screens a `--screen` rerun shoots, said in its evidence (empty in a full run). */
const RERUN_SCREENS = [];

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

/** `run`, its output through a file: a process that exits with a large
 * reply still in a pipe loses the pipe's tail (a feed's whole layout, half a
 * megabyte), and a file keeps it. */
function runToFile(cmd, args, file, opts = {}) {
  const fd = openSync(file, 'w');
  try {
    const r = spawnSync(cmd, args, { cwd: ROOT, timeout: 1_800_000, ...opts, stdio: ['ignore', fd, fd] });
    return { ok: r.status === 0 && !r.error, out: readFileSync(file, 'utf8'), error: r.error };
  } finally { closeSync(fd); }
}

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
  // Then the screen's layout whole (what it renders, by testId), and the
  // allowlist's target's last.
  return ['clock +1000 real', 'clock settle', `prefer prefers-color-scheme ${scheme}`, ...settle, ...taps.flatMap(t => [t, ...settle]), `screenshot ${png}`, 'logs', 'layout',
    ...(ALLOW[screen] ? [`layout ${ALLOW[screen].target}`] : [])];
}

/** The testIds only the packages' views give (none of the clone's own
 * sources writes one): a node shown with one is a library component's,
 * rendered. PostCard's content part, ActionRow's row, AtCard's visit part,
 * PostButton's own (no id). */
export const RENDERED = [
  ['PostCard', id => id.startsWith('postcard-content-'), 'postcard-content-'],
  ['ActionRow', id => id.startsWith('actions-'), '`actions-'],
  ['AtCard', id => id.startsWith('atcard-visit-'), 'atcard-visit-'],
  ['PostButton', id => id === 'compose-post', '"compose-post"'],
];

/** The RENDERED testIds a drive's whole layout shows: each on a node with
 * a non-zero box that reaches into the viewport (a node laid out at zero
 * size, or off the screen, shows nothing). */
export function renderedIds(out) {
  // The whole layout's reply: the op replies that parse with nodes and a
  // viewport (a log line may mention either), the last.
  const layout = out.split('\n').filter(l => l.startsWith('{') && l.includes('"nodes"')).map(l => { try { return JSON.parse(l); } catch { return null; } })
    .findLast(o => Array.isArray(o?.nodes) && o.viewport);
  if (!layout) return null;
  const { nodes, viewport: v } = layout;
  const shows = n => n.w > 0 && n.h > 0 && n.x < v.w && n.y < v.h && n.x + n.w > 0 && n.y + n.h > 0;
  const ids = nodes.filter(shows).map(n => n.testId ?? n.props?.testId).filter(Boolean);
  return [...new Set(ids.filter(id => RENDERED.some(([, is]) => is(id))))];
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
      const wasm = host === 'web-wasm';
      const args = [join(copy, 'exact.mjs'), 'agent', wasm ? 'web' : host, '--json', '--storage', 'stage12-gate', '--reset-storage', '--epoch', EPOCH,
        ...(host === 'ios' || host === 'macos' ? [] : ['--size', SIZE]), ...(host === 'ios' && process.env.EXACT_SIM ? ['--sim', process.env.EXACT_SIM] : []),
        ...screenOps(screen, scheme, png)];
      let r = runToFile(process.execPath, args, png.replace(/\.png$/, '.out'), { cwd: copy, env: cloneWebEnv(env, wasm) });
      // A drive that failed before any layout (the simulator refusing a
      // launch, say) is driven once more, and says so.
      let retried = '';
      if (!r.ok && !r.out.includes('"nodes"')) {
        retried = r.out.trim().split('\n').filter(l => /rror/.test(l)).slice(0, 1).join('').slice(0, 160) || 'no layout';
        r = runToFile(process.execPath, args, png.replace(/\.png$/, '.out'), { cwd: copy, env: cloneWebEnv(env, wasm) });
      }
      // iOS's keyboard, where the shot needs it up: driven again while it is down.
      const kb = host === 'ios' && KEYBOARD_UP.includes(screen) ? keyboardRedrive(ALLOW[screen].target, REFUSED, r, () => runToFile(process.execPath, args, png.replace(/\.png$/, '.out'), { cwd: copy, env: cloneWebEnv(env, wasm) })) : { r, down: false, keyboard: '' };
      r = kb.r;
      const refused = [...new Set(r.out.match(new RegExp(`${REFUSED} [^"\\s]+`, 'g')) ?? [])];
      const said = refused.length ? `the fixture refused: ${refused.join(', ')}` : r.ok ? '' : r.out.trim().split('\n').slice(-2).join(' / ').slice(0, 300);
      const allow = r.ok ? allowOf(screen, r.out) : [];
      const shown = r.ok ? renderedIds(r.out) : null;
      rows.push({ host, scheme, screen, png, ok: r.ok && existsSync(png) && !refused.length && allow !== null && shown !== null && !kb.down, sha: existsSync(png) ? sha(readFileSync(png)) : null, allow: allow ?? [], shown: shown ?? [], named: r.ok ? namedOf(NAMED, screen, r.out) : [], components: r.ok ? renderedComponents(r.out) : [], appComponents: r.ok ? renderedComponents(r.out, true) : [], ...(retried ? { retried } : {}), ...(kb.keyboard ? { keyboard: kb.keyboard } : {}), said: kb.down ? 'the software keyboard never came up (three drives)' : allow === null ? `no layout for ${ALLOW[screen].target}` : shown === null ? 'no layout' : said });
    }
  }
  return rows;
}

/** The library components a drive's whole layout shows, by the source map
 * each shown node carries (its declaring file and component): one a
 * package's file declares, as `@exact/<package>/<file>#<Component>`. */
export function renderedComponents(out, app = false) {
  const line = out.split('\n').filter(l => l.startsWith('{') && l.includes('"nodes"') && l.includes('"viewport"')).at(-1);
  if (!line) return [];
  const layout = JSON.parse(line);
  // `app`: only those the copy's own files instantiate (the innermost call site, chain[0], in no package's file).
  const own = n => n.sourceMap?.chain?.[0]?.file && !/\/(?:packages|node_modules\/@exact)\//.test(n.sourceMap.chain[0].file);
  const ids = layout.nodes.filter(n => shown(n, layout.viewport) && (!app || own(n))).map(n => {
    // A package's file, in the checkout's packages/ or a copy's node_modules/@exact/.
    const m = /\/(?:packages|node_modules\/@exact)\/([\w-]+)\/([\w-]+)\.contract$/.exec(n.sourceMap?.file ?? '');
    return m && n.sourceMap.component ? `@exact/${m[1]}/${m[2]}.contract#${String(n.sourceMap.component).replace(new RegExp(`__${m[2].replace(/\W/g, '_')}$`), '')}` : null;
  });
  return [...new Set(ids.filter(Boolean))];
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
    os: `darwin ${release()}`, macos: run('sw_vers', ['-productVersion']).out.trim(), xcode, simulator: device, statusBar: STATUS_BAR.join(' '), epoch: EPOCH, size: { web: SIZE, 'web-wasm': SIZE, linux: SIZE, ios: 'the simulator\'s screen', macos: 'its own window' }, schemes: SCHEMES,
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
export { pixelDiff };

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
    const named = unionNamed(b.named, a.named);
    const d = pixelDiff(readFileSync(b.png), readFileSync(a.png), allow.map(r => r.rect), b.host.startsWith('web') ? WEB_NOISE : null, named);
    return { shot: key(b), ok: d.differ === 0, differ: d.differ, total: d.total, allow, ...(d.names ? { names: d.names, png: [b.png, a.png] } : {}) };
  });
}

/** The tag the baseline was taken at, in the copy. */
export let BASELINE_TAG = 's12e-baseline';
/** The copy's own duplicates a stage retires (stage 15 B), each a declaration's start: before declares each, after none. */
export const RETIRED = [];
/** The exports a stage's adoption instantiates from the copy's own files (stage 15 B's Avatar and VerifiedBadge, which
 * the packages placed inside PostCard and ProfileHeader already): after's files place each, before's none. */
export const APP_PLACED = [];
/** A stage's NAMED pixel changes (stage 15 B batch 2, scripts/stage12-named.mjs) and its bindings (theme2's option B). */
export const NAMED = [], BINDINGS = [];

/** The library components a source map's view nodes instantiate from a file no package's: the innermost call site
 * (`chain[0]`, the instantiation's span) in the copy's own sources, not inside another package component. */
export function appPlaced(map) {
  const home = new Map((map.sources ?? []).map(s => [s.file, s.home ?? '']));
  return [...new Set((map.nodes ?? []).filter(n => n.library && n.chain?.[0] && !(home.get(n.chain[0].file) ?? '').startsWith('@')).map(n => n.library))];
}

/** Whether the copy's tracked contracts declare `line` (a declaration's start, so a trailing comment still counts);
 * an error when git grep fails rather than answering. */
export function declares(copy, line) {
  const [keyword, name] = line.split(/\s+/), esc = t => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  // Any run of blanks between the keyword and the name; the name ends where a Contract identifier can't go on.
  const pattern = `^${esc(keyword)}[[:blank:]]+${esc(name)}([^A-Za-z0-9_-]|$)`;
  const r = spawnSync('git', ['grep', '-q', '-E', '-e', pattern, '--', '*.contract'], { cwd: copy, encoding: 'utf8' });
  if (r.status === 0 || r.status === 1) return { hit: r.status === 0 };
  return { error: `git grep in ${copy} exited ${r.status}: ${(r.stderr ?? '').trim().split('\n').at(-1)}` };
}

/** The files `before` differs from the baseline's tag by: only the lock a
 * newer SDK needs (`exact.mjs lock`) may differ. */
export function beforeDrift(before) {
  return run('git', ['diff', '--name-only', BASELINE_TAG, 'HEAD'], { cwd: before }).out.split('\n').filter(f => f && f !== 'Cargo.lock');
}

/** A copy's tracked changes not committed (staged or not): a build reads
 * the working tree, so a side must be its commit. */
export const uncommitted = copy => run('git', ['status', '--porcelain', '--untracked-files=no'], { cwd: copy }).out.split('\n').filter(Boolean);

/** The library components a copy's app renders: the identities
 * (`<home>#<component>`) of its plan's view nodes from a package's file,
 * by the build's source map (`contract build --map`), so an import its
 * views never place counts for nothing. */
function usesOf(copy, dir, env) {
  mkdirSync(dir, { recursive: true });
  const plan = join(dir, 'app.plan');
  const r = run(process.execPath, [join(copy, 'exact.mjs'), 'contract', 'build', 'app.contract', '-o', plan, '--map'], { cwd: copy, env });
  const mapFile = `${plan}.map.json`;
  if (!r.ok || !existsSync(mapFile)) return null;
  return renderedLibrary(JSON.parse(readFileSync(mapFile, 'utf8')));
}

/** A source map's view nodes from a package's file, as identities: the
 * component by its declared name (the build suffixes one whose name another
 * component shares with its file's stem, `AtCard__at_card`). */
export function renderedLibrary(map) {
  const home = new Map((map.sources ?? []).map(s => [s.file, s.home]));
  return [...new Set((map.nodes ?? []).map(n => {
    const h = home.get(n.file) ?? '';
    const stem = h.split('/').pop()?.replace(/\.contract$/, '').replace(/\W/g, '_');
    return `${h}#${String(n.component).replace(new RegExp(`__${stem}$`), '')}`;
  }).filter(id => id.startsWith('@')))];
}

/** Conform on the clone (theme2 22:49, 03:25 and 06:52): the adoption
 * fails nothing beyond the baseline's. The baseline is before's failures
 * seen in two full runs (one run's own boot flake no baseline). A side
 * with more than 2 failures beyond it is rerun once in full, and that run
 * stands. Then at most 2 remain, each a FLAKY candidate: the clone is one
 * target, so "alone" is its whole run, three times, none showing a failure
 * beyond the baseline. Each needs its id in QUEUE, and none may have been
 * seen in the gate's run before (`work`'s record). Anything else fails. */
export function cloneConform(copies, runOf, work, queue = readFileSync(join(ROOT, 'QUEUE.md'), 'utf8')) {
  const said = [], runs = { before: [], after: [] };
  const go = side => { const r = runOf(copies[side], side); if (r) runs[side].push(r); return r; };
  const b1 = go('before');
  const b2 = b1?.failures.length ? go('before') : b1;
  if (!b1 || !b2) return { ok: false, runs, flaky: [], said: ['conform on the clone: before did not run'] };
  const baseline = b1.failures.filter(f => b2.failures.includes(f));
  const beyond = r => r.failures.filter(f => !baseline.includes(f));
  let a = go('after');
  if (!a) return { ok: false, runs, flaky: [], said: ['conform on the clone: after did not run'] };
  if (beyond(a).length > 2) {
    said.push(`after: ${beyond(a).length} failures beyond the baseline, more than 2: rerun once in full (theme2 06:52)`);
    a = go('after');
    if (!a) return { ok: false, runs, flaky: [], said: [...said, 'conform on the clone: the full rerun did not run'] };
  }
  const added = beyond(a);
  const last = (() => { try { return JSON.parse(readFileSync(join(work, 'flaky-last-clone.json'), 'utf8')).failures ?? []; } catch { return []; } })();
  let why = null;
  if (added.length > 2) why = `${added.length} failures beyond the baseline after a full rerun: more than 2 (theme2 06:52)`;
  else if (added.some(f => last.includes(f))) why = 'beyond the baseline in the gate\'s run before too: a flake to chase (theme2 03:25 (2))';
  else if (added.length) {
    for (let n = 1; n <= 3 && !why; n++) { const r = go('after'); if (!r || beyond(r).length) why = `rerun ${n} of 3 ${r ? 'failed again' : 'did not run'} (theme2 03:25 (3))`; }
    const id = f => f.replace(/^FAIL \S+ /, '');
    if (!why && added.some(f => !queue.includes(id(f)))) why = 'passed 3 of 3, but QUEUE does not name each failure';
  }
  writeFileSync(join(work, 'flaky-last-clone.json'), JSON.stringify({ at: new Date().toISOString(), failures: added }));
  if (why) said.push(`  ${why}`);
  const flaky = why ? [] : added;
  if (flaky.length) said.push('  FLAKY: beyond the baseline in the full run, absent from 3 of 3 reruns (theme2 03:25, 06:52; QUEUE\'d):', ...flaky.map(f => `    ${f}`));
  return { ok: !why, before: { ...b2, failures: baseline }, after: a, runs, flaky, failing: why ? added : [], said };
}

export const conformSame = c => !!c.ok;
const conformEvidence = c => {
  if (!c.before || !c.after) return c.said;
  const line = (side, r, n) => `  ${side} run ${n}: ${r.summary.split(';')[0]} over ${r.compared} steps`;
  // Both lists whole, each failure by its line (theme2 22:49 (2)).
  return [`conform on the clone (host/web-js/conform.mjs --strict --linux, wasm against JS; theme2 22:49, 03:25, 06:52): ${c.ok ? (c.flaky.length ? `after's beyond the baseline only ${c.flaky.length} FLAKY` : 'after\'s a subset of the baseline') : 'FAILS'}`,
    ...c.runs.before.map((r, i) => line('before', r, i + 1)), ...c.runs.after.map((r, i) => line('after', r, i + 1)), ...c.said,
    ...(c.failing ?? []).map(f => `  NEW ${f}`), ...c.before.failures.map(f => `  baseline: ${f}`), ...c.after.failures.map(f => `  after: ${f}`)];
};

/** The SDK a copy builds with: its exact.mjs's EXACT2 (the environment's,
 * else its default). */
function sdkOf(copy) {
  const named = /const EXACT2 = resolve\(import\.meta\.dir, process\.env\.EXACT2 \?\? "([^"]+)"\)/.exec(readFileSync(join(copy, 'exact.mjs'), 'utf8'))?.[1];
  return process.env.EXACT2 ? resolve(process.env.EXACT2) : named ? resolve(copy, named) : null;
}

/** host/web-js/conform.mjs on a copy, run from the SDK it builds with (its
 * patches name that checkout's crates), by its app's name: its failures,
 * each a `FAIL` line with a screenshot's file name set aside; null when it
 * didn't run. */
export function conformOf(copy, side) {
  const sdk = sdkOf(copy), name = /name: "([^"]+)"/.exec(readFileSync(join(copy, 'exact.mjs'), 'utf8'))?.[1];
  if (!sdk || !name) return null;
  const work = join(ROOT, 'target', 'stage12-gate', `conform-${side}`);
  const r = run(process.execPath, [join(sdk, 'host/web-js/conform.mjs'), name, '--build', '--strict', '--linux', '--jobs', '1', '--wasm-root', join(work, 'wasm'), '--out', join(work, 'out')],
    { cwd: sdk, timeout: 7_200_000, env: { ...process.env, EXACT_APP_DIR: copy, CARGO_TARGET_DIR: join(ROOT, 'target', `stage12-gate-${side}`) } });
  return conformRead(r.out, name, r.error);
}

/** A conform run's output read: null unless it finished (no spawn error or
 * timeout), its table's row for the app compared steps, and it printed as
 * many FAIL lines as its summary counts. */
export function conformRead(out, name, error = null) {
  if (error) return null;
  const lines = out.split('\n');
  const summary = lines.find(l => /^\d+ failures? across \d+ targets?/.test(l));
  const row = lines.find(l => l.startsWith(`| ${name} |`))?.split('|').map(c => c.trim());
  const compared = Number(row?.[3]);
  const failures = conformFailures(out);
  if (!summary || !(compared > 0) || failures.length !== Number(/^(\d+)/.exec(summary)[1])) return null;
  return { summary, compared, failures };
}
export const conformFailures = out => out.split('\n').filter(l => l.startsWith('FAIL ')).map(l => l.replace(/\s*\([^()]*side-by-side\.png\)\s*$/, ''));

/** What the adoption must use: the four components from the two packages
 * (stage 12; a stage's profile adds its own, `useStage`). */
export const ADOPTED = ['@exact/cards/post-card.contract#PostCard', '@exact/cards/at-card.contract#AtCard', '@exact/actions/action-row.contract#ActionRow', '@exact/actions/post-button.contract#PostButton'];
/** What the baseline may use already: an earlier stage's adoption (none for stage 12). */
export const BEFORE_USES = [];

/** One side's builds, on every host, into a target of its own; iOS onto the
 * simulator, which both sides share (one app id), so a side is shot right
 * after its own build. */
function buildSide(copy, side) {
  const env = { ...process.env, CARGO_TARGET_DIR: join(ROOT, 'target', `stage12-gate-${side}`), CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? '4' };
  for (const verb of [['web-build'], ['web-build', '--wasm'], ['linux'], ['mac'], ['ios', ...(process.env.EXACT_SIM ? ['--sim', process.env.EXACT_SIM] : [])]]) {
    const wasm = verb[1] === '--wasm';
    const r = run(process.execPath, [join(copy, 'exact.mjs'), ...verb], { cwd: copy, env: cloneWebEnv(env, wasm) });
    if (!r.ok) return `${side}: ${verb.join(' ')} failed: ${r.out.trim().split('\n').slice(-2).join(' / ').slice(0, 300)}`;
  }
  if (jsTargetBuild(cloneWasmDist(env))) return `${side}: ${cloneWasmDist(env)} is not a wasm build`;
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
  // The adoption is the packages': after's views place all four library
  // components, before's none.
  const uses = Object.fromEntries([['before', opts.before], ['after', opts.copy]].map(([side, copy]) => [side, usesOf(copy, join(ROOT, 'target', 'stage12-gate', `${side}-map`), { ...process.env, CARGO_TARGET_DIR: join(ROOT, 'target', `stage12-gate-${side}`) })]));
  if (!uses.before || !uses.after) return fail(`a copy's app did not build with its source map: ${['before', 'after'].filter(s => !uses[s]).join(', ')}`);
  const missing = ADOPTED.filter(id => !uses.after.includes(id));
  if (missing.length) return fail(`--copy does not use ${missing.join(', ')}: not the adoption`);
  // A package's own helpers (its common.contract's, which an adopted
  // component places) count with the component, not on their own.
  const early = uses.before.filter(id => !BEFORE_USES.includes(id) && !/\/common\.contract#/.test(id));
  if (early.length) return fail(`--before already uses library components (${early.join(', ')}): not the baseline`);
  // A component the packages already placed inside another (stage 15 B's Avatar and VerifiedBadge, inside PostCard)
  // is no proof of the adoption: the copy's own files instantiating it is, and its own copies going.
  const maps = Object.fromEntries(['before', 'after'].map(side => [side, JSON.parse(readFileSync(join(ROOT, 'target', 'stage12-gate', `${side}-map`, 'app.plan.map.json'), 'utf8'))]));
  const unplaced = APP_PLACED.filter(id => !appPlaced(maps.after).includes(id)), placedBefore = APP_PLACED.filter(id => appPlaced(maps.before).includes(id));
  if (unplaced.length) return fail(`--copy's own files don't place ${unplaced.join(', ')}: not the adoption`);
  if (placedBefore.length) return fail(`--before's own files place ${placedBefore.join(', ')} already: not the baseline`);
  const said = RETIRED.flatMap(line => [['after', opts.copy], ['before', opts.before]].map(([side, copy]) => ({ line, side, ...declares(copy, line) })));
  const broken = said.filter(x => x.error), kept = said.filter(x => x.side === 'after' && x.hit), unborn = said.filter(x => x.side === 'before' && x.hit === false);
  if (broken.length) return fail(broken.map(x => x.error).join('; '));
  if (kept.length) return fail(`--copy still declares ${kept.map(x => x.line).join(', ')}: not the adoption`);
  if (unborn.length) return fail(`--before never declared ${unborn.map(x => x.line).join(', ')}: not the baseline`);
  // Theme2's option B: each binding draws nothing of its own and places only what it binds to (after's source map).
  const bound = [...bindingsCheck(maps.after, BINDINGS), ...BINDINGS.map(b => ({ binding: b.component, ...bindingShape(maps.after, readFileSync(join(opts.copy, b.file), 'utf8'), b) }))];
  if (bound.some(b => !b.ok)) return fail(...bound.map(b => `binding ${b.binding}: ${b.said}`));
  // Conform on the clone (the JS target's harness: the app's wasm build
  // against its JS build, step by step), on each side: the adoption adds no
  // failure the baseline doesn't have.
  const conform = cloneConform({ before: opts.before, after: opts.copy }, conformOf, join(ROOT, 'target', 'stage12-gate'));
  const shots = {};
  for (const [side, copy] of [['before', opts.before], ['after', opts.copy]]) {
    const said = buildSide(copy, side);
    if (said) return fail(said);
    const env = { ...process.env, CARGO_TARGET_DIR: join(ROOT, 'target', `stage12-gate-${side}`) };
    shots[side] = baseline(copy, join(ROOT, 'target', 'stage12-gate', side), CLONE_HOSTS, env);
  }
  const r = comparePixels(shots.before, shots.after);
  const bad = r.filter(x => !x.ok);
  // Named changes: each name's pixels and box, and both sides' crops of every named box on every shot, for theme2 to read.
  const crops = join(ROOT, 'target', 'stage12-gate', 'named-crops');
  const report = namedReport(NAMED, r), cropped = r.flatMap(x => (x.names ? namedCrops(x.png[0], x.png[1], x.names, crops, x.shot) : []));
  const missingNames = report.filter(n => n.status === 'MISSING');
  // Rendered: each library component shown on the screens shot (a testId
  // only its package's view writes, which the copy's own sources don't),
  // and none of PostCard's, ActionRow's or AtCard's before.
  const own = run('git', ['grep', '-l', '-F', ...RENDERED.flatMap(([, , text]) => ['-e', text]), '--', '*.contract'], { cwd: opts.copy }).out.trim().split('\n').filter(Boolean);
  const seen = side => new Set(shots[side].shots.flatMap(x => x.shown ?? []));
  const unseen = RENDERED.filter(([, is]) => ![...seen('after')].some(is)).map(([name]) => name);
  const shownEarly = RENDERED.filter(([name]) => name !== 'PostButton' && !BEFORE_USES.some(id => id.endsWith(`#${name}`)))
    .filter(([, is]) => [...seen('before')].some(is)).map(([name]) => name);
  // And by the source map (theme2 23:03): each adopted component shown on
  // some screen shot on every host, the screens that show it said; none of
  // the stage's own shown before.
  const onScreens = (side, host, id) => [...new Set(shots[side].shots.filter(x => x.host === host && (x.components ?? []).includes(id)).map(x => x.screen))];
  const hosts = [...new Set(shots.after.shots.map(x => x.host))];
  const byComponent = ADOPTED.map(id => ({ id, name: id.split('#')[1], per: Object.fromEntries(hosts.map(h => [h, onScreens('after', h, id)])) }));
  const notShown = byComponent.flatMap(c => hosts.filter(h => !c.per[h].length).map(h => `${c.name} on ${h}`));
  const shownBefore = ADOPTED.filter(id => !BEFORE_USES.includes(id)).filter(id => hosts.some(h => onScreens('before', h, id).length)).map(id => id.split('#')[1]);
  // An export the packages also place inside another counts only where the copy's own files instantiate it, shown.
  const appOn = (host, id) => shots.after.shots.some(x => x.host === host && (x.appComponents ?? []).includes(id));
  const appNotShown = APP_PLACED.flatMap(id => hosts.filter(h => !appOn(h, id)).map(h => `${id.split('#')[1]} on ${h}`));
  const p = shots.after.provenance;
  const evidence = [...(RERUN_SCREENS.length ? [`a rerun of the screens ${RERUN_SCREENS.join(', ')} only (--screen): the counts and lists below are theirs, not the stage's`] : []), `before ${shots.before.provenance.copy.slice(0, 7)} (${BASELINE_TAG}, lock for the SDK), after ${p.copy.slice(0, 7)}, SDK ${p.exact2?.slice(0, 10)}: ${r.length} shots, ${r.length - bad.length} identical outside their allowlists`,
    ...bad.map(x => `  ${x.shot}: ${x.differ ?? x.said}${x.differ ? ' px' : ''}`),
    ...['before', 'after'].flatMap(side => shots[side].shots.filter(x => x.retried).map(x => `  ${side} ${x.host} ${x.scheme} ${x.screen}: driven again after a drive that failed before any layout (${x.retried})`)),
    ...['before', 'after'].flatMap(side => shots[side].shots.filter(x => x.keyboard).map(x => `  ${side} ${x.host} ${x.scheme} ${x.screen}: ${x.keyboard}`)),
    'the allowlist (LLP 1139 §5: at most 10% of a library component\'s box): the caret strip lies inside the composer\'s own field (compose-text, the app\'s) and the system keyboard\'s rect below the viewport (from the layout\'s keyboard inset), so neither covers a library component\'s box, by construction (allowOf)',
    `the adoption places ${ADOPTED.length} of ${ADOPTED.length} library components in its views (${ADOPTED.map(i => i.split('#')[1]).join(', ')}; the build's source map), before ${BEFORE_USES.length ? `only the earlier stage's (${BEFORE_USES.map(i => i.split('#')[1]).join(', ')})` : 'none'}`,
    ...(TITLES['consumer-pixels'] && !unseen.length && !own.length && !shownEarly.length ? [] : [`rendered on the screens shot: ${RENDERED.length - unseen.length} of ${RENDERED.length} (${RENDERED.map(([name]) => name).filter(n => !unseen.includes(n)).join(', ')})${unseen.length ? `; NOT shown: ${unseen.join(', ')}` : ''}${own.length ? `; the copy's own sources write a package's testId: ${own.join(', ')}` : ''}${shownEarly.length ? `; before already shows ${shownEarly.join(', ')}` : ''}`]),
    `shown on the screens shot, by each node's source map, every host: ${byComponent.length - new Set(notShown.map(x => x.split(' on ')[0])).size} of ${byComponent.length}${notShown.length ? `; NOT shown: ${notShown.join(', ')}` : ''}${shownBefore.length ? `; before already shows ${shownBefore.join(', ')}` : ''}`,
    ...(APP_PLACED.length ? [`shown where the copy's own files instantiate them, every host: ${APP_PLACED.map(id => id.split('#')[1]).join(', ')}${appNotShown.length ? `; NOT shown: ${appNotShown.join(', ')}` : ''}`] : []),
    ...byComponent.map(c => { const all = [...new Set(Object.values(c.per).flat())]; const same = hosts.every(h => c.per[h].length === all.length); return `  ${c.name}: ${same ? all.join(', ') : hosts.map(h => `${h} ${c.per[h].join(', ') || 'none'}`).join('; ')}`; }),
    ...bound.map(b => `binding ${b.binding}: ${b.said}`),
    ...(NAMED.length ? [`named changes (theme2 23:16: inside each named box allowed and reported, strict outside; crops of every named box on every shot in ${crops}, ${cropped.length} files):`,
      ...report.map(n => `  ${n.status} ${n.screen} ${n.target}: ${n.change}; ${n.differ} px over ${n.shots.length} of ${n.seen} shots${n.shots.length ? `, boxes ${n.shots.map(h => `${h.shot.split(' ').slice(0, 2).join('-')} ${h.box.join(',')}`).join('; ')}` : ''}`)] : []),
    ...conformEvidence(conform)];
  return missingNames.length || bad.length || !r.length || !conformSame(conform) || unseen.length || own.length || shownEarly.length || notShown.length || shownBefore.length || appNotShown.length ? fail(...evidence) : pass(...evidence);
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
export function drivePlan(host, plan, png, steps = [], app = null, driver = scaleDrive) {
  if (host === 'web-js') {
    if (!jsTargetBuild(webDist())) throw new Error(`the web dist ${webDist()} is not a JS-target build (bun host/web/build.mjs <app>)`);
    return driver('web', plan, png, steps, app);
  }
  if (host === 'web-wasm') {
    // library-gate's own artifact (apps/library-gate/web, theme2 13:47): a
    // wasm links what its own plan uses, so another app's refuses a cell
    // using what that app doesn't (Unlinked).
    const dist = wasmDist(), build = `EXACT_APP_DIR=${LIBRARY_GATE} EXACT_WEB_DIST=${dist} bun host/web/build.mjs library-gate --wasm`;
    if (!existsSync(join(dist, '.exact-build.json')) || jsTargetBuild(dist)) throw new Error(`${dist} is not a wasm build (${build})`);
    if (JSON.parse(readFileSync(join(dist, '.exact-build.json'), 'utf8')).app?.id !== 'com.exact.library-gate') throw new Error(`${dist} is not library-gate's wasm build (${build})`);
    const was = { dist: process.env.EXACT_WEB_DIST, app: process.env.EXACT_APP_DIR };
    Object.assign(process.env, { EXACT_WEB_DIST: dist, EXACT_APP_DIR: LIBRARY_GATE });
    try { return driver('web', plan, png, steps, app); } finally {
      for (const [k, v] of [['EXACT_WEB_DIST', was.dist], ['EXACT_APP_DIR', was.app]]) if (v === undefined) delete process.env[k]; else process.env[k] = v;
    }
  }
  return driver(host, plan, png, steps, app);
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

/** The library's components with a slot (library-gate.test.mjs checks each
 * of their cells holds its marker). */
export const SLOTTED = ['PostCard', 'ProfileHeader', 'ComposeFrame', 'BarItem', 'PersonRow', 'MenuGroup'];
/** The ids of library-gate's slotted cells (each a marker). */
export const markerIds = () => placements(readFileSync(join(LIBRARY_GATE, 'app.contract'), 'utf8'), SLOTTED).map(p => p.id);

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
export function viewportTop(img, layout, unscrolled = null) {
  const k = img.width / layout.viewport.w;
  if (img.height <= layout.viewport.h * k + 2) return 0;
  // A page scrolled past its top mark (a marker below the fold, tapped into
  // view) has the host's bars where an unscrolled shot of it had them.
  const mark = layout.nodes?.find(n => n.testId === 'gate-top');
  if (unscrolled != null && mark && mark.y + mark.h <= 0) return unscrolled;
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
 * its cells in the two text shots). A tap on the marker toggles every
 * glyph. */
export function slotOn(host, plan, dir, app = 'unsaid') {
  const ids = markerIds();
  if (!ids.length) return [{ host, id: '(none)', ok: false, why: 'apps/library-gate places no slotted cell' }];
  if (host === 'terminal') {
    // Each marker tapped (a pressable box taps there since 2193289076), its
    // cells read in the two text shots. The terminal taps only what it
    // shows, and library-gate's page is taller than 64 rows with nothing
    // scrolling it (its root grows), so the screen is made as tall as the
    // page first.
    const tall = 'resize 64x512';
    return ids.map(id => {
      const a = join(dir, `terminal-${id}-a.txt`), b = join(dir, `terminal-${id}-b.txt`);
      let layout;
      try { layout = drivePlan('terminal', plan, null, [tall, `tap marker-${id}`, 'layout', `screenshot ${a}`, `tap marker-${id}`, `screenshot ${b}`]); } catch (e) { return { host, id, ok: false, why: whyOf(e) }; }
      const first = layout.earlier?.[0];
      if (!first) return { host, id, ok: false, why: 'the first render\'s layout was not read' };
      const [A, B] = [markerIn(first, id), markerIn(layout, id)];
      if (A.why || B.why) return { host, id, ok: false, why: A.why || B.why };
      const [ca, c] = [A.n.cells, B.n.cells];
      if (!c || c.w <= 0 || c.h <= 0) return { host, id, ok: false, why: 'no cells laid out' };
      if (!ca || !sameRect(ca, c)) return { host, id, ok: false, why: `its cells moved between the renders (${JSON.stringify(ca)} then ${JSON.stringify(c)})` };
      const [ta, tb] = [a, b].map(f => readFileSync(f, 'utf8').split('\n'));
      const cut = t => Array.from({ length: c.h }, (_, r) => [...(t[c.y + r] ?? '')].slice(c.x, c.x + c.w).join(''));
      const differ = cut(ta).join('\n') !== cut(tb).join('\n');
      return { host, id, ok: differ, why: differ ? '' : 'its cells read the same with both glyphs', cells: c };
    });
  }
  // Where the viewport starts in a shot of the page at its top, for a shot
  // scrolled past its top mark.
  let unscrolled = null;
  try {
    const png = join(dir, `${host}-unscrolled.png`), l = drivePlan(host, plan, png, [], app);
    unscrolled = viewportTop(decodePng(readFileSync(png)), l);
  } catch { /* each marker's own shot says why */ }
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
      const [topA, topB] = [viewportTop(IA, first, unscrolled), viewportTop(IB, layout, unscrolled)];
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
 * apps/library-gate's design and then removed. Each read is a length
 * measured on the layout (`at`, in points) on every instance in `ids` (its
 * id in library-gate) and what it is: `is` the value itself, `twice` /
 * `once` its change from the base drive (a box that grows on both sides,
 * or moves), or `same` (an instance that reads another token, which must
 * not move). `axis` says which way a terminal cell rounds it (none for a
 * size). */
const POST_CARDS = ['pc-base', 'pc-inthread', 'pc-lineabove', 'pc-linebelow', 'sample-pc-pinned', 'sample-pc-reposted'];
const ACTION_ROWS = ['ar-base', 'ar-big', 'ar-haptics', 'ar-reposted', 'ar-liked', 'sample-ar-replydisabled', 'ar-saved'];
const POST_BUTTONS = ['pb-base', 'sample-pb-reply', 'sample-pb-disabled'];
export const RESTYLE = [
  {
    name: 'space.md', what: 'a base name', set: '24px', unset: { op: 'remove-token', name: 'space.md' },
    reads: [
      // cards.pad-end, and in a thread cards.pad-end-thread: both space.md.
      { who: 'PostCard', what: 'its end padding (cards.pad-end, -thread)', ids: POST_CARDS, is: true, axis: 'w', at: (L, id) => right(L, `post-${id}`) - right(L, `postcard-content-${id}`) },
      // Between its counted buttons' row and save's: three columns of flex
      // 1 each, like's as wide as repost's or, squeezed, its own button
      // (a flex item's min-content), so the first row ends there.
      { who: 'ActionRow', what: 'its gap (param gap, actions.row-gap)', ids: ACTION_ROWS, is: true, axis: 'w', at: (L, id) => {
        const like = box(L, `like-${id}`);
        return box(L, `bookmark-${id}`).x - (like.x + Math.max(like.x - box(L, `repost-${id}`).x, like.w));
      } },
      { who: 'AtCard', what: 'its padding (cards.link-pad), the visit button\'s move', ids: ['at-base'], once: true, axis: 'w', at: (L, id) => box(L, `atcard-visit-${id}`).x - box(L, `atcard-${id}`).x },
      { who: 'PostButton', what: 'its side padding (actions.post-pad), its width', ids: POST_BUTTONS, twice: true, axis: 'w', at: (L, id) => box(L, `compose-post-${id}`).w },
    ],
  },
  {
    name: 'cards.pad-start', what: 'a package token', set: '20px', unset: { op: 'remove-token', name: 'cards.pad-start' },
    reads: [
      { who: 'PostCard', what: 'its start padding', ids: POST_CARDS.filter(id => id !== 'pc-inthread'), is: true, axis: 'w', at: (L, id) => box(L, `postcard-avatar-${id}`).x - box(L, `post-${id}`).x },
      { who: 'PostCard', what: 'its start padding in a thread (cards.pad-start-thread, not this token)', ids: ['pc-inthread'], same: true, axis: 'w', at: (L, id) => box(L, `postcard-avatar-${id}`).x - box(L, `post-${id}`).x },
    ],
  },
  {
    // A library component's param by its full name (LLP 1139 D8).
    name: '@exact/actions/post-button.contract#PostButton.height', param: true, what: 'a param', set: '40px', unset: { op: 'unset-param', name: '@exact/actions/post-button.contract#PostButton.height' },
    // A size, not a space: the terminal keeps it in points (only spaces
    // are whole cells, LLP 1121 stage 11 (c)).
    reads: [{ who: 'PostButton', what: 'its height', ids: POST_BUTTONS, is: true, axis: null, at: (L, id) => box(L, `compose-post-${id}`).h }],
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
export async function restyleWrites(dir, list = RESTYLE) {
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
    for (const c of list) {
      const set = c.ops?.set ?? [c.param ? { op: 'set-param', name: c.name, value: c.set } : { op: 'set-token', name: c.name, value: c.set }];
      const unset = c.ops?.unset ?? [c.unset];
      const said = (r, what) => { if (!r.ok) throw new Error(`${c.name}: ${what}: ${r.error?.message ?? JSON.stringify(r.error)}`); return r.ok; };
      const proposed = said(await server.ask({ method: 'propose', ops: set, files }), 'propose');
      const written = said(await server.ask({ method: 'transaction', client, ops: set, files }), 'transaction');
      files = written.files;
      const withLine = readFileSync(design, 'utf8');
      const removal = said(await server.ask({ method: 'propose', ops: unset, files }), 'propose its removal');
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
  if (read.same) return base;
  if (read.is) return q(v);
  return base + (read.twice ? 2 : 1) * (q(v) - q(v0));
}

/** The restyle line on `hosts`: library-gate's base drive, then each case's
 * set and removed designs, measured against `scale::column_for`'s value for
 * the host (scale-check's `baseLength` over kernel/tables/schema.json and
 * the design's own px tokens). */
export async function restyle(opts) {
  const dir = join(ROOT, 'target', 'stage12-gate', 'restyle');
  // Stage 13 C's cases beside stage 12's (theme2 10:28): their writes as one.
  const exact = EXACT ? exactCases().map(c => ({ ...c, ops: caseOps(c) })) : [];
  let writes;
  try { writes = await restyleWrites(dir, [...RESTYLE, ...exact]); } catch (e) { return fail(`the design server: ${String(e?.message ?? e).slice(0, 300)}`); }
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
  const rows = [], exactRows = [];
  for (const host of hosts) {
    const L = {};
    try {
      for (const [k, plan] of Object.entries(plans)) {
        if (k === 'base' || RESTYLE.some(c => k.startsWith(`${c.name} `))) L[k] = drivePlan(host, plan, join(dir, `${host}-${k.replace(/\W+/g, '-')}.png`), [], opts.app);
      }
    } catch (e) { rows.push({ host, ok: false, why: whyOf(e) }); continue; }
    const cell = L.base.cell ?? null;
    for (const c of RESTYLE) {
      // The web's two targets read one column (the neutral look).
      const v0 = lengthOf(c.param ? paramOf(c)?.default : c.name, host.startsWith('web-') ? 'web' : host, cell);
      const v1 = Number(/^(\d+(?:\.\d+)?)px$/.exec(c.set)[1]);
      for (const read of c.reads) for (const id of read.ids) {
        const row = { host, case: c.name, who: read.who, what: read.what, id, ok: true, why: [] };
        try {
          const b = read.at(L.base, id), s = read.at(L[`${c.name} set`], id), r = read.at(L[`${c.name} removed`], id);
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
    if (exact.length) exactRows.push(...readExact(host, exact, plans, opts.app));
  }
  const missing = SLOT_HOSTS.filter(h => !hosts.includes(h));
  const bad = rows.filter(r => !r.ok);
  const n = x => (Number.isFinite(x) ? Math.round(x * 100) / 100 : x);
  // The mutation (theme2 10:28 (4)), on Linux: the case fails, naming the token.
  let mutation = null;
  if (exact.length && hosts.includes('linux')) {
    try {
      const caught = [...new Set(mutationFails(join(dir, 'mutation'), 'linux').map(r => `${r.key} ${r.row}`))];
      const want = `${MUTATION.token} ${MUTATION.row}`;
      mutation = { ok: caught.length === 1 && caught[0] === want, line: `the mutation (a scratch @exact/actions, ${MUTATION.file}'s ${MUTATION.from} made ${MUTATION.to}), on linux: ${MUTATION.case} fails naming ${caught.join(', ') || 'nothing'}${caught.length === 1 && caught[0] === want ? '' : `, not only ${want}`}` };
    } catch (e) { mutation = { ok: false, line: `the mutation: ${whyOf(e)}` }; }
  }
  const evidence = [
    ...writes.map(w => `${w.name} (${RESTYLE.find(c => c.name === w.name)?.what ?? 'stage 13 C\'s batch'}): propose ${w.classes.set}, written ${w.classes.written}, its removal proposed ${w.classes.removal}, undone ${w.classes.undo}; the design's bytes ${w.restored ? 'restored' : 'NOT restored'}`),
    // One line per host, case and read: its instances, and each that failed.
    ...[...new Set(rows.filter(r => r.case).map(r => `${r.host}\u0000${r.case}\u0000${r.what}`))].flatMap(key => {
      const group = rows.filter(r => `${r.host}\u0000${r.case}\u0000${r.what}` === key), [first] = group, failed = group.filter(r => !r.ok);
      return [`${failed.length ? 'FAIL' : 'ok  '} ${first.host} ${first.case}: ${first.who} ${first.what}: ${group.length - failed.length} of ${group.length} instances; ${first.id}: before ${n(first.before)}, set ${n(first.set)}, removed ${n(first.removed)} (expected ${n(first.want?.[0])}, ${n(first.want?.[1])})`,
        ...failed.map(r => `      ${r.id}: before ${n(r.before)}, set ${n(r.set)}, removed ${n(r.removed)} (expected ${n(r.want?.[0])}, ${n(r.want?.[1])}): ${r.why}`)];
    }),
    ...rows.filter(r => !r.case).map(r => `FAIL ${r.host}: ${r.why}`),
    ...exactEvidence(exact, exactRows),
    ...(mutation ? [`${mutation.ok ? 'ok  ' : 'FAIL'} ${mutation.line}`] : []),
    ...missing.map(h => `  ${h}: a required host not run`),
    ...skipped,
  ];
  const exactBad = exactRows.some(r => !r.ok) || (exact.length && !exactRows.length) || (mutation && !mutation.ok);
  return bad.length || exactBad || missing.length || !rows.length || writes.some(w => !w.restored) ? fail(...evidence) : pass(...evidence);
}

/** Sources that fail to build at both ends of every run, named: apps/expose
 * reads `./local`, which a checkout lacks (stage 11's gate, the same). */
/** Whether the restyle line runs stage 13 C's exact cases (`useStage`). */
let EXACT = false;
/** The release comparison's runs when a stage names them (`useStage`). */
let RELEASE = null;
/** A stage's own line headlines (`useStage`). */
let TITLES = {};

/** Stage 13 C's cases on `host` (theme2 10:28, 10:42): the runner's tree
 * of the base design names each read's node, by its instance's first
 * testId and a path; a drive of the base design, then of each case set and
 * removed, reads every node's rows (kind 1, exact). Each drive, the base's
 * included, has the same node at each path, or it is no read of that node. One row a
 * read, instance and row: { host, case, key, kind, id, row, ok, why }. */
export function readExact(host, exact, plans, app, drive = (plan, ids) => drivePlan(host, plan, null, ids, app, (h, p, _png, list, a) => inspectDrive(h, p, list, a))) {
  const out = [];
  let tree;
  try { tree = drive(plans.base, []).tree; } catch (e) { return [{ host, ok: false, why: whyOf(e) }]; }
  // With the exports' case, VerifiedBadge's "" cell: its box, and nothing in it.
  if (exact.some(c => c.batch === 'Avatar and VerifiedBadge')) {
    for (const inst of EMPTY) {
      const box = locate(tree, inst, '.'), n = box?.children?.length ?? 0;
      out.push({ host, empty: true, inst, ok: !!box && n === 0, why: box ? `${n} node${n === 1 ? '' : 's'} in it` : 'not in the tree' });
    }
  }
  const reads = exact.map(c => ({ c, reads: caseReads(c).flatMap(r => r.ids.map(inst => ({ ...r, inst, root: COMPONENTS[r.component].root(inst) }))) }));
  for (const { reads: list } of reads) for (const r of list) r.id = locate(tree, r.root, r.path)?.id ?? null;
  const ids = list => [...new Set(list.map(r => r.id).filter(id => id != null))];
  let base;
  try { base = drive(plans.base, ids(reads.flatMap(x => x.reads))); } catch (e) { return [{ host, ok: false, why: whyOf(e) }]; }
  for (const { c, reads: list } of reads) {
    const at = {};
    for (const side of ['set', 'removed']) {
      try { at[side] = drive(plans[`${c.name} ${side}`], ids(list)); } catch (e) { at[side] = e; }
    }
    for (const r of list) {
      for (const row of r.rows) {
        const o = { host, case: c.name, key: r.key, kind: r.kind, inst: r.inst, path: r.path, row, want: r.want, ok: true, why: [] };
        if (r.id == null) o.why.push(`no node at ${r.root} ${r.path}`);
        const value = side => {
          const d = side === 'base' ? base : at[side];
          if (d instanceof Error) throw d;
          if (locate(d.tree, r.root, r.path)?.id !== r.id) throw new Error(`${side}: another node at ${r.root} ${r.path}`);
          const v = d.nodes.get(r.id)?.style?.[row];
          if (!v) throw new Error(`${side}: no ${row} on #${r.id}`);
          // The JS target's typed computed value, before layout, where it gives one.
          return v.computed ?? v.value;
        };
        try {
          const b = value('base'), sv = value('set'), rv = value('removed');
          Object.assign(o, { before: b, set: sv, removed: rv });
          if (!standsApart(b, r.want, r.kind, row)) o.why.push(`the set value ${r.want} is within the read's tolerance of the base ${JSON.stringify(b)}`);
          if (!matches(sv, r.want, r.kind, row)) o.why.push(`set ${JSON.stringify(sv)}, expected ${r.want}`);
          if (!same(rv, b, r.kind)) o.why.push(`removed ${JSON.stringify(rv)}, not back to ${JSON.stringify(b)}`);
        } catch (e) { o.why.push(String(e?.message ?? e)); }
        o.ok = !o.why.length;
        o.why = o.why.join('; ');
        out.push(o);
      }
    }
  }
  return out;
}

/** The mutation (theme2 10:28 (4)): library-gate built against a scratch
 * copy of @exact/actions whose HeaderButton sizes its width by
 * actions.header-icon where it reads actions.header-slot, a length for a
 * length; its case, set and removed, read on `host`. The reads that fail:
 * the case catches the swap only if they are the width's, naming
 * actions.header-slot. */
/** library-gate's plan with token case `name` set (its lines in the
 * design), built inside this checkout so the packages resolve. */
export function s13cPlan(name) {
  const c = exactCases().find(x => x.name === name);
  const design = readFileSync(join(LIBRARY_GATE, 'app.tamber'), 'utf8').trimEnd();
  const lines = c.set.map(t => (c.param ? null : `  ${t.name}${t.kind === 'color' ? '' : `: ${t.kind}`} = ${t.value}`)).filter(Boolean);
  return restylePlan(join(ROOT, 'target', 'stage12-gate', 'boot', name.replace(/\W+/g, '-')), `${design}\n${lines.join('\n')}\n`);
}

export const MUTATION = { file: 'header-button.contract', from: 'width="actions.header-slot"', to: 'width="actions.header-icon"', token: 'actions.header-slot', row: 'width', case: 'HeaderButton tokens' };
export function mutationFails(dir, host = 'linux', drive = undefined) {
  rmSync(dir, { recursive: true, force: true });
  for (const pkg of ['actions', 'cards', 'lists']) cpSync(join(ROOT, 'packages', pkg), join(dir, 'node_modules', '@exact', pkg), { recursive: true });
  const file = join(dir, 'node_modules/@exact/actions', MUTATION.file), text = readFileSync(file, 'utf8');
  if (!text.includes(MUTATION.from)) throw new Error(`${MUTATION.file} no longer says ${MUTATION.from}: the mutation needs another use`);
  writeFileSync(file, text.replace(MUTATION.from, MUTATION.to));
  const c = exactCases().find(x => x.name === MUTATION.case);
  const design = readFileSync(join(LIBRARY_GATE, 'app.tamber'), 'utf8').trimEnd();
  const lines = c.set.map(t => `  ${t.name}${t.kind === 'color' ? '' : `: ${t.kind}`} = ${t.value}`);
  const plan = (name, tamber) => {
    const app = join(dir, name, 'app');
    mkdirSync(app, { recursive: true });
    writeFileSync(join(app, 'app.contract'), readFileSync(join(LIBRARY_GATE, 'app.contract')));
    writeFileSync(join(app, 'app.tamber'), tamber);
    const out = join(dir, name, 'library-gate.plan');
    const r = run('cargo', ['run', '-q', '-p', 'contract', '--', 'build', join(app, 'app.contract'), '-o', out]);
    if (!r.ok) throw new Error(`the mutated library-gate did not compile (${name}): ${r.out.slice(-400)}`);
    return out;
  };
  const base = plan('base', `${design}\n`);
  const plans = { base, [`${c.name} set`]: plan('set', `${design}\n${lines.join('\n')}\n`), [`${c.name} removed`]: base };
  return readExact(host, [c], plans, null, ...(drive ? [drive] : [])).filter(r => !r.ok);
}

/** The exact cases' evidence: each read's kind once (every read is kind
 * 1, the computed row on its node), then a line a host and case with its
 * reads' count, and every failed read by name. */
export function exactEvidence(exact, rows) {
  if (!exact.length) return [];
  const keys = [...new Set(exact.flatMap(c => caseReads(c).map(r => r.key)))];
  const n = x => (typeof x === 'number' ? Math.round(x * 100) / 100 : JSON.stringify(x));
  return [
    `stage 13 C's cases and stage 15 B's exports' case, every read kind 1 (exact: the row's computed value on its node, layout <id>), checked against its own token's or param's value; ${keys.length} tokens and params read:`,
    ...keys.map(k => `  ${k}: ${readsOf(k).map(r => `${r.path} ${r.rows.join(',')} (${r.ids.length})`).join('; ')}`),
    ...[...new Set(rows.filter(r => r.case).map(r => `${r.host}\u0000${r.case}`))].flatMap(key => {
      const group = rows.filter(r => `${r.host}\u0000${r.case}` === key), failed = group.filter(r => !r.ok), [first] = group;
      return [`${failed.length ? 'FAIL' : 'ok  '} ${first.host} ${first.case}: ${group.length - failed.length} of ${group.length} reads exact, each back at its base once removed`,
        ...failed.slice(0, 40).map(r => `      ${r.key} ${r.inst} ${r.path} ${r.row}: before ${n(r.before)}, set ${n(r.set)}, removed ${n(r.removed)}: ${r.why}`),
        ...(failed.length > 40 ? [`      … and ${failed.length - 40} more`] : [])];
    }),
    ...rows.filter(r => r.empty).map(r => `${r.ok ? 'ok  ' : 'FAIL'} ${r.host} ${r.inst}: VerifiedBadge "none" draws nothing, its box empty${r.ok ? '' : ` (${r.why})`}`),
    ...rows.filter(r => !r.case && !r.empty).map(r => `FAIL ${r.host} stage 13 C's cases: ${r.why}`),
  ];
}

export const FAIL_ALIKE = ['apps/expose/app.contract'];

/** `--release`'s comparison (~/x2ios/rel12/release-groups.py, as stage
 * 11's: every app's plan and the press and motion corpus built at the
 * parent and the end of each run of stage 12 E commits on main): PASS when
 * every run compares every app now in apps/ and the corpus, each built at
 * both ends and byte-identical, but apps/library-gate's own (the gate app
 * those commits make and change) and FAIL_ALIKE's. */
export function releaseIdentity(file, sources = releaseSources(), expectedRuns = releaseRuns(), inTree = (rev, src) => run('git', ['cat-file', '-e', `${rev}:${src}`]).ok) {
  if (!file || !existsSync(file)) return { status: 'WAITS', on: 'the release identity run (--release <its JSON>)', evidence: [] };
  const rows = JSON.parse(readFileSync(file, 'utf8'));
  if (!Array.isArray(rows) || !rows.length) return { status: 'FAIL', evidence: ['release identity: no comparisons'] };
  const malformed = rows.filter(r => typeof r?.run !== 'string' || typeof r?.source !== 'string' || typeof r?.status !== 'string' || !Array.isArray(r?.commits) || !r.commits.length);
  if (malformed.length) return { status: 'FAIL', evidence: [`release identity: ${malformed.length} rows lack a run, source, status or commits`] };
  const own = r => r.source === 'apps/library-gate/app.contract';
  const alike = r => r.status === 'both failed' && FAIL_ALIKE.includes(r.source);
  const count = st => rows.filter(r => r.status === st).length;
  const unnamed = rows.filter(r => r.status !== 'identical' && !own(r) && !alike(r));
  const runs = [...new Set(rows.map(r => r.run))];
  // Every run of stage 12 E commits on this checkout's history, found here
  // as the script finds them, and no other.
  const absent = expectedRuns.filter(run => !runs.includes(run)).map(run => `  RUN NOT COMPARED ${run}`);
  const label = RELEASE?.label ?? 'stage 12 E';
  const extra = runs.filter(run => !expectedRuns.includes(run)).map(run => `  NOT A RUN OF ${label.toUpperCase()} ${run}`);
  // A source that isn't in the tree at either end of a run (an app added
  // since) has nothing to compare there.
  const short = runs.flatMap(r => sources.filter(src => !rows.some(x => x.run === r && x.source === src) && r.split('..').some(rev => inTree(rev, src))).map(src => `  NOT COMPARED ${src} over ${r}`));
  const evidence = [`release identity: ${rows.length} comparisons over ${runs.length} runs of ${label} commits; ${count('identical')} identical, ${rows.filter(own).length} apps/library-gate's own, ${rows.filter(alike).length} failing alike (${FAIL_ALIKE.join(', ')})`,
    ...unnamed.map(r => `  ${r.status === 'DIFFERS' ? 'MOVED' : r.status === 'both failed' ? 'FAILED AT BOTH ENDS' : 'BUILD FAILED'} ${r.source} over ${r.run} (${r.commits[0]})`), ...short, ...absent, ...extra];
  return { status: unnamed.length || short.length || absent.length || extra.length ? 'FAIL' : 'PASS', evidence };
}

/** The runs of consecutive stage 12 E commits from LLP 1139's acceptance
 * to HEAD, in topological order (~/x2ios/rel12/release-groups.py's
 * matching), each `<parent>..<last>` by ten-character hashes. */
export const STAGE12E = /^Stage 12 E|^LLP 1139 stage 12 E|^stage12-gate/;
export function releaseRuns(from = '92f1cae184') {
  // A stage whose commits landed as named ranges (s13c's pieces): those.
  if (RELEASE) return RELEASE.runs.map(([first, last]) => `${run('git', ['rev-parse', `${first}^`]).out.trim().slice(0, 10)}..${run('git', ['rev-parse', last]).out.trim().slice(0, 10)}`);
  const log = run('git', ['log', '--topo-order', '--reverse', '--format=%H %s', `${from}^..HEAD`]).out.trim().split('\n').filter(Boolean);
  const runs = [];
  let cur = null;
  for (const line of log) {
    const [h, ...subject] = line.split(' ');
    if (STAGE12E.test(subject.join(' '))) {
      cur ??= { base: run('git', ['rev-parse', `${h}^`]).out.trim() };
      cur.end = h;
    } else if (cur) { runs.push(cur); cur = null; }
  }
  if (cur) runs.push(cur);
  return runs.map(r => `${r.base.slice(0, 10)}..${r.end.slice(0, 10)}`);
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
  // This run's own report: an earlier one is never read for it.
  rmSync(join(work, 'conform', 'report.json'), { force: true });
  const conform = run(process.execPath, [join(ROOT, 'host/web-js/conform.mjs'), '--synthetic', '--strict', '--linux', '--build', '--jobs', '1', '--wasm-root', join(work, 'wasm'), '--out', join(work, 'conform')], { timeout: 7_200_000 });
  const said = conform.out.split('\n');
  const summary = said.find(l => /failures across/.test(l)) ?? `no summary${conform.error ? ` (${conform.error.code ?? conform.error.message})` : ''}: ${conform.out.trim().split('\n').slice(-3).join(' / ').slice(0, 400)}`;
  const failed = said.filter(l => /^[\w-]+: [1-9]\d* failures?$/.test(l));
  const readers = run('git', ['grep', '-l', '@exact/reading', '--', 'apps/*/app.contract']).out.trim().split('\n').filter(Boolean);
  // A failure the branch's run shares with the same run on its base is the
  // base's (stage 9's and 11's precedent: a conform red identical on main
  // is unchanged), so the line holds when the branch adds none.
  const finished = !conform.error && said.some(l => /failures across/.test(l));
  const mine = finished ? conformReport(join(work, 'conform', 'report.json'), ROOT) : null;
  const base = opts.conformBase ? conformReport(opts.conformBase, run('git', ['rev-parse', '--show-toplevel'], { cwd: dirname(resolve(opts.conformBase)) }).out.trim()) : null;
  const added = mine && base ? mine.failures.filter(f => !base.failures.includes(f)) : null;
  const flakes = added?.length ? flakyOf(added, base, work) : { flaky: [], seen: [], failing: added ?? [], said: [] };
  // Every target this run saw fail that its base doesn't (FLAKY or not),
  // which the next run reads; a finished run with none clears the record.
  // With no base to tell them apart, every failing target counts as seen;
  // a run whose report didn't read leaves the record as it was.
  const seen = !mine ? null : base ? flakes.seen : [...new Set(mine.failures.map(f => f.split(' | ')[0]))];
  if (finished && seen) writeFileSync(join(work, 'flaky-last.json'), JSON.stringify({ at: new Date().toISOString(), head: run('git', ['rev-parse', 'HEAD']).out.trim(), targets: seen }));
  const clean = conform.ok || !!(mine && base && mine.targets > 0 && base.targets > 0 && !flakes.failing.length);
  const evidence = [`conform --synthetic --strict --linux: ${summary}`, ...failed.map(l => `  ${l}`),
    ...(conform.ok ? [] : !base ? ['  its failures are compared with the same run on the branch\'s base (--conform-base <its report.json>), which wasn\'t given'] : !mine ? ['  its report.json was not read'] : [`  against the base's run (${opts.conformBase}, ${base.failures.length} failures over ${base.targets} targets): ${added.length ? `${added.length} the branch adds` : 'every failure the base\'s own'}`, ...flakes.failing.map(f => `    NEW ${f}`), ...mine.failures.filter(f => !added.includes(f)).map(f => `    both: ${f}`), ...flakes.said]),
    `@exact/reading's users: ${readers.join(', ') || 'none'} (in release identity)`];
  const release = releaseIdentity(opts.release);
  evidence.push(...release.evidence);
  if (!clean || release.status !== 'PASS') return release.status === 'WAITS' && clean ? waits(release.on, ...evidence) : fail(...evidence);
  return pass(...evidence);
}

/** theme2's 03:25 ruling: a target the base doesn't fail that fails in the
 * full run is rerun alone three times; all three passing lists it FLAKY
 * (not passed), its failure kept, and it needs a QUEUE line naming it. At
 * most 2 in a run; a target the gate's run before also saw failing (the
 * record noRegressions keeps in `work`, FLAKY or not) is a failure to chase; a rerun that fails is a failure,
 * with no further retries. A new failure in a target the base fails too is
 * no flake. Returns the flaky targets, the failures that stay, and what to
 * say. */
export function flakyOf(added, base, work, rerun = conformAlone, queue = readFileSync(join(ROOT, 'QUEUE.md'), 'utf8')) {
  const target = f => f.split(' | ')[0];
  const baseTargets = new Set(base.failures.map(target));
  const last = (() => { try { return JSON.parse(readFileSync(join(work, 'flaky-last.json'), 'utf8')).targets ?? []; } catch { return []; } })();
  const flaky = [], failing = [], said = [];
  for (const t of [...new Set(added.map(target))]) {
    const lines = added.filter(f => target(f) === t);
    const short = t.replace(/^synthetic-/, '');
    let why = null;
    if (baseTargets.has(t)) why = 'the base fails this target too: a new failure in it is no flake';
    else if (last.includes(t)) why = 'FLAKY in the gate\'s run before too: a flake to chase (theme2 03:25 (2))';
    else {
      for (let n = 1; n <= 3 && !why; n++) if (!rerun(t, n)) why = `rerun alone ${n} of 3 failed (theme2 03:25 (3))`;
      // A QUEUE entry of its own: "Conformance's `<name>` fixture fails now
      // and then", with the evidence.
      if (!why && !queue.split('\n').some(l => l.startsWith(`- **Conformance's \`${short}\` fixture fails now and then`))) why = `passed alone 3 of 3, but QUEUE has no entry "Conformance's \`${short}\` fixture fails now and then"`;
    }
    if (why) { failing.push(...lines); said.push(`    ${t}: ${why}`); } else flaky.push({ target: t, lines });
  }
  if (flaky.length > 2) { failing.push(...flaky.flatMap(f => f.lines)); said.push(`    ${flaky.length} FLAKY targets: more than 2 (theme2 03:25 (1))`); flaky.length = 0; }
  said.unshift(...flaky.flatMap(f => [`  FLAKY ${f.target}: failed in the full run, passed alone 3 of 3 (theme2 03:25; QUEUE'd); its failure:`, ...f.lines.map(l => `    ${l}`)]));
  // Every target seen failing here that the base doesn't, whatever became of it.
  const seen = [...new Set(added.map(target))].filter(t => !baseTargets.has(t));
  return { flaky: flaky.map(f => f.target), seen, failing, said };
}

/** One target's conform run alone (its dists from the full run): true when
 * it finished with no failure. */
function conformAlone(target, n) {
  const work = join(ROOT, 'target', 'stage12-gate');
  const which = target.startsWith('synthetic-') ? ['--synthetic', '--only', target.replace(/^synthetic-/, '')] : [target];
  const r = run(process.execPath, [join(ROOT, 'host/web-js/conform.mjs'), ...which, '--strict', '--linux', '--jobs', '1', '--wasm-root', join(work, 'wasm'), '--out', join(work, `alone-${target}-${n}`)], { timeout: 1_800_000 });
  return r.ok && /^0 failures across 1 targets?/m.test(r.out);
}

/** A conform report.json's failures, each `target | step | what` with the
 * checkout that ran it (`root`) taken out of its paths, and its targets
 * compared; null when unreadable. */
export function conformReport(file, root) {
  try {
    const r = JSON.parse(readFileSync(file, 'utf8'));
    // The checkout as given and as resolved (/tmp is /private/tmp), the
    // longer first.
    const bare = String(root).replace(/\/+$/, '');
    const prefixes = [...new Set([bare, existsSync(bare) ? realpathSync(bare) : bare])].sort((a, b) => b.length - a.length).map(p => `${p}/`);
    // Only where a path starts: the text's start, or after a space, quote,
    // bracket, colon or equals sign (not inside another path).
    const escape = t => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const strip = what => prefixes.reduce((w, p) => w.replace(new RegExp(`(^|[\\s"'«(\\[:=,])${escape(p)}`, 'g'), '$1<checkout>/'), String(what));
    return { targets: Object.keys(r.targets ?? {}).length, failures: (r.failures ?? []).map(f => `${f.target} | ${f.step} | ${strip(f.what)}`) };
  } catch { return null; }
}

/** A Rust Duration's Debug text (`250ns`, `1.2µs`, `3.4ms`, `1.5s`) in
 * milliseconds; NaN for another. */
export function durationMs(text) {
  const m = /^(\d+(?:\.\d+)?)(ns|µs|us|ms|s)$/.exec(String(text).trim());
  return m ? Number(m[1]) * { ns: 1e-6, µs: 1e-3, us: 1e-3, ms: 1, s: 1000 }[m[2]] : NaN;
}

/** The slot check's time (the lead's 10:15Z ask; rt1's stage 12 D2 tests in
 * tamber_serve_slots.rs): the clone's design alone, and with the real
 * @exact/cards and @exact/actions (an ignored test, which needs this
 * checkout's node_modules), each a touched name's check and every chain's,
 * under 10 ms. */
export const SLOT_BAR_MS = 10;
export function slotCheckTimes(out) {
  return out.split('\n').filter(l => l.startsWith('check_slots')).map(l => {
    const m = /one touched name (\S+), every chain (\S+)/.exec(l);
    return { line: l, real: l.includes('@exact/cards'), one: durationMs(m?.[1]), all: durationMs(m?.[2]) };
  });
}
function slotCheckTime() {
  const r = run('cargo', ['test', '-q', '-p', 'contract', '--test', 'it', 'the_slot_check_alone', '--', '--include-ignored', '--nocapture', '--test-threads', '1']);
  const times = slotCheckTimes(r.out);
  const real = times.find(t => t.real);
  const evidence = times.map(t => `${t.real ? 'with the real packages' : 'the clone\'s design alone'}: one touched name ${t.one} ms, every chain ${t.all} ms (${t.line.replace(/^check_slots,? ?/, '').slice(0, 160)})`);
  if (!r.ok || !real) return fail(...evidence, r.ok ? 'the real packages\' timing did not print (bun install: node_modules/@exact)' : `the tests failed: ${r.out.trim().split('\n').slice(-3).join(' / ').slice(0, 300)}`);
  const slow = times.filter(t => !(t.one < SLOT_BAR_MS && t.all < SLOT_BAR_MS));
  return slow.length ? fail(...evidence, `over ${SLOT_BAR_MS} ms or unread: ${slow.length}`) : pass(...evidence, `bar: ${SLOT_BAR_MS} ms a check`);
}

/** The fixture form's sign-in (the composer screen's steps, theme2 09:19 (c)
 * and 09:49): made-up credentials, the copy's synthetic data, no network. */
const SIGN_IN = ['tap bottomBarSignInBtn', 'type loginPasswordInput fixture-password', 'type loginUsernameInput me.fixture.invalid', 'tap loginNextButton'];
const SETTINGS = ['tap open-drawer', 'tap drawer-settings'];

/** Stage 13 C's adoption, screens and package testIds, which stage 15 B keeps. */
const S13C_ADOPTED = [...ADOPTED, '@exact/lists/list-row.contract#ListRow', '@exact/lists/switch-row.contract#SwitchRow', '@exact/cards/profile-header.contract#ProfileHeader',
  '@exact/actions/compose-frame.contract#ComposeFrame', '@exact/actions/bar-item.contract#BarItem', '@exact/actions/header-button.contract#HeaderButton'];
const S13C_SCREENS = [
  ...SCREENS.map(([n, t]) => [n, [...t]]),
  ['settings', [...SETTINGS]],
  ['settings-account', [...SETTINGS, 'tap settings-account']],
  ['settings-content', [...SETTINGS, 'tap settings-content']],
  ['settings-notifications', [...SETTINGS, 'tap settings-notifications']],
  // Signed in through the fixture form, so the bottom bar's tabs (BarItem) show.
  ['home-signed-in', [...SIGN_IN, 'clock +1500 real']],
];
const S13C_RENDERED = [...RENDERED, ['ProfileHeader', id => id.startsWith('profile-header-'), 'profile-header-'], ['ComposeFrame', id => id === 'compose-bar', '"compose-bar"']];
/** Stage 15 B batch 1's exports, which the packages placed inside PostCard and ProfileHeader already. */
const S15B_EXPORTS = ['@exact/cards/common.contract#Avatar', '@exact/cards/common.contract#VerifiedBadge'];

/** Each stage's gate (LLP 1139 §5): its baseline's tag on the copy, the
 * components its adoption adds (an earlier stage's the baseline may use
 * already), the screens shot, the package testIds that say a component
 * rendered, and the restyle cases. Stage 12's is the module's defaults;
 * `useStage` swaps a stage's in, so both stages run one set of lines. */
export const STAGES = {
  s12: { name: 'Stage 12', tag: 's12e-baseline', out: 'stage12-gate-run.txt', adopted: [...ADOPTED], before: [], screens: SCREENS.map(([n, t]) => [n, [...t]]), rendered: [...RENDERED], restyle: [...RESTYLE] },
  s13c: {
    name: 'Stage 13 C', tag: 's13c-baseline', out: 's13c-gate-run.txt',
    adopted: [...S13C_ADOPTED],
    before: [...ADOPTED],
    screens: S13C_SCREENS.map(([n, t]) => [n, [...t]]),
    // ListRow, SwitchRow, BarItem and HeaderButton write only the app's own
    // testIds; the build's source map says they're placed.
    rendered: [...S13C_RENDERED],
    restyle: [...RESTYLE],
    // Stage 13 C's tokens and params, a batch a component (s13c-restyle.mjs).
    exact: true,
    titles: {
      'consumer-pixels': 'The clone\'s settings rows, profile header, composer frame, bottom bar items and header buttons come from @exact/lists, @exact/cards and @exact/actions (ListRow, SwitchRow, ProfileHeader, ComposeFrame, BarItem, HeaderButton; stage 12\'s four kept) with no visual change (conform clean; zero differing pixels outside the allowlist, web, iOS, macOS, Linux, light and dark)',
    },
    // Release identity over the four pieces as they landed on main.
    release: { label: 'stage 13 C', runs: [['21a1a836b3', '21a1a836b3'], ['4841b08acf', '42c2b03a35'], ['0091eae8d0', 'a3c7cd25cd'], ['d9feffb3c0', 'ba581775cb']] },
  },
  s15b: {
    name: 'Stage 15 B batch 1', tag: 's15b-baseline', out: 's15b-gate-run.txt',
    // The copy's own Avatar and VerifiedBadge calls become the exports, which were shown before too (inside PostCard
    // and ProfileHeader), so `retired` proves the adoption: the copy's own copies, header styles and settings wrappers go.
    adopted: [...S13C_ADOPTED, ...S15B_EXPORTS],
    before: [...S13C_ADOPTED, ...S15B_EXPORTS],
    appPlaced: [...S15B_EXPORTS],
    retired: ['component Avatar', 'component VerifiedBadge', 'style ChatHeaderButton', 'style HeaderSlot',
      'component NavRow', 'component ActRow', 'component ValueRow', 'component SwitchRow', 'component CheckRow'],
    screens: [
      ...S13C_SCREENS.map(([n, t]) => [n, [...t]]),
      // Signed in: the chat list (its header buttons, its avatars) and the notifications (avatars and badges).
      ['chat', [...SIGN_IN, 'clock +1500 real', 'tap bottomBarMessagesBtn', 'clock +1500 real']],
      ['notifications', [...SIGN_IN, 'clock +1500 real', 'tap bottomBarNotificationsBtn', 'clock +1500 real']],
      // A thread anchored on Bo's reply (Bo is verified): the anchor's header is the copy's own (screens.contract),
      // so its badge is a VerifiedBadge the copy instantiates, which no other screen shows (theme2's s15b ruling (b)).
      ['thread-bo', ['tap post-1:at://did:plc:fixture1/app.bsky.feed.post/fx4']],
    ],
    rendered: [...S13C_RENDERED],
    restyle: [...RESTYLE],
    exact: true,
    // The package side as it landed on main (the copy's adoption is the copy's, not main's).
    release: { label: 'stage 15 B batch 1', runs: [['17c3169cef', '44e0294c88']] },
    titles: {
      'consumer-pixels': 'The clone\'s avatars and verification badges come from @exact/cards (Avatar, VerifiedBadge), its chat header buttons from @exact/actions HeaderButton and its settings rows from @exact/lists directly, the duplicates gone, with no visual change (conform clean; zero differing pixels outside the allowlist, web, iOS, macOS, Linux, light and dark)',
    },
  },
};

// Stage 15 B batch 2's: batch 1's and its own (scripts/stage12-s15b2.mjs).
STAGES.s15b2 = s15b2Stage(STAGES.s15b, SIGN_IN);

/** Run the gate as `stage`'s (`--stage`): its tag, adoption, screens, rendered testIds and restyle cases. */
export function useStage(stage) {
  const p = STAGES[stage];
  BASELINE_TAG = p.tag;
  EXACT = !!p.exact;
  RELEASE = p.release ?? null;
  TITLES = p.titles ?? {};
  for (const [list, items] of [[ADOPTED, p.adopted], [BEFORE_USES, p.before], [SCREENS, p.screens], [RENDERED, p.rendered], [RESTYLE, p.restyle], [RETIRED, p.retired ?? []], [APP_PLACED, p.appPlaced ?? []], [NAMED, p.named ?? []], [BINDINGS, p.bindings ?? []]]) list.splice(0, list.length, ...items);
  return p;
}

/** The gate's lines, in LLP 1139 §5's order. */
export function lines(opts) {
  return withTitles([
    ['consumer-pixels', 'The clone\'s post card, AtCard, action row and Post button come from the two packages with no visual change (conform clean; zero differing pixels outside the allowlist, web, iOS, macOS, Linux, light and dark)', () => {
      if (!opts.baseline || !existsSync(join(opts.baseline, 'baseline.json'))) return waits('the baseline (`baseline --copy … --out …`, taken before the change)');
      if (!opts.copy || !opts.before) return waits('the clone copy\'s adoption (--copy) and its before (--before, the baseline\'s tag)');
      return consumerPixels(opts);
    }],
    ['restyle', 'Setting space.md, cards.pad-start or a param moves every library component reading it; removing the line returns it (per host, measured on apps/library-gate; live or rebuild from the design server)', () => restyle(opts)],
    ['anatomy', 'C\'s compiler tests: every anatomy, slot, library-literal, library-platform and library-control refusal with its span', () => {
      // C's own suites (reload1's stage 12 C), run by name.
      // And a library package's params held in range in every scale column,
      // the terminal's included (tamber_base_expression's column ranges).
      const r = run('cargo', ['test', '-q', '-p', 'contract', '--test', 'it', '--', 'anatomy::', 'anatomy_slot::', 'library_rules::', 'library_packages::', 'tamber_base_expression::']);
      const counts = [...r.out.matchAll(/test result: (\w+)\. (\d+) passed; (\d+) failed/g)];
      const passed = counts.reduce((n, m) => n + Number(m[2]), 0), failed = counts.reduce((n, m) => n + Number(m[3]), 0);
      const said = `contract tests anatomy::, anatomy_slot::, library_rules::, library_packages::, tamber_base_expression::: ${passed} passed, ${failed} failed`;
      return r.ok && passed > 0 && failed === 0 ? pass(said) : fail(said, r.out.trim().split('\n').slice(-4).join(' / ').slice(0, 400));
    }],
    ['slot-rendered', 'Every variant cell of apps/library-gate renders its slot\'s marker, both glyphs, on every required host', () => slotRendered(opts)],
    ['slot-check-time', 'The slot check (check_slots) on the clone with both library packages takes under about 10 ms a check (the lead, 10:15Z; rt1 measures it)', () => slotCheckTime()],
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
  ]);
}

/** A stage's own headline for a line where stage 12's names its components. */
const withTitles = list => list.map(([name, title, fn]) => [name, TITLES[name] ?? title, fn]);

export function parse(argv) {
  const opts = { mode: 'gate', stage: 's12', only: [], hosts: [], screens: [], app: 'unsaid', out: null, copy: null, before: null, baseline: null, release: null, conformBase: null };
  const args = [...argv];
  if (args[0] === 'baseline') { opts.mode = 'baseline'; opts.out = null; args.shift(); }
  for (let i = 0; i < args.length; i += 2) {
    const [flag, v] = [args[i], args[i + 1]];
    if (!['--only', '--out', '--copy', '--before', '--baseline', '--host', '--screen', '--app', '--release', '--conform-base', '--stage'].includes(flag)) return `unknown argument ${flag}`;
    if (v === undefined || v.startsWith('--')) return `${flag} needs a value`;
    if (flag === '--only') opts.only.push(v); else if (flag === '--host') opts.hosts.push(v); else if (flag === '--screen') opts.screens.push(v); else if (flag === '--conform-base') opts.conformBase = v; else opts[flag.slice(2)] = v;
  }
  if (!STAGES[opts.stage]) return `--stage ${opts.stage}: one of ${Object.keys(STAGES).join(', ')}`;
  if (opts.mode === 'baseline' && (!opts.copy || !opts.out)) return 'baseline needs --copy and --out';
  // Only a full run writes the profile's gate record (~/x2theme/<out>): a run of some lines (`--only`) is no
  // record, and writing it there lost one (s13c-gate-run.txt, 2026-10-26). It goes to a scratch file named
  // for its stage, lines and process, unless `--out` names a path.
  if (opts.mode === 'gate' && !opts.out) {
    // Its name from the lines asked for, each once and checked below to be a line's id (letters and dashes).
    opts.out = opts.only.length
      ? join(tmpdir(), `stage12-gate-${opts.stage}-${[...new Set(opts.only)].join('+')}-${process.pid}.txt`)
      : join(homedir(), 'x2theme', STAGES[opts.stage].out);
  }
  const allowed = opts.mode === 'baseline' ? CLONE_HOSTS : SLOT_HOSTS;
  const unknownHost = opts.hosts.find(h => !allowed.includes(h));
  if (unknownHost) return `--host ${unknownHost}: one of ${allowed.join(', ')}`;
  // `--screen`: a rerun of some of the stage's screens (a re-shot pair, a screen added), never a full record.
  if (opts.screens.length && (opts.mode === 'baseline' || !opts.only.length)) return '--screen reshoots some screens of a gate run: name its lines with --only (never a baseline)';
  const names = STAGES[opts.stage].screens?.map(([n]) => n) ?? SCREENS.map(([n]) => n);
  const unknownScreen = opts.screens.find(n => !names.includes(n));
  if (unknownScreen !== undefined) return `--screen ${unknownScreen}: one of ${names.join(', ')}`;
  const ids = lines(opts).map(([id]) => id);
  // Every `--only` a line's id, the empty string too: a scratch name is built from them.
  const missing = opts.only.find(o => !ids.includes(o));
  if (missing !== undefined) return `--only ${missing}: one of ${ids.join(', ')}`;
  return opts;
}

if (import.meta.main) {
  const opts = parse(process.argv.slice(2));
  if (typeof opts === 'string') { console.error(`${opts}\nusage: bun scripts/stage12-gate.mjs [--stage s12|s13c|s15b|s15b2] [--only <line> …] [--screen <name> …] [--out <file>] [--copy <dir>] [--baseline <dir>] [--release <json>] | baseline [--stage s12|s13c|s15b|s15b2] --copy <dir> --out <dir> [--host <h> …]`); process.exit(2); }
  const stage = useStage(opts.stage);
  if (opts.screens.length) SCREENS.splice(0, SCREENS.length, ...SCREENS.filter(([n]) => opts.screens.includes(n)));
  RERUN_SCREENS.push(...opts.screens);
  if (opts.mode === 'baseline') {
    const r = baseline(resolve(opts.copy), resolve(opts.out), opts.hosts.length ? opts.hosts : CLONE_HOSTS);
    const bad = r.shots.filter(s => !s.ok);
    console.log(`baseline: ${r.shots.length - bad.length} of ${r.shots.length} screens; plan ${r.plan.sha ?? `FAILED (${r.plan.said})`}; ${join(opts.out, 'baseline.json')}`);
    for (const s of bad) console.log(`  FAILED ${s.host} ${s.scheme} ${s.screen}: ${s.said}`);
    process.exit(bad.length || !r.plan.sha ? 1 : 0);
  }
  const head = run('git', ['rev-parse', '--short', 'HEAD']).out.trim();
  const text = [`# ${stage.name} gate run (LLP 1139 §5), ${new Date().toISOString()}, ${head}; ${opts.only.length ? `lines ${opts.only.join(', ')}` : 'every line'}${opts.screens.length ? `, screens ${opts.screens.join(', ')} only` : ''}`, ''];
  for (const [id, title, check] of lines(opts)) {
    let r;
    try { r = !opts.only.length || opts.only.includes(id) ? await check() : notRun('not selected'); } catch (e) { r = fail(`the line stopped: ${String(e?.stack ?? e).split('\n').slice(0, 3).join(' / ').slice(0, 400)}`); }
    text.push(`${r.status.padEnd(r.status.startsWith('WAITS') ? 0 : 5)}  ${id}: ${title}`, ...r.evidence.map(e => `    ${e}`), '');
    console.log(`${r.status}  ${id}`);
  }
  writeFileSync(opts.out, `${text.join('\n')}\n`);
  console.log(`written to ${opts.out}${opts.only.length ? ' (a run of some lines: no gate record)' : ''}`);
}
