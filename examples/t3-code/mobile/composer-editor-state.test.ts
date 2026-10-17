// Source365aa87982 revision behavior plus app-owned native mount admission.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { expect, test } from 'bun:test';
import * as editor from './composer-editor-state';
const at = (end:number) => ({start:end,end});

test('native acknowledgments reject old and noninteger counts', () => {
  expect(editor.acknowledgeComposerNativeEvent(7,6)).toBeNull();
  expect(editor.acknowledgeComposerNativeEvent(7,NaN)).toBeNull();
  expect(editor.acknowledgeComposerNativeEvent(7,7.5)).toBeNull();
  expect(editor.acknowledgeComposerNativeEvent(7,7)).toBe(7);
  expect(editor.acknowledgeComposerNativeEvent(7,8)).toBe(8);
});
test('lagged value and caret cannot acquire the latest native revision', () => {
  const snapshots=[{eventCount:1,value:'old',selection:at(3)},{eventCount:2,value:'new',selection:at(3)},
    {eventCount:3,value:'new',selection:at(1)}];
  expect(editor.resolveComposerControlledEventCount('old',at(3),3,snapshots)).toBe(1);
  expect(editor.resolveComposerControlledEventCount('new',at(3),3,snapshots)).toBe(2);
  expect(editor.resolveComposerControlledEventCount('new',at(2),3,snapshots)).toBe(2);
  expect(editor.resolveComposerControlledEventCount('new',at(1),3,snapshots)).toBe(3);
  expect(editor.resolveComposerControlledEventCount('external',at(8),3,snapshots)).toBe(3);
});
test('initial document is non-echo; assumed control keeps its originating revision', () => {
  expect(editor.isComposerNativeEcho('restored',at(8),0,[])).toBe(false);
  const snapshots=editor.assumeComposerControlledState([],0,'restored');
  expect(editor.resolveComposerControlledEventCount('restored',at(3),0,snapshots)).toBe(0);
  expect(editor.isComposerNativeEcho('restored',at(3),0,snapshots)).toBe(false);
  expect(editor.isComposerNativeEcho('restored',null,0,snapshots)).toBe(true);
});
test('assumed parent change cannot erase later native events or pin an old ABA value', () => {
  const snapshots=editor.assumeComposerControlledState([
    {eventCount:1,value:'A',selection:at(1)}, {eventCount:2,value:'B',selection:at(1)},
    {eventCount:4,value:'later',selection:at(5)}],3,'A');
  expect(snapshots).toEqual([{eventCount:3,value:'A',selection:null},{eventCount:4,value:'later',selection:at(5)}]);
  expect(editor.resolveComposerControlledEventCount('B',at(1),4,snapshots)).toBe(4);
  expect(editor.resolveComposerControlledEventCount('A',at(0),4,snapshots)).toBe(3);
});
test('pruning preserves latest acknowledged snapshot so unrelated renders remain echoes', () => {
  const snapshots=[{eventCount:1,value:'a',selection:at(1)},{eventCount:2,value:'ab',selection:at(2)},
    {eventCount:3,value:'abc',selection:at(3)}];
  const pruned=editor.pruneAcknowledgedComposerNativeEvents(snapshots,2);
  expect(pruned).toEqual(snapshots.slice(1));
  expect(editor.isComposerNativeEcho('ab',at(2),2,pruned)).toBe(true);
  expect(editor.isComposerNativeEcho('ab',at(1),2,pruned)).toBe(false);
  expect(snapshots).toHaveLength(3);
});

const identity:editor.ComposerEditorIdentity={owner:'draft',editorId:'thread-composer',routeVisit:'visit1',renderEpoch:'epoch1'};
const seed=()=>editor.mobileComposerEditorAdmit(null,identity,{value:'A',selection:at(1)});
const event=(patch:Partial<editor.ComposerEditorEvent>={}):editor.ComposerEditorEvent=>({
  ...identity,mountId:'mount1',eventCount:0,kind:'ready',value:'A',selection:at(1),composing:false,focused:false,...patch});
const observe=(state:editor.ComposerEditorState,patch:Partial<editor.ComposerEditorEvent>={})=>editor.mobileComposerEditorAccept(state,event(patch));
const stage=(state:editor.ComposerEditorState)=>editor.mobileComposerEditorStageEffect(state,state.latestEffect!.id);
const ready=()=>stage(observe(seed()).state).state;

test('ready binds only current epoch and mount; old ready cannot steal remount', () => {
  const state=seed();
  expect(observe(state,{renderEpoch:'old'}).accepted).toBe(false);
  expect(observe(state,{kind:'text',eventCount:1}).accepted).toBe(false);
  expect(observe(state,{value:'',selection:at(0)}).accepted).toBe(false);
  const accepted=observe(state);
  expect(accepted.accepted).toBe(true);
  expect(editor.mobileComposerEditorControlled(accepted.state).mountId).toBe('mount1');
  expect(observe(accepted.state,{mountId:'mount2'}).accepted).toBe(false);
  const replacement=editor.mobileComposerEditorAdmit(accepted.state,{...identity,renderEpoch:'epoch2'},{value:'B',selection:at(1)});
  expect(observe(replacement).accepted).toBe(false);
  expect(observe(replacement,{renderEpoch:'epoch2',mountId:'mount2',value:'B'}).accepted).toBe(true);
  expect(editor.mobileComposerEditorRetire(replacement)).toBeNull();
});

test('observed text and caret project before persistence; stale client draft cannot reseed', () => {
  let state=ready();
  const typed=observe(state,{kind:'text',eventCount:1,value:'😀AB',selection:at(4),focused:true});
  state=typed.state;
  expect(state.committedValue).toBe('A');
  expect(typed.effect?.writeText).toBe(true);
  expect(editor.mobileComposerEditorControlled(state)).toMatchObject({document:{value:'😀AB',selection:at(4)},acknowledgedEventCount:1});
  expect(editor.mobileComposerEditorAdmit(state,identity,{value:'A',selection:at(1)})).toBe(state);
  state=stage(state).state;
  const moved=observe(state,{kind:'selection',eventCount:2,value:'😀AB',selection:at(2),focused:true});
  expect(moved.effect?.writeText).toBe(false);
  expect(editor.mobileComposerEditorControlled(moved.state)).toMatchObject({document:{value:'😀AB',selection:at(2)},acknowledgedEventCount:2});
  const committed=editor.mobileComposerEditorCommitted(moved.state,typed.effect!);
  expect(committed.committedValue).toBe('😀AB');
  expect(committed.selection).toEqual(at(2));
});

test('same-count contradiction, unknown owner and invalid UTF16 ranges are rejected', () => {
  const state=ready();
  expect(observe(state).duplicate).toBe(true);
  expect(observe(state).effect).toBeNull();
  expect(observe(state,{kind:'text',value:'B'}).accepted).toBe(false);
  expect(observe(state,{focused:true}).accepted).toBe(false);
  expect(observe(state,{kind:'selection',eventCount:1,owner:'other'}).accepted).toBe(false);
  for(const selection of [{start:-1,end:0},{start:0,end:2},{start:1,end:0},{start:0.5,end:1}])
    expect(observe(state,{kind:'selection',eventCount:1,selection}).accepted).toBe(false);
  expect(editor.mobileComposerEditorDecodeEvent('{')).toBeNull();
  expect(observe(state,{kind:'submit',eventCount:1}).accepted).toBe(false);
});

test('latest event effect is claimed once; stale durable replies never overwrite new input', () => {
  const first=observe(ready(),{kind:'text',eventCount:1,value:'B'});
  const firstStaged=stage(first.state);
  const second=observe(firstStaged.state,{kind:'text',eventCount:2,value:'C'});
  expect(editor.mobileComposerEditorStageEffect(second.state,first.effect!.id).effect).toBeNull();
  const secondStaged=stage(second.state);
  expect(stage(secondStaged.state).effect).toBeNull();
  const committed=editor.mobileComposerEditorCommitted(secondStaged.state,second.effect!);
  expect(editor.mobileComposerEditorCommitted(committed,first.effect!)).toBe(committed);
  expect(committed.value).toBe('C');
  const another=editor.mobileComposerEditorAdmit(committed,{...identity,routeVisit:'visit2'},{value:'new',selection:at(3)});
  expect(editor.mobileComposerEditorCommitted(another,second.effect!)).toBe(another);
});

test('CAS has no prospective text/context side effect; matching applied terminal gates acknowledgment', () => {
  const initial=ready();
  const pending=editor.mobileComposerEditorCommand(initial,'command1',1,{value:'/model ',selection:at(7),tokensJson:'[]'});
  expect(pending.command?.expected).toEqual({eventCount:0,value:'A',selection:at(1)});
  expect(pending.state.value).toBe('A');
  expect(editor.mobileComposerEditorCommand(pending.state,'command2',2,{value:'x',selection:at(1),tokensJson:'[]'}).command).toBeNull();
  expect(observe(pending.state,{kind:'commandApplied',eventCount:1,value:'/model ',selection:at(7),commandId:'wrong',commandRevision:1,reason:''}).accepted).toBe(false);
  const result=observe(pending.state,{kind:'commandApplied',eventCount:1,value:'/model ',selection:at(7),commandId:'command1',commandRevision:1,reason:''});
  expect(result.commandEffect?.command?.commandId).toBe('command1');
  expect(result.effect?.writeText).toBe(true);
  expect(editor.mobileComposerEditorControlled(result.state).ackCommandId).toBe('');
  expect(editor.mobileComposerEditorStageEffect(result.state,result.commandEffect!.id).effect).toBeNull();
  const documentStaged=stage(result.state);
  const staged=editor.mobileComposerEditorStageEffect(documentStaged.state,result.commandEffect!.id);
  expect(editor.mobileComposerEditorControlled(staged.state).ackCommandId).toBe('command1');
  expect(staged.state.pendingCommand).toBeNull();
  expect(stage(staged.state).effect).toBeNull();
});

test('typing before native CAS rejection remains authoritative and cannot commit staged context', () => {
  const pending=editor.mobileComposerEditorCommand(ready(),'command1',1,{value:'replacement',selection:at(11),tokensJson:'[]'}).state;
  const typing=observe(pending,{kind:'text',eventCount:1,value:'AB',selection:at(2)});
  const typed=stage(typing.state).state;
  const rejected=observe(typed,{kind:'commandRejected',eventCount:2,value:'AB',selection:at(2),commandId:'command1',commandRevision:1,reason:'superseded'});
  expect(rejected.effect?.event.kind).toBe('commandRejected');
  expect(rejected.effect?.writeText).toBe(false);
  expect(stage(rejected.state).state.value).toBe('AB');
  expect(editor.mobileComposerEditorControlled(editor.mobileComposerEditorStageEffect(stage(rejected.state).state,rejected.commandEffect!.id).state).ackCommandId).toBe('command1');
});

test('composition suppresses command staging; identity-captured focus/submit are plain effects', () => {
  const composing=observe(ready(),{kind:'text',eventCount:1,composing:true}).state;
  expect(editor.mobileComposerEditorCommand(stage(composing).state,'c',1,{value:'x',selection:at(1),tokensJson:'[]'}).command).toBeNull();
  const focused=observe(ready(),{kind:'focus',eventCount:1,focused:true});
  expect(focused.effect?.event).toMatchObject({kind:'focus',owner:'draft',routeVisit:'visit1',focused:true});
  const submit=observe(stage(focused.state).state,{kind:'submit',eventCount:2,focused:true,alternate:true});
  expect(submit.effect?.event.alternate).toBe(true);
  expect(submit.effect?.writeText).toBe(false);
  expect(submit.state.committedEventCount).toBe(-1);
});

test('retained terminal survives latest-event latch, further typing and composition without replaying old text', () => {
  const pending=editor.mobileComposerEditorCommand(ready(),'c',1,{value:'picked',selection:at(6),tokensJson:'[]'}).state;
  const outcome:editor.ComposerCommandTerminal={kind:'commandApplied',commandId:'c',commandRevision:1,eventCount:1,
    value:'picked',selection:at(6),composing:false,focused:true,reason:''};
  // Native terminal event was overtaken before observation. Later envelope retains it.
  const later=observe(pending,{kind:'text',eventCount:2,value:'picked new',selection:at(10),focused:true,composing:true,pendingCommand:outcome});
  expect(later.accepted).toBe(true);
  expect(later.state.value).toBe('picked new');
  expect(later.commandEffect?.event.value).toBe('picked');
  expect(later.commandEffect?.writeText).toBe(false);
  expect(later.effect?.writeText).toBe(true);
  const changed=observe(later.state,{kind:'selection',eventCount:3,value:'picked new',selection:at(7),focused:true,composing:true,pendingCommand:outcome});
  expect(changed.accepted).toBe(true);
  expect(changed.commandEffect?.id).toBe(later.commandEffect?.id);
  const staged=stage(changed.state).state;
  const acknowledged=editor.mobileComposerEditorStageEffect(staged,changed.commandEffect!.id);
  expect(acknowledged.state.value).toBe('picked new');
  expect(acknowledged.state.selection).toEqual(at(7));
  expect(acknowledged.state.composing).toBe(true);
  expect(acknowledged.state.ackCommandId).toBe('c');
  expect(editor.mobileComposerEditorStageEffect(acknowledged.state,changed.commandEffect!.id).effect).toBeNull();
  const continued=observe(acknowledged.state,{kind:'text',eventCount:4,value:'done',selection:at(4),pendingCommand:outcome});
  expect(continued.accepted).toBe(true);
  expect(continued.commandEffect).toBeNull();
});

test('IME rejection retains outcome while composition continues, without a terminal editing barrier', () => {
  const pending=editor.mobileComposerEditorCommand(ready(),'c',1,{value:'B',selection:at(1),tokensJson:'[]'}).state;
  const outcome:editor.ComposerCommandTerminal={kind:'commandRejected',commandId:'c',commandRevision:1,eventCount:1,
    value:'A',selection:at(1),composing:true,focused:true,reason:'composing'};
  const rejected=observe(pending,{...outcome,pendingCommand:outcome});
  expect(rejected.accepted).toBe(true);
  const composing=observe(rejected.state,{kind:'text',eventCount:2,value:'Aa',selection:at(2),composing:true,focused:true,pendingCommand:outcome});
  expect(composing.accepted).toBe(true);
  expect(composing.commandEffect?.event.kind).toBe('commandRejected');
  const ack=editor.mobileComposerEditorStageEffect(stage(composing.state).state,composing.commandEffect!.id);
  expect(ack.state.composing).toBe(true);
  expect(ack.state.value).toBe('Aa');
  expect(ack.state.ackCommandId).toBe('c');
  const tampered={...outcome,reason:'different'};
  expect(observe(rejected.state,{kind:'text',eventCount:2,value:'Aa',selection:at(2),pendingCommand:tampered}).accepted).toBe(false);
});

test('R2 command IDs cannot be reused after acknowledgment, even after another command', () => {
  const issue=(state:editor.ComposerEditorState,id:string,revision:number,value:string,count:number)=>{
    const pending=editor.mobileComposerEditorCommand(state,id,revision,{value,selection:at(value.length),tokensJson:'[]'}).state;
    const applied=observe(pending,{kind:'commandApplied',eventCount:count,value,selection:at(value.length),commandId:id,commandRevision:revision,reason:''});
    expect(applied.accepted).toBe(true);
    return editor.mobileComposerEditorStageEffect(stage(applied.state).state,applied.commandEffect!.id).state;
  };
  let state=issue(ready(),'c1',1,'B',1);
  expect(editor.mobileComposerEditorCommand(state,'c1',2,{value:'C',selection:at(1),tokensJson:'[]'}).command).toBeNull();
  state=issue(state,'c2',2,'C',2);
  expect(editor.mobileComposerEditorCommand(state,'c1',3,{value:'D',selection:at(1),tokensJson:'[]'}).command).toBeNull();
  expect(state.pendingCommand).toBeNull();
  const fresh=editor.mobileComposerEditorAdmit(state,{...identity,renderEpoch:'new'},{value:'A',selection:at(1)});
  expect(fresh.issuedCommandIds).toEqual([]);
});

test('R2 retained terminal must advance beyond the captured expected count', () => {
  const current=stage(observe(ready(),{kind:'selection',eventCount:5}).state).state;
  const pending=editor.mobileComposerEditorCommand(current,'c',1,{value:'B',selection:at(1),tokensJson:'[]'}).state;
  for(const count of [1,5]) {
    const outcome:editor.ComposerCommandTerminal={kind:'commandApplied',commandId:'c',commandRevision:1,eventCount:count,
      value:'B',selection:at(1),composing:false,focused:false,reason:''};
    expect(observe(pending,{kind:'text',eventCount:6,value:'C',pendingCommand:outcome}).accepted).toBe(false);
  }
});

test('R2 normal event cannot claim the exact terminal count with a conflicting snapshot', () => {
  const pending=editor.mobileComposerEditorCommand(ready(),'c',1,{value:'B',selection:at(1),tokensJson:'[]'}).state;
  const outcome:editor.ComposerCommandTerminal={kind:'commandApplied',commandId:'c',commandRevision:1,eventCount:1,
    value:'B',selection:at(1),composing:false,focused:false,reason:''};
  expect(observe(pending,{kind:'text',eventCount:1,value:'C',pendingCommand:outcome}).accepted).toBe(false);
  expect(observe(pending,{...outcome,pendingCommand:{...outcome,value:'C'}}).accepted).toBe(false);
  expect(observe(pending,{...outcome,pendingCommand:outcome}).accepted).toBe(true);
  expect(observe(pending,{kind:'text',eventCount:2,value:'C',pendingCommand:outcome}).accepted).toBe(true);
});
