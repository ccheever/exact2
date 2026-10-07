// @ref llp/1107.010-mobile-browser-devices.decision.md#connection-and-command-ownership
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileStreamOwner } from './browser-mobile-owner';
import { applyBrowserEvent, applyBrowserList, emptyBrowserTabs, normalizeMobilePreviewUrl } from './browser-mobile-model';
import { mobileBrowserRead, mobileBrowserSnapshot, mobileBrowserAction, mobileBrowserEvents, mobileBrowserStatus, BROWSER_EVENTS } from './browser-mobile-data';

function fixture() {
  const client = new T3Client(); client.origin = 'https://preview.test'; client.environmentId = 'env'; client.threadId = 'thread'; client.generation = 4;
  client.connection = 'connected'; client.configLive = client.shellLive = client.threadLive = true;
  const calls: Obj[] = [], grants = ['orchestration:read', 'preview:operate'];
  let tabs: Obj[] = [{ threadId: 'thread', tabId: 'one', runtime: 'server', navStatus: { _tag: 'Loaded', title: 'One', url: 'https://one.test' }, updatedAt: '2026-01-01', canGoBack: true, canGoForward: true }];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: client.generation, value: request.op === 'http' ? { authenticated: true, permissions: grants, scopes: ['orchestration:read'] }
      : request.op === 'subscribe' ? { id: '4-7' } : request.method === 'preview.list' ? { serverEpoch: 'epoch', revision: 1, sessions: tabs }
      : request.action === 'status' ? { tabId: 'one', status: 'streaming', control: { canOperate: true, controller: 'you' } } : {} };
  } };
  return { client, native, calls, grants, owner: mobileStreamOwner(client), setTabs(value: Obj[]) { tabs = value; } };
}
test('source list/event replay preserves newer navigation and stable tab order', () => {
  const event = { revision: 3, serverEpoch: 'a', tabId: 'one', type: 'changed', snapshot: { tabId: 'one', runtime: 'server', updatedAt: 'new' } };
  const state = applyBrowserList({ serverEpoch: 'a', revision: 2, sessions: [{ tabId: 'one', updatedAt: 'old' }, { tabId: 'two' }] }, [event]);
  expect(state.sessions.map(row => row.tabId)).toEqual(['one', 'two']); expect(state.sessions[0]?.updatedAt).toBe('new');
  expect(applyBrowserEvent(state, { revision: 4, serverEpoch: 'a', tabId: 'one', type: 'closed' }).sessions).toEqual([{ tabId: 'two' }]);
  expect(applyBrowserEvent(emptyBrowserTabs(), { revision: 0 })).toEqual(emptyBrowserTabs());
});
test('selected fallback stays pinned despite newer activity; epochs demand authoritative refresh', async () => {
  const f = fixture(); await mobileBrowserRead(f.owner, f.native, f.client);
  expect(mobileBrowserSnapshot(f.owner, f.client).title).toBe('One');
  mobileBrowserEvents([{ generation: 4, key: BROWSER_EVENTS, subscriptionId: '4-7', value: { threadId: 'thread', serverEpoch: 'epoch', revision: 3, tabId: 'two', type: 'changed', snapshot: { tabId: 'two', runtime: 'server', navStatus: { _tag: 'Loaded', title: 'Two' }, updatedAt: '2099' } } }], f.client);
  expect(mobileBrowserSnapshot(f.owner, f.client).title).toBe('One'); expect(mobileBrowserSnapshot(f.owner, f.client).count).toBe(2);
  mobileBrowserEvents([{ generation: 4, key: BROWSER_EVENTS, subscriptionId: '4-7', value: { threadId: 'thread', serverEpoch: 'new', revision: 1, tabId: 'one', type: 'closed' } }], f.client);
  expect(mobileBrowserSnapshot(f.owner, f.client).needsRefresh).toBe(true); expect(mobileBrowserSnapshot(f.owner, f.client).count).toBe(2);
});
test('explicit empty permissions deny reads even when legacy scopes allow them', async () => {
  const f = fixture(); f.grants.length = 0; const data = await mobileBrowserRead(f.owner, f.native, f.client);
  expect(data.error).toContain('cannot view'); expect(f.calls.some(call => call.op === 'subscribe')).toBe(false);
});
test('history uses exact stream command and adjusts use real owning thread RPC', async () => {
  const f = fixture(); await mobileBrowserRead(f.owner, f.native, f.client); await mobileBrowserStatus(f.owner, f.native, f.client);
  await mobileBrowserAction(f.owner, 'back', '', f.native, f.client);
  expect(f.calls.find(call => call.action === 'command')?.input).toEqual({ type: 'history', delta: -1 });
  await mobileBrowserAction(f.owner, 'appearance:dark', '', f.native, f.client);
  expect(f.calls.find(call => call.method === 'preview.adjust')?.payload).toEqual({ threadId: 'thread', tabId: 'one', colorScheme: 'dark' });
});
test('late permission reply cannot subscribe or mutate a newly selected thread', async () => {
  const f = fixture(); let release!: () => void; const gate = new Promise<void>(resolve => { release = resolve; });
  const native: Native = { ...f.native, async later(input) { if (obj(input).op === 'http') await gate; return f.native.later(input); } };
  const answer = mobileBrowserRead(f.owner, native, f.client); f.client.threadId = 'other'; release(); await expect(answer).rejects.toMatchObject({kind:'superseded'});
  expect(f.calls.some(call => call.op === 'subscribe')).toBe(false); expect(mobileBrowserSnapshot(mobileStreamOwner(f.client), f.client).count).toBe(0);
});
test('preview URL normalization follows source loopback and public-host rules', () => {
  expect(normalizeMobilePreviewUrl('localhost:5173')).toBe('http://localhost:5173/'); expect(normalizeMobilePreviewUrl('example.com')).toBe('https://example.com/');
  expect(() => normalizeMobilePreviewUrl('file:///a')).toThrow(); expect(() => normalizeMobilePreviewUrl(' ')).toThrow();
});

test('replayed native batches cannot crowd distinct events out of the source200-event replay buffer', async () => {
  const f = fixture(); await mobileBrowserRead(f.owner, f.native, f.client);
  const events = Array.from({length:200}, (_,i) => ({seq:i+1,generation:4,key:BROWSER_EVENTS,subscriptionId:'4-7',value:{threadId:'thread',serverEpoch:'epoch',revision:i+2,tabId:`extra-${i}`,type:'opened',snapshot:{tabId:`extra-${i}`,runtime:'server',updatedAt:'2026',navStatus:{_tag:'Idle'}}}}));
  mobileBrowserEvents(events,f.client);
  for(let n=0;n<205;n++) mobileBrowserEvents([events[199]],f.client);
  await mobileBrowserRead(f.owner,f.native,f.client);
  expect(mobileBrowserSnapshot(f.owner,f.client).count).toBe(201);
});

test('late native status for tabA cannot grant control on selected tabB', async () => {
 const f=fixture();f.setTabs([{threadId:'thread',tabId:'one',runtime:'server',navStatus:{_tag:'Idle'},updatedAt:'2'},{threadId:'thread',tabId:'two',runtime:'server',navStatus:{_tag:'Idle'},updatedAt:'1'}]);
 await mobileBrowserRead(f.owner,f.native,f.client);let release!:()=>void;const gate=new Promise<void>(resolve=>{release=resolve});
 const native:Native={...f.native,async later(input){if(obj(input).action==='status')await gate;return f.native.later(input);}};
 const pending=mobileBrowserStatus(f.owner,native,f.client);await mobileBrowserAction(f.owner,'select','two',f.native,f.client);release();await pending;
 expect(mobileBrowserSnapshot(f.owner,f.client).canNavigate).toBe(false);
});
test('epoch invalidation survives an older in-flight list', async () => {
 const f=fixture();await mobileBrowserRead(f.owner,f.native,f.client);let release!:()=>void,reached!:()=>void;const gate=new Promise<void>(resolve=>{release=resolve}),started=new Promise<void>(resolve=>{reached=resolve});
 const native:Native={...f.native,async later(input){if(obj(input).method==='preview.list'){reached();await gate;}return f.native.later(input);}};
 const pending=mobileBrowserRead(f.owner,native,f.client);await started;
 mobileBrowserEvents([{seq:1,generation:4,key:BROWSER_EVENTS,subscriptionId:'4-7',value:{threadId:'thread',serverEpoch:'new',revision:1,tabId:'one',type:'closed'}}],f.client);
 release();await pending;expect(mobileBrowserSnapshot(f.owner,f.client).needsRefresh).toBe(true);
});

test('retryDue re-subscribes once; authorization failure alone does not retry', async () => {
 const f=fixture();await mobileBrowserRead(f.owner,f.native,f.client);
 mobileBrowserEvents([{seq:1,generation:4,key:BROWSER_EVENTS,subscriptionId:'4-7',value:{_transportError:{kind:'Authorization',message:'Denied'}}}],f.client);
 expect(mobileBrowserSnapshot(f.owner,f.client).needsRefresh).toBe(false);
 mobileBrowserEvents([{seq:2,generation:4,key:BROWSER_EVENTS,subscriptionId:'4-7',value:{_retryDue:true}}],f.client);
 expect(mobileBrowserSnapshot(f.owner,f.client).needsRefresh).toBe(true);
 await mobileBrowserRead(f.owner,f.native,f.client);expect(f.calls.filter(call=>call.op==='subscribe')).toHaveLength(2);
});

test('failed retry consumes only its own invalidation and cannot loop without a new marker', async () => {
 const f=fixture();await mobileBrowserRead(f.owner,f.native,f.client);
 mobileBrowserEvents([{seq:1,generation:4,key:BROWSER_EVENTS,subscriptionId:'4-7',value:{_retryDue:true}}],f.client);
 f.grants.length=0;await mobileBrowserRead(f.owner,f.native,f.client);
 expect(mobileBrowserSnapshot(f.owner,f.client).needsRefresh).toBe(false);
 expect(mobileBrowserSnapshot(f.owner,f.client).error).toContain('cannot view');
});
test('older permission reply cannot overwrite a newer denial', async () => {
 const f=fixture();let release!:()=>void;const gate=new Promise<void>(resolve=>{release=resolve});
 const native:Native={...f.native,async later(input){if(obj(input).op==='http'){await gate;return{ok:true,generation:4,value:{authenticated:true,permissions:['orchestration:read','preview:operate']}};}return f.native.later(input);}};
 const old=mobileBrowserRead(f.owner,native,f.client);f.grants.length=0;await mobileBrowserRead(f.owner,f.native,f.client);release();await expect(old).rejects.toMatchObject({kind:'superseded'});
 expect(mobileBrowserSnapshot(f.owner,f.client).count).toBe(0);expect(f.calls.some(call=>call.op==='subscribe')).toBe(false);
});
