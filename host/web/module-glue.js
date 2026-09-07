// A private browser realm per data-module incarnation. @ref LLP 1027 D6;
// LLP 1027.000 D3. Trusted app code, NOT a security sandbox. No page or
// guest builtin is patched. Loaded only after the page's first pixel.
const decoder = new TextDecoder('utf-8', { fatal: true });
const realms = new Map();
const turns = new Map();
let nextTurn = 1;
// A task boundary drains the browser's complete microtask checkpoint without
// a polling timer or pretending a Promise is an HTTP request.
const checkpoint = () => new Promise(resolve => {
  const channel = new MessageChannel();
  channel.port1.onmessage = () => { channel.port1.close(); channel.port2.close(); resolve(); };
  channel.port2.postMessage(null);
});
let nextId = 1, prelude;
const hash = async bytes => {
  // Dev protocol supplies its LAN-capable implementation; static pages need HTTPS.
  if (globalThis.exact.moduleDigest) return globalThis.exact.moduleDigest(bytes);
  if (!crypto.subtle) throw new Error('module integrity requires HTTPS or the dev protocol');
  return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), b => b.toString(16).padStart(2, '0')).join('');
};
async function read(url, limit) {
  const response = await fetch(url, { redirect: 'error' });
  if (!response.ok || new URL(response.url).origin !== location.origin) throw new Error('module fetch failed or left the app origin');
  const reader = response.body.getReader(), chunks = []; let size = 0;
  for (;;) {
    const { done, value } = await reader.read(); if (done) break;
    size += value.length;
    if (size > limit) { await reader.cancel(); throw new Error('module payload too large'); }
    chunks.push(value);
  }
  const bytes = new Uint8Array(size); let at = 0;
  for (const chunk of chunks) { bytes.set(chunk, at); at += chunk.length; }
  return bytes;
}
export async function baked() {
  const receipt = await read(new URL('./app.module.json', import.meta.url), 1024 * 1024);
  const meta = JSON.parse(decoder.decode(receipt));
  return { receipt, script: await read(new URL('./app.js', import.meta.url), Math.min(meta.web?.bytes ?? 0, 32 * 1024 * 1024)) };
}
export async function prepare(payload, admitted, id = nextId++) {
  const meta = JSON.parse(decoder.decode(payload.receipt));
  if (meta.version !== 1 || meta.abi !== 1 || meta.appId !== admitted.appId || meta.grants?.trim() !== admitted.grants.trim()
      || meta.web?.file !== 'app.js' || meta.web.bytes !== payload.script.length || meta.web.sha256 !== await hash(payload.script)
      || !/^[0-9a-f]{64}$/.test(meta.module?.sha256)) throw new Error('module integrity, ABI, identity, or grants mismatch');
  prelude ??= read(new URL('./module-prelude.js', import.meta.url), 256 * 1024).then(bytes => decoder.decode(bytes)).catch(error => { prelude = null; throw error; });
  const before = await prelude;
  const frame = document.createElement('iframe'); frame.hidden = true;
  frame.setAttribute('aria-hidden', 'true'); document.body.append(frame);
  const win = frame.contentWindow;
  let context = null, initializationError = null, busy = false, disposed = false, tail = Promise.resolve();
  win.addEventListener('error', event => { initializationError = event.message; event.preventDefault(); });
  win.__exact_host = (op, name, value) => {
    if (!context) throw new Error('host call outside an answer');
    if (op === 1) { context.requests.set(Number(name), JSON.parse(value)); return; }
    if (op === 2) { context.reads.push(name); return context.store.get(name); }
    if (op === 5) return; // Storage capability check; this host has no provider.
    if (!context.grants.has(name) || name.startsWith('exact.kept.')) return `secret ${name} is not granted`;
    context.writes.push([name, op === 3 ? value : null]);
    if (op === 3) context.store.set(name, value); else context.store.delete(name);
  };
  try {
    // Disable accidental browser I/O before the module captures globals.
    for (const key of ['XMLHttpRequest', 'WebSocket', 'EventSource', 'setTimeout', 'setInterval', 'requestAnimationFrame']) {
      Object.defineProperty(win, key, { value: () => { throw new Error(`${key} is unavailable in data sources`); }, configurable: false });
    }
    for (const source of [before, decoder.decode(payload.script)]) {
      const script = win.document.createElement('script'); script.textContent = source; win.document.head.append(script);
      if (initializationError) throw new Error(initializationError);
    }
    if (win.exact?.abi !== 1 || win.exact.appId !== admitted.appId || win.exact.grants?.trim() !== admitted.grants.trim() || typeof win.exact.answer !== 'function') throw new Error('module exports mismatch the admitted client');
    const pending = new Map();
    const key = r => JSON.stringify([r.source,r.args]);
    const finish = (answer, request) => {
      const result = {...answer, reads:context.reads, writes:context.writes};
      if (answer.tag === 1) {
        result.request = context.requests.get(answer.ticket);
        if (!result.request) throw new Error('module awaits a fetch it never made');
        context.requests.delete(answer.ticket);
        pending.set(key(request), {call:answer.call,ticket:answer.ticket,requests:context.requests});
      }
      context = null; busy = false;
      return result;
    };
    const begin = request => {
      if (disposed) throw new Error('module environment disposed');
      busy = true;
      context = {store:new Map(request.store),grants:new Set(request.grants),reads:[],writes:[],requests:new Map()};
      if (request.op === 'answer') return JSON.parse(win.__exact_call(request.source,JSON.stringify(request.args)));
      const parked = pending.get(key(request));
      if (!parked) throw new Error('reply for an answer not in flight');
      pending.delete(key(request));
      context.requests = parked.requests;
      win.__exact_fulfill(String(parked.ticket),JSON.stringify(request.outcome));
      return {tag:3,call:parked.call};
    };
    const defer = (request, started = null) => {
      const token = nextTurn++;
      turns.set(token, {id, run: () => {
        const run = tail.then(async () => {
          if (disposed) throw new Error('module environment disposed');
          try {
            let answer = started || begin(request);
            if (answer.tag === 3) {
              await checkpoint();
              if (disposed) throw new Error('module environment disposed');
              answer = JSON.parse(win.__exact_settle(String(answer.call)));
            }
            return finish(answer,request);
          } catch (error) { context = null; busy = false; throw error; }
        });
        tail = run.catch(() => {}); return run;
      }});
      return {continuation:token};
    };
    const realm = { frame, meta, id,
      invoke(request) {
        if (busy) return defer(request);
        try {
          const answer = begin(request);
          return answer.tag === 3 ? defer(request,answer) : finish(answer,request);
        } catch (error) { const {reads,writes} = context || {}; context = null; busy = false; return {error:String(error),reads,writes}; }
      },
      dispose() {
        disposed = true; pending.clear(); realms.delete(id); frame.remove();
        for (const [token,turn] of turns) if (turn.id === id) turns.delete(token);
      },
    };
    realms.set(id, realm); return realm;
  } catch (error) { frame.remove(); throw error; }
}
export function call(request) {
  const realm = realms.get(request.id);
  if (!realm) return { error: 'browser module not loaded' };
  if (request.op === 'activate') return realm.meta.appId === request.appId && realm.meta.grants.trim() === request.grants.trim() && realm.meta.module.sha256 === request.revision
    ? { ok: true } : { error: 'browser module admission mismatch' };
  if (request.op === 'answer' || request.op === 'resume') return realm.invoke(request);
  return { error: 'unknown browser module operation' };
}
export function run(token) {
  const turn = turns.get(token); turns.delete(token);
  if (!turn) return Promise.reject(new Error('browser continuation is no longer live'));
  return turn.run();
}
globalThis.exact.moduleRuntime = { prepare, baked, call, run };
