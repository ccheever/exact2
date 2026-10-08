// @ref llp/1109.010-mobile-browser-devices.decision.md#connection-and-command-ownership
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { deviceStateEvent, DEVICE_STATE_KEY } from './shared/r4-surfaces-device';
import { mobileStreamOwner } from './browser-mobile-owner';
import { mobileDevicesRead, mobileDevicesAction, mobileDevicesSnapshot, mobileDevicesEvents } from './devices-mobile-data';
import { threadDevicePreviews } from './devices-mobile-model';
import { createShakeDetector } from './devices-mobile-shake';

test('same Android serial on different hosts resolves its owning host and device', () => {
  const rows = threadDevicePreviews({ sessions: [{ hostId: 'b', deviceId: 'serial', threadId: 't', platform: 'android' }], hosts: [{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }],
    devices: [{ hostId: 'a', id: 'serial', name: 'Wrong' }, { hostId: 'b', id: 'serial', name: 'Right', version: '14' }] }, 't');
  expect(rows[0]?.name).toBe('Right'); expect(rows[0]?.description).toBe('14 · B'); expect(rows[0]?.key).toBe('["b","serial"]');
});
test('only the shutdown operation releases its busy flag; target remains captured', async () => {
  const client = new T3Client(); client.origin = 'https://devices.test'; client.environmentId = 'env'; client.threadId = 't'; client.generation = 0; client.connection = 'connected'; client.configLive = client.shellLive = client.threadLive = true;
  const calls: Obj[] = []; let reached!: () => void; const started = new Promise<void>(resolve => { reached = resolve; }); let finish!: () => void; const gate = new Promise<void>(resolve => { finish = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); if (request.method === 'device.shutdown') { reached(); await gate; }
    return { ok: true, generation: client.generation, value: request.op === 'http' ? { authenticated: true, permissions: ['orchestration:read', 'orchestration:operate'] } : request.op === 'subscribe' ? { id: '0-4' } : {} };
  } };
  const owner = mobileStreamOwner(client); await mobileDevicesRead(owner, native, client);
  deviceStateEvent(client, { key: DEVICE_STATE_KEY, subscriptionId: '0-4', value: { hosts: [{ id: 'h', label: 'Host' }], devices: [], sessions: [{ hostId: 'h', deviceId: 'd', threadId: 't', platform: 'ios' }] } });
  const running = mobileDevicesAction(owner, 'shutdown', '', native, client);
  await started;
  await mobileDevicesAction(owner, 'select', '["h","d"]', native, client);
  expect(mobileDevicesSnapshot(owner, client).operating).toBe(true);
  await mobileDevicesAction(owner, 'shutdown', '', native, client); expect(calls.filter(call => call.method === 'device.shutdown')).toHaveLength(1);
  finish(); await running; expect(mobileDevicesSnapshot(owner, client).operating).toBe(false);
  expect(calls.find(call => call.method === 'device.shutdown')?.payload).toEqual({ hostId: 'h', deviceId: 'd', platform: 'ios' });
});
test('source shake needs two jolts and observes cooldown', () => {
  const shake = createShakeDetector(); expect(shake({ x: 2, y: 0, z: 0, timestamp: 0 })).toBe(false);
  expect(shake({ x: 2, y: 0, z: 0, timestamp: 50 })).toBe(true); expect(shake({ x: 2, y: 0, z: 0, timestamp: 100 })).toBe(false);
  expect(shake({ x: 1, y: 0, z: 0, timestamp: 2000 })).toBe(false);
});

test('shutdown permission await cannot retarget an action to another selected device', async () => {
  const client = new T3Client(); Object.assign(client,{origin:'https://device.test',environmentId:'env',threadId:'t',generation:1,connection:'connected',configLive:true,shellLive:true,threadLive:true});
  let release!:()=>void, reached!:()=>void, checks=0; const gate=new Promise<void>(resolve=>{release=resolve}), pending=new Promise<void>(resolve=>{reached=resolve}); const calls:Obj[]=[];
  const native:Native={available:true,watch(){},async later(value){const request=obj(value);calls.push(request);
    if(request.op==='http' && ++checks===2){reached();await gate;}
    return {ok:true,generation:1,value:request.op==='http'?{authenticated:true,permissions:['orchestration:read','orchestration:operate']}:request.op==='subscribe'?{id:'1-9'}:{}};
  }};
  const owner=mobileStreamOwner(client);await mobileDevicesRead(owner,native,client);
  deviceStateEvent(client,{key:DEVICE_STATE_KEY,subscriptionId:'1-9',value:{hosts:[{id:'host'}],devices:[],sessions:[{hostId:'host',deviceId:'first',threadId:'t',platform:'ios'},{hostId:'host',deviceId:'second',threadId:'t',platform:'ios'}]}});
  const operation=mobileDevicesAction(owner,'shutdown','',native,client);await pending;
  await mobileDevicesAction(owner,'select','["host","second"]',native,client);release();await operation;
  expect(calls.find(call=>call.method==='device.shutdown')?.payload).toEqual({hostId:'host',deviceId:'first',platform:'ios'});
  expect(mobileDevicesSnapshot(owner,client).selected).toBe('["host","second"]');
});

test('device retry markers reopen the shared subscription without changing its reducer', async () => {
 const client=new T3Client();Object.assign(client,{origin:'https://device.test',environmentId:'env',threadId:'t',generation:4,connection:'connected',configLive:true,shellLive:true,threadLive:true});const calls:Obj[]=[];
 const native:Native={available:true,watch(){},async later(input){const request=obj(input);calls.push(request);return{ok:true,generation:4,value:request.op==='http'?{authenticated:true,permissions:['orchestration:read']}:request.op==='subscribe'?{id:'4-9'}:{}};}};
 const owner=mobileStreamOwner(client);await mobileDevicesRead(owner,native,client);
 mobileDevicesEvents([{seq:1,generation:4,key:DEVICE_STATE_KEY,subscriptionId:'4-9',value:{_retryDue:true}}],client);
 expect(mobileDevicesSnapshot(owner,client).needsRefresh).toBe(true);await mobileDevicesRead(owner,native,client);
 expect(calls.filter(call=>call.op==='subscribe')).toHaveLength(2);expect(mobileDevicesSnapshot(owner,client).needsRefresh).toBe(false);
});


test('a superseded subscribe cannot clear a newer permission denial swallowed by shared watchDevice', async () => {
  const client = new T3Client(); Object.assign(client, { origin: 'https://device.test', environmentId: 'env', threadId: 't', generation: 4, connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  let deny = false, release!: () => void, reached!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), pending = new Promise<void>(resolve => { reached = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); if (request.op === 'subscribe') { reached(); await gate; }
    return { ok: true, generation: 4, value: request.op === 'http' ? { authenticated: true, permissions: deny ? [] : ['orchestration:read'] } : { id: '4-1' } };
  } };
  const owner = mobileStreamOwner(client), older = mobileDevicesRead(owner, native, client).catch(error => error.kind);
  await pending; deny = true; await mobileDevicesRead(owner, native, client);
  const denial = mobileDevicesSnapshot(owner, client).error; expect(denial).toBe('This connection cannot view previews.');
  release(); expect(await older).toBe('superseded');
  expect(mobileDevicesSnapshot(owner, client).error).toBe(denial);
});
