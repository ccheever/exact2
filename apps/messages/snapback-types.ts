import type { ProgramJson } from './vendor/snapback4/interpreter';
import type { SchemaJson } from './vendor/snapback4/store';

export type Request = Record<string,unknown>;
export interface NativeModule {call(request:Request):Request}
export interface Core {call(request:Request):Promise<Request>}
export type Backend={generation:number;schema:SchemaJson;programs:ProgramJson[]};
export type Queued={id:string;seq:number;op:string;args:Request;new_ids:string[];predicted:string[]};

export function result<T>(answer:Request):T {
  const value=answer.denied?answer:answer.ok as Request|null;
  if(value?.denied) {
    const denied=value.denied as {code?:string;message?:string};
    throw new Error(`${denied.code||'E_STORE'}: ${denied.message||'Snapback refused the operation'}`);
  }
  return answer.ok as T;
}
