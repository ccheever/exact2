import {test,expect} from 'bun:test';
import {focusController} from '../navigation.js';

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
