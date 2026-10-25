import { test, expect, beforeAll, afterAll } from 'bun:test';
import { resolve } from 'node:path';
import { readFileSync } from 'node:fs';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { glueDeclarations } from './glue-fixture.mjs';
import { standIns } from './stand-ins.mjs';

// A keydown an input method processed is composing (UI Events, legacy `keyCode` 229): WebKit sends the Enter that
// commits a composition after `compositionend`, `isComposing` false and `keyCode` 229, and web code skips it with
// `e.isComposing || e.keyCode === 229`. Each engine's own KeyboardEvent, synthesized, reaches the JS target's `key`
// and `submit` and both wasm glues' key wire.
// rt.js beside the modules it imports: the real ones it binds with, stand-ins for the rest (as form-chrome.test.mjs).
const webJs = resolve(import.meta.dir, '../../web-js');
const REAL = ['rt.js', 'held.js', 'font-family.js', 'observers.js', 'grid.js', 'overlay.js', 'roster.js', 'router.js', 'route-pattern.js', 'schedule.js', 'budget.js'];
const STUBS = standIns(REAL);
const chords = ['../glue.js', '../document-glue.js'].map(file => `(() => { ${glueDeclarations(readFileSync(new URL(file, import.meta.url), 'utf8'), ['keyChord'])}; return keyChord; })()`);
let server;
beforeAll(() => {
  server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/') return new Response(`<!doctype html><input id="field"><script>window.chords = [${chords.join(',')}];</script>`, { headers: { 'Content-Type': 'text/html' } });
    if (path === '/favicon.ico') return new Response(null, { status: 204 });
    const name = path.slice(1), source = REAL.includes(name) ? readFileSync(resolve(webJs, name)) : STUBS.get(name);
    return source == null ? new Response(null, { status: 404 }) : new Response(source, { headers: { 'Content-Type': 'text/javascript' } });
  } });
});
afterAll(() => server?.stop(true));

for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a keyCode-229 keydown is composing`, async () => {
  const browser = await suiteBrowser(engine);
  try {
    const page = await browser.newPage();
    await page.goto(server.url.href);
    const heard = await page.evaluate(async () => {
      const rt = await import('/rt.js'), field = document.getElementById('field'), heard = { key: [], submits: 0, wire: [] };
      rt.on(field, 'key', (_, record) => heard.key.push(record[7]), rt.onKey);
      rt.on(field, 'submit', () => heard.submits++, rt.onSubmit);
      field.focus();
      const cases = [
        { key: 'Enter', code: 'Enter', keyCode: 13 }, // a plain Enter
        { key: 'Enter', code: 'Enter', keyCode: 229 }, // WebKit's committing Enter
        { key: 'Process', code: 'KeyA', keyCode: 229 }, // a key an input method takes, as Chromium on Windows names it
        { key: 'Enter', code: 'Enter', keyCode: 229, isComposing: true }, // Chromium's and Gecko's candidate Enter
      ];
      for (const init of cases) {
        const ev = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
        if (ev.keyCode !== init.keyCode) throw new Error(`this engine's KeyboardEvent drops keyCode ${init.keyCode}`);
        heard.wire.push(window.chords.map(chord => chord(ev).split('\n')[3]));
        field.dispatchEvent(ev);
        await new Promise(r => setTimeout(r, 20)); // onSubmit's own setTimeout runs the submit
      }
      return heard;
    });
    expect(heard).toEqual({
      key: [false, true, true, true],
      submits: 1,
      wire: [['false', 'false'], ['true', 'true'], ['true', 'true'], ['true', 'true']],
    });
  } finally { await browser.close(); }
}, 30000);
