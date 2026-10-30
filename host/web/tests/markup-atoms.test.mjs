// @ref LLP 1045.000 §4, D4, D5, D7 — inline atoms in the web's Markdown
// editor, driven by keys and the pointer in Chrome, Firefox and WebKit (the
// go/no-go of LLP 1045 §1), and in the reader.
import { afterAll, test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { buildEditor } from '../../web-js/module.mjs';
import { browserMissing } from '../../../scripts/agent-playwright.mjs';
import { need } from '../../../scripts/coverage.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const engines = ['chromium', 'firefox', 'webkit'];
const missing = Object.fromEntries(await Promise.all(engines.map(async e => [e, await browserMissing(e)])));
const engineTest = Object.fromEntries(engines.map(e => [e, need(e === 'chromium' ? 'chrome' : e, missing[e])]));
const wasm = engines.some(e => !missing[e]) ? readFileSync(await buildEditor()) : null;

const KINDS = JSON.stringify([{ name: 'citation', link: 'cite:' }, { name: 'mention', prefix: '@' }]);
const CITE = '[Quote](cite:1)';
const faces = list => JSON.stringify(list.map(([source, label, title = '', color = '', backgroundColor = '', direction = '', icon = '']) => ({ source, label, icon, title, color, backgroundColor, direction })));
// An icon the web draws: a black square.
const ICON = 'data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect width="1" height="1"/></svg>');
// Whether a pill's icon is on its right: its middle past the pill's, on its
// label's line, where the page's `img { display: block }` would put it above.
const iconOnTheRight = box => box.evaluate(b => {
  const i = b.querySelector('img').getBoundingClientRect(), r = b.getBoundingClientRect(), label = document.createRange();
  label.selectNodeContents(b.lastChild);
  const t = label.getBoundingClientRect();
  if (i.bottom <= t.top + 1) throw new Error(`the icon is above its label: ${JSON.stringify([i, t])}`);
  return i.left + i.width / 2 > r.left + r.width / 2;
});
const attr = s => s.replace(/&/g, '&amp;').replace(/"/g, '&quot;');

async function editor(engine, source, drive, { width = 500, faceList = [], styles = '{}' } = {}) {
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
    const path = new URL(request.url).pathname;
    if (['/markup-editor.js', '/navigation.js', '/atoms.js', '/geometry-glue.js'].includes(path)) return new Response(Bun.file(resolve(WEB, path.slice(1))), { headers: { 'content-type': 'text/javascript' } });
    if (path === '/markup-editor.wasm') return new Response(wasm, { headers: { 'content-type': 'application/wasm' } });
    return new Response(`<!doctype html><meta charset="utf-8"><style>body{font:16px/1.4 system-ui} img, video, svg { display: block; }</style>
      <textarea id="editor" markup="markdown" data-atoms="${attr(KINDS)}" data-atom-faces="${attr(faces(faceList))}" data-atom-styles="${attr(styles)}" style="width:${width}px;height:200px"></textarea>
      <script type="module">
        globalThis.exact = {};
        let field = document.getElementById('editor');
        field.value = ${JSON.stringify(source)};
        window.events = [];
        await import('./markup-editor.js');
        const install = await exact.installMarkupEditor;
        field = install(field, { live: el => el.isConnected, select() {}, replace(el) {
          el.addEventListener('atompress', e => events.push(['atompress', ...e.detail]));
          el.addEventListener('input', e => { events.push(['input', el.value]); el.value = el.value; });
        } });
        window.field = () => field;
        window.ready = true;
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  const context = await (await browser(engine)).newContext();
  try {
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(`http://127.0.0.1:${server.port}/`);
    await page.waitForFunction(() => window.ready);
    await drive(page, page.locator('#editor'));
    expect(errors).toEqual([]);
  } finally { await context.close(); server.stop(true); }
}

const launched = new Map();
function browser(engine) {
  if (!launched.has(engine)) launched.set(engine, suiteBrowser(engine));
  return launched.get(engine);
}
afterAll(async () => { for (const b of launched.values()) await (await b.catch(() => null))?.close(); }, 60000);

const settle = page => page.evaluate(() => new Promise(ok => requestAnimationFrame(() => setTimeout(ok))));
const state = field => field.evaluate(el => [el.value, el.selectionStart, el.selectionEnd]);
const island = field => field.locator('.exact-atom').first();

for (const engine of engines) {
  engineTest[engine](`${engine}: an atom is one pill whose source is the value, crossed by an arrow in one step`, async () => {
    const source = `a ${CITE} b`, start = 2, end = 2 + CITE.length;
    await editor(engine, source, async (page, field) => {
      const box = island(field);
      expect(await box.textContent()).toBe('A quote');
      expect(await box.getAttribute('role')).toBe('button');
      expect(await box.getAttribute('aria-label')).toBe('A quote');
      expect(await box.getAttribute('title')).toBe('The quote');
      expect(await box.getAttribute('contenteditable')).toBe('false');
      expect(await field.evaluate(el => el.value)).toBe(source);
      // The pill keeps the line's height: one line of text, as without the atom.
      const [line, plain] = await field.evaluate(el => [el.firstChild.getBoundingClientRect().height, parseFloat(getComputedStyle(el).lineHeight)]);
      expect(Math.abs(line - plain)).toBeLessThan(1);
      await field.focus();
      // A caret inside goes to the end; ← and → cross it in one step.
      await field.evaluate((el, at) => el.setSelectionRange(at, at), start + 3);
      expect(await state(field)).toEqual([source, end, end]);
      await page.keyboard.press('ArrowLeft'); await settle(page);
      expect(await state(field)).toEqual([source, start, start]);
      await page.keyboard.press('ArrowRight'); await settle(page);
      expect(await state(field)).toEqual([source, end, end]);
      // ⇧← takes it whole.
      await page.keyboard.press('Shift+ArrowLeft'); await settle(page);
      expect(await state(field)).toEqual([source, start, end]);
    }, { faceList: [[CITE, 'A quote', 'The quote']] });
  }, 60000);

  engineTest[engine](`${engine}: Backspace and Delete remove an atom whole, undo brings it back, typing lands outside`, async () => {
    const source = `a ${CITE} b`, start = 2, end = 2 + CITE.length;
    await editor(engine, source, async (page, field) => {
      await field.focus();
      await field.evaluate((el, at) => el.setSelectionRange(at, at), end);
      await page.keyboard.press('Backspace'); await settle(page);
      expect(await state(field)).toEqual(['a  b', start, start]);
      await page.keyboard.press('ControlOrMeta+Z'); await settle(page);
      expect(await field.evaluate(el => el.value)).toBe(source);
      expect(await field.locator('.exact-atom').count()).toBe(1);
      await field.evaluate((el, at) => el.setSelectionRange(at, at), start);
      await page.keyboard.press('Delete'); await settle(page);
      expect(await field.evaluate(el => el.value)).toBe('a  b');
      await page.keyboard.press('ControlOrMeta+Z'); await settle(page);
      await field.evaluate((el, at) => el.setSelectionRange(at, at), end);
      await page.keyboard.type('!'); await settle(page);
      expect(await state(field)).toEqual([`a ${CITE}! b`, end + 1, end + 1]);
      await field.evaluate((el, at) => el.setSelectionRange(at, at), start);
      await page.keyboard.type('x'); await settle(page);
      expect(await state(field)).toEqual([`a x${CITE}! b`, start + 1, start + 1]);
      expect(await field.locator('.exact-atom').count()).toBe(1);
      // Typing over a range that holds the atom replaces it.
      await field.evaluate((el, [a, b]) => el.setSelectionRange(a, b), [start, end + 2]);
      await page.keyboard.type('z'); await settle(page);
      expect(await field.evaluate(el => el.value)).toBe('a z b');
    });
  }, 60000);

  engineTest[engine](`${engine}: a press selects the atom, then is atompress; the selection reports it`, async () => {
    const source = `see ${CITE} now`, start = 4, end = 4 + CITE.length;
    await editor(engine, source, async (page, field) => {
      await island(field).click({ modifiers: ['Shift'] }); await settle(page);
      expect(await state(field)).toEqual([source, start, end]);
      expect(await page.evaluate(() => events.filter(e => e[0] === 'atompress'))).toEqual([['atompress', 'citation', CITE, 'cite:1', start, end, true, false, false, false]]);
      // Copy takes the atom's source (where a script's ClipboardEvent carries its DataTransfer: not Firefox's).
      const copied = await field.evaluate(el => { const data = new DataTransfer(), e = new ClipboardEvent('copy', { clipboardData: data, bubbles: true, cancelable: true }); el.dispatchEvent(e); return e.clipboardData ? data.getData('text/plain') : null; });
      if (copied !== null && engine !== 'firefox') expect(copied).toBe(CITE);
    });
  }, 60000);

  engineTest[engine](`${engine}: a gated token becomes an atom when its face arrives, and a face update redraws it`, async () => {
    const source = 'hi @src/main.rs now';
    await editor(engine, source, async (page, field) => {
      expect(await field.locator('.exact-atom').count()).toBe(0);
      await field.evaluate(el => { el.setAttribute('data-atom-faces', JSON.stringify([{ source: '@src/main.rs', label: 'main.rs', icon: '', title: '' }])); el.exactMarkup.sync(); });
      expect(await island(field).textContent()).toBe('main.rs');
      await field.evaluate(el => { el.setAttribute('data-atom-faces', JSON.stringify([{ source: '@src/main.rs', label: 'lib/main.rs', icon: '', title: '' }])); el.exactMarkup.sync(); });
      expect(await island(field).textContent()).toBe('lib/main.rs');
      // A face whose paint alone changes draws its pill again.
      await field.evaluate(el => { el.setAttribute('data-atom-faces', JSON.stringify([{ source: '@src/main.rs', label: 'lib/main.rs', icon: '', title: '', color: 'rgb(7, 8, 9)', backgroundColor: '' }])); el.exactMarkup.sync(); });
      expect(await island(field).evaluate(b => getComputedStyle(b).color)).toBe('rgb(7, 8, 9)');
      expect(await field.evaluate(el => el.value)).toBe(source);
      // rangeFrame over the atom is its pill.
      const [frame, pill] = await field.evaluate(el => [el.exactMarkup.rangeFrame(3, 15), el.querySelector('.exact-atom').getBoundingClientRect()].map(r => [r.x, r.y, r.width, r.height].map(Math.round)));
      expect(frame).toEqual(pill);
    });
  }, 60000);

  engineTest[engine](`${engine}: a face's paint is its pill's, over its kind's; one the browser does not take leaves the kind's`, async () => {
    const styles = JSON.stringify({ mention: 'color:rgb(1, 2, 3);background-color:rgb(4, 5, 6)' });
    const faceList = [['@x', 'X', '', 'light-dark(#d52c36, #ff6762)'], ['@y', 'Y', '', '', '#1a85d433'], ['@z', 'Z', '', 'no colour', 'red;display:none']];
    await editor(engine, 'a @x @y @z b', async (page, field) => {
      const paint = () => field.evaluate(el => [...el.querySelectorAll('.exact-atom')].map(b => [getComputedStyle(b).color, getComputedStyle(b).backgroundColor, getComputedStyle(b).display]));
      expect(await paint()).toEqual([
        ['rgb(213, 44, 54)', 'rgb(4, 5, 6)', 'inline-block'],
        ['rgb(1, 2, 3)', 'rgba(26, 133, 212, 0.2)', 'inline-block'],
        ['rgb(1, 2, 3)', 'rgb(4, 5, 6)', 'inline-block'],
      ]);
      // `light-dark()` takes the page's scheme, as the kind's does.
      await page.evaluate(() => { document.documentElement.style.colorScheme = 'dark'; });
      expect((await paint())[0][0]).toBe('rgb(255, 103, 98)');
    }, { faceList, styles });
  }, 60000);

  engineTest[engine](`${engine}: an island an edit removed without beforeinput removes its atom's source`, async () => {
    const source = `a ${CITE} b`;
    await editor(engine, source, async (page, field) => {
      await field.focus();
      await field.evaluate(el => { const box = el.querySelector('.exact-atom'), r = document.createRange(); r.setStartBefore(box); r.setEndAfter(box); getSelection().removeAllRanges(); getSelection().addRange(r); document.execCommand('delete'); });
      await settle(page);
      expect(await field.evaluate(el => el.value)).toBe('a  b');
    });
  }, 60000);

  engineTest[engine](`${engine}: a long label is cut at the line's width, and adjacent atoms are two pills`, async () => {
    const long = '@a-very-long-file-name-that-does-not-fit-in-a-narrow-field.rs';
    await editor(engine, `${long} [B](cite:2)[C](cite:3) x`, async (page, field) => {
      const [boxes, lineWidth] = await field.evaluate(el => [[...el.querySelectorAll('.exact-atom')].map(b => [b.scrollWidth > b.clientWidth, b.getBoundingClientRect().width]), el.firstChild.clientWidth]);
      expect(boxes.length).toBe(3);
      expect(boxes[0][0]).toBe(true);
      expect(boxes[0][1]).toBeLessThanOrEqual(lineWidth + 0.5);
      await field.focus();
      const end = long.length + 1 + '[B](cite:2)'.length;
      await field.evaluate((el, at) => el.setSelectionRange(at, at), end);
      await page.keyboard.press('Backspace'); await settle(page);
      expect(await field.evaluate(el => el.value)).toBe(`${long} [C](cite:3) x`);
    }, { width: 160, faceList: [[long, long]] });
  }, 60000);
}

for (const engine of engines) {
  engineTest[engine](`${engine}: ⌥⌫ takes an atom and the spaces to the caret`, async () => {
    const source = `a ${CITE}   b`, after = 2 + CITE.length + 3;
    await editor(engine, source, async (page, field) => {
      await field.focus();
      await field.evaluate((el, at) => el.setSelectionRange(at, at), after);
      await page.keyboard.press(process.platform === 'darwin' ? 'Alt+Backspace' : 'Control+Backspace'); await settle(page);
      expect(await state(field)).toEqual(['a b', 2, 2]);
    });
  }, 60000);

  // LLP 1045.000 D4: the reader's pill too follows its line's direction, its
  // icon first, unless its face pins one; a collapsed `rangeFrame` at a
  // pill's start is the edge its line's direction puts first, pinned or not (D7).
  engineTest[engine](`${engine}: in a right-to-left reader a pill's icon is on the right, unless its face pins left to right`, async () => {
    const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      const path = new URL(request.url).pathname;
      if (path.endsWith('.js')) return new Response(Bun.file(resolve(WEB, path.slice(1))), { headers: { 'content-type': 'text/javascript' } });
      // The host page's reset (index.html) makes an image a block.
      return new Response(`<!doctype html><meta charset="utf-8"><style>img, video, svg { display: block; }</style><p id="text" markup="markdown" data-exact-text style="direction:rtl"></p><script type="module">
        globalThis.exact = {};
        const { renderMarkup } = await import('./navigation.js');
        await import('./atoms.js');
        const icon = ${JSON.stringify(ICON)};
        renderMarkup(document.getElementById('text'), JSON.stringify([['שלום ', 1, 0, 0, ''], ['Quote', 1, 0, 72, '', 0, ['citation', 'button', icon, '', 'cite:1', '${CITE}', 5, 20, '', '', '']],
          [' ו ', 1, 0, 0, ''], ['Pinned', 1, 0, 72, '', 0, ['citation', 'button', icon, '', 'cite:2', '[P](cite:2)', 23, 34, '', '', 'ltr']]]));
        window.ready = true;
      </script>`, { headers: { 'content-type': 'text/html' } });
    } });
    const context = await (await browser(engine)).newContext();
    try {
      const page = await context.newPage();
      await page.goto(`http://127.0.0.1:${server.port}/`);
      await page.waitForFunction(() => window.ready && document.querySelector('.exact-atom img')?.complete);
      const [follows, pinned] = [page.locator('.exact-atom').first(), page.locator('.exact-atom').nth(1)];
      expect(await follows.evaluate(b => getComputedStyle(b).direction)).toBe('rtl');
      expect(await iconOnTheRight(follows)).toBe(true);
      expect(await pinned.evaluate(b => getComputedStyle(b).direction)).toBe('ltr');
      expect(await iconOnTheRight(pinned)).toBe(false);
      // Each pill's start is its right edge and its end its left, as the line runs.
      const edges = (box, s, e) => box.evaluate((b, [s, e]) => {
        const text = document.getElementById('text'), r = b.getBoundingClientRect();
        return [exact.atoms.rangeFrame(text, s, s).left, exact.atoms.rangeFrame(text, e, e).left, r.left, r.right].map(Math.round);
      }, [s, e]);
      for (const [box, s, e] of [[follows, 5, 20], [pinned, 23, 34]]) {
        const [start, end, left, right] = await edges(box, s, e);
        expect([start, end]).toEqual([right, left]);
      }
    } finally { await context.close(); server.stop(true); }
  }, 60000);

  engineTest[engine](`${engine}: the reader's atom is a button a press, Return or Space reports, and rangeFrame is its pill`, async () => {
    const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      const path = new URL(request.url).pathname;
      if (path.endsWith('.js')) return new Response(Bun.file(resolve(WEB, path.slice(1))), { headers: { 'content-type': 'text/javascript' } });
      return new Response(`<!doctype html><meta charset="utf-8"><div id="root"><p id="text" markup="markdown" data-exact-text data-atom-styles='{"citation":"color:rgb(1, 2, 3)"}'></p></div><script type="module">
        globalThis.exact = {};
        const { renderMarkup } = await import('./navigation.js');
        const text = document.getElementById('text');
        const json = JSON.stringify([['see ', 1, 0, 0, ''], ['A quote', 1, 700, 72, '', 0, ['citation', 'button', '', 'The quote', 'cite:1', '${CITE}', 4, 19, '', '']], [' and ', 1, 0, 0, ''], ['L', 1, 0, 72, '', 0, ['ref', 'link', '', '', 'cite:2', '[L](cite:2)', 24, 35, '#1a85d4', '']]]);
        renderMarkup(text, json);
        window.before = text.querySelectorAll('.exact-atom').length; // until atoms.js is here: the labels alone
        await import('./atoms.js'); renderMarkup(text, json);
        await import('./geometry-glue.js'); const geometry = exact.geometry(document.getElementById('root'));
        window.range = (a, b) => { const out = new Float64Array(4); return geometry.range(text, a, b, out) ? [...out].map(Math.round) : null; };
        window.events = []; text.addEventListener('atompress', e => events.push(e.detail));
        window.ready = true;
      </script>`, { headers: { 'content-type': 'text/html' } });
    } });
    const context = await (await browser(engine)).newContext();
    try {
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', e => errors.push(e.message));
      await page.goto(`http://127.0.0.1:${server.port}/`);
      await page.waitForFunction(() => window.ready || window.failed, null, { timeout: 15000 }).catch(() => {});
      expect(errors).toEqual([]);
      expect(await page.evaluate(() => window.before)).toBe(0);
      const box = page.locator('.exact-atom').first(), link = page.locator('.exact-atom').nth(1);
      expect(await box.getAttribute('role')).toBe('button');
      expect(await box.getAttribute('tabindex')).toBe('0');
      expect(await box.getAttribute('aria-description')).toBe('The quote');
      expect(await box.evaluate(b => [getComputedStyle(b).fontWeight, getComputedStyle(b).color])).toEqual(['700', 'rgb(1, 2, 3)']);
      expect(await link.evaluate(b => getComputedStyle(b).color)).toBe('rgb(26, 133, 212)'); // its face's paint
      await box.click();
      await box.focus(); await page.keyboard.press('Enter'); await page.keyboard.down(' ');
      const detail = ['citation', CITE, 'cite:1', 4, 19, false, false, false, false];
      expect(await page.evaluate(() => events.length)).toBe(2); // a button's Space waits for its key up
      await page.keyboard.up(' ');
      expect(await page.evaluate(() => events)).toEqual([detail, detail, detail]);
      // A link atom takes Return, not Space.
      await link.focus(); await page.keyboard.press(' '); await page.keyboard.press('Enter');
      expect(await page.evaluate(() => events.map(e => e[0]))).toEqual(['citation', 'citation', 'citation', 'ref']);
      const [atom] = await page.evaluate(() => exact.atoms.state(() => 'text'));
      expect([atom.id, atom.kind, atom.source, atom.start, atom.end, atom.label]).toEqual(['text', 'citation', CITE, 4, 19, 'A quote']);
      // rangeFrame over the atom's range is its pill; elsewhere in the text, none.
      const pill = await box.evaluate(b => { const r = b.getBoundingClientRect(); return [r.x, r.y, r.width, r.height].map(Math.round); });
      expect(await page.evaluate(() => window.range(4, 19))).toEqual(pill);
      expect(await page.evaluate(() => window.range(0, 2))).toBe(null);
      // A disabled text presses nothing.
      await page.evaluate(() => document.getElementById('root').setAttribute('inert', ''));
      await box.click({ force: true });
      expect(await page.evaluate(() => events.length)).toBe(4);
    } finally { await context.close(); server.stop(true); }
  }, 60000);
}

for (const engine of engines) {
  engineTest[engine](`${engine}: ⌘⇧C copies a selected atom's label, found in the whole source`, async () => {
    await editor(engine, 'hi @x now', async (page, field) => {
      await page.evaluate(() => { window.copied = []; Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: t => { copied.push(t); return Promise.resolve(); } } }); });
      await island(field).click(); await settle(page);
      await page.keyboard.press('ControlOrMeta+Shift+C'); await settle(page);
      expect(await page.evaluate(() => window.copied)).toEqual(['Ex']);
    }, { faceList: [['@x', 'Ex']] });
  }, 60000);

  engineTest[engine](`${engine}: a disabled field presses no atom; a read-only one presses without selecting`, async () => {
    await editor(engine, `a ${CITE} b`, async (page, field) => {
      await field.evaluate(el => { el.setAttribute('readonly', ''); el.exactMarkup.sync(); });
      await island(field).click(); await settle(page);
      expect(await page.evaluate(() => events.filter(e => e[0] === 'atompress').length)).toBe(1);
      expect(await field.evaluate(el => el.selectionEnd - el.selectionStart)).toBe(0);
      await field.evaluate(el => { el.removeAttribute('readonly'); el.setAttribute('disabled', ''); el.exactMarkup.sync(); });
      await island(field).click({ force: true }); await settle(page);
      expect(await page.evaluate(() => events.filter(e => e[0] === 'atompress').length)).toBe(1);
    });
  }, 60000);

  engineTest[engine](`${engine}: in a right-to-left line an atom's start is its right edge`, async () => {
    await editor(engine, `שלום ${CITE} עולם`, async (page, field) => {
      await field.evaluate(el => { el.style.direction = 'rtl'; });
      const [start, end, pill] = await field.evaluate((el, [a, b]) => [el.exactMarkup.rangeFrame(a, a), el.exactMarkup.rangeFrame(b, b), el.querySelector('.exact-atom').getBoundingClientRect()].map(r => [Math.round(r.left), Math.round(r.right)]), [5, 5 + CITE.length]);
      expect(start[0]).toBe(pill[1]);
      expect(end[0]).toBe(pill[0]);
    });
  }, 60000);

  // LLP 1045.000 D4: in a right-to-left line a pill lays out from the right,
  // its icon first, as the line does; a face's `direction` pins its own.
  engineTest[engine](`${engine}: in a right-to-left line a pill's icon is on the right, unless its face pins left to right`, async () => {
    for (const [direction, right] of [['', true], ['ltr', false], ['rtl', true]]) {
      await editor(engine, `שלום ${CITE} עולם`, async (page, field) => {
        await field.evaluate(el => { el.setAttribute('data-paragraph-dirs', 'rtl'); el.exactMarkup.sync(); });
        await page.evaluate(() => new Promise(ok => requestAnimationFrame(() => setTimeout(ok))));
        const box = island(field);
        expect(await box.evaluate(b => getComputedStyle(b).direction)).toBe(direction || 'rtl');
        expect(await iconOnTheRight(box)).toBe(right);
      }, { faceList: [[CITE, 'Quote', '', '', '', direction, ICON]] });
    }
  }, 60000);

  // @ref LLP 1045 D11 — a line `paragraphDirs` makes right to left is one for an atom's edges.
  engineTest[engine](`${engine}: in a line paragraphDirs makes right to left, an atom's start is its right edge`, async () => {
    await editor(engine, `שלום ${CITE} עולם`, async (page, field) => {
      await field.evaluate(el => { el.setAttribute('data-paragraph-dirs', 'rtl'); el.exactMarkup.sync(); });
      await page.evaluate(() => new Promise(ok => requestAnimationFrame(() => setTimeout(ok))));
      const [line, start, right] = await field.evaluate((el, a) => {
        const island = el.querySelector('.exact-atom');
        return [getComputedStyle(el.firstChild).direction, Math.round(el.exactMarkup.rangeFrame(a, a).left), Math.round(island.getBoundingClientRect().right)];
      }, 5);
      expect(line).toBe('rtl');
      expect(start).toBe(right);
    });
  }, 60000);

  // @ref LLP 1045 D11 — `auto` reads the letters a line draws as text, as Apple's MarkupEditor does.
  engineTest[engine](`${engine}: auto takes a line's first strong letter outside its syntax and atoms`, async () => {
    await editor(engine, '@src/a.ts اشرح هذا \n@x اكتب\n[1](https://example.test) مرحبا\n42', async (page, field) => {
      await field.evaluate(el => { el.setAttribute('data-paragraph-dirs', 'auto'); el.exactMarkup.sync(); });
      await page.evaluate(() => new Promise(ok => requestAnimationFrame(() => setTimeout(ok))));
      // The atom and the link's destination are left out; `@x` has no face, so it is text; 42 has no strong letter.
      expect(await field.evaluate(el => [...el.children].map(line => line.getAttribute('dir')))).toEqual(['rtl', 'ltr', 'rtl', null]);
    }, { faceList: [['@src/a.ts', 'a.ts']] });
  }, 60000);

  engineTest[engine](`${engine}: auto reads a footnote mark's label as text, wherever the caret is`, async () => {
    await editor(engine, '[^n] مرحبا\n\n[^note]: ملاحظة', async (page, field) => {
      await field.evaluate(el => { el.setAttribute('data-paragraph-dirs', 'auto'); el.exactMarkup.sync(); });
      await page.evaluate(() => new Promise(ok => requestAnimationFrame(() => setTimeout(ok))));
      const dirs = () => field.evaluate(el => [...el.children].map(line => line.getAttribute('dir')));
      expect(await dirs()).toEqual(['ltr', null, 'ltr']);
      // A caret on a footnote's line reveals its syntax; the answer stays.
      await field.focus();
      for (const at of [2, 20]) {
        await field.evaluate((el, at) => el.setSelectionRange(at, at), at); await settle(page);
        expect(await dirs()).toEqual(['ltr', null, 'ltr']);
      }
    });
  }, 60000);
}

for (const engine of engines) {
  engineTest[engine](`${engine}: an atom-only field, and an atom inside bold, edit as one`, async () => {
    await editor(engine, CITE, async (page, field) => {
      await field.focus();
      await field.evaluate((el, at) => el.setSelectionRange(at, at), CITE.length);
      await page.keyboard.type('x'); await settle(page);
      expect(await state(field)).toEqual([`${CITE}x`, CITE.length + 1, CITE.length + 1]);
      await field.evaluate(el => el.setSelectionRange(0, 0));
      await page.keyboard.type('y'); await settle(page);
      expect(await field.evaluate(el => el.value)).toBe(`y${CITE}x`);
      await field.evaluate((el, at) => el.setSelectionRange(at, at), CITE.length + 1);
      await page.keyboard.press('Backspace'); await settle(page);
      expect(await field.evaluate(el => el.value)).toBe('yx');
    });
    const bold = `a **${CITE}** b`, end = 4 + CITE.length;
    await editor(engine, bold, async (page, field) => {
      expect(await island(field).evaluate(b => getComputedStyle(b).fontWeight)).toBe('700');
      await field.focus();
      await field.evaluate((el, at) => el.setSelectionRange(at, at), end + 2);
      expect(await field.evaluate(el => el.selectionStart)).toBe(end);
      await page.keyboard.press('Backspace'); await settle(page);
      expect(await field.evaluate(el => el.value)).toBe('a  b');
    });
  }, 60000);

  engineTest[engine](`${engine}: a long label at the largest text size stays one line, cut with an ellipsis`, async () => {
    const long = '@a-very-long-file-name-that-does-not-fit-in-a-narrow-field.rs';
    await editor(engine, `${long} x`, async (page, field) => {
      await field.evaluate(el => { el.style.fontSize = '53px'; }); // iOS's largest accessibility body size
      await settle(page);
      const [lines, clipped, width, line] = await field.evaluate(el => { const b = el.querySelector('.exact-atom'), r = b.getBoundingClientRect(); return [Math.round(r.height / parseFloat(getComputedStyle(b).lineHeight || r.height)), b.scrollWidth > b.clientWidth, r.width, el.firstChild.clientWidth]; });
      expect(clipped).toBe(true);
      expect(width).toBeLessThanOrEqual(line + 0.5);
      expect(lines).toBeLessThanOrEqual(1);
    }, { width: 400, faceList: [[long, long]] });
  }, 60000);

  // LLP 1045.000 D2: on the JS target a face's colour is what this browser
  // takes (`shape.js`), as a bound colour row's is. A cascade keyword or a
  // substitution anywhere is none, but for a role in its web form, which is
  // the role (here a stand-in map's); so is a standard space's (LLP 1100 D1).
  engineTest[engine](`${engine}: a face's colour on the JS target is what the browser takes`, async () => {
    const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
      if (new URL(request.url).pathname === '/shape.js') return new Response(Bun.file(resolve(WEB, '../web-js/shape.js')), { headers: { 'content-type': 'text/javascript' } });
      return new Response(`<!doctype html><meta charset="utf-8"><script type="module">
        const journal = []; globalThis.exact = { journal, now: () => 0 };
        const { atomFacesRecord } = await import('./shape.js');
        const colours = ['color(display-p3 1 0 0)', 'oklch(70% 0.1 200)', 'var(--exact-separator)', 'var(--x)', 'env(safe-area-inset-top)', 'if(media(print): red)', 'color(--dci-p3 1 0 0)', 'notacolour', 'light-dark(currentcolor, #000)', 'inh\\\\65rit'];
        const roles = [['separator', '', false, false, '-exact-separator']], cm = c => c === '-exact-separator' ? 'var(--exact-separator, #c6c6c8ff)' : c;
        const out = JSON.parse(atomFacesRecord(colours.map(c => ['@a', '', '', '', c, '']), cm, () => 9, roles));
        window.result = { kept: out.map(f => f.color), journal };
      </script>`, { headers: { 'content-type': 'text/html' } });
    } });
    const context = await (await browser(engine)).newContext();
    try {
      const page = await context.newPage();
      await page.goto(`http://127.0.0.1:${server.port}/`);
      await page.waitForFunction(() => window.result);
      const { kept, journal } = await page.evaluate(() => window.result);
      expect(kept).toEqual(['color(display-p3 1 0 0)', 'oklch(70% 0.1 200)', 'var(--exact-separator, #c6c6c8ff)', '', '', '', '', '', '', '']);
      expect(journal).toEqual(['var(--x)', 'env(safe-area-inset-top)', 'if(media(print): red)', 'color(--dci-p3 1 0 0)', 'notacolour', 'light-dark(currentcolor, #000)', 'inh\\65rit']
        .map(c => `t=0 view 9: invalid atomFaces colour ${JSON.stringify(c)}; its kind's paint stands`));
    } finally { await context.close(); server.stop(true); }
  }, 60000);
}

// An input method's composition over a range holding an atom (Chromium's IME through CDP).
engineTest.chromium('chromium: a composition over a range holding an atom deletes it first, its own undo step', async () => {
  const source = `a ${CITE} b`;
  await editor('chromium', source, async (page, field) => {
    await field.focus();
    await field.evaluate((el, b) => el.setSelectionRange(1, b), 2 + CITE.length + 1);
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Input.imeSetComposition', { text: 'か', selectionStart: 1, selectionEnd: 1 });
    await cdp.send('Input.insertText', { text: 'か' }); await settle(page);
    expect(await field.evaluate(el => el.value)).toBe('aかb');
    await page.keyboard.press('ControlOrMeta+Z'); await settle(page);
    expect(await field.evaluate(el => el.value)).toBe('ab');
    await page.keyboard.press('ControlOrMeta+Z'); await settle(page);
    expect(await field.evaluate(el => el.value)).toBe(source);
  });
}, 60000);
