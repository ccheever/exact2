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

test('contact lookups preserve restored identity, duplicate selection and rebase order', async()=>{
  const dir=await mkdtemp(join(tmpdir(),'messages-contacts-'));
  try {
    const entry=new URL('./app.ts',import.meta.url).pathname;
    const built=await Bun.build({entrypoints:[entry],target:'bun',outdir:dir,naming:'app.mjs',plugins:[{
      name:'restored-contact-fixture',setup(build){build.onLoad({filter:/\/apps\/messages\/app\.ts$/},async args=>({
        loader:'ts',contents:await readFile(args.path,'utf8')+'\nexport const contactFixture={restore,snapshot,sources,people,editRecords};\n',
      }));},
    }]});
    expect(built.success).toBe(true);
    const {contactFixture:f}=await import(join(dir,'app.mjs'));
    const initial=JSON.parse(JSON.stringify([...f.snapshot()]));
    const records=new Map<string,any>(initial);
    const maya=records.get('person:maya');
    records.set('person:maya',{...maya,person:{...maya.person,name:'First Maya'},position:-Number.MAX_VALUE});
    records.set('duplicate:maya',{...maya,person:{...maya.person,name:'Last Maya'},position:100});
    f.restore(records);
    const chat=(id:string)=>f.sources.conversation([id,0,'','','']);
    expect(chat('maya').name).toBe('First Maya');
    const edit=f.editRecords('markRead',['maya']);
    f.sources.markRead(['maya']);
    expect(f.people.filter((p:any)=>p.id==='maya').map((p:any)=>p.unread)).toEqual([false,true]);
    const changed=edit.capture().get('person:maya');
    expect(changed.person.name).toBe('First Maya');expect(changed.position).toBe(100);
    // Numeric exhaustion rebases positions without changing which duplicate is
    // selected or replacing the referenced person with a detached copy.
    f.sources.sendMessage(['address:front%40example.test','New first contact','',0,0]);
    expect(chat('address:front%40example.test').name).toBe('front@example.test');
    expect(chat('maya').name).toBe('First Maya');
    expect(f.snapshot().get('person:maya').position).toBe(6);
    f.sources.createLocalContact(['Maya','Renamed','','+14155550101','','']);
    expect(chat('maya').name).toBe('Maya Renamed');
    expect(f.people.filter((p:any)=>p.id==='maya').map((p:any)=>p.name)).toEqual(['Maya Renamed','Last Maya']);
    // Restoring a replacement model drops removed contacts and old references.
    f.restore(new Map(initial));
    expect(chat('maya').name).toBe('Maya Chen');
    expect(chat('address:front%40example.test').name).toBe('Maya Chen');
    f.sources.markRead(['maya']);
    expect(f.people[0].unread).toBe(false);
    f.sources.setConversationUnread(['maya',true]);
    expect(f.people[0].unread).toBe(true);
    // Every matching contact remains reachable, in source order, while each
    // answer contains at most 200 contact rows. Selection is outside that page.
    for(let i=0;i<450;i++)f.sources.sendMessage([`address:page-${i}%40example.test`,'Page fixture','',0,0]);
    const inbox=(cursor='',query='')=>f.sources.inbox([query,0,cursor]);
    const recipients=(cursor='',selected='',query='')=>f.sources.recipients([selected,query,'Draft',0,cursor]);
    const walk=(read:(cursor:string)=>any)=>{
      const pages:any[]=[],seen=new Set<string>();let cursor='';
      do{
        expect(seen.has(cursor)).toBe(false);seen.add(cursor);
        const page=read(cursor);expect(page.people.length).toBeLessThanOrEqual(200);
        pages.push(page);cursor=page.later;
      }while(cursor);
      return pages;
    };
    const ids=(page:any)=>page.people.map((person:any)=>person.id);
    const pages=walk(inbox),all=f.people.map((person:any)=>person.id);
    expect(pages.map(ids).flat()).toEqual(all);expect(pages.map(p=>p.people.length)).toEqual([200,200,56]);
    expect(ids(inbox(pages[2].earlier))).toEqual(ids(pages[1]));
    expect(ids(inbox(pages[1].earlier))).toEqual(ids(pages[0]));
    const selected='maya|sam',contactPages=walk(cursor=>recipients(cursor,selected));
    expect(contactPages.map(ids).flat()).toEqual(f.people.filter((p:any)=>p.address && !selected.split('|').includes(p.id)).map((p:any)=>p.id));
    for(const page of contactPages){expect(page.selected.map((p:any)=>p.id)).toEqual(['maya','sam']);expect(page.canSend).toBe(true);}
    const anchor=pages[0].later,anchorId=pages[1].people[0].id,nextId=pages[1].people[1].id;
    f.sources.sendMessage(['address:prepended%40example.test','New first','',0,0]);
    expect(inbox(anchor).people[0].id).toBe(anchorId);
    f.sources.deleteConversation([anchorId,0]);expect(inbox(anchor).people[0].id).toBe(nextId);
    f.sources.recoverConversations([anchorId,0]);expect(inbox(anchor).people[0].id).toBe(anchorId);
    expect(recipients(anchor,anchorId).people[0].id).toBe(nextId);
    expect(walk(cursor=>inbox(cursor,'PAGE-1')).map(ids).flat()).toEqual(f.people.filter((p:any)=>p.name.toLowerCase().includes('page-1')).map((p:any)=>p.id));
    // Replacement sync may remove the anchor. A rebase may change its stored
    // position; surviving identity wins over the old numeric fallback.
    const without=new Map<string,any>(JSON.parse(JSON.stringify([...f.snapshot()])));
    without.delete('person:'+anchorId);
    for(const [key,row] of without)if(row.kind==='message' && row.conversation===anchorId)without.delete(key);
    f.restore(without);expect(inbox(anchor).people[0].id).toBe(nextId);
    const rebasing=new Map<string,any>(JSON.parse(JSON.stringify([...f.snapshot()])));
    rebasing.get('person:'+f.people[0].id).position=-Number.MAX_VALUE;
    const survivingCursor=inbox().later,survivingId=JSON.parse(survivingCursor)[0];
    f.restore(rebasing);f.sources.sendMessage(['address:rebased-page%40example.test','Rebase','',0,0]);
    expect(inbox(survivingCursor).people[0].id).toBe(survivingId);
    for(const invalid of ['x','[]','["x"]','["x","bad",0]','["",1,0]','["x",1e309,0]','["x",1,-1]','["x",1,0.5]']){
      expect(()=>inbox(invalid)).toThrow('Invalid contact cursor');expect(()=>recipients(invalid)).toThrow('Invalid contact cursor');
    }
    expect(inbox('', 'no such person')).toEqual({people:[],earlier:'',later:''});
    // An unrecognized address is offered before name matches. It is a real
    // first-page choice, even though it has no persisted contact position yet.
    for(let i=0;i<250;i++)f.sources.createLocalContact([`match@example.test ${i}`,'','','',`synthetic-${i}@example.test`,'']);
    const syntheticPages=walk(cursor=>recipients(cursor,'','match@example.test'));
    expect(syntheticPages.map(ids).flat().length).toBe(251);
    expect(syntheticPages[0].people[0].id).toBe('address:match%40example.test');
    expect(ids(recipients(syntheticPages[1].earlier,'','match@example.test'))).toEqual(ids(syntheticPages[0]));
    // Sync can remove a persisted page anchor while the offered address is
    // still only synthetic. Skip that unpositioned row during anchor recovery.
    const syntheticRecords=new Map<string,any>(JSON.parse(JSON.stringify([...f.snapshot()])));
    syntheticRecords.delete('person:'+syntheticPages[1].people[0].id);
    f.restore(syntheticRecords);
    const recoveredPage=recipients(syntheticPages[0].later,'','match@example.test');
    expect(ids(recoveredPage)).toEqual(ids(syntheticPages[1]).slice(1));
    expect(ids(recipients(recoveredPage.earlier,'','match@example.test'))).toEqual(ids(syntheticPages[0]));
    for(const person of syntheticPages[1].people)syntheticRecords.delete('person:'+person.id);
    f.restore(syntheticRecords);
    const clampedPage=recipients(syntheticPages[0].later,'','match@example.test');
    expect(ids(clampedPage)).toEqual(ids(syntheticPages[0]));
    expect(clampedPage.earlier).toBe('');expect(clampedPage.later).toBe('');

    f.restore(new Map(initial));
    for(let i=0;i<450;i++)f.sources.sendMessage([`address:duplicate-page-${i}%40example.test`,'Duplicate page fixture','',0,0]);
    const duplicates=new Map<string,any>(JSON.parse(JSON.stringify([...f.snapshot()])));
    const first=duplicates.get('person:'+f.people[0].id);
    duplicates.set('duplicate:page',{...first,person:{...first.person,name:'Later duplicate'},position:duplicates.get('person:'+f.people[200].id).position-.5});
    f.restore(duplicates);
    const duplicatePages=walk(inbox);
    expect(duplicatePages.map(ids).flat()).toEqual(f.people.map((person:any)=>person.id));
    expect(ids(inbox(duplicatePages[1].earlier))).toEqual(ids(duplicatePages[0]));
    // Archived contacts share cursor identity/order, but recovery totals and
    // selection span the complete archive rather than only the displayed page.
    f.restore(new Map(initial));
    for(let i=0;i<450;i++){
      const id=`address:deleted-page-${i}%40example.test`;
      f.sources.sendMessage([id,'Archived fixture','',0,0]);
      f.sources.deleteConversation([id,0]);
    }
    const archived=new Map(f.snapshot()),day=86400000;
    const deleted=(cursor='',selection='',now=0)=>f.sources.recentlyDeleted([selection,0,now,cursor]);
    const archivedIds=f.people.filter((p:any)=>p.id.startsWith('address:deleted-page-')).map((p:any)=>p.id);
    const deletedPages=walk(deleted);
    expect(deletedPages.map(ids).flat()).toEqual(archivedIds);
    for(const page of deletedPages){expect(page.count).toBe(450);expect(page.targets).toBe(archivedIds.join('|'));}
    expect(ids(deleted(deletedPages[1].earlier))).toEqual(ids(deletedPages[0]));
    const archivedSelection=[archivedIds[0],archivedIds[249],archivedIds[449]].join('|');
    for(const page of walk(cursor=>deleted(cursor,archivedSelection))){
      expect(page.targets).toBe(archivedSelection);expect(page.count).toBe(3);
      for(const row of page.people)expect(row.chosen).toBe(archivedSelection.split('|').includes(row.id));
    }
    const at=deletedPages[0].later,archiveAnchor=archivedIds[200];
    for(const operation of ['recoverConversations','purgeConversations']){
      f.restore(archived);f.sources[operation]([archiveAnchor,0]);
      expect(deleted(at).people[0].id).toBe(archivedIds[201]);
      expect(deleted(at).count).toBe(449);
    }
    f.restore(archived);
    const snapshot=JSON.stringify([...f.snapshot()]);
    for(const cursor of ['bad','[]','["missing",0,-1]']){
      expect(()=>deleted(cursor,'',30*day)).toThrow('Invalid contact cursor');
      expect(JSON.stringify([...f.snapshot()])).toBe(snapshot);
    }
    const expired=deleted(at,archivedSelection,30*day);
    expect(expired).toEqual({people:[],earlier:'',later:'',targets:'',count:0});
    expect(deleted('',archivedSelection,0)).toEqual(expired);

  }finally{await rm(dir,{recursive:true,force:true});}
});


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
export async function inspectFixture(){return JSON.parse(JSON.stringify({live:[...snapshot()],held:[...replica.initial()],durable:[...await replica.read()],awaiting:[...indexes].flatMap(([id,index])=>[...index.awaitingRead].map(row=>[id,row.id])),peopleOrder:people.map(p=>p.id),pending:[...pending],ticks,revision,sources:Object.keys(sources)}));}
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
    const beforeGroupBlock=await inspect();
    expect(beforeGroupBlock.pending.some(([id]:[string,unknown])=>id==='group:dad|maya')).toBe(true);
    const afterGroupBlock=await act('blockConversation',['group:dad|maya',true]);
    expect(canonical(afterGroupBlock.live)).toEqual(canonical(beforeGroupBlock.live));
    expect(afterGroupBlock.pending).toEqual(beforeGroupBlock.pending);
    await act('sendMessage',['maya','Receipt and reply','m9',100,4000]);
    for(const now of [102,103,104,115,116]){
      await act('advanceReplies',[now,'maya',now*1000]);
      if(now===103)expect(app.fixtureEditKeys()).not.toContain('message:maya:m9');
    }
    // Unrelated archived rows must stay out of a local edit, but cross-thread
    // expiry must still be committed, retried and represented as deletion.
    const archivedIds:string[]=[];
    for(const [thread,now] of [['sam',0],['jules',86400000]] as const){
      const body=`Recovery footprint ${thread}`;
      const sent=await act('sendMessage',[thread,body,'',200,5000]);
      const id=sent.live.find(([,row]:any)=>row.kind==='message' && row.conversation===thread && row.message.body===body)[1].message.id;
      archivedIds.push(id);await act('deleteMessages',[thread,id,now]);
    }
    await act('recentlyDeleted',['',0,0,'']);expect(app.fixtureEditKeys()).toEqual([]);
    await act('deleteMessages',['maya','m1',0]);
    expect(app.fixtureEditKeys().sort()).toEqual(['message:maya:m1','person:maya']);
    await act('recoverConversations',['maya',0]);
    expect(app.fixtureEditKeys().sort()).toEqual(['message:maya:m1','person:maya']);
    await act('purgeConversations',['',0]);expect(app.fixtureEditKeys()).toEqual([]);
    // An empty delete does not run expiry in the handler, even at a later clock.
    const noDelete=canonical((await inspect()).live);
    await act('deleteMessages',['maya','missing-message',31*86400000]);
    expect(canonical((await inspect()).live)).toEqual(noDelete);
    await expect(call('recentlyDeleted',['',0,30.5*86400000,'malformed'])).rejects.toThrow('Invalid contact cursor');
    expect(canonical((await inspect()).live)).toEqual(noDelete);
    failCommit=true;
    await expect(call('recentlyDeleted',['',0,30.5*86400000,''])).rejects.toThrow('footprint commit refused');
    expect(canonical((await inspect()).live)).toEqual(noDelete);
    await act('recentlyDeleted',['',0,30.5*86400000,'']);
    expect(app.fixtureEditKeys()).toEqual(['message:sam:'+encodeURIComponent(archivedIds[0])]);
    await act('recoverConversations',['jules',30.5*86400000]);
    expect(app.fixtureEditKeys().sort()).toEqual(['message:jules:'+encodeURIComponent(archivedIds[1]),'person:jules']);
    await act('deleteMessages',['maya','m9|m10',0]);
    await act('recentlyDeleted',['',0,0,'']);
    await act('recoverConversations',['maya',0]);
    await act('deleteConversation',['maya',0]);
    await act('purgeConversations',['maya',0]);
    await act('deleteConversation',['dad',0]);await act('deleteMessages',['alex','alex-1',0]);
    await act('recentlyDeleted',['',0,31*86400000,'']);
    await act('deleteMessages',['sam','sam-1',0]);
    await act('deleteMessages',['jules','jules-1',86400000]);
    await act('recoverConversations',['sam|jules',30.5*86400000]);
    await act('deleteMessages',['sam','sam-2',0]);
    await act('deleteMessages',['jules','jules-2',0]);
    await act('purgeConversations',['sam',31*86400000]);
    await act('sendMessage',['maya','After recovery expiry','',200,5000]);
    await act('sendMessage',['dad','Another pending conversation','',201,5500]);
    const scheduled=await inspect();
    expect(scheduled.pending.length).toBeGreaterThan(2);
    // Replacing an entry must retain its position on refusal, including the
    // first/middle/last schedule. A refused new conversation must leave no entry.
    const scheduleIds=scheduled.pending.map(([id]:[string,unknown])=>id);
    expect(scheduleIds).not.toContain('alex');
    for(const id of new Set([scheduleIds[0],scheduleIds[Math.floor(scheduleIds.length/2)],scheduleIds.at(-1),'alex','address:refused-schedule%40example.test'])) {
      failCommit=true;
      await expect(call('sendMessage',[id,'Refused schedule','',202,6000])).rejects.toThrow('footprint commit refused');
      const restored=await inspect();
      expect(canonical(restored.live)).toEqual(canonical(scheduled.live));
      expect(restored.pending).toEqual(scheduled.pending);
      expect(restored.ticks).toBe(scheduled.ticks);
    }
    // Single removals must restore first, middle and last insertion positions.
    // Other pending schedules and the clock must survive either refusal.
    for(const id of new Set([scheduleIds[0],scheduleIds[Math.floor(scheduleIds.length/2)],scheduleIds.at(-1)])){
      for(const [source,args] of [['blockConversation',[id,true]],['deleteConversation',[id,0]]] as [string,unknown[]][]){
        failCommit=true;
        await expect(call(source,args)).rejects.toThrow('footprint commit refused');
        const restored=await inspect();
        expect(canonical(restored.live)).toEqual(canonical(scheduled.live));
        expect(restored.pending).toEqual(scheduled.pending);
        expect(restored.ticks).toBe(scheduled.ticks);
      }
    }
    // Refused edits must retain the precise reply order and clock, including
    // sources that replace or delete scheduled activity before persistence.
    for(const [source,args] of [
      ['sendMessage',['maya','Refused replacement schedule','',202,6000]],
      ['blockConversation',['maya',true]],
      ['deleteConversation',['maya',0]],
      ['blockConversation',['alex',true]],
      ['deleteConversation',['alex',0]],
      ['advanceReplies',[216,'maya',216000]],
    ] as [string,unknown[]][]) {
      failCommit=true;
      await expect(call(source,args)).rejects.toThrow('footprint commit refused');
      const restored=await inspect();
      expect(canonical(restored.live)).toEqual(canonical(scheduled.live));
      expect(restored.pending).toEqual(scheduled.pending);
      expect(restored.ticks).toBe(scheduled.ticks);
    }
    // Alex has no pending reply; changing block state must retain the other
    // schedules on both successful writes and a refused unblock.
    const blockedAlex=await act('blockConversation',['alex',true]);
    expect(blockedAlex.pending).toEqual(scheduled.pending);
    failCommit=true;
    await expect(call('blockConversation',['alex',false])).rejects.toThrow('footprint commit refused');
    const refusedUnblock=await inspect();
    expect(canonical(refusedUnblock.live)).toEqual(canonical(blockedAlex.live));
    expect(refusedUnblock.pending).toEqual(scheduled.pending);
    expect(refusedUnblock.ticks).toBe(scheduled.ticks);
    expect((await act('blockConversation',['alex',false])).pending).toEqual(scheduled.pending);
    // An unblock also preserves an existing schedule when already unblocked.
    expect((await act('blockConversation',['maya',false])).pending).toEqual(scheduled.pending);
    const before=canonical((await inspect()).live);
    await expect(call('sendMessage',['maya','🌲'.repeat(20000),'',200,6000])).rejects.toThrow('UTF-8');
    expect(canonical((await inspect()).live)).toEqual(before);
    failCommit=true;
    await expect(call('saveDraft',['maya','Refused draft',''])).rejects.toThrow('footprint commit refused');
    expect(canonical((await inspect()).live)).toEqual(before);
    expect((await inspect()).pending).toEqual(scheduled.pending);
    expect((await inspect()).ticks).toBe(scheduled.ticks);
    await act('saveDraft',['maya','Retry draft','']);
    expect((await inspect()).pending).toEqual(scheduled.pending);
    // Idle ticks retain the clock without advancing records. Equality at a
    // deadline must run; rewinding below a crossed receipt re-arms that event.
    await act('advanceReplies',[1000,'maya',1000000]);
    const clockStart=await act('sendMessage',['maya','Clock boundary','',2000,2000000]);
    const sentKey=clockStart.live.find(([,row]:any)=>row.kind==='message' && row.message.body==='Clock boundary')[0];
    for(const now of [2000,2001,2002.999]) {
      const idle=await act('advanceReplies',[now,'maya',now*1000]);
      expect(idle.revision).toBe(clockStart.revision);expect(idle.ticks).toBe(now);
      expect(canonical(idle.live)).toEqual(canonical(clockStart.live));
    }
    failCommit=true;
    await expect(call('advanceReplies',[2003,'maya',2003000])).rejects.toThrow('footprint commit refused');
    const refusedReceipt=await inspect();
    expect(canonical(refusedReceipt.live)).toEqual(canonical(clockStart.live));
    expect(refusedReceipt.pending).toEqual(clockStart.pending);
    expect(refusedReceipt.ticks).toBe(2002.999);
    const receipt=await act('advanceReplies',[2003,'maya',2003000]);
    expect(receipt.live.find(([key]:[string,unknown])=>key===sentKey)[1].message.delivery).toBe('Read');
    expect(receipt.revision).toBe(refusedReceipt.revision+1);
    expect((await act('advanceReplies',[2003,'maya',2003000])).revision).toBe(receipt.revision);
    await act('advanceReplies',[2002,'maya',2002000]);
    expect((await act('advanceReplies',[2003,'maya',2003000])).revision).toBe(receipt.revision+1);
    const beforeReply=await act('advanceReplies',[2014.999,'maya',2014999]);
    failCommit=true;
    await expect(call('advanceReplies',[2015,'maya',2015000])).rejects.toThrow('footprint commit refused');
    const refusedReply=await inspect();
    expect(canonical(refusedReply.live)).toEqual(canonical(beforeReply.live));
    expect(refusedReply.pending).toEqual(beforeReply.pending);expect(refusedReply.ticks).toBe(beforeReply.ticks);
    const retriedReply=await act('advanceReplies',[2015,'maya',2015000]);
    expect(retriedReply.pending).toEqual([]);expect(retriedReply.live.length).toBe(beforeReply.live.length+1);
    // Newly earlier schedules must wake the model; later replacement or removal
    // may cause an extra scan but must not deliver the old scheduled reply.
    await act('sendMessage',['maya','Later clock','',2100,2100000]);
    await act('sendMessage',['dad','Earlier clock','',2050,2050000]);
    const earlier=await act('advanceReplies',[2053,'maya',2053000]);
    const delivery=(body:string)=>earlier.live.find(([,row]:any)=>row.kind==='message' && row.message.body===body)[1].message.delivery;
    expect(delivery('Earlier clock')).toBe('Read');expect(delivery('Later clock')).toBe('Delivered');
    await act('advanceReplies',[2065,'maya',2065000]);
    await act('blockConversation',['maya',true]);
    expect((await act('advanceReplies',[2103,'maya',2103000])).pending).toEqual([]);
    await act('blockConversation',['maya',false]);
    await act('sendMessage',['maya','Replace deadline','',2200,2200000]);
    const replaced=await act('sendMessage',['maya','Later replacement','',2300,2300000]);
    expect((await act('advanceReplies',[2215,'maya',2215000])).revision).toBe(replaced.revision);
    expect((await act('advanceReplies',[2303,'maya',2303000])).revision).toBe(replaced.revision+1);
    // One tick can read and reply to one thread, read another, and leave a
    // future thread alone. Refusal restores the captured receipt rows and order.
    await act('sendMessage',['maya','Mixed due reply','',3000,3000000]);
    await act('sendMessage',['dad','Mixed due receipt','',3012,3012000]);
    const beforeMixedDue=await act('sendMessage',['sam','Mixed future reply','',4000,4000000]);
    failCommit=true;
    await expect(call('advanceReplies',[3015,'maya',3015000])).rejects.toThrow('footprint commit refused');
    const refusedMixedDue=await inspect();
    expect(canonical(refusedMixedDue.live)).toEqual(canonical(beforeMixedDue.live));
    expect(refusedMixedDue.pending).toEqual(beforeMixedDue.pending);
    expect(refusedMixedDue.ticks).toBe(beforeMixedDue.ticks);
    const mixedDue=await act('advanceReplies',[3015,'maya',3015000]);
    for(const [body,delivery] of [['Mixed due reply','Read'],['Mixed due receipt','Read'],['Mixed future reply','Delivered']]){
      expect(mixedDue.live.find(([,row]:any)=>row.kind==='message' && row.message.body===body)[1].message.delivery).toBe(delivery);
    }
    expect(mixedDue.live.length).toBe(beforeMixedDue.live.length+1);
    expect(mixedDue.pending).toEqual(beforeMixedDue.pending.filter(([id]:[string,unknown])=>id!=='maya'));
    // Sparse reply removals restore original insertion order at every position,
    // with survivors between removals and with no survivors at all.
    await act('advanceReplies',[10000,'maya',10000000]);
    const replyIds=['maya','dad','sam','alex','jules'];
    for(const id of replyIds)await act('blockConversation',[id,false]);
    for(const removed of [[0],[2],[4],[0,2,4],[0,1,2,3,4]]){
      await act('advanceReplies',[10000,'maya',10000000]);
      for(let i=0;i<replyIds.length;i++)await act('sendMessage',[replyIds[i],'Ordered reply removal','',removed.includes(i)?5000:6000,5000000]);
      const before=await inspect();
      expect(before.pending.map(([id]:[string,unknown])=>id)).toEqual(replyIds);
      failCommit=true;
      await expect(call('advanceReplies',[5015,'maya',5015000])).rejects.toThrow('footprint commit refused');
      const refused=await inspect();
      expect(canonical(refused.live)).toEqual(canonical(before.live));
      expect(refused.pending).toEqual(before.pending);expect(refused.ticks).toBe(before.ticks);
      const retried=await act('advanceReplies',[5015,'maya',5015000]);
      expect(retried.pending).toEqual(before.pending.filter((_:unknown,i:number)=>!removed.includes(i)));
      expect(retried.live.length).toBe(before.live.length+removed.length);
    }
    // Expiry deadlines survive earlier insertions, strict equality, refusal,
    // rewind, removal of the earliest archive, and reopening the durable model.
    const day=86400000;
    await act('recentlyDeleted',['',0,40*day,'']);
    const archiveOne=async(id:string,body:string,now:number)=>{
      const sent=await act('sendMessage',[id,body,'',0,0]);
      const [key,row]=sent.live.find(([,row]:any)=>row.kind==='message' && row.message.body===body);
      await act('deleteMessages',[id,row.message.id,now]);return key;
    };
    const laterExpiry=await archiveOne('maya','Later archive expiry',10*day);
    const earlierExpiry=await archiveOne('dad','Earlier archive expiry',0);
    const beforeExpiry=await act('recentlyDeleted',['',0,30*day-.5,'']);
    expect(new Map(beforeExpiry.live).has(earlierExpiry)).toBe(true);
    failCommit=true;
    await expect(call('recentlyDeleted',['',0,30*day,''])).rejects.toThrow('footprint commit refused');
    expect(canonical((await inspect()).live)).toEqual(canonical(beforeExpiry.live));
    const expired=await act('recentlyDeleted',['',0,30*day,'']);
    expect(new Map(expired.live).has(earlierExpiry)).toBe(false);
    expect(new Map(expired.live).has(laterExpiry)).toBe(true);
    expect(canonical((await act('recentlyDeleted',['',0,5*day,''])).live)).toEqual(canonical(expired.live));
    await act('recoverConversations',['maya',39*day]);
    const reopenedExpiry=await archiveOne('dad','Reopened archive expiry',9*day);
    app=await import(output+`?instance=${generation++}`);
    await call('conversation',['maya',0,'','','']);
    expect(new Map((await act('recentlyDeleted',['',0,39*day-.5,''])).live).has(reopenedExpiry)).toBe(true);
    expect(new Map((await act('recentlyDeleted',['',0,39*day,''])).live).has(reopenedExpiry)).toBe(false);
    const nonfiniteExpiry=await archiveOne('maya','Nonfinite expiry clock',10*day);
    expect(new Map((await act('recentlyDeleted',['',0,NaN,''])).live).has(nonfiniteExpiry)).toBe(false);
    // One archive can contain different deadlines in insertion order. Its
    // summary minimum must advance after partial expiry and rebuild on reopen.
    await archiveOne('maya','Summary latest',10*day);
    await archiveOne('maya','Summary earliest',0);
    await archiveOne('maya','Summary middle',9*day);
    const summary=async(now:number)=>{
      const result=await call('recentlyDeleted',['maya',0,now,'']);await inspect();
      return result.people.find((p:any)=>p.id==='maya');
    };
    expect(await summary(0)).toMatchObject({count:3,days:30,chosen:true});
    expect(await summary(30*day)).toMatchObject({count:2,days:9});
    failCommit=true;
    await expect(call('recentlyDeleted',['maya',0,39*day,''])).rejects.toThrow('footprint commit refused');
    expect(await summary(30*day)).toMatchObject({count:2,days:9});
    app=await import(output+`?instance=${generation++}`);
    await call('conversation',['maya',0,'','','']);
    expect(await summary(30*day)).toMatchObject({count:2,days:9});
    expect(await summary(39*day)).toMatchObject({count:1,days:1});
    expect(await summary(40*day)).toBeUndefined();
    // Appending to an existing archive may lower its deadline. A refused save
    // must undo both rows and the minimum, including before reopen and retry.
    const appendFirst=await archiveOne('maya','Append existing archive',10*day);
    const appendKeys=[appendFirst];
    for(const body of ['Append second','Append third']){
      const sent=await act('sendMessage',['maya',body,'',0,0]);
      appendKeys.push(sent.live.find(([,row]:any)=>row.kind==='message' && row.message.body===body)[0]);
    }
    const beforeAppendDelete=await inspect();
    const appendIds=appendKeys.slice(1).map(key=>new Map(beforeAppendDelete.live).get(key) as any).map(row=>row.message.id).join('|');
    failCommit=true;
    await expect(call('deleteMessages',['maya',appendIds,0])).rejects.toThrow('footprint commit refused');
    const refusedAppend=await inspect();
    expect(canonical(refusedAppend.live)).toEqual(canonical(beforeAppendDelete.live));
    expect(refusedAppend.pending).toEqual(beforeAppendDelete.pending);
    expect(await summary(0)).toMatchObject({count:1,days:40});
    await act('deleteMessages',['maya',appendIds,0]);
    expect(await summary(0)).toMatchObject({count:3,days:30});
    const appended=await inspect();
    app=await import(output+`?instance=${generation++}`);
    await call('conversation',['maya',0,'','','']);
    expect(canonical((await inspect()).live)).toEqual(canonical(appended.live));
    expect(await summary(0)).toMatchObject({count:3,days:30});
    const recoveredAppend=await act('recoverConversations',['maya',0]);
    expect(recoveredAppend.live.filter(([key]:[string,unknown])=>appendKeys.includes(key)).map(([key,row]:[string,any])=>[key,row.expires]))
      .toEqual(appendKeys.map(key=>[key,null]));
    expect(await summary(0)).toBeUndefined();
    // A due bucket must not expire future buckets. Skipped buckets still
    // contribute to the next global deadline, including after failed admission.
    const futureBucket=await archiveOne('maya','Future bucket expiry',20*day);
    const middleBucket=await archiveOne('dad','Middle bucket expiry',10*day);
    const dueBucket=await archiveOne('sam','Due bucket expiry',0);
    const beforeBucketExpiry=await inspect();
    failCommit=true;
    await expect(call('recentlyDeleted',['',0,30*day,''])).rejects.toThrow('footprint commit refused');
    expect(canonical((await inspect()).live)).toEqual(canonical(beforeBucketExpiry.live));
    const firstBucketExpiry=new Map((await act('recentlyDeleted',['',0,30*day,''])).live);
    expect(firstBucketExpiry.has(dueBucket)).toBe(false);
    expect(firstBucketExpiry.has(middleBucket)).toBe(true);
    expect(firstBucketExpiry.has(futureBucket)).toBe(true);
    const secondBucketExpiry=await act('recentlyDeleted',['',0,40*day,'']);
    expect(new Map(secondBucketExpiry.live).has(middleBucket)).toBe(false);
    expect(new Map(secondBucketExpiry.live).has(futureBucket)).toBe(true);
    expect(canonical((await act('recentlyDeleted',['',0,5*day,''])).live)).toEqual(canonical(secondBucketExpiry.live));
    expect(new Map((await act('recentlyDeleted',['',0,50*day,''])).live).has(futureBucket)).toBe(false);
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
    // The existing 512-record limit can refuse a whole due batch before disk
    // admission. Its larger removal set must also restore every schedule.
    for(let i=0;i<260;i++)await call('sendMessage',[`address:reply-cap-${i}%40example.test`,'Batch reply','',0,0]);
    const beforeCap=await inspect();
    expect(beforeCap.pending.length).toBe(260);
    await expect(call('advanceReplies',[15,'maya',15000])).rejects.toThrow('at most 512 records');
    const refusedCap=await inspect();
    expect(canonical(refusedCap.live)).toEqual(canonical(beforeCap.live));
    expect(refusedCap.pending).toEqual(beforeCap.pending);expect(refusedCap.ticks).toBe(beforeCap.ticks);
    expect((await act('saveDraft',['maya','After refused reply batch',''])).pending).toEqual(beforeCap.pending);
    // A single conversation may itself exceed the atomic edit cap. Its one
    // scheduled reply must survive refusal along with the full transcript.
    const oversized='address:single-removal-cap%40example.test';
    for(let i=0;i<514;i++)await call('sendMessage',[oversized,`Large transcript ${i}`,'',100,100000]);
    const beforeLargeDelete=await inspect();
    await expect(call('deleteConversation',[oversized,0])).rejects.toThrow('at most 512 records');
    const refusedLargeDelete=await inspect();
    expect(canonical(refusedLargeDelete.live)).toEqual(canonical(beforeLargeDelete.live));
    expect(refusedLargeDelete.pending).toEqual(beforeLargeDelete.pending);
    expect(refusedLargeDelete.ticks).toBe(beforeLargeDelete.ticks);
  }finally{globalThis.fetch=originalFetch;sql.close();await rm(dir,{recursive:true,force:true});}
},30000);
