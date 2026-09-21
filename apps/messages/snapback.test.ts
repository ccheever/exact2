import { expect, test } from 'bun:test';
import { Database } from 'bun:sqlite';
import { copyFile, mkdir, mkdtemp, rm } from 'node:fs/promises';
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
  const sql=new Database(join(dir,'device.sqlite'));
  const storage={sqlite:{open:async()=>({
    execute:async(text:string,params:unknown[]=[])=>sql.prepare(text).run(...params as never[]),
    query:async(text:string,params:unknown[]=[])=>({rows:sql.prepare(text).values(...params as never[])}),
    transaction:async(commands:{sql:string;params?:unknown[]}[])=>{
      if(failCommit && commands.some(command=>command.params?.[0]==='o')){
        failCommit=false;throw new Error('disk fixture refused commit');
      }
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
    recordQueries=0;await sync();expect(recordQueries).toBe(0);
    online=false;await sync();expect(recordQueries).toBe(0);
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
  }finally{
    globalThis.fetch=originalFetch;server.kill();await server.exited;sql.close();await rm(dir,{recursive:true,force:true});
  }
},30000);
