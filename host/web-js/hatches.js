// A node marked `hatch="word"` on the JS target (@ref LLP 1075.003.000 §3.2).
// The app's page module (`modules/web/index.js`) may export
//
//   export function element(e)       // after the commit that mounts the node,
//                                    // again when its data-* words change
//   export function elementEnded(e)  // before the node leaves
//
// with `e` = { hatch, data, element, isNew, isLive, click(), focus(), blur(),
// input(text), diagnostics }:
// `element` is the node's own element and `data` its `dataset`, the web's own
// (so the host's words, `data-testid` and the rest, are there too). On the web
// a hatched node gives up nothing but the call. The page module loads after
// first paint, as a module view's does (rt.js `painted`), and a node mounted before
// then is told once it has. rt.js re-exports `ht`: only a plan that marks a
// node bundles this.
import { onEnd, journal, clock, viewId, inflight, painted } from "./rt.js";

let Page = null;
const said = new Set(), warned = new Set();
const say = line => journal.push(`t=${clock.now} hatch ${line}`);
const page = () => Page ??= painted().then(() => import("./native.js")).then(m => m.pageTable());
// `state.hatches` (the agent), as Apple's: per word, its live nodes, and its
// calls by moment. A row's calls after the first of each moment are counted,
// not journaled, so a fling does not flood the journal.
const stats = Object.create(null), scopes = Object.create(null);
const counted = word => stats[word] ??= { live: 0, reusable: 0, lost: [], calls: Object.create(null) };
const publish = () => {
  const x = globalThis.exact;
  if (!x || x.hatchState) return;
  x.hatchState = state; x.hatchPerf = { reply: perfReply, site: perfSite, window: perfWindow }; x.hatchActs = { drains: () => A.drains, queued: () => Acts.length }; x.diagnostics = diagnostics("module");
};

// What Exact measures by itself, and what hatch code adds (@ref LLP
// 1075.003.000.001 §3.1–3.2): development only, as `perf` is (LLP 1079 D1);
// a production page keeps nothing and each call returns on this one flag.
let Dev = null;
const dev = () => Dev ??= globalThis.exact?.plan != null;

// Every call, timed: a node's by plan site and moment, another scope's by its
// hatch. Times are exclusive: a call nested in another is charged to itself.
const Timing = new Map(), Stack = [];
// The last 1,024 calls (start, end, hatch), for a late frame's join (§3.1).
const Calls = [];
let callAt = 0;
function timed(hatch, site, moment, f) {
  if (!dev()) return f();
  const frame = { inner: 0 }, t0 = performance.now();
  Stack.push(frame);
  try { return f(); } finally {
    const t1 = performance.now(), whole = t1 - t0, own = Math.max(0, whole - frame.inner);
    Stack.pop();
    if (Stack.length) Stack[Stack.length - 1].inner += whole;
    const key = `${hatch}\n${site ?? ""}\n${moment}`;
    let w = Timing.get(key);
    if (!w) Timing.set(key, w = { hatch, ...(site != null ? { site } : {}), moment, calls: 0, ms: 0, worst: 0 });
    w.calls++; w.ms += own; if (own > w.worst) w.worst = own;
    if (Calls.length < 1024) Calls.push([t0, t1, hatch]); else { Calls[callAt] = [t0, t1, hatch]; callAt = (callAt + 1) % 1024; }
  }
}

// `diagnostics` (§3.2), with its bounds per page module: 64 counters, 64
// timings (a cumulative count, sum and max, and the last 256 samples), 32 open
// spans, 16 snapshots of 4 KB, and a 256-byte log line, 20 a second of session
// clock per scope. Past a bound a call is refused and counted, never grown.
const NAME = /^[a-z0-9.-]{1,64}$/, enc = new TextEncoder();
const D = { counters: new Map(), timings: new Map(), snapshots: new Map(), buckets: new Map(), bad: new Set(), open: 0, rejected: 0, abandoned: 0, limited: 0 };
const cut = (text, max) => {
  if (enc.encode(text).length <= max) return text;
  let out = "", n = 0;
  for (const ch of text) { const k = enc.encode(ch).length; if (n + k > max - 3) break; out += ch; n += k; }
  return out + "…";
};
const named = (scope, name) => {
  if (typeof name === "string" && NAME.test(name)) return true;
  D.rejected++;
  const shown = cut(String(name), 64);
  if (D.bad.size < 8 && !D.bad.has(shown)) {
    D.bad.add(shown);
    say(`${scope}: diagnostics refused the name ${JSON.stringify(shown)} (lowercase letters, digits, "-" and ".", at most 64 bytes)`);
  }
  return false;
};
const slot = (map, cap, scope, name, make) => {
  const key = `${scope}\n${name}`;
  let v = map.get(key);
  if (!v) { if (map.size >= cap) { D.rejected++; return null; } map.set(key, v = make()); }
  return v;
};
const sample = (scope, name, ms, measured) => {
  if (!(ms >= 0)) { D.rejected++; return; }
  const t = slot(D.timings, 64, scope, name, () => ({ count: 0, sum: 0, max: 0, ring: [], at: 0, dropped: 0, measured: false }));
  if (!t) return;
  t.count++; t.sum += ms; if (ms > t.max) t.max = ms; if (measured) t.measured = true;
  if (t.ring.length < 256) t.ring.push(ms); else { t.ring[t.at] = ms; t.at = (t.at + 1) % 256; t.dropped++; }
};
const INERT = { end() {} };
/** One scope's diagnostics: `module`, or `element <word>` for a node's. Its
 * lines and names carry the scope; `spans` holds a node's open spans, which
 * its end closes as abandoned. */
function diagnostics(scope, spans) {
  return {
    log(text) {
      if (!dev()) return;
      const now = clock.now;
      let b = D.buckets.get(scope);
      if (!b) D.buckets.set(scope, b = { from: now, lines: 0, more: 0 });
      if (now - b.from >= 1000 || now < b.from) {
        if (b.more) say(`${scope}: … ${b.more} more`);
        b.from = now; b.lines = 0; b.more = 0;
      }
      if (b.lines >= 20) { b.more++; D.limited++; return; }
      b.lines++;
      say(`${scope}: ${cut(String(text), 256)}`);
    },
    count(name, by = 1) {
      if (!dev() || !named(scope, name)) return;
      if (!Number.isSafeInteger(by) || by < 0) { D.rejected++; return; }
      const c = slot(D.counters, 64, scope, name, () => ({ n: 0 }));
      if (c) c.n += by;
    },
    /** One sample of a timing the hatch measured itself (wall time, perhaps). */
    measure(name, ms) { if (dev() && named(scope, name)) sample(scope, name, Number(ms), true); },
    /** A span on the session clock, so two identical drives time it alike. */
    begin(name) {
      if (!dev() || !named(scope, name)) return INERT;
      if (D.open >= 32) { D.rejected++; return INERT; }
      D.open++;
      const from = clock.now;
      const span = { end() { if (span.done) return; span.done = true; spans?.delete(span); D.open--; sample(scope, name, clock.now - from, false); } };
      spans?.add(span);
      return span;
    },
    /** Ask for Save Trace (LLP 1079 D5): at most one a second of wall time; a
     * sooner ask joins the pending one. Under the agent's clock no frame is
     * sampled, so there is no trace to save. */
    saveTrace() {
      if (!dev() || D.trace) return;
      const save = () => { D.trace = null; D.traced = performance.now(); globalThis.exact?.frames?.save?.(); };
      const wait = D.traced == null ? 0 : 1000 - (performance.now() - D.traced);
      if (wait > 0) D.trace = setTimeout(save, wait); else save();
    },
    /** A snapshot, JSON, the latest kept. One that does not fit is refused whole. */
    publish(name, value) {
      if (!dev() || !named(scope, name)) return;
      let json;
      try { json = JSON.stringify(value); } catch { json = undefined; }
      if (json === undefined || enc.encode(json).length > 4096) { D.rejected++; return; }
      const v = slot(D.snapshots, 16, scope, name, () => ({ json: "null" }));
      if (v) v.json = json;
    },
  };
}
/** A node's open spans when it ends: closed as abandoned, counted, not timed. */
const abandon = spans => { for (const span of spans) { span.done = true; D.open--; D.abandoned++; } spans.clear(); };

// A reply of at most 64 KB: the longest list is halved until it fits.
const fit = (reply, lists) => {
  while (JSON.stringify(reply).length > 65536) {
    let longest = null, size = 0;
    for (const [holder, key] of lists) {
      const n = JSON.stringify(holder[key] ?? null).length;
      if (n > size && (Array.isArray(holder[key]) ? holder[key].length : Object.keys(holder[key] ?? {}).length) > 0) { longest = [holder, key]; size = n; }
    }
    if (!longest) break;
    const [holder, key] = longest, v = holder[key];
    if (Array.isArray(v)) v.length = Math.floor(v.length / 2);
    else for (const k of Object.keys(v).slice(Math.floor(Object.keys(v).length / 2))) delete v[k];
    reply.truncated = true;
  }
  return reply;
};
// What each scope's hatch code recorded, by scope then name.
const grouped = (map, value) => {
  const out = {};
  for (const [key, v] of map) { const [scope, name] = key.split("\n"); (out[scope] ??= {})[name] = value(v); }
  return out;
};

/** `state.hatches` (§3.3): the words with their nodes and calls, the other
 * scopes with theirs, and what hatch code counted and published. A read
 * changes nothing. */
function state() {
  const counters = grouped(D.counters, c => c.n), published = grouped(D.snapshots, v => JSON.parse(v.json));
  const words = {}, others = {};
  const add = (entry, scope) => ({ ...entry, ...(counters[scope] ? { counters: counters[scope] } : {}), ...(published[scope] ? { published: published[scope] } : {}) });
  for (const word of Object.keys(stats)) words[word] = add(stats[word], `element ${word}`);
  for (const scope of new Set([...Object.keys(scopes), ...Object.keys(counters), ...Object.keys(published)])) {
    if (scope.startsWith("element ")) { const word = scope.slice(8); words[word] ??= add(counted(word), scope); continue; }
    others[scope] = add(scopes[scope] ?? {}, scope);
  }
  const reply = { words, scopes: others, ...(globalThis.exact?.hatchWords ? { platform: globalThis.exact.hatchWords } : {}),
    unhandled: [...Unhandled].map(([word, reason]) => ({ word, reason })), refused: A.refused, inFlight: Acts.length,
    measuring: dev(), rejected: D.rejected, abandoned: D.abandoned, limited: D.limited };
  return fit(reply, [[reply, "words"], [reply, "scopes"], [reply, "unhandled"]]);
}

const quantile = (sorted, q) => sorted.length ? sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))] : 0;
/** `perf hatches` (§3.3), a form of `perf`: every call Exact timed, by word
 * and by site and moment, and the hatches' own counters and timings. All of
 * it is cumulative; a difference is two reads. */
function perfReply(tags) {
  const calls = [...Timing.values()].map(w => ({ ...w })), by = {};
  for (const w of calls) { const t = by[w.hatch] ??= { calls: 0, ms: 0, worst: 0 }; t.calls += w.calls; t.ms += w.ms; if (w.worst > t.worst) t.worst = w.worst; }
  const timings = grouped(D.timings, t => {
    const sorted = [...t.ring].sort((a, b) => a - b);
    return { count: t.count, sum: t.sum, max: t.max, p50: quantile(sorted, 0.5), p95: quantile(sorted, 0.95), samples: sorted.length, dropped: t.dropped, ...(t.measured ? { measured: true } : {}) };
  });
  const reply = { ...tags, seq: clock.epoch, plan: globalThis.exact?.plan ?? null, measuring: dev(), hatches: by, calls, tickets: 0,
    counters: grouped(D.counters, c => c.n), timings, rejected: D.rejected, abandoned: D.abandoned, limited: D.limited };
  return fit(reply, [[reply, "calls"], [reply, "hatches"], [reply, "counters"], [reply, "timings"]]);
}
/** A late frame's `hatches` (§3.1): the calls that overlapped its window
 * `(from, to]` on `performance.now()`'s clock, each charged its overlap;
 * `coverage: partial` when the window reaches past the oldest call kept.
 * Null when none did. A join by window, not a causal trace. */
function perfWindow(from, to) {
  const by = new Map();
  let oldest = Infinity;
  for (const [t0, t1, hatch] of Calls) {
    if (t0 < oldest) oldest = t0;
    if (!(t1 > from && t0 <= to)) continue;
    const w = by.get(hatch) ?? { hatch, calls: 0, ms: 0 };
    w.calls++; w.ms += Math.max(0, Math.min(t1, to) - Math.max(t0, from));
    by.set(hatch, w);
  }
  if (!by.size) return null;
  return { hatches: [...by.values()].map(w => ({ ...w, ms: Math.round(w.ms * 100) / 100 })), ...(Calls.length === 1024 && oldest > from ? { coverage: "partial" } : {}) };
}
/** `perf <target>`'s row for a hatched site: its hatch's calls and time. */
function perfSite(site) {
  let calls = 0, ms = 0;
  for (const w of Timing.values()) if (w.site === site) { calls += w.calls; ms += w.ms; }
  return calls ? { hatch: { calls, ms } } : null;
}

// What a hatch asks of an authored element (@ref LLP 1075.003.000.001 §2.5):
// `click`, `focus`, `blur`, and `input(text)`, which replaces a field's whole
// value. Each is queued, never run inside the hatch: one FIFO a page, at
// most 256 acts and 1 MB of text, an act past either refused by name and
// counted. A drain runs the acts queued when it starts, at most 64: the
// first in a microtask, as on Apple's next turn, and each later one in a
// task, so a hatch that keeps asking cannot starve the page. An act is in
// flight (`clock settle` waits for it) from its asking until it has run.
const Acts = [], INPUTS = new Set(["text", "search", "url", "tel", "password", "email", "number"]);
const A = { bytes: 0, refused: 0, drains: 0, scheduled: false, tasks: false, port: null, composing: null };
const refuse = (what, why) => { A.refused++; say(`${what}: ${why}`); };
function ask(h, what, kind, text) {
  if (!h.isLive) return;
  const size = kind === "input" ? enc.encode(text = String(text)).length : 0;
  if (size > 65536) return refuse(what, `input refused: ${size} bytes is over 64 KB`);
  if (Acts.length >= 256 || A.bytes + size > 1 << 20) return refuse(what, `${kind} refused: the act queue is full`);
  Acts.push({ h, what, kind, text, size });
  A.bytes += size;
  inflight.n++;
  // While later drains are tasks, one is already on its way.
  if (A.scheduled || A.tasks) return;
  A.scheduled = true;
  queueMicrotask(drain);
}
function drain() {
  A.scheduled = false;
  A.drains++;
  for (const act of Acts.splice(0, 64)) {
    A.bytes -= act.size;
    try { run(act); } catch (error) { say(`${act.what}: ${act.kind} threw ${error?.message ?? error}`); } finally { inflight.n--; }
  }
  // What these acts queued (a `changed` that clicks again) waits for a task;
  // a task that finds nothing queued gives the next act its microtask back.
  if (typeof MessageChannel !== "function") { A.tasks = false; if (Acts.length && !A.scheduled) { A.scheduled = true; setTimeout(drain); } return; }
  A.tasks = true;
  if (!A.port) { const channel = new MessageChannel(); A.port = channel.port2; channel.port1.onmessage = () => { if (Acts.length) drain(); else A.tasks = false; }; channel.port1.unref?.(); }
  A.port.postMessage(0);
}
function run({ h, what, kind, text }) {
  const e = h.element;
  if (!h.isLive || !e) return say(`${what}: ${kind}() after its end does nothing`);
  if (kind !== "input") { say(`${what}: ${kind} (delivery: hatch)`); return e[kind](); }
  // `input` (§2.5): an authored text field's whole value, through the DOM's
  // own primitive, without moving focus. Journaled by length, never by text.
  const field = e.localName === "textarea" || e.localName === "input" && INPUTS.has(e.type);
  if (!field) return refuse(what, `input refused: ${e.isContentEditable ? "a contenteditable editor" : "not an editable text field"}`);
  if (e.disabled || e.readOnly) return refuse(what, `input refused: the field is ${e.disabled ? "disabled" : "readonly"}`);
  if (A.composing === e) return refuse(what, "input refused: the field is composing");
  const max = e.maxLength >= 0 ? e.maxLength : Infinity, value = [...text].length > max ? [...text].slice(0, max).join("") : text;
  // `email` and `number` have no selection API: the HTML standard has `setRangeText` throw there.
  if (e.type === "email" || e.type === "number") e.value = value; else e.setRangeText(value, 0, e.value.length, "end");
  e.dispatchEvent(new InputEvent("input", { inputType: "insertReplacementText", bubbles: true }));
  say(`${what}: input (${e.type === "password" ? "protected" : `${[...value].length} chars`}, delivery: hatch)`);
}
if (typeof document !== "undefined" && document.addEventListener) {
  document.addEventListener("compositionstart", ev => { A.composing = ev.target; }, true);
  document.addEventListener("compositionend", () => { A.composing = null; }, true);
}

// The words this platform handles, when the app's manifest gives words their
// platforms (web-js/build.mjs; LLP 1075.003.000.001 §4.3): a word it leaves
// out is shown and never called, journaled once and listed in `state`.
const Unhandled = new Map();

/** The page module's `element` hatch for `e`, and its end. */
export function ht(e) {
  if (typeof requestAnimationFrame !== "function" || globalThis.__exactRender) return;
  publish();
  const word = e.getAttribute("data-hatch"), id = viewId(e);
  const handled = globalThis.exact?.hatchWords;
  if (handled && !handled.includes(word)) {
    if (!Unhandled.has(word)) { Unhandled.set(word, "plan"); say(`element ${word}: not handled on this platform (app.json \`hatches\`); its nodes are shown and never called`); }
    return;
  }
  // What a hatch asks of an element is queued (`ask`): it runs after the
  // effect or commit that called the hatch, never inside it, as on Apple.
  const what = `element ${word} #${id}`;
  const spans = new Set(), site = e.dataset.site != null ? Number(e.dataset.site) : null;
  const h = {
    hatch: word, element: e, data: e.dataset, isNew: true, isLive: true,
    click() { ask(h, what, "click"); }, focus() { ask(h, what, "focus"); }, blur() { ask(h, what, "blur"); }, input(text) { ask(h, what, "input", text); },
    diagnostics: diagnostics(`element ${word}`, spans),
  };
  // A throw is caught and journaled (LLP 1075.003.000.001 §4.4): this node's
  // built and changed are not called again; its end still is, so what it
  // added before throwing is still taken back.
  let module = null, inRow = false, threw = false;
  const call = (name, moment) => {
    if (typeof module?.[name] !== "function" || (threw && name === "element")) return;
    const n = counted(word).calls[moment] = (counted(word).calls[moment] ?? 0) + 1;
    if (!inRow || n === 1) say(`element ${word} #${id}: ${moment}`);
    try { timed(`element ${word}`, site, moment, () => module[name](h)); } catch (error) {
      if (name === "element") threw = true;
      say(`element ${word} #${id}: ${name} threw ${error?.message ?? error}${name === "element" ? "; this node's hatch is not called again, its end still is" : ""}`);
    }
  };
  inflight.n++;
  page().then(m => {
    if (!h.isLive) return;
    module = m;
    publish();
    counted(word).live++;
    if (!said.has(word)) { said.add(word); say(`element ${word}: nothing beyond the call on this host (LLP 1075.003.000)`); }
    const row = e.closest("[data-listitemkey]");
    inRow = !!row;
    if (row && !warned.has(word)) {
      warned.add(word);
      let list = row.parentElement;
      while (list && !list.$list) list = list.parentElement;
      say(`element ${word} is in a row of list ${list?.dataset.testid ?? "#" + (list ? viewId(list) : "?")}: each row's mount calls its hatch on the main thread`);
    }
    // A change of its words (dataset.js) is heard after the effect that made
    // it, once however many words changed: what the hatch does in answer (a
    // click) is then an ordinary update, not one inside the running effect.
    // Installed before `built`, so what `built` itself causes is heard too.
    let due = false;
    e.$ht = () => {
      if (due) return;
      due = true;
      queueMicrotask(() => { due = false; if (h.isLive) { h.isNew = false; call("element", "changed"); } });
    };
    call("element", "built");
  }, error => say(`element ${word} #${id}: no page module: ${error?.message ?? error}`))
    .finally(() => setTimeout(() => inflight.n--));
  onEnd(() => {
    delete e.$ht;
    const told = module && h.isLive;
    // As on Apple: the handle is no longer live in `elementEnded` (its
    // `click()` does nothing), and keeps no element after it, since a list
    // may give the element to another row.
    h.isLive = false;
    if (told) { counted(word).live--; call("elementEnded", "ended"); }
    abandon(spans);
    h.data = { ...e.dataset };
    h.element = null;
  });
}

// The page module's container hatches (LLP 1075.003.000 §3.7), loaded after
// first paint only for a page module that exports one (web-js/build.mjs):
//
//   export function navigation(e)  // a navigation root's element, once
//   export function route(r)       // a route's element, when it mounts:
//                                  // { key, data, element, navigation, isNew, isLive }
//   export function routeEnded(r)  // the route's element is leaving
//   export function tabs(e)        // the root's tablist's element, once
//
// Each runs as `navigation.js` projects a router change.
const roots = new WeakSet(), lists = new WeakSet(), routes = new Map();
export function containers() {
  publish();
  return page().then(m => {
    const project = root => {
      if (!root) return;
      const live = new Set();
      const call = (name, what, arg) => {
        if (typeof m[name] !== "function") return;
        say(`${what}`);
        const scope = name === "routeEnded" ? "route" : name, moment = name === "routeEnded" ? "ended" : "built";
        const calls = (scopes[scope] ??= { calls: Object.create(null) }).calls;
        calls[moment] = (calls[moment] ?? 0) + 1;
        try { timed(scope, null, moment, () => m[name](arg)); } catch (error) { say(`${name} threw ${error?.message ?? error}`); }
      };
      for (const nav of root.querySelectorAll("[navigationBack]")) {
        if (!roots.has(nav)) { roots.add(nav); call("navigation", `navigation #${viewId(nav)}: built`, nav); }
        for (const list of nav.querySelectorAll('[role="tablist"]')) {
          if (lists.has(list) || list.parentElement?.closest("[navigationKey]") !== nav) continue;
          lists.add(list);
          call("tabs", `tabs: built`, list);
        }
        for (const element of nav.querySelectorAll("[navigationKey]")) {
          // A route is its nearest root's; a nested root is not a route; one
          // leaving (its exit animation playing) has ended.
          if (element.hasAttribute("navigationBack") || element.parentElement?.closest("[navigationBack]") !== nav
            || element.closest("[data-exiting]")) continue;
          live.add(element);
          if (routes.has(element)) continue;
          const r = { key: element.getAttribute("navigationKey"), data: element.dataset, element, navigation: nav, isNew: true, isLive: true };
          routes.set(element, r);
          call("route", `route ${r.key}: built`, r);
        }
      }
      for (const [element, r] of routes) {
        if (live.has(element) && element.isConnected) continue;
        routes.delete(element);
        call("routeEnded", `route ${r.key}: ended`, r);
        r.isLive = false;
      }
    };
    globalThis.exact.onProject = project;
    project(document.getElementById("exact-root"));
  }, error => say(`no page module: ${error?.message ?? error}`));
}
