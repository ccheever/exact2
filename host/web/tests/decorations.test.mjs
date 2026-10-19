// @ref LLP 1114.007 — a plain textarea's paint-only decorations and `rangeFrame`.
import { afterAll, test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import * as playwright from 'playwright-core';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { browserMissing } from '../../../scripts/agent-playwright.mjs';
import { need } from '../../../scripts/coverage.mjs';
import { buildEditor } from '../../web-js/module.mjs';
import { edit, resolve as reconcile } from '../decorations.js';

const WEB = resolve(import.meta.dir, '..');

// The kernel's table (kernel/src/text/decorations_tests.rs): the same
// replacement, in UTF-16 units, never splitting a surrogate pair.
test('the replacement between two strings matches the kernel', () => {
  for (const [before, after, expected] of [
    ['abc', 'abc', [3, 3, 3]], ['', 'abc', [0, 0, 3]], ['abc', '', [0, 3, 0]], ['abc', 'xabc', [0, 0, 1]],
    ['abc', 'abxc', [2, 2, 3]], ['abc', 'abcx', [3, 3, 4]], ['abcdef', 'abef', [2, 4, 2]], ['abcdef', 'abXYZef', [2, 4, 5]],
    ['a😀z', 'a😎z', [1, 3, 3]], ['a😀z', '😀z', [0, 1, 0]], ['a😀z', 'a😀😀z', [3, 3, 5]], ['aaa', 'a', [1, 3, 1]],
    ['🈀', '😀', [0, 2, 2]],
  ]) expect([before, after, edit(before, after)]).toEqual([before, after, { start: expected[0], oldEnd: expected[1], newEnd: expected[2] }]);
});

test('spans resolve as the kernel resolves them', () => {
  const record = { value: '😀@alice end', spans: [{ name: 'mention', start: 2, end: 8 }, { name: 'selected', start: 3, end: 6 }] };
  for (const [current, expected] of [
    ['x😀@alice end', [[3, 9], [4, 7]]], ['😀@alice end!', [[2, 8], [3, 6]]],
    ['😀@alXice end', [[2, 9], [3, 7]]], ['😀@a end', [[2, 4], [3, 4]]],
  ]) {
    const applied = reconcile(record, current);
    expect(applied.provisional).toBe(true);
    expect(applied.spans.map(s => [s.start, s.end])).toEqual(expected);
  }
  expect(reconcile(record, record.value)).toEqual({ provisional: false, spans: record.spans });
  const clamped = { value: '@alice', spans: [{ name: 'm', start: 0, end: 999 }, { name: 'e', start: 3, end: 3 }, { name: 'r', start: 5, end: 1 }, { name: 'o', start: 99, end: 100 }] };
  expect(reconcile(clamped, '@alice').spans).toEqual([{ name: 'm', start: 0, end: 6 }]);
  expect(reconcile(clamped, 'x@alice').spans).toEqual([{ name: 'm', start: 0, end: 7 }]);
  expect(reconcile(clamped, '').spans).toEqual([]);
  // As the runner's codec: offsets truncate toward zero, never below; a non-finite one refuses the record.
  expect(reconcile({ value: 'abc', spans: [{ name: 'n', start: -1.2, end: 2.9 }] }, 'abc').spans).toEqual([{ name: 'n', start: 0, end: 2 }]);
  expect(reconcile({ value: 'abc', spans: [{ name: 'n', start: 0, end: 1 }, { name: 'x', start: null, end: 2 }] }, 'abc').spans).toEqual([]);
});

const engines = ['chromium', 'firefox', 'webkit'];
const missing = Object.fromEntries(await Promise.all(engines.map(async engine => [engine, await browserMissing(engine)])));
const wasm = missing.chromium ? null : readFileSync(await buildEditor());
const browsers = new Map();
async function browser(engine) {
  if (!browsers.has(engine)) browsers.set(engine, playwright[engine].launch({ headless: true, ...(engine === 'chromium' ? { executablePath: chromium().executable } : {}) }));
  return browsers.get(engine);
}
afterAll(async () => { await Promise.all([...browsers.values()].map(async value => (await value).close())); }, 30000);

const value = 'hi @alice.bsky.social see https://bsky.app #art';
const record = { value, spans: [{ name: 'mention', start: 3, end: 21 }, { name: 'link', start: 26, end: 42 }, { name: 'tag', start: 43, end: 47 }] };
const table = { mention: 'color:rgb(16,131,254);', link: 'color:rgb(16,131,254);text-decoration-line:underline;', tag: 'color:rgb(16,131,254);' };

async function page(engine, drive, markdown = false, text = value) {
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/markup-editor.wasm') return new Response(wasm, { headers: { 'content-type': 'application/wasm' } });
    if (path.endsWith('.js')) return new Response(Bun.file(resolve(WEB, path.slice(1))), { headers: { 'content-type': 'text/javascript' } });
    if (path !== '/') return new Response(null, { status: 404 });
    return new Response(`<!doctype html><meta charset="utf-8"><div id="root"><textarea id="field" ${markdown ? 'markup="markdown"' : ''} style="width:400px;height:120px;font:16px/1.5 sans-serif"></textarea></div>
      <script type="module">
        globalThis.exact = {}; window.logs = [];
        let field = document.getElementById('field'); field.value = ${JSON.stringify(text)};
        await import('./geometry-glue.js'); const geometry = exact.geometry(document.getElementById('root'));
        window.range = (start, end) => { const out = new Float64Array(4), bits = geometry.range(field, start, end, out); return bits ? [...out] : null; };
        if (${markdown}) {
          await import('./markup-editor.js');
          field = (await exact.installMarkupEditor)(field, { live: () => true, select() {}, replace(el) { field = el; } });
        } else {
          field.setAttribute('data-decorations', ${JSON.stringify(JSON.stringify(record))});
          field.setAttribute('data-decoration-styles', ${JSON.stringify(JSON.stringify(table))});
          const { install } = await import('./decorations.js'); install(field, line => logs.push(line));
        }
        window.field = () => field; window.ready = true;
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  const context = await (await browser(engine)).newContext();
  try {
    const p = await context.newPage(), errors = [];
    p.on('pageerror', error => errors.push(error.message));
    await p.goto(server.url.href);
    await p.waitForFunction(() => window.ready);
    await drive(p);
    expect(errors).toEqual([]);
  } finally { await context.close(); server.stop(true); }
}

for (const engine of engines) {
  const check = need(engine === 'chromium' ? 'chrome' : engine, missing[engine]);
  check(`${engine}: decorations paint over the textarea, or nothing where value ranges are missing`, async () => {
    await page(engine, async p => {
      const paints = await p.evaluate(() => typeof HTMLTextAreaElement.prototype.createValueRange === 'function');
      if (engine === 'chromium') expect(paints).toBe(true);
      const facts = await p.evaluate(() => { const el = field(); return { tag: el.localName, value: el.value, editable: el.isContentEditable, highlights: [...CSS.highlights.keys()].length, applied: el.exactDecorations.applied }; });
      expect(facts).toMatchObject({ tag: 'textarea', value: facts.value, editable: false, highlights: paints ? 3 : 0 });
      expect(facts.applied).toMatchObject({ provisional: false, spans: record.spans });
      // Typing before a span leaves it provisional and shifted until the app's next record.
      await p.locator('#field').focus(); await p.evaluate(() => field().setSelectionRange(0, 0)); await p.keyboard.type('!');
      expect(await p.evaluate(() => field().exactDecorations.applied)).toMatchObject({ provisional: true, spans: [{ start: 4, end: 22 }, { start: 27, end: 43 }, { start: 44, end: 48 }] });
      await p.evaluate(() => { const el = field(), next = JSON.parse(el.getAttribute('data-decorations')); next.value = el.value; next.spans = next.spans.map(s => ({ ...s, start: s.start + 1, end: s.end + 1 })); el.setAttribute('data-decorations', JSON.stringify(next)); });
      await p.waitForFunction(() => field().exactDecorations.applied.provisional === false);
      // The browser's undo is the person's typing alone.
      await p.keyboard.press('ControlOrMeta+z');
      expect(await p.evaluate(() => field().value)).toBe(value);
      // Nothing is repainted while text is composed.
      const held = await p.evaluate(async () => {
        const el = field(), before = [...CSS.highlights.keys()].join();
        el.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
        el.setAttribute('data-decorations', JSON.stringify({ value: el.value, spans: [] })); await new Promise(r => setTimeout(r));
        const during = [...CSS.highlights.keys()].join();
        el.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }));
        return [before === during, [...CSS.highlights.keys()].length];
      });
      expect(held).toEqual([true, 0]);
      expect(await p.evaluate(() => logs)).toEqual([]);
    });
  }, 30000);

  check(`${engine}: rangeFrame reads the textarea's own layout`, async () => {
    await page(engine, async p => {
      const paints = await p.evaluate(() => typeof HTMLTextAreaElement.prototype.createValueRange === 'function');
      const facts = await p.evaluate(() => {
        const el = field(), box = el.getBoundingClientRect(), caret = range(3, 3), word = range(3, 21), truncated = range(3.9, 21.2);
        const refused = [range(-1, 3), range(5, 3), range(0, el.value.length + 1), range(NaN, 3)];
        el.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true })); const composing = range(3, 3);
        el.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }));
        el.style.display = 'none'; const hidden = range(3, 3); el.style.display = '';
        return { box: [box.x, box.y, box.width, box.height], caret, word, truncated, refused, composing, hidden };
      });
      expect(facts.refused).toEqual([null, null, null, null]);
      expect([facts.composing, facts.hidden]).toEqual([null, null]);
      if (!paints) { expect([facts.caret, facts.word]).toEqual([null, null]); return; }
      const [x, y, width, height] = facts.caret;
      expect(width).toBe(0); expect(height).toBeGreaterThan(10);
      expect(x).toBeGreaterThan(facts.box[0]); expect(y).toBeGreaterThanOrEqual(facts.box[1]);
      expect(facts.word[2]).toBeGreaterThan(50); expect(facts.word[0]).toBe(x);
      expect(facts.truncated).toEqual(facts.word);
    });
  }, 30000);
}

need('chrome', missing.chromium)('chromium: rangeFrame on a Markdown field reads source offsets', async () => {
  await page('chromium', async p => {
    const facts = await p.evaluate(() => [range(3, 3), range(3, 21), range(0, field().value.length + 1)]);
    expect(facts[0][2]).toBe(0); expect(facts[0][3]).toBeGreaterThan(10);
    expect(facts[1][2]).toBeGreaterThan(50); expect(facts[2]).toBe(null);
  }, true);
  // Hidden syntax adds no box: `**bold**` measures as its visible word.
  await page('chromium', async p => {
    const [marked, word, hidden, inMarker, wordEnd, newline] = await p.evaluate(() => [range(11, 19), range(13, 17), range(11, 13), range(18, 18), range(17, 17), range(10, 11)]);
    expect(marked[2]).toBeGreaterThan(20);
    expect(marked).toEqual(word);
    expect(hidden).toBe(null);
    // A caret inside hidden syntax is at the nearer visible edge; a line break spans both lines.
    expect(inMarker).toEqual(wordEnd);
    expect(newline[3]).toBeGreaterThan(word[3] * 1.5);
  }, true, 'first line\n**bold** word');
}, 30000);
