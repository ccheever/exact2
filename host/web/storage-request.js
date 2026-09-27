// Portable Rust storage operations use the same filesystem and SQLite services
// as TS. Handles never cross this boundary. @ref LLP 1027.001 D2.
import { createFileSystem } from './storage-fs.js';
import { createSqlite } from './storage-sqlite.js';
import { agentStorageRefusal, storageKey } from './storage-environment.js';
const maxBytes = 16 << 20;
const encoder = new TextEncoder();
const bytes = args => {
  if (typeof args.text === 'string') return encoder.encode(args.text);
  if (!Array.isArray(args.bytes) || args.bytes.some(n => !Number.isInteger(n) || n < 0 || n > 255)) throw new Error('storage: invalid bytes');
  return Uint8Array.from(args.bytes);
};
const sqlValue = value => {
  if (value === null || typeof value === 'string' || typeof value === 'number' && Number.isFinite(value)) return value;
  if (value && typeof value === 'object' && Object.keys(value).length === 1) {
    if (typeof value.integer === 'string' && /^-?(0|[1-9][0-9]*)$/.test(value.integer)) {
      const n = BigInt(value.integer);
      if (n >= -(1n << 63n) && n < (1n << 63n)) return n;
    }
    if ('bytes' in value) return bytes(value);
  }
  throw new Error('invalid SQLite parameter');
};
const wire = value => {
  if (typeof value === 'bigint') return {integer:String(value)};
  if (value instanceof ArrayBuffer || ArrayBuffer.isView(value)) return {bytes:Array.from(new Uint8Array(value.buffer || value, value.byteOffset || 0, value.byteLength))};
  return value;
};
const result = r => ({changes:String(r.changes),lastInsertRowid:String(r.lastInsertRowid)});
function base64(bytes) { let text='';for (let i=0;i<bytes.length;i+=16384)text+=String.fromCharCode(...bytes.subarray(i,i+16384));return btoa(text); }
export function createStorageRequests(appId, admitted) {
  const services = new Map(), key = storageKey(appId, location.href); let disposed = false;
  const check = () => { if(disposed)throw new Error('storage source unloaded'); };
  const lines = admitted.split('\n').map(s=>s.trim()).filter(Boolean);
  return {
    async run(payload, scope = null) {
      try {
        check();
        if (key == null) throw new Error(agentStorageRefusal);
        if (encoder.encode(payload).length > maxBytes) throw new Error('storage request exceeds its byte limit');
        const request=JSON.parse(payload), {op,args}=request;
        if (request.version!==1 || !args || typeof args.path!=='string' || !args.path.startsWith('app:/')) throw new Error('invalid portable storage request');
        scope ??= admitted;
        if (typeof scope!=='string' || scope.split('\n').map(s=>s.trim()).filter(Boolean).some(s=>!lines.includes(s))) throw new Error("source scope exceeds the app's admitted grants");
        if (!services.has(scope)) services.set(scope,{fs:createFileSystem(key,scope),sqlite:createSqlite(key,scope)});
        const {fs,sqlite}=services.get(scope); let value;
        if (op==='sqlite' || op==='sqlite.transaction') {
          if (!Array.isArray(args.commands) || args.commands.length>10000) throw new Error('invalid SQLite commands');
          const commands=args.commands.map(c=>{
            if (!c || !['execute','query'].includes(c.kind) || typeof c.sql!=='string' || !Array.isArray(c.params)) throw new Error('invalid SQLite command');
            return {...c,params:c.params.map(sqlValue)};
          });
          if(op==='sqlite.transaction' && commands.some(c=>c.kind!=='execute'))throw new Error('SQLite write transaction requires execute commands');
          const db=await sqlite.open(args.path);
          try {
            check();
            if(op==='sqlite.transaction') value=(await db.transaction(commands.map(({sql,params})=>({sql,params})))).map(result);
            else {
              value=[];
              for(const c of commands){check();if(c.kind==='query'){
                const r=await db.query(c.sql,c.params);value.push({columns:r.columns,rows:r.rows.map(row=>row.map(wire))});
              }else value.push(result(await db.execute(c.sql,c.params)));}
            }
          } finally { await db.close(); }
        } else {
          check();
          switch(op) {
            case 'fs.readFile':value={base64:base64(new Uint8Array(await fs.readFile(args.path)))};break;
            case 'fs.writeFile':case 'fs.atomicWriteFile':case 'fs.appendFile':value=await fs[op.slice(3)](args.path,bytes(args));break;
            case 'fs.rename':case 'fs.copyFile':
              if(typeof args.destination!=='string'||!args.destination.startsWith('app:/'))throw new Error('portable storage needs an app:/ destination');
              value=await fs[op.slice(3)](args.path,args.destination);break;
            case 'fs.mkdir':case 'fs.rm':case 'fs.readdir':case 'fs.realpath':case 'fs.stat':value=await fs[op.slice(3)](args.path);break;
            default:throw new Error('unsupported storage operation '+op);
          }
        }
        check();
        const encoded=encoder.encode(JSON.stringify(value??null));
        if(encoded.length>maxBytes)throw new Error('storage result exceeds its byte limit');
        return encoded;
      } catch(error) { return encoder.encode(JSON.stringify({error:String(error?.message??error)})); }
    },
    dispose() { disposed=true;for(const service of services.values()){service.fs.dispose();service.sqlite.dispose();}services.clear(); },
  };
}

if(globalThis.exact)globalThis.exact.createStorageRequests=createStorageRequests;
