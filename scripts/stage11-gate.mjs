// The stage 11 gate (LLP 1121 §5 stage 11, ~/x2theme/stage11-brief.md "The
// gate"): each line run from what already checks it, its evidence kept, and
// written PASS, FAIL, WAITS-ON (with what it waits on) or NOT-RUN (a line
// the selection left out) to one file. It checks nothing itself but the scale
// table's sources; the hosts' lengths are scripts/scale-check.mjs's, the
// package designs' refusals the contract tests', the server's rows its own
// replies.
//
//   bun scripts/stage11-gate.mjs [--host <h> …] [--app <android app>] [--only <line> …] [--out <file>] [--clone <copy>] [--release <json>]
//
// Hosts default to all six: web, linux, macos, ios, the Android reader (--app,
// an app with no data: unsaid; the base values only) and the terminal. The
// fixtures: packages/scale-fixture-cards, packages/scale-fixture-notes and
// apps/scale-gate (its design
// sets space.md = 20px; base/ has none). Each host's build must be fresh (a
// stale one's drive says how to build it: host/web/build.mjs caltrain-web,
// host/apple/build.mjs caltrain-apple [--ios --sim], cargo build --profile
// host-dev -p caltrain-linux, agent-android.mjs build unsaid --apk); EXACT_SIM names the simulator, and the Android reader takes
// the emulator its owner file gives.
import { spawn, spawnSync } from 'node:child_process';
import { appendFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { homedir, tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { authoredTests } from './app.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const GATE = join(ROOT, 'apps/scale-gate');
// Copies the drives take: inside the checkout, so `@exact/*` resolves through
// its node_modules, and never a fixture itself, which a set-token writes.
const WORK = join(ROOT, 'target/stage11-gate');
export const HOSTS = ['web', 'linux', 'macos', 'ios', 'android', 'terminal'];
// The Android reader runs an app's APK, which reads no package design.
const PACKAGE_HOSTS = HOSTS.filter(h => h !== 'android');
// The fixture packages (stage 12 E's rename, theme2 06:46: a library may
// want the short names): their names, their designs' and their tokens'.
const [CARDS, NOTES] = ['scale-fixture-cards', 'scale-fixture-notes'];
const FIXTURES = [CARDS, NOTES];

/** A line's result: its status, and the evidence it rests on, a line each. */
const pass = (...evidence) => ({ status: 'PASS', evidence });
const fail = (...evidence) => ({ status: 'FAIL', evidence });
const waits = (on, ...evidence) => ({ status: `WAITS-ON ${on}`, evidence });
const notRun = (...evidence) => ({ status: 'NOT-RUN', evidence });

/** `cmd args` run in the checkout: its status and its output. */
function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { cwd: ROOT, encoding: 'utf8', timeout: 1_800_000, maxBuffer: 1 << 26, ...opts });
  return { ok: r.status === 0 && !r.error, out: `${r.stdout ?? ''}${r.stderr ?? ''}`, error: r.error };
}

/** A cargo test run that ran something: every `test result` ok, some passed. */
export function ranOk(out) {
  const results = [...out.matchAll(/test result: (\w+)\. (\d+) passed; (\d+) failed/g)];
  return results.length > 0 && results.every(m => m[1] === 'ok' && m[3] === '0') && results.some(m => Number(m[2]) > 0);
}

/** scale-check's rows for `plan` on `host`: `ok` when every box held. */
function scaleCheck(plan, expect, host, app) {
  const args = [join(ROOT, 'scripts/scale-check.mjs'), plan, expect, '--host', host, '--out', join(WORK, `shots-${host}`)];
  if (host === 'android') args.push('--app', app);
  const r = run(process.execPath, args);
  // A failed drive's row carries the first line after it that says why.
  const out = r.out.split('\n');
  const rows = out.flatMap((l, i) => {
    if (!/^(ok|FAIL) /.test(l)) return [];
    const why = /the drive failed$/.test(l) && out.slice(i + 1, i + 20).find(w => /error|unavailable|panicked|not found|timed out/i.test(w));
    return [why ? `${l}: ${why.trim().slice(0, 240)}` : l];
  });
  return { ok: r.ok && rows.length > 0 && rows.every(l => l.startsWith('ok')), rows: rows.length ? rows : [r.out.trim().split('\n').slice(-3).join(' / ')] };
}

/** A copy of `from`'s app.contract (and its design, edited by `design`) as a
 * plan with no app id (a host's plan push refuses another app's). */
function planCopy(name, from, design) {
  const dir = join(WORK, name);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  cpSync(join(from, 'app.contract'), join(dir, 'app.contract'));
  const tamber = join(from, 'app.tamber');
  if (design && existsSync(tamber)) writeFileSync(join(dir, 'app.tamber'), design(readFileSync(tamber, 'utf8')));
  return join(dir, 'app.contract');
}

/** `hosts` run through scale-check on `plan`: a PASS or FAIL with each row,
 * or NOT-RUN when the selection leaves none of them. */
function onHosts(hosts, plan, expect, opts, say) {
  if (!hosts.length) return notRun(`no host of this line's in the selection (--host ${opts.hosts.join(' ')})`);
  const rows = hosts.map(h => [h, scaleCheck(plan, expect, h, opts.app)]);
  const evidence = rows.flatMap(([h, r]) => [`${h}: ${r.ok ? say : 'FAILED'}`, ...r.rows.map(l => `  ${l}`)]);
  return rows.every(([, r]) => r.ok) ? pass(...evidence) : fail(...evidence);
}

/** A free loopback port. */
async function freePort() {
  const server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const { port } = server.address();
  await new Promise(ok => server.close(ok));
  return port;
}

/** The children this run started, stopped if it is interrupted. */
const children = new Set();
const kill = (child, signal) => { try { process.kill(-child.pid, signal); } catch { try { child.kill(signal); } catch { /* gone */ } } };
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => { for (const child of children) kill(child, 'SIGKILL'); process.exit(130); });
}

/** `spawn`, the child kept to be stopped. */
function own(cmd, args, opts) {
  const child = spawn(cmd, args, opts);
  children.add(child);
  child.once('exit', () => children.delete(child));
  return child;
}

/** A child's exit awaited, after SIGTERM (to its group when it leads one),
 * then SIGKILL if it outlives ten seconds. */
async function stop(child) {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  const gone = new Promise(ok => child.once('exit', ok));
  const late = ms => new Promise(ok => setTimeout(() => ok('late'), ms));
  kill(child, 'SIGTERM');
  if ((await Promise.race([gone, late(10_000)])) === 'late') {
    kill(child, 'SIGKILL');
    await Promise.race([gone, late(5_000)]);
  }
}

/** A design server (or dev loop) child: the address it says, its token, and
 * `ask(request)` with a deadline; on any failure here the child is stopped. */
async function owned(child, address, said) {
  try {
    const url = await new Promise((ok, no) => {
      const hear = chunk => { said.text += chunk; said.log?.(chunk); const m = address.exec(said.text); if (m) ok(m[1]); };
      child.stdout.on('data', hear);
      child.stderr.on('data', hear);
      child.on('exit', code => no(new Error(`it exited (${code}): ${said.text.slice(-400)}`)));
      setTimeout(() => no(new Error(`no address in 15 min: ${said.text.slice(-400)}`)), 900_000);
    });
    const { token } = await (await fetch(`${url}/__tamber/token`, { signal: AbortSignal.timeout(30_000) })).json();
    const ask = async request => (await fetch(`${url}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request), signal: AbortSignal.timeout(120_000) })).json();
    return { url, ask };
  } catch (e) {
    await stop(child);
    throw e;
  }
}

/** The design server on a copy of the gate app. */
async function serve() {
  const dir = join(WORK, `served-${process.pid}`);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(join(GATE, f), join(dir, f));
  const child = own(process.execPath, [join(ROOT, 'scripts/exact.mjs'), 'tamber', 'serve', dir], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'], detached: true });
  const { ask } = await owned(child, /(http:\/\/127\.0\.0\.1:\d+)/, { text: '' });
  return { dir, ask, stop: async () => { await stop(child); rmSync(dir, { recursive: true, force: true }); } };
}

/** The two package designs' bytes, and whether git sees them as committed. */
function packageFiles() {
  const bytes = FIXTURES.map(p => readFileSync(join(ROOT, `packages/${p}/${p}.tamber`), 'utf8'));
  const clean = run('git', ['diff', '--quiet', '--', ...FIXTURES.map(p => `packages/${p}/${p}.tamber`)]).ok;
  return { bytes, clean };
}

/** Whether A2 part 2 is in: describe carries a package token's package. */
function a2In(described) {
  return JSON.stringify(described ?? {}).includes(`"package":"@exact/${CARDS}"`);
}

/** The object describe gives a package token's row in, wherever it sits. */
function packageRow(node, name) {
  if (!node || typeof node !== 'object') return null;
  if ((node.name === name || node.token === name) && 'package' in node) return node;
  for (const v of Object.values(node)) {
    const found = packageRow(v, name);
    if (found) return found;
  }
  return null;
}

/** The packages' paddings (`[cards, notes]`, px) in an agent's `layout` reply. */
function paddings(reply) {
  const of = id => reply.nodes.find(n => n.testId === id);
  return [['cards-card', 'cards-inner'], ['notes-note', 'notes-inner']].map(([box, child]) => of(child).y - of(box).y);
}

/** The live line: the web dev loop on a copy of the gate app, its page driven
 * by the agent; after the first layout the dev loop's design server sets
 * space.md to 28px (a value edit: the page takes it with no reload), after
 * the second removes the app's line (the base again; the token list moves,
 * which the dev loop may reload for). Each edit is answered before the layout
 * after it is read, or the run is said to have raced. */
async function live() {
  // The dev loop's design reader built first: a stale one sends a value edit
  // through a build, and the loop is stopped before its own rebuild of it
  // lands, so every run would.
  const reader = run('cargo', ['build', '-q', '--profile', 'host-dev', '-p', 'exact-web-js']);
  if (!reader.ok) return fail('the dev loop\'s design reader (exact-web-js, host-dev) did not build', reader.out.split('\n').slice(-5).join(' / '));
  const dir = join(WORK, `live-${process.pid}`);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(join(GATE, f), join(dir, f));
  // Its log and its build out of the checkout: a write beside the app dir can
  // reach the dev loop's watcher as an edit, and rebuild it in a loop.
  const log = join(tmpdir(), `stage11-gate-live-${process.pid}.log`);
  const dist = join(tmpdir(), `stage11-gate-live-dist-${process.pid}`);
  rmSync(log, { force: true });
  const port = await freePort();
  const dev = own(process.execPath, [join(ROOT, 'host/web/dev.mjs'), '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_APP_DIR: dir, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'], detached: true });
  const said = { text: '', log: c => appendFileSync(log, c) };
  let drive;
  try {
    const { url: base, ask } = await owned(dev, new RegExp(`(http://127\\.0\\.0\\.1:${port})/`), said);
    // Its first builds settled (5 s with nothing said, at most a minute), so
    // the page the drive opens is not reloaded under it.
    const settled = async () => {
      let quiet = 0;
      for (let last = said.text.length, t = 0; quiet < 5 && t < 60; t++) {
        await new Promise(ok => setTimeout(ok, 1000));
        quiet = said.text.length === last ? quiet + 1 : 0;
        last = said.text.length;
      }
      return quiet >= 5;
    };
    if (!(await settled())) return fail('the dev loop did not settle in a minute: its builds kept coming');
    const edit = async op => ask({ method: 'transaction', client: 'gate', files: (await ask({ method: 'hello' })).ok?.files, ops: [op] });
    const settle = ['clock +3000 real', 'clock data', 'clock +3000 real', 'clock data'];
    drive = own(process.execPath, [join(ROOT, 'scripts/agent.mjs'), 'web', '--url', base, '--storage', `stage11-gate-${process.pid}`, '--reset-storage', '--json', 'layout', ...settle, 'layout', ...settle, ...settle, 'layout'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    const replies = [], edits = [];
    const ops = [{ op: 'set-token', name: 'space.md', value: '28px' }, { op: 'remove-token', name: 'space.md' }];
    let heard = '', buf = '', queue = Promise.resolve();
    const arrivals = [];
    drive.stderr.on('data', c => { heard = (heard + c).slice(-2000); });
    // One reader of the drive's lines; an edit after a layout runs to its
    // answer while later lines wait their turn.
    const onLayout = async line => {
      replies.push(JSON.parse(line));
      if (replies.length > ops.length) return;
      const from = said.text.length;
      edits.push({ reply: await edit(ops[replies.length - 1]), from, done: Date.now() });
    };
    const read = new Promise((ok, no) => {
      drive.stdout.on('data', c => {
        buf += c;
        let at;
        while ((at = buf.indexOf('\n')) >= 0) {
          const line = buf.slice(0, at); buf = buf.slice(at + 1);
          if (!line.startsWith('{') || !line.includes('"nodes"')) continue;
          // Each layout's arrival, which is when the agent took it; one taken
          // before the edit before it was answered raced it.
          const when = Date.now();
          arrivals.push(when);
          queue = queue.then(() => onLayout(line));
        }
      });
      drive.on('exit', code => (code === 0 ? ok() : no(new Error(`the drive exited ${code}: ${(heard || buf).trim().split('\n').filter(l => !/^\s+at /.test(l)).slice(-4).join(' / ')}`))));
      setTimeout(() => no(new Error('the drive took over 5 min')), 300_000);
    });
    await read;
    await queue;
    const raced = edits.some((e, k) => !(arrivals[k + 1] > e.done));
    const [first, moved, back] = replies.map(paddings);
    // What the dev loop said between the set-token and the next edit.
    const setTokenSaid = said.text.slice(edits[0]?.from ?? 0, edits[1]?.from ?? said.text.length);
    // The set-token's: a batch read from the compiler with no build, no
    // page reloaded or newly connected (the same page took it).
    const unbuilt = /batch ready [^\n]*(no build\)[^\n]*no reload|no plan)/.test(setTokenSaid);
    const reloaded = /reloaded seq=|[1-9]\d* pages? reloading|page connected|\(built in |\(rebuilt in |plan ready|pushed to/.test(setTokenSaid);
    const evidence = [`paddings (cards, notes): ${JSON.stringify(first)} at 20px; ${JSON.stringify(moved)} after set-token space.md 28px; ${JSON.stringify(back)} after remove-token (the web's base)`,
      `edits: ${edits.map(e => (e.reply.ok ? 'ok' : JSON.stringify(e.reply.error))).join(', ')}`,
      `the dev loop after the set-token: ${setTokenSaid.split('\n').filter(l => /edit →|page connected|reloaded/.test(l)).join(' / ') || '(nothing)'}`];
    if (raced) return fail(...evidence, 'a layout was read before the edit before it was answered');
    const held = first.every(p => p === 20) && moved.every(p => p === 28) && back.every(p => p === 16) && edits.every(e => e.reply.ok);
    if (!unbuilt || reloaded) return fail(...evidence, 'the set-token was not a batch with no build and no reload on the same page');
    return held ? pass(...evidence) : fail(...evidence);
  } finally {
    await stop(drive);
    await stop(dev);
    // The dev loop's compiler runs in a group of its own: any process still
    // naming this run's app copy or build is this run's, and is ended.
    for (const pid of run('pgrep', ['-f', dir]).out.split('\n').concat(run('pgrep', ['-f', dist]).out.split('\n')).map(Number).filter(n => n > 0 && n !== process.pid)) {
      try { process.kill(pid, 'SIGKILL'); } catch { /* gone */ }
    }
    rmSync(dir, { recursive: true, force: true });
    rmSync(dist, { recursive: true, force: true });
    rmSync(log, { force: true });
  }
}

/** Every scale cell names its source, as the kernel's build holds them (the
 * brief's "no brand's 4 px grid"): `=metric` a metrics row there is, `{px,
 * measured|guideline: id}` whose docs/scale-measured.md entry binds exactly
 * `name = px`, `{stretch: "=metric", why}` with its entry under Role
 * stretches, `full` for radius.full, and the terminal's `{cells: n}` (n a
 * number) or a radius's 0. A bare number is no source. */
export function citations(schema, measured) {
  const metrics = new Set((schema.metrics ?? []).map(r => r[0]));
  const entries = measured.split('\n### ');
  const bad = [];
  for (const [name, ...cells] of schema.scale) {
    cells.forEach((cell, i) => {
      const terminal = i === cells.length - 1;
      const where = `${name} column ${i + 1}`;
      if (typeof cell === 'string' && cell.startsWith('=')) {
        if (!metrics.has(cell.slice(1))) bad.push(`${where}: ${cell} names no metrics row`);
      } else if (!terminal && cell && typeof cell === 'object' && 'stretch' in cell) {
        // A stretch (stage 13 D5): a metric, why in a line, and its entry under Role stretches.
        const ok = typeof cell.stretch === 'string' && cell.stretch.startsWith('=') && metrics.has(cell.stretch.slice(1));
        const stretches = (measured.replaceAll('\r\n', '\n').split('\n## ').find(s => s.startsWith('Role stretches\n')) ?? '').split('\n### ');
        if (!ok || Object.keys(cell).length !== 2 || typeof cell.why !== 'string' || !cell.why.trim() || cell.why.includes('\n')) bad.push(`${where}: ${JSON.stringify(cell)} is no stretch of a metrics row with its why in a line`);
        else if (!stretches.some(e => e.startsWith(`\`${cell.stretch}\``))) bad.push(`${where}: ${cell.stretch} has no entry under Role stretches`);
      } else if (cell === 'full') {
        if (name !== 'radius.full') bad.push(`${where}: full outside radius.full`);
      } else if (terminal && cell && typeof cell === 'object' && 'cells' in cell) {
        if (!Number.isFinite(cell.cells) || Object.keys(cell).length !== 1) bad.push(`${where}: ${JSON.stringify(cell)} is no number of cells`);
      } else if (terminal && cell === 0 && name.startsWith('radius.')) {
        // A corner a terminal never draws.
      } else if (cell && typeof cell === 'object' && Number.isFinite(cell.px) && Object.keys(cell).length === 2 && (cell.measured || cell.guideline)) {
        const id = cell.measured ?? cell.guideline;
        const entry = entries.find(e => e.startsWith(`\`${id}\``));
        const binds = entry?.split('\n').find(l => l.startsWith('Binds: ')) ?? '';
        if (!entry) bad.push(`${where}: ${id} has no entry in docs/scale-measured.md`);
        else if (!binds.includes(`\`${name} = ${cell.px}\``)) bad.push(`${where}: ${id}'s entry binds no \`${name} = ${cell.px}\``);
      } else bad.push(`${where}: ${JSON.stringify(cell)} names no source`);
    });
  }
  return bad;
}

/** `--release`'s comparison (each run of stage 11 commits, each source):
 * PASS when every plan is identical or moved only as named. */
export function releaseIdentity(file) {
  if (!file || !existsSync(file)) return { status: 'WAITS', on: 'the release identity run (--release <its JSON>)', evidence: [] };
  const rows = JSON.parse(readFileSync(file, 'utf8'));
  const digest = r => r.bytes?.sizes?.[0] === r.bytes?.sizes?.[1] && r.bytes?.range && r.bytes.range[0] >= 17 && r.bytes.range[1] <= 24 && r.commits.some(c => /stage 11 B1/.test(c));
  const own = r => r.source === 'apps/design-tool/app.contract' && r.commits.some(c => /[Dd]esign tool|tool half/.test(c));
  const count = st => rows.filter(r => r.status === st).length;
  const moved = rows.filter(r => r.status === 'DIFFERS');
  const unnamed = moved.filter(r => !digest(r) && !own(r));
  const failed = rows.filter(r => r.status === 'build failed');
  const runs = new Set(rows.map(r => r.run)).size;
  const evidence = [`release identity: ${rows.length} comparisons over ${runs} runs of stage 11 commits; ${count('identical')} identical, ${moved.filter(digest).length} B1's schema digest only (header bytes 17-24), ${moved.filter(own).length} the design tool's own source, ${count('both failed')} failing alike (apps/expose's ./local)`,
    ...unnamed.map(r => `  MOVED ${r.source} over ${r.run} (${r.commits[0]})`), ...failed.map(r => `  BUILD FAILED ${r.source} over ${r.run}`)];
  return { status: unnamed.length || failed.length ? 'FAIL' : 'PASS', evidence };
}

/** The gate's lines, in the brief's order. */
export function lines(opts) {
  const pick = h => (opts.hosts.length ? opts.hosts.filter(x => h.includes(x)) : h);
  const scale = join(ROOT, 'host/web-js/conformance/scale');
  const cargoTest = (args, name) => {
    const r = run('cargo', ['test', '-q', ...args]);
    const ok = r.ok && ranOk(r.out);
    return { ok, said: `${name}: ${ok ? 'pass' : 'FAIL'}`, out: `${r.error ? `${r.error.message}\n` : ''}${r.out}` };
  };
  return [
    ['base-values', 'With no app value set, space.* and radius.* resolve per host to the scale table\'s values (Q3)', () => {
      // C2's conformance plan with its design's own space.md taken out, so
      // every base name is each host's own.
      const plan = planCopy('base-values', scale, t => t.replace(/^\s*space\.md\s*=.*\n/m, ''));
      return onHosts(pick(HOSTS), plan, join(scale, 'expect.json'), opts, 'all held at its column');
    }],
    ['citations', 'Every row cites the metric or guideline it comes from; no row is a brand\'s 4 px grid', () => {
      const schema = JSON.parse(readFileSync(join(ROOT, 'kernel/tables/schema.json'), 'utf8'));
      const bad = citations(schema, readFileSync(join(ROOT, 'docs/scale-measured.md'), 'utf8'));
      const kernel = cargoTest(['-p', 'exact-kernel', '--lib', 'style::scale'], 'kernel style::scale tests');
      const evidence = [`${schema.scale.length} rows, ${schema.scale.reduce((n, r) => n + r.length - 1, 0)} cells read`, ...bad, kernel.said];
      return bad.length === 0 && kernel.ok ? pass(...evidence) : fail(...evidence, kernel.out.split('\n').slice(-5).join(' / '));
    }],
    ['packages-restyle', 'Two packages restyle together from one app value (space.md = 20px), web, iOS, the Mac, Linux and the terminal', () => {
      // The expectation is the number 20, not the name: a missed override
      // would read the base on both sides.
      const plan = planCopy('restyle', GATE, t => t);
      return onHosts(pick(PACKAGE_HOSTS), plan, join(GATE, 'expect.json'), opts, 'both packages at 20 px (whole cells in the terminal)');
    }],
    ['packages-base', 'With no app value, both packages read each host\'s base values', () => {
      const plan = planCopy('bare', join(GATE, 'base'));
      return onHosts(pick(PACKAGE_HOSTS), plan, join(GATE, 'base/expect.json'), opts, 'the base');
    }],
    ...[
      ['package-full-name', 'A package\'s token is set by the app by its full name', 'a_package_design_styles_its_views_and_the_app_sets_its_tokens_by_full_name'],
      // Two packages cannot define one name: a package's names are under its
      // own name, and two packages taking one name are refused naming both;
      // the app's line for a package's name is the token, whole.
      ['package-one-name', 'Two packages defining one name is refused unless the app sets it', 'one_name_from_two_files_is_refused_unless_the_app_sets_it'],
      ['package-no-dials', 'A package declaring a dial or an axis is refused', 'axes_dials_params_components_icons_and_exports_are_not_a_packages'],
      ['package-when-axis', 'A package token whose `when` names an axis the app doesn\'t declare is refused', 'a_package_tokens_when_names_only_the_apps_axes'],
      ['package-no-base', 'A package may not define a base name (an app may, for every package: ruling 4)', 'a_package_may_not_name_a_base_name_nor_reach_outside_itself'],
    ].map(([id, title, test]) => [id, title, () => {
      const r = cargoTest(['-p', 'contract', '--test', 'it', `package_designs::${test}`, '--', '--exact'], `contract tests package_designs::${test}`);
      return r.ok ? pass(r.said) : fail(r.said, r.out.split('\n').slice(-6).join(' / '));
    }]),
    ['server-rows', 'The server shows base rows (their columns, the app\'s override), package tokens with their package, resolve and explain\'s source', async () => {
      const s = await serve();
      try {
        const evidence = [], bad = [];
        const described = await s.ask({ method: 'describe' });
        const base = described.ok?.base ?? [];
        const md = base.find(r => r.name === 'space.md');
        const columns = ['ios', 'mac', 'neutral', 'material', 'terminal'];
        const schema = JSON.parse(readFileSync(join(ROOT, 'kernel/tables/schema.json'), 'utf8'));
        const names = schema.scale.map(r => r[0]);
        const whole = base.map(r => r.name).join() === names.join() && base.every(r => columns.every(c => r.columns?.[c]?.value != null && r.columns?.[c]?.source != null));
        evidence.push(`describe: ${base.length} base rows, ${whole ? 'the table\'s nine in its order, each with its five columns\' values and sources' : 'not the table\'s nine, or a column missing'}; space.md override ${JSON.stringify(md?.override ?? null)}`);
        if (base.length !== 9 || !whole || md?.override?.text !== '20px') bad.push('describe: nine base rows with their columns, space.md overridden at 20px');
        const resolved = await s.ask({ method: 'resolve', token: 'space.md' });
        evidence.push(`resolve space.md: base ${resolved.ok?.base?.name}, override ${resolved.ok?.base?.override?.text}`);
        const tuples = resolved.ok?.tuples ?? [];
        evidence.push(`resolve space.md: ${tuples.length} tuple${tuples.length === 1 ? '' : 's'} at ${[...new Set(tuples.map(t => t.value))].join(', ')}`);
        if (resolved.ok?.base?.override?.text !== '20px' || !tuples.length || tuples.some(t => t.value !== '20px')) bad.push('resolve space.md: the app\'s override, 20px in every tuple');
        // The server's own exact assertions of the rows (values, sources, kinds).
        const exact = cargoTest(['-p', 'contract', '--test', 'it', 'tamber_serve_base'], 'the server\'s base-row tests (tamber_serve_base)');
        evidence.push(exact.said);
        if (!exact.ok) bad.push('tamber_serve_base');
        const why = await s.ask({ method: 'explain', token: 'space.md' });
        evidence.push(`explain space.md: kind ${why.ok?.kind}, value ${why.ok?.value}`);
        if (why.ok?.kind !== 'base' || why.ok?.value !== '20px') bad.push('explain space.md: the base, at the app\'s 20px');
        const via = await s.ask({ method: 'explain', token: `${CARDS}.pad` });
        evidence.push(`explain ${CARDS}.pad: bases ${JSON.stringify(via.ok?.bases ?? null)}`);
        if (!(via.ok?.bases ?? []).includes('space.md')) bad.push(`explain ${CARDS}.pad: reaches space.md`);
        const rule = await s.ask({ method: 'explain', token: `${NOTES}.rule` });
        evidence.push(`explain ${NOTES}.rule: value ${rule.ok?.value} (the app's, by its full name)`);
        if (!/^#00ff00/i.test(rule.ok?.value ?? '')) bad.push(`explain ${NOTES}.rule: the app's #00ff00`);
        // A2 part 2's: package tokens on describe, with their package and file.
        const pad = packageRow(described.ok, `${CARDS}.pad`);
        const packaged = a2In(described.ok);
        evidence.push(`describe: ${CARDS}.pad ${pad ? `package ${JSON.stringify(pad.package ?? null)}, file ${JSON.stringify(pad.file ?? null)}` : 'with no package row'}`);
        if (packaged && (pad?.package !== `@exact/${CARDS}` || String(pad?.file ?? '').split('/').pop() !== `${CARDS}.tamber`)) bad.push(`describe: ${CARDS}.pad is @exact/${CARDS}'s, from its ${CARDS}.tamber`);
        if (bad.length) return fail(...evidence, ...bad);
        return packaged ? pass(...evidence) : waits('A2 part 2 (package rows on describe)', ...evidence);
      } finally { await s.stop(); }
    }],
    ['server-writes', 'set-token of a base or package name writes the app\'s line, never a package file; add-token of a base name points to set-token; an op on a package file is refused', async () => {
      const s = await serve();
      try {
        const evidence = [], bad = [];
        const before = packageFiles();
        const untouched = () => { const now = packageFiles(); return (now.clean || !before.clean) && now.bytes.every((t, i) => t === before.bytes[i]); };
        const files = async () => (await s.ask({ method: 'hello' })).ok?.files;
        const write = async op => s.ask({ method: 'transaction', client: 'gate', files: await files(), ops: [op] });
        const design = () => readFileSync(join(s.dir, 'app.tamber'), 'utf8');
        const set = await write({ op: 'set-token', name: 'space.md', value: '24px' });
        evidence.push(`set-token space.md 24px: ${set.ok ? 'written' : JSON.stringify(set.error)}; the app's line ${/^\s*space\.md = 24px/m.test(design()) ? 'there' : 'missing'}; package files ${untouched() ? 'untouched' : 'CHANGED'}`);
        if (!set.ok || !/^\s*space\.md = 24px/m.test(design()) || !untouched()) bad.push('set-token space.md writes the app\'s line and no package file');
        // A2 part 2's: present once describe carries package rows.
        const described = (await s.ask({ method: 'describe' })).ok;
        const a2 = a2In(described);
        const pkg = await write({ op: 'set-token', name: `${CARDS}.fill`, value: '#00ff00' });
        const wrote = new RegExp(`^\\s*${CARDS}\\.fill = #00ff00`, 'm').test(design());
        evidence.push(`set-token ${CARDS}.fill: ${pkg.ok ? (wrote ? 'the app\'s override line' : 'ok, no app line') : `refused (${pkg.error?.message ?? ''})`}; package files ${untouched() ? 'untouched' : 'CHANGED'}`);
        const add = await write({ op: 'add-token', name: 'space.lg', value: '30px' });
        const pointed = !add.ok && /set-token/.test(JSON.stringify(add.error?.diagnostics ?? add.error?.message ?? ''));
        evidence.push(`add-token space.lg: ${add.ok ? 'taken' : `refused: ${JSON.stringify(add.error?.diagnostics?.[0]?.message ?? add.error?.message ?? '')}`}; package files ${untouched() ? 'untouched' : 'CHANGED'}`);
        // The package file as the server names it (A2 part 2's `packages`,
        // `<package>/<file>`), else no probe: a path it doesn't know is
        // refused as no file of the design, which proves nothing of the guard.
        const packageFile = (described?.packages ?? []).map(p => p.file).find(f => String(f).endsWith(`/${CARDS}.tamber`));
        let refusedOutside = false;
        if (packageFile) {
          const outside = await write({ op: 'set-token', file: packageFile, name: `${CARDS}.fill`, value: '#123456' });
          const said = JSON.stringify(outside.error ?? '');
          refusedOutside = !outside.ok && !/is no file of this design/.test(said);
          evidence.push(`set-token naming ${packageFile}: ${outside.ok ? 'taken' : `refused: ${said.slice(0, 160)}`}; package files ${untouched() ? 'untouched' : 'CHANGED'}`);
        } else evidence.push('the server names no package file yet: the guard not probed');
        if (!untouched()) {
          bad.push('a package file changed (tracked bytes): put back');
          FIXTURES.forEach((p, i) => writeFileSync(join(ROOT, `packages/${p}/${p}.tamber`), before.bytes[i]));
        }
        if (bad.length) return fail(...evidence, ...bad);
        const pending = [!(pkg.ok && wrote) && 'set-token on a package name writes the app\'s override', !pointed && 'add-token of a base name refused for set-token', !refusedOutside && 'an op naming a package file refused'].filter(Boolean);
        if (!pending.length) return pass(...evidence);
        return a2 ? fail(...evidence, ...pending.map(p => `A2 part 2 is in, but not: ${p}`)) : waits(`A2 part 2 (${pending.join('; ')})`, ...evidence);
      } finally { await s.stop(); }
    }],
    ['private', 'The fixture packages are private, and nothing publishes them', () => {
      const evidence = [], bad = [];
      for (const p of FIXTURES) {
        const manifest = JSON.parse(readFileSync(join(ROOT, `packages/${p}/package.json`), 'utf8'));
        evidence.push(`@exact/${p}: private ${manifest.private === true}`);
        if (manifest.private !== true || manifest.name !== `@exact/${p}`) bad.push(`@exact/${p} is not private`);
      }
      const publishes = run('git', ['grep', '-n', '-E', '(npm|bun|pnpm|yarn) publish', '--', 'scripts', '.github', 'packages']).out.split('\n').filter(l => FIXTURES.some(p => l.includes(p)) || /workspaces?|packages\/\*/.test(l));
      evidence.push(publishes.length ? `publishing: ${publishes.join(' / ')}` : 'no publish command names them or the workspace');
      if (publishes.length) bad.push('a publish names them');
      return bad.length ? fail(...evidence, ...bad) : pass(...evidence);
    }],
    ['live', 'Live: set-token space.md on the running gate app moves both packages with no reload (web); remove-token returns them to the base', () => live()],
    ['tool', 'The design tool shows base and package tokens and writes the app\'s override (dtool2\'s D)', () => {
      // dtool2's stage 11 tests: any in the tool's own test file (against its
      // stub server), and those against the real server, in
      // scripts/design-tool.test.mjs or design-tool-scale.test.mjs.
      const own = authoredTests(join(ROOT, 'apps/design-tool')).flatMap(f => [...readFileSync(f, 'utf8').matchAll(/^test "(stage 11[^"]*)"/gm)].map(m => [basename(f), m[1]]));
      const real = ['scripts/design-tool.test.mjs', 'scripts/design-tool-scale.test.mjs'].filter(f => existsSync(join(ROOT, f))).flatMap(f =>
        [...readFileSync(join(ROOT, f), 'utf8').matchAll(/^test\('((?:[^'\\]|\\.)*)'/gm)].map(m => [f, m[1].replace(/\\(.)/g, '$1')]).filter(([, n]) => /stage 11/i.test(n)));
      if (!own.length && !real.length) return waits('dtool2\'s D (the tool half): no design-tool test says stage 11 yet');
      const evidence = [], bad = [];
      if (own.length) {
        // verify runs the tool's tests with its stub server up (app.json's
        // `server` command), as dtool2 runs them.
        const r = run(process.execPath, [join(ROOT, 'scripts/verify.mjs'), 'web', '--app', 'design-tool', '--json']);
        let results = [];
        const line = r.out.split('\n').findLast(l => l.startsWith('{"host"'));
        try { results = JSON.parse(line).tests?.results ?? []; } catch { /* said below */ }
        for (const [file, name] of own) {
          const result = results.find(x => x.file === file && x.name === name);
          const passed = result && !result.failures?.length;
          evidence.push(`${file} "${name}": ${passed ? 'pass' : `FAIL (${result ? result.failures.join(' / ').slice(0, 200) : r.out.trim().split('\n').slice(-2).join(' / ').slice(0, 200)})`} (the tool's stub server)`);
          if (!passed) bad.push(name);
        }
      }
      for (const [file, name] of real) {
        const r = run('bun', ['test', file, '-t', name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')]);
        const passed = r.ok && Number(/(\d+) pass/.exec(r.out)?.[1] ?? 0) === 1;
        evidence.push(`${file} "${name.slice(0, 160)}": ${passed ? 'pass (a real tamber serve)' : `FAIL (${r.out.trim().split('\n').slice(-3).join(' / ').slice(0, 200)})`}`);
        if (!passed) bad.push(name);
      }
      if (bad.length) return fail(...evidence);
      return real.length ? pass(...evidence) : waits('dtool2\'s D: a test against the real design server (the tool\'s own passes against its stub)', ...evidence);
    }],
    ['clone', 'An app\'s own name under space. (the clone\'s space.header) keeps its meaning (Q5), on a copy of a signed-out clone\'s design', () => {
      // dt2-gate2 by default; its design and icons copied (nothing signed in,
      // nothing of the clone written), space.header made 44px, a length no
      // column of space.md has, so a fold to the base would show.
      const from = join(opts.clone, 'app.tamber');
      if (!existsSync(from)) return waits(`a clone copy at ${opts.clone} (--clone)`);
      const text = readFileSync(from, 'utf8');
      if (!/^\s*space\.header\s*=/m.test(text)) return waits(`the clone copy at ${opts.clone} names no space.header`);
      const dir = join(WORK, 'clone');
      rmSync(dir, { recursive: true, force: true });
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, 'app.tamber'), text.replace(/^(\s*space\.header\s*=\s*)\S+/m, '$144px'));
      if (existsSync(join(opts.clone, 'assets/icons'))) cpSync(join(opts.clone, 'assets/icons'), join(dir, 'assets/icons'), { recursive: true });
      const r = run('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'eval', join(dir, 'app.tamber')]);
      const got = /^space\.header = (\S+)/m.exec(r.out)?.[1];
      const evidence = [`the copy's space.header = 44px folds to ${got ?? '(none)'} (design-fold evidence: the clone's views are not driven here)`];
      return r.ok && got === '44px' ? pass(...evidence) : fail(...evidence, r.out.split('\n').slice(-4).join(' / '));
    }],
    ['no-regressions', 'Release output unchanged for apps using none of this; conform clean', () => {
      const conform = run(process.execPath, [join(ROOT, 'host/web-js/conform.mjs'), '--synthetic', '--strict', '--linux', '--build', '--jobs', '1', '--wasm-root', join(WORK, 'wasm'), '--out', join(WORK, 'conform')], { timeout: 7_200_000 });
      // Its summary and each target that failed; with no summary (it stopped
      // short), its last lines.
      const said = conform.out.split('\n');
      const summary = said.find(l => /failures across/.test(l)) ?? `no summary${conform.error ? ` (${conform.error.code ?? conform.error.message})` : ''}: ${conform.out.trim().split('\n').slice(-3).join(' / ').slice(0, 400)}`;
      const failed = said.filter(l => /^[\w-]+: [1-9]\d* failures?$/.test(l));
      const names = cargoTest(['-p', 'contract', '--test', 'it', 'tamber_base_scale'], 'an app\'s own space.* names (tamber_base_scale)');
      const evidence = [`conform --synthetic --strict --linux: ${summary}`, ...failed.map(l => `  ${l}`), names.said];
      // Release identity, as stage 9's: every app's plan built at the parent
      // and the end of each run of stage 11 commits on main (--release, that
      // run's JSON). A plan may move only as named: B1's schema digest (the
      // plan header's bytes 17-24, the size unchanged), or an app built on
      // stage 11 whose own source the run changed (the design tool).
      const release = releaseIdentity(opts.release);
      evidence.push(...release.evidence);
      if (!conform.ok || !names.ok || release.status !== 'PASS') return release.status === 'WAITS' && conform.ok && names.ok ? waits(release.on, ...evidence) : fail(...evidence);
      return pass(...evidence);
    }],
  ];
}

/** The command line: hosts, the Android app, the lines to run, the file. */
export function parse(argv) {
  const opts = { hosts: [], app: 'unsaid', only: [], out: join(homedir(), 'x2theme/stage11-gate-run.txt'), clone: join(homedir(), '.tuft/projects/dt2-gate2'), release: null };
  for (let i = 0; i < argv.length; i++) {
    const [flag, v] = [argv[i], argv[i + 1]];
    if (!['--host', '--app', '--only', '--out', '--clone', '--release'].includes(flag)) return `unknown argument ${flag}`;
    if (v === undefined || v.startsWith('--')) return `${flag} needs a value`;
    if (flag === '--host') opts.hosts.push(v); else if (flag === '--only') opts.only.push(v); else opts[flag.slice(2)] = v;
    i++;
  }
  const unknown = opts.hosts.find(h => !HOSTS.includes(h));
  if (unknown) return `--host ${unknown}: one of ${HOSTS.join(', ')}`;
  const ids = lines(opts).map(([id]) => id);
  const missing = opts.only.find(o => !ids.includes(o));
  if (missing) return `--only ${missing}: one of ${ids.join(', ')}`;
  return opts;
}

if (import.meta.main) {
  const opts = parse(process.argv.slice(2));
  if (typeof opts === 'string') { console.error(`${opts}\nusage: bun scripts/stage11-gate.mjs [--host <h> …] [--app <android app>] [--only <line> …] [--out <file>] [--clone <copy>] [--release <json>]`); process.exit(2); }
  mkdirSync(WORK, { recursive: true });
  const head = run('git', ['rev-parse', '--short=10', 'HEAD']).out.trim();
  const selection = [opts.hosts.length ? `hosts ${opts.hosts.join(' ')}` : 'every host', opts.only.length ? `lines ${opts.only.join(' ')}` : 'every line'].join(', ');
  const out = [`# Stage 11 gate run (LLP 1121 §5 stage 11), ${new Date().toISOString()}, ${head}; ${selection}`, ''];
  let failed = 0;
  for (const [id, title, check] of lines(opts)) {
    if (opts.only.length && !opts.only.includes(id)) continue;
    let r;
    try { r = await check(); } catch (e) { r = fail(String(e?.message ?? e)); }
    if (r.status === 'FAIL' || r.status === 'NOT-RUN') failed++;
    const block = [`${r.status}  ${id}: ${title}`, ...r.evidence.map(e => `    ${e}`)];
    console.log(block.join('\n'));
    out.push(...block, '');
  }
  writeFileSync(opts.out, `${out.join('\n')}\n`);
  console.log(`written to ${opts.out}`);
  process.exit(failed ? 1 : 0);
}
