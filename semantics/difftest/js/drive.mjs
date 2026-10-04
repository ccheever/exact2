// The web JS target's half of a differential run (`difftest --js`): each
// case's program compiled by `exact-web-js` (host/web-js, LLP 1071), the
// module bundled with its runtime and run headless under Bun, in a fresh VM
// context per case over the render DOM (host/web-js/dom.js) given event
// listeners, its data sources answered synchronously from the runner's
// oracle transcript, and its state printed as `Contract.Observe` prints it
// (semantics/Contract/Observe.lean).
//
//   bun semantics/difftest/js/drive.mjs <batch.json>
//
// The batch: { compiler, work, hostSources: [name…], cases: [{ file, events,
// answers: [[source, [arg…], answer]…], facts: {resource: value} }] }, every
// number as {"$": "<16 hex digits of IEEE bits>"}. Prints one JSON line per
// case: {lines} (the observation), {outside} (exact-web-js refused the
// program: outside the JS target) or {error} (the driver failed).
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import { createDocument } from '../../../host/web-js/dom.js';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../../..');
const webJs = resolve(root, 'host/web-js'), web = resolve(root, 'host/web');

// ---------------------------------------------------------------- values
const bits = new DataView(new ArrayBuffer(8));
const hex = n => { bits.setFloat64(0, n); return bits.getBigUint64(0).toString(16).padStart(16, '0'); };
const fromHex = h => { bits.setBigUint64(0, BigInt('0x' + h)); return bits.getFloat64(0); };
/** A batch value (numbers as their bits) as a runtime value. */
const decode = v => v === null || typeof v !== 'object' ? v : Array.isArray(v) ? v.map(decode) : fromHex(v.$);
/** A runtime value's key, numbers by their bits (the oracle's `canonical`). */
const key = v => typeof v === 'number' ? 'n' + hex(v) : Array.isArray(v) ? '[' + v.map(key).join(',') + ']' : JSON.stringify(v ?? null);
const quote = s => '"' + [...s].map(c => c === '"' ? '\\"' : c === '\\' ? '\\\\' : c === '\n' ? '\\n' : c === '\r' ? '\\r' : c === '\t' ? '\\t'
  : c.codePointAt(0) < 0x20 ? '\\u00' + c.codePointAt(0).toString(16).padStart(2, '0') : c).join('') + '"';
const number = n => 'n' + (n !== n ? '7ff8000000000000' : hex(n));
/** A runtime value as the observation prints it, by its type as names.js
 * `types` gives it (`"n" "b" "s" "u"`, `["?", T]`, `["[", T]`, a record
 * `{field: T}`): lists and records are arrays, unit and `none` null,
 * `some(v)` v. */
function typed(v, t) {
  if (t === 'u') return v == null ? '()' : `?${untyped(v)}`;
  if (typeof t === 'string') return untyped(v);
  if (Array.isArray(t) && t[0] === '?') return v == null ? 'none' : `some(${typed(v, t[1])})`;
  if (Array.isArray(t)) return Array.isArray(v) ? `[${v.map(x => typed(x, t[1])).join(',')}]` : `?${untyped(v)}`;
  return Array.isArray(v) ? `{${Object.values(t).map((ft, k) => typed(v[k], ft)).join(',')}}` : `?${untyped(v)}`;
}
/** A value with no type to read it by (a command's argument): null is `none`. */
const untyped = v => typeof v === 'number' ? number(v) : typeof v === 'string' ? quote(v) : typeof v === 'boolean' ? String(v)
  : v == null ? 'none' : Array.isArray(v) ? `[${v.map(untyped).join(',')}]` : `?${String(v)}`;

// ---------------------------------------------------------------- the DOM
// dom.js renders pages; a drive also needs listeners and the few element
// reads rt.js's handlers make.
const probe = createDocument();
const ElementProto = Object.getPrototypeOf(probe.createElement('div'));
const NodeProto = Object.getPrototypeOf(ElementProto);
Object.assign(NodeProto, {
  addEventListener(type, f) { ((this.$listeners ??= {})[type] ??= []).push(f); },
  removeEventListener(type, f) { const l = this.$listeners?.[type]; if (l && l.includes(f)) l.splice(l.indexOf(f), 1); },
});
Object.defineProperties(ElementProto, {
  type: { get() { return this.getAttribute('type') ?? ''; }, configurable: true },
  children: { get() { return this.childNodes.filter(c => c.nodeType === 1); }, configurable: true },
  parentElement: { get() { return this.parentNode?.nodeType === 1 ? this.parentNode : null; }, configurable: true },
});
Object.assign(ElementProto, {
  closest() { return null; }, focus() {}, blur() {}, click() {}, scrollTo() {},
  contains(n) { for (; n; n = n.parentNode) if (n === this) return true; return false; },
});
// A date, time or datetime-local input holds HTML's value format, within its
// `min` and `max`, or nothing (runner/src/runner/control.rs).
const DATE = '\\d{4,}-(0[1-9]|1[0-2])-(0[1-9]|[12]\\d|3[01])', TIME = '([01]\\d|2[0-3]):[0-5]\\d(:[0-5]\\d(\\.\\d{1,3})?)?';
const FORMATS = { date: new RegExp(`^${DATE}$`), time: new RegExp(`^${TIME}$`), 'datetime-local': new RegExp(`^${DATE}T${TIME}$`) };
function datetime(e, text) {
  const f = e.localName === 'input' && FORMATS[e.type];
  if (!f || text === '') return true;
  if (!f.test(text)) return false;
  const bound = k => { const b = e.getAttribute(k); return b && f.test(b) ? b : null; };
  return !(bound('min') && text < bound('min')) && !(bound('max') && text > bound('max'));
}
const fire = (e, type) => { for (const f of [...(e.$listeners?.[type] ?? [])]) f({ type, target: e, currentTarget: e, stopPropagation() {}, preventDefault() {}, key: '' }); };

// ---------------------------------------------------------------- the runtime
// Every file the bundle may reach, as host/web-js/build.mjs lays them out.
function runtime(dir) {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  for (const f of readdirSync(webJs)) if (f.endsWith('.js')) cpSync(resolve(webJs, f), resolve(dir, f));
  for (const f of ['frames.js', 'motion-glue.js', 'input-glue.js', 'markup-editor.js', 'textflow-glue.js', 'timer-glue.js', 'presence-glue.js', 'native-glue.js',
    'geometry-glue.js', 'collection-glue.js', 'image-glue.js', 'navigation.js', 'canvas2d-glue.js', 'auth-glue.js', 'storage-environment.js', 'http-body.js', 'grant-admission.js'])
    cpSync(resolve(web, f), resolve(dir, f));
  writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(webJs, 'admission.js'), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'"));
  writeFileSync(resolve(dir, 'draw.js'), 'export const drawer = null;\n');
  writeFileSync(resolve(dir, 'path2d.js'), 'export const browserPath2D = globalThis.Path2D;\n');
  writeFileSync(resolve(dir, 'admission-data.js'), "import {createGrantSet} from './admission.js';export const tsGrantSet=createGrantSet(''),rustGrantSet=createGrantSet('');\n");
  writeFileSync(resolve(dir, 'entry.js'), [
    "import app from './app.js';",
    "import names, { types } from './names.js';",
    "import { data, journal, clock, advance, Hosts, inflight } from './rt.js';",
    'globalThis.__drive = { app, names, types, data, journal, clock, advance, Hosts, inflight };',
  ].join('\n'));
}

/** Compile and bundle one program; the bundle's text, or {outside}. */
async function build(compiler, dir, file) {
  for (const f of ['app.js', 'names.js']) rmSync(resolve(dir, f), { force: true });
  const c = spawnSync(compiler, ['js', file, '-o', dir], { encoding: 'utf8' });
  if (c.status !== 0) return { outside: (c.stderr || c.stdout || `exit ${c.status}`).trim() };
  const r = await Bun.build({ entrypoints: [resolve(dir, 'entry.js')], format: 'iife', target: 'browser' });
  if (!r.success) throw new Error(r.logs.map(String).join('\n'));
  return { code: await r.outputs[0].text() };
}

// ---------------------------------------------------------------- one case
async function drive(code, c, hostSources) {
  const document = createDocument();
  const ctx = vm.createContext({
    document, location: { pathname: '/', search: '', href: 'http://difftest.invalid/', origin: 'http://difftest.invalid' },
    history: { replaceState() {}, pushState() {}, go() {}, state: null }, localStorage: { length: 0, key() {}, getItem() { return null; }, setItem() {}, removeItem() {} },
    addEventListener() {}, removeEventListener() {}, navigator: {},
    setTimeout, clearTimeout, queueMicrotask, performance, console, URL, URLSearchParams, TextEncoder, TextDecoder,
    Event: class {}, CustomEvent: class {}, __exactRender: true,
  });
  ctx.globalThis = ctx; ctx.self = ctx; ctx.window = ctx;
  vm.runInContext(code, ctx, { filename: 'app.js' });
  const { app, names, types, data, journal, clock, advance, Hosts } = ctx.__drive;
  const notes = [];
  // The runner's transcript answers every call; one it never made is a divergence.
  const answers = new Map(c.answers.map(([source, args, answer]) => [source + key(decode(args)), answer]));
  const unanswered = (source, args) => {
    notes.push(`# js: the oracle has no answer for ${source}${key(args)}`);
    const e = new Error(`the oracle has no answer for ${source}`); e.refuse = true; throw e;
  };
  data.answer = (source, args) => {
    const k = source + key(args);
    return answers.has(k) ? { v: decode(answers.get(k)) } : unanswered(source, args);
  };
  data.reserved = Object.fromEntries(hostSources.map(s => [s, (source, args, name) => name in c.facts ? decode(c.facts[name]) : unanswered(source, args)]));
  // Every command, in order: the runtime's own and any it does not carry.
  let commands = [];
  const record = name => (...args) => { commands.push(`command ${name}${args.map(a => ' ' + untyped(a)).join('')}`); };
  for (const k of Object.keys(Hosts)) Hosts[k] = record(k);
  Object.setPrototypeOf(Hosts, new Proxy({}, { get: (_, name) => typeof name === 'string' ? record(name) : undefined }));

  const out = ['== boot'];
  const mark = () => journal.start + journal.length;
  const since = at => journal.slice(Math.max(0, at - journal.start));
  const outcome = lines => lines.some(l => /\bpoisoned\b/.test(l)) ? 'poisoned' : lines.some(l => /\brefused\b/.test(l)) ? 'refused' : 'ok';
  const settle = async () => { await new Promise(r => setTimeout(r, 0)); };
  let state;
  const before = mark();
  try { state = app(); } catch (e) {
    out.push(/poison/.test(e.message) ? 'outcome poisoned' : 'outcome refused', `# ${e.message}`);
    return [...out, ...notes];
  }
  await settle();
  const booted = outcome(since(before)) === 'poisoned' ? 'poisoned' : 'ok';
  out.push(`outcome ${booted}`);
  if (booted === 'poisoned') return [...out, ...notes];
  const root = document.root;
  // Elements in preorder; a virtualized list's rows are its window's (left out).
  const walk = (e, f, windowed) => { for (const k of e.childNodes) if (k.nodeType === 1) { f(k); if (!(windowed && windowedList(k))) walk(k, f, windowed); } };
  const windowedList = e => e.getAttribute('role') === 'list' && e.hasAttribute('data-scroll');
  const find = id => { let hit = null; walk(root, e => { if (!hit && e.getAttribute('data-testid') === id) hit = e; }, false); return hit; };
  const observe = () => {
    names.forEach((group, g) => group.forEach((name, i) => {
      let v;
      try { v = state[g][i](); } catch (e) { notes.push(`# js: ${['slot', 'derive', 'resource'][g]} ${name}: ${e.message}`); return; }
      out.push(`${['slot', 'derive', 'resource'][g]} ${name} ${typed(v, types[g][i])}`);
    }));
    out.push(...commands); commands = [];
    walk(root, e => {
      const id = e.getAttribute('data-testid');
      if (id == null) return;
      // A text, or an option (its label is its text, as agent.js reads it).
      const svgText = (e.localName === 'text' || e.localName === 'tspan') && !e.childElementCount;
      const text = e.hasAttribute('data-exact-text') || e.localName === 'option' || svgText ? quote(e.$source ?? e.textContent) : '-';
      out.push(`view ${quote(id)} ${text}`);
    }, true);
  };
  observe();
  for (const ev of c.events) {
    const at = mark();
    let refused = false;
    if (ev.tap !== undefined) {
      out.push(`== tap ${quote(ev.tap)}`);
      const e = find(ev.tap);
      if (!e || !e.$listeners?.click?.length) refused = true; else fire(e, 'click');
    } else if (ev.type !== undefined) {
      const [id, text] = ev.type;
      out.push(`== type ${quote(id)} ${quote(text)}`);
      const e = find(id);
      // What a browser can deliver, as the runner holds a host to it
      // (runner/src/runner/control.rs): a select reports one of its enabled
      // options, a checkbox no text.
      const options = e?.localName === 'select' ? e.getElementsByTagName('option') : null;
      const deliverable = e && e.type !== 'checkbox' && datetime(e, text)
        && (!options || options.some(o => !o.hasAttribute('disabled') && (o.getAttribute('value') ?? o.textContent) === text));
      if (!deliverable || !e.$listeners?.change?.length) refused = true; else { e.value = text; fire(e, 'change'); }
    } else {
      const ms = fromHex(ev.clock.$);
      out.push(`== clock +${String(ms)}`);
      advance(clock.now + ms);
    }
    await settle();
    const o = refused ? 'refused' : outcome(since(at));
    out.push(`outcome ${o}`);
    if (o !== 'ok') out.push(...since(at).map(l => `# ${l}`));
    if (o === 'poisoned') break;
    observe();
  }
  return [...out, ...notes];
}

const batch = JSON.parse(readFileSync(process.argv[2], 'utf8'));
runtime(batch.work);
for (const c of batch.cases) {
  let result;
  try {
    const b = await build(batch.compiler, batch.work, c.file);
    result = b.outside !== undefined ? b : { lines: await drive(b.code, c, batch.hostSources) };
  } catch (e) { result = { error: String(e?.stack ?? e) }; }
  process.stdout.write(JSON.stringify(result) + '\n');
}
