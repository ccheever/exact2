// A stream's caller's signal on the web JS target (ts-stream.js `open`): the stream ends when the signal aborts, as a
// browser's fetch does, whatever the app's own code does to the signal or to `AbortSignal.any`, and lets go of the
// signal when it ends.
import { afterAll, beforeAll, test, expect } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const root = resolve(import.meta.dir, '../..');
// A folder of the runtime's modules side by side, as a build lays them out, with the app's grants in
// `admission-data.js`; the rest of the runtime is a stub.
function runtime(prefix) {
  const dir = mkdtempSync(resolve(tmpdir(), prefix));
  for (const name of ['ts-stream.js', 'ts-fetch.js', 'ts-body-from.js', 'shape.js', 'uploads.js']) cpSync(resolve(root, 'host/web-js', name), resolve(dir, name));
  for (const name of ['http-body.js', 'grant-admission.js', 'faults.js', 'address-space.js', 'navigation.js', 'storage-environment.js']) cpSync(resolve(root, 'host/web', name), resolve(dir, name));
  writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(root, 'host/web-js/admission.js'), 'utf8').replaceAll("'../web/", "'./"));
  writeFileSync(resolve(dir, 'admission-data.js'), "import { createGrantSet } from './admission.js'; export const tsGrantSet = createGrantSet(null);\n");
  writeFileSync(resolve(dir, 'rt.js'), 'export const clock={agent:false,now:0},journal=[],Resources=[],inflight={n:0};export const drive=()=>{},Origin={now:()=>"app"};export const checkpoint=()=>({kept:null});export const commit=f=>f();export const R=()=>{};export const painted=()=>Promise.resolve();\n');
  writeFileSync(resolve(dir, 'rt-link.js'), "import * as rt from './rt.js'; export { rt };\n");
  writeFileSync(resolve(dir, 'names.js'), 'export const sourceTypes = {};\n');
  return dir;
}
const at = (dir, name) => pathToFileURL(resolve(dir, name)).href;

let dir, open;
beforeAll(async () => {
  dir = runtime('exact-ts-stream-abort-');
  ({ open } = await import(at(dir, 'ts-stream.js')));
});
afterAll(() => rmSync(dir, { recursive: true, force: true }));

// A stream of `signal`, with the controller the runtime ends it through
const opened = signal => {
  const stream = new AbortController();
  return { stream, ended: open({ input: 'https://example.test/feed', init: { exactStream: event => event.data, signal } }, v => v, null, () => {}, stream) };
};

test("a stream ends at its caller's abort past an app's listener that stops it", async () => {
  const caller = new AbortController(), reason = new Error('the app stopped it');
  caller.signal.addEventListener('abort', event => event.stopImmediatePropagation());
  const { stream, ended } = opened(caller.signal);
  caller.abort(reason);
  expect(stream.signal.aborted).toBe(true);
  expect(stream.signal.reason).toBe(reason);
  await ended;
});

test("a stream ends at its caller's abort after the app replaces AbortSignal.any", async () => {
  const any = AbortSignal.any;
  // Each signal the replacement composes stops its own abort event before any other listener hears it
  AbortSignal.any = function (signals) {
    const signal = Reflect.apply(any, this, [signals]);
    signal.addEventListener('abort', event => event.stopImmediatePropagation());
    return signal;
  };
  try {
    const caller = new AbortController(), reason = new Error('aborted');
    const { stream, ended } = opened(caller.signal);
    caller.abort(reason);
    expect(stream.signal.reason).toBe(reason);
    await ended;
  } finally { AbortSignal.any = any; }
});

test('a stream whose signal is no AbortSignal answers so, in Hermes\'s words', async () => {
  for (const signal of [17, 0, {}]) expect(await opened(signal).ended).toEqual({ error: 'fetch: init.signal is not an AbortSignal', code: 'error', reason: '' });
});

test("a stream that has ended no longer hears its caller's signal", async () => {
  const caller = new AbortController(), { stream, ended } = opened(caller.signal);
  await ended;
  caller.abort();
  expect(stream.signal.aborted).toBe(false);
});

test("the runtime composes with the page's AbortSignal.any when the app's first import replaces it", async () => {
  const app = runtime('exact-ts-stream-order-');
  writeFileSync(resolve(app, 'ts-data.js'), readFileSync(resolve(root, 'host/web-js/ts-data.js'), 'utf8')
    .replace("'__APP_TS__'", "'./app.js'").replace('__AUTH_IMPORT__', '').replace('__AUTH_INSTALL__', ''));
  // The app's first import reaches the page's AbortSignal through a signal, which no binding of the runtime's stands
  // in for, and replaces `any` with one whose signals stop their own abort event
  writeFileSync(resolve(app, 'replace-any.js'), `const Signal = new AbortController().signal.constructor, any = Signal.any;
Signal.any = function (signals) { const signal = Reflect.apply(any, this, [signals]); signal.addEventListener('abort', event => event.stopImmediatePropagation()); return signal; };
export const restore = () => { Signal.any = any; };
`);
  writeFileSync(resolve(app, 'app.js'), "import './replace-any.js';\nimport { fetch } from './ts-fetch.js';\nexport const grants = '';\nexport const answer = () => fetch;\n");
  try {
    await import(at(app, 'ts-data.js'));
    const { ownSignal } = await import(at(app, 'ts-fetch.js')), caller = new AbortController();
    let heard = false;
    ownSignal(caller.signal).addEventListener('abort', () => { heard = true; });
    caller.abort();
    expect(heard).toBe(true);
  } finally {
    (await import(at(app, 'replace-any.js'))).restore();
    rmSync(app, { recursive: true, force: true });
  }
});

for (const name of ['rows', 'optionalRows']) {
  test(`${name} preserve frozen row identity across pages`, async () => withRows(({ setRows, answer }) => {
    const first = frozenRow('first'), second = frozenRow('second');
    setRows([first, second]);
    const initial = answer(name);
    setRows([second, first]);
    const reordered = answer(name);
    expect(reordered[0]).toBe(initial[1]);
    expect(reordered[1]).toBe(initial[0]);
    if (name === 'optionalRows') {
      setRows([null, first]);
      const optional = answer(name);
      expect(optional[0]).toBeNull();
      expect(optional[1]).toBe(initial[0]);
    }
  }));
}

test('frozen containers recheck mutable children and getters', async () => withRows(({ setRows, answer }) => {
  const mutable = Object.freeze({id:'mutable', parts:[{text:'before'}]});
  setRows([mutable]);
  const before = answer();
  mutable.parts[0].text = 'after';
  expect(answer()).toEqual([['mutable', [['after']]]]);
  expect(before).toEqual([['mutable', [['before']]]]);
  mutable.parts[0].extra = true;
  expect(answer).toThrow('field `extra` is not in the shape');
  let text = 'before';
  const accessor = Object.freeze({id:'getter', parts:Object.freeze([Object.freeze({get text() { return text; }})])});
  setRows([accessor]);
  expect(answer()).toEqual([['getter', [['before']]]]);
  text = 'after';
  expect(answer()).toEqual([['getter', [['after']]]]);
}));

test('frozen cycles report shape errors and sparse arrays recheck inherited elements', async () => withRows(({ setRows, answer }) => {
  const cyclic = {id:'cycle', parts:Object.freeze([])};
  cyclic.unexpected = cyclic;
  setRows([Object.freeze(cyclic)]);
  expect(answer).toThrow('field `unexpected` is not in the shape');
  const sparse = Object.freeze({id:'sparse', parts:Object.freeze(Array(1))});
  const inherited = Object.getOwnPropertyDescriptor(Array.prototype, '0');
  try {
    Object.defineProperty(Array.prototype, '0', {value:Object.freeze({text:'before'}), configurable:true, writable:true});
    setRows([sparse]);
    expect(answer()).toEqual([['sparse', [['before']]]]);
    Object.defineProperty(Array.prototype, '0', {value:Object.freeze({text:'after'}), configurable:true, writable:true});
    expect(answer()).toEqual([['sparse', [['after']]]]);
  } finally {
    if (inherited) { Object.defineProperty(Array.prototype, '0', inherited); } else { delete Array.prototype[0]; }
  }
}));

test('array validation follows JSON serialization rather than iteration', async () => withRows(({ setRows, answer }) => {
  setRows([frozenRow('first')]);
  answer();
  const serializer = Object.getOwnPropertyDescriptor(Array.prototype, 'toJSON');
  try {
    Object.defineProperty(Array.prototype, 'toJSON', {value() { return [...this, {text:17}]; }, configurable:true});
    expect(answer).toThrow('expected a string, got a number');
  } finally {
    if (serializer) { Object.defineProperty(Array.prototype, 'toJSON', serializer); } else { delete Array.prototype.toJSON; }
  }
  let iterableText = 17;
  const parts = [Object.freeze({text:'indexed'})];
  Object.defineProperty(parts, Symbol.iterator, {value:function* () { yield {text:iterableText}; }});
  setRows([Object.freeze({id:'iterator', parts:Object.freeze(parts)})]);
  expect(answer()).toEqual([['iterator', [['indexed']]]]);
  iterableText = false;
  expect(answer()).toEqual([['iterator', [['indexed']]]]);
}));

test('frozen proxies cannot bypass serialization or revocation checks', async () => withRows(({ setRows, answer }) => {
  let invalid = false;
  const child = new Proxy(Object.freeze({text:'stable'}), {
    get(target, key, receiver) {
      return key === 'toJSON' && invalid ? () => ({text:17}) : Reflect.get(target, key, receiver);
    },
  });
  setRows([Object.freeze({id:'proxy', parts:Object.freeze([child])})]);
  expect(answer()).toEqual([['proxy', [['stable']]]]);
  invalid = true;
  expect(answer).toThrow('expected a string, got a number');
  const revocable = Proxy.revocable(Object.freeze({text:'revocable'}), {});
  setRows([Object.freeze({id:'revocable', parts:Object.freeze([revocable.proxy])})]);
  answer();
  revocable.revoke();
  expect(answer).toThrow();
}));

async function withRows(check) {
  const directory = runtime('exact-ts-rows-');
  writeFileSync(resolve(directory, 'ts-data.js'), readFileSync(resolve(root, 'host/web-js/ts-data.js'), 'utf8')
    .replace("'__APP_TS__'", "'./app.js'").replace('__AUTH_IMPORT__', '').replace('__AUTH_INSTALL__', ''));
  writeFileSync(resolve(directory, 'names.js'), "const row = {id:'s', parts:['[', {text:'s'}]}; export const sourceTypes = {rows:[[], ['[', row]], optionalRows:[[], ['[', ['?', row]]]}\n");
  writeFileSync(resolve(directory, 'app.js'), "export const appId='test.rows'; let rows=[]; export const setRows=value=>{rows=value;}; export const answer=()=>rows;\n");
  try {
    const { setRows } = await import(at(directory, 'app.js'));
    const { install } = await import(at(directory, 'ts-data.js'));
    const data = { q:[] };
    install(data);
    await check({ setRows, answer:(name = 'rows') => data.answer(name, [], new Map()).v });
  } finally { rmSync(directory, { recursive:true, force:true }); }
}

function frozenRow(id) {
  return Object.freeze({id, parts:Object.freeze([Object.freeze({text:id})])});
}
