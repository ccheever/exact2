// The stage 12 gate (LLP 1139 §5, "The gate"; ~/x2theme/iosfix-s12e-plan.md,
// theme2's 06:46 approval): each line written PASS, FAIL, WAITS-ON (naming
// what it waits on) or NOT-RUN (a line the selection left out) to one file,
// with its evidence. A line whose parts aren't built yet says WAITS-ON; none
// is passed over.
//
//   bun scripts/stage12-gate.mjs [--only <line> …] [--out <file>] [--copy <clone copy>] [--baseline <dir>]
//   bun scripts/stage12-gate.mjs baseline --copy <clone copy> --out <dir> [--host <h> …]
//
// `baseline` takes the consumer's "before" (LLP 1139 §5: baselines taken
// before the change, with their provenance): the clone copy's fixture build
// (signed out, synthetic data, no network: its data/fixture.ts) on each of
// the clone's own targets among the required hosts, light and dark, every
// screen in SCREENS, plus the copy's plan bytes. The copy is a git clone of
// the clone, never the clone itself; its exact.mjs names this checkout or
// another by EXACT2. EXACT_SIM names the simulator.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir, release } from 'node:os';
import { join, resolve } from 'node:path';
import { decodePng } from './png.mjs';

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
  return ['clock +1000 real', 'clock settle', `prefer prefers-color-scheme ${scheme}`, ...settle, ...taps.flatMap(t => [t, ...settle]), `screenshot ${png}`, 'logs'];
}

/** One host's drive of the copy: every screen, both schemes, each its own
 * launch on a reset store (so no screen follows another's state). */
// The simulator's status bar, pinned for an iOS drive (its clock and
// signal are the machine's otherwise, in every shot).
export const STATUS_BAR = ['--time', '9:41', '--dataNetwork', 'wifi', '--wifiMode', 'active', '--wifiBars', '3', '--cellularMode', 'notSupported', '--batteryState', 'charged', '--batteryLevel', '100'];

function drive(copy, host, dir) {
  const rows = [];
  const sim = host === 'ios' ? process.env.EXACT_SIM : null;
  if (sim) run('xcrun', ['simctl', 'status_bar', sim, 'override', ...STATUS_BAR]);
  try {
    rows.push(...driveScreens(copy, host, dir));
  } finally {
    if (sim) run('xcrun', ['simctl', 'status_bar', sim, 'clear']);
  }
  return rows;
}

function driveScreens(copy, host, dir) {
  const rows = [];
  for (const scheme of SCHEMES) {
    for (const [screen] of SCREENS) {
      const png = join(dir, `${host}-${scheme}-${screen}.png`);
      const args = [join(copy, 'exact.mjs'), 'agent', host, '--storage', 'stage12-gate', '--reset-storage', '--epoch', EPOCH,
        ...(host === 'ios' || host === 'macos' ? [] : ['--size', SIZE]), ...(host === 'ios' && process.env.EXACT_SIM ? ['--sim', process.env.EXACT_SIM] : []),
        ...screenOps(screen, scheme, png)];
      const r = run(process.execPath, args, { cwd: copy });
      const refused = [...new Set(r.out.match(new RegExp(`${REFUSED} [^"\\s]+`, 'g')) ?? [])];
      const said = refused.length ? `the fixture refused: ${refused.join(', ')}` : r.ok ? '' : r.out.trim().split('\n').slice(-2).join(' / ').slice(0, 300);
      rows.push({ host, scheme, screen, png, ok: r.ok && existsSync(png) && !refused.length, sha: existsSync(png) ? sha(readFileSync(png)) : null, said });
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
  return {
    exact2: git(ROOT, 'rev-parse', 'HEAD'), copy: git(copy, 'rev-parse', 'HEAD'), copyDirty: git(copy, 'status', '--porcelain') !== '',
    os: `darwin ${release()}`, macos: run('sw_vers', ['-productVersion']).out.trim(), xcode, simulator: device, statusBar: STATUS_BAR.join(' '), epoch: EPOCH, size: SIZE, schemes: SCHEMES,
    taken: new Date().toISOString(),
  };
}

/** The copy's plan bytes (its app.contract, compiled by its exact2). */
function planBytes(copy, dir) {
  const plan = join(dir, 'app.plan');
  const r = run(process.execPath, [join(copy, 'exact.mjs'), 'contract', 'build', 'app.contract', '-o', plan], { cwd: copy });
  return r.ok && existsSync(plan) ? { plan, sha: sha(readFileSync(plan)), bytes: readFileSync(plan).length } : { plan: null, said: r.out.trim().split('\n').slice(-3).join(' / ') };
}

export function baseline(copy, out, hosts = CLONE_HOSTS) {
  mkdirSync(out, { recursive: true });
  const rows = hosts.flatMap(h => drive(copy, h, out));
  const record = { provenance: provenance(copy), plan: planBytes(copy, out), shots: rows };
  writeFileSync(join(out, 'baseline.json'), `${JSON.stringify(record, null, 1)}\n`);
  return record;
}

/** Pixels differing between two screenshots outside `allow` (rectangles
 * [x, y, w, h], each with a reason): {differ, total, size}. A size change is
 * every pixel. */
export function pixelDiff(a, b, allow = []) {
  const A = decodePng(a), B = decodePng(b);
  if (A.width !== B.width || A.height !== B.height) return { differ: A.width * A.height, total: A.width * A.height, size: [A.width, A.height, B.width, B.height] };
  const inside = (x, y) => allow.some(([rx, ry, rw, rh]) => x >= rx && x < rx + rw && y >= ry && y < ry + rh);
  let differ = 0;
  for (let y = 0; y < A.height; y++) {
    for (let x = 0; x < A.width; x++) {
      const i = (y * A.width + x) * 4;
      if ((A.data[i] !== B.data[i] || A.data[i + 1] !== B.data[i + 1] || A.data[i + 2] !== B.data[i + 2] || A.data[i + 3] !== B.data[i + 3]) && !inside(x, y)) differ++;
    }
  }
  return { differ, total: A.width * A.height };
}

/** The gate's lines, in LLP 1139 §5's order. */
export function lines(opts) {
  return [
    ['consumer-pixels', 'The clone\'s post card, AtCard, action row and Post button come from the two packages with no visual change (conform clean; zero differing pixels outside the allowlist, web, iOS, macOS, Linux, light and dark)', () => {
      if (!opts.baseline || !existsSync(join(opts.baseline, 'baseline.json'))) return waits('the baseline (`baseline --copy … --out …`, taken before the change)');
      return waits('the clone copy\'s adoption of @exact/cards and @exact/actions (after the packages: B1, then C)');
    }],
    ['restyle', 'Setting space.md, cards.pad or a param moves every library component reading it; removing the line returns it (per host, measured on apps/library-gate)', () =>
      waits('the packages (stage 12 B1\'s package params) and apps/library-gate')],
    ['anatomy', 'C\'s compiler tests: every anatomy, slot, library-literal, library-platform and library-control refusal with its span', () =>
      waits('stage 12 C (anatomy, reload1)')],
    ['slot-rendered', 'Every variant cell of apps/library-gate renders its slot\'s marker, both glyphs, on every required host', () =>
      waits('stage 12 C (parts and the slot) and apps/library-gate')],
    ['slot-check-time', 'The slot check (check_slots) on the clone with both library packages takes under about 10 ms a check (the lead, 10:15Z; rt1 measures it)', () =>
      waits('stage 12 C (check_slots), the packages and the adoption, then rt1\'s measurement')],
    ['no-forks', 'lower-library-platform clean over both packages', () =>
      waits('stage 12 C (lower-library-platform) and the packages')],
    ['no-regressions', 'Release output unchanged for apps using no library component; conform clean; @exact/reading\'s users unchanged', () =>
      waits('the packages and the adoption (the comparison runs over stage 12 E\'s commits)')],
  ];
}

export function parse(argv) {
  const opts = { mode: 'gate', only: [], hosts: [], out: join(homedir(), 'x2theme/stage12-gate-run.txt'), copy: null, baseline: null };
  const args = [...argv];
  if (args[0] === 'baseline') { opts.mode = 'baseline'; opts.out = null; args.shift(); }
  for (let i = 0; i < args.length; i += 2) {
    const [flag, v] = [args[i], args[i + 1]];
    if (!['--only', '--out', '--copy', '--baseline', '--host'].includes(flag)) return `unknown argument ${flag}`;
    if (v === undefined || v.startsWith('--')) return `${flag} needs a value`;
    if (flag === '--only') opts.only.push(v); else if (flag === '--host') opts.hosts.push(v); else opts[flag.slice(2)] = v;
  }
  if (opts.mode === 'baseline' && (!opts.copy || !opts.out)) return 'baseline needs --copy and --out';
  const unknownHost = opts.hosts.find(h => !CLONE_HOSTS.includes(h));
  if (unknownHost) return `--host ${unknownHost}: one of ${CLONE_HOSTS.join(', ')}`;
  const ids = lines(opts).map(([id]) => id);
  const missing = opts.only.find(o => !ids.includes(o));
  if (missing) return `--only ${missing}: one of ${ids.join(', ')}`;
  return opts;
}

if (import.meta.main) {
  const opts = parse(process.argv.slice(2));
  if (typeof opts === 'string') { console.error(`${opts}\nusage: bun scripts/stage12-gate.mjs [--only <line> …] [--out <file>] [--copy <dir>] [--baseline <dir>] | baseline --copy <dir> --out <dir> [--host <h> …]`); process.exit(2); }
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
    const r = !opts.only.length || opts.only.includes(id) ? await check() : notRun('not selected');
    text.push(`${r.status.padEnd(r.status.startsWith('WAITS') ? 0 : 5)}  ${id}: ${title}`, ...r.evidence.map(e => `    ${e}`), '');
    console.log(`${r.status}  ${id}`);
  }
  writeFileSync(opts.out, `${text.join('\n')}\n`);
  console.log(`written to ${opts.out}`);
}
