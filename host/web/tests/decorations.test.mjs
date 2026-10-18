import { afterAll, expect } from 'bun:test';
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import * as playwright from 'playwright-core';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { browserMissing } from '../../../scripts/agent-playwright.mjs';
import { need } from '../../../scripts/coverage.mjs';
import { buildEditor } from '../../web-js/module.mjs';

const WEB = resolve(import.meta.dir, '..');
const engines = ['chromium', 'firefox', 'webkit'];
const missing = Object.fromEntries(await Promise.all(engines.map(async engine => [engine, await browserMissing(engine)])));
const wasm = engines.some(engine => !missing[engine]) ? readFileSync(await buildEditor()) : null;
const browsers = new Map();
async function browser(engine) {
  if (!browsers.has(engine)) browsers.set(engine, playwright[engine].launch({ headless: true, ...(engine === 'chromium' ? { executablePath: chromium().executable } : {}) }));
  return browsers.get(engine);
}
afterAll(async () => { await Promise.all([...browsers.values()].map(async value => (await value).close())); }, 30000);

const source = '## Heading\nhello **world** @alice\n- [ ] task';
const layout = { value: source, spans: [{ name: 'accent', start: 19, end: 24 }],
  blocks: [{ name: 'heading', start: 0, end: 11 }, { name: 'task', start: 34, end: 44, marker: 'checkbox:off' }],
  hidden: [{ start: 0, end: 3 }, { start: 17, end: 19, reveal: { start: 17, end: 26 } }, { start: 24, end: 26, reveal: { start: 17, end: 26 } }, { start: 34, end: 40 }],
  atoms: [{ name: 'pill', start: 27, end: 33, label: 'Alice' }] };
const table = { heading: 'font-size:20px;font-weight:600;margin-bottom:8px;', accent: 'color:rgb(32,86,200);', task: 'padding-left:24px;', pill: 'background-color:rgb(210,230,250);' };

async function field(engine, record, drive) {
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/favicon.ico') return new Response(null, { status: 204 });
    if (path === '/markup-editor.wasm') return new Response(wasm, { headers: { 'content-type': 'application/wasm' } });
    if (path === '/names.js') return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
    if (path.endsWith('.js')) {
      const file = resolve(WEB, path.slice(1)), fallback = resolve(WEB, '../web-js', path.slice(1));
      return new Response(Bun.file(existsSync(file) ? file : fallback), { headers: { 'content-type': 'text/javascript' } });
    }
    return new Response(`<!doctype html><meta charset="utf-8"><textarea id="editor" aria-label="Editor" style="width:500px;height:240px;font:16px/1.5 sans-serif"></textarea><button id="outside">Outside</button>
      <script type="module">
        globalThis.exact = {}; window.events = []; window.toggles = [];
        let field = document.getElementById('editor'); field.value = ${JSON.stringify(record.value)};
        field.setAttribute('data-decorations', ${JSON.stringify(JSON.stringify(record))});
        field.setAttribute('data-decoration-styles', ${JSON.stringify(JSON.stringify(table))});
        await import('./decoration-editor.js');
        field = (await exact.installDecorationEditor)(field, {log: message => {throw Error(message)}, replace(node) {field=node;}, toggle(node,checked,start,end) {toggles.push([checked,start,end]);} });
        field.addEventListener('input', event => events.push([field.value,event.inputType]));
        window.field = () => field;
        window.rangeEdit = (...args) => import('./navigation.js').then(module => module.setFieldRangeText(field, ['editor', ...args], message => {throw Error(message)}));
        window.cancelNextInput = () => import('./navigation.js').then(({controlEvent}) => controlEvent(field, 'beforeinput', (kind, fn, capture) => field.addEventListener(kind, fn, {capture, once:true}), (_kind, _payload, event) => event.preventDefault()));
        window.ready = true;
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  const context = await (await browser(engine)).newContext();
  try {
    const page = await context.newPage(), errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(server.url.href);
    await page.waitForFunction(() => window.ready);
    await drive(page, page.locator('#editor'));
    expect(errors).toEqual([]);
  } finally { await context.close(); server.stop(true); }
}

for (const engine of engines) {
  const check = need(engine === 'chromium' ? 'chrome' : engine, missing[engine]);
  check(`${engine}: layout decorations preserve the source, apply styles and report range/toggle geometry`, async () => {
    await field(engine, layout, async (page, editor) => {
      expect(await editor.evaluate(el => el.value)).toBe(source);
      expect(await editor.evaluate(el => getComputedStyle(el.firstElementChild).fontSize)).toBe('20px');
      expect(await editor.locator('.exact-decoration-atom').innerText()).toBe('Alice');
      expect(await editor.evaluate(el => el.exactDecorations.applied.provisional)).toBe(false);
      const box = await editor.evaluate(el => { const r = el.exactDecorations.rangeFrame(19,24); return {width:r.width,height:r.height}; });
      expect(box.width).toBeGreaterThan(0); expect(box.height).toBeGreaterThan(0);
      await editor.locator('[role=checkbox]').click();
      expect(await page.evaluate(() => toggles)).toEqual([[false,34,44]]);
      expect(await editor.evaluate(el => el.value)).toBe(source);
      expect(await page.evaluate(() => events)).toEqual([]);
      const hidden = await editor.evaluate(async el => {
        await import('./geometry-glue.js'); const geometry = exact.geometry(document.body), out = new Float64Array(4);
        const shown = geometry.range(el,19,24,out); el.style.display = 'none'; const hidden = geometry.range(el,19,24,out); el.style.display = '';
        return [shown,hidden];
      });
      expect(hidden).toEqual([1,0]);
    });
  }, 30000);

  check(`${engine}: app range replacement is one undo step with source and selection restored`, async () => {
    await field(engine, layout, async (page, editor) => {
      await editor.focus(); await editor.evaluate(el => el.setSelectionRange(11,16));
      await page.evaluate(() => rangeEdit('Hi',11,16,'select','undo'));
      expect(await editor.evaluate(el => el.value)).toBe(source.replace('hello','Hi'));
      expect(await editor.evaluate(el => [el.selectionStart,el.selectionEnd])).toEqual([11,13]);
      expect(await page.evaluate(() => events)).toHaveLength(1);
      await editor.press('ControlOrMeta+z');
      expect(await editor.evaluate(el => [el.value,el.selectionStart,el.selectionEnd])).toEqual([source,11,16]);
      await editor.press('ControlOrMeta+Shift+z');
      expect(await editor.evaluate(el => el.value)).toBe(source.replace('hello','Hi'));
    });
  }, 30000);

  check(`${engine}: stale ranges reconcile after native typing and marked text is never restyled`, async () => {
    await field(engine, layout, async (page, editor) => {
      await editor.focus(); await editor.evaluate(el => el.setSelectionRange(0,0));
      await editor.press('ArrowRight'); await editor.press('End');
      await editor.evaluate(el => el.setSelectionRange(el.value.length,el.value.length));
      await page.keyboard.type('!');
      expect(await editor.evaluate(el => el.value)).toBe(source+'!');
      expect(await editor.evaluate(el => el.exactDecorations.applied.provisional)).toBe(true);
      const composition = await editor.evaluate(el => {
        el.dispatchEvent(new CompositionEvent('compositionstart'));
        const before = el.firstChild;
        const text = el.lastElementChild.lastElementChild.firstChild; text.data += '한';
        getSelection().setBaseAndExtent(text,text.length,text,text.length);
        el.dispatchEvent(new InputEvent('input',{inputType:'insertCompositionText',data:'한',isComposing:true,bubbles:true}));
        const marked = el.value.endsWith('!한') && events.at(-1)[0] === el.value;
        const record = JSON.parse(el.getAttribute('data-decorations')); record.value = el.value;
        el.setAttribute('data-decorations',JSON.stringify(record)); el.exactDecorations.sync();
        const held = before === el.firstChild;
        el.dispatchEvent(new CompositionEvent('compositionend'));
        return [held,marked,el.value,el.exactDecorations.applied.provisional];
      });
      expect(composition).toEqual([true,true,source+'!한',false]);
    });
  }, 30000);

  check(`${engine}: paint-only records use the native field where value ranges exist`, async () => {
    const record = { value:'hello world', spans:[{name:'accent',start:6,end:11}], blocks:[],hidden:[],atoms:[] };
    await field(engine, record, async (page, editor) => {
      const native = await page.evaluate(() => typeof HTMLTextAreaElement.prototype.createValueRange === 'function' && typeof Highlight === 'function' && !!CSS.highlights);
      expect(await editor.evaluate(el => el.localName)).toBe(native ? 'textarea' : 'div');
      expect(await editor.evaluate(el => el.value)).toBe('hello world');
      await editor.focus(); await editor.evaluate(el => el.setSelectionRange(0,0)); await page.keyboard.type('!');
      expect(await editor.evaluate(el => el.value)).toBe('!hello world');
      expect(await editor.evaluate(el => el.exactDecorations.applied.spans[0])).toMatchObject({start:7,end:12});
      expect(await editor.evaluate(el => el.exactDecorations.rangeFrame(7,12).width)).toBeGreaterThan(0);
      await page.evaluate(() => rangeEdit('X',1,1,'end','undo'));
      expect(await editor.evaluate(el => el.value)).toBe('!Xhello world');
      await editor.press('ControlOrMeta+z'); expect(await editor.evaluate(el => el.value)).toBe('!hello world');
      await editor.press('ControlOrMeta+z'); expect(await editor.evaluate(el => el.value)).toBe('hello world');
      const plain = await page.evaluate(async () => {
        await import('./geometry-glue.js'); const el = document.createElement('textarea'); el.value = 'plain text'; document.body.append(el);
        const out = new Float64Array(4), bits = exact.geometry(document.body).range(el,0,5,out); el.remove(); return [bits,out[2]];
      });
      expect(plain[0]).toBe(native ? 1 : 0); if (native) expect(plain[1]).toBeGreaterThan(0);
      const promoted = await page.evaluate(async () => {
        const {bindRuntime} = await import('./decoration-editor.js'), el = document.createElement('textarea');
        el.value = 'hello world'; el.setAttribute('data-decorations', JSON.stringify({value:el.value,spans:[{name:'accent',start:6,end:11}],blocks:[],hidden:[],atoms:[]}));
        el.setAttribute('data-decoration-styles', JSON.stringify({accent:'color:red;'})); document.body.append(el);
        const Views = new Map([[1,el]]), cleanup = bindRuntime(el,{install:await exact.installDecorationEditor,id:1,Ids:new WeakMap(),Views,log:message=>{throw Error(message)}});
        el.setAttribute('data-decoration-styles', JSON.stringify({accent:'font-size:28px;'}));
        await new Promise(resolve => queueMicrotask(resolve));
        const field = Views.get(1), size = getComputedStyle([...field.querySelectorAll('span')].find(span=>span.textContent==='world')).fontSize;
        cleanup(); field.remove(); return [field.localName,size];
      });
      expect(promoted).toEqual(['div','28px']);
    });
  }, 30000);

  check(`${engine}: chips remain atomic across style overlaps and clipboard copies preserve source`, async () => {
    const record = structuredClone(layout); record.spans.push({name:'accent', start:28, end:30});
    await field(engine, record, async (page, editor) => {
      expect(await editor.locator('.exact-decoration-atom').count()).toBe(1);
      await editor.focus(); await editor.evaluate(el => el.setSelectionRange(0, el.value.length));
      const copied = await editor.evaluate(el => {
        const clipboardData = new DataTransfer();
        const event = new Event('copy', {cancelable:true, bubbles:true});
        Object.defineProperty(event, 'clipboardData', {value:clipboardData}); el.dispatchEvent(event);
        return clipboardData.getData('text/plain');
      });
      expect(copied).toBe(source);
      await page.evaluate(() => rangeEdit('Bob',28,30,'select','undo'));
      expect(await editor.evaluate(el => el.value)).toBe(source.replace('@alice','Bob'));
      await editor.press('ControlOrMeta+z');
      expect(await editor.evaluate(el => el.value)).toBe(source);
      await editor.evaluate(el => el.setSelectionRange(33,33)); await page.evaluate(() => cancelNextInput()); await editor.press('Backspace');
      expect(await editor.evaluate(el => el.value)).toBe(source);
      await editor.evaluate(el => el.setSelectionRange(33,33)); await editor.press('Backspace');
      expect(await editor.evaluate(el => el.value)).toBe(source.replace('@alice',''));
      expect(await page.evaluate(()=>events.at(-1)[1])).toBe('deleteContentBackward');
      await editor.evaluate(el => {el.value = 'Server replacement';}); await editor.press('ControlOrMeta+z');
      expect(await editor.evaluate(el => el.value)).toBe('Server replacement');
    });
  }, 30000);

  check(`${engine}: native line edits preserve UTF-16 source and range frames follow scrolling`, async () => {
    const value = 'a😀b\n'+'long wrapped source '.repeat(30), record = {value,spans:[{name:'accent',start:1,end:3}],blocks:[],hidden:[],atoms:[]};
    // A layout declaration exercises the source editor on every browser.
    record.blocks.push({name:'heading',start:0,end:5});
    await field(engine, record, async (_page, editor) => {
      await editor.focus(); await editor.evaluate(el=>el.setSelectionRange(3,3)); await editor.press('Enter');
      expect(await editor.evaluate(el=>el.value)).toBe('a😀\nb\n'+value.slice(5));
      await editor.press('ControlOrMeta+z'); expect(await editor.evaluate(el=>el.value)).toBe(value);
      const moved = await editor.evaluate(el=>{
        const before = el.exactDecorations.rangeFrame(5,25).y, top = el.scrollTop; el.scrollTop = 40;
        return [before-el.exactDecorations.rangeFrame(5,25).y, el.scrollTop-top];
      });
      expect(moved[0]).toBeGreaterThan(0); expect(moved[0]).toBe(moved[1]);
    });
  }, 30000);
}
