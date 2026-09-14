// Rust business-logic modules run in the browser's Wasm engine, after first
// pixel. Each Rust adapter owns one private instance. LLP 1029.000 §3.
export function rustRuntime(memory) {
  const instances = new Map();
  let next = 1;
  const limit = 32 * 1024 * 1024;
  const bytes = (ptr, len) => {
    if (len > limit) throw new Error('Rust module buffer exceeds 32 MiB');
    return new Uint8Array(memory().buffer, ptr >>> 0, len >>> 0);
  };
  return {
    load(ptr, len) {
      try {
        const compiled = new WebAssembly.Module(bytes(ptr, len));
        if (WebAssembly.Module.imports(compiled).length) throw new Error('Rust module must be importless');
        const exports = new WebAssembly.Instance(compiled, {}).exports;
        if (!(exports.memory instanceof WebAssembly.Memory) || exports.exact_logic_abi?.() !== 2) throw new Error('Rust module ABI mismatch');
        for (const name of ['create', 'destroy', 'alloc', 'dealloc', 'call', 'output', 'output_len']) {
          if (typeof exports['exact_logic_' + name] !== 'function') throw new Error('Rust module export missing: ' + name);
        }
        const session = exports.exact_logic_create();
        if (!session) throw new Error('Rust module could not create a session');
        const id = next++;
        instances.set(id, { exports, session, output: new Uint8Array() });
        return id;
      } catch (error) { console.error('Rust module:', error); return 0; }
    },
    call(id, ptr, len) {
      const state = instances.get(id);
      if (!state) return 0xffffffff;
      const e = state.exports;
      let input = 0;
      try {
        const copied = bytes(ptr, len).slice();
        input = e.exact_logic_alloc(len);
        if (!input && len) throw new Error('Rust module allocation failed');
        new Uint8Array(e.memory.buffer, input >>> 0, len).set(copied);
        if (e.exact_logic_call(state.session, input, len) !== 0) throw new Error('Rust module rejected the call');
        const count = e.exact_logic_output_len(state.session) >>> 0;
        if (count > limit) throw new Error('Rust module reply exceeds 32 MiB');
        state.output = new Uint8Array(e.memory.buffer, e.exact_logic_output(state.session) >>> 0, count).slice();
        return count;
      } catch (error) { state.output = new Uint8Array(); console.error('Rust module:', error); return 0xffffffff; }
      finally {
        if (input) {
          try { e.exact_logic_dealloc(input, len); }
          catch(error) { state.output=new Uint8Array();console.error('Rust module deallocation:',error);return 0xffffffff; }
        }
      }
    },
    read(id, ptr, len) {
      const state = instances.get(id);
      if (!state || len !== state.output.length) return 1;
      try { bytes(ptr, len).set(state.output); return 0; } catch { return 1; }
    },
    drop(id) {
      const state = instances.get(id);
      if (!state) return;
      instances.delete(id);
      try { state.exports.exact_logic_destroy(state.session); }
      catch(error) { console.error('Rust module destruction:',error); }
    },
  };
}
if (globalThis.exact) globalThis.exact.createRustRuntime = rustRuntime;

// A normal browser tab follows its canonical app URL after first pixel. The
// immutable release serving this script supplies the initial module identity.
// Development uses its ordered generation stream instead of this poller.
export async function followRustUpdates(exact, sourceURL) {
  const digest = async bytes => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), n=>n.toString(16).padStart(2,'0')).join('');
  const origin = new URL(sourceURL).origin;
  const read = async (url, limit) => {
    url = new URL(url);
    if (url.origin !== origin) throw new Error('Rust update leaves the app origin');
    const response = await fetch(url, {cache:'no-store',signal:AbortSignal.timeout(15000)});
    if (!response.ok || new URL(response.url).origin !== origin) throw new Error('Rust update fetch refused');
    const reader=response.body.getReader(), chunks=[]; let count=0;
    try {
      for (;;) { const {value,done}=await reader.read();if(done)break;count+=value.length;if(count>limit)throw new Error('Rust update exceeds payload bound');chunks.push(value); }
    } catch(error) { await reader.cancel(); throw error; }
    const bytes=new Uint8Array(count);let at=0;for(const chunk of chunks){bytes.set(chunk,at);at+=chunk.length;}return bytes;
  };
  const envelope = async url => JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(await read(url,64*1024)));
  const initial=await envelope(new URL('./exact.json', sourceURL));
  if (initial.dev || !initial.rust) return;
  const identity = value => [value.plan?.sha256,value.rust?.wasm.receipt.sha256,value.module?.receipt.sha256].join(':');
  let accepted=identity(initial);
  const canonical=new URL('./exact.json', location.href);
  const load = async (card, base) => {
    if (!Number.isSafeInteger(card?.bytes)||card.bytes<0||card.bytes>32*1024*1024||!/^[0-9a-f]{64}$/.test(card.sha256))throw new Error('Invalid Rust update card');
    const url=new URL(card.url,base),bytes=await read(url,card.bytes);
    if(bytes.length!==card.bytes||await digest(bytes)!==card.sha256)throw new Error('Rust update digest mismatch');
    return {bytes,sha256:card.sha256,url:url.href};
  };
  const poll = async () => {
    try {
      const next=await envelope(canonical);
      if(next.dev)return;
      if(next.app?.id!==exact.compat.inputs.app)throw new Error('Rust update names another app');
      const pair=next.rust?.wasm;
      if(pair && identity(next)!==accepted) {
        const javascript = next.module;
        const cards=[next.plan,pair.receipt,pair.module,...(javascript?[javascript.receipt,javascript.web]:[]),...(next.assets??[])];
        if(cards.reduce((n,c)=>n+c.bytes,0)>256*1024*1024)throw new Error('Rust update exceeds generation bound');
        const [plan,receipt,module]=await Promise.all(cards.slice(0,3).map(c=>load(c,canonical)));
        const js=javascript?await Promise.all([javascript.receipt,javascript.web].map(c=>load(c,canonical))):null;
        const assets=new Map();
        for(const card of next.assets??[]) {
          if(!/^(assets|deck|shaders)\//.test(card.name)||/[\\\x00-\x1f]/.test(card.name)||card.name.split('/').some(p=>!p||p==='.'||p==='..')||assets.has(card.name))throw new Error('Invalid Rust update asset name');
          assets.set(card.name,await load(card,canonical));
        }
        if(await exact.reloadGeneration(plan.bytes,assets,()=>true,js?{receipt:js[0].bytes,script:js[1].bytes}:null,{receipt:receipt.bytes,module:module.bytes}))accepted=identity(next);
      }
    } catch(error) { console.error('Rust update retained the active app:',error); }
    setTimeout(poll,5000);
  };
  setTimeout(poll,5000);
}
if (globalThis.exact) globalThis.exact.followRustUpdates = followRustUpdates;
