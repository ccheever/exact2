// Run by exact-web-js's Rust test: real emitted plans and Rust DocTree pages,
// the JS renderer before adoption, a fresh client, and both adopted pages.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdirSync, readdirSync, copyFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { rolldown } from 'rolldown';
import { chromium as playwright } from 'playwright-core';
import { chromium } from '../../../../scripts/agent-launch.mjs';
import { renderer } from '../../../web-js/render.mjs';

const work = process.argv[2], root = resolve(import.meta.dir, '../../../..');
const { executable, unavailable } = chromium();
if (unavailable) throw new Error(unavailable);
const names = JSON.parse(readFileSync(resolve(work, 'cases.json')));
const pages = new Map();
const readIsolation = () => Object.fromEntries([...document.querySelectorAll('[data-testid]')].map(e => [e.dataset.testid, getComputedStyle(e).isolation]));
const withoutScripts = html => html.replace(/<script\b[^>]*>[\s\S]*?<\/script>/g, '');
const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
  const path = new URL(req.url).pathname;
  if (path === '/favicon.ico') return new Response(null, { status: 204 });
  if (pages.has(path)) return new Response(pages.get(path), { headers: { 'content-type': 'text/html' } });
  return new Response(Bun.file(resolve(work, '.' + path)));
} });
const browser = await playwright.launch({ executablePath: executable, headless: true, args: ['--no-sandbox'] });
const failures = [];
try {
  for (let i = 0; i < names.length; i++) {
    try {
      const dir = resolve(work, String(i)), gen = resolve(dir, '.gen');
      mkdirSync(gen, { recursive: true });
      // The same runtime files the app build copies, with JS target modules
      // taking precedence over the host's glue.
      for (const folder of ['host/web', 'host/web-js']) for (const file of readdirSync(resolve(root, folder)).filter(f => f.endsWith('.js'))) copyFileSync(resolve(root, folder, file), resolve(gen, file));
      for (const f of ['app.js', 'names.js', 'paint.js']) copyFileSync(resolve(dir, f), resolve(gen, f));
      writeFileSync(resolve(gen, 'entry.js'), `import app from './app.js'; import { journal } from './rt.js'; app(); globalThis.journal=journal; globalThis.ready=true;`);
      writeFileSync(resolve(gen, 'server-entry.js'), `import app from './app.js'; globalThis.__start=async()=>{app();return {activate:'eager',policy:'build'}};globalThis.__render=async()=>({root:document.rootHTML(),time:0,answers:'[]',pending:[]});`);
      for (const [entry, file] of [['entry.js', 'app.js'], ['server-entry.js', '.gen/server.js']]) {
        const bundle = await rolldown({ input: resolve(gen, entry), logLevel: 'silent', external: ['./draw.js'] });
        await bundle.write({ file: resolve(dir, file), format: 'iife', codeSplitting: false });
        await bundle.close();
      }
      const css = readFileSync(resolve(dir, 'app.css'), 'utf8');
      const shell = `<!doctype html><html><head><title>Paint</title><meta name="viewport" content="width=device-width">\n<style>${css}</style></head><body><div id="exact-root"></div><script type="module" src="./app.js"></script></body></html>`;
      writeFileSync(resolve(dir, 'shell.html'), shell);
      const rendered = (await renderer(dir)('/')).html;
      const rust = readFileSync(resolve(dir, 'rust.html'), 'utf8');
      const page = await browser.newPage();
      const sliced = names[i].startsWith('sliced keyed');
      if (sliced) await page.addInitScript(() => {
        // Deterministic budgets, with adoption tasks driven one at a time.
        let now = 0;
        performance.now = () => ++now;
        globalThis.adoptionSlices = [];
        globalThis.scheduler = { postTask: f => { adoptionSlices.push(f); return Promise.resolve(); } };
      });
      const check = async (html, adopt = false) => {
        pages.set(`/${i}/index.html`, html);
        await page.goto(`${server.url}${i}/index.html`);
        if (adopt) await page.waitForFunction(() => globalThis.ready);
        const result = await page.evaluate(readIsolation);
        if (adopt && sliced && (await page.evaluate(() => journal)).some(s => s.includes('adopted the document'))) {
          assert.deepEqual(result, expected, 'waiting rows retain server isolation');
          assert.ok(await page.evaluate(() => adoptionSlices.length > 0), 'the list exceeded the initial budget');
          assert.ok(await page.evaluate(() => {
            const row = document.querySelector('[data-testid="row-200"]');
            const waiting = row.$n === undefined;
            row.dispatchEvent(new Event('pointerdown', { bubbles: true }));
            return waiting && row.$n !== undefined;
          }), 'input adopts a waiting row');
          assert.deepEqual(await page.evaluate(readIsolation), expected, 'event-triggered adoption flushes paint');
          for (let slice = 0; await page.evaluate(() => adoptionSlices.length); slice++) {
            assert.ok(slice < 200, 'adoption completes');
            await page.evaluate(() => adoptionSlices.shift()());
            assert.deepEqual(await page.evaluate(readIsolation), expected, `paint after adoption slice ${slice}`);
          }
        }
        return result;
      };
      const expected = await check(withoutScripts(shell).replace('<div id="exact-root"></div>', `<div id="exact-root">${rust}</div>`));
      assert.deepEqual(await check(withoutScripts(rendered)), expected, 'JS server before adoption');
      assert.deepEqual(await check(shell, true), expected, 'fresh JS client');
      for (const step of JSON.parse(readFileSync(resolve(dir, 'steps.json')))) {
        await page.evaluate(id => document.querySelector(`[data-testid="${id}"]`).click(), step.action);
        assert.deepEqual(await page.evaluate(readIsolation), step.isolation, `after ${step.action}`);
        assert.deepEqual(await page.evaluate(() => Object.fromEntries([...document.querySelectorAll('[data-testid]')].map(e => [e.dataset.testid, getComputedStyle(e).zIndex]))), step.zIndex, 'authored z-index remains exact and clamped');
      }
      // Keep the server checkpoint, but load the fixture entry directly so
      // adoption is deterministic and independent of capture scheduling.
      const adopted = rendered.replace(/<script>[^]*?<\/script>/, '') + '<script src="./app.js"></script>';
      assert.deepEqual(await check(adopted, true), expected, 'adopted JS document');
      assert.ok((await page.evaluate(() => journal)).some(s => s.includes('adopted the document')));
      const fromRust = adopted.replace(/<div id="exact-root">[^]*?<\/div>(?=<script type="application\/vnd.exact.checkpoint")/, `<div id="exact-root">${rust}</div>`);
      assert.notEqual(fromRust, adopted, 'Rust document substituted');
      assert.deepEqual(await check(fromRust, true), expected, 'adopted Rust document');
      const stale = fromRust.replace(/<div([^>]*data-testid[^>]*)>/g, (tag, attrs) => attrs.includes('style="') ? tag.replace(/style="([^"]*)"/, (_, css) => `style="${css}isolation:isolate;"`) : `<div${attrs} style="isolation:isolate">`);
      assert.deepEqual(await check(stale, true), expected, 'adoption drops stale isolation even without candidacy');
      assert.ok((await page.evaluate(() => journal)).some(s => s.includes('adopted the document')));
      await page.close();
    } catch (e) { failures.push(`${names[i]}: ${e.stack}`); }
  }
} finally { await browser.close(); server.stop(true); }
assert.deepEqual(failures, []);
console.log(`${names.length} cases agree: Rust document, JS server, JS client and both adoptions`);
