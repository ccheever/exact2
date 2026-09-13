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
const restoring='The server rejected a change; waiting to restore its saved version.';
const payloadLimit=(backend.schema.tables.records.columns.payload as {Json:{max_bytes:number}}).Json.max_bytes;
// UTF-8 bytes without a browser-only TextEncoder in the native data module.
function jsonBytes(text:string):number {
  let bytes=0;
  for(let i=0;i<text.length;i++){
    const n=text.charCodeAt(i);
    if(n<128)bytes++;else if(n<2048)bytes+=2;
    else if(n>=0xd800 && n<=0xdbff && text.charCodeAt(i+1)>=0xdc00 && text.charCodeAt(i+1)<=0xdfff){bytes+=4;i++;}
    else bytes+=3;
  }
  return bytes;
}

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
  private needsSnapshot=false;
  namespace='';
  private constructor(private core:Core) {}
  static async open(storage:Storage,core:Core|undefined):Promise<MessagesReplica> {
    const client=new MessagesReplica(core||await browserCore(storage,path,backend));
    client.device=await client.call<string>({op:'meta',key:'exact:device'});
    if(!client.device)throw new Error('Snapback did not provide a durable device identity');
    client.counter=Number(await client.call<string|null>({op:'meta',key:'exact:counter'}))||0;
    client.namespace=`${client.device}:${await client.next()}:`;
    client.generation=(await client.call<{generation:number}>({op:'state'})).generation;
    client.needsSnapshot=await client.call({op:'meta',key:'exact:needs-snapshot'})==='1';
    client.error=await client.call<string|null>({op:'meta',key:'exact:save-error'})||'';
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
  status():string {return (this.needsSnapshot?restoring:this.error) || (this.queued?`${this.queued} change${this.queued===1?'':'s'} saved on this device; waiting to sync.`:this.online?'Synced':'Saved on this device.');}
  async failed(error:unknown):Promise<void> {this.error=`Could not save: ${error instanceof Error?error.message:String(error)}`;await this.call({op:'set_meta',key:'exact:save-error',value:this.error});}
  async seed(records:Records):Promise<void> {
    if(await this.call({op:'meta',key:'exact:initialized'}))return;
    await this.persist(records,true);
    this.held=await this.read();
    await this.call({op:'set_meta',key:'exact:initialized',value:'1'});
  }
  async persist(records:Records,seed=false):Promise<void> {
    const changes:[string,unknown][]=[];
    // Validate the entire edit before queueing any of its records. Both twins
    // use the schema's byte limit, even if a replica interpreter is permissive.
    for(const key of new Set([...(seed?[]:this.held.keys()),...records.keys()])) {
      const payload=records.has(key)?records.get(key):null,text=JSON.stringify(payload);
      if(JSON.stringify(this.held.get(key)??null)===text)continue;
      if(text===undefined || jsonBytes(text)>payloadLimit)throw new Error(`A Messages record exceeds ${payloadLimit} UTF-8 bytes.`);
      if([...key].length>1024)throw new Error('A Messages record key is too long.');
      changes.push([key,payload]);
    }
    if(!changes.length)return;
    if(changes.length>512)throw new Error('A Messages edit can change at most 512 records.');
    if(this.error){await this.call({op:'set_meta',key:'exact:save-error',value:''});this.error='';}
    const seq=await this.next(),id=`${this.device}:${seq}`;
    const entry:Queued={id,seq,op:seed?'seedRecords':'putRecords',args:{
      recordIds:changes.map(([key])=>this.recordId(key)),
      keys:changes.map(([key])=>key),payloads:changes.map(([,payload])=>payload),
    },new_ids:[],predicted:[]};
    // Queue one complete edit first. A crash before prediction leaves that same
    // atomic mutation to replay. Both interpreters commit all related rows in
    // one transaction, so a refused row cannot leave a changed preview or draft.
    await this.call({op:'enqueue',entry});this.queued++;
    try {
      await this.call({op:'predict',name:entry.op,viewer,args:entry.args,now:0,newIds:[],entropy:seq});
    }catch(error){await this.call({op:'dequeue',id});this.queued--;throw error;}
    // Publish a new complete snapshot only after the durable prediction commits.
    // Seeding rereads the store, because seedRecords preserves existing rows.
    if(!seed){
      const committed=new Map(this.held);
      for(const [key,payload] of changes){if(payload===null)committed.delete(key);else committed.set(key,payload);}
      this.held=committed;
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
      if(!fresh.schema?.tables.records || !fresh.programs?.some(p=>p.name==='putRecords'))throw new Error('The local Snapback origin is not the Messages backend');
      const changedGeneration=fresh.generation!==this.generation;
      for(const entry of await local(()=>this.call<Queued[]>({op:'queued'}))) {
        const sent=await this.request(`/m/${entry.op}`,{id:entry.id,args:entry.args,newIds:entry.new_ids});
        if(sent.state!=='sent' && sent.state!=='failed')throw new Error('Snapback returned an unsettled write');
        await local(async()=>{
        if(sent.state==='failed'){
          // Keep the durable partition until a replacement actually arrives.
          // Persist this obligation before removing the rejected outbox entry.
          await this.call({op:'set_meta',key:'exact:needs-snapshot',value:'1'});
          this.needsSnapshot=true;
          console.warn('Snapback rejected a Messages edit',sent.why||sent.denied);
        }
        await this.call({op:'dequeue',id:entry.id});this.queued--;
        });
      }
      let watermark=this.needsSnapshot||changedGeneration?0:(await local(()=>this.call<{watermark:number}>({op:'state'}))).watermark;
      let after:string|undefined,first=true;
      for(;;){
        const page=await this.request(`/sync?from=${watermark}&limit=4000${after?`&after=${encodeURIComponent(after)}`:''}`);
        await local(async()=>{
          if(first && changedGeneration){await this.call({op:'adopt',backend:fresh});this.generation=fresh.generation;}
          await this.call({op:'apply',page,first});
        });first=false;
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
      if(this.needsSnapshot){await this.call({op:'set_meta',key:'exact:needs-snapshot',value:'0'});this.needsSnapshot=false;}
      this.held=current;this.online=true;
      if(changed)settle(current);
      });
    }catch(error){this.online=false;console.info('Messages is offline; edits remain in the Snapback outbox.',error instanceof Error?error.message:String(error));}
    finally{this.syncing=false;}
  }
}
