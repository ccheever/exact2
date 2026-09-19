import type { Storage } from './app.contract.d.ts';
import { initSync, WebDevice } from './snapback/generated/device';
import { result, type Backend, type Core, type Request } from './snapback-types';

let initialized=false;
export async function loadBrowserDevice():Promise<void> {
  if(initialized)return;
  // Each concurrent answer owns its own fetch ticket. Sharing a pending
  // promise would leave the other answers waiting without a host operation.
  const response=await fetch('/assets/snapback4-device.wasm');
  if(!response.ok)throw new Error(`Snapback4 device HTTP ${response.status}`);
  initSync({module:new Uint8Array(await response.arrayBuffer())});
  initialized=true;
}

// The same Rust device as native, loaded only when post-pixel storage opens.
// SQLite commits each drained device transaction atomically. A failed commit
// discards the speculative device and reconstructs it from the durable image.
export async function browserCore(storage:Storage,path:string,backend:Backend):Promise<Core> {
  await loadBrowserDevice();
  const db=await storage.sqlite.open(path);
  try {
    await db.execute('CREATE TABLE IF NOT EXISTS snapback_device (s TEXT NOT NULL,k TEXT NOT NULL,v TEXT NOT NULL,PRIMARY KEY(s,k)) WITHOUT ROWID');
    const kept=async()=>JSON.stringify((await db.query('SELECT s,k,v FROM snapback_device')).rows.map(([s,k,v])=>({s,k,v})));
    const saved=await kept();
    let device=new WebDevice(saved==='[]'?JSON.stringify(backend):null,saved);
    const commit=async()=>{
      const changes=JSON.parse(device.drain()) as {s:string;k:string;v:string|null}[];
      if(changes.length)await db.transaction(changes.map(({s,k,v})=>v===null
        ?{sql:'DELETE FROM snapback_device WHERE s=? AND k=?',params:[s,k]}
        :{sql:'INSERT OR REPLACE INTO snapback_device VALUES (?,?,?)',params:[s,k,v]}));
    };
    await commit();
    if(!result<string|null>(JSON.parse(device.call(JSON.stringify({op:'meta',key:'exact:device'}))))) {
      const identity=(await db.query('SELECT lower(hex(randomblob(16))) AS id')).rows[0][0];
      result(JSON.parse(device.call(JSON.stringify({op:'set_meta',key:'exact:device',value:identity}))));
      await commit();
    }
    return {async call(request:Request) {
      try {
        const answer=JSON.parse(device.call(JSON.stringify(request))) as Request;
        await commit();
        return answer;
      }catch(error){
        device.free();device=new WebDevice(null,await kept());device.drain();
        throw error;
      }
    }};
  }catch(error){await db.close();throw error;}
}
