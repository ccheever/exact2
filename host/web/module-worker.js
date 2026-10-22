// The module's private realm on a dedicated Worker (LLP 1027.002 D2): the
// same prelude, storage capability and turn discipline as the iframe realm
// in module-glue.js, off the page's main thread. Trusted app code, NOT a
// security sandbox. Values cross as messages; the page's runner commits.
// Loaded only after the page's first pixel, by module-glue.js.
import { bindAnswerStorage, createStorage, finishLetGo } from './storage.js';
import { agentStream, keyStore } from './storage-environment.js';
import { sameGrantDeclaration } from './grant-admission.js';

const checkpoint = () => new Promise(resolve => {
  const channel = new MessageChannel();
  channel.port1.onmessage = () => { channel.port1.close(); channel.port2.close(); resolve(); };
  channel.port2.postMessage(null);
});
let context = null, storage = null, admitted = null, tail = Promise.resolve(), stream = null;
const pending = new Map();
// Calls the runner let go between storage steps. Delivered to the end, then
// retired, as the page realm does.
const owed = new Map();
const scratch = owner => ({owner, store:new Map(), grants:new Set(), reads:[], writes:[], externalRead:false, entropy:false, topics:[], requests:new Map()});
function release(owner, callId) {
  if (!String(self.__exact_forget(String(callId))).startsWith('storage')) { storage.retire(owner); return; } // 'storage', or 'storage rejected'
  owed.set(callId, owner);
}
// The page's SHA-256 digests in flight, on a LAN dev page (see `init`).
const digests = new Map();
let nextDigest = 1;
// The runner's target first: two targets asking one source with equal
// arguments are two calls (LLP 1027 D1a).
const key = r => JSON.stringify([r.target ?? null, r.source, r.args]);

self.__exact_host = (op, name, value) => {
  if (!context) throw new Error('host call outside an answer');
  if (op === 6) {
    // A page module answers `native.later` on the page; nothing here can answer at once.
    if (name === 'kind' || name === 'available') return admitted.native ? 'native' : '';
    // A topic the page module announces asks this answer again (LLP 1016.002).
    if (name === 'watch') { context.topics.push(String(value)); return; }
    if (name === 'later') return admitted.native ? 'later' : '';
    throw new Error('the browser answers no native call at once; use native.later');
  }
  if (op === 1) { context.requests.set(Number(name), JSON.parse(value)); return; }
  if (op === 2) { context.reads.push(name); return context.store.get(name); }
  if (op === 5) { context.externalRead = true; return; }
  // A draw of secure randomness: a device read the runner counts (LLP 1069.005 D2).
  if (op === 8) { context.entropy = true; return; }
  // Under the agent, the realm's repeatable random bytes (LLP 1069.005 D2b).
  if (op === 11) return stream ? Array.from(stream(Number(value)), b => b.toString(16).padStart(2, '0')).join('') : undefined;
  // `authCallback()` is the page's; `openAuthSession` refuses here: its popup
  // must open in the press's call stack, on the page (LLP 1069.006).
  if (op === 12) { if (name === 'placement') return 'worker'; context.externalRead = true; return `${location.origin}/.exact/auth/callback`; }
  if (!context.grants.has(name) || name.startsWith('exact.kept.')) return `secret ${name} is not granted`;
  context.writes.push([name, op === 3 ? value : null]);
  if (op === 3) context.store.set(name, value); else context.store.delete(name);
};

// Indirect eval runs the verified text at the worker's global scope, where
// a classic script would: the prelude finds `this`, the module defines `exact`.
const evaluate = source => (0, eval)(source);

function init(message) {
  admitted = message.admitted;
  // Disable accidental browser I/O before the module captures globals: a
  // function, so `new WebSocket(url)` refuses by name too, as the JS target's
  // ts-fetch.js does. The prelude refuses timers and the clock, by name, as
  // Hermes does.
  for (const name of ['XMLHttpRequest', 'WebSocket', 'EventSource']) {
    Object.defineProperty(self, name, { value: function () { throw new Error(`${name} is unavailable in data sources`); }, configurable: false });
  }
  // The worker's timers, before the prelude takes them from this global: a storage lock's bound (storage-fs.js),
  // held in this closure and never the module's.
  const timers = { set: self.setTimeout.bind(self), clear: self.clearTimeout.bind(self) };
  storage = createStorage(self, admitted, () => context.owner, message.storage, timers);
  // A cell belongs to the answer that accepted the call, though its operation
  // may be issued in another answer's turn (one queued behind it), as in the
  // page realm.
  bindAnswerStorage(self, storage, () => context.owner);
  // The page reads the drive's seed; this realm's stream starts here (D2b).
  if (message.seed !== null && message.seed !== undefined) stream = agentStream(message.seed, 'typescript');
  // A LAN dev page has no `crypto.subtle`: SHA-256 is the dev protocol's,
  // asked of the page (LLP 1069.005 D1).
  if (!self.crypto.subtle && message.pageDigest) {
    self.__exact_digest = bytes => new Promise((resolve, reject) => {
      const id = nextDigest++;
      digests.set(id, { resolve, reject });
      postMessage({ op: 'digest', id, bytes });
    });
  }
  // Kept keys (LLP 1069.005 D1b): the CryptoKeyPair in this realm's IndexedDB.
  self.__exact_keys = keyStore(message.storage, self.indexedDB);
  evaluate(message.binary);
  evaluate(message.prelude);
  self.__exact_storage = storage.capability;
  self.__exact_install_storage();
  evaluate(message.script);
  if ((self.exact?.abi !== 1 && self.exact?.abi !== 2) || self.exact.appId !== admitted.appId || !sameGrantDeclaration(admitted.grantSet, self.exact.grants) || typeof self.exact.answer !== 'function') throw new Error('module exports mismatch the admitted client');
}

function begin(request) {
  context = {owner:{}, store:new Map(request.store), grants:new Set(request.grants), reads:[], writes:[], externalRead:false,entropy:false,topics:[], requests:new Map()};
  if (request.op === 'answer') return JSON.parse(self.__exact_call(request.source, JSON.stringify(request.args)));
  const parked = pending.get(key(request));
  if (!parked) throw new Error('reply for an answer not in flight');
  pending.delete(key(request));
  context.owner = parked.owner;
  context.requests = parked.requests;
  self.__exact_fulfill(String(parked.ticket), JSON.stringify(request.outcome));
  return {tag:3, call:parked.call};
}

function finish(answer, request) {
  const result = {...answer, reads:context.reads, writes:context.writes, externalRead:context.externalRead,entropy:context.entropy,topics:context.topics};
  if (answer.tag === 1) {
    result.request = context.requests.get(answer.ticket);
    if (!result.request) throw new Error('module awaits a fetch it never made');
    context.requests.delete(answer.ticket);
    pending.set(key(request), {call:answer.call, ticket:answer.ticket, requests:context.requests, owner:context.owner});
  }
  if (answer.tag !== 1) storage.retire(context.owner);
  context = null;
  return result;
}

// The runner let go of every targeted call not in flight (LLP 1016 D5):
// drop it, its storage owner, and its call in the prelude.
async function forget(inFlight) {
  const staying = new Set(inFlight.map(key));
  for (const [parkedKey, parked] of pending) {
    if (JSON.parse(parkedKey)[0] === null || staying.has(parkedKey)) continue;
    pending.delete(parkedKey); release(parked.owner, parked.call);
  }
  // An answer parked behind another's storage is let go too, its turn
  // refused, as the page realm does.
  parked = parked.filter(p => {
    if ((p.request.target ?? null) === null || staying.has(key(p.request))) return true;
    release(p.context.owner, p.answer.call);
    p.done.reject(new Error('the runner let this answer go'));
    return false;
  });
  if (owed.size) await finishLetGo(storage, owed, letGoHooks);
}
const letGoHooks = {
  letGo: () => self.__exact_let_go('', ''),
  disposed: () => false,
  deliver: async owner => {
    const prev = context;
    context = scratch(owner);
    try { await storage.deliver(owner); deliveries++; await checkpoint(); }
    finally { context = prev; }
  },
};

// An answer whose storage operation is queued behind another answer's waits
// with the module (prelude.js `settle`'s `waiting`): it parks and lets the
// turn go, so the answer whose operation is in flight can be resumed and
// deliver it, and is asked again after each delivery, until a pass delivers
// nothing (LLP 1097 D3), as the page realm does (module-glue.js). Held in
// the turn instead, it waited on an operation only that other answer's turn
// could deliver.
let parked = [], deliveries = 0;
async function proceed(answer, request, done) {
  for (;;) {
    if (answer.tag === 1 && answer.ticket === 0 && !answer.waiting) {
      await storage.deliver(context.owner);
      deliveries++;
      await checkpoint();
      answer = JSON.parse(self.__exact_settle(String(answer.call)));
      continue;
    }
    if (answer.tag === 1 && answer.waiting) {
      parked.push({ answer, request, context, done });
      context = null;
      return;
    }
    done.resolve(finish(answer, request));
    return;
  }
}
async function resettle() {
  for (;;) {
    const before = deliveries;
    // A let-go chain a delivery made the head, finished first.
    if (owed.size) await finishLetGo(storage, owed, letGoHooks);
    const waiting = parked; parked = [];
    for (const p of waiting) {
      context = p.context;
      try { await proceed(JSON.parse(self.__exact_settle(String(p.answer.call))), p.request, p.done); }
      catch (error) { storage.retire(context?.owner); context = null; p.done.reject(error); }
    }
    if (deliveries === before || !(parked.length || owed.size)) break;
  }
}
const deferred = () => { const d = {}; d.promise = new Promise((resolve, reject) => { d.resolve = resolve; d.reject = reject; }); return d; };

// One turn: begin or resume; stay here through its own storage; end at an
// answer, at a `fetch`, which the page's host runs (LLP 1027.002 D3), or
// parked behind another answer's storage. Its result comes when its `done`
// does, perhaps after later turns.
async function turn(request, done) {
  let answer = begin(request);
  if (answer.tag === 3) {
    await checkpoint();
    answer = JSON.parse(self.__exact_settle(String(answer.call)));
  }
  await proceed(answer, request, done);
}

self.onmessage = ({ data }) => {
  if (data.op === 'digest') {
    const waiter = digests.get(data.id); digests.delete(data.id);
    if (data.error !== undefined) waiter?.reject(new Error(data.error)); else waiter?.resolve(data.hex);
    return;
  }
  if (data.op === 'init') {
    try { init(data); postMessage({ token: 0, result: { ok: true } }); }
    catch (error) { postMessage({ token: 0, error: String(error?.message ?? error) }); }
    return;
  }
  // After the turns before it, so a call they park is dropped too.
  if (data.op === 'forget') { tail = tail.then(() => forget(data.inFlight)).then(() => { if (parked.length || owed.size) return resettle(); }).catch(() => {}); return; }
  if (data.op !== 'turn') return;
  const done = deferred();
  const run = tail.then(async () => {
    const before = deliveries;
    try { await turn(data.request, done); }
    catch (error) { storage?.retire(context?.owner); context = null; done.reject(error); }
    // Its delivery may have put a parked answer's operation in flight:
    // asked again in a turn of its own, after this one.
    if (deliveries !== before && (parked.length || owed.size)) { const again = tail.then(resettle); tail = again.catch(() => {}); }
  });
  tail = run.catch(() => {});
  done.promise.then(result => postMessage({ token: data.token, result }), error => postMessage({ token: data.token, error: String(error?.message ?? error) }));
};
