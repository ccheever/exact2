// The JS dev loop's builds (host/web-js/dev.mjs): a build that an input
// moved during is set aside, never published (not dist, which a fresh page
// loads, nor the design or a reload); an edit stops a young build; a value
// batch during a verifying build applies at once, and the build after
// carries it. EXACT_DEV_BUILD_HOLD_MS holds each build's end, as a slow one.
import { afterEach, expect, test } from 'bun:test';
import { spawnOwned, stopAll } from '../../scripts/test-children.mjs';
import { createHash } from 'node:crypto';
import { mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const ROOT = resolve(new URL('../..', import.meta.url).pathname);
const sha = text => createHash('sha256').update(text).digest('hex');

// A dev loop a test started ends with the test, a timeout included.
afterEach(() => stopAll());

/** The JS dev loop on the app at `dir`, its dist in a temporary directory,
 * `env` beside the process's own. */
async function devLoop(dir, env = {}) {
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-js-builds-'));
  // Owned: ended with what it starts when the test ends, a timeout included (scripts/test-children.mjs).
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', dir, '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist, ...env }, stdio: ['ignore', 'pipe', 'pipe'] });
  const stop = async () => {
    await dev.stop(30_000);
    rmSync(dist, { recursive: true, force: true });
  };
  let out = '', err = '';
  dev.stderr.on('data', d => { err = (err + d).slice(-4000); });
  try {
    await new Promise((ok, fail) => {
      dev.stdout.on('data', d => { out += d; if (out.includes(`127.0.0.1:${port}/`)) ok(); });
      dev.on('error', fail);
      dev.on('exit', code => fail(new Error(`the dev loop exited ${code}: ${out}${err}`)));
    });
  } catch (e) { await stop(); throw e; }
  return { base: `http://127.0.0.1:${port}`, stop, said: () => out, dist };
}

test('the JS loop: a build an edit lands during is set aside, never published, and the newer files build', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-superseded-'));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Superseded', app: { id: 'com.exact.superseded', name: 'Superseded' } }));
  const view = (text) => `component App\n  view\n    column background-color="surface.card" min-height=40\n      text "${text}"\n`;
  writeFileSync(join(dir, 'app.contract'), view('qz-first'));
  writeFileSync(join(dir, 'app.tamber'), 'tamber superseded\n\ntheme\n  surface.card = light-dark(#f1f3f5, #16181c)\n');
  let loop = null;
  try {
    // Each build's end comes 2 s after it: an edit lands as one ends.
    loop = await devLoop(dir, { EXACT_DEV_BUILD_HOLD_MS: '2000' });
    const { said, dist } = loop;
    const until = async (pass, ms) => { const end = Date.now() + ms; while (!pass()) { if (Date.now() > end) throw new Error(`not seen in: ${said().slice(-2000)}`); await new Promise(ok => setTimeout(ok, 100)); } };
    await until(() => said().includes('plan ready') && said().includes('design reader ready'), 900_000);
    // What dist holds, whatever file carries the text.
    const holds = (text) => { const walk = (d) => readdirSync(d, { withFileTypes: true }).some(e => e.isDirectory() ? walk(join(d, e.name)) : readFileSync(join(d, e.name)).includes(text)); return walk(dist); };
    expect(holds('qz-first')).toBe(true);
    // An edit, and another while its build runs: the first's build is set
    // aside, and only the second's is published.
    // Sampled throughout: a fresh page never loads the set-aside build.
    let stale = false;
    const sampler = setInterval(() => { try { stale ||= holds('qz-second'); } catch { /* replaced meanwhile */ } }, 10);
    writeFileSync(join(dir, 'app.contract'), view('qz-second'));
    // Its build (a fraction of a second here) has read the files, and is
    // held 2 s at its end.
    await new Promise(ok => setTimeout(ok, 1200));
    writeFileSync(join(dir, 'app.contract'), view('qz-third'));
    await until(() => said().includes('edit → plan ready') && holds('qz-third'), 300_000);
    await new Promise(ok => setTimeout(ok, 500));
    clearInterval(sampler);
    expect(said()).toContain('edit → a build superseded');
    // The input that moved, named (stage 15 D1.5).
    expect(said()).toMatch(/set aside: app\.contract moved while it built/);
    expect(stale).toBe(false);
  } finally {
    await loop?.stop().catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  }
}, 1_200_000);

test('the JS loop: an edit stops a young build, leaving dist as it was; a value batch during a verifying build applies, and the next build carries it', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-stopped-'));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Stopped', app: { id: 'com.exact.stopped', name: 'Stopped' } }));
  const view = (text) => `component App\n  view\n    column background-color="surface.card" min-height=40\n      text "${text}"\n`;
  const design = (dark) => `tamber stopped\n\ntheme\n  surface.card = light-dark(#f1f3f5, ${dark})\n`;
  writeFileSync(join(dir, 'app.contract'), view('qz-one'));
  writeFileSync(join(dir, 'app.tamber'), design('#16181c'));
  let loop = null;
  try {
    loop = await devLoop(dir, { EXACT_DEV_BUILD_HOLD_MS: '4000' });
    const { said, dist, base } = loop;
    const until = async (pass, ms) => { const end = Date.now() + ms; while (!pass()) { if (Date.now() > end) throw new Error(`not seen in: ${said().slice(-2000)}`); await new Promise(ok => setTimeout(ok, 50)); } };
    const count = (re) => (said().match(re) ?? []).length;
    await until(() => said().includes('plan ready') && said().includes('design reader ready'), 900_000);
    const bytes = () => { const out = []; const walk = (d) => { for (const e of readdirSync(d, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) e.isDirectory() ? walk(join(d, e.name)) : out.push(e.name, sha(readFileSync(join(d, e.name)))); }; walk(dist); return out.join(','); };
    let first = bytes(), seen = new Set();
    // A sample is one dist (the same directory before and after it is read;
    // a swap is two renames).
    const sample = () => { const ino = statSync(dist).ino, b = bytes(); return statSync(dist).ino === ino ? b : null; };
    const sampler = setInterval(() => { try { const b = sample(); if (b) seen.add(b); } catch { /* replaced meanwhile */ } }, 10);
    // Three edits at once: the first's build is stopped young, and only
    // that one (one stop in a burst); the last edit's build publishes.
    // The second edit must find the first's build begun: a build begins
    // after the edit's debounce and its own preparation (the compiler's
    // copy, the reader's build started), past 30 ms on a loaded machine,
    // and then both edits are one build, published, and the burst is
    // made again.
    let aside, plans;
    for (let tries = 0; ; tries++) {
      if (tries === 5) throw new Error(`no build of a burst's first edit was begun by its second, in 5 tries: ${said().slice(-2000)}`);
      first = bytes(); seen = new Set(); aside = count(/edit → a build superseded/g); plans = count(/edit → plan ready/g);
      const readers = count(/design reader ready/g);
      writeFileSync(join(dir, 'app.contract'), view(`qz-two-${tries}`));
      await new Promise(ok => setTimeout(ok, 30));
      writeFileSync(join(dir, 'app.contract'), view(`qz-three-${tries}`));
      await until(() => said().includes('(aborted)') || count(/edit → plan ready/g) > plans, 300_000);
      if (said().includes('(aborted)')) break;
      await until(() => count(/design reader ready/g) > readers, 300_000);
    }
    // The stopped build ends (set aside) and the next begins; an edit in
    // that one sets it aside too, but stops it not.
    await until(() => count(/edit → a build superseded/g) >= aside + 1, 60_000);
    await new Promise(ok => setTimeout(ok, 300));
    writeFileSync(join(dir, 'app.contract'), view('qz-four'));
    await until(() => count(/edit → plan ready/g) > plans, 300_000);
    expect(count(/\(aborted\)/g)).toBe(1);
    expect(count(/edit → a build superseded/g)).toBe(aside + 2);
    clearInterval(sampler);
    // dist was the first build's until the last one published: never the
    // stopped build's, nor a half-written one.
    const last = bytes();
    expect([...seen].every(b => b === first || b === last)).toBe(true);
    expect(last).not.toBe(first);
    const holds = (text) => { const walk = (d) => readdirSync(d, { withFileTypes: true }).some(e => e.isDirectory() ? walk(join(d, e.name)) : readFileSync(join(d, e.name)).includes(text)); return walk(dist); };
    expect(holds('qz-four')).toBe(true);
    // A value-only edit (its batch, then its verifying build, held at its
    // end) and another while that build runs: the second's batch applies
    // at once, the build is set aside, and the next carries the value.
    // (The reader's build is a cargo build on the shared target/, queued
    // behind its lock while another build holds it: a build's bound.)
    await until(() => count(/design reader ready/g) >= 3, 300_000);
    const supersededBefore = count(/edit → a build superseded/g);
    writeFileSync(join(dir, 'app.tamber'), design('#202020'));
    await until(() => count(/edit → batch ready/g) >= 1, 60_000);
    await new Promise(ok => setTimeout(ok, 500));
    const held = Date.now();
    writeFileSync(join(dir, 'app.tamber'), design('#303030'));
    // Its batch from the design read, not a build's, before the held
    // build could end.
    await until(() => count(/edit → batch ready .*no build/g) >= 2, 60_000);
    expect(Date.now() - held).toBeLessThan(3000);
    // Served at once (the pages' snapshot), before the held build ends.
    expect(JSON.stringify(await (await fetch(`${base}/__dev/design`)).json())).toContain('#303030');
    await until(() => count(/edit → a build superseded/g) > supersededBefore, 60_000);
    await until(() => said().includes('edit → checked'), 120_000);
    const served = await (await fetch(`${base}/__dev/design`)).json();
    expect(JSON.stringify(served)).toContain('#303030');
    // And during a full build (app.json's, which leaves the design's shape):
    // the batch applies at once, and the build after serves it.
    // Past the age an edit stops a build at, with the design reader built
    // again after it (any edit but a design's builds it), and the build
    // still running (held at its end). The reader's build is a cargo build
    // on the shared target/, queued behind its lock while another build
    // holds it: back after the full build has ended, the step is staged again.
    let batches, settled, asideBefore;
    for (let tries = 0; ; tries++) {
      if (tries === 5) throw new Error(`the design reader was not built again while a full build ran, in 5 tries: ${said().slice(-2000)}`);
      batches = count(/edit → batch ready .*no build/g); settled = count(/edit → plan ready|edit → checked/g);
      asideBefore = count(/edit → a build superseded/g);
      const readers = count(/design reader ready/g);
      writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: `Stopped again ${tries}`, app: { id: 'com.exact.stopped', name: 'Stopped' } }));
      await new Promise(ok => setTimeout(ok, 2200));
      await until(() => count(/design reader ready/g) > readers, 300_000);
      if (count(/edit → plan ready|edit → checked/g) === settled) break;
    }
    const full = Date.now();
    writeFileSync(join(dir, 'app.tamber'), design('#404040'));
    await until(() => count(/edit → batch ready .*no build/g) > batches, 60_000);
    expect(Date.now() - full).toBeLessThan(3000);
    await until(() => count(/edit → plan ready|edit → checked/g) > settled, 300_000);
    expect(count(/edit → a build superseded/g)).toBe(asideBefore + 1);
    expect(JSON.stringify(await (await fetch(`${base}/__dev/design`)).json())).toContain('#404040');
  } finally {
    await loop?.stop().catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  }
}, 1_200_000);

test('the JS loop: a value-only design edit while the design reader builds waits for it, then is a batch, no full build', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-reader-wait-'));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Reader wait', app: { id: 'com.exact.readerwait', name: 'Reader wait' } }));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    column background-color="surface.card" min-height=40\n      text "qz-wait"\n');
  const design = (light) => `tamber readerwait\n\ntheme\n  surface.card = light-dark(${light}, #16181c)\n`;
  writeFileSync(join(dir, 'app.tamber'), design('#f1f3f5'));
  let loop = null;
  try {
    // The reader's first build is taken 3 minutes after cargo ends (a reader building again after any other edit, on
    // a loaded machine): the app's first build is ready well before it.
    loop = await devLoop(dir, { EXACT_DEV_READER_HOLD_MS: '180000' });
    const { said } = loop;
    const count = (re) => (said().match(re) ?? []).length;
    const until = async (pass, ms) => { const end = Date.now() + ms; while (!pass()) { if (Date.now() > end) throw new Error(`not seen in: ${said().slice(-2000)}`); await new Promise(ok => setTimeout(ok, 100)); } };
    await until(() => said().includes('plan ready'), 900_000);
    if (said().includes('design reader ready')) throw new Error(`the reader was ready before the first build; the hold is too short: ${said().slice(-1500)}`);
    writeFileSync(join(dir, 'app.tamber'), design('#e0f7e9'));
    await until(() => said().includes('edit → waiting for the design reader'), 60_000);
    // Nothing builds while it waits.
    expect(count(/edit → building/g)).toBe(0);
    await until(() => said().includes('design reader ready'), 400_000);
    await until(() => count(/edit → batch ready/g) >= 1, 120_000);
    // The edit's batch came after the reader, then the one build that checks it, and no other.
    expect(said().indexOf('design reader ready')).toBeLessThan(said().indexOf('edit → batch ready'));
    await until(() => count(/edit → checked/g) >= 1, 300_000);
    expect(count(/edit → building \(checking the batch\)/g)).toBe(1);
    expect(count(/edit → building/g)).toBe(1);
    expect(count(/edit → plan ready/g)).toBe(0);
    expect(JSON.stringify(await (await fetch(`${loop.base}/__dev/design`)).json())).toContain('e0f7e9');
  } finally {
    await loop?.stop().catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  }
}, 1_200_000);

test("the JS loop: an edit to theme-fixture's `when` token builds with setMode and its pins, and a refused one says the design's refusal", async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-axes-'));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Axes', app: { id: 'com.exact.axes', name: 'Axes' } }));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  action dim\n    setMode("dark-style", "dim")\n  view\n    column background-color="surface.page" min-height=40\n      button press=dim\n        text "dim"\n      column -exact-mode="dark-style=black" background-color="surface.page"\n        text "pinned"\n');
  const design = readFileSync(resolve(ROOT, 'apps/theme-fixture/app.tamber'), 'utf8');
  const page = '  surface.page = light-dark(#ffffff, #000000)\n    when dark-style=dim: light-dark(#ffffff, #15202b)\n';
  expect(design).toContain(page);
  writeFileSync(join(dir, 'app.tamber'), design);
  let loop = null;
  try {
    loop = await devLoop(dir);
    const { said, base } = loop;
    const until = async (pass, ms) => { const end = Date.now() + ms; while (!pass()) { if (Date.now() > end) throw new Error(`not seen in: ${said().slice(-2000)}`); await new Promise(ok => setTimeout(ok, 100)); } };
    const count = (re) => (said().match(re) ?? []).length;
    await until(() => said().includes('plan ready') && said().includes('design reader ready'), 900_000);
    // The `when` line's dark side: its batch, then its build, which checks
    // setMode and the pin against the design's axes.
    const checks = count(/edit → checked|edit → batch ready .*built in|edit → plan ready/g);
    writeFileSync(join(dir, 'app.tamber'), design.replace(page, page.replace('#15202b', '#16212c')));
    await until(() => count(/edit → checked|edit → batch ready .*built in|edit → plan ready/g) > checks || said().includes('build failed'), 300_000);
    expect(said()).not.toContain('build failed');
    expect(JSON.stringify(await (await fetch(`${base}/__dev/design`)).json())).toContain('#16212c');
    // The default's light side alone, which `under scheme=dark` refuses:
    // the build says so, and not that the design declares no axes.
    writeFileSync(join(dir, 'app.tamber'), design.replace(page, page.replace('#ffffff, #000000', '#fefefe, #000000')));
    await until(() => said().includes('build failed') && said().includes('[tamber-'), 300_000);
    expect(said()).toContain('tamber-under-light');
    expect(said()).not.toContain('declares no axes');
  } finally {
    await loop?.stop().catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  }
}, 1_200_000);

test('the JS loop: two loops on two apps under one target/ build at once, each into its own stage', async () => {
  const app = (name) => {
    const dir = mkdtempSync(join(tmpdir(), `exact-stage-${name}-`));
    writeFileSync(join(dir, 'app.json'), JSON.stringify({ name, app: { id: `com.exact.${name}`, name } }));
    writeFileSync(join(dir, 'app.contract'), `component App\n  view\n    text "${name}"\n`);
    writeFileSync(join(dir, 'app.tamber'), `tamber ${name}\n\ntheme\n  ink = #000000\n`);
    return dir;
  };
  const dirs = [app('stagea'), app('stageb')];
  let loops = [];
  try {
    // The first's end held 8 s: the second's whole build lands inside it (one stage between them was emptied under
    // the first, which then built nothing, or the other's files).
    // One target/ for both, named (an app outside a workspace otherwise gets <its dir>/target).
    const target = process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target');
    loops = await Promise.all([devLoop(dirs[0], { EXACT_DEV_BUILD_HOLD_MS: '8000', CARGO_TARGET_DIR: target }), devLoop(dirs[1], { CARGO_TARGET_DIR: target })]);
    for (const [i, name] of ['stagea', 'stageb'].entries()) {
      expect(loops[i].said()).not.toContain('does not build');
      expect(JSON.parse(readFileSync(join(loops[i].dist, '.exact-build.json'), 'utf8')).app.id).toBe(`com.exact.${name}`);
    }
  } finally {
    await Promise.all(loops.map(l => l.stop()));
    for (const dir of dirs) rmSync(dir, { recursive: true, force: true });
  }
}, 1_200_000);
