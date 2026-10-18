// A build put in place (serve.mjs `replaceBuild`) while another process
// serves the tree: where the system exchanges two directories in one step
// a reader never finds no build, nor a file gone under it; elsewhere (the
// two renames, EXACT_DIRECTORY_EXCHANGE=0) the swap still lands whole. A
// swap that fails leaves the build as it was.
import { expect, test } from 'bun:test';
import { spawn } from 'node:child_process';
import { lstatSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const SERVE = resolve(import.meta.dir, 'serve.mjs');

/** `swaps` builds put in place, one every `gap` ms, while a reader process
 * asks for the page; what it found. */
async function stress(env, swaps = 100, gap = 20) {
  const dir = mkdtempSync(join(tmpdir(), 'exact-replace-build-')), dist = join(dir, 'dist');
  mkdirSync(dist); writeFileSync(join(dist, 'index.html'), '<p>0');
  const writer = `import { mkdirSync, writeFileSync } from 'node:fs'; import { replaceBuild } from ${JSON.stringify(SERVE)};
    for (let i = 1; i <= ${swaps}; i++) { const stage = ${JSON.stringify(dir)} + '/stage-' + i; mkdirSync(stage);
      for (let k = 0; k < 20; k++) writeFileSync(stage + '/f' + k + '.txt', 'x'.repeat(2000) + i);
      writeFileSync(stage + '/index.html', '<p>' + i); replaceBuild(stage, ${JSON.stringify(dist)}); await new Promise(ok => setTimeout(ok, ${gap})); }`;
  const reader = `import { readFileSync } from 'node:fs'; import { buildTreeFile } from ${JSON.stringify(SERVE)};
    let ok = 0, miss = 0, err = 0; const stop = () => { console.log(JSON.stringify({ ok, miss, err })); process.exit(0); };
    process.on('SIGTERM', stop); setTimeout(stop, 60000);
    for (;;) { const f = buildTreeFile(${JSON.stringify(dist)}, '/index.html', 'text/html'); if (!f) miss++; else { try { readFileSync(f.path); ok++; } catch { err++; } }
      if ((ok + miss + err) % 500 === 0) await new Promise(setImmediate); }`;
  const run = (code) => spawn(process.execPath, ['-e', code], { env: { ...process.env, ...env }, stdio: ['ignore', 'pipe', 'inherit'] });
  const r = run(reader); let out = ''; r.stdout.on('data', d => { out += d; });
  await new Promise(ok => setTimeout(ok, 300));
  const w = run(writer); await new Promise(ok => w.on('exit', ok));
  r.kill('SIGTERM'); await new Promise(ok => r.on('exit', ok));
  const final = readFileSync(join(dist, 'index.html'), 'utf8');
  rmSync(dir, { recursive: true, force: true });
  return { ...JSON.parse(out.trim().split('\n').pop()), final };
}

const { directoryExchange } = await import(SERVE);

test.skipIf(!directoryExchange())('exchanged in one step, a reader never finds no build nor a file gone', async () => {
  const got = await stress({});
  expect(got.final).toBe('<p>100');
  expect(got.ok).toBeGreaterThan(1000);
  expect({ miss: got.miss, err: got.err }).toEqual({ miss: 0, err: 0 });
}, 120_000);

test('by two renames (no exchange) the swap still lands whole, but a reader can find no build or a file gone', async () => {
  // As fast as it can, to meet the moment between the renames (a run that
  // never met it is run again, three times at most).
  let got;
  for (let run = 0; run < 3 && !(got?.miss + got?.err); run++) {
    got = await stress({ EXACT_DIRECTORY_EXCHANGE: '0' }, 400, 0);
    expect(got.final).toBe('<p>400');
    expect(got.ok).toBeGreaterThan(1000);
  }
  expect(got.miss + got.err).toBeGreaterThan(0);
}, 300_000);

test('a swap that fails leaves the build as it was', async () => {
  const { replaceBuild } = await import(SERVE);
  const dir = mkdtempSync(join(tmpdir(), 'exact-replace-build-')), dist = join(dir, 'dist');
  mkdirSync(dist); writeFileSync(join(dist, 'index.html'), '<p>kept');
  expect(() => replaceBuild(join(dir, 'no-stage'), dist)).toThrow();
  expect(readFileSync(join(dist, 'index.html'), 'utf8')).toBe('<p>kept');
  // A stage that is a link is refused, not put in place as a link.
  mkdirSync(join(dir, 'elsewhere')); writeFileSync(join(dir, 'elsewhere', 'index.html'), '<p>linked');
  symlinkSync(join(dir, 'elsewhere'), join(dir, 'linked'));
  expect(() => replaceBuild(join(dir, 'linked'), dist)).toThrow();
  expect(lstatSync(dist).isDirectory()).toBe(true);
  expect(readFileSync(join(dist, 'index.html'), 'utf8')).toBe('<p>kept');
  // A swap killed between its renames left only `.previous`: no stage keeps it.
  renameSync(dist, `${dist}.previous`);
  expect(() => replaceBuild(join(dir, 'no-stage'), dist)).toThrow();
  expect(readFileSync(join(`${dist}.previous`, 'index.html'), 'utf8')).toBe('<p>kept');
  rmSync(dir, { recursive: true, force: true });
});
