// A private browser realm per data-module incarnation. @ref LLP 1027 D6;
// LLP 1027.000 D3. Trusted app code, NOT a security sandbox. No page or
// guest builtin is patched. Loaded only after the page's first pixel.
import { bindAnswerStorage, createStorage, finishLetGo } from './storage.js';
import { agentSeed, agentStream, keyStore, storageKey } from './storage-environment.js';
import { admitsNetwork, grantError, sameGrantDeclaration, scopedGrantSet } from './grant-admission.js';
const decoder = new TextDecoder('utf-8', { fatal: true });
const hex = bytes => Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
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
// A source's GET leaves when the source asks for it. Its answer still
// settles after the turn's microtask drain, which is a task, and a frame can
// render before that task runs; the runner's `request` op for the same GET
// then claims the response in flight (`claim`) instead of fetching. Only a
// module's own `net.fetch` origins, and nothing with a body. A GET its turn
// didn't report is aborted as the turn ends; one the runner didn't claim while
// the report was delivered is aborted a task later.
const early = new Map(); // `GET url headers` -> [{ response, controller }]
const earlyKey = (url, headers) => `GET ${url} ${JSON.stringify(headers ?? [])}`;
// The runner's normalized set is the only authority this early request reads.
export function fetchEarly(request, grants) {
  try { new URL(request.url); } catch { return null; } // a relative (asset) URL is the host's own
  if (grantError(grants) || request.method !== 'GET' || request.body || !admitsNetwork(grants, request.url, 'fetch')) return null;
  const key = earlyKey(request.url, request.headers), controller = new AbortController();
  const entry = { controller, response: fetch(request.url, { method: 'GET', headers: request.headers, redirect: 'follow', cache: 'default', signal: controller.signal }) };
  entry.response.catch(() => {});
  early.set(key, [...(early.get(key) ?? []), entry]);
  return () => {
    const list = early.get(key), at = list?.indexOf(entry) ?? -1;
    if (at < 0) return;
    list.splice(at, 1); if (!list.length) early.delete(key);
    controller.abort();
  };
}
export function claim(url, init) {
  if (init.method !== 'GET' || init.body || init.redirect !== 'follow' || init.cache !== 'default') return null;
  const key = earlyKey(url, init.headers), list = early.get(key), entry = list?.shift();
  if (!entry) return null;
  if (!list.length) early.delete(key);
  init.signal?.addEventListener('abort', () => entry.controller.abort());
  return entry.response;
}
export async function baked() {
  // These are the exact paired bytes already downloaded in app.wasm. Copy
  // each result before the next export reuses the bridge's output buffer.
  // Admission below still validates the receipt, identity, grants and hash.
  const wasm = globalThis.exact.wasm;
  const artifact = index => {
    const length = wasm.exact_module_artifact(index);
    return new Uint8Array(wasm.memory.buffer, wasm.exact_out(), length).slice();
  };
  return { receipt: artifact(0), script: artifact(1) };
}
export async function prepare(payload, admitted, id = nextId++) {
  const ceiling = new Set(admitted.grants.split('\n').map(s=>s.trim()).filter(Boolean));
  const meta = JSON.parse(decoder.decode(payload.receipt));
  if (meta.version !== 1 || (meta.abi !== 1 && meta.abi !== 2) || meta.appId !== admitted.appId || typeof meta.grants !== 'string' || meta.grants.split('\n').map(s=>s.trim()).filter(Boolean).some(s=>!ceiling.has(s))
      || meta.web?.file !== 'app.js' || meta.web.bytes !== payload.script.length || meta.web.sha256 !== await hash(payload.script)
      || !/^[0-9a-f]{64}$/.test(meta.module?.sha256)) throw new Error('module integrity, ABI, identity, or grants mismatch');
  const childGrantSet = scopedGrantSet(admitted.grantSet, meta.grants);
  if (grantError(childGrantSet)) throw new Error('module integrity, ABI, identity, or grants mismatch');
  admitted = {...admitted,grants:meta.grants,grantSet:childGrantSet};
  prelude ??= read(new URL('./module-prelude.js', import.meta.url), 256 * 1024).then(bytes => decoder.decode(bytes)).catch(error => { prelude = null; throw error; });
  const before = await prelude;
  if (admitted.placement === 'worker') return prepareWorker(payload, admitted, id, before, meta);
  const frame = document.createElement('iframe'); frame.hidden = true;
  frame.setAttribute('aria-hidden', 'true'); document.body.append(frame);
  const win = frame.contentWindow;
  let context = null, initializationError = null, disposed = false, tail = Promise.resolve();
  const seed = agentSeed(), stream = seed === null ? null : agentStream(seed, 'typescript');
  let storage;
  // The module's journal (LLP 1097 D8): the runtime's own lines (a failed
  // storage operation, an unhandled rejection) and its `console`, which the
  // runner writes to `logs` as it does on every host.
  const journal = [];
  win.addEventListener('error', event => { initializationError = event.message; event.preventDefault(); });
  win.addEventListener('unhandledrejection', event => {
    const reason = event.reason;
    journal.push(`data: unhandled rejection: ${reason && typeof reason === 'object' && reason.message !== undefined ? reason.message : String(reason)}`);
  });
  for (const level of win.console ? ['log', 'info', 'warn', 'error', 'debug'] : []) {
    const original = win.console[level].bind(win.console);
    win.console[level] = (...args) => { journal.push(`console: ${args.map(a => typeof a === 'string' ? a : String(a)).join(' ')}`); original(...args); };
  }
  win.__exact_host = (op, name, value) => {
    if (op === 13) { journal.push(String(name)); return '1'; }
    if (!context) throw new Error('host call outside an answer');
    if (op === 6) {
    // A page module answers `native.later` on the page; nothing here can answer at once.
    if (name === 'kind' || name === 'available') return admitted.native ? 'native' : '';
    // A topic the page module announces asks this answer again (LLP 1016.002).
    if (name === 'watch') { context.topics.push(String(value)); return; }
    if (name === 'later') return admitted.native ? 'later' : '';
    throw new Error('the browser answers no native call at once; use native.later');
  }
    if (op === 1) {
      // A stream is the page's to open (LLP 1016.000), never fetched early.
      const request = JSON.parse(value), drop = request.stream ? null : fetchEarly(request, admitted.grantSet);
      context.requests.set(Number(name), request); if (drop) context.early.set(Number(name), drop); return;
    }
    if (op === 2) { context.reads.push(name); return context.store.get(name); }
    if (op === 5) { context.externalRead = true; return; }
    // A draw of secure randomness: a device read the runner counts (LLP 1069.005 D2).
    if (op === 8) { context.entropy = true; return; }
    // Under the agent, the realm's repeatable random bytes (LLP 1069.005 D2b).
    if (op === 11) return stream ? hex(stream(Number(value))) : undefined;
    // `authCallback()`: this page's callback page, a device fact (LLP 1069.006 D2).
    if (op === 12) { if (name !== 'callback') return; context.externalRead = true; return `${location.origin}/.exact/auth/callback`; }
    if (!context.grants.has(name) || name.startsWith('exact.kept.')) return `secret ${name} is not granted`;
    context.writes.push([name, op === 3 ? value : null]);
    if (op === 3) context.store.set(name, value); else context.store.delete(name);
  };
  try {
    storage = createStorage(win, admitted, () => context.owner);
    // A cell belongs to the answer that accepted the call. The adapter runs
    // later, when the operation is issued, which may be a background round.
    bindAnswerStorage(win, storage, () => context.owner);
    // Disable accidental browser I/O before the module captures globals; the
    // prelude refuses timers and the clock, by name, as Hermes does.
    for (const key of ['XMLHttpRequest', 'WebSocket', 'EventSource']) {
      Object.defineProperty(win, key, { value: () => { throw new Error(`${key} is unavailable in data sources`); }, configurable: false });
    }
    // A LAN dev page has no `crypto.subtle`: the realm's SHA-256 digest is
    // the dev protocol's, as module integrity's is (LLP 1069.005 D1).
    if (!win.crypto.subtle && globalThis.exact.moduleDigest) win.__exact_digest = bytes => globalThis.exact.moduleDigest(bytes);
    // Kept keys (LLP 1069.005 D1b): the CryptoKeyPair in this realm's IndexedDB.
    win.__exact_keys = keyStore(storageKey(admitted.appId), win.indexedDB);
    for (const source of [before, decoder.decode(payload.script)]) {
      const script = win.document.createElement('script'); script.textContent = source; win.document.head.append(script);
      if (initializationError) throw new Error(initializationError);
      if (source === before) {
        win.__exact_storage = storage.capability;
        win.__exact_install_storage();
        // The page's own realm: storage an answer did not await finishes
        // after it, as the background's (LLP 1097 D6, D7). A worker realm
        // (module-worker.js) never sets it.
        win.__exact_main_thread?.();
      }
    }
    if ((win.exact?.abi !== 1 && win.exact?.abi !== 2) || win.exact.appId !== admitted.appId || !sameGrantDeclaration(childGrantSet, win.exact.grants) || typeof win.exact.answer !== 'function') throw new Error('module exports mismatch the admitted client');
    const pending = new Map(), streams = new Map();
    // The runner's target first: two targets asking one source with equal
    // arguments are two calls (LLP 1027 D1a).
    const key = r => JSON.stringify([r.target ?? null,r.source,r.args]);
    // The background (LLP 1097 D7): the owner its storage completions
    // land on, and the answers parked until a background delivery may
    // settle what they await.
    const backgroundOwner = {};
    const backgroundContext = () => ({owner:backgroundOwner,store:new Map(),grants:new Set(),reads:[],writes:[],externalRead:false,entropy:false,topics:[],requests:new Map(),early:new Map()});
    // Calls the runner let go between storage steps. Their owners stay until
    // the chain has been delivered, as `finish_let_go` does natively.
    const owed = new Map();
    const scratch = owner => ({owner,store:new Map(),grants:new Set(),reads:[],writes:[],externalRead:false,entropy:false,topics:[],requests:new Map(),early:new Map()});
    const letGoHooks = {
      letGo: () => win.__exact_let_go('', ''),
      disposed: () => disposed,
      deliver: async owner => {
        const prev = context;
        context = scratch(owner);
        try { await storage.deliver(owner); await checkpoint(); }
        finally { context = prev; }
      },
    };
    const release = (owner, callId) => {
      if (win.__exact_forget(String(callId)) !== 'storage') { storage.retire(owner); return; }
      owed.set(callId, owner);
      const run = tail.then(() => finishLetGo(storage, owed, letGoHooks));
      tail = run.catch(() => {});
    };
    let parked = [];
    const finish = (answer, request) => {
      const result = {...answer, reads:context.reads, writes:context.writes, externalRead:context.externalRead,entropy:context.entropy,topics:context.topics};
      const reported = answer.tag === 1 ? context.early.get(answer.ticket) : null;
      for (const drop of context.early.values()) if (drop !== reported) drop();
      if (reported) setTimeout(reported, 0);
      if (answer.tag === 1) {
        result.request = context.requests.get(answer.ticket);
        if (!result.request) throw new Error('module awaits a fetch it never made');
        context.requests.delete(answer.ticket);
        // A stream's call is mapped per message, never resumed (LLP 1016.000).
        if (result.request.stream) streams.set(key(request), {call:answer.call,owner:context.owner});
        else pending.set(key(request), {call:answer.call,ticket:answer.ticket,requests:context.requests,owner:context.owner});
      }
      if (answer.tag !== 1) { storage.rehome(context.owner, backgroundOwner); storage.retire(context.owner); }
      context = null;
      return result;
    };
    // An answer between its storage steps keeps its store context and the
    // realm's turn until its value is ready. One waiting with the module (its
    // operation queued behind the background's, or a promise the background
    // will settle) parks and lets the turn go; a background delivery asks it
    // again (LLP 1097 D7).
    const proceed = async (answer, request, done) => {
      for (;;) {
        if (answer.tag === 1 && answer.ticket === 0 && !answer.waiting) {
          await storage.deliver(context.owner);
          await checkpoint();
          if (disposed) throw new Error('module environment disposed');
          answer = JSON.parse(win.__exact_settle(String(answer.call)));
          continue;
        }
        if (answer.tag === 1 && answer.waiting) {
          parked.push({answer, request, context, done});
          context = null;
          return;
        }
        done.resolve(finish(answer, request));
        return;
      }
    };
    // Ask parked answers again, one at a time, in the turn that delivered.
    const resettle = async () => {
      const waiting = parked; parked = [];
      for (const p of waiting) {
        context = p.context;
        try { await proceed(JSON.parse(win.__exact_settle(String(p.answer.call))), p.request, p.done); }
        catch (error) { for (const drop of context?.early.values() ?? []) drop(); storage.retire(context?.owner); context = null; p.done.reject(error); }
      }
    };
    // One background round, as the runner's background ticket runs it: wait
    // outside the realm's turns for the background's next completion, so no
    // answer waits for background work to begin, then take the turn to
    // deliver it with the background current, and say what is left.
    const backgroundRound = async () => {
      await storage.ready(backgroundOwner);
      const run = tail.then(async () => {
        if (disposed) throw new Error('module environment disposed');
        context = backgroundContext();
        win.__exact_enter_background();
        const delivered = storage.deliverNow(backgroundOwner);
        await checkpoint();
        if (disposed) throw new Error('module environment disposed');
        // A let-go chain this delivery just issued is finished before the
        // round returns, while storage is not refused.
        await finishLetGo(storage, owed, letGoHooks);
        if (disposed) throw new Error('module environment disposed');
        context = null;
        await resettle();
        return {delivered, ...JSON.parse(win.__exact_background())};
      });
      tail = run.catch(() => {}); return run;
    };
    const begin = request => {
      if (disposed) throw new Error('module environment disposed');
      context = {owner:{},store:new Map(request.store),grants:new Set(request.grants),reads:[],writes:[],externalRead:false,entropy:false,topics:[],requests:new Map(),early:new Map()};
      if (request.op === 'answer') return JSON.parse(win.__exact_call(request.source,JSON.stringify(request.args)));
      const parked = pending.get(key(request));
      if (!parked) throw new Error('reply for an answer not in flight');
      pending.delete(key(request));
      context.owner = parked.owner;
      context.requests = parked.requests;
      win.__exact_fulfill(String(parked.ticket),JSON.stringify(request.outcome));
      return {tag:3,call:parked.call};
    };
    const defer = request => {
      const token = nextTurn++;
      // `request.store` is replaced at dispatch, on the runner's thread,
      // with the store as committed then (LLP 1027.002 D3, change 1).
      turns.set(token, {id, request, run: () => {
        const run = tail.then(async () => {
          if (disposed) throw new Error('module environment disposed');
          try {
            let answer = begin(request);
            if (answer.tag === 3) {
              await checkpoint();
              if (disposed) throw new Error('module environment disposed');
              answer = JSON.parse(win.__exact_settle(String(answer.call)));
            }
            // Keep this answer's store context while its own storage is
            // pending; other answers queue behind it; a fetch releases the
            // turn normally, and so does an answer that parks.
            const done = {}; done.promise = new Promise((resolve, reject) => { done.resolve = resolve; done.reject = reject; });
            await proceed(answer, request, done);
            return done;
          } catch (error) { for (const drop of context?.early.values() ?? []) drop(); storage.retire(context?.owner); context = null; throw error; }
        });
        tail = run.catch(() => {});
        return run.then(done => done.promise);
      }});
      return {continuation:token};
    };
    const realm = { frame, meta, grantSet: childGrantSet, id, placement: 'main',
      // The background's state, for the runner's poll (LLP 1097 D5, D8).
      background: () => JSON.parse(win.__exact_background()),
      // A background round: a turn the host runs under the runner's ticket.
      backgroundRound() {
        const token = nextTurn++;
        turns.set(token, {id, request: {target: null}, run: backgroundRound});
        return {token};
      },
      journal: () => journal.splice(0),
      // Canvas 2D (LLP 1056 D1): a draw awaits nothing, so it runs now.
      // Text is measured and images answered on the page (LLP 1056 D8, D9).
      draw: request => { const h = globalThis.exact?.canvas2dHost; return JSON.parse(win.__exact_draw(request, h?.measure, h?.image)); },
      retire: retired => win.__exact_retire(retired),
      invoke(request) {
        // A context is installed only inside the queue that will finish it.
        // The host may run continuation tokens in a different order from calls.
        return defer(request);
      },
      // One message of a stream, or its end: the source's `exactStream`
      // maps it now, in this call — a mapper never awaits (LLP 1016.000 D1).
      message(request) {
        if (disposed) return { error: 'module environment disposed' };
        const k = key(request), open = streams.get(k);
        if (!open) return { error: 'a message for a stream not open' };
        context = {owner:{},store:new Map(request.store),grants:new Set(request.grants),reads:[],writes:[],externalRead:false,entropy:false,topics:[],requests:new Map(),early:new Map()};
        try {
          const answer = JSON.parse(win.__exact_message(String(open.call), JSON.stringify(request.outcome)));
          if (!request.outcome.message) { streams.delete(k); release(open.owner, open.call); }
          return finish(answer, request);
        } catch (error) { context = null; return { error: String(error?.message ?? error) }; }
      },
      // The runner let go of every targeted call not in flight (LLP 1016 D5):
      // drop it, its storage owner, and its call in the prelude.
      forget(inFlight) {
        const keep = new Set(inFlight.map(key));
        for (const [parkedKey, parked] of pending) {
          if (JSON.parse(parkedKey)[0] === null || keep.has(parkedKey)) continue;
          pending.delete(parkedKey); release(parked.owner, parked.call);
        }
        for (const [streamKey, open] of streams) {
          if (JSON.parse(streamKey)[0] === null || keep.has(streamKey)) continue;
          streams.delete(streamKey); release(open.owner, open.call);
        }
        parked = parked.filter(p => {
          if ((p.request.target ?? null) === null || keep.has(key(p.request))) return true;
          release(p.context.owner, p.answer.call);
          p.done.reject(new Error('the runner let this answer go'));
          return false;
        });
        forgetTurns(id, keep, key);
      },
      dispose() {
        disposed = true; storage.dispose(); pending.clear(); realms.delete(id); frame.remove();
        for (const [token,turn] of turns) if (turn.id === id) turns.delete(token);
      },
    };
    realms.set(id, realm); return realm;
  } catch (error) { storage?.dispose(); frame.remove(); throw error; }
}
// The module's realm on a dedicated Worker (LLP 1027.002 D2): the prelude,
// storage capability and turn discipline of the iframe realm, off the
// page's main thread (module-worker.js). Same interface as the iframe realm.
async function prepareWorker(payload, admitted, id, before, meta) {
  const worker = new Worker(new URL('./module-worker.js', import.meta.url), { type: 'module' });
  const waiting = new Map();
  let disposed = false;
  const fail = message => { for (const w of waiting.values()) w.reject(new Error(message)); waiting.clear(); };
  worker.onmessage = ({ data }) => {
    // The worker's SHA-256 on a LAN dev page, which has no `crypto.subtle`
    // (LLP 1069.005 D1): the dev protocol's, here on the page.
    if (data.op === 'digest') {
      Promise.resolve().then(() => globalThis.exact.moduleDigest(data.bytes)).then(hex => worker.postMessage({ op: 'digest', id: data.id, hex }),
        error => worker.postMessage({ op: 'digest', id: data.id, error: String(error?.message ?? error) }));
      return;
    }
    const w = waiting.get(data.token); if (!w) return;
    waiting.delete(data.token);
    if (data.error !== undefined) w.reject(new Error(data.error)); else w.resolve(data.result);
  };
  worker.onerror = event => { event.preventDefault(); fail(`module worker failed: ${event.message}`); };
  worker.onmessageerror = () => fail('module worker message failed');
  const ready = new Promise((resolve, reject) => waiting.set(0, { resolve, reject }));
  worker.postMessage({ op: 'init', token: 0, prelude: before, script: decoder.decode(payload.script), admitted,
    storage: storageKey(admitted.appId), pageDigest: !!globalThis.exact.moduleDigest, seed: agentSeed() });
  try { await ready; } catch (error) { worker.terminate(); throw error; }
  const realm = { frame: null, meta, grantSet: admitted.grantSet, id, placement: 'worker',
    forget(inFlight) {
      forgetTurns(id, new Set(inFlight.map(workerKey)), workerKey);
      worker.postMessage({ op: 'forget', inFlight });
    },
    invoke(request) {
      const token = nextTurn++;
      turns.set(token, {id, request, run: () => new Promise((resolve, reject) => {
        if (disposed) return reject(new Error('module environment disposed'));
        waiting.set(token, { resolve, reject });
        worker.postMessage({ op: 'turn', token, request });
      })});
      return {continuation: token};
    },
    dispose() {
      disposed = true; fail('module environment disposed'); worker.terminate(); realms.delete(id);
      for (const [token, turn] of turns) if (turn.id === id) turns.delete(token);
    },
  };
  realms.set(id, realm); return realm;
}
// The runner's target first, as each realm keys its parked calls.
const workerKey = r => JSON.stringify([r.target ?? null, r.source, r.args]);
// Turns not yet run for a request the runner let go: running one would only
// produce a reply the runner drops.
function forgetTurns(id, keep, keyOf) {
  for (const [token, turn] of turns) {
    if (turn.id !== id || (turn.request.target ?? null) === null) continue;
    if (!keep.has(keyOf(turn.request))) turns.delete(token);
  }
}
export function call(request) {
  const realm = realms.get(request.id);
  if (!realm) return { error: 'browser module not loaded' };
  if (request.op === 'activate') return realm.meta.appId === request.appId && sameGrantDeclaration(realm.grantSet, request.grants) && realm.meta.module.sha256 === request.revision && realm.placement === (request.placement ?? 'main')
    ? { ok: true } : { error: `browser module admission mismatch: the page's module is ${realm.meta.module.sha256.slice(0, 12)} (${realm.meta.appId}, ${realm.placement}); the wasm admits ${String(request.revision).slice(0, 12)} (${request.appId}, ${request.placement ?? 'main'}) — rebuild the wasm (r + Enter in the dev loop)` };
  if (request.op === 'dispatch') {
    const turn = turns.get(request.token);
    if (!turn || turn.id !== request.id) return { error: 'browser continuation is no longer live' };
    turn.request.store = request.store; turn.request.grants = request.grants;
    return { ok: true };
  }
  if (request.op === 'discard') {
    const turn = turns.get(request.token);
    if (turn && turn.id === request.id) turns.delete(request.token);
    return { ok: true };
  }
  if (request.op === 'forget') { realm.forget(request.inFlight ?? []); return { ok: true }; }
  // Background work (LLP 1097 D5, D8): the page's realm has it; a worker's
  // answers wait for their storage and it has none.
  if (request.op === 'background') return realm.background ? realm.background() : { head: false };
  if (request.op === 'background-round') return realm.backgroundRound ? realm.backgroundRound() : { error: 'no background work in a worker-placed module' };
  if (request.op === 'journal') return { lines: realm.journal ? realm.journal() : [] };
  if (request.op === 'draw') return realm.draw ? realm.draw(request.request) : { error: 'a worker-placed module does not draw Canvas 2D yet' };
  if (request.op === 'retire') { realm.retire?.(request.retired); return { ok: true }; }
  if (request.op === 'answer' || request.op === 'resume') return realm.invoke(request);
  // A worker realm answers only through turns; a stream's mapper runs now.
  if (request.op === 'message') return realm.message ? realm.message(request) : { error: 'a worker-placed module does not stream yet (LLP 1069.004)' };
  return { error: 'unknown browser module operation' };
}
export function run(token) {
  const turn = turns.get(token); turns.delete(token);
  if (!turn) return Promise.reject(new Error('browser continuation is no longer live'));
  return turn.run();
}
globalThis.exact.moduleRuntime = { prepare, baked, call, run, claim };
