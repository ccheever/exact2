import { removeReviewCommentChip } from '../../src/composer/composer-editor';
import { removeAttachmentReferences } from '../../src/composer/composer-attachments';
import { prGuideTaskHandoff } from '../../src/pull-requests/pages-pr-handoffs';
import { snapshotOps } from '../../src/client/client-ops-snapshot';
import type { Files } from '../../src/client/protocol';
import { adoptAppFiles } from '../../src/media/save-file';
import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { composerFieldAcknowledged, composerFieldNative, composerFieldObserved, composerFieldResource } from '../../src/composer/composer-field-bridge';
import { requestComposerField } from '../../src/composer/composer-field-request';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { editorLocal } from '../../src/composer/composer-editor';
import { stashEntries } from '../../src/composer/composer-editor-stash';

function setup(value = 'old') {
  const client = new T3Client(), owner = composerFieldOwner(client), initiating: unknown[] = [], acknowledgments: unknown[] = [], watches: string[] = [];
  client.local.drafts[client.draftKey] = value;
  const raw = (calls: unknown[]): Native => ({ available: true, watch(topic) { watches.push(topic); }, async later(request) { calls.push(request); return {ok:true,generation:client.generation,value:{}}; } });
  const first = raw(initiating), current = raw(acknowledgments);
  let observed = { generation: client.generation, snapshotOwner: client.snapshotOwner, owner, value, start: value.length, end: value.length, live: true, composing: false, focused: true, inputSerial: 0 };
  composerFieldResource(client, [observed, 0], owner, first);
  const native = composerFieldNative(client, first, () => composerFieldOwner(client));
  const next = (after = 0) => composerFieldResource(client, [observed, after], owner, current);
  const input = (text: string) => { observed = {...observed, value:text, start:text.length,end:text.length,inputSerial:observed.inputSerial+1}; client.local.drafts[client.draftKey]=text; composerFieldObserved(client,[observed,true],owner); };
  return {client,owner,native,current,initiating,acknowledgments,watches,next,input, observation:()=>observed,
    ack:(seq:number, applied=true)=>composerFieldAcknowledged(client,[observed,seq,applied],owner,current)};
}

test('completed resource watches actual wake; queued admission never runs an effect or invents application', async () => {
  const f=setup(); let applied=0;
  const admission=await requestComposerField(f.native,{op:'editorEdit',all:true,text:'new'},{applied:async native=>{applied++;await native.later({op:'cleanup'});}});
  expect(admission).toMatchObject({accepted:true,queued:true,applied:false,seq:1}); expect(applied).toBe(0); expect(f.client.draft).toBe('old');
  expect(f.watches).toContain('t3.notify'); expect(f.initiating).toEqual([{op:'wake',topic:'t3.notify'}]);
  const command=f.next(); expect(command.ready).toBe(true); f.input('new');
  expect(applied).toBe(0); expect((await f.ack(command.seq)).applied).toBe(true);
  expect(applied).toBe(1); expect(f.acknowledgments).toEqual([{op:'cleanup'}]);
  await f.ack(command.seq); expect(applied).toBe(1);
});

test('early ACK uses its current native once after independent input, never the initiating ticket', async () => {
  const f=setup();let effects=0;
  await requestComposerField(f.native,{op:'editorEdit',all:true,text:'new'},{applied:async native=>{effects++;await native.later({op:'cleanup'});}});
  const command=f.next(),calls:unknown[]=[];
  const current:Native={available:true,watch(){},async later(request){calls.push(request);if((request as {op:string}).op==='wake')f.input('new');return{ok:true,generation:f.client.generation,value:{}};}};
  const ack=await composerFieldAcknowledged(f.client,[f.observation(),command.seq,true],f.owner,current);
  expect(ack.applied).toBe(true);expect(f.client.error).toBe('');expect(effects).toBe(1);
  expect(calls).toEqual([{op:'wake',topic:'t3.notify'},{op:'cleanup'}]);
  await f.ack(command.seq);expect(effects).toBe(1);expect(f.initiating).toEqual([{op:'wake',topic:'t3.notify'}]);
});

test('manual source and owner loss during an early ACK still refuse each receipt once', async () => {
  for(const invalidation of ['source','owner','owner-rejected-wake']){
    const f=setup();let applied=0,refused=0,wakes=0;
    await requestComposerField(f.native,{op:'editorEdit',all:true,text:'new'},{applied:()=>{applied++;},refused:()=>{refused++;}});
    const command=f.next(),current:Native={available:true,watch(){},async later(){wakes++;if(invalidation==='source')f.input('manual');else{f.client.generation++;if(invalidation==='owner-rejected-wake')throw Error('superseded');f.next();}return{ok:true,generation:f.client.generation,value:{}};}};
    expect((await composerFieldAcknowledged(f.client,[f.observation(),command.seq,true],f.owner,current)).applied).toBe(false);
    expect(wakes).toBe(1);expect(applied).toBe(0);expect(refused).toBe(1);
    await f.ack(command.seq,false);expect(refused).toBe(1);
    if(invalidation!=='source')expect(f.client.error).toBe('');else expect(f.client.error).toContain('prompt changed');
  }
});

test('a bounded early ACK without input never invents application and retires with a specific missing-input diagnostic', async () => {
  const f=setup();let effects=0,refused=0,wakes=0;
  await requestComposerField(f.native,{op:'editorEdit',all:true,text:'new'},{applied:()=>{effects++;},refused:()=>{refused++;}});
  const command=f.next(),current:Native={available:true,watch(){},async later(){wakes++;return{ok:true,generation:f.client.generation,value:{}};}};
  const ack=await composerFieldAcknowledged(f.client,[f.observation(),command.seq,true],f.owner,current);
  expect(ack.applied).toBe(false);expect(effects).toBe(0);expect(refused).toBe(1);expect(wakes).toBeGreaterThan(0);expect(wakes).toBeLessThanOrEqual(16);
  expect(f.client.error).toBe('The editor did not acknowledge this edit. Please try again.');
  expect(f.next(command.seq).ready).toBe(false);expect((await requestComposerField(f.native,{op:'editorEdit',text:'!'})).accepted).toBe(true);
});

test('refused ACK and source/owner invalidation drain completion metadata once without external work', async () => {
  for (const invalidation of ['ack','source','owner']) {
    const f=setup(); let applied=0,refused=0;
    await requestComposerField(f.native,{op:'editorEdit',text:'!'},{applied:()=>{applied++;},refused:()=>{refused++;}});
    const command=f.next();
    if(invalidation==='ack') await f.ack(command.seq,false);
    else if(invalidation==='source') f.input('manual');
    else {f.client.generation++;f.next();}
    expect(applied).toBe(0);expect(refused).toBe(1);expect(f.acknowledgments).toEqual([]);
    await f.ack(command.seq,false);expect(refused).toBe(1);
  }
});

test('bounded refusal drains survive one throwing state-only callback', async () => {
  const f=setup();let refused=0;
  for(let i=0;i<16;i++) expect((await requestComposerField(f.native,{op:'editorEdit',text:'!'},{refused:()=>{refused++;if(i===0)throw Error('test refusal');}})).accepted).toBe(true);
  expect((await requestComposerField(f.native,{op:'editorEdit',text:'!'})).accepted).toBe(false);
  f.input('manual');expect(refused).toBe(16);f.input('manual2');expect(refused).toBe(16);
});

test('a finite cleanup that loses its owner cannot perform later external work or poison the replacement field', async () => {
  const f=setup();let external=0;
  await requestComposerField(f.native,{op:'editorEdit',all:true,text:'new',then:'models'},{applied:async native=>{
    await native.later({op:'first-cleanup'});external++;f.client.generation++;await native.later({op:'second-cleanup'});
  }});
  const command=f.next();f.input('new');const ack=await f.ack(command.seq);
  expect(external).toBe(1);expect(f.acknowledgments).toEqual([{op:'first-cleanup'}]);expect(f.client.error).toBe('');expect(ack.then).toBe('');expect(ack.send).toBe(false);
});

test('stash restore remains available until actual input and ACK, and remains after refusal', async () => {
  for(const accepted of [false,true]) {
    const f=setup('');(f.client.local as {promptStash?:unknown[]}).promptStash=[{id:'entry',prompt:'saved',createdAt:'2026-01-01',environmentId:''}];
    await editorLocal(f.client,f.native,'stash-restore','entry','');expect(stashEntries(f.client.local)).toHaveLength(1);expect(f.client.draft).toBe('');
    const command=f.next();if(accepted)f.input('saved');await f.ack(command.seq,accepted);
    expect(stashEntries(f.client.local)).toHaveLength(accepted?0:1);
  }
});

test('cleanup failure and same-source input ABA never publish focus/removal/then completion', async () => {
  for (const outcome of ['failure','input-ABA']) {
    const f=setup();
    await requestComposerField(f.native,{op:'editorEdit',all:true,text:'new',then:'models'},{focus:'next-image',removedImage:'image',applied:async native=>{
      await native.later({op:'cleanup'});
      if(outcome==='failure')throw Error('Cleanup failed');
      f.input('new!');f.input('new');
    }});
    const command=f.next();f.input('new');const ack=await f.ack(command.seq);
    expect(ack).toMatchObject({applied:true,focus:'',removedImage:'',then:'',send:false,inputSerial:-1});
  }
});


test('image removal preserves shelf/bytes before ACK and on refusal; applied ACK persists before releasing bytes and returns focus metadata', async () => {
  for (const applied of [false,true]) {
    const f=setup('![shot](t3-context://v1/image/image_shot)');
    f.client.local.snapshotDrafts[f.client.draftKey]=[{id:'shot',name:'shot.png'}];
    // The app's storage (Exact's storage.fs): its save of the preference file is recorded among the acknowledgment's requests.
    adoptAppFiles({fs:{async readFile(){return new ArrayBuffer(0);},async writeFile(){},async mkdir(){},async atomicWriteFile(){f.acknowledgments.push({op:'savePreferences'});}}});
    const out={message:'',id:'',value:''};
    await snapshotOps.call(f.client,'remove-snapshot','shot','next-image',0,f.native,{} as Files,out);
    expect(f.client.snapshotDrafts).toHaveLength(1);expect(f.acknowledgments).toEqual([]);expect(out.message).toBe('');
    const command=f.next();if(applied)f.input('');const ack=await f.ack(command.seq,applied);
    expect(f.client.snapshotDrafts).toHaveLength(applied?0:1);
    const ops=f.acknowledgments.map(request=>(request as {op:string}).op);
    if(applied) {
      expect(ops.indexOf('savePreferences')).toBeGreaterThanOrEqual(0);
      expect(ops.indexOf('savePreferences')).toBeLessThan(ops.indexOf('snapshotDraftRemove'));
      expect(ack).toMatchObject({focus:'next-image',removedImage:'shot',inputSerial:1});
    } else { expect(ops).toEqual([]);expect(ack.focus).toBe(''); }
  }
});


test('refused image edit cannot masquerade as immediate successful removal/focus', async () => {
  const f=setup('![shot](t3-context://v1/image/image_shot)');
  f.client.local.snapshotDrafts[f.client.draftKey]=[{id:'shot',name:'shot.png'}];
  composerFieldObserved(f.client,[{...f.observation(),composing:true},false],f.owner);
  const out={message:'',id:'',value:''};
  await expect(snapshotOps.call(f.client,'remove-snapshot','shot','next-image',0,f.native,{} as Files,out)).rejects.toThrow('The prompt changed before attachment references could be removed.');
  expect(out.message).toBe('');expect(f.client.snapshotDrafts).toHaveLength(1);expect(f.acknowledgments).toEqual([]);
});


test('foreground attachment removal reads the live field ahead of storage before considering no-edit cleanup', async () => {
  const f=setup('latest ![shot](t3-context://v1/image/image_shot) text');
  f.client.local.drafts[f.client.draftKey]='stale without the chip';let finished=0;
  expect(await removeAttachmentReferences(f.client,f.native,f.client.draftKey,'image','shot',{applied:()=>{finished++;}})).toBe(true);
  expect(finished).toBe(0);const command=f.next();
  expect(command).toMatchObject({value:f.observation().value,text:'latest text'});
  f.input(command.text);await f.ack(command.seq);expect(finished).toBe(1);
});

test('foreground review removal and PR handoff prepare from actual field source and guard exact admission source', async () => {
  for(const kind of ['review','handoff']) for(const race of [false,true]) {
    const source=kind==='review'?'latest [Review](t3-context://v1/review-comment/ctx) text':'latest sentence';
    const f=setup(source);f.client.local.drafts[f.client.draftKey]='stale';
    const native:Native=race?{...f.native,later:request=>{if((request as {op?:string}).op==='editorEdit')f.input('intervening source');return f.native.later(request);}}:f.native;
    const attempt=kind==='review'?removeReviewCommentChip(f.client,native,'ctx'):prGuideTaskHandoff(f.client,native,{},'thread',{prompt:'Explain now'});
    if(race) {
      await expect(attempt).rejects.toThrow('The prompt could not be updated.');expect(f.next().ready).toBe(false);expect(f.client.draft).toBe('intervening source');
    } else {
      await attempt;const command=f.next();expect(command.value).toBe(source);
      expect(command.text).toBe(kind==='review'?'latest text':'latest sentence\n\nExplain now');
      expect(f.client.draft).toBe('stale');
    }
  }
});


test('review removal ACK does not delete metadata after Undo restores its source link', async () => {
  const source='latest [Review](t3-context://v1/review-comment/ctx) text',f=setup(source);let removed=0;
  await removeReviewCommentChip(f.client,f.native,'ctx',{applied:()=>{removed++;}});
  const command=f.next();f.input(command.text);f.input(source);const ack=await f.ack(command.seq);
  expect(ack.applied).toBe(true);expect(removed).toBe(0);expect(f.client.draft).toBe(source);expect(f.client.error).toBe('The review comment is still referenced by the prompt.');
});

test('pre-ACK later input, same-source Undo ABA and caret changes retain historical applied but publish no new UI effect', async () => {
  for(const change of ['text','ABA','caret']) {
    const f=setup();await requestComposerField(f.native,{op:'editorEdit',all:true,text:'new',then:'models'},{focus:'next',removedImage:'image'});
    const command=f.next();f.input('new');
    if(change==='text')f.input('new!');
    if(change==='ABA'){f.input('new!');f.input('new');}
    if(change==='caret')composerFieldObserved(f.client,[{...f.observation(),start:0,end:0},false],f.owner);
    const ack=change==='caret'?await composerFieldAcknowledged(f.client,[{...f.observation(),start:0,end:0},command.seq,true],f.owner,f.current):await f.ack(command.seq);
    expect(ack).toMatchObject({applied:true,then:'',focus:'',removedImage:'',send:false,inputSerial:-1});
  }
});
