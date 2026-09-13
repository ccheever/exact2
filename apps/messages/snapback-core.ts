import type { Storage } from './app.contract.d.ts';
import { Interpreter, Refused } from './vendor/snapback4/interpreter';
import { Replica, type StreamPage } from './vendor/snapback4/replica';
import { ReplicaStore } from './snapback-store';

import type { Backend, Core, Queued, Request } from './snapback-types';

export async function browserCore(storage:Storage,path:string,backend:Backend):Promise<Core> {
  const store=await ReplicaStore.open(storage,path,backend.schema);
  const held=await store.read(tx=>tx.getMeta('backend')) as Backend|undefined;
  if(held)backend=held;
  store.setSchema(backend.schema);
  const replica=new Replica(store,backend.schema,{sync:async()=>{throw new Error('sync is owned by the app transport');}},backend.generation);
  await store.write(async tx=>{await tx.setMeta('backend',backend);if(!held)await tx.setMeta('generation',backend.generation);});
  const getProgram=(name:unknown)=>{
    const program=backend.programs.find(p=>p.name===name);
    if(!program)throw new Error(`Unknown Snapback operation ${name}`);
    return program;
  };
  return {async call(request) {
    try {
      let ok:unknown=null;
      switch(request.op) {
        case 'state':ok=await replica.state();break;
        case 'meta':ok=await store.read(tx=>tx.getMeta(String(request.key))) ?? null;break;
        case 'set_meta':await store.write(tx=>tx.setMeta(String(request.key),String(request.value)));break;
        case 'adopt':{
          const next=request.backend as Backend;
          if(next.generation!==backend.generation){await store.clearRows();await store.write(tx=>tx.setMeta('watermark',0));}
          backend=next;store.setSchema(next.schema);replica.adopt(next.schema,next.generation);
          await store.write(async tx=>{await tx.setMeta('backend',next);await tx.setMeta('generation',next.generation);});break;
        }
        case 'apply':{
          const touched=new Set<string>();await replica.apply(request.page as unknown as StreamPage,touched,request.first!==false);ok={touched:[...touched]};break;
        }
        case 'query':case 'predict':{
          const mode=request.op==='query'?'query':'mutation',program=getProgram(request.name);
          const run=async(tx:Parameters<Parameters<typeof store.read>[0]>[0])=>{
            const ids=[...((request.newIds as string[]|undefined)||[])];
            const interpreter=new Interpreter(tx,backend.schema,{viewer:String(request.viewer),args:(request.args as Request)||{},now:Number(request.now)||0,newIds:ids,mint:()=>{throw new Error('The caller must supply newIds');}},mode);
            const read=await interpreter.run(program);
            return mode==='query'?{...read,tables:[...read.tables]}:{result:read.data,written:interpreter.writes.map(w=>w.table),read:[...read.tables],predicted:interpreter.writes.map(w=>`${w.table}/${w.row.id}`)};
          };
          ok=mode==='query'?await store.read(run):await store.write(run);break;
        }
        case 'queued':ok=await store.read(async tx=>(await tx.side('outbox').all()).map(r=>r.value as Queued).sort((a,b)=>a.seq-b.seq));break;
        case 'enqueue':{const entry=request.entry as Queued;await store.write(tx=>tx.side('outbox').put(entry.id,entry));break;}
        case 'dequeue':await store.write(tx=>tx.side('outbox').delete(String(request.id)));break;
        case 'withdraw':await store.write(async tx=>{
          for(const key of request.predicted as string[]){const slash=key.indexOf('/'),table=key.slice(0,slash),id=key.slice(slash+1);if((await tx.get(table,id))?.pending)await tx.delete(table,id);}
        });break;
        case 'clear_rows':await store.clearRows();await store.write(tx=>tx.setMeta('watermark',0));break;
        default:throw new Error(`Unsupported Snapback core operation ${request.op}`);
      }
      return {ok};
    } catch(error) {
      return {denied:error instanceof Refused?error.refusal:{code:'E_STORE',family:'store',message:error instanceof Error?error.message:String(error)}};
    }
  }};
}
