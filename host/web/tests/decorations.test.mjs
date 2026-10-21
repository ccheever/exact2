// @ref LLP 1114.007 — a plain textarea's paint-only decorations and `rangeFrame`.
import { afterAll, test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium, suiteBrowser } from '../../../scripts/agent-launch.mjs';
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
  if (!browsers.has(engine)) browsers.set(engine, suiteBrowser(engine));
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
        window.geometry = geometry;
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

// A collapsed range on an empty line, or in an empty field, is where the
// caret is painted (docs/contract-grammar.md, theme2's 18:42 ruling), as
// Chrome's createValueRange gives it on a line with text: `a\n\nb`'s empty
// middle line sits one line below `a`'s start and one above `b`'s, at the
// same left edge and with the same text height; `ab\n`'s last empty line
// below `ab`; an empty field's caret at its content box's start.
need('chrome', missing.chromium)('chromium: rangeFrame on an empty line and an empty field is the painted caret', async () => {
  await page('chromium', async p => {
    const [first, middle, last] = await p.evaluate(() => [range(0, 0), range(2, 2), range(3, 3)]);
    expect(middle[0]).toBeCloseTo(first[0], 1);
    expect(middle[0]).toBeCloseTo(last[0], 1);
    expect(middle[1] - first[1]).toBeCloseTo(last[1] - middle[1], 1);
    expect(middle[1]).toBeGreaterThan(first[1]);
    expect([middle[2], middle[3]]).toEqual([0, first[3]]);
  }, false, 'a\n\nb');
  await page('chromium', async p => {
    const [start, end, after] = await p.evaluate(() => [range(0, 0), range(2, 2), range(3, 3)]);
    // One line below `ab` (the page's 16px/1.5: 24), at the line's start.
    expect(after[1] - start[1]).toBeCloseTo(24, 1);
    expect(after[0]).toBeCloseTo(start[0], 1);
    expect([after[2], after[3]]).toEqual([0, end[3]]);
  }, false, 'ab\n');
  await page('chromium', async p => {
    const [caret, box, typed] = await p.evaluate(async () => {
      const el = field(), r = el.getBoundingClientRect(), s = getComputedStyle(el), empty = range(0, 0);
      el.value = 'x'; const typed = range(0, 0);
      return [empty, [r.x + el.clientLeft + parseFloat(s.paddingLeft), r.y + el.clientTop + parseFloat(s.paddingTop)], typed];
    });
    expect(caret[0]).toBeCloseTo(box[0], 1);
    expect([caret[1], caret[3]]).toEqual([typed[1], typed[3]]);
    expect(caret[2]).toBe(0);
  }, false, '');
}, 30000);

// The twin that finds the painted caret agrees with Chrome's own caret on
// lines with text, under the field's styles a twin must copy: padding and
// borders, a scroll, `wrap=off` with a long line, letter spacing, centred
// text, a vertical writing mode, a transform (scale and rotation) and zoom,
// and a transformed ancestor.
need('chrome', missing.chromium)('chromium: the caret twin is Chrome\'s caret where Chrome has one', async () => {
  const text = 'first line of the field that is long enough to wrap in a box four hundred wide, or not at all\nsecond\n\nfourth line here\n' + 'more\n'.repeat(12) + 'end';
  await page('chromium', async p => {
    const cases = [
      '', 'padding:13px 7px;border:5px solid red', 'letter-spacing:2px;word-spacing:5px', 'text-align:center',
      'white-space:pre', 'writing-mode:vertical-rl;height:300px', 'transform:scale(1.5) rotate(10deg)', 'zoom:1.25', 'scroll',
      'wrap', 'ancestor', 'white-space:normal', 'white-space:nowrap', 'transition:all .2s', 'scroll-behavior:smooth;--scroll:1',
      'position:fixed;left:30px;top:20px', 'trailing', 'white-space:normal;--indent', 'max-width:400px;padding:10px;border:2px solid;text-align:center',
    ];
    const results = await p.evaluate(({ cases, text }) => cases.map(c => {
      const el = field(), base = el.getAttribute('style');
      el.value = text;
      if (c === 'scroll') el.scrollTop = 60;
      else if (c === 'trailing') { el.value = text + '\n'; el.scrollTop = el.scrollHeight; }
      else if (c === 'wrap') el.setAttribute('wrap', 'off');
      else if (c === 'ancestor') el.parentElement.style.transform = 'translate(11px, 7px) scale(0.8)';
      else el.setAttribute('style', `${base};${c}`);
      if (c.includes('--indent')) el.value = '              ' + text.replace('\nsecond', '\n            second');
      if (c.includes('--scroll')) { el.style.scrollBehavior = 'auto'; el.scrollTop = 60; el.style.scrollBehavior = 'smooth'; }
      const diffs = [];
      for (const k of [0, 5, 40, 61, 70, 80, 95, 101, 110, 130, text.length]) {
        if (el.value[k - 1] === '\n' && (el.value[k] === '\n' || k === el.value.length)) continue;
        // After a space a wrap may fall: either line's end is that caret.
        if (el.value[k - 1] === ' ') continue;
        const chrome = el.createValueRange(k, k).getBoundingClientRect(), twin = geometry.caretAt(el, k);
        const d = Math.max(Math.abs(chrome.x - twin.x), Math.abs(chrome.y - twin.y), Math.abs(chrome.height - twin.height));
        if (d > 0.5) diffs.push(`${c || 'plain'} @${k}: chrome ${[chrome.x, chrome.y, chrome.height].map(n => n.toFixed(2))} twin ${[twin.x, twin.y, twin.height].map(n => n.toFixed(2))}`);
      }
      el.setAttribute('style', base); el.removeAttribute('wrap'); el.scrollTop = 0; el.parentElement.style.transform = '';
      return diffs;
    }).flat(), { cases, text });
    expect(results).toEqual([]);
  });
}, 30000);

// Measuring through the twin leaves nothing behind and touches nothing
// (theme2's sign-off condition): the twin is gone after each read, was
// hidden from sight and from assistive technology and took no focus while
// it stood, and the page's scroll, the field's scroll and its selection
// and focus are as they were.
need('chrome', missing.chromium)('chromium: the caret twin leaves the page, the field and its selection as they were', async () => {
  await page('chromium', async p => {
    const facts = await p.evaluate(() => {
      const el = field();
      document.body.style.height = '3000px';
      scrollTo(0, 40);
      el.value = 'a\n\nb\n' + 'line\n'.repeat(20);
      el.focus();
      el.setSelectionRange(3, 5, 'backward');
      el.scrollTop = 30;
      const seen = [];
      const watch = new MutationObserver(batch => batch.forEach(m => m.addedNodes.forEach(n => {
        if (n.nodeType === 1) seen.push([n.getAttribute('aria-hidden'), n.inert, n.style.visibility, n.tabIndex]);
      })));
      watch.observe(document.body, { childList: true, subtree: true });
      const before = [scrollX, scrollY, el.scrollTop, el.selectionStart, el.selectionEnd, el.selectionDirection, document.activeElement === el];
      const answer = range(2, 2);
      const after = [scrollX, scrollY, el.scrollTop, el.selectionStart, el.selectionEnd, el.selectionDirection, document.activeElement === el];
      const records = watch.takeRecords();
      records.forEach(m => m.addedNodes.forEach(n => { if (n.nodeType === 1) seen.push([n.getAttribute('aria-hidden'), n.inert, n.style.visibility, n.tabIndex]); }));
      watch.disconnect();
      return { answer, before, after, seen, left: document.querySelectorAll('exact-caret-twin').length };
    });
    expect(facts.answer).not.toBe(null);
    expect(facts.after).toEqual(facts.before);
    expect(facts.left).toBe(0);
    expect(facts.seen).toEqual([['true', true, 'hidden', -1]]);
  });
}, 30000);
