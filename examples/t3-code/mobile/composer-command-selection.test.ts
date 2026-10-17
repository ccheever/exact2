// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileComposerMenuSelection as select} from './composer-command-selection';
import {mobileComposerTrigger, type ComposerCommandItem} from './composer-command-model';
const usage:ComposerCommandItem={id:'pcmd:usage-limits',type:'provider-slash-command',command:{name:'usage-limits'},label:'/usage-limits',description:''};
function input(text='/us suffix',caret=3){return {draftMessage:text,trigger:mobileComposerTrigger(text,{start:caret,end:caret},true,false)!,item:usage,allowInteractionMode:true,openUsageLimits:true}}
test('offered local Limits removes only the trigger with exact UTF16 caret and preserves suffix',()=>{
  for(const [text,caret,after,cursor] of [['/us suffix',3,' suffix',0],['😀\n/us trailing',6,'😀\n trailing',3],['before\n/us',10,'before\n',7]] as const)
    expect(select(input(text,caret))).toEqual({text:after,cursor,interactionMode:null,localCommand:'usage-limits'});
});
test('without a callback including attachment-bearing drafts Limits remains a provider command',()=>{
  expect(select({...input(),openUsageLimits:false})).toEqual({text:'/usage-limits  suffix',cursor:14,interactionMode:null,localCommand:null});
});
test('other provider commands never open Limits',()=>{
  const item:ComposerCommandItem={...usage,id:'pcmd:feedback',command:{name:'feedback'},label:'/feedback'};
  expect(select({...input(),item})).toEqual({text:'/feedback  suffix',cursor:10,interactionMode:null,localCommand:null});
});
