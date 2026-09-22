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
