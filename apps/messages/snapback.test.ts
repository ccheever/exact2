import { expect, test } from 'bun:test';
import { Database } from 'bun:sqlite';
import { copyFile, mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { MessagesReplica, origin, path, viewer } from './snapback-client';
import { browserCore } from './snapback-core';
import { backend } from './snapback/backend';
import { result, type Backend } from './snapback-types';
import type { Storage } from './app.contract.d.ts';

// Actual published Wasm + SQLite + CLI server. No interpreter or sync mocks.
test('0.2.30 device retains offline edits, acquires receipts, reopens, and rolls back failed storage', async()=>{
  const dir=await mkdtemp(join(tmpdir(),'messages-snapback-'));
  const reservation=Bun.serve({port:0,fetch:()=>new Response()});
  const base=`http://127.0.0.1:${reservation.port}`;await reservation.stop(true);
  await mkdir(join(dir,'snapback'));
  await copyFile(new URL('./snapback/schema.q',import.meta.url),join(dir,'snapback/schema.q'));
  const binary=process.env.SNAPBACK4_BIN?[process.env.SNAPBACK4_BIN]:[process.execPath,new URL('../../node_modules/snapback4/bin/snapback4.js',import.meta.url).pathname];
  const startServer=()=>Bun.spawn([...binary,'dev','--port',String(new URL(base).port),'--no-watch'],{cwd:dir,stdout:'ignore',stderr:'pipe'});
  let server=startServer();
  const originalFetch=globalThis.fetch;
  let online=false,loseReceipt=false,failCommit=false,failRead=false,failApply=false,failSchema=false,recordQueries=0;
  let writes=0,failStorageRead=false,failMetadataCommit=false;
  const sql=new Database(join(dir,'device.sqlite'));
  const storage={sqlite:{open:async()=>({
    execute:async(text:string,params:unknown[]=[])=>sql.prepare(text).run(...params as never[]),
    query:async(text:string,params:unknown[]=[])=>{
      if(failStorageRead){failStorageRead=false;throw new Error('storage fixture refused read');}
      return {rows:sql.prepare(text).values(...params as never[])};
    },
    transaction:async(commands:{sql:string;params?:unknown[]}[])=>{
      if(failCommit && commands.some(command=>command.params?.[0]==='o')){
        failCommit=false;throw new Error('disk fixture refused commit');
      }
      if(failMetadataCommit){failMetadataCommit=false;throw new Error('metadata fixture refused commit');}
      writes++;
      sql.transaction(()=>{for(const command of commands)sql.prepare(command.sql).run(...(command.params||[]) as never[]);})();
    },close:async()=>{},
  })}} as unknown as Storage;
  const local=async<T>(work:()=>Promise<T>)=>work();
  const open=async()=>{
    const core=await browserCore(storage,path,backend as unknown as Backend);
    return {core,client:await MessagesReplica.open(storage,{call:async request=>{
      if(request.op==='query'){
        recordQueries++;
        if(failRead){failRead=false;throw new Error('read fixture refused query');}
      }
      const answer=await core.call(request);
      if(failApply&&request.op==='apply'&&(answer.ok as {touched:string[]})?.touched.length){
        failApply=false;throw new Error('apply fixture lost committed reply');
      }
      return answer;
    }})};
  };
  globalThis.fetch=(async(input:RequestInfo|URL,init?:RequestInit)=>{
    if(String(input)==='/assets/snapback4-device.wasm')return new Response(Bun.file(new URL('./assets/snapback4-device.wasm',import.meta.url)));
    if(!online)throw new Error('offline fixture');
    if(failSchema&&String(input).endsWith('/schema'))throw new Error('schema fixture unavailable');
    const response=await originalFetch(String(input).replace(origin,base),init);
    if(loseReceipt && String(input).includes('/m/')){loseReceipt=false;throw new Error('lost receipt fixture');}
    return response;
  }) as typeof fetch;
  try {
    for(let n=0;;n++){
      try {if((await originalFetch(`${base}/schema`,{headers:{'x-snapback-persona':'alice'}})).ok)break;}catch{}
      if(n===200||server.exitCode!==null)throw new Error('backend did not start');
      await Bun.sleep(25);
    }
    let {client,core}=await open();
    await client.seed(new Map([['draft',{z:1,text:'fixture'}]]));
    await client.persist(new Map([['draft',{z:1,text:'Offline 🌲'}]]));
    ({client,core}=await open());
    expect(client.initial().get('draft')).toEqual({z:1,text:'Offline 🌲'});
    expect(result<any>(await core.call({op:'sync_state'})).acquired).toBe(false);
    online=true;
    await client.sync(3000,local,()=>{});
    expect(client.status()).toBe('Synced');
    expect(result<any>(await core.call({op:'sync_state'})).acquired).toBe(true);
    expect(result<any[]>(await core.call({op:'queued'}))).toHaveLength(0);
    ({client,core}=await open());
    expect(client.initial().get('draft')).toEqual({z:1,text:'Offline 🌲'});
    await client.persist(new Map([['draft',{z:1,text:'Offline 🌲'}]]));
    expect(result<any[]>(await core.call({op:'queued'}))).toHaveLength(0);
    failCommit=true;
    await expect(client.persist(new Map([['draft',{z:1,text:'must roll back'}]]))).rejects.toThrow('disk fixture');
    expect(client.initial().get('draft')).toEqual({z:1,text:'Offline 🌲'});
    expect(result<any[]>(await core.call({op:'queued'}))).toHaveLength(0);
    ({client,core}=await open());
    expect(client.initial().get('draft')).toEqual({z:1,text:'Offline 🌲'});
    await client.persist(new Map([['draft',{z:1,text:'receipt survives loss'}]]));
    loseReceipt=true;
    await client.sync(6000,local,()=>{});
    expect(result<any[]>(await core.call({op:'queued'}))).toHaveLength(1);
    ({client,core}=await open());
    await client.sync(9000,local,()=>{});
    expect(client.status()).toBe('Synced');
    expect(result<any[]>(await core.call({op:'queued'}))).toHaveLength(0);
    expect(client.initial().get('draft')).toEqual({z:1,text:'receipt survives loss'});
    const response=await originalFetch(`${base}/q/records`,{method:'POST',headers:{'content-type':'application/json','x-snapback-persona':'alice'},body:JSON.stringify({args:{c:null}})});
    expect((await response.json() as any).data.map((r:any)=>r.payload)).toEqual([{z:1,text:'receipt survives loss'}]);
    let ticks=9000;
    const sync=(publish:(rows:Map<string,unknown>)=>void=()=>{})=>client.sync(ticks+=3000,local,publish);
    recordQueries=0;writes=0;await sync();expect(recordQueries).toBe(0);expect(writes).toBe(0);
    online=false;await sync();expect(recordQueries).toBe(0);expect(writes).toBe(0);
    const metadata=async(value:string)=>core.call({op:'set_meta',key:'fixture:value',value});
    await metadata('old');writes=0;await metadata('old');expect(writes).toBe(0);
    await metadata('new');expect(writes).toBe(1);
    failStorageRead=true;await expect(metadata('lost read')).rejects.toThrow('storage fixture refused read');
    expect(result(await core.call({op:'meta',key:'fixture:value'}))).toBe('new');
    failMetadataCommit=true;await expect(metadata('lost write')).rejects.toThrow('metadata fixture refused commit');
    expect(result(await core.call({op:'meta',key:'fixture:value'}))).toBe('new');
    expect(sql.prepare("SELECT v FROM snapback_device WHERE s='m' AND k='client:fixture:value'").get()).toEqual({v:'new'});
    await metadata('x'.repeat(8193));writes=0;await metadata('x'.repeat(8193));
    expect(writes).toBe(1); // Large commits retain the existing bounded request path.
    await metadata('new');
    const remote=async(key:string,text:string,id:string)=>{
      const response=await originalFetch(`${base}/m/putRecords`,{method:'POST',headers:{'content-type':'application/json','x-snapback-persona':'alice'},body:JSON.stringify({id,args:{recordIds:[`${viewer}:${encodeURIComponent(key)}`],keys:[key],payloads:[{text}]},newIds:[]})});
      const receipt=await response.json() as any;expect(receipt.state).toBe('sent');return receipt;
    };
    online=true;await remote('draft','remote after read failure','remote:read');
    failRead=true;await expect(sync()).rejects.toThrow('read fixture refused query');
    expect(client.initial().get('draft')).toEqual({z:1,text:'receipt survives loss'});
    online=false;let published:Map<string,unknown>|undefined;
    await sync(rows=>{published=rows;});
    expect(published?.get('draft')).toEqual({text:'remote after read failure'});
    recordQueries=0;await sync();expect(recordQueries).toBe(0);
    online=true;await remote('draft','remote after publication failure','remote:publish');
    await expect(sync(()=>{throw new Error('publication fixture refused model');})).rejects.toThrow('publication fixture refused model');
    expect(client.initial().get('draft')).toEqual({text:'remote after read failure'});
    online=false;await sync(rows=>{published=rows;});
    expect(published?.get('draft')).toEqual({text:'remote after publication failure'});
    online=true;await remote('draft','remote after apply failure','remote:apply');
    failApply=true;await sync(rows=>{published=rows;});
    expect(failApply).toBe(false);
    expect(published?.get('draft')).toEqual({text:'remote after apply failure'});
    const oldWatermark=result<any>(await core.call({op:'sync_state'})).watermark;
    // A different store can catch up to the same numeric watermark. Reset is
    // already durable when the subsequent schema fetch fails.
    server.kill();await server.exited;await rm(join(dir,'.snapback4'),{recursive:true,force:true});server=startServer();
    for(let n=0;;n++){
      try{if((await originalFetch(`${base}/schema`,{headers:{'x-snapback-persona':'alice'}})).ok)break;}catch{}
      if(n===200||server.exitCode!==null)throw new Error('replacement backend did not start');await Bun.sleep(25);
    }
    for(let seq=1;seq<=oldWatermark;seq++)expect((await remote('replacement',`new store ${seq}`,`replacement:${seq}`)).seq).toBe(seq);
    online=true;failSchema=true;await sync(rows=>{published=rows;});
    expect(published?.size).toBe(0);expect(client.initial().size).toBe(0);
    expect(result<any>(await core.call({op:'sync_state'})).watermark).toBe(0);
    failSchema=false;await sync(rows=>{published=rows;});
    expect(published?.get('replacement')).toEqual({text:`new store ${oldWatermark}`});
    expect(client.initial().has('draft')).toBe(false);
    expect(result<any>(await core.call({op:'sync_state'})).watermark).toBe(oldWatermark);
    recordQueries=0;await sync();expect(recordQueries).toBe(0);
    // Capture changed values before the first storage await, including nested
    // objects and arrays. Caller mutations must not alter the admitted value or
    // the rollback image, either while persistence is pending or after it ends.
    const owned={nested:{text:'Captured 🌲'},list:[1,2],z:null};
    const unrelated=client.initial().get('replacement');
    const image=new Map([['owned',owned]]);
    const saving=client.edit(image);
    expect(client.initial().has('owned')).toBe(false);
    owned.nested.text='Changed during await';owned.list.push(3);image.delete('owned');
    await saving;
    const captured={nested:{text:'Captured 🌲'},list:[1,2],z:null};
    expect(client.initial().get('owned')).toEqual(captured);
    owned.nested.text='Changed after await';owned.list.push(4);
    expect(client.initial().get('owned')).toEqual(captured);
    await sync();({client,core}=await open());
    expect(client.initial().get('owned')).toEqual(captured);
    expect(client.initial().get('replacement')).toEqual(unrelated);
    const reordered=new Map(client.initial());
    reordered.set('owned',{z:null,list:[1,2],nested:{text:'Captured 🌲'}});
    writes=0;await client.edit(reordered);expect(writes).toBe(0);
    const changed=new Map(client.initial());changed.set('owned',owned);
    failCommit=true;await expect(client.edit(changed)).rejects.toThrow('disk fixture');
    expect(client.initial().get('owned')).toEqual(captured);
    await client.edit(changed);
    owned.nested.text='After second save';
    expect(client.initial().get('owned')).toEqual({nested:{text:'Changed after await'},list:[1,2,3,4],z:null});
    await sync();({client,core}=await open());
    expect(client.initial().get('owned')).toEqual({nested:{text:'Changed after await'},list:[1,2,3,4],z:null});
  }finally{
    globalThis.fetch=originalFetch;server.kill();await server.exited;sql.close();await rm(dir,{recursive:true,force:true});
  }
},30000);


// The test-only bundle exposes the existing full snapshot as an independent
// oracle. Production bytecode has neither the export nor this extra traversal.
test('every durable source footprint matches the complete model and durable device', async()=>{
  const dir=await mkdtemp(join(tmpdir(),'messages-footprints-'));
  const sql=new Database(join(dir,'device.sqlite'));
  const originalFetch=globalThis.fetch;
  let failCommit=false;
  const storage={sqlite:{open:async()=>({
    execute:async(text:string,params:unknown[]=[])=>sql.prepare(text).run(...params as never[]),
    query:async(text:string,params:unknown[]=[])=>({rows:sql.prepare(text).values(...params as never[])}),
    transaction:async(commands:{sql:string;params?:unknown[]}[])=>{
      if(failCommit && commands.some(c=>c.params?.[0]==='o')){failCommit=false;throw new Error('footprint commit refused');}
      sql.transaction(()=>{for(const c of commands)sql.prepare(c.sql).run(...(c.params||[]) as never[]);})();
    },close:async()=>{},
  })}} as unknown as Storage;
  globalThis.fetch=(async(input:RequestInfo|URL)=>{
    if(String(input)==='/assets/snapback4-device.wasm')return new Response(Bun.file(new URL('./assets/snapback4-device.wasm',import.meta.url)));
    throw new Error('offline footprint fixture');
  }) as typeof fetch;
  try {
    const entry=new URL('./app.ts',import.meta.url).pathname;
    const output=join(dir,'app.mjs');
    const built=await Bun.build({entrypoints:[entry],target:'bun',outdir:dir,naming:'app.mjs',plugins:[{
      name:'full-model-oracle',setup(build){build.onLoad({filter:/\/apps\/messages\/app\.ts$/},async args=>({
        loader:'ts',contents:await readFile(args.path,'utf8')+`
export async function inspectFixture(){return JSON.parse(JSON.stringify({live:[...snapshot()],held:[...replica.initial()],durable:[...await replica.read()],awaiting:[...indexes].flatMap(([id,index])=>[...index.awaitingRead].map(row=>[id,row.id])),peopleOrder:people.map(p=>p.id),sources:Object.keys(sources)}));}
export function omitFixtureEdit(){const edit=replica.edit.bind(replica);replica.edit=async records=>{replica.edit=edit;return edit(new Map());};}
export function undeclaredFixtureSource(){sources.undeclared=()=>{people[0].unread=!people[0].unread;return changed();};}
let fixtureKeys=[];
export function recordFixtureEdits(){const edit=replica.edit.bind(replica);replica.edit=async records=>{fixtureKeys=[...records.keys()];return edit(records);};}
export function fixtureEditKeys(){return fixtureKeys;}
export async function fixturePositions(positions){const rows=new Map(JSON.parse(JSON.stringify([...replica.initial()])));for(const [id,position] of positions)rows.get('person:'+id).position=position;await replica.edit(rows);restore(replica.initial());}
`,
      }));},
    }]});
    expect(built.success).toBe(true);
    let generation=0,app=await import(output+`?instance=${generation++}`);
    const call=(source:string,args:unknown[])=>app.answer(source,args,{},storage,undefined);
    const canonical=(rows:[string,unknown][])=>new Map(rows.sort(([a],[b])=>a.localeCompare(b)));
    const inspect=async()=>{
      const values=await app.inspectFixture();
      expect(canonical(values.live)).toEqual(canonical(values.held));
      expect(canonical(values.live)).toEqual(canonical(values.durable));
      const waiting=values.live.filter(([_key,row]:[string,any])=>row.kind==='message' && row.expires===null && row.message.outgoing && row.message.delivery!=='Read')
        .map(([_key,row]:[string,any])=>JSON.stringify([row.conversation,row.message.id])).sort();
      expect(values.awaiting.map((pair:string[])=>JSON.stringify(pair)).sort()).toEqual(waiting);
      const order=values.live.filter(([_key,row]:[string,any])=>row.kind==='person')
        .sort(([,a]:any,[,b]:any)=>a.position-b.position || a.person.id.localeCompare(b.person.id))
        .map(([,row]:any)=>row.person.id);
      expect(values.peopleOrder).toEqual(order);
      return values;
    };
    const exercised=new Set<string>();
    const act=async(source:string,args:unknown[])=>{exercised.add(source);await call(source,args);return inspect();};
    await call('conversation',['maya',0,'','','']);await inspect();app.recordFixtureEdits();
    await act('saveDraft',['maya','A durable draft 🌲','m9']);
    expect(app.fixtureEditKeys()).toEqual(['person:maya']);
    await act('markRead',['maya']);
    await act('setConversationUnread',['maya',true]);
    await act('muteConversation',['maya']);await act('muteConversation',['maya']);
    await act('blockConversation',['maya',true]);
    await act('sendMessage',['maya','Blocked reply scheduling','',0,1000]);
    await act('blockConversation',['maya',false]);
    await act('react',['maya','m9','❤️']);expect(app.fixtureEditKeys()).toEqual(['message:maya:m9']);
    await act('react',['maya','m9','❤️']);
    await act('createLocalContact',['New','Contact','','+14155550999','new@example.test','Notes']);
    expect(app.fixtureEditKeys().sort()).toEqual(['person:address:%2B14155550999','person:address:new%40example.test']);
    await act('createLocalContact',['Maya','Renamed','','+14155550101','','Updated']);
    expect(app.fixtureEditKeys()).toEqual(['person:maya']);
    const positions=(values:any)=>new Map(values.live.filter(([,row]:any)=>row.kind==='person').map(([key,row]:any)=>[key,row.position]));
    const priorPositions=positions(await inspect());
    await act('sendMessage',['address:'+encodeURIComponent('fresh@example.test'),'New contact','',0,2000]);
    expect(app.fixtureEditKeys().filter((key:string)=>key.startsWith('person:'))).toEqual(['person:address:fresh%40example.test']);
    const afterPositions=positions(await inspect());
    for(const [key,position] of priorPositions)expect(afterPositions.get(key)).toBe(position);
    const priorOrder=(await inspect()).peopleOrder;
    await act('createLocalContact',['Name','Only','','','','']);
    expect(app.fixtureEditKeys()).toHaveLength(1);
    expect((await inspect()).peopleOrder.slice(0,-1)).toEqual(priorOrder);
    await act('createLocalContact',['','','','','','']);expect(app.fixtureEditKeys()).toEqual([]);
    await act('sendMessage',['group:dad|maya','New group','',0,3000]);
    await act('sendMessage',['maya','Receipt and reply','m9',100,4000]);
    for(const now of [102,103,104,115,116]){
      await act('advanceReplies',[now,'maya',now*1000]);
      if(now===103)expect(app.fixtureEditKeys()).not.toContain('message:maya:m9');
    }
    await act('deleteMessages',['maya','m9|m10',0]);
    await act('recentlyDeleted',['',0,0]);
    await act('recoverConversations',['maya',0]);
    await act('deleteConversation',['maya',0]);
    await act('purgeConversations',['maya',0]);
    await act('deleteConversation',['dad',0]);await act('deleteMessages',['alex','alex-1',0]);
    await act('recentlyDeleted',['',0,31*86400000]);
    await act('deleteMessages',['sam','sam-1',0]);
    await act('deleteMessages',['jules','jules-1',86400000]);
    await act('recoverConversations',['sam|jules',30.5*86400000]);
    await act('deleteMessages',['sam','sam-2',0]);
    await act('deleteMessages',['jules','jules-2',0]);
    await act('purgeConversations',['sam',31*86400000]);
    await act('sendMessage',['maya','After recovery expiry','',200,5000]);
    const before=canonical((await inspect()).live);
    await expect(call('sendMessage',['maya','🌲'.repeat(20000),'',200,6000])).rejects.toThrow('UTF-8');
    expect(canonical((await inspect()).live)).toEqual(before);
    failCommit=true;
    await expect(call('saveDraft',['maya','Refused draft',''])).rejects.toThrow('footprint commit refused');
    expect(canonical((await inspect()).live)).toEqual(before);
    await act('saveDraft',['maya','Retry draft','']);
    // Imported positions can be tied/fractional or at finite Number extremes.
    // Ordinary edits retain them; insertion rebases only if +/-1 cannot progress.
    await app.fixturePositions([['maya',.5],['dad',.5],['alex',-.25]]);
    const imported=positions(await inspect());
    await act('saveDraft',['maya','Preserve imported ordering','']);
    expect(positions(await inspect())).toEqual(imported);
    await app.fixturePositions([['maya',-Number.MAX_VALUE]]);
    const beforeRebase=await inspect();
    failCommit=true;
    const rebasedId='address:rebase%40example.test';
    await expect(call('sendMessage',[rebasedId,'Refused rebase','',200,7000])).rejects.toThrow('footprint commit refused');
    expect(canonical((await inspect()).live)).toEqual(canonical(beforeRebase.live));
    await act('sendMessage',[rebasedId,'Retry rebase','',200,7000]);
    expect((await inspect()).peopleOrder).toEqual([rebasedId,...beforeRebase.peopleOrder]);
    await app.fixturePositions([['dad',Number.MAX_VALUE]]);
    const beforeAppend=(await inspect()).peopleOrder;
    await act('createLocalContact',['Extreme','Append','','+14155550888','extreme@example.test','']);
    expect((await inspect()).peopleOrder).toEqual([...beforeAppend,'address:%2B14155550888','address:extreme%40example.test']);
    const saved=await inspect();
    const pure=['conversation','conversationDraft','inbox','recipients','syncState','syncMessages'];
    expect([...exercised].sort()).toEqual(saved.sources.filter((s:string)=>!pure.includes(s)).sort());
    app=await import(output+`?instance=${generation++}`);
    await call('conversation',['maya',0,'','','']);
    expect(canonical((await inspect()).live)).toEqual(canonical(saved.live));
    // Negative control: omit one declared edit. The full-model oracle must see
    // the lost change, rather than sharing the footprint's blind spot.
    app.omitFixtureEdit();await call('saveDraft',['maya','Deliberately omitted','']);
    const omitted=await app.inspectFixture();
    expect(canonical(omitted.live)).not.toEqual(canonical(omitted.durable));
    app=await import(output+`?instance=${generation++}`);
    await call('conversation',['maya',0,'','','']);const intact=await inspect();
    app.undeclaredFixtureSource();
    await expect(call('undeclared',[])).rejects.toThrow('no durable footprint');
    expect(canonical((await inspect()).live)).toEqual(canonical(intact.live));
  }finally{globalThis.fetch=originalFetch;sql.close();await rm(dir,{recursive:true,force:true});}
},30000);
