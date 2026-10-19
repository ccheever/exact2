// The stage 11 gate (LLP 1121 §5 stage 11, ~/x2theme/stage11-brief.md "The
// gate"): each line run from what already checks it, its evidence kept, and
// written PASS, FAIL, WAITS-ON (with what it waits on) or NOT-RUN (a line
// the selection left out) to one file. It checks nothing itself but the scale
// table's sources; the hosts' lengths are scripts/scale-check.mjs's, the
// package designs' refusals the contract tests', the server's rows its own
// replies.
//
//   bun scripts/stage11-gate.mjs [--host <h> …] [--app <android app>] [--only <line> …] [--out <file>] [--clone <copy>]
//
// Hosts default to all six: web, linux, macos, ios, the Android reader (--app,
// an app with no data: unsaid; the base values only) and the terminal. The
// fixtures: packages/cards, packages/notes and apps/scale-gate (its design
// sets space.md = 20px; base/ has none). Each host's build must be fresh (a
// stale one's drive says how to build it: host/web/build.mjs caltrain-web,
// host/apple/build.mjs caltrain-apple [--ios --sim], agent-android.mjs build
// unsaid --apk); EXACT_SIM names the simulator, and the Android reader takes
// the emulator its owner file gives.
import { spawn, spawnSync } from 'node:child_process';
import { appendFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const GATE = join(ROOT, 'apps/scale-gate');
// Copies the drives take: inside the checkout, so `@exact/*` resolves through
// its node_modules, and never a fixture itself, which a set-token writes.
const WORK = join(ROOT, 'target/stage11-gate');
export const HOSTS = ['web', 'linux', 'macos', 'ios', 'android', 'terminal'];
// The Android reader runs an app's APK, which reads no package design.
const PACKAGE_HOSTS = HOSTS.filter(h => h !== 'android');

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
  const rows = r.out.split('\n').filter(l => /^(ok|FAIL) /.test(l));
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

/** A child's exit awaited, after `kill` (its group when it leads one). */
async function stop(child) {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  const gone = new Promise(ok => child.once('exit', ok));
  try { process.kill(-child.pid, 'SIGTERM'); } catch { child.kill('SIGTERM'); }
  await Promise.race([gone, new Promise(ok => setTimeout(ok, 10_000))]);
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
  const child = spawn(process.execPath, [join(ROOT, 'scripts/exact.mjs'), 'tamber', 'serve', dir], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'], detached: true });
  const { ask } = await owned(child, /(http:\/\/127\.0\.0\.1:\d+)/, { text: '' });
  return { dir, ask, stop: async () => { await stop(child); rmSync(dir, { recursive: true, force: true }); } };
}

/** The two package designs' bytes, and whether git sees them as committed. */
function packageFiles() {
  const bytes = ['cards', 'notes'].map(p => readFileSync(join(ROOT, `packages/${p}/${p}.tamber`), 'utf8'));
  const clean = run('git', ['diff', '--quiet', '--', 'packages/cards/cards.tamber', 'packages/notes/notes.tamber']).ok;
  return { bytes, clean };
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
  const dev = spawn(process.execPath, [join(ROOT, 'host/web/dev.mjs'), '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_APP_DIR: dir, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'], detached: true });
  const said = { text: '', log: c => appendFileSync(log, c) };
  let drive;
  try {
    const { url: base, ask } = await owned(dev, new RegExp(`(http://127\\.0\\.0\\.1:${port})/`), said);
    // Its first builds settled (5 s with nothing said, at most a minute), so
    // the page the drive opens is not reloaded under it.
    for (let quiet = 0, last = said.text.length, t = 0; quiet < 5 && t < 60; t++) {
      await new Promise(ok => setTimeout(ok, 1000));
      quiet = said.text.length === last ? quiet + 1 : 0;
      last = said.text.length;
    }
    const edit = async op => ask({ method: 'transaction', client: 'gate', files: (await ask({ method: 'hello' })).ok?.files, ops: [op] });
    const settle = ['clock +3000 real', 'clock data', 'clock +3000 real', 'clock data'];
    drive = spawn(process.execPath, [join(ROOT, 'scripts/agent.mjs'), 'web', '--url', base, '--storage', `stage11-gate-${process.pid}`, '--reset-storage', '--json', 'layout', ...settle, 'layout', ...settle, 'layout'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    const replies = [], edits = [];
    const ops = [{ op: 'set-token', name: 'space.md', value: '28px' }, { op: 'remove-token', name: 'space.md' }];
    let heard = '', raced = false, buf = '', queue = Promise.resolve();
    drive.stderr.on('data', c => { heard = (heard + c).slice(-2000); });
    // One reader of the drive's lines; an edit after a layout runs to its
    // answer while later lines wait their turn.
    const onLayout = async line => {
      replies.push(JSON.parse(line));
      if (replies.length > ops.length) return;
      const from = said.text.length;
      edits.push({ reply: await edit(ops[replies.length - 1]), from });
    };
    const read = new Promise((ok, no) => {
      drive.stdout.on('data', c => {
        buf += c;
        let at;
        while ((at = buf.indexOf('\n')) >= 0) {
          const line = buf.slice(0, at); buf = buf.slice(at + 1);
          if (!line.startsWith('{') || !line.includes('"nodes"')) continue;
          // A layout arriving while the edit after the one before it is
          // still unanswered: the drive raced it.
          queue = queue.then(() => { if (edits.length < Math.min(replies.length, ops.length)) raced = true; return onLayout(line); });
        }
      });
      drive.on('exit', code => (code === 0 ? ok() : no(new Error(`the drive exited ${code}: ${(heard || buf).trim().split('\n').filter(l => !/^\s+at /.test(l)).slice(-4).join(' / ')}`))));
      setTimeout(() => no(new Error('the drive took over 5 min')), 300_000);
    });
    await read;
    await queue;
    const [first, moved, back] = replies.map(paddings);
    // What the dev loop said between the set-token and the next edit.
    const setTokenSaid = said.text.slice(edits[0]?.from ?? 0, edits[1]?.from ?? said.text.length);
    const reloaded = /reloaded seq=|[1-9]\d* pages? reloading/.test(setTokenSaid);
    const evidence = [`paddings (cards, notes): ${JSON.stringify(first)} at 20px; ${JSON.stringify(moved)} after set-token space.md 28px; ${JSON.stringify(back)} after remove-token (the web's base)`,
      `edits: ${edits.map(e => (e.reply.ok ? 'ok' : JSON.stringify(e.reply.error))).join(', ')}`,
      `the dev loop after the set-token: ${reloaded ? 'a reload' : /no reload/.test(setTokenSaid) ? 'no reload (a value edit)' : 'nothing said of a reload'}`];
    if (raced) return fail(...evidence, 'a layout was read before the edit before it was answered');
    const held = first.every(p => p === 20) && moved.every(p => p === 28) && back.every(p => p === 16) && edits.every(e => e.reply.ok);
    return held && !reloaded ? pass(...evidence) : fail(...evidence);
  } finally {
    await stop(drive);
    await stop(dev);
    rmSync(dir, { recursive: true, force: true });
    rmSync(dist, { recursive: true, force: true });
  }
}

/** Every scale cell names its source, as the kernel's build holds them (the
 * brief's "no brand's 4 px grid"): `=metric` a metrics row there is, `{px,
 * measured|guideline: id}` whose docs/scale-measured.md entry binds exactly
 * `name = px`, `full` for radius.full, and the terminal's `{cells: n}` (n a
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

/** The gate's lines, in the brief's order. */
export function lines(opts) {
  const pick = h => (opts.hosts.length ? opts.hosts.filter(x => h.includes(x)) : h);
  const scale = join(ROOT, 'host/web-js/conformance/scale');
  const cargoTest = (args, name) => {
    const r = run('cargo', ['test', '-q', ...args]);
    return { ok: ranOk(r.out), said: `${name}: ${ranOk(r.out) ? 'pass' : 'FAIL'}`, out: r.out };
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
        const whole = base.every(r => columns.every(c => r.columns?.[c]?.value != null && r.columns?.[c]?.source != null));
        evidence.push(`describe: ${base.length} base rows, ${whole ? 'each with its five columns\' values and sources' : 'a column missing'}; space.md override ${JSON.stringify(md?.override ?? null)}`);
        if (base.length !== 9 || !whole || md?.override?.text !== '20px') bad.push('describe: nine base rows with their columns, space.md overridden at 20px');
        const resolved = await s.ask({ method: 'resolve', token: 'space.md' });
        evidence.push(`resolve space.md: base ${resolved.ok?.base?.name}, override ${resolved.ok?.base?.override?.text}`);
        if (resolved.ok?.base?.override?.text !== '20px') bad.push('resolve space.md: the app\'s override');
        const why = await s.ask({ method: 'explain', token: 'space.md' });
        evidence.push(`explain space.md: kind ${why.ok?.kind}, value ${why.ok?.value}`);
        if (why.ok?.kind !== 'base' || why.ok?.value !== '20px') bad.push('explain space.md: the base, at the app\'s 20px');
        const via = await s.ask({ method: 'explain', token: 'cards.pad' });
        evidence.push(`explain cards.pad: bases ${JSON.stringify(via.ok?.bases ?? null)}`);
        if (!(via.ok?.bases ?? []).includes('space.md')) bad.push('explain cards.pad: reaches space.md');
        const rule = await s.ask({ method: 'explain', token: 'notes.rule' });
        evidence.push(`explain notes.rule: value ${rule.ok?.value} (the app's, by its full name)`);
        if (!/^#00ff00/i.test(rule.ok?.value ?? '')) bad.push('explain notes.rule: the app\'s #00ff00');
        // A2 part 2's: package tokens on describe, with their package and file.
        const tokens = described.ok?.tokens ?? [];
        const pad = tokens.find(t => t.name === 'cards.pad');
        const packaged = tokens.some(t => t.package != null);
        evidence.push(`describe: cards.pad ${pad ? `package ${JSON.stringify(pad.package ?? null)}, file ${JSON.stringify(pad.file ?? null)}` : 'absent'}`);
        if (packaged && (pad?.package !== '@exact/cards' || !String(pad?.file ?? '').endsWith('cards.tamber'))) bad.push('describe: cards.pad is @exact/cards\'s, from cards.tamber');
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
        const a2 = ((await s.ask({ method: 'describe' })).ok?.tokens ?? []).some(t => t.package != null);
        const pkg = await write({ op: 'set-token', name: 'cards.fill', value: '#00ff00' });
        const wrote = /^\s*cards\.fill = #00ff00/m.test(design());
        evidence.push(`set-token cards.fill: ${pkg.ok ? (wrote ? 'the app\'s override line' : 'ok, no app line') : `refused (${pkg.error?.message ?? ''})`}; package files ${untouched() ? 'untouched' : 'CHANGED'}`);
        const add = await write({ op: 'add-token', name: 'space.lg', value: '30px' });
        const pointed = !add.ok && /set-token/.test(JSON.stringify(add.error?.diagnostics ?? add.error?.message ?? ''));
        evidence.push(`add-token space.lg: ${add.ok ? 'taken' : `refused: ${JSON.stringify(add.error?.diagnostics?.[0]?.message ?? add.error?.message ?? '')}`}; package files ${untouched() ? 'untouched' : 'CHANGED'}`);
        const outside = await write({ op: 'set-token', file: '../../packages/cards/cards.tamber', name: 'cards.fill', value: '#123456' });
        const refusedOutside = !outside.ok && /outside|package|not the app/i.test(JSON.stringify(outside.error ?? ''));
        evidence.push(`set-token naming the package file: ${outside.ok ? 'taken' : `refused: ${JSON.stringify(outside.error?.message ?? outside.error ?? '')}`}; package files ${untouched() ? 'untouched' : 'CHANGED'}`);
        if (!untouched()) bad.push('a package file changed (tracked bytes)');
        if (bad.length) return fail(...evidence, ...bad);
        const pending = [!(pkg.ok && wrote) && 'set-token on a package name writes the app\'s override', !pointed && 'add-token of a base name refused for set-token', !refusedOutside && 'an op naming a package file refused'].filter(Boolean);
        if (!pending.length) return pass(...evidence);
        return a2 ? fail(...evidence, ...pending.map(p => `A2 part 2 is in, but not: ${p}`)) : waits(`A2 part 2 (${pending.join('; ')})`, ...evidence);
      } finally { await s.stop(); }
    }],
    ['live', 'Live: set-token space.md on the running gate app moves both packages with no reload (web); remove-token returns them to the base', () => live()],
    ['tool', 'The design tool shows base and package tokens and writes the app\'s override (dtool2\'s D)', () => {
      // Its tests that say stage 11 or the base scale by name: dtool2's.
      const names = [...readFileSync(join(ROOT, 'scripts/design-tool.test.mjs'), 'utf8').matchAll(/^test\('((?:[^'\\]|\\.)*)'/gm)].map(m => m[1].replace(/\\(.)/g, '$1')).filter(n => /stage 11|base (row|name|scale)|package token/i.test(n));
      if (!names.length) return waits('dtool2\'s D (the tool half): no design-tool test names stage 11, the base scale or a package token yet');
      const r = run('bun', ['test', 'scripts/design-tool.test.mjs', '-t', names.map(n => n.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|')]);
      const passed = Number(/(\d+) pass/.exec(r.out)?.[1] ?? 0);
      return r.ok && passed >= names.length ? pass(...names.map(n => `design-tool test: ${n}`), `${passed} passed`) : fail(...names, r.out.split('\n').slice(-6).join(' / '));
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
      const conform = run(process.execPath, [join(ROOT, 'host/web-js/conform.mjs'), '--synthetic', '--strict', '--linux', '--build', '--jobs', '1', '--wasm-root', join(WORK, 'wasm'), '--out', join(WORK, 'conform')]);
      const summary = conform.out.split('\n').find(l => /failures across/.test(l)) ?? conform.out.trim().split('\n').slice(-1)[0];
      const names = cargoTest(['-p', 'contract', '--test', 'it', 'tamber_base_scale'], 'an app\'s own space.* names (tamber_base_scale)');
      const evidence = [`conform --synthetic --strict --linux: ${summary}`, names.said];
      if (!conform.ok || !names.ok) return fail(...evidence);
      // The release builds' byte identity is run on main for E, against the
      // commit before stage 11 (B2's 14/14 was against B1's).
      return waits('E\'s release identity run on main (every app\'s release output against the commit before stage 11)', ...evidence);
    }],
  ];
}

/** The command line: hosts, the Android app, the lines to run, the file. */
export function parse(argv) {
  const opts = { hosts: [], app: 'unsaid', only: [], out: join(homedir(), 'x2theme/stage11-gate-run.txt'), clone: join(homedir(), '.tuft/projects/dt2-gate2') };
  for (let i = 0; i < argv.length; i++) {
    const [flag, v] = [argv[i], argv[i + 1]];
    if (!['--host', '--app', '--only', '--out', '--clone'].includes(flag)) return `unknown argument ${flag}`;
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
  if (typeof opts === 'string') { console.error(`${opts}\nusage: bun scripts/stage11-gate.mjs [--host <h> …] [--app <android app>] [--only <line> …] [--out <file>] [--clone <copy>]`); process.exit(2); }
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
