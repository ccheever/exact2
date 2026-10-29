// @ref LLP 1045 D1/D5/D6 — native DOM edits, controlled source and shared undo.
import { afterAll, test, expect } from 'bun:test';
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium as installed, suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { buildEditor } from '../../web-js/module.mjs';
import { browserMissing } from '../../../scripts/agent-playwright.mjs';
import { need } from '../../../scripts/coverage.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const chrome = installed();
const engines = ['chromium', 'firefox', 'webkit'];
const missing = Object.fromEntries(await Promise.all(engines.map(async e => [e, await browserMissing(e)])));
const engineTest = Object.fromEntries(engines.map(e => [e, need(e === 'chromium' ? 'chrome' : e, missing[e])]));
const wasm = engines.some(e => !missing[e]) ? readFileSync(await buildEditor()) : null;

/** `attrs`: the textarea's own attributes beside its markup, as a page writes them before the editor mounts. */
async function editor(engine, source, drive, runtime = false, attrs = '') {
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
    const path = new URL(request.url).pathname;
    if (['/markup-editor.js', '/navigation.js', '/atoms.js', '/geometry-glue.js'].includes(path)) return new Response(Bun.file(resolve(WEB, path.slice(1))), { headers: { 'content-type': 'text/javascript' } });
    if (path === '/markup-editor.wasm') return new Response(wasm, { headers: { 'content-type': 'application/wasm' } });
    if (path === '/names.js') return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } }); // no data sources in this fixture
    if (runtime && path.endsWith('.js')) {
      const js = resolve(WEB, '../web-js', path.slice(1));
      return new Response(Bun.file(existsSync(js) ? js : resolve(WEB, path.slice(1))), { headers: { 'content-type': 'text/javascript' } });
    }
    return new Response(`<!doctype html><meta charset="utf-8"><textarea id="editor" markup="markdown" ${attrs} style="width:500px;height:240px"></textarea><pre id="echo"></pre>
      <script type="module">
        globalThis.exact = {};
        let field = document.getElementById('editor');
        field.value = ${JSON.stringify(source)};
        // What the app hears: each field event's InputEvent fields. \`prevent(e)\` cancels a beforeinput.
        window.events = []; window.logs = []; window.prevent = () => false;
        const record = (type, value, start, end, direction, inputType, data, cancelable, composing) => events.push({ type, value, start, end, direction, inputType, data, cancelable, composing });
        if (${runtime}) {
          const { mde, on, onKey, onBeforeInput, onSelectionChange, onValue, Hosts } = await import('./rt.js');
          const { commands } = await import('./commands.js');
          window.command = (name, args) => commands(s => logs.push(s))[name](...args);
          on(field, 'beforeinput', r => { record('beforeinput', r[0], ...r.slice(2)); if (prevent(r)) Hosts.preventDefault(); }, onBeforeInput);
          on(field, 'selectionchange', r => record('selectionchange', r[0], ...r.slice(2)), onSelectionChange);
          on(field, 'input', (v, r) => record('input', v, ...r.slice(2)), onValue);
          window.heard = [];
          on(document.body, 'keycapture', key => {
            heard.push('outer:' + key);
            if (key === 's') Hosts.stopPropagation();
            if (key === 'p') Hosts.preventDefault();
          }, onKey);
          on(document.body, 'key', key => heard.push('bubble:' + key), onKey);
          on(field, 'key', key => heard.push('key:' + key), onKey);
          on(field, 'keycapture', key => {
            heard.push('capture:' + key);
            if (key === 'x' || key.toLowerCase() === 'b') Hosts.preventDefault();
            if (key === 't') Hosts.stopPropagation();
          }, onKey);
          window.original = field; // the runtime's element, whose attributes it forwards to the editor
          mde(field);
        } else {
          await import('./markup-editor.js');
          const { runFocusCommands } = await import('./navigation.js');
          window.command = (name, args) => runFocusCommands([{ name, args }], { root: document.body, ready: true, inertAncestor: () => false, log: s => logs.push(s) });
          const install = await exact.installMarkupEditor;
          const host = { live: el => el.isConnected, select() {}, replace(el) {
            field = el;
            const heard = e => record(e.type, el.value, el.selectionStart, el.selectionEnd, el.selectionDirection, e.inputType ?? '', e.data ?? null, e.cancelable ?? false, e.isComposing ?? false);
            field.addEventListener('beforeinput', e => { heard(e); if (prevent(e)) e.preventDefault(); });
            field.addEventListener('selectionchange', heard);
            field.addEventListener('input', e => { heard(e); document.getElementById('echo').textContent = field.value; field.value = field.value; });
          } };
          window.restoreMarkup = () => { field.setAttribute('markup', 'markdown'); field = install(field, host); };
          field = install(field, host);
          window.field = () => field;
          window.ready = true;
        }
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  const context = await (await browser(engine)).newContext();
  try {
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(`http://127.0.0.1:${server.port}/`);
    await page.waitForFunction(runtime ? () => document.getElementById('editor')?.exactMarkup : () => window.ready);
    await drive(page, page.locator('#editor'));
    expect(errors).toEqual([]);
  } finally { await context.close(); server.stop(true); }
}

// One browser per engine for this file; each editor above gets a context of
// its own (its page, its undo, its storage), closed when it is done.
const launched = new Map();
function browser(engine) {
  if (!launched.has(engine)) launched.set(engine, suiteBrowser(engine));
  return launched.get(engine);
}
afterAll(async () => { for (const b of launched.values()) await (await b.catch(() => null))?.close(); }, 60000);

for (const engine of engines) {
  engineTest[engine](`${engine}: JS runtime forwards Markdown key cancellation in both directions`, async () => {
    await editor(engine, 'hello', async (page, field) => {
      await field.focus();
      await field.evaluate(el => getSelection().selectAllChildren(el));
      await page.keyboard.press('x');
      expect(await field.evaluate(el => el.value)).toBe('hello');
      expect(await page.evaluate(() => heard.splice(0))).toEqual(['outer:x', 'capture:x', 'key:x', 'bubble:x']);
      await page.keyboard.press('p');
      expect(await field.evaluate(el => el.value)).toBe('hello');
      expect(await page.evaluate(() => heard.splice(0))).toEqual(['outer:p', 'capture:p', 'key:p', 'bubble:p']);
      await page.keyboard.press('s');
      expect(await field.evaluate(el => el.value)).toBe('s');
      expect(await page.evaluate(() => heard.splice(0))).toEqual(['outer:s']);
      await page.keyboard.press('t');
      expect(await field.evaluate(el => el.value)).toBe('st');
      expect(await page.evaluate(() => heard.splice(0))).toEqual(['outer:t', 'capture:t']);
      await field.evaluate(el => getSelection().selectAllChildren(el));
      await page.keyboard.press('ControlOrMeta+B');
      expect(await field.evaluate(el => el.value)).toBe('st');
    }, true);
  }, 60000);

  engineTest[engine](`${engine}: a cancelled capture shortcut leaves selected Markdown unchanged`, async () => {
    await editor(engine, 'hello', async (page, field) => {
      await field.focus();
      await field.evaluate(el => {
        getSelection().selectAllChildren(el);
        document.addEventListener('keydown', e => {
          if (e.isTrusted && e.key.toLowerCase() === 'b' && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            e.exactStopped = true;
            e.$stopped = true;
          }
        }, true);
      });
      await page.keyboard.press('ControlOrMeta+B');
      expect(await field.evaluate(el => el.value)).toBe('hello');
      for (const flag of ['exactStopped', '$stopped']) {
        await field.evaluate((el, flag) => {
          const event = new KeyboardEvent('keydown', { key: 'b', ctrlKey: true, bubbles: true, cancelable: true });
          event[flag] = true;
          el.dispatchEvent(event);
        }, flag);
        expect(await field.evaluate(el => el.value)).toBe('hello');
      }
    });
  }, 60000);

  engineTest[engine](`${engine}: browser Select All includes hidden syntax`, async () => {
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

  engineTest[engine](`${engine}: multi-line replacement reports only the new source and undo restores every line`, async () => {
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

  engineTest[engine](`${engine}: native structural edits and composition reconcile the live tree`, async () => {
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
      // While composing, the value is the text the input method left, as a plain field's.
      expect(await field.evaluate(el => el.value)).toBe('composed\nnext');
      await field.evaluate(el => el.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true })));
      expect(await field.evaluate(el => el.value)).toBe('composed\nnext');
      expect(await page.locator('#echo').textContent()).toBe('composed\nnext');
      await page.keyboard.press('ControlOrMeta+Z');
      expect(await field.evaluate(el => [...el.children].map(line => line.textContent).join('\n'))).toBe('one\n\ntwo');
      await page.keyboard.press('ControlOrMeta+Shift+Z');
      expect(await field.evaluate(el => el.value)).toBe('composed\nnext');
    });
  }, 60000);

  engineTest[engine](`${engine}: composition replaces selected hidden fences and preserves cancelled compositions`, async () => {
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

// A frame and a task: the document's own selectionchange, and what it queues, have run.
const settle = page => page.evaluate(() => new Promise(ok => requestAnimationFrame(() => setTimeout(ok))));
const heard = page => page.evaluate(() => events.splice(0));
const only = (list, type) => list.filter(e => e.type === type).map(({ value, start, end, direction, inputType, data, cancelable, composing }) => [value, start, end, direction, inputType, data, cancelable, composing]);

for (const engine of engines) for (const runtime of [false, true]) {
  // @ref LLP 1045 D11 — the app's base direction for each line, as HTML's `dir`.
  engineTest[engine](`${engine}${runtime ? ' (JS runtime)' : ''}: each line takes its paragraphDirs word, in place`, async () => {
    await editor(engine, 'One\nاثنان ثلاثة\n- [ ] أربعة\n', async (page, field) => {
      // The app's words: the runtime writes the textarea it replaced and forwards them; the glue syncs the editor.
      const say = async words => {
        await page.evaluate(([words, runtime]) => {
          (runtime ? window.original : field()).setAttribute('data-paragraph-dirs', words);
          if (!runtime) field().exactMarkup.sync();
        }, [words, runtime]);
        await settle(page);
      };
      const dirs = () => field.evaluate(el => [...el.children].map(line => line.getAttribute('dir')));
      await say('ltr rtl');
      // A line past the last word takes the last: the list item and the empty last line.
      expect(await dirs()).toEqual(['ltr', 'rtl', 'rtl', 'rtl']);
      // A right-to-left list item's marker and gutter are on its right, where a press toggles its box.
      const gutter = await field.evaluate(el => {
        const line = el.children[2], box = line.getBoundingClientRect(), css = getComputedStyle(line);
        return { x: box.right - 10, y: box.top + box.height / 2, left: css.paddingLeft, right: css.paddingRight };
      });
      expect([gutter.left, gutter.right]).toEqual(['0px', '28px']);
      await page.mouse.click(gutter.x, gutter.y); await settle(page);
      expect(await field.evaluate(el => el.value)).toBe('One\nاثنان ثلاثة\n- [x] أربعة\n');
      // The empty last line's caret is at its start, the right.
      expect(await field.evaluate(el => {
        const caret = el.exactMarkup.rangeFrame(el.value.length, el.value.length), box = el.getBoundingClientRect();
        return caret.x > box.x + box.width / 2;
      })).toBe(true);
      // Typing keeps the line's node and its word: a direction never redraws a line.
      await field.evaluate(el => { window.kept = el.children[1]; el.focus(); el.setSelectionRange(15, 15); });
      await page.keyboard.type('y'); await settle(page);
      expect(await field.evaluate(el => [el.value.split('\n')[1], el.children[1] === window.kept, window.kept.getAttribute('dir')])).toEqual(['اثنان ثلاثةy', true, 'rtl']);
      // Another word leaves its line, and those past it, to the field.
      await say('rtl sideways');
      expect(await dirs()).toEqual(['rtl', null, null, null]);
      await say('');
      expect(await dirs()).toEqual([null, null, null, null]);
    }, runtime);
  }, 60000);

  engineTest[engine](`${engine}${runtime ? ' (JS runtime)' : ''}: a word stays with its line through an edit that adds or removes one`, async () => {
    await editor(engine, 'A\nب\nC', async (page, field) => {
      const dirs = () => field.evaluate(el => [...el.children].map(line => line.getAttribute('dir')));
      // Words the page wrote before the editor mounted.
      expect(await dirs()).toEqual(['ltr', 'rtl', 'ltr']);
      // Return at the end of the first line: the new line takes the word above it, and ب keeps its own.
      await field.evaluate(el => { el.focus(); el.setSelectionRange(1, 1); });
      await page.keyboard.press('Enter'); await settle(page);
      expect(await field.evaluate(el => el.value)).toBe('A\n\nب\nC');
      expect(await dirs()).toEqual(['ltr', 'ltr', 'rtl', 'ltr']);
      // Delete at the empty line takes it back out (forward: WebKit's Backspace navigates in the runtime harness).
      await page.keyboard.press('Delete'); await settle(page);
      expect(await field.evaluate(el => el.value)).toBe('A\nب\nC');
      expect(await dirs()).toEqual(['ltr', 'rtl', 'ltr']);
      // The app's next words go by line number again.
      await page.evaluate(runtime => {
        (runtime ? window.original : field()).setAttribute('data-paragraph-dirs', 'rtl');
        if (!runtime) field().exactMarkup.sync();
      }, runtime);
      await settle(page);
      expect(await dirs()).toEqual(['rtl', 'rtl', 'rtl']);
    }, runtime, 'data-paragraph-dirs="ltr rtl ltr"');
  }, 60000);

  engineTest[engine](`${engine}${runtime ? ' (JS runtime)' : ''}: quote bars, list markers and the placeholder sit at each line's start`, async () => {
    await editor(engine, '> اقتباس\n- عنصر', async (page, field) => {
      // Each line's start border and padding, physical: [left border, right border, left padding, right padding].
      const sides = () => field.evaluate(el => [...el.children].map(line => {
        const css = getComputedStyle(line);
        return [css.borderLeftWidth, css.borderRightWidth, css.paddingLeft, css.paddingRight];
      }));
      expect(await sides()).toEqual([['18px', '0px', '0px', '0px'], ['0px', '0px', '28px', '0px']]);
      // A field whose own direction is right to left, with no words: CSS direction alone.
      await field.evaluate(el => { el.style.direction = 'rtl'; });
      expect(await sides()).toEqual([['0px', '18px', '0px', '0px'], ['0px', '0px', '0px', '28px']]);
      await field.evaluate(el => { el.style.direction = ''; });
      await page.evaluate(runtime => {
        (runtime ? window.original : field()).setAttribute('data-paragraph-dirs', 'rtl');
        if (!runtime) field().exactMarkup.sync();
      }, runtime);
      await settle(page);
      expect(await sides()).toEqual([['0px', '18px', '0px', '0px'], ['0px', '0px', '0px', '28px']]);
      // Emptied, the placeholder is drawn on the first line, in that line's direction.
      await field.evaluate(el => { el.value = ''; });
      await settle(page);
      expect(await field.evaluate(el => {
        const line = el.children[0];
        return [el.value, line.getAttribute('dir'), getComputedStyle(line, '::before').content, getComputedStyle(line, '::before').direction];
      })).toEqual(['', 'rtl', '"Ask"', 'rtl']);
    }, runtime, 'placeholder="Ask"');
  }, 60000);

  engineTest[engine](`${engine}${runtime ? ' (JS runtime)' : ''}: source selection, range edits, their undo and the app's beforeinput`, async () => {
    await editor(engine, 'hello', async (page, field) => {
      const command = (name, ...args) => page.evaluate(([name, args]) => command(name, args), [name, args]);
      const state = () => field.evaluate(el => [el.value, el.selectionStart, el.selectionEnd, el.selectionDirection]);
      await field.focus(); await settle(page); await heard(page);
      // A script's selection: source offsets and its direction, then one selectionchange.
      await command('setSelectionRange', 'editor', 1, 4, 'backward'); await settle(page);
      expect(await state()).toEqual(['hello', 1, 4, 'backward']);
      expect(only(await heard(page), 'selectionchange')).toEqual([['hello', 1, 4, 'backward', '', null, false, false]]);
      // Two range edits in one task: one insertReplacementText input each, and two undo steps.
      await command('setSelectionRange', 'editor', 5, 5); await settle(page); await heard(page);
      await page.evaluate(() => { command('setRangeText', ['editor', '!', 5, 5, 'end']); command('setRangeText', ['editor', '?', 6, 6, 'end']); });
      await settle(page);
      expect(await state()).toEqual(['hello!?', 7, 7, 'none']);
      expect(only(await heard(page), 'input')).toEqual([['hello!', 6, 6, 'none', 'insertReplacementText', '!', false, false], ['hello!?', 7, 7, 'none', 'insertReplacementText', '?', false, false]]);
      await page.keyboard.press('ControlOrMeta+Z');
      expect(await state()).toEqual(['hello!', 6, 6, 'none']);
      await page.keyboard.press('ControlOrMeta+Z');
      expect(await state()).toEqual(['hello', 5, 5, 'none']);
      await page.keyboard.press('ControlOrMeta+Shift+Z');
      expect(await state()).toEqual(['hello!', 6, 6, 'none']);
      const history = only(await heard(page), 'beforeinput');
      expect(history.map(e => [e[4], e[6]])).toEqual([['historyUndo', true], ['historyUndo', true], ['historyRedo', true]]);
      // The app's beforeinput runs first: one it cancels changes nothing.
      await page.evaluate(() => { window.prevent = e => ['historyUndo', 'insertParagraph', 'insertLineBreak'].includes(e.inputType ?? e[5]); });
      await page.keyboard.press('ControlOrMeta+Z');
      await page.keyboard.press('Enter');
      await settle(page);
      expect(await state()).toEqual(['hello!', 6, 6, 'none']);
      const cancelled = await heard(page);
      expect(only(cancelled, 'input')).toEqual([]);
      expect(only(cancelled, 'beforeinput').map(e => e[4])).toEqual(['historyUndo', expect.stringMatching(/^insert(Paragraph|LineBreak)$/)]);
      await page.evaluate(() => { window.prevent = () => false; });
      // Typing: the platform's input type and data, the selection it left.
      await page.keyboard.type('y'); await settle(page);
      expect(only(await heard(page), 'input')).toEqual([['hello!y', 7, 7, 'none', 'insertText', 'y', false, false]]);
      // Return in a list: the app hears it before the list continues.
      await command('setRangeText', 'editor', '- one', 0, 99, 'end'); await settle(page); await heard(page);
      await page.keyboard.press('Enter'); await settle(page);
      expect(await state()).toEqual(['- one\n- ', 8, 8, 'none']);
      const enter = (await heard(page)).filter(e => e.type !== 'selectionchange');
      expect(enter.map(e => [e.type, e.value, e.start, e.cancelable])).toEqual([['beforeinput', '- one', 5, true], ['input', '- one\n- ', 8, false]]);
      // Each step undoes alone; the typing before the range edits survives.
      await page.keyboard.press('ControlOrMeta+Z');
      expect(await state()).toEqual(['- one', 5, 5, 'none']);
      await page.keyboard.press('ControlOrMeta+Z');
      expect((await state())[0]).toBe('hello!y');
      await page.keyboard.press('ControlOrMeta+Z');
      expect((await state())[0]).toBe('hello!');
      // HTML's refusals, logged; nothing changes.
      await page.evaluate(() => { logs.length = 0; });
      await command('setRangeText', 'editor', 'x', 3, 1);
      expect(await page.evaluate(() => logs.splice(0))).toEqual(['setRangeText "editor" refused: start exceeds end']);
      expect((await state())[0]).toBe('hello!');
      // A caret the app puts in hidden syntax is drawn, and reads back, at its visible edge (LLP 1045 D6).
      await command('setRangeText', 'editor', 'a **b** c', 0, 99, 'end');
      await command('setSelectionRange', 'editor', 3, 3);
      expect([2, 4]).toContain((await state())[1]);
    }, runtime);
  }, 60000);
}

// Chrome's input method, through CDP: a composition's events carry the DOM's
// text and selection in source offsets, hidden syntax counted.
engineTest.chromium('chromium: a composition reports source offsets as it composes', async () => {
  for (const runtime of [false, true]) await editor('chromium', 'a **b** c', async (page, field) => {
    const cdp = await page.context().newCDPSession(page);
    await field.focus();
    await page.evaluate(() => command('setSelectionRange', ['editor', 5, 5]));
    await settle(page); await heard(page);
    await cdp.send('Input.imeSetComposition', { text: 'に', selectionStart: 1, selectionEnd: 1 });
    await cdp.send('Input.imeSetComposition', { text: 'にほ', selectionStart: 2, selectionEnd: 2 });
    await settle(page);
    const composing = await heard(page);
    expect(only(composing, 'beforeinput').map(e => [e[4], e[6], e[7]])).toEqual([['insertCompositionText', false, true], ['insertCompositionText', false, true]]);
    expect(only(composing, 'input')).toEqual([['a **bに** c', 6, 6, 'none', 'insertCompositionText', 'に', false, true], ['a **bにほ** c', 7, 7, 'none', 'insertCompositionText', 'にほ', false, true]]);
    // A range edit waits for the composition to end.
    await page.evaluate(() => { logs.length = 0; command('setRangeText', ['editor', 'x', 0, 0]); });
    expect(await page.evaluate(() => logs.splice(0))).toEqual(['setRangeText "editor" refused: text is being composed']);
    await cdp.send('Input.insertText', { text: '日本' });
    await settle(page);
    expect(await field.evaluate(el => [el.value, el.selectionStart, el.selectionEnd])).toEqual(['a **b日本** c', 7, 7]);
    expect(only(await heard(page), 'input').at(-1)).toEqual(['a **b日本** c', 7, 7, 'none', 'insertCompositionText', '日本', false, false]);
    await page.keyboard.press('ControlOrMeta+Z');
    expect(await field.evaluate(el => el.value)).toBe('a **b** c');
  }, runtime);
}, 60000);

engineTest.chromium('chromium: Markdown editing escapes an inert ancestor at an open modal', async () => {
  await editor('chromium', 'hello', async (page, field) => {
    await field.evaluate(el => {
      const ancestor = document.createElement('div'), dialog = document.createElement('dialog');
      el.before(ancestor); ancestor.append(dialog); dialog.append(el); dialog.showModal(); ancestor.inert = true;
      el.focus(); getSelection().selectAllChildren(el);
    });
    await page.keyboard.insertText('edited');
    expect(await field.evaluate(el => el.value)).toBe('edited');
    await page.keyboard.press('ControlOrMeta+Z');
    expect(await field.evaluate(el => el.value)).toBe('hello');
    for (const target of ['editor', 'dialog']) {
      expect(await field.evaluate((el, target) => {
        const blocked = target === 'editor' ? el : el.closest('dialog');
        blocked.inert = true;
        const accepted = el.exactMarkup.format('bold');
        blocked.inert = false;
        return [accepted, el.value];
      }, target)).toEqual([false, 'hello']);
    }
  }, true);
}, 60000);
