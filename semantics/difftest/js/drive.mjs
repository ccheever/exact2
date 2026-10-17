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
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import { createDocument } from '../../../host/web-js/dom.js';
import { driverRuntime, localImports } from '../../../host/web-js/glue.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../../..');
const webJs = resolve(root, 'host/web-js');

// ---------------------------------------------------------------- values
const bits = new DataView(new ArrayBuffer(8));
const hex = n => { bits.setFloat64(0, n); return bits.getBigUint64(0).toString(16).padStart(16, '0'); };
const fromHex = h => { bits.setBigUint64(0, BigInt('0x' + h)); return bits.getFloat64(0); };
/** A batch value (numbers as their bits) as a runtime value. */
const decode = v => v === null || typeof v !== 'object' ? v : Array.isArray(v) ? v.map(decode) : fromHex(v.$);
/** A runtime value's key, numbers by their bits, NaN canonical (the oracle's `canonical`):
 * the runner's NaN and the module's may differ in sign. */
const key = v => typeof v === 'number' ? number(v) : Array.isArray(v) ? '[' + v.map(key).join(',') + ']' : JSON.stringify(v ?? null);
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
  getElementsByClassName(names) {
    const classes = names.split(/\s+/).filter(Boolean), found = [];
    const walk = n => { for (const c of n.childNodes) { if (c.nodeType === 1 && classes.every(k => c.classList.contains(k))) found.push(c); walk(c); } };
    walk(this); return found;
  },
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
  for (const f of driverRuntime(root)) cpSync(resolve(root, f), resolve(dir, basename(f)));
  // `sin` and `cos` answer the runner's bits (`trig` below), not `Math`'s.
  const roster = resolve(dir, 'roster.js'), engine = 'export const x_sin = Math.sin, x_cos = Math.cos;';
  const text = readFileSync(roster, 'utf8');
  if (!text.includes(engine)) throw new Error(`roster.js no longer has \`${engine}\``);
  writeFileSync(roster, text.replace(engine, 'export const x_sin = x => globalThis.__trig.sin(x), x_cos = x => globalThis.__trig.cos(x);'));
  for (const f of ['admission.js', 'agent.js']) writeFileSync(resolve(dir, f), localImports(readFileSync(resolve(webJs, f), 'utf8')));
  writeFileSync(resolve(dir, 'draw.js'), 'export const drawer = null;\n');
  writeFileSync(resolve(dir, 'path2d.js'), 'export const browserPath2D = globalThis.Path2D;\n');
  writeFileSync(resolve(dir, 'admission-data.js'), "import {createGrantSet} from './admission.js';export const tsGrantSet=createGrantSet(''),rustGrantSet=createGrantSet('');\n");
}

/** The bundle's entry for the program just compiled into `dir`: its voice
 * table installed when the compiler flags one, as host/web-js/build.mjs's
 * `main.js` does, so a sequence (transport.js) has the table to play into. */
function entry(dir) {
  const flag = resolve(dir, 'sounds.flag');
  writeFileSync(resolve(dir, 'entry.js'), [
    "import app from './app.js';",
    ...(existsSync(flag) ? [`import { install as sounds } from './sounds.js'; sounds(${readFileSync(flag, 'utf8')});`] : []),
    "import names, { types } from './names.js';",
    "import { data, journal, clock, advance, Hosts, inflight, Mutations, Sounds } from './rt.js';",
    'globalThis.__drive = { app, names, types, data, journal, clock, advance, Hosts, inflight, Mutations, Sounds };',
  ].join('\n'));
}

/** Compile and bundle one program; the bundle's text, or {outside}. */
async function build(compiler, dir, file) {
  for (const f of ['app.js', 'names.js']) rmSync(resolve(dir, f), { force: true });
  const c = spawnSync(compiler, ['js', file, '-o', dir], { encoding: 'utf8' });
  if (c.status !== 0) return { outside: (c.stderr || c.stdout || `exit ${c.status}`).trim() };
  entry(dir);
  const r = await Bun.build({ entrypoints: [resolve(dir, 'entry.js')], format: 'iife', target: 'browser', define: { 'import.meta.url': '"http://difftest.invalid/app.js"' } });
  if (!r.success) throw new Error(r.logs.map(String).join('\n'));
  return { code: await r.outputs[0].text() };
}

// ---------------------------------------------------------------- one case
async function drive(code, c, hostSources) {
  const document = createDocument();
  // The page's storage: the device's store at launch (LLP 1116 D5), which
  // persist.js reads as a slot is made and writes after each commit.
  const kept = new Map(Object.entries(c.stored ?? {}));
  const localStorage = {
    get length() { return kept.size; }, key: i => [...kept.keys()][i] ?? null,
    getItem: k => kept.has(k) ? kept.get(k) : null, setItem: (k, v) => { kept.set(k, String(v)); }, removeItem: k => { kept.delete(k); },
  };
  const ctx = vm.createContext({
    // Agent mode owns the clock without prerender's eager tab construction.
    document, location: { pathname: '/', search: '?agent', href: 'http://difftest.invalid/?agent', origin: 'http://difftest.invalid' },
    history: { replaceState() {}, pushState() {}, go() {}, state: null }, localStorage,
    addEventListener() {}, removeEventListener() {}, navigator: {},
    matchMedia: () => ({ matches: false, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} }),
    getComputedStyle: () => ({ getPropertyValue: () => '' }),
    setTimeout, clearTimeout, queueMicrotask, performance, console: quiet, fetch: () => Promise.reject(new Error('no network in a drive')), URL, URLSearchParams, TextEncoder, TextDecoder,
    Event: class {}, CustomEvent: class {},
    // No browser layout runs here; symbol projection's observer has nothing to report.
    MutationObserver: class { observe() {} disconnect() {} },
    // The render DOM has one element class: the runtime's `instanceof HTML…Element` is a tag check here.
    ...Object.fromEntries([['HTMLInputElement', 'input'], ['HTMLTextAreaElement', 'textarea'], ['HTMLSelectElement', 'select'], ['HTMLOptionElement', 'option'],
      ['HTMLButtonElement', 'button'], ['HTMLIFrameElement', 'iframe']].map(([name, tag]) => [name, { [Symbol.hasInstance]: el => el?.localName === tag }])),
  });
  ctx.globalThis = ctx; ctx.self = ctx; ctx.window = ctx;
  // The JS target's `sin` and `cos` are the engine's `Math.sin` and
  // `Math.cos`, held to the runner's libm by value, not by bits (LLP 1003 §7).
  // Here they answer the runner's bits for each input the harness computed
  // (`c.trig`, by `sin:`/`cos:` and the input's bits); an input it has not
  // computed yet is asked for (`asked`) and the case is driven again.
  const trig = new Map(Object.entries(c.trig ?? {})), asked = new Set();
  const answer = (f, x) => {
    const k = `${f}:${hex(x)}`;
    if (trig.has(k)) return fromHex(trig.get(k));
    asked.add(k);
    return Math[f](x);
  };
  ctx.__trig = { sin: x => answer('sin', x), cos: x => answer('cos', x) };
  vm.runInContext(code, ctx, { filename: 'app.js' });
  const { app, names, types, data, journal, clock, advance, Hosts, Mutations, Sounds } = ctx.__drive;
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
  // The host's facts (viewport, page, time…) as the runner answered them at
  // boot: the module's own readers of the page (facts.js) assign in vain.
  data.reserved = {};
  for (const s of hostSources) {
    const f = (source, args, name) => name in c.facts ? decode(c.facts[name]) : unanswered(source, args);
    Object.defineProperty(data.reserved, s, { get: () => f, set() {}, enumerable: true });
  }
  // Every command, in order: the runtime's own and any it does not carry.
  let commands = [];
  const record = name => (...args) => { commands.push(`command ${name}${args.map(a => ' ' + untyped(a)).join('')}`); };
  for (const k of Object.keys(Hosts)) Hosts[k] = record(k);
  // `setMode` is the runtime's own, as the runner's (LLP 1121 D5): no host is
  // told, and the window's mode selects presentation, which no line shows.
  Hosts.setMode = () => {};
  // The voice table's commands are the runtime's own (rt.js `Sounds.own`): they reach `Sounds.apply`
  // with the commit's whole command list, in order, not `Hosts`. Record the list as the runner does:
  // the host commands just recorded from it are replaced by the whole of it.
  let applySounds = Sounds.apply;
  Object.defineProperty(Sounds, 'apply', {
    configurable: true,
    get: () => cmds => {
      // `setMode` was never recorded (above), and the runner tells no host of it.
      const shown = cmds.filter(([name]) => name !== 'setMode');
      if (shown.some(([name]) => Sounds.own.has(name))) {
        const hosts = shown.filter(([name]) => !Sounds.own.has(name)).length;
        commands.splice(commands.length - hosts, hosts, ...shown.map(([name, args]) => `command ${name}${args.map(a => ' ' + untyped(a)).join('')}`));
      }
      return applySounds?.(cmds);
    },
    set: f => { applySounds = f; },
  });
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
    return { lines: [...out, ...notes], asked: [...asked] };
  }
  await settle();
  const booted = outcome(since(before)) === 'poisoned' ? 'poisoned' : 'ok';
  out.push(`outcome ${booted}`);
  if (booted === 'poisoned') return { lines: [...out, ...notes], asked: [...asked] };
  const root = document.root;
  // Elements in preorder; a virtualized list's rows are its window's (left out), as observe.rs leaves
  // out a collection's. list.js attaches its collection as `$list`, regardless of the author's role.
  // A grouped list (`list appearance="auto"`) scrolls too but builds every row.
  const walk = (e, f, windowed) => { for (const k of e.childNodes) if (k.nodeType === 1) { f(k); if (!(windowed && windowedList(k))) walk(k, f, windowed); } };
  const windowedList = e => !!e.$list;
  const find = id => { let hit = null; walk(root, e => { if (!hit && e.getAttribute('data-testid') === id) hit = e; }, false); return hit; };
  // What the store keeps for a persisted state, read back by its slot's type
  // (persist.js's text: a non-finite number as its name).
  const revive = (v, t) => t === 'n' && typeof v === 'string' ? Number(v) : Array.isArray(t) && t[0] === '?' ? (v === null ? null : revive(v, t[1]))
    : Array.isArray(t) && Array.isArray(v) ? v.map(x => revive(x, t[1])) : v;
  const storeLines = () => {
    for (const name of c.persisted ?? []) {
      const i = names[0].indexOf(name), text = localStorage.getItem('exact.state.' + name);
      let shown = '-';
      try { if (text != null) shown = typed(revive(JSON.parse(text), types[0][i]), types[0][i]); } catch {}
      out.push(`store ${name} ${shown}`);
    }
  };
  const observe = () => {
    names.forEach((group, g) => {
      if (g === 1) storeLines(); // after the slots, as observe.rs writes them
      group.forEach((name, i) => {
        // The locale slot (`#locale`) is no root slot the runner shows.
        if (name.startsWith('#')) return;
        let v;
        try { v = state[g][i](); } catch (e) { notes.push(`# js: ${['slot', 'derive', 'resource'][g]} ${name}: ${e.message}`); return; }
        out.push(`${['slot', 'derive', 'resource'][g]} ${name} ${typed(v, types[g][i])}`);
      });
    });
    // Each queue's waiting sends (LLP 1092 D12), in declaration order, as observe.rs writes them.
    for (const m of Mutations) if (m.queue) out.push(`queued ${m.name} ${m.wait?.length ?? 0}`);
    out.push(...commands); commands = [];
    walk(root, e => {
      const id = e.getAttribute('data-testid');
      if (id == null) return;
      // A text, or an option (its label is its text, as agent.js reads it).
      const svgText = (e.localName === 'text' || e.localName === 'tspan') && !e.childElementCount;
      // A paragraph of inline runs has no text of its own: its runs carry it (agent.js `record`, `run`).
      const run = e.parentElement?.hasAttribute('data-exact-text') && e.parentElement.getAttribute('markup') !== 'markdown';
      const text = (e.hasAttribute('data-exact-text') || e.localName === 'option' || svgText || run) && (e.$source != null || !e.childElementCount)
        ? quote(e.$source ?? e.textContent) : '-';
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
      // options, a checkbox or a radio no text.
      const options = e?.localName === 'select' ? e.getElementsByTagName('option') : null;
      const deliverable = e && e.type !== 'checkbox' && e.type !== 'radio' && datetime(e, text)
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
  return { lines: [...out, ...notes], asked: [...asked] };
}

// A loaded piece the drive does not carry (the Markdown wasm, a fetch)
// fails after its case: never the process.
process.on('unhandledRejection', () => {});
const quiet = process.env.DIFFTEST_JS_DEBUG ? console : { ...console, error() {}, warn() {} };
const batch = JSON.parse(readFileSync(process.argv[2], 'utf8'));
runtime(batch.work);
for (const c of batch.cases) {
  let result;
  try {
    const b = await build(batch.compiler, batch.work, c.file);
    result = b.outside !== undefined ? b : await drive(b.code, c, batch.hostSources);
  } catch (e) { result = { error: String(e?.stack ?? e) }; }
  process.stdout.write(JSON.stringify(result) + '\n');
}
