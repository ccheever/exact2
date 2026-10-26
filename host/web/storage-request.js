// Portable Rust storage operations use the same filesystem and SQLite services
// as TS. Handles never cross this boundary. @ref LLP 1027.001 D2.
import { bodyTarget, createFileSystem, requestBody } from './storage-fs.js';
import { createSqlite } from './storage-sqlite.js';
import { agentStorageRefusal, storageKey } from './storage-environment.js';
import { grantError } from './grant-admission.js';
import './documents-glue.js';
const maxBytes = 16 << 20;
let toldAgent = false;
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
// `compressImage`'s edit from a request's record (LLP 1133.019 D15); a null field is an absent one.
const edit = args => Object.fromEntries(['rotate','flip','crop'].filter(k=>args[k]!=null).map(k=>[k,args[k]]));
function base64(bytes) { let text='';for (let i=0;i<bytes.length;i+=16384)text+=String.fromCharCode(...bytes.subarray(i,i+16384));return btoa(text); }
export function createStorageRequests(appId, admitted) {
  const services = new Map(), key = storageKey(appId); let disposed = false;
  // Each database's requests take turns. The worker opens a database under
  // its file's exclusive lock, in one queue per page: a second open waiting
  // for the lock there holds back the first's queries and close, and both
  // stall until the lock's 10 s bound refuses the second.
  const turns = new Map();
  const inTurn = async (path, work) => {
    const before = turns.get(path) ?? Promise.resolve();
    let done;
    const mine = before.then(() => new Promise(resolve => { done = resolve; }));
    turns.set(path, mine);
    await before;
    try { return await work(); }
    finally { done(); if (turns.get(path) === mine) turns.delete(path); }
  };
  const check = () => { if(disposed)throw new Error('storage source unloaded'); };
  return {
    async run(payload, scope = null) {
      try {
        check();
        if (encoder.encode(payload).length > maxBytes) throw new Error('storage request exceeds its byte limit');
        const request=JSON.parse(payload), {op,args}=request;
        scope ??= admitted;
        const invalid=grantError(scope);if(invalid)throw new Error(invalid);
        // Two app files, never a document (LLP 1069.002 A1.1): refused before the document branch.
        if ((op==='fs.compressImage'||op==='fs.transformImage') && ![args?.path,args?.destination].every(p=>typeof p==='string'&&p.startsWith('app:/'))) throw new Error(op.slice(3)+': needs app:/ paths');
        // A document the person chose is not app storage (LLP 1069.010 D1).
        // A document copied into an app file needs app storage too (documents-glue.js `copyFile`).
        const copiesIn=op==='fs.copyFile'&&typeof args?.path==='string'&&args.path.startsWith('doc:/')&&typeof args.destination==='string'&&args.destination.startsWith('app:/');
        if (request.version===1 && typeof args?.path==='string' && args.path.startsWith('doc:/') && !copiesIn) {
          const data=['fs.writeFile','fs.atomicWriteFile','fs.appendFile'].includes(op)?bytes(args):null;
          const encoded=encoder.encode(JSON.stringify(await globalThis.exact.documents.run(op,args,data,scope)??null));
          if(encoded.length>maxBytes)throw new Error('storage result exceeds its byte limit');
          return encoded;
        }
        if (key == null) {
          // The first refused read or write is said once, as on every host (trivia F7): the page's console reaches the driver's logs.
          if (!toldAgent) { toldAgent = true; console.warn(`storage refused (agent): ${agentStorageRefusal}`); }
          throw new Error(agentStorageRefusal);
        }
        if (request.version!==1 || !args || typeof args.path!=='string' || !(args.path.startsWith('app:/') || copiesIn)) throw new Error('invalid portable storage request');
        const scopeKey=scope.seal;
        if (!services.has(scopeKey)) services.set(scopeKey,{fs:createFileSystem(key,scope),sqlite:createSqlite(key,scope)});
        const {fs,sqlite}=services.get(scopeKey); let value;
        if (op==='sqlite' || op==='sqlite.transaction') {
          if (!Array.isArray(args.commands) || args.commands.length>10000) throw new Error('invalid SQLite commands');
          const commands=args.commands.map(c=>{
            if (!c || !['execute','query'].includes(c.kind) || typeof c.sql!=='string' || !Array.isArray(c.params)) throw new Error('invalid SQLite command');
            return {...c,params:c.params.map(sqlValue)};
          });
          if(op==='sqlite.transaction' && commands.some(c=>c.kind!=='execute'))throw new Error('SQLite write transaction requires execute commands');
          value=await inTurn(args.path, async () => {
            const db=await sqlite.open(args.path);
            try {
              check();
              if(op==='sqlite.transaction') return (await db.transaction(commands.map(({sql,params})=>({sql,params})))).map(result);
              const out=[];
              for(const c of commands){check();if(c.kind==='query'){
                const r=await db.query(c.sql,c.params);out.push({columns:r.columns,rows:r.rows.map(row=>row.map(wire))});
              }else out.push(result(await db.execute(c.sql,c.params)));}
              return out;
            } finally { await db.close(); }
          });
        } else {
          check();
          switch(op) {
            case 'fs.readFile':value={base64:base64(new Uint8Array(await fs.readFile(args.path)))};break;
            case 'fs.writeFile':case 'fs.atomicWriteFile':case 'fs.appendFile':value=await fs[op.slice(3)](args.path,bytes(args));break;
            case 'fs.copyFile':
              if(copiesIn){value=await globalThis.exact.documents.copyFile(args.path,args.destination,scope,fs);break;}
            // falls through
            case 'fs.rename':
              if(typeof args.destination!=='string'||!args.destination.startsWith('app:/'))throw new Error('portable storage needs an app:/ destination');
              value=await fs[op.slice(3)](args.path,args.destination);break;
            case 'fs.compressImage':
              value=await fs.compressImage(args.path,args.destination,{...edit(args),maxDimension:args.maxDimension,maxBytes:args.maxBytes,...(args.type==null?{}:{type:args.type})});break;
            case 'fs.transformImage': {
              // The record is D14's options beside the two paths (LLP 1133.019 D17).
              const {path,destination,...options}=args;
              value=await fs.transformImage(path,destination,options);break;
            }
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

if(globalThis.exact){globalThis.exact.createStorageRequests=createStorageRequests;globalThis.exact.requestBody=requestBody;globalThis.exact.bodyTarget=bodyTarget;}
