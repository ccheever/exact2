// The module's private realm on a dedicated Worker (LLP 1027.002 D2): the
// same prelude, storage capability and turn discipline as the iframe realm
// in module-glue.js, off the page's main thread. Trusted app code, NOT a
// security sandbox. Values cross as messages; the page's runner commits.
// Loaded only after the page's first pixel, by module-glue.js.
import { createStorage } from './storage.js';

const checkpoint = () => new Promise(resolve => {
  const channel = new MessageChannel();
  channel.port1.onmessage = () => { channel.port1.close(); channel.port2.close(); resolve(); };
  channel.port2.postMessage(null);
});
let context = null, storage = null, admitted = null, tail = Promise.resolve();
const pending = new Map();
const key = r => JSON.stringify([r.source, r.args]);

self.__exact_host = (op, name, value) => {
  if (!context) throw new Error('host call outside an answer');
  if (op === 6) { if (name === 'available') return 'web'; throw new Error('native modules are unavailable in the browser'); }
  if (op === 1) { context.requests.set(Number(name), JSON.parse(value)); return; }
  if (op === 2) { context.reads.push(name); return context.store.get(name); }
  if (op === 5) { context.externalRead = true; return; }
  if (!context.grants.has(name) || name.startsWith('exact.kept.')) return `secret ${name} is not granted`;
  context.writes.push([name, op === 3 ? value : null]);
  if (op === 3) context.store.set(name, value); else context.store.delete(name);
};

// Indirect eval runs the verified text at the worker's global scope, where
// a classic script would: the prelude finds `this`, the module defines `exact`.
const evaluate = source => (0, eval)(source);

function init(message) {
  admitted = message.admitted;
  // Disable accidental browser I/O before the module captures globals.
  for (const name of ['XMLHttpRequest', 'WebSocket', 'EventSource', 'setTimeout', 'setInterval', 'requestAnimationFrame']) {
    Object.defineProperty(self, name, { value: () => { throw new Error(`${name} is unavailable in data sources`); }, configurable: false });
  }
  storage = createStorage(self, admitted, () => context.owner, message.agent);
  evaluate(message.prelude);
  self.__exact_storage = storage.capability;
  self.__exact_install_storage();
  evaluate(message.script);
  if (self.exact?.abi !== 1 || self.exact.appId !== admitted.appId || self.exact.grants?.trim() !== admitted.grants.trim() || typeof self.exact.answer !== 'function') throw new Error('module exports mismatch the admitted client');
}

function begin(request) {
  context = {owner:{}, store:new Map(request.store), grants:new Set(request.grants), reads:[], writes:[], externalRead:false, requests:new Map()};
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
  const result = {...answer, reads:context.reads, writes:context.writes, externalRead:context.externalRead};
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

// One turn: begin or resume; stay here through every storage wait; end at
// an answer or at a `fetch`, which the page's host runs (LLP 1027.002 D3).
async function turn(request) {
  let answer = begin(request);
  if (answer.tag === 3) {
    await checkpoint();
    answer = JSON.parse(self.__exact_settle(String(answer.call)));
  }
  while (answer.tag === 1 && answer.ticket === 0) {
    await storage.deliver(context.owner);
    await checkpoint();
    answer = JSON.parse(self.__exact_settle(String(answer.call)));
  }
  return finish(answer, request);
}

self.onmessage = ({ data }) => {
  if (data.op === 'init') {
    try { init(data); postMessage({ token: 0, result: { ok: true } }); }
    catch (error) { postMessage({ token: 0, error: String(error?.message ?? error) }); }
    return;
  }
  if (data.op !== 'turn') return;
  const run = tail.then(async () => {
    try { return await turn(data.request); }
    catch (error) { storage?.retire(context?.owner); context = null; throw error; }
  });
  tail = run.catch(() => {});
  run.then(result => postMessage({ token: data.token, result }), error => postMessage({ token: data.token, error: String(error?.message ?? error) }));
};
