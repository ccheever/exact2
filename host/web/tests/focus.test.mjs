import {test,expect} from 'bun:test';
import {focusController} from '../navigation.js';
import {createInputHandlers} from '../input-glue.js';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';

// Production focus transaction, including autofocus during the dispatched commit.
test('E11 only enclosing canvases receive pointer focus; new autofocus wins',()=>{
  const previous={document:globalThis.document,HTMLButtonElement:globalThis.HTMLButtonElement,getComputedStyle:globalThis.getComputedStyle};
  class Button {
    constructor(canvas=null) {this.canvas=canvas;this.attrs={};this.isConnected=true;}
    closest() {return this.canvas;}
    getAttribute(name) {return this.attrs[name]??null;}
    hasAttribute(name) {return name in this.attrs;}
    matches() {return false;}
    getClientRects() {return [{}];}
    setAttribute() {}
    focus() {document.activeElement=this;}
  }
  const canvas={isConnected:true,focus(){document.activeElement=this;},matches:()=>true,contains:el=>el.canvas===canvas};
  const button=new Button(canvas),plain=new Button(),slider=new Button(canvas),action=new Button(canvas);
  slider.attrs.role='slider';action.attrs['data-action']='move';
  globalThis.document={body:{},activeElement:null};globalThis.HTMLButtonElement=Button;
  globalThis.getComputedStyle=()=>({visibility:'visible'});
  const elements=[],focus=focusController({ready:()=>true,elements:()=>elements,inert:()=>false});
  const event=detail=>({detail,preventDefault(){this.prevented=true;},stopPropagation(){}});
  try {
    for(const el of [plain,slider,action]) {el.focus();focus.press(event(1),el,()=>{});expect(document.activeElement).toBe(el);}
    button.focus();const pointer=event(1);focus.press(pointer,button,()=>expect(document.activeElement).toBe(button));expect(document.activeElement).toBe(canvas);expect(pointer.prevented).toBe(true);
    button.focus();focus.press(event(0),button,()=>{});expect(document.activeElement).toBe(button);
    for(const old of [button,plain]) {
      old.focus();const next=new Button();next.exactAutofocus=true;
      focus.press(event(1),old,()=>{elements.push(next);focus.autofocus();});
      expect(document.activeElement).toBe(next);
    }
    button.focus();const explicit=new Button();focus.press(event(1),button,()=>explicit.focus());expect(document.activeElement).toBe(explicit);
    for(const detail of [0,1]) {
      for(const destination of ['canvas','autofocus','explicit','removed-canvas','ordinary']) {
        const old=new Button(destination==='ordinary'?null:canvas),next=new Button();old.focus();
        focus.press(event(detail),old,()=>{
          old.isConnected=false;old.canvas=null;document.activeElement=document.body;
          if(destination==='removed-canvas')canvas.isConnected=false;
          if(destination==='autofocus'){next.exactAutofocus=true;elements.push(next);focus.autofocus();}
          if(destination==='explicit')next.focus();
        });
        expect(document.activeElement).toBe(destination==='autofocus'||destination==='explicit'?next:destination==='canvas'?canvas:document.body);
        canvas.isConnected=true;
      }
    }
  } finally {Object.assign(globalThis,previous);}
});

// A carried restart: focus follows its place in the runner's tree, and the
// restarted tree autofocuses nothing; a node mounted later still may.
test('a carried restart keeps focus at its place and autofocuses nothing',()=>{
  const previous={document:globalThis.document,getComputedStyle:globalThis.getComputedStyle};
  class El {
    constructor(autofocus=false) {this.exactAutofocus=autofocus;this.isConnected=true;}
    getClientRects() {return [{}];}
    matches() {return false;}
    setAttribute() {}
    focus() {document.activeElement=this;}
  }
  globalThis.document={body:{},activeElement:null};globalThis.getComputedStyle=()=>({visibility:'visible'});
  const tree=(root,ids,types=['Button','Button'])=>({roots:[root],nodes:[{id:root,parent:null,type:'View',children:ids},...ids.map((id,i)=>({id,parent:root,type:types[i],children:[]}))]});
  let elements=[];const focus=focusController({ready:()=>true,elements:()=>elements,inert:()=>false});
  const run=(kept,next,t)=>{const views=new Map(next);focus.restart(kept,()=>{elements=[...views.values()];focus.autofocus();},()=>t,id=>views.get(id));};
  try {
    // A fresh boot (no kept place, not even null) autofocuses as it mounts.
    const first=new El(true),other=new El();run(undefined,[[2,first],[3,other]],tree(1,[2,3]));expect(document.activeElement).toBe(first);
    other.focus();
    const kept=focus.keep(tree(1,[2,3]),3);expect(kept).toEqual({path:[0,1],type:'Button'});
    // New ids and a changed label: the place and the type are what count.
    const first2=new El(true),other2=new El();document.activeElement=document.body;
    run(kept,[[11,first2],[12,other2]],tree(10,[11,12]));
    expect(document.activeElement).toBe(other2);
    // Nothing was focused: the restart autofocuses nothing.
    const first3=new El(true);document.activeElement=document.body;
    run(null,[[21,first3]],tree(20,[21]));
    expect(document.activeElement).toBe(document.body);
    // The place now holds another type: nothing is focused.
    const field=new El();document.activeElement=document.body;
    run(kept,[[31,new El()],[32,field]],tree(30,[31,32],['Button','TextInput']));
    expect(document.activeElement).toBe(document.body);
    // A node mounted after the restart still autofocuses.
    const later=new El(true);elements.push(later);focus.autofocus();
    expect(document.activeElement).toBe(later);
  } finally {Object.assign(globalThis,previous);}
});

test('declared shortcuts support named keys and leave text input and composition with editors',()=>{
  const previous={document:globalThis.document,getComputedStyle:globalThis.getComputedStyle};
  const handlers=new Map();
  let chord='',presses=0,editing=false,inert=false,modal=null,shown=[],activates=false;
  const button={isConnected:true,disabled:false,getClientRects:()=>[{}],getAttribute:()=>chord,click:()=>presses++};
  const root={addEventListener(){},querySelectorAll:s=>s.includes('aria-modal')?shown:[button],contains:()=>false};
  const target={matches:s=>s.startsWith('button')?activates:editing,closest:()=>modal};
  // Every listener on the document hears the key, in order, as a browser's would.
  globalThis.document={addEventListener:(name,handler)=>handlers.set(name,[...(handlers.get(name)??[]),handler]),activeElement:target};
  globalThis.getComputedStyle=()=>({visibility:'visible'});
  const key=(key,mods={},extra={})=>{
    const event={key,metaKey:false,ctrlKey:false,altKey:false,shiftKey:false,...mods,...extra,
      composedPath:()=>[target],preventDefault(){this.prevented=true;},stopImmediatePropagation(){this.stopped=true;}};
    for(const handler of handlers.get('keydown'))handler(event);return event;
  };
  try {
    createInputHandlers({root,views:new Map(),retiredViews:new Set(),ready:()=>true,inertAncestor:()=>inert,dispatch(){}});
    const cases=[
      ['Meta+Shift+Enter','Enter',{metaKey:true,shiftKey:true}],
      ['Meta+Shift+ArrowUp','ArrowUp',{metaKey:true,shiftKey:true}],
      ['Control+ArrowDown','ArrowDown',{ctrlKey:true}],
      ['Meta+Shift+Plus','+',{metaKey:true,shiftKey:true}],
      ['Meta+Shift++','+',{metaKey:true,shiftKey:true}],
      ['c','c',{}],['Space',' ',{}],['Escape','Escape',{}],['PageDown','PageDown',{}],['F12','F12',{}],
    ];
    for(const [declaration,name,mods] of cases){
      chord=declaration;
      const before=presses;
      expect(key(name,mods).prevented).toBe(true);expect(presses).toBe(before+1);
      expect(key(name,{...mods,altKey:true}).prevented).toBeUndefined();expect(presses).toBe(before+1);
    }
    chord='Alt+c';expect(key('ç',{altKey:true}).prevented).toBeUndefined();
    chord='Alt+e';expect(key('Dead',{altKey:true}).prevented).toBeUndefined();
    chord='Alt+ç';expect(key('ç',{altKey:true}).prevented).toBe(true);
    editing=true;
    expect(key('ç',{altKey:true}).prevented).toBeUndefined();
    for(const [declaration,name,mods] of [['c','c',{}],['ArrowUp','ArrowUp',{}],['Space',' ',{}],['Alt+c','c',{altKey:true}]]) {
      chord=declaration;expect(key(name,mods).prevented).toBeUndefined();
    }
    chord='Meta+Shift+Enter';
    expect(key('Enter',{metaKey:true,shiftKey:true}).prevented).toBe(true);
    const before=presses;
    expect(key('Enter',{metaKey:true,shiftKey:true},{isComposing:true}).prevented).toBeUndefined();
    chord='Escape';expect(key('Escape',{}, {isComposing:true}).prevented).toBeUndefined();expect(presses).toBe(before);
    expect(key('Escape').prevented).toBe(true);
    editing=false;chord='c';
    const remaining=presses;
    expect(key('c',{}, {repeat:true}).prevented).toBe(true);expect(presses).toBe(remaining);
    button.disabled=true;expect(key('c').prevented).toBe(true);expect(presses).toBe(remaining);button.disabled=false;
    inert=true;expect(key('c').prevented).toBeUndefined();inert=false;
    modal={contains:()=>false};expect(key('c').prevented).toBeUndefined();modal=null;
    // Gallery F22: a shown `aria-modal` view holds the shortcuts behind it; a hidden one holds none.
    shown=[{getClientRects:()=>[{}],contains:()=>false}];expect(key('c').prevented).toBeUndefined();
    shown=[{getClientRects:()=>[],contains:()=>false}];expect(key('c').prevented).toBe(true);shown=[];
    // Onboarding F27: Enter or Space on a focused button is its own, not a shortcut's.
    const own=presses;activates=true;chord='Enter';expect(key('Enter').prevented).toBeUndefined();
    expect(key('Enter',{metaKey:true}).prevented).toBeUndefined();chord='Meta+Enter';expect(key('Enter',{metaKey:true}).prevented).toBe(true);activates=false;
    const remaining2=presses;expect(remaining2).toBe(own+1);
    for(const invalid of ['Meta+','Meta+++','Meta+Unknown','Cmd+c','F0','F36','Enter+c']) {
      chord=invalid;expect(key('c',{metaKey:true}).prevented).toBeUndefined();
    }
    expect(presses).toBe(remaining2);
  } finally {Object.assign(globalThis,previous);}
});

// LLP 1104 D3/D6: exercise the shipped stylesheet in Chrome. A disabled native
// field's ink is the look's grey (the role table's `GrayText`, which the
// neutral sheet carries; LLP 1116 D1's look left it to the user agent), never
// the page's own colour.
test('bare fields restore keyboard focus and a disabled native field is the look\'s grey, not the page\'s ink', async () => {
  const {readFileSync} = await import('node:fs');
  const {withLook} = await import('../look.mjs');
  const browser = await suiteBrowser();
  try {
    const page = await browser.newPage();
    const css = withLook(readFileSync(new URL('../index.html', import.meta.url), 'utf8')).match(/<style>([\s\S]*?)<\/style>/)[1];
    for (const scheme of ['light', 'dark']) {
      await page.setContent(`<style>${css}#exact-root#exact-root{.c1{outline-style:none}}</style><div id="exact-root" style="color:red;color-scheme:${scheme}">
        <input id="bare"><textarea id="area"></textarea><textarea id="markdown" markup="markdown"></textarea>
        <input id="native" data-native disabled value="Disabled"><textarea id="native-area" data-native disabled>Disabled</textarea>
        <input id="authored" data-native disabled style="color:rgb(0,128,0)">
        <input id="quiet" style="outline-style:none"><textarea id="quiet-area" class="c1"></textarea><input id="quiet-native" data-native class="c1">
        <span id="gray" style="color:var(--exact-gray-text)"></span>
      </div>`);
      for (const id of ['bare', 'area']) {
        await page.keyboard.press('Tab');
        expect(await page.evaluate(() => document.activeElement.id)).toBe(id);
        const outline = await page.$eval(`#${id}`, el => {const s=getComputedStyle(el);return [s.outlineStyle,s.outlineWidth,s.outlineOffset];});
        expect(outline[0]).not.toBe('none');
        expect(outline[1]).not.toBe('0px');
        expect(outline[2]).toBe('2px');
      }
      const color = id => page.$eval(`#${id}`, el => getComputedStyle(el).color);
      const gray = await color('gray');
      expect(gray).not.toBe('rgb(255, 0, 0)'); // the sheet carries the role: the probe does not inherit the page's red
      expect(await color('native')).toBe(gray);
      expect(await color('native-area')).toBe(gray);
      expect(await color('authored')).toBe('rgb(0, 128, 0)');
      // Source mode retains the Markdown editor's existing focus treatment.
      await page.keyboard.press('Tab');
      expect(await page.$eval('#markdown', el => getComputedStyle(el).outlineStyle)).toBe('none');
      // `outline="none"` as the live host writes it (inline) and the JS
      // target does (its class): the author's row outranks the restore and,
      // on a native field, the UA's ring; Tab still reaches each.
      for (const id of ['quiet', 'quiet-area', 'quiet-native']) {
        await page.keyboard.press('Tab');
        expect(await page.evaluate(() => document.activeElement.id)).toBe(id);
        expect(await page.$eval(`#${id}`, el => el.matches(':focus-visible') && getComputedStyle(el).outlineStyle)).toBe('none');
      }
    }
  } finally { await browser.close(); }
}, 20000);

for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: native buttons keep the UA focus ring and bare buttons restore it`, async () => {
  const {readFileSync} = await import('node:fs');
  const {withLook} = await import('../look.mjs');
  const browser = await suiteBrowser(engine);
  try {
    const page = await browser.newPage();
    const reference = await browser.newPage();
    const css = withLook(readFileSync(new URL('../index.html', import.meta.url), 'utf8')).match(/<style>([\s\S]*?)<\/style>/)[1];
    const tab = engine === 'webkit' ? 'Alt+Tab' : 'Tab';
    const outline = (page, id) => page.$eval(`#${id}`, el => {
      const s = getComputedStyle(el);
      return [s.outlineStyle, s.outlineWidth, s.outlineOffset, s.outlineColor, s.boxShadow];
    });
    for (const scheme of ['light', 'dark']) {
      await reference.setContent(`<style>html {color-scheme:${scheme}} button {color:inherit;font:inherit;letter-spacing:inherit}</style><button id="ua">UA</button>`);
      await page.setContent(`<style>${css} html {color-scheme:${scheme}}</style><div id="exact-root" style="color-scheme:${scheme}">
        <button id="native" data-native data-button-style="bordered">Native</button>
        <button id="bare">Bare</button><button id="disabled" data-native disabled>Disabled</button>
        <input id="last">
      </div>`);
      for (const p of [reference, page]) await p.evaluate(() => {
        window.presses = 0;
        document.addEventListener('click', e => { if (e.target.tagName === 'BUTTON') window.presses++; });
      });
      // The page's `bordered` look writes the accent as the label's ink (LLP 1116 D1);
      // an engine whose ring is `currentColor` draws the UA ring in that ink.
      const ink = await page.$eval('#native', el => getComputedStyle(el).color);
      await reference.$eval('#ua', (el, ink) => { el.style.color = ink; }, ink);
      await reference.keyboard.press(tab);
      await page.keyboard.press(tab);
      expect(await page.evaluate(() => document.activeElement.id)).toBe('native');
      expect(await page.$eval('#native', el => el.matches(':focus-visible'))).toBe(true);
      expect(await outline(page, 'native')).toEqual(await outline(reference, 'ua'));
      expect((await outline(page, 'native'))[0]).not.toBe('none');
      for (const key of ['Enter', 'Space']) {
        await page.keyboard.press(key);
        await reference.keyboard.press(key);
      }
      expect(await page.evaluate(() => window.presses)).toBe(2);
      expect(await reference.evaluate(() => window.presses)).toBe(2);
      await page.keyboard.press(tab);
      expect(await page.evaluate(() => document.activeElement.id)).toBe('bare');
      const bare = await outline(page, 'bare');
      expect(bare[0]).not.toBe('none');
      expect(bare[1]).not.toBe('0px');
      expect(bare[2]).toBe('2px');
      await page.keyboard.press(tab);
      expect(await page.evaluate(() => document.activeElement.id)).toBe('last');
    }
  } finally { await browser.close(); }
}, 30000);

// #282: the real HTML dialog path, reached through the JS target's commands.
test('action dialog commands keep modal focus and preserve bare close as window close', async () => {
  const {readFileSync} = await import('node:fs');
  const browser = await suiteBrowser();
  try {
    const page = await browser.newPage();
    await page.setContent('<div id="exact-root"><button id="opener">Open</button><button id="outside">Outside</button><dialog id="form"><button id="first">First</button><button id="last">Last</button></dialog><div id="box"></div></div><dialog id="other"></dialog>');
    const source = readFileSync(new URL('../../web-js/commands.js', import.meta.url), 'utf8').replace(/^import .*$/gm, '').replace('export const commands', 'const commands');
    // Of commands.js's imports the dialog commands reach only navigation.js's `liveById`; nothing leaves here, so
    // finding by id is enough.
    await page.addScriptTag({content: `const liveById = id => document.getElementById(id);\n${source}\nwindow.journal=[];window.quit=0;window.close=()=>quit++;window.hostCommands=commands(line=>journal.push(line));`});
    const glue = readFileSync(new URL('../glue.js', import.meta.url), 'utf8');
    const runtime = readFileSync(new URL('../../web-js/rt.js', import.meta.url), 'utf8');
    await page.addScriptTag({content: `(() => {
      const el = document.querySelector('#form'), views = new Map([[1, el]]), retiredViews = new Set(), inputReady = true, page = null;
      window.wasmClosed = []; window.jsClosed = [];
      const now = () => 0, send = () => {}, wasm = {exact_dispatch: (id, kind) => wasmClosed.push([id, kind])};
      ${glue.slice(glue.indexOf('function attach('), glue.indexOf('function viewFor('))}
      attach(el, 1, ['close']);
      const clock = {agent: true, now: 0, timers: []}, start = 0, Mutations = [], OnHooks = {}, Owner = null;
      const onEnd = () => {}; let Propagating = null;
      ${runtime.slice(runtime.indexOf('export function on('), runtime.indexOf('// Each event family')).replace('export function', 'function')}
      on(el, 'close', () => jsClosed.push(document.activeElement.id));
    })();`});
    const closed = async n => {
      await page.waitForFunction(n => wasmClosed.length === n && jsClosed.length === n, n);
      expect(await page.evaluate(() => wasmClosed.every(([id, kind]) => id === 1 && kind === 49))).toBe(true);
      expect(await page.evaluate(() => jsClosed.every(id => id === 'opener'))).toBe(true);
    };
    await page.focus('#opener');
    await page.evaluate(() => hostCommands.showModal('form'));
    expect(await page.evaluate(() => [document.querySelector('#form').matches(':modal'), document.activeElement.id])).toEqual([true, 'first']);
    await page.evaluate(() => hostCommands.showModal('form'));
    expect(await page.evaluate(() => journal.splice(0))).toEqual(['showModal "form" ignored: dialog is already open']);
    const trace = async () => {
      const focus = [];
      for (const key of ['Shift+Tab', 'Tab', 'Tab', 'Tab']) {
        await page.keyboard.press(key);
        focus.push(await page.evaluate(() => document.activeElement.id));
      }
      return focus;
    };
    const actual = await trace();
    await page.evaluate(() => { hostCommands.close('form'); hostCommands.close('form'); });
    await closed(1);
    await page.focus('#opener');
    await page.evaluate(() => document.querySelector('#form').showModal());
    const reference = await trace();
    expect(actual).toEqual(reference); // Chrome includes its own browser-focus boundary.
    expect(actual).not.toContain('outside');
    expect(actual).toContain('first');
    expect(actual).toContain('last');
    await page.evaluate(() => document.querySelector('#first').focus());
    await page.focus('#outside');
    expect(await page.evaluate(() => document.activeElement.id)).toBe('first');
    await page.evaluate(() => hostCommands.close('form'));
    expect(await page.evaluate(() => [document.querySelector('#form').open, document.activeElement.id, quit])).toEqual([false, 'opener', 0]);
    await closed(2);
    await page.evaluate(() => hostCommands.showModal('form'));
    await page.keyboard.press('Escape');
    expect(await page.evaluate(() => [document.querySelector('#form').open, document.activeElement.id])).toEqual([false, 'opener']);
    await closed(3);
    await page.evaluate(() => { hostCommands.showModal('box'); hostCommands.showModal('missing'); hostCommands.close(); });
    expect(await page.evaluate(() => [journal.length, quit])).toEqual([2, 1]);
    expect(await page.evaluate(() => journal.every(line => line.includes('refused')))).toBe(true);
  } finally { await browser.close(); }
}, 60000);

test('a queued dialog close cannot act after its conditional scope ends', async () => {
  const server = Bun.serve({port: 0, hostname: '127.0.0.1', async fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/') return new Response('<div id="exact-root"><button id="opener">Open</button><div id="branch"></div></div>', {headers: {'Content-Type': 'text/html'}});
    if (path === '/host/web-js/names.js') return new Response('export const sourceTypes = [];', {headers: {'Content-Type': 'text/javascript'}});
    if (!path.startsWith('/host/')) return new Response(null, {status: 404});
    const local = Bun.file(new URL(`../../..${path}`, import.meta.url));
    const file = await local.exists() ? local : Bun.file(new URL(`../../..${path.replace('/web-js/', '/web/')}`, import.meta.url));
    return await file.exists() ? new Response(file) : new Response(null, {status: 404});
  }});
  const browser = await suiteBrowser();
  try {
    const page = await browser.newPage();
    for (const retain of [false, true]) {
      await page.goto(server.url.href);
      const result = await page.evaluate(async retain => {
        const {scope, when, sig, write, commit, on} = await import('/host/web-js/rt.js');
        const mounted = sig(true), opener = document.querySelector('#opener');
        let closed = 0, dialog;
        scope(() => when(document.querySelector('#branch'), mounted, parent => {
          dialog = document.createElement('dialog');
          dialog.innerHTML = '<button autofocus>Close</button>';
          parent.append(dialog);
          on(dialog, 'close', () => closed++);
        }));
        opener.focus();
        dialog.showModal();
        opener.addEventListener('focus', () => commit(() => write(mounted.n, false)), {once: true});
        const delivered = new Promise(resolve => dialog.addEventListener('close', resolve, {once: true}));
        dialog.close();
        const detached = !dialog.isConnected;
        // An exit animation can retain a retired element in the DOM.
        if (retain) document.body.append(dialog);
        await delivered;
        return {closed, detached, focused: document.activeElement.id};
      }, retain);
      expect(result).toEqual({closed: 0, detached: true, focused: 'opener'});
    }
  } finally { await browser.close(); server.stop(true); }
}, 60000);

test('autofocus is deferred and consumed per mounted control, preserving other UI focus', async () => {
  const { readFileSync } = await import('node:fs');
  const { runInNewContext } = await import('node:vm');
  const source = readFileSync(new URL('../navigation.js', import.meta.url), 'utf8');
  const start = source.indexOf('export function focusController');
  const fn = source.slice(start, source.indexOf('\n}\n', start) + 2).replace('export ', '');
  const element = () => ({exactAutofocus:true, getClientRects:()=>[{}], matches:()=>false,
    setAttribute() {}, focus() { document.activeElement = this; }});
  const document = {body:{}, activeElement:null};
  const first = element(), other = element();
  const views = new Map([[1,first]]);
  const context = {document, views, root:{querySelectorAll:()=>[...views.values()]}, inputReady:true,
    inertAncestor:()=>false, getComputedStyle:()=>({visibility:'visible'})};
  const focus = runInNewContext(fn+';focusController({ready:()=>inputReady,elements:()=>views.values(),inert:inertAncestor}).autofocus', context);
  document.activeElement = other;
  focus();
  expect(document.activeElement, 'existing focus must win').toBe(other);
  document.activeElement = document.body;
  focus();
  expect(document.activeElement, 'a refused mounted control stays consumed').toBe(document.body);
  const later = element();
  views.set(1, later);
  context.inputReady = false;
  focus();
  expect(document.activeElement, 'input readiness gates autofocus').toBe(document.body);
  context.inputReady = true;
  focus();
  expect(document.activeElement, 'a later mount takes absent focus').toBe(later);
  const victory = element();
  const canvas = {matches:selector=>selector === '[data-gpu-input]', contains:el=>el===victory};
  document.activeElement = canvas;
  views.set(2, victory);
  focus();
  expect(document.activeElement, 'a canvas yields raw input focus to its new UI control').toBe(victory);
});

test('applying autofocus props cannot trigger browser focus during a batch', async () => {
  const { readFileSync } = await import('node:fs');
  const { runInNewContext } = await import('node:vm');
  const source = readFileSync(new URL('../glue.js', import.meta.url), 'utf8');
  const fn = source.slice(source.indexOf('function applyProps('), source.indexOf('function ensureMessageListener('));
  class Element {}
  const apply = runInNewContext(fn+';applyProps', {syncMedia:()=>{}, syncMarkup:()=>{}, settleValue:()=>{}, inputReady:true, HTMLIFrameElement:Element, HTMLImageElement:Element, HTMLVideoElement:Element});
  const attrs = new Map();
  const el = {setAttribute:(k,v)=>attrs.set(k,v), removeAttribute:k=>attrs.delete(k)};
  apply(el, {autofocus:'true'}, []);
  expect(attrs.has('autofocus')).toBe(false);
  expect(el.exactAutofocus).toBe(true);
  apply(el, {}, ['autofocus']);
  expect(el.exactAutofocus).toBe(false);
});
