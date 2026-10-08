import {test,expect} from 'bun:test';
import {focusController} from '../navigation.js';
import {createInputHandlers} from '../input-glue.js';

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

// LLP 1104 D3/D6: exercise the shipped stylesheet in Chrome, beside UA controls.
test('bare fields restore keyboard focus and disabled native ink comes from the UA', async () => {
  const {readFileSync} = await import('node:fs');
  const {chromium: browserPath} = await import('../../../scripts/agent-launch.mjs');
  const {chromium} = await import('playwright-core');
  const browser = await chromium.launch({executablePath: browserPath().executable, headless: true});
  try {
    const page = await browser.newPage();
    const css = readFileSync(new URL('../index.html', import.meta.url), 'utf8').match(/<style>([\s\S]*?)<\/style>/)[1];
    for (const scheme of ['light', 'dark']) {
      await page.setContent(`<style>${css}</style><div id="exact-root" style="color:red;color-scheme:${scheme}">
        <input id="bare"><textarea id="area"></textarea><textarea id="markdown" markup="markdown"></textarea>
        <input id="native" data-native disabled value="Disabled"><textarea id="native-area" data-native disabled>Disabled</textarea>
        <input id="authored" data-native disabled style="color:rgb(0,128,0)">
      </div><iframe srcdoc='<style>html {color-scheme:${scheme}}</style><input id="ua" disabled><textarea id="ua-area" disabled></textarea>'></iframe>`);
      for (const id of ['bare', 'area']) {
        await page.keyboard.press('Tab');
        expect(await page.evaluate(() => document.activeElement.id)).toBe(id);
        const outline = await page.$eval(`#${id}`, el => {const s=getComputedStyle(el);return [s.outlineStyle,s.outlineWidth,s.outlineOffset];});
        expect(outline[0]).not.toBe('none');
        expect(outline[1]).not.toBe('0px');
        expect(outline[2]).toBe('2px');
      }
      const color = id => page.$eval(`#${id}`, el => getComputedStyle(el).color);
      const ua = id => page.$eval('iframe', (frame, id) => frame.contentWindow.getComputedStyle(frame.contentDocument.getElementById(id)).color, id);
      expect(await color('native')).toBe(await ua('ua'));
      expect(await color('native-area')).toBe(await ua('ua-area'));
      expect(await color('authored')).toBe('rgb(0, 128, 0)');
      // Source mode retains the Markdown editor's existing focus treatment.
      await page.keyboard.press('Tab');
      expect(await page.$eval('#markdown', el => getComputedStyle(el).outlineStyle)).toBe('none');
    }
  } finally { await browser.close(); }
}, 20000);
