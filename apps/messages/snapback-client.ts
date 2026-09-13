import type { Storage } from './app.contract.d.ts';
import { backend as compiledBackend } from './snapback/backend';
import { browserCore } from './snapback-core';
import { result, type Backend, type Core, type NativeModule, type Queued, type Request } from './snapback-types';

// A development persona is explicit and restricted to this local origin. Change
// these together when running a different local Snapback project; a production
// deployment must provide its authenticated session instead of a dev persona.
export const origin='http://127.0.0.1:4400';
export const persona='alice';
export const viewer=`dev:${persona}`;
export const path=`app:/data/messages-${encodeURIComponent(`${origin}:${viewer}`)}.sqlite`;
export const grants=`sqlite.open ${path}\nnet.fetch ${origin}`;
export type Records=Map<string,unknown>;
const backend=compiledBackend as unknown as Backend;
const headers={'content-type':'application/json','x-snapback-persona':persona};
const absent='native storage is unavailable during bake or in an unconfigured host';

export function nativeCore(native:NativeModule|undefined|null):Core|null|undefined {
  if(!native)return undefined;
  try {
    try {result(native.call({op:'open',origin,viewer,path,backend:null}));}
    catch(error){
      const reason=error instanceof Error?error.message:String(error);
      if(reason==='this Snapback4 partition needs the server\'s backend once' || reason.includes('this device has never synced'))result(native.call({op:'open',origin,viewer,path,backend}));
      else throw error;
    }
  }catch(error){if(error instanceof Error && error.message===absent)return null;throw error;}
  return {call:async request=>native.call(request)};
}
export class MessagesReplica {
  private held:Records=new Map();
  private counter=0;
  private device='';
  private lastSync=-Infinity;
  private generation=backend.generation;
  private syncing=false;
  private queued=0;
  private online=false;
  private error='';
  namespace='';
  private constructor(private core:Core) {}
  static async open(storage:Storage,core:Core|undefined):Promise<MessagesReplica> {
    const client=new MessagesReplica(core||await browserCore(storage,path,backend));
    client.device=await client.call<string>({op:'meta',key:'exact:device'});
    if(!client.device)throw new Error('Snapback did not provide a durable device identity');
    client.counter=Number(await client.call<string|null>({op:'meta',key:'exact:counter'}))||0;
    client.namespace=`${client.device}:${await client.next()}:`;
    client.generation=(await client.call<{generation:number}>({op:'state'})).generation;
    // Reconstruct a prediction if the process stopped after queueing it.
    for(const entry of await client.call<Queued[]>({op:'queued'}))await client.call({op:'predict',name:entry.op,viewer,args:entry.args,now:0,newIds:entry.new_ids,entropy:entry.seq});
    client.queued=(await client.call<Queued[]>({op:'queued'})).length;
    client.held=await client.read();
    return client;
  }
  private async call<T>(request:Request):Promise<T>{return result<T>(await this.core.call(request));}
  private async next():Promise<number>{const n=++this.counter;await this.call({op:'set_meta',key:'exact:counter',value:String(n)});return n;}
  private recordId(key:string):string{return `${viewer}:${encodeURIComponent(key)}`;}
  async read():Promise<Records> {
    const rows:Records=new Map();let cursor:string|null=null;
    do {
      const page:{data:{key:string;payload:unknown}[];complete:boolean;next:string|null}=await this.call({op:'query',name:'records',viewer,args:{c:cursor},now:0});
      if(!page.complete && !page.next)throw new Error('Messages replica cannot read its complete local records');
      for(const row of page.data)if(row.payload!==null)rows.set(row.key,row.payload);
      cursor=page.next;
    }while(cursor);
    return rows;
  }
  initial():Records{return this.held;}
  status():string {return this.error || (this.queued?`${this.queued} change${this.queued===1?'':'s'} saved on this device; waiting to sync.`:this.online?'Synced':'Saved on this device.');}
  failed(error:unknown):void {this.error=`Could not save: ${error instanceof Error?error.message:String(error)}`;}
  async seed(records:Records):Promise<void> {
    if(await this.call({op:'meta',key:'exact:initialized'}))return;
    await this.persist(records,true);
    this.held=await this.read();
    await this.call({op:'set_meta',key:'exact:initialized',value:'1'});
  }
  async persist(records:Records,seed=false):Promise<void> {
    for(const key of new Set([...(seed?[]:this.held.keys()),...records.keys()])) {
      const payload=records.has(key)?records.get(key):null;
      if(JSON.stringify(this.held.get(key)??null)===JSON.stringify(payload))continue;
      const seq=await this.next(),id=`${this.device}:${seq}`;
      const entry:Queued={id,seq,op:seed?'seedRecord':'putRecord',args:{recordId:this.recordId(key),key,payload},new_ids:[],predicted:[]};
      // Queue first: a crash before prediction still leaves an uploadable write.
      await this.call({op:'enqueue',entry});this.queued++;
      try {
        const prediction=await this.call<{predicted:string[]}>({op:'predict',name:entry.op,viewer,args:entry.args,now:0,newIds:[],entropy:seq});
        entry.predicted=prediction.predicted;
        // The native outbox rejects duplicate inserts, so prediction keys need
        // not be rewritten: they are derivable from this one-row operation.
      }catch(error){await this.call({op:'dequeue',id});this.queued--;throw error;}
      if(payload===null)this.held.delete(key);else this.held.set(key,payload);
    }
  }
  private async request(endpoint:string,body?:unknown):Promise<Request> {
    const response=await fetch(`${origin}${endpoint}`,{headers,...(body===undefined?{}:{method:'POST',body:JSON.stringify(body)})});
    if(!response.ok)throw new Error(`Snapback HTTP ${response.status}`);
    const value=await response.json() as Request;
    if(value.denied)result(value);
    return value;
  }
  async sync(now:number,local:<T>(work:()=>Promise<T>)=>Promise<T>,settle:(records:Records)=>void):Promise<void> {
    if(this.syncing || (now>=this.lastSync && now-this.lastSync<3000))return;
    this.syncing=true;this.lastSync=now;
    try {
      const fresh=await this.request('/schema') as unknown as Backend;
      if(!fresh.schema?.tables.records || !fresh.programs?.some(p=>p.name==='putRecord'))throw new Error('The local Snapback origin is not the Messages backend');
      if(fresh.generation!==this.generation)await local(async()=>{await this.call({op:'adopt',backend:fresh});this.generation=fresh.generation;});
      for(const entry of await local(()=>this.call<Queued[]>({op:'queued'}))) {
        const sent=await this.request(`/m/${entry.op}`,{id:entry.id,args:entry.args,newIds:entry.new_ids});
        if(sent.state!=='sent' && sent.state!=='failed')throw new Error('Snapback returned an unsettled write');
        await local(async()=>{
        await this.call({op:'dequeue',id:entry.id});this.queued--;
        if(sent.state==='failed'){
          await this.call({op:'withdraw',predicted:[`records/${String(entry.args.recordId)}`]});
          await this.call({op:'clear_rows'});
          this.error='The server rejected a change; the saved server version was restored.';
          console.warn('Snapback rejected a Messages edit',sent.why||sent.denied);
        }
        });
      }
      let watermark=(await local(()=>this.call<{watermark:number}>({op:'state'}))).watermark;
      let after:string|undefined,first=true;
      for(;;){
        const page=await this.request(`/sync?from=${watermark}&limit=4000${after?`&after=${encodeURIComponent(after)}`:''}`);
        await local(()=>this.call({op:'apply',page,first}));first=false;
        if(page.snapshot && page.more && page.next){after=String(page.next);continue;}
        after=undefined;watermark=Number(page.watermark);
        if(!page.more)break;
      }
      // A snapshot or authoritative image replaces predictions. Replay any
      // writes still queued so offline edits remain visible after reconnect.
      await local(async()=>{
      for(const entry of await this.call<Queued[]>({op:'queued'}))await this.call({op:'predict',name:entry.op,viewer,args:entry.args,now:0,newIds:entry.new_ids,entropy:entry.seq});
      const current=await this.read();
      const changed=JSON.stringify([...current].sort())!==JSON.stringify([...this.held].sort());
      this.held=current;this.online=true;
      if(changed)settle(current);
      });
    }catch(error){this.online=false;console.info('Messages is offline; edits remain in the Snapback outbox.',error instanceof Error?error.message:String(error));}
    finally{this.syncing=false;}
  }
}
