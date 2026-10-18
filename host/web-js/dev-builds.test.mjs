// The JS dev loop's builds (host/web-js/dev.mjs): a build that an input
// moved during is set aside, never published (not dist, which a fresh page
// loads, nor the design or a reload); an edit stops a young build; a value
// batch during a verifying build applies at once, and the build after
// carries it. EXACT_DEV_BUILD_HOLD_MS holds each build's end, as a slow one.
import { expect, test } from 'bun:test';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const ROOT = resolve(new URL('../..', import.meta.url).pathname);
const sha = text => createHash('sha256').update(text).digest('hex');

/** The JS dev loop on the app at `dir`, its dist in a temporary directory,
 * `env` beside the process's own. */
async function devLoop(dir, env = {}) {
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-js-builds-'));
  const dev = spawn(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', dir, '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist, ...env }, stdio: ['ignore', 'pipe', 'pipe'] });
  const stop = async () => {
    if (dev.exitCode === null && dev.signalCode === null) {
      const exit = new Promise(ok => dev.once('exit', ok));
      dev.kill('SIGTERM');
      if (!(await Promise.race([exit.then(() => true), new Promise(ok => setTimeout(() => ok(false), 30_000).unref())]))) dev.kill('SIGKILL');
    }
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
    const first = bytes(), seen = new Set();
    const sampler = setInterval(() => { try { seen.add(bytes()); } catch { /* replaced meanwhile */ } }, 10);
    // An edit and another at once: the first's build is stopped young.
    writeFileSync(join(dir, 'app.contract'), view('qz-two'));
    await new Promise(ok => setTimeout(ok, 30));
    writeFileSync(join(dir, 'app.contract'), view('qz-three'));
    await until(() => said().includes('(aborted)'), 60_000);
    await until(() => said().includes('edit → plan ready'), 300_000);
    clearInterval(sampler);
    // dist was the first build's until the last one published: never the
    // stopped build's, nor a half-written one.
    const last = bytes();
    expect([...seen].every(b => b === first || b === last)).toBe(true);
    expect(last).not.toBe(first);
    // A value-only edit (its batch, then its verifying build, held at its
    // end) and another while that build runs: the second's batch applies
    // at once, the build is set aside, and the next carries the value.
    await until(() => count(/design reader ready/g) >= 3, 120_000);
    const supersededBefore = count(/edit → a build superseded/g);
    writeFileSync(join(dir, 'app.tamber'), design('#202020'));
    await until(() => count(/edit → batch ready/g) >= 1, 60_000);
    await new Promise(ok => setTimeout(ok, 500));
    writeFileSync(join(dir, 'app.tamber'), design('#303030'));
    await until(() => count(/edit → batch ready/g) >= 2, 60_000);
    await until(() => count(/edit → a build superseded/g) > supersededBefore, 60_000);
    await until(() => said().includes('edit → checked'), 120_000);
    const served = await (await fetch(`${base}/__dev/design`)).json();
    expect(JSON.stringify(served)).toContain('#303030');
  } finally {
    await loop?.stop().catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  }
}, 1_200_000);
