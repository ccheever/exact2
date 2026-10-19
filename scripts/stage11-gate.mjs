// The stage 11 gate (LLP 1121 §5 stage 11, ~/x2theme/stage11-brief.md "The
// gate"): each line run from what already checks it, its evidence kept, and
// written PASS, FAIL or WAITS-ON (with what it waits on) to one file. It
// checks nothing itself but the scale table's sources; the hosts' lengths are
// scripts/scale-check.mjs's, the package designs' refusals the contract
// tests', the server's rows its own replies.
//
//   bun scripts/stage11-gate.mjs [--host <h> …] [--app <android app>] [--only <line> …] [--out <file>] [--clone <copy>]
//
// Hosts default to web, linux, macos and ios for the packages' lines, and
// those plus the Android reader (--app, an app with no data: unsaid) and the
// terminal for the base values. The fixtures: packages/cards, packages/notes
// and apps/scale-gate (its design sets space.md = 20px; base/ has none).
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const GATE = join(ROOT, 'apps/scale-gate');
// Copies the drives take: inside the checkout, so `@exact/*` resolves through
// its node_modules, and never the fixture itself, which a set-token writes.
const WORK = join(ROOT, 'target/stage11-gate');
const PACKAGE_HOSTS = ['web', 'linux', 'macos', 'ios'];
const BASE_HOSTS = [...PACKAGE_HOSTS, 'android', 'terminal'];

/** A line's result: its status, and the evidence it rests on, a line each. */
const pass = (...evidence) => ({ status: 'PASS', evidence });
const fail = (...evidence) => ({ status: 'FAIL', evidence });
const waits = (on, ...evidence) => ({ status: `WAITS-ON ${on}`, evidence });

/** `cmd args` run in the checkout: its status and its output. */
function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { cwd: ROOT, encoding: 'utf8', timeout: 1_800_000, maxBuffer: 1 << 26, ...opts });
  return { ok: r.status === 0 && !r.error, out: `${r.stdout ?? ''}${r.stderr ?? ''}`, error: r.error };
}

/** scale-check's rows for `plan` on `host`: `ok` when every box held. */
function scaleCheck(plan, expect, host, app) {
  const args = [join(ROOT, 'scripts/scale-check.mjs'), plan, expect, '--host', host, '--out', join(WORK, `shots-${host}`)];
  if (host === 'android') args.push('--app', app);
  const r = run(process.execPath, args);
  const rows = r.out.split('\n').filter(l => /^(ok|FAIL) /.test(l));
  return { ok: r.ok && rows.length > 0 && rows.every(l => l.startsWith('ok')), rows: rows.length ? rows : [r.out.trim().split('\n').slice(-3).join(' / ')] };
}

/** The gate app as a plan with no app id (a host's plan push refuses another
 * app's), its design beside it unless `bare`. */
function planCopy(name, bare) {
  const dir = join(WORK, name);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  cpSync(join(GATE, bare ? 'base/app.contract' : 'app.contract'), join(dir, 'app.contract'));
  if (!bare) cpSync(join(GATE, 'app.tamber'), join(dir, 'app.tamber'));
  return join(dir, 'app.contract');
}

/** The design server on a copy of the gate app: `ask(request)` its reply. */
async function serve() {
  const dir = join(WORK, 'served');
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(join(GATE, f), join(dir, f));
  const child = spawn(process.execPath, [join(ROOT, 'scripts/exact.mjs'), 'tamber', 'serve', dir], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  const url = await new Promise((ok, no) => {
    const hear = chunk => {
      said += chunk;
      const m = /(http:\/\/127\.0\.0\.1:\d+)/.exec(said);
      if (m) ok(m[1]);
    };
    child.stdout.on('data', hear);
    child.stderr.on('data', hear);
    child.on('exit', code => no(new Error(`tamber serve exited (${code}): ${said}`)));
    setTimeout(() => no(new Error(`tamber serve said no address: ${said}`)), 900_000);
  });
  const { token } = await (await fetch(`${url}/__tamber/token`)).json();
  const ask = async request => (await fetch(`${url}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) })).json();
  return { url, dir, ask, stop: () => child.kill() };
}

/** The packages' paddings (`[cards, notes]`, px) in an agent's `layout` reply. */
function paddings(reply) {
  const of = id => reply.nodes.find(n => n.testId === id);
  return [['cards-card', 'cards-inner'], ['notes-note', 'notes-inner']].map(([box, child]) => of(child).y - of(box).y);
}

/** The live line: the web dev loop on a copy of the gate app, its page
 * driven by the agent; between two layouts the dev loop's design server
 * sets space.md to 28px, then removes the app's line (the base again). */
async function live() {
  const dir = join(WORK, 'live');
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  for (const f of ['app.contract', 'app.tamber', 'app.json']) cpSync(join(GATE, f), join(dir, f));
  const port = 20_000 + (process.pid % 20_000);
  const base = `http://127.0.0.1:${port}`;
  const dev = spawn(process.execPath, [join(ROOT, 'host/web/dev.mjs'), '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_APP_DIR: dir, EXACT_WEB_DIST: join(WORK, 'live-dist') }, stdio: ['ignore', 'pipe', 'pipe'] });
  let drive;
  try {
    await new Promise((ok, no) => {
      let said = '';
      const hear = c => { said += c; if (said.includes(`127.0.0.1:${port}/`)) ok(); };
      dev.stdout.on('data', hear); dev.stderr.on('data', hear);
      dev.on('exit', code => no(new Error(`the dev loop exited (${code}): ${said.slice(-400)}`)));
      setTimeout(() => no(new Error('the dev loop said no address in 15 min')), 900_000);
    });
    const { token } = await (await fetch(`${base}/__tamber/token`)).json();
    const ask = async request => (await fetch(`${base}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) })).json();
    const edit = async op => ask({ method: 'transaction', client: 'gate', files: (await ask({ method: 'hello' })).ok?.files, ops: [op] });
    const settle = ['clock +3000 real', 'clock data', 'clock +3000 real', 'clock data'];
    drive = spawn(process.execPath, [join(ROOT, 'scripts/agent.mjs'), 'web', '--url', base, '--storage', `stage11-gate-${process.pid}`, '--json', 'layout', ...settle, 'layout', ...settle, 'layout'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
    const replies = [], edits = [];
    let heard = '';
    drive.stderr.on('data', c => { heard = (heard + c).slice(-2000); });
    await new Promise((ok, no) => {
      let buf = '';
      drive.stdout.on('data', async c => {
        buf += c;
        let at;
        while ((at = buf.indexOf('\n')) >= 0) {
          const line = buf.slice(0, at); buf = buf.slice(at + 1);
          if (!line.startsWith('{') || !line.includes('"nodes"')) continue;
          replies.push(JSON.parse(line));
          // After the first layout, the edit; after the second, its removal.
          if (replies.length === 1) edits.push(await edit({ op: 'set-token', name: 'space.md', value: '28px' }));
          if (replies.length === 2) edits.push(await edit({ op: 'remove-token', name: 'space.md' }));
        }
      });
      drive.on('exit', code => (code === 0 ? ok() : no(new Error(`the drive exited ${code}: ${(heard || buf).trim().split('\n').slice(-4).join(' / ')}`))));
    });
    const [first, moved, back] = replies.map(paddings);
    const evidence = [`paddings (cards, notes): ${JSON.stringify(first)} at 20px; ${JSON.stringify(moved)} after set-token space.md 28px; ${JSON.stringify(back)} after remove-token (the web's base)`,
      `edits: ${edits.map(e => (e.ok ? 'ok' : JSON.stringify(e.error))).join(', ')}`];
    const live = first.every(p => p === 20) && moved.every(p => p === 28) && edits.every(e => e.ok);
    if (live && back.every(p => p === 16)) return pass(...evidence);
    // Removed, the page reads the base's variable, which B2 declares on :root.
    if (live && back.every(p => p === 0)) return waits('B2 (the base\'s variables on :root: a removed override reads none)', ...evidence);
    return fail(...evidence);
  } finally {
    drive?.kill();
    dev.kill();
  }
}

/** Every scale cell names its source (the brief's "no brand's 4 px grid"):
 * `=metric` a metrics row there is, `{px, measured|guideline: id}` an entry
 * docs/scale-measured.md binds to that value, `full` for radius.full, and the
 * terminal's `{cells: n}` or a radius's 0. A bare number is no source. */
export function citations(schema, measured) {
  const metrics = new Set((schema.metrics ?? []).map(r => r[0]));
  const bad = [];
  for (const [name, ...cells] of schema.scale) {
    cells.forEach((cell, i) => {
      const terminal = i === cells.length - 1;
      const where = `${name} column ${i + 1}`;
      if (typeof cell === 'string' && cell.startsWith('=')) {
        if (!metrics.has(cell.slice(1))) bad.push(`${where}: ${cell} names no metrics row`);
      } else if (cell === 'full') {
        if (name !== 'radius.full') bad.push(`${where}: full outside radius.full`);
      } else if (terminal && (cell?.cells !== undefined || (cell === 0 && name.startsWith('radius.')))) {
        // The terminal's cells, or a corner it never draws.
      } else if (cell && typeof cell === 'object' && Number.isFinite(cell.px) && (cell.measured || cell.guideline)) {
        const id = cell.measured ?? cell.guideline;
        const entry = measured.split(/\n(?=#)/).find(s => s.includes(`\`${id}\``));
        const step = name.split('.')[1];
        if (!entry) bad.push(`${where}: ${id} has no entry in docs/scale-measured.md`);
        else if (!new RegExp(`${name.replace('.', '\\.')}\\s*=\\s*${cell.px}\\b|${step}\\s*=\\s*${cell.px}\\b`).test(entry)) bad.push(`${where}: ${id}'s entry binds no ${name} = ${cell.px}`);
      } else bad.push(`${where}: ${JSON.stringify(cell)} names no source`);
    });
  }
  return bad;
}

/** The gate's lines, in the brief's order. */
export function lines(opts) {
  const hosts = h => (opts.hosts.length ? opts.hosts.filter(x => h.includes(x)) : h);
  const scaleFixture = join(ROOT, 'host/web-js/conformance/scale/app.contract');
  // scale-check reads base names (space.md) from the table once C2 is in.
  const named = readFileSync(join(ROOT, 'scripts/scale-check.mjs'), 'utf8').includes('baseLength');
  const c2 = () => waits('C2 (x2/s11c2, after B2): scale-check reads no base names on this branch');
  return [
    ['base-values', 'With no app value set, space.* and radius.* resolve per host to the scale table\'s values (Q3)', () => {
      if (!existsSync(scaleFixture) || !named) return c2();
      const rows = hosts(BASE_HOSTS).map(h => [h, scaleCheck(scaleFixture, scaleFixture.replace('app.contract', 'expect.json'), h, opts.app)]);
      const evidence = rows.flatMap(([h, r]) => [`${h}: ${r.ok ? 'all held' : 'FAILED'}`, ...r.rows.map(l => `  ${l}`)]);
      return rows.every(([, r]) => r.ok) ? pass(...evidence) : fail(...evidence);
    }],
    ['citations', 'Every row cites the metric or guideline it comes from; no row is a brand\'s 4 px grid', () => {
      const schema = JSON.parse(readFileSync(join(ROOT, 'kernel/tables/schema.json'), 'utf8'));
      const bad = citations(schema, readFileSync(join(ROOT, 'docs/scale-measured.md'), 'utf8'));
      const kernel = run('cargo', ['test', '-q', '-p', 'exact-kernel', '--lib', 'style::scale']);
      const evidence = [`${schema.scale.length} rows, ${schema.scale.reduce((n, r) => n + r.length - 1, 0)} cells read`, ...bad, `kernel style::scale tests: ${kernel.ok ? 'pass' : 'FAIL'}`];
      return bad.length === 0 && kernel.ok ? pass(...evidence) : fail(...evidence, kernel.out.split('\n').slice(-5).join(' / '));
    }],
    ['packages-restyle', 'Two packages restyle together from one app value (space.md = 20px), web, iOS, the Mac and Linux', () => {
      if (!named) return c2();
      const plan = planCopy('plan', false);
      const rows = hosts(PACKAGE_HOSTS).map(h => [h, scaleCheck(plan, join(GATE, 'expect.json'), h)]);
      const evidence = [...rows.flatMap(([h, r]) => [`${h}: ${r.ok ? 'both packages at 20 px' : 'FAILED'}`, ...r.rows.map(l => `  ${l}`)]),
        'terminal: not in this line: the packages\' views are written in px, which a terminal entry refuses (it writes cells)'];
      return rows.every(([, r]) => r.ok) ? pass(...evidence) : fail(...evidence);
    }],
    ['packages-base', 'With no app value, both packages read each host\'s base values', () => {
      if (!named) return c2();
      const plan = planCopy('bare', true);
      const rows = hosts(PACKAGE_HOSTS).map(h => [h, scaleCheck(plan, join(GATE, 'base/expect.json'), h)]);
      const evidence = rows.flatMap(([h, r]) => [`${h}: ${r.ok ? 'the base' : 'FAILED'}`, ...r.rows.map(l => `  ${l}`)]);
      return rows.every(([, r]) => r.ok) ? pass(...evidence) : fail(...evidence);
    }],
    ...[
      ['package-full-name', 'A package\'s token is set by the app by its full name', 'a_package_design_styles_its_views_and_the_app_sets_its_tokens_by_full_name'],
      ['package-one-name', 'Two packages defining one name is refused unless the app sets it', 'one_name_from_two_files_is_refused_unless_the_app_sets_it'],
      ['package-no-dials', 'A package declaring a dial or an axis is refused', 'axes_dials_params_components_icons_and_exports_are_not_a_packages'],
      ['package-when-axis', 'A package token whose `when` names an axis the app doesn\'t declare is refused', 'a_package_tokens_when_names_only_the_apps_axes'],
      ['package-no-base', 'A package may not define a base name (an app may, for every package: ruling 4)', 'a_package_may_not_name_a_base_name_nor_reach_outside_itself'],
    ].map(([id, title, test]) => [id, title, () => {
      const r = run('cargo', ['test', '-q', '-p', 'contract', '--test', 'it', `package_designs::${test}`, '--', '--exact']);
      const ran = /test result: ok\. 1 passed/.test(r.out);
      return ran ? pass(`contract tests package_designs::${test}: pass`) : fail(`package_designs::${test}`, r.out.split('\n').slice(-6).join(' / '));
    }]),
    ['server-rows', 'The server shows base rows (their columns, the app\'s override), package tokens with their package, and explain\'s source', async () => {
      const s = await serve();
      try {
        const evidence = [], bad = [];
        const described = await s.ask({ method: 'describe' });
        const base = described.ok?.base ?? [];
        const md = base.find(r => r.name === 'space.md');
        evidence.push(`describe: ${base.length} base rows; space.md override ${JSON.stringify(md?.override ?? null)}`);
        if (base.length !== 9 || md?.override?.text !== '20px') bad.push('describe: nine base rows, space.md overridden at 20px');
        const tokens = described.ok?.tokens ?? [];
        const pad = tokens.find(t => t.name === 'cards.pad');
        evidence.push(`describe: cards.pad ${pad ? `package ${JSON.stringify(pad.package ?? null)}, file ${JSON.stringify(pad.file ?? null)}` : 'absent'}`);
        const packaged = pad?.package != null;
        const why = await s.ask({ method: 'explain', token: 'space.md' });
        evidence.push(`explain space.md: kind ${why.ok?.kind}, value ${why.ok?.value}`);
        if (why.ok?.kind !== 'base' || why.ok?.value !== '20px') bad.push('explain space.md: the base, at the app\'s 20px');
        const via = await s.ask({ method: 'explain', token: 'cards.pad' });
        evidence.push(`explain cards.pad: bases ${JSON.stringify(via.ok?.bases ?? null)}`);
        if (!(via.ok?.bases ?? []).includes('space.md')) bad.push('explain cards.pad: reaches space.md');
        if (bad.length) return fail(...evidence, ...bad);
        return packaged ? pass(...evidence) : waits('A2 part 2 (package rows on describe)', ...evidence);
      } finally { s.stop(); }
    }],
    ['server-writes', 'set-token of a base or package name writes the app\'s line, never a package file; add-token of a base name points to set-token; an op on a package file is refused', async () => {
      const s = await serve();
      try {
        const evidence = [], bad = [];
        const before = ['cards', 'notes'].map(p => readFileSync(join(ROOT, `packages/${p}/${p}.tamber`), 'utf8'));
        const files = async () => (await s.ask({ method: 'hello' })).ok?.files;
        const set = await s.ask({ method: 'transaction', client: 'gate', files: await files(), ops: [{ op: 'set-token', name: 'space.md', value: '24px' }] });
        const design = readFileSync(join(s.dir, 'app.tamber'), 'utf8');
        evidence.push(`set-token space.md 24px: ${set.ok ? 'written' : JSON.stringify(set.error)}; app.tamber ${/space\.md = 24px/.test(design) ? 'has' : 'lacks'} the line`);
        if (!set.ok || !/space\.md = 24px/.test(design)) bad.push('set-token space.md writes the app\'s line');
        const pkg = await s.ask({ method: 'transaction', client: 'gate', files: await files(), ops: [{ op: 'set-token', name: 'cards.fill', value: '#00ff00' }] });
        const wrote = /cards\.fill = #00ff00/.test(readFileSync(join(s.dir, 'app.tamber'), 'utf8'));
        evidence.push(`set-token cards.fill: ${pkg.ok ? (wrote ? 'the app\'s override line' : 'ok, no app line') : `refused (${pkg.error?.message ?? ''})`}`);
        const after = ['cards', 'notes'].map(p => readFileSync(join(ROOT, `packages/${p}/${p}.tamber`), 'utf8'));
        if (after.some((t, i) => t !== before[i])) bad.push('a package file changed');
        evidence.push(`package files ${after.every((t, i) => t === before[i]) ? 'untouched' : 'CHANGED'}`);
        const add = await s.ask({ method: 'transaction', client: 'gate', files: await files(), ops: [{ op: 'add-token', name: 'space.lg', value: '30px' }] });
        evidence.push(`add-token space.lg: ${add.ok ? 'taken' : `refused: ${add.error?.message ?? JSON.stringify(add.error)}`}`);
        const files2 = await files();
        const outside = await s.ask({ method: 'transaction', client: 'gate', files: { ...files2, '../../packages/cards/cards.tamber': 'x' }, ops: [{ op: 'set-token', name: 'space.md', value: '26px' }] });
        evidence.push(`an op naming a package file: ${outside.ok ? 'taken' : `refused: ${outside.error?.message ?? ''}`}`);
        if (bad.length) return fail(...evidence, ...bad);
        // A2 part 2's: a package name's set-token writes the app's override,
        // add-token of a base name points to set-token, and an op naming a
        // package file is refused.
        const pending = [!(pkg.ok && wrote) && 'set-token on a package name', !(!add.ok && /set-token/.test(JSON.stringify(add.error))) && 'add-token of a base name refused for set-token', outside.ok && 'an op naming a package file refused'].filter(Boolean);
        return pending.length ? waits(`A2 part 2 (${pending.join('; ')})`, ...evidence) : pass(...evidence);
      } finally { s.stop(); }
    }],
    ['live', 'Live: set-token space.md on the running gate app moves both packages with no rebuild (web), remove-token returns them to the base', () => live()],
    ['tool', 'The design tool shows base and package tokens and writes the app\'s override (dtool2\'s D)', () => {
      // Its tests that say stage 11 or the base scale by name: dtool2's.
      const names = [...readFileSync(join(ROOT, 'scripts/design-tool.test.mjs'), 'utf8').matchAll(/^test\('((?:[^'\\]|\\.)*)'/gm)].map(m => m[1]).filter(n => /stage 11|base (row|name|scale)|package token/i.test(n));
      if (!names.length) return waits('dtool2\'s D (the tool half): no design-tool test names stage 11, the base scale or a package token yet');
      const r = run('bun', ['test', 'scripts/design-tool.test.mjs', '-t', names.map(n => n.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|')]);
      return r.ok ? pass(...names.map(n => `design-tool test: ${n}`)) : fail(...names, r.out.split('\n').slice(-6).join(' / '));
    }],
    ['clone', 'The clone\'s space.header keeps its meaning (Q5), on a signed-out copy of its design', () => {
      // A clone copy signed out (dt2-gate2 by default), its design copied
      // here without its icon tokens (whose files the fold reads; they name
      // no space), so nothing is signed in and nothing of the clone written.
      const from = join(opts.clone, 'app.tamber');
      if (!existsSync(from)) return waits(`a clone copy at ${opts.clone} (--clone)`);
      const text = readFileSync(from, 'utf8');
      const said = /^\s*space\.header\s*=\s*(\S+)/m.exec(text)?.[1];
      if (!said) return waits(`the clone copy at ${opts.clone} names no space.header`);
      const dir = join(WORK, 'clone');
      rmSync(dir, { recursive: true, force: true });
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, 'app.tamber'), text.split('\n').filter(l => !/^\s+icon\.\S+\s*=/.test(l)).join('\n'));
      const r = run('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'eval', join(dir, 'app.tamber')]);
      const got = /^space\.header = (\S+)/m.exec(r.out)?.[1];
      const evidence = [`the clone's app.tamber: space.header = ${said}; this checkout folds it to ${got ?? '(none)'}`];
      return r.ok && got === said ? pass(...evidence) : fail(...evidence, r.out.split('\n').slice(-4).join(' / '));
    }],
    ['no-regressions', 'Release output unchanged for apps using none of this; the clone\'s space.header keeps its meaning; conform clean', () => {
      const conform = run(process.execPath, [join(ROOT, 'host/web-js/conform.mjs'), '--synthetic', '--strict', '--linux', '--build', '--jobs', '1', '--wasm-root', join(WORK, 'wasm'), '--out', join(WORK, 'conform')]);
      const summary = conform.out.split('\n').find(l => /failures across/.test(l)) ?? conform.out.trim().split('\n').slice(-1)[0];
      const header = run('cargo', ['test', '-q', '-p', 'contract', '--test', 'it', 'tamber_base_scale']);
      const evidence = [`conform --synthetic --strict --linux: ${summary}`, `an app's own space.* names (tamber_base_scale): ${header.ok ? 'pass' : 'FAIL'}`, 'release identity: B2\'s 14/14 (theme2 22:07); E re-runs it on main after B2, C2 and A2'];
      return conform.ok && header.ok ? pass(...evidence) : fail(...evidence);
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
  return opts;
}

if (import.meta.main) {
  const opts = parse(process.argv.slice(2));
  if (typeof opts === 'string') { console.error(`${opts}\nusage: bun scripts/stage11-gate.mjs [--host <h> …] [--app <android app>] [--only <line> …] [--out <file>] [--clone <copy>]`); process.exit(2); }
  mkdirSync(WORK, { recursive: true });
  const head = run('git', ['rev-parse', '--short=10', 'HEAD']).out.trim();
  const out = [`# Stage 11 gate run (LLP 1121 §5 stage 11), ${new Date().toISOString()}, ${head}`, ''];
  let failed = 0;
  for (const [id, title, check] of lines(opts)) {
    if (opts.only.length && !opts.only.includes(id)) continue;
    let r;
    try { r = await check(); } catch (e) { r = fail(String(e?.message ?? e)); }
    if (r.status === 'FAIL') failed++;
    const block = [`${r.status}  ${id}: ${title}`, ...r.evidence.map(e => `    ${e}`)];
    console.log(block.join('\n'));
    out.push(...block, '');
  }
  writeFileSync(opts.out, `${out.join('\n')}\n`);
  console.log(`written to ${opts.out}`);
  process.exit(failed ? 1 : 0);
}
