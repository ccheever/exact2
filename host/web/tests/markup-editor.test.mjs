// @ref LLP 1045 D1/D5/D6 — native DOM edits, controlled source and shared undo.
import { afterAll, test, expect } from 'bun:test';
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium as installed } from '../../../scripts/agent-launch.mjs';
import { buildEditor } from '../../web-js/module.mjs';
import * as playwright from 'playwright-core';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const chrome = installed();
const engines = ['chromium', 'firefox', 'webkit'].filter(name => name === 'chromium' ? !chrome.unavailable : existsSync(playwright[name].executablePath()));
const wasm = engines.length ? readFileSync(await buildEditor()) : null;

async function editor(engine, source, drive) {
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/markup-editor.js') return new Response(Bun.file(resolve(WEB, 'markup-editor.js')), { headers: { 'content-type': 'text/javascript' } });
    if (path === '/markup-editor.wasm') return new Response(wasm, { headers: { 'content-type': 'application/wasm' } });
    return new Response(`<!doctype html><meta charset="utf-8"><textarea id="editor" markup="markdown" style="width:500px;height:240px"></textarea><pre id="echo"></pre>
      <script type="module">
        globalThis.exact = {};
        await import('./markup-editor.js');
        let field = document.getElementById('editor');
        field.value = ${JSON.stringify(source)};
        const install = await exact.installMarkupEditor;
        const host = { live: el => el.isConnected, select() {}, replace(el) {
          field = el;
          field.addEventListener('input', () => { document.getElementById('echo').textContent = field.value; field.value = field.value; });
        } };
        window.restoreMarkup = () => { field.setAttribute('markup', 'markdown'); field = install(field, host); };
        field = install(field, host);
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

// One browser per engine for this file; each editor above gets a context of
// its own (its page, its undo, its storage), closed when it is done.
const launched = new Map();
function browser(engine) {
  if (!launched.has(engine)) launched.set(engine, playwright[engine].launch({ headless: true, ...(engine === 'chromium' ? { executablePath: chrome.executable } : {}) }));
  return launched.get(engine);
}
afterAll(async () => { for (const b of launched.values()) await (await b.catch(() => null))?.close(); });

for (const engine of engines) {
  test(`${engine}: browser Select All includes hidden syntax`, async () => {
    for (const source of ['# Heading', '```js\ncode\n```', '# Heading\n\n[link](https://example.com)\n\n```js\ncode\n```']) {
      await editor(engine, source, async (page, field) => {
        await field.focus();
        await field.evaluate(() => document.execCommand('selectAll'));
        await page.keyboard.insertText('replacement');
        expect(await field.evaluate(el => el.value)).toBe('replacement');
        await page.keyboard.press('ControlOrMeta+Z');
        expect(await field.evaluate(el => el.value)).toBe(source);
      });
    }
    await editor(engine, '# Heading\n\nBody', async (page, field) => {
      await field.focus();
      await field.evaluate(el => {
        const text = el.querySelector('.md-h1').lastChild;
        getSelection().setBaseAndExtent(text, 0, text, text.textContent.length);
      });
      await page.keyboard.insertText('Changed');
      expect(await field.evaluate(el => el.value)).toBe('# Changed\n\nBody');
    });
  }, 60000);

  test(`${engine}: multi-line replacement reports only the new source and undo restores every line`, async () => {
    for (const source of ['one\n\ntwo', '# Heading\n\n[link](https://example.com)\n\n```js\ncode\n```', '- [x] done\n\n> quoted\n\n- two']) {
      await editor(engine, source, async (page, field) => {
        // Untouched hidden constructs also survive the source-mode round trip.
        await field.evaluate(el => { el.removeAttribute('markup'); el.exactMarkup.sync(); });
        expect(await field.inputValue()).toBe(source);
        await page.evaluate(() => restoreMarkup());
        expect(await field.evaluate(el => el.value)).toBe(source);
        await field.click();
        await page.keyboard.press('ControlOrMeta+A');
        await page.keyboard.insertText('replacement');
        expect(await field.evaluate(el => el.value)).toBe('replacement');
        expect(await page.locator('#echo').textContent()).toBe('replacement');
        await page.keyboard.press('ControlOrMeta+Z');
        expect(await field.evaluate(el => el.value)).toBe(source);
        expect(await field.evaluate(el => [...el.children].map(line => line.textContent).join('\n'))).toBe(source);
        await page.keyboard.press('ControlOrMeta+Shift+Z');
        expect(await field.evaluate(el => el.value)).toBe('replacement');
        expect(await field.textContent()).toBe('replacement');
        // Source mode receives the same controlled value after replacement.
        await field.evaluate(el => { el.removeAttribute('markup'); el.exactMarkup.sync(); });
        expect(await field.inputValue()).toBe('replacement');
        await page.evaluate(() => restoreMarkup());
        expect(await field.evaluate(el => el.value)).toBe('replacement');
      });
    }
  }, 60000);

  test(`${engine}: native structural edits and composition reconcile the live tree`, async () => {
    await editor(engine, 'one\n\ntwo', async (page, field) => {
      await field.click();
      await field.evaluate(el => {
        window.nativeLine = el.firstChild;
        getSelection().setBaseAndExtent(el.firstChild.firstChild, 3, el.firstChild.firstChild, 3);
      });
      await page.keyboard.type('!');
      expect(await field.evaluate(el => el.value)).toBe('one!\n\ntwo');
      expect(await field.evaluate(el => el.firstChild === window.nativeLine)).toBe(true);
      await page.keyboard.press('ControlOrMeta+Z');
      expect(await field.evaluate(el => el.value)).toBe('one\n\ntwo');
      // execCommand is a real browser edit that omits beforeinput. It forces
      // the native reconciliation path even when shared rules handle typing.
      await field.evaluate(el => { getSelection().selectAllChildren(el); document.execCommand('insertText', false, 'native'); });
      expect(await field.evaluate(el => el.value)).toBe('native');
      await page.keyboard.press('ControlOrMeta+Z');
      expect(await field.evaluate(el => [...el.children].map(line => line.textContent).join('\n'))).toBe('one\n\ntwo');
      await field.evaluate(el => {
        el.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
        getSelection().selectAllChildren(el);
        document.execCommand('insertText', false, 'composed\nnext');
      });
      expect(await field.evaluate(el => el.value)).toBe('one\n\ntwo');
      await field.evaluate(el => el.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true })));
      expect(await field.evaluate(el => el.value)).toBe('composed\nnext');
      expect(await page.locator('#echo').textContent()).toBe('composed\nnext');
      await page.keyboard.press('ControlOrMeta+Z');
      expect(await field.evaluate(el => [...el.children].map(line => line.textContent).join('\n'))).toBe('one\n\ntwo');
      await page.keyboard.press('ControlOrMeta+Shift+Z');
      expect(await field.evaluate(el => el.value)).toBe('composed\nnext');
    });
  }, 60000);

  test(`${engine}: composition replaces selected hidden fences and preserves cancelled compositions`, async () => {
    const source = '# Heading\n\n[link](https://example.com)\n\n```js\ncode\n```';
    await editor(engine, source, async (page, field) => {
      await field.click();
      await page.keyboard.press('ControlOrMeta+A');
      await field.evaluate(el => {
        el.dispatchEvent(new CompositionEvent('compositionstart'));
        el.dispatchEvent(new CompositionEvent('compositionend'));
      });
      expect(await field.evaluate(el => el.value)).toBe(source);
      await page.keyboard.press('ControlOrMeta+A');
      await field.evaluate(el => {
        el.dispatchEvent(new CompositionEvent('compositionstart'));
        document.execCommand('insertText', false, 'composed\nnext');
        el.dispatchEvent(new CompositionEvent('compositionend'));
      });
      expect(await field.evaluate(el => el.value)).toBe('composed\nnext');
      expect(await page.locator('#echo').textContent()).toBe('composed\nnext');
      await page.keyboard.press('ControlOrMeta+Z');
      expect(await field.evaluate(el => [...el.children].map(line => line.textContent).join('\n'))).toBe(source);
      await page.keyboard.press('ControlOrMeta+Shift+Z');
      expect(await field.evaluate(el => el.value)).toBe('composed\nnext');
    });
  }, 60000);
}
if (!engines.length) test.skip(`Markdown editor needs an installed browser: ${chrome.unavailable}`, () => {});
