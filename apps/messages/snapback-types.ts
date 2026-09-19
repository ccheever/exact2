export type Request = Record<string,unknown>;
export interface NativeModule {call(request:Request):Request}
export interface Core {call(request:Request):Promise<Request>}
export type Backend={generation:number;schema:{tables:Record<string,{columns:Record<string,unknown>}>};programs:{name:string}[]};
export type Queued={id:string;seq:number;op:string;args:Request;new_ids:string[];predicted:string[];viewer:string;now:number;predictable:boolean;sent_seq?:number|null;observed_seq?:number|null};
export type SyncState={acquired:boolean;backend_required:boolean;store_id:string|null;watermark:number;generation:number;send_revision:number;pending_ids:string[];restore_refusal?:unknown};

export function result<T>(answer:Request):T {
  const value=answer.denied?answer:answer.ok as Request|null;
  if(value?.denied) {
    const denied=value.denied as {code?:string;message?:string};
    throw new Error(`${denied.code||'E_STORE'}: ${denied.message||'Snapback refused the operation'}`);
  }
  return answer.ok as T;
}
