// A page is served from the build it booted (generations.mjs; theme2's ruling on the build-swap pair): a page's
// files all come from one build while another replaces it, old builds are kept while their pages may ask and reaped
// as the ruling keeps them, and a generation path is checked strictly.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildTreeFile, replaceBuild } from './serve.mjs';
import { GENERATION, generationOf, generationPath, leaseGeneration, leasedGenerations, reapGenerations, servedFile } from './generations.mjs';

/** A JS-target build naming `k` in every file: its document (base `/`), its module, a lazy chunk named by `k` as a
 * hashed chunk is, its plan; and its receipt. */
function build(dir, k) {
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'index.html'), `<!doctype html><html><head><base href="/"><script type="module" src="./app.js"></script></head><body>build ${k}</body></html>`);
  writeFileSync(join(dir, 'app.js'), `build ${k}`);
  writeFileSync(join(dir, `chunk-${k}.js`), `build ${k}`);
  writeFileSync(join(dir, 'app.plan'), `build ${k}`);
  writeFileSync(join(dir, '.exact-build.json'), JSON.stringify({ exactBuild: 1, target: 'js', files: [{ name: 'app.js', sha256: String(k) }] }));
  return dir;
}
const workspace = () => { const root = mkdtempSync(join(tmpdir(), 'exact-generations-')); return { root, dist: join(root, 'dist'), stage: (k) => build(join(root, `stage-${k}`), k) }; };
const text = (r) => (r.body == null ? null : Buffer.from(r.body).toString());
const baseOf = (html) => /<base href="([^"]+)">/.exec(html)?.[1];

test('a page booted on one build gets that build\'s files after another lands; the path by name mixes them (the negative control)', async () => {
  const { root, dist, stage } = workspace();
  try {
    replaceBuild(stage(1), dist);
    // The page: its document names its generation in its base.
    const page = await servedFile(dist, '/', 'text/html');
    expect([page.status, text(page)]).toEqual([200, expect.stringContaining('build 1')]);
    const base = baseOf(text(page));
    expect(base).toBe(`/.exact/gen/${generationOf(dist)}/`);
    replaceBuild(stage(2), dist);
    // Build 2 in place: the page's module, chunk and plan are still build 1's, from the kept tree.
    for (const file of ['app.js', 'chunk-1.js', 'app.plan']) expect(text(await servedFile(dist, base + file))).toBe('build 1');
    // The path by name, as servers read it before: the module is build 2's now, and the chunk gone (the mix).
    expect(readFileSync(buildTreeFile(dist, '/app.js').path, 'utf8')).toBe('build 2');
    expect(buildTreeFile(dist, '/chunk-1.js')).toBe(null);
    // A new page boots build 2.
    expect(baseOf(text(await servedFile(dist, '/', 'text/html')))).toBe(`/.exact/gen/${generationOf(dist)}/`);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('a page\'s files all come from one build while 100 builds replace it', async () => {
  const { root, dist, stage } = workspace();
  try {
    replaceBuild(stage(0), dist);
    let k = 0, done = false, mixed = 0, pages = 0;
    // Each build waits its turn in a stage of its own, made before the swap (as the builds' are).
    const writer = (async () => { while (k < 100) { const s = stage(++k); replaceBuild(s, dist); await new Promise((ok) => setTimeout(ok, 1)); } done = true; })();
    while (!done) {
      const page = await servedFile(dist, '/', 'text/html');
      const booted = /build (\d+)/.exec(text(page))[1], base = baseOf(text(page));
      const files = await Promise.all(['app.js', `chunk-${booted}.js`, 'app.plan'].map((f) => servedFile(dist, base + f)));
      // A page whose build was reaped meanwhile is told so (410), never served another build's file.
      for (const f of files) if (f.status !== 410 && text(f) !== `build ${booted}`) mixed++;
      pages++;
      await new Promise((ok) => setTimeout(ok, 0)); // (the writer's turn: these reads resolve without one)
    }
    await writer;
    expect(pages).toBeGreaterThan(10);
    expect(mixed).toBe(0);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('builds are kept by generation and reaped as the ruling keeps them: the newest 3, the leased, the young, within the cap', async () => {
  const { root, dist, stage } = workspace();
  try {
    const ids = [];
    for (let k = 1; k <= 6; k++) { replaceBuild(stage(k), dist); ids.push(generationOf(dist)); }
    // Builds 1 to 5 were replaced; all are under 15 minutes old, so all are kept.
    const kept = () => readdirSync(`${dist}.gen`).filter((n) => GENERATION.test(n)).sort();
    expect(kept()).toEqual(ids.slice(0, 5).sort());
    // Aged past 15 minutes: only the newest 3 stay, and a leased one, and nothing a dead process leased.
    const old = new Date(Date.now() - 20 * 60_000);
    for (const id of ids.slice(0, 5)) utimesSync(join(`${dist}.gen`, id), old, old);
    const release = leaseGeneration(dist, ids[0]);
    const dead = spawnSync(process.execPath, ['-e', '0']).pid ?? 999999;
    mkdirSync(join(`${dist}.leases`, ids[1]), { recursive: true });
    writeFileSync(join(`${dist}.leases`, ids[1], String(dead)), 'a start long gone');
    expect([...leasedGenerations(dist)]).toEqual([ids[0]]);
    // (The newest by when each was put aside: all aged alike, so by name order the three kept are any three; give
    // them distinct ages.)
    for (const [i, id] of ids.slice(0, 5).entries()) { const at = new Date(Date.now() - (30 - i) * 60_000); utimesSync(join(`${dist}.gen`, id), at, at); }
    const reaped = reapGenerations(dist);
    expect(kept()).toEqual([ids[0], ids[2], ids[3], ids[4]].sort());
    expect(reaped).toEqual([ids[1]]);
    // A reaped page is told its build is gone (410); an id that names nothing here is not found (404).
    expect((await servedFile(dist, `/.exact/gen/${ids[1]}/app.js`)).status).toBe(410);
    expect((await servedFile(dist, `/.exact/gen/${'0'.repeat(32)}/app.js`)).status).toBe(404);
    // The lease ended: reaped at the next pass. The cap: the oldest unleased first.
    release();
    expect(reapGenerations(dist)).toEqual([ids[0]]);
    const one = readdirSync(join(`${dist}.gen`, ids[4])).reduce((n, f) => n + statSync(join(`${dist}.gen`, ids[4], f)).size, 0);
    expect(reapGenerations(dist, { newest: 3, ageMs: 15 * 60_000, bytes: one })).toEqual([ids[2], ids[3]]);
    const again = leaseGeneration(dist, ids[4]);
    expect(reapGenerations(dist, { newest: 0, ageMs: 0, bytes: 0 })).toEqual([]);
    again();
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('a generation path is checked strictly: its id\'s form, no dot segment or encoded slash, nothing read outside', async () => {
  const { root, dist, stage } = workspace();
  try {
    replaceBuild(stage(1), dist);
    const id = generationOf(dist);
    writeFileSync(join(root, 'secret'), 'outside');
    for (const path of [`/.exact/gen/${id}/../../secret`, `/.exact/gen/${id}/%2e%2e/secret`, `/.exact/gen/${id}/a%2fb`, `/.exact/gen/${id.slice(1)}/app.js`,
      `/.exact/gen/${id.toUpperCase()}/app.js`, `/.exact/gen/${id}`, `/.exact/gen/../dist/app.js`]) {
      const r = await servedFile(dist, path);
      expect([path, r.status]).toEqual([path, 400]);
    }
    expect(generationPath(`/.exact/gen/${id}/app.js`)).toEqual({ id, path: '/app.js' });
    expect(generationPath('/app.js')).toBe(null);
    // A path the build has no file for is an app location: its document, never a file outside it.
    const outside = await servedFile(dist, `/.exact/gen/${id}/secret`, 'text/html');
    expect([outside.route, text(outside).includes('outside')]).toEqual(['/index.html', false]);
    expect(text(await servedFile(dist, `/.exact/gen/${id}/app.js`))).toBe('build 1');
    // Under another base (a server that serves the build under a path): the same, under that path.
    expect(generationPath(`/tool/.exact/gen/${id}/app.js`, '/tool/')).toEqual({ id, path: '/app.js' });
  } finally { rmSync(root, { recursive: true, force: true }); }
});
