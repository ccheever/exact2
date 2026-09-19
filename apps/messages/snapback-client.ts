import type { Storage } from './app.contract.d.ts';
import { backend as compiledBackend } from './snapback/backend';
import { browserCore } from './snapback-core';
import { result, type Backend, type Core, type NativeModule, type Queued, type Request, type SyncState } from './snapback-types';

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
const payloadLimit=(backend.schema.tables.records.columns.payload as {Json:{max_bytes:number}}).Json.max_bytes;
// @ref LLP 1027.001 D2 — standard UTF-8 on every executor
const encoder = new TextEncoder();
function jsonBytes(text:string):number { return encoder.encode(text).byteLength; }
// Rust's JSON objects have sorted keys. Compare values independent of the
// app's insertion order so a read never manufactures another pending edit.
function canonical(value:unknown):string|undefined {
  return JSON.stringify(value,(_key,row)=>row && typeof row==='object' && !Array.isArray(row)
    ?Object.fromEntries(Object.keys(row).sort().map(key=>[key,row[key]])):row);
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
    client.error=await client.call<string|null>({op:'meta',key:'exact:save-error'})||'';
    client.queued=(await client.call<Queued[]>({op:'queued'})).length;
    client.held=await client.read();
    return client;
  }
  private async call<T>(request:Request):Promise<T>{return result<T>(await this.core.call(request));}
  private async next():Promise<number>{const n=++this.counter;await this.call({op:'set_meta',key:'exact:counter',value:String(n)});return n;}
  private recordId(key:string):string{return `${viewer}:${encodeURIComponent(key)}`;}
  async read():Promise<Records> {
    const rows:Records=new Map();let cursor:string|null=null;
    if((await this.call<SyncState>({op:'sync_state'})).acquired)do {
      const page:{data:{key:string;payload:unknown}[];complete:boolean;next:string|null}=await this.call({op:'query',name:'records',viewer,args:{c:cursor},now:0});
      if(!page.complete && !page.next)throw new Error('Messages replica cannot read its complete local records');
      for(const row of page.data)if(row.payload!==null)rows.set(row.key,row.payload);
      cursor=page.next;
    }while(cursor);
    // Before acquisition the device holds intents, not server facts. Render
    // our own queued record edits as pending local content, including on reopen.
    for(const entry of await this.call<Queued[]>({op:'queued'})) {
      const keys=entry.args.keys as string[],payloads=entry.args.payloads as unknown[];
      for(let i=0;i<keys.length;i++) {
        if(entry.op==='seedRecords' && rows.has(keys[i]))continue;
        if(payloads[i]===null)rows.delete(keys[i]);else rows.set(keys[i],payloads[i]);
      }
    }
    return rows;
  }
  initial():Records{return this.held;}
  status():string {return this.error || (this.queued?`${this.queued} change${this.queued===1?'':'s'} saved on this device; waiting to sync.`:this.online?'Synced':'Saved on this device.');}
  async failed(error:unknown):Promise<void> {this.error=`Could not save: ${error instanceof Error?error.message:String(error)}`;await this.call({op:'set_meta',key:'exact:save-error',value:this.error});}
  async seed(records:Records):Promise<void> {
    if(await this.call({op:'meta',key:'exact:initialized'}))return;
    await this.persist(records,true);
    this.held=await this.read();
    await this.call({op:'set_meta',key:'exact:initialized',value:'1'});
  }
  async persist(records:Records,seed=false):Promise<void> {
    const changes:[string,unknown][]=[];
    // Validate the entire edit before admitting one atomic mutation.
    for(const key of new Set([...(seed?[]:this.held.keys()),...records.keys()])) {
      const payload=records.has(key)?records.get(key):null,text=JSON.stringify(payload);
      if(canonical(this.held.get(key)??null)===canonical(payload))continue;
      if(text===undefined || jsonBytes(text)>payloadLimit)throw new Error(`A Messages record exceeds ${payloadLimit} UTF-8 bytes.`);
      if([...key].length>442)throw new Error('A Messages record key is too long.');
      changes.push([key,payload]);
    }
    if(!changes.length)return;
    if(changes.length>512)throw new Error('A Messages edit can change at most 512 records.');
    if(this.error){await this.call({op:'set_meta',key:'exact:save-error',value:''});this.error='';}
    const seq=await this.next(),id=`${this.device}:${seq}`;
    const entry:Queued={id,seq,op:seed?'seedRecords':'putRecords',args:{
      recordIds:changes.map(([key])=>this.recordId(key)),
      keys:changes.map(([key])=>key),payloads:changes.map(([,payload])=>payload),
    },new_ids:[],predicted:[],viewer,now:0,predictable:true};
    await this.call({op:'admit',entry});this.queued++;
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
  private async acquire(local:<T>(work:()=>Promise<T>)=>Promise<T>):Promise<void> {
    let watermark=(await local(()=>this.call<SyncState>({op:'sync_state'}))).watermark;
    let after:string|undefined,catchingUp=false;
    for(let pages=0;pages<100_000;pages++) {
      const captured=await local(()=>this.call<SyncState>({op:'sync_state'}));
      if(captured.restore_refusal)throw new Error(JSON.stringify(captured.restore_refusal));
      const page=await this.request('/sync',{from:watermark,limit:4000,after:after??null,
        stream:catchingUp,pending:captured.pending_ids,...(captured.store_id?{store_id:captured.store_id}:{})});
      page.send_revision=captured.send_revision;page.requested_watermark=watermark;page.captured_watermark=captured.watermark;
      let reset=false,backendRequired=captured.backend_required;
      if(typeof page.store_id==='string' && page.store_id!==captured.store_id || Number(page.watermark)<watermark) {
        const observed=await local(()=>this.call<{reset:boolean;backend_required:boolean;send_revision:number}>({op:'observe_store',page}));
        reset=observed.reset;backendRequired=observed.backend_required;page.send_revision=observed.send_revision;
        if(reset)page.captured_watermark=0;
      }
      if(reset || backendRequired || Number(page.generation)!==captured.generation) {
        const fresh=await this.request('/schema') as unknown as Backend;
        if(!fresh.schema?.tables.records || !fresh.programs?.some(p=>p.name==='putRecords'))throw new Error('The local Snapback origin is not the Messages backend');
        await local(()=>this.call({op:'adopt',backend:fresh,store_id:page.store_id??captured.store_id,send_revision:page.send_revision}));
        watermark=0;after=undefined;catchingUp=false;continue;
      }
      if(Number(page.watermark)<watermark || catchingUp && page.snapshot || page.snapshot && page.more && !page.next)throw new Error('Invalid Snapback sync page');
      page.stage_snapshot=!!page.snapshot;page.snapshot_catchup=catchingUp;
      await local(()=>this.call({op:'apply',page,first:!catchingUp && after===undefined}));
      if(page.snapshot && page.more){after=String(page.next);continue;}
      after=undefined;watermark=Number(page.watermark);
      if(page.snapshot){catchingUp=true;continue;}
      if(!page.more)return;
    }
    throw new Error('Snapback sync exceeded its page bound');
  }
  async sync(now:number,local:<T>(work:()=>Promise<T>)=>Promise<T>,settle:(records:Records)=>void):Promise<void> {
    if(this.syncing || (now>=this.lastSync && now-this.lastSync<3000))return;
    this.syncing=true;this.lastSync=now;
    try {
      // Establish the server store identity before replaying any old outbox.
      await this.acquire(local);
      for(const entry of await local(()=>this.call<Queued[]>({op:'queued'}))) {
        if(entry.sent_seq!=null)continue;
        if(entry.observed_seq!=null){
          await local(()=>this.call({op:'settle',id:entry.id,seq:entry.observed_seq}));continue;
        }
        await local(()=>this.call({op:'begin_send'}));
        const sent=await this.request(`/m/${entry.op}`,{id:entry.id,args:entry.args,newIds:entry.new_ids});
        if(sent.state!=='sent' && sent.state!=='failed')throw new Error('Snapback returned an unsettled write');
        if((sent.why as {retryable?:boolean}|undefined)?.retryable)throw new Error('Snapback asked to retry the write');
        if(sent.state==='sent' && (!Number.isSafeInteger(sent.seq) || Number(sent.seq)<0))throw new Error('Snapback returned an invalid write receipt');
        await local(async()=>{
          await this.call({op:'settle',id:entry.id,...(sent.state==='sent'?{seq:sent.seq}:{})});
          if(sent.state==='failed')await this.failed(JSON.stringify(sent.why||sent.denied));
        });
      }
      await this.acquire(local);
      this.online=true;
    }catch(error){this.online=false;console.info('Messages is offline; edits remain in the Snapback outbox.',error instanceof Error?error.message:String(error));}
    finally{
      try {
      await local(async()=>{
        this.queued=(await this.call<Queued[]>({op:'queued'})).length;
        const current=await this.read();
        const changed=canonical([...current].sort())!==canonical([...this.held].sort());
        this.held=current;if(changed)settle(current);
      });
      }finally{this.syncing=false;}
    }
  }
}
