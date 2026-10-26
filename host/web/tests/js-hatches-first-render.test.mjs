// The JS target's hatch words and page module, known before the first render (pass 1 #6 of the design tool's hands-on
// passes, theme2's 18:11 ruling): a word app.json gives only other platforms is shown and never called from the very
// first render, and an app with no page module (`modules/web/`) asks for no module artifact, so its page has no
// `modules/index.js` 404. An app with a page module still loads it and calls its word.
import { expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';

const ROOT = resolve(import.meta.dir, '../../..');

/** A tiny app with one hatched node, its word for `platforms`, a page module that calls it when `module`; built for
 * the JS target, served, opened: its journal's hatch lines and the requests for a module artifact. */
async function run(name, platforms, module) {
  const app = mkdtempSync(join(tmpdir(), `exact-hatch-first-${name}-`)), dist = join(app, 'dist');
  writeFileSync(join(app, 'app.json'), JSON.stringify({ name, app: { id: `com.exact.hatch-first-${name}`, name }, hatches: { tick: platforms } }));
  writeFileSync(join(app, 'app.contract'), 'component App\n  view\n    column padding=16\n      text "hatched" hatch="tick" testId="hatched"\n');
  if (module) {
    mkdirSync(join(app, 'modules/web'), { recursive: true });
    writeFileSync(join(app, 'modules/web/index.js'), 'export const hatches = { tick: { element() {} } };\nexport function element(h) { globalThis.ticked = (globalThis.ticked ?? 0) + 1; }\n');
  }
  const built = spawnSync(process.execPath, [resolve(ROOT, 'host/web-js/build.mjs'), name, '--out', dist, '--render', 'none', '--dev'], { cwd: ROOT, env: { ...process.env, EXACT_APP_DIR: app }, encoding: 'utf8' });
  expect(built.status, built.stdout + built.stderr).toBe(0);
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: req => { const path = new URL(req.url).pathname; const file = Bun.file(join(dist, path.endsWith('/') ? `${path}index.html` : path)); return file.size ? new Response(file) : new Response('Not found.', { status: 404 }); } });
  const browser = await suiteBrowser('chromium');
  try {
    const page = await browser.newPage(), artifacts = [];
    page.on('request', r => { if (/\/modules\/index\.js/.test(r.url())) artifacts.push(r.url()); });
    await page.goto(`http://127.0.0.1:${server.port}/`, { waitUntil: 'load' });
    await page.waitForFunction(() => globalThis.exact?.ready === true);
    await page.waitForTimeout(1500);
    const hatchLines = await page.evaluate(() => (globalThis.exact.journal ?? []).filter(l => / hatch /.test(l)));
    return { artifacts, hatchLines, ticked: await page.evaluate(() => globalThis.ticked ?? 0) };
  } finally { await browser.close(); server.stop(true); }
}

test('a word only another platform handles, with no page module: shown from the first render, no artifact asked for', async () => {
  const r = await run('elsewhere', ['ios'], false);
  expect(r.artifacts).toEqual([]);
  expect(r.hatchLines.some(l => /element tick: not handled on this platform/.test(l)), r.hatchLines.join('\n')).toBe(true);
  expect(r.hatchLines.some(l => /did not load|no page module/.test(l)), r.hatchLines.join('\n')).toBe(false);
}, 600_000);

test('a word the web handles, with a page module: the artifact is loaded and the word called', async () => {
  const r = await run('here', ['web'], true);
  expect(r.artifacts.length).toBeGreaterThan(0);
  expect(r.ticked, r.hatchLines.join('\n')).toBeGreaterThan(0);
}, 600_000);
