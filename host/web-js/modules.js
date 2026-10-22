// A native module's sources and sends on the JS target (LLP 1133.002 D13,
// §6; LLP 1124 D7, D9), substituted as the runner's `modules.rs` does
// natively, by what each name's `exact.json` declares: a permission whose
// state is `prompt` until `prefer permission.<name>` or a ticket sets it;
// a source's preferred value (`prefer extension`) or its default; a send
// answered from its `from` source's preferred value, else held under its
// `ticket` for the agent (`type @t …`). A held request is a promise the
// agent's answer settles. Imported by an app's module only when its plan
// names a module.
//
// A send that answers nothing (type `u`) is a host command of the source's
// name, run once its commit stands (LLP 1133.004 D5), and its declared list,
// value and admission answer under the agent (§7). An event the root hears
// is raised by the host (an activation the implementation binds, `isTrusted`
// only) or emitted (the device's origin), and heard in order, once, each as
// its own commit (D6).
import { data, Resources, commit, R, Failed, nextTicket, say, clock, Clocked, After, Hosts, navigateRoot, locationOf, inflight, Origin } from "./rt.js";
import { reportTime } from "./navigation.js";

// `rows`: source → [extension.name, send, substitute, type, request, options' type, live];
// `bound`: extension → a loader of its web implementation, where the app binds one
// (LLP 1133.001 D4), which answers outside the agent; `events`: extension.event →
// [its action, taken when heard; activation | emit; hold; its parameters' types; its name].
export function modules(rows, bound = {}, events = {}) {
  const byKey = new Map(), preferred = new Map(), permission = new Map(), held = [];
  for (const [source, [key, send, sub, type, request, options, live]] of Object.entries(rows)) byKey.set(key, { source, key, send, sub, type, request, options, live });
  // The origin of the commit running now (LLP 1133.002 D12): the device's while an emitted event's handler runs, and,
  // since data code and an answer's commit cannot say which request they follow, while any request such a handler
  // made is still in flight (`deviceBase`, the in-flight count before it): the least privileged origin.
  let origin = "person", deviceBase = null;
  const originNow = () => origin === "device" || deviceBase !== null ? "device" : "person";
  Origin.now = originNow;
  After.push(() => { if (deviceBase !== null && inflight.n <= deviceBase) deviceBase = null; });
  const asks = (row, send) => row.request === "always" ? send : row.request === "when-prompt" ? !send : false;
  const personRequired = (row, send) => {
    const p = row.sub.permission;
    if (originNow() === "person" || !p || row.sub.admission === "none" || stateOf(p.name) !== "prompt" || !asks(row, send)) return false;
    say(`${row.key.split(".")[1]}: refused: ${p.denied}, reason person-required`);
    return true;
  };
  // A request's own timeout, the option its declaration's `timeout` names: from a Contract call's record, or data code's object.
  const timeoutOf = (row, args) => {
    const name = row.sub.timeout, at = name && row.options ? Object.keys(row.options).indexOf(name) : -1;
    const v = Array.isArray(args?.[0]) ? args[0][at] : args?.[name];
    return typeof v === "number" && v >= 0 ? v : null;
  };
  // A value ticket that would wait `timeout=0` fails `TIMEOUT` at once (D2).
  const decideTimed = (row, send, timeout) => { const d = decide(row, send); return d.hold === "value" && timeout === 0 ? { now: { error: "TIMEOUT" } } : d; };
  // Value tickets whose timeout has passed on the agent's clock fail `TIMEOUT`, retired (§6).
  Clocked.push(() => { for (const h of held.filter(h => h.deadline != null && h.deadline <= clock.now)) { say(`device position ${h.ticket}: timed out`); release(h, { error: "TIMEOUT" }); } });
  const sibling = (row, declared) => declared && byKey.get(`${row.key.slice(0, row.key.indexOf("."))}.${declared}`);
  const isStatus = row => row.sub.permission?.status === row.key.slice(row.key.indexOf(".") + 1);
  const stateOf = name => permission.get(name) ?? "prompt";
  const now = () => reportTime(clock.now)[0] + clock.now; // the agent clock's date, as `exactTime` reads it
  const filled = (row, v) => {
    if (errorCode(v)) return v;
    const out = structuredClone(v);
    for (const [path, filler] of Object.entries(row.sub.fill ?? {})) fill(out, path, filler === "now" ? now() : filler);
    return out;
  };
  const prefers = row => preferred.has(row.key) ? filled(row, preferred.get(row.key)) : undefined;
  const valueOf = row => prefers(row) ?? row.sub.default;
  const decide = (row, send) => {
    // A target's own restriction holds under the agent too (§6): the web has no precision request.
    if (row.sub.unsupported?.includes("web")) return { now: { error: "NotSupportedError" } };
    const p = row.sub.permission;
    if (p && row.sub.admission !== "none") {
      const state = stateOf(p.name);
      if (!send && isStatus(row)) return { now: { ...(valueOf(row) ?? {}), [p.field]: state } };
      if (state === "denied" && row.sub.admission !== "request") return { now: { error: p.denied } };
      if (state === "prompt" && asks(row, send)) return originNow() === "person" ? { hold: "permission" } : { now: { error: p.denied } };
    }
    if (!send) return { now: valueOf(row) ?? { error: "NotSupportedError" } };
    // A send that answers another source's value, raising it when it matches `raise.when` (D10).
    const answers = sibling(row, row.sub.answers);
    if (answers) {
      const value = decide(answers, false).now ?? {};
      const when = row.sub.raise?.when;
      return when && Object.entries(when).every(([k, v]) => value[k] === v) ? { hold: "raise" } : { now: value };
    }
    const from = sibling(row, row.sub.from), v = from && prefers(from);
    if (v !== undefined) return { now: v };
    if (row.sub.ticket && from) return { hold: "value" };
    return { now: { error: "NotSupportedError" } };
  };
  const answer = (row, json) => {
    const code = errorCode(json);
    if (code) throw new Failed(`${row.key}: ${code}`, "error", code);
    return value(json, row.type, row.key);
  };
  const capability = h => h.waits === "permission" ? "permission" : h.waits === "raise" ? h.row.sub.raise.ticket : h.row.sub.ticket;
  const hold = (row, waits, name, json = false, timeout = null) => {
    // A newer ask of the same reader or mutation supersedes its held one (§6: retired).
    for (const h of held.filter(h => h.name === name)) { retire(h); h.reject(new Failed(`${row.key}: superseded`)); }
    return new Promise((resolve, reject) => {
      const h = { ticket: nextTicket(), row, waits, name, send: row.send, resolve, reject, json, timeout, deadline: waits === "value" && timeout != null ? clock.now + timeout : null };
      held.push(h);
      say(`device ${capability(h)} ${h.ticket} held (agent)`);
    });
  };
  const retire = h => { const i = held.indexOf(h); if (i >= 0) held.splice(i, 1); };
  const release = (h, json) => { retire(h); if (h.json) return h.resolve(complete(json, h.row.type)); try { h.resolve(answer(h.row, json)); } catch (e) { h.reject(e); } };
  const reask = keys => {
    commit(() => { const sources = new Set([...keys].map(k => byKey.get(k).source)); for (const r of Resources) if (sources.has(r.source)) R(r); }, "module");
    for (const key of keys) globalThis.exact?.announceModule?.(key); // data code's readers
  };
  // Outside the agent the bound implementation answers: loaded on first use (LLP 1133.002 D15).
  const loaded = new Map(), live = new Set();
  const implementation = row => {
    const extension = row.key.slice(0, row.key.indexOf("."));
    if (clock.agent || !bound[extension]) return null;
    if (!loaded.has(extension)) loaded.set(extension, bound[extension]().then(m => m.implementation?.({
      changed: key => byKey.has(key) && reask([key]),
      // A package's own raising has the device's origin; a person's is the host's to give (D6).
      emit: (name, args) => raise(`${extension}.${name}`, args, "device"),
      // Only an event that is the web's user activation (HTML's activation-triggering input events, and `click`), and
      // trusted, is a person's; any other type binds nothing.
      activation: (target, type, name, args, { navigate } = {}) => !ACTIVATING.has(type) ? say(`${extension}.${name}: a ${type} is no person's activation; nothing is bound`) : target.addEventListener(type, e => {
        if (!e.isTrusted) return say(`${extension}.${name}: an untrusted ${type} raises nothing`);
        try { globalThis.focus?.(); } catch {}
        if (navigate) navigateRoot(locationOf(navigate)); else raise(`${extension}.${name}`, args, "person");
      }),
    }) ?? null));
    return loaded.get(extension);
  };
  const viaImplementation = (row, args, from = originNow()) => implementation(row)?.then(async impl => {
    const name = row.key.slice(row.key.indexOf(".") + 1);
    if (!impl?.[name]) return { error: "NotSupportedError" };
    // A device-origin call that would ask is refused while the platform's state is `prompt` (LLP 1133.002 D12).
    const p = row.sub.permission;
    if (from !== "person" && p?.status && row.sub.admission !== "none" && asks(row, row.send) && impl[p.status]) {
      const status = await impl[p.status]({});
      if (status?.[p.field] === "prompt") { say(`${name}: refused: ${p.denied}, reason person-required`); return { error: p.denied }; }
    }
    if (row.live) live.add(row.key);
    return impl[name](args ?? {});
  });
  // ---- a send that answers nothing, and its declared substitute (LLP 1133.004 D5, §7)
  const lists = new Map(), values = new Map(), effects = [];
  const listOf = row => [...byKey.values()].filter(r => r.key.split(".")[0] === row.key.split(".")[0] && r.sub.list).map(r => r.sub.list.state);
  const applyEffect = (row, args) => {
    const name = row.key.split(".")[1], sub = row.sub;
    if (sub.list) {
      const entries = lists.get(sub.list.state) ?? [], value = args[sub.list.key] ?? null, ref = nextTicket();
      const kept = value == null ? entries : entries.filter(e => e.fields[sub.list.key] !== value);
      kept.push({ ref, row, fields: { ...Object.fromEntries(Object.keys(row.options ?? {}).map(k => [k, null])), ...args } });
      lists.set(sub.list.state, kept);
      say(`${name}: listed${typeof value === "string" ? ` (${value})` : ""} @${ref}`);
    }
    if (sub.unlist) {
      for (const state of listOf(row)) lists.set(state, (lists.get(state) ?? []).filter(e => e.fields[sub.unlist] !== args[sub.unlist]));
      say(`${name}: unlisted`);
    }
    if (sub.set) {
      const from = sub.set.from, given = from ? args[from] : undefined;
      const value = !from ? (sub.set.value ?? null) : given == null ? (sub.set.absent ?? null) : given === 0 ? (sub.set.zero ?? null) : given;
      values.set(`${row.key.split(".")[0]}/${sub.set.state}`, value);
      say(`${name}: ${sub.set.state} ${JSON.stringify(value)}`);
    }
  };
  /** One call: done (`null`), refused (`{error}`), or held on the permission ticket until it is answered. */
  const effect = (row, args, from = originNow()) => {
    const before = origin;
    if (from === "device") origin = "device";
    try { return effectNow(row, args, from); } finally { origin = before; }
  };
  const effectNow = (row, args, from) => {
    const name = row.key.split(".")[1];
    const real = viaImplementation(row, args);
    if (real) return real.then(r => { if (errorCode(r)) say(`${name}: refused: ${errorCode(r)}`); return errorCode(r) ? r : null; });
    const p = row.sub.permission, state = p && row.sub.admission !== "none" ? stateOf(p.name) : null;
    if (state === "denied") { say(`${name}: refused: ${p.denied}`); return Promise.resolve({ error: p.denied }); }
    if (state === "prompt" && asks(row, true)) {
      if (personRequired(row, true)) return Promise.resolve({ error: p.denied });
      return new Promise(resolve => { const h = { ticket: nextTicket(), row, args, resolve }; effects.push(h); say(`device permission ${h.ticket} held (agent)`); });
    }
    applyEffect(row, args);
    return Promise.resolve(null);
  };
  const settleEffects = (name, state) => {
    for (const h of effects.filter(h => h.row.sub.permission?.name === name)) {
      if (state === "prompt") continue;
      effects.splice(effects.indexOf(h), 1);
      if (state === "granted") { applyEffect(h.row, h.args); h.resolve(null); }
      else { say(`${h.row.key.split(".")[1]}: refused: ${h.row.sub.permission.denied}`); h.resolve({ error: h.row.sub.permission.denied }); }
    }
  };
  for (const row of byKey.values()) if (row.send && row.type === "u") Hosts[row.source] = Object.assign((from, record) => { effect(row, object(record, row.options) ?? {}, from); }, { origin: true });
  const delivered = e => { const at = e.row.sub.list.at, t = at ? e.fields[at] : null; return t == null || t <= now(); };
  // ---- events (LLP 1133.004 D6)
  const queued = [];
  const raise = (key, args, from) => {
    const declared = events[key];
    if (!declared) return say(`${key}: no handler hears it`);
    const [, raised, bound] = declared;
    queued.push({ key, args: Array.isArray(args) ? args : [], origin: raised === "activation" ? from : "device" });
    // Each event's own bound: past it, its own oldest is dropped.
    while (queued.filter(q => q.key === key).length > bound) { const old = queued.splice(queued.findIndex(q => q.key === key), 1)[0]; say(`${key}: dropped (${typeof old.args[0] === "string" ? old.args[0] : JSON.stringify(old.args[0] ?? null)})`); }
    say(`${key}: raised`);
    queueMicrotask(hear);
  };
  const hear = () => {
    while (queued.length) {
      const { key, args, origin: from } = queued.shift(), [action, , , types, name] = events[key];
      let values;
      try { values = types.map((t, i) => value(args[i] ?? null, t, key)); } catch (e) { say(`${key}: refused: ${e.message}`); continue; }
      say(`${key}: heard by ${name}`);
      const before = origin;
      origin = from;
      const base = inflight.n;
      try { action()(...values); } finally { origin = before; }
      if (from === "device" && inflight.n > base) deviceBase = deviceBase === null ? base : Math.min(deviceBase, base);
    }
  };
  /** An entry's click (§7): its `navigate` field opened, else its event raised as an activation. */
  const activate = (row, entry, ref) => {
    const tap = row.sub.list.tap ?? {}, location = tap.navigate ? entry[tap.navigate] : null;
    if (typeof location === "string") { navigateRoot(locationOf(location)); return { ref, navigate: location, delivery: "substituted" }; }
    if (!tap.event) return { ref, delivery: "substituted" };
    const args = (tap.args ?? []).map(a => typeof a === "string" ? entry[a] ?? null : a && typeof a === "object" ? Object.fromEntries(Object.entries(a).map(([k, f]) => [k, entry[f] ?? null])) : a);
    raise(`${row.key.split(".")[0]}.${tap.event}`, args, "person");
    return { ref, event: tap.event, delivery: "substituted" };
  };
  // The last reader of a live source gone, its acquisition stops (D5).
  After.push(() => {
    for (const key of [...live]) {
      const row = byKey.get(key);
      if (Resources.some(r => r.source === row.source) || (globalThis.exact?.moduleReaders?.(key) ?? []).length) continue;
      live.delete(key);
      implementation(row)?.then(impl => impl?.release?.(key.slice(key.indexOf(".") + 1)));
    }
  });
  const reserved = (data.reserved ??= {});
  for (const row of byKey.values()) {
    reserved[row.source] = (_, args, name) => {
      const real = viaImplementation(row, args?.[0] !== undefined ? object(args[0], row.options) : {});
      if (real) return real.then(json => answer(row, json));
      const timeout = row.send ? timeoutOf(row, args) : null, d = decideTimed(row, row.send, timeout);
      if (d.hold) return hold(row, d.hold, name, false, timeout);
      return answer(row, d.now);
    };
  }
  // `allow` sets the status fields the app's grant line names (its approximate grant reads `approximate`).
  const allow = name => {
    const grants = (data.grants ?? "").split("\n").map(l => l.trim().split(/\s+/)[0]);
    for (const row of byKey.values()) {
      if (row.sub.permission?.name !== name || !isStatus(row)) continue;
      const status = { ...(valueOf(row) ?? {}) };
      delete status[row.sub.permission.field]; // the state is the permission's
      for (const [field, byLine] of Object.entries(row.sub.permission.allow ?? {})) {
        const line = Object.keys(byLine).filter(l => grants.includes(l)).sort((a, b) => b.length - a.length)[0];
        if (line) status[field] = byLine[line];
      }
      preferred.set(row.key, status);
    }
  };
  const setPermission = (name, state) => {
    permission.set(name, state);
    say(`permission ${name}: ${state}`);
    if (state === "granted") allow(name);
    for (const h of [...held]) {
      if (h.row.sub.permission?.name !== name) continue;
      if (state === "denied") { release(h, { error: h.row.sub.permission.denied }); continue; }
      if (state !== "granted" || h.waits !== "permission") continue;
      const d = decideTimed(h.row, h.send, h.timeout);
      if (d.hold) { h.waits = d.hold; h.ticket = nextTicket(); h.deadline = d.hold === "value" && h.timeout != null ? clock.now + h.timeout : null; say(`device ${capability(h)} ${h.ticket} held (agent)`); } else release(h, d.now);
    }
    settleEffects(name, state);
    reask([...byKey.values()].filter(r => r.sub.permission?.name === name).map(r => r.key));
  };
  const settleValues = () => {
    for (const h of held.filter(h => h.waits === "value")) {
      const from = sibling(h.row, h.row.sub.from), v = from && prefers(from);
      if (v !== undefined) release(h, v);
    }
  };
  // The agent's value: JSON, or the `short` form, numbers by comma into the declared paths.
  const typed = (row, text) => {
    let v;
    try { v = JSON.parse(text); } catch {}
    if (v === null || typeof v !== "object" || Array.isArray(v)) {
      const short = row.sub.short ?? [], parts = String(text).split(","), numbers = parts.map(Number);
      if (!short.length || numbers.length > short.length || numbers.some(n => !isFinite(n) || parts.some(p => !p.trim()))) {
        throw new Failed(`${row.key}: type its JSON${short.length ? `, or ${short.join(",")}` : ""}`);
      }
      v = {};
      short.forEach((path, i) => { if (i < numbers.length) fill(v, path, numbers[i]); });
    }
    value(filled(row, v), row.type, row.key);
    return v;
  };
  // A data-code call's reader, for `state.extensions`: the resource whose answer read it.
  const readers = new Map(); // key -> Set of resource names
  (globalThis.exact ??= {}).extensions = {
    // `state.extensions` (LLP 1133.002 §6), as the runner's `extensions_json` writes it.
    state() {
      const out = {};
      for (const row of byKey.values()) {
        const [extension, declared] = [row.key.slice(0, row.key.indexOf(".")), row.key.slice(row.key.indexOf(".") + 1)];
        const e = out[extension] ??= { bound: !clock.agent && !!bound[extension], preferred: {}, readers: {}, held: [] };
        if (row.sub.permission) e.permission = stateOf(row.sub.permission.name);
        if (preferred.has(row.key)) e.preferred[declared] = preferred.get(row.key);
        if (!row.send) e.readers[declared] = [...new Set([...Resources.filter(r => r.source === row.source || r.watches?.includes(row.key)).map(r => r.name), ...(globalThis.exact?.moduleReaders?.(row.key) ?? [])])];
        for (const h of held.filter(h => h.row === row)) e.held.push({ ticket: h.ticket, name: declared, waits: h.waits });
        for (const h of effects.filter(h => h.row === row)) e.held.push({ ticket: h.ticket, name: declared, waits: "permission" });
      }
      for (const q of queued) {
        const extension = q.key.split(".")[0], e = out[extension] ??= { bound: false, preferred: {}, readers: {}, held: [] };
        (e.queued ??= []).push({ event: q.key.split(".")[1], args: q.args, origin: q.origin });
      }
      return out;
    },
    // A package's emit, as its context raises it: the device's (the tests' way in, the agent binding no implementation).
    raiseForTests: (key, args = []) => raise(key, args, "device"),
    // Each declared list and value, by its state name (§7), for the agent's top-level state.
    declared() {
      const out = {};
      for (const row of byKey.values()) {
        if (row.sub.list) out[row.sub.list.state] = (lists.get(row.sub.list.state) ?? []).map(e => ({ ...e.fields, delivered: delivered(e), ref: e.ref }));
        if (row.sub.set) out[row.sub.set.state] = values.get(`${row.key.split(".")[0]}/${row.sub.set.state}`) ?? null;
      }
      return out;
    },
    // A relaunched page's launch flag (`--notification '<entry>'`): the entry as its tap would deliver it.
    launch(req) {
      const row = [...byKey.values()].find(r => r.sub.list?.launch === req.launch);
      if (!row) return { error: `--${req.launch}: no module of this app lists entries a launch hands over` };
      let entry;
      try { entry = typeof req.entry === "string" ? JSON.parse(req.entry) : req.entry; } catch (e) { return { error: `--${req.launch}: not JSON: ${e.message}` }; }
      say(`launch --${req.launch}`);
      return activate(row, entry ?? {}, 0);
    },
    // A call from data code (ts-data.js `__exact_module`): JSON, as the native runtime answers it.
    call(body) {
      const row = byKey.get(`${body.extension}.${body.source ?? body.send}`);
      if (!row || row.send !== !!body.send) return Promise.resolve({ error: "NotSupportedError" });
      if (row.type === "u") return effect(row, body.args ?? {});
      const real = viaImplementation(row, body.args);
      if (real) return real.then(json => complete(json, row.type));
      const timeout = row.send ? timeoutOf(row, body.args) : null, d = decideTimed(row, row.send, timeout);
      if (d.hold) return hold(row, d.hold, row.key, true, timeout);
      say(`native ${row.key} (substituted)`);
      return Promise.resolve(complete(d.now, row.type));
    },
    holds: () => [...held.map(h => ({ name: h.name, ticket: h.ticket, device: { capability: capability(h), args: h.waits === "permission" ? { capability: h.row.sub.permission.name, request: h.row.key.split(".")[1], origin: "app" } : { request: h.row.key.split(".")[1] } } })),
      ...effects.map(h => ({ name: h.row.key, ticket: h.ticket, device: { capability: "permission", args: { capability: h.row.sub.permission.name, request: h.row.key.split(".")[1], origin: "app" } } }))],
    // `tap @t allow|deny|cancel`, `type @t <value>`, `tap @ref` on a listed entry; undefined for a ticket that is not a module's.
    answer(req) {
      const entry = req.op === "tap" && [...lists.values()].flat().find(e => e.ref === req.ticket);
      if (entry) {
        if (!delivered(entry)) return { error: `@${req.ticket} is not delivered yet: advance the clock past its time` };
        const state = entry.row.sub.list.state;
        lists.set(state, lists.get(state).filter(e => e !== entry));
        say(`${state} @${req.ticket}: tapped`);
        return activate(entry.row, entry.fields, req.ticket);
      }
      const e = effects.find(x => x.ticket === req.ticket);
      if (e) {
        if (req.op !== "tap" || !["allow", "deny", "cancel"].includes(req.choice)) return { error: `@${e.ticket} (permission): tap takes cancel | allow | deny` };
        say(`device permission ${e.ticket} ${req.choice === "cancel" ? "cancelled" : `answered: ${req.choice}`}`);
        if (req.choice === "cancel") { effects.splice(effects.indexOf(e), 1); say(`${e.row.key.split(".")[1]}: refused: ${e.row.sub.permission.denied}`); e.resolve({ error: e.row.sub.permission.denied }); }
        else setPermission(e.row.sub.permission.name, req.choice === "allow" ? "granted" : "denied");
        return { ticket: req.ticket, capability: "permission", answered: req.choice, delivery: "substituted" };
      }
      const h = held.find(x => x.ticket === req.ticket);
      if (!h) return undefined;
      const cap = capability(h);
      if (h.waits === "permission") {
        if (req.op !== "tap") return { error: `@${h.ticket} (permission): answered by tap, not type` };
        if (!["allow", "deny", "cancel"].includes(req.choice)) return { error: `@${h.ticket} (permission): tap takes cancel | allow | deny, not ${req.choice}` };
        say(`device permission ${h.ticket} ${req.choice === "cancel" ? "cancelled" : `answered: ${req.choice}`}`);
        if (req.choice === "allow") setPermission(h.row.sub.permission.name, "granted");
        else if (req.choice === "deny") setPermission(h.row.sub.permission.name, "denied");
        else release(h, { error: h.row.sub.permission.denied });
        return { ticket: req.ticket, capability: cap, answered: req.choice, delivery: "substituted" };
      }
      if (h.waits === "raise") {
        if (req.op !== "tap") return { error: `@${h.ticket}: answered by tap, not type` };
        if (!["allow", "deny", "cancel"].includes(req.choice)) return { error: `@${h.ticket} (${cap}): tap takes cancel | allow | deny, not ${req.choice}` };
        const answers = sibling(h.row, h.row.sub.answers);
        if (req.choice === "allow") { preferred.set(answers.key, { ...(preferred.get(answers.key) ?? {}), ...h.row.sub.raise.allow }); say(`${answers.key}: raised by the agent`); }
        // Declined or dismissed is no failure: the call answers the value as it is.
        release(h, decide(answers, false).now);
        reask([answers.key]);
        return { ticket: req.ticket, capability: cap, answered: req.choice, delivery: "substituted" };
      }
      const from = sibling(h.row, h.row.sub.from);
      if (req.op === "tap") {
        if (req.choice !== "cancel") return { error: `@${h.ticket} (${cap}): tap takes cancel, not ${req.choice}` };
        say(`device ${cap} ${h.ticket} cancelled`);
        release(h, h.row.sub.cancel ?? { error: "NotSupportedError" });
        return { ticket: req.ticket, capability: cap, answered: "cancel", delivery: "substituted" };
      }
      let v;
      try { v = typed(from, req.text); } catch (e) { return { error: `@${h.ticket}: ${e.message}` }; }
      say(`device ${cap} ${h.ticket} answered: a value`);
      preferred.set(from.key, v);
      settleValues();
      reask([from.key]);
      return { ticket: req.ticket, capability: cap, answered: "value", delivery: "substituted" };
    },
    // `prefer extension` and `prefer permission` (LLP 1124 D7, LLP 1133.002 §6).
    prefer(req) {
      for (const [name, state] of Object.entries(req.permission ?? {})) {
        if (![...byKey.values()].some(r => r.sub.permission?.name === name)) return { error: `prefer permission: no module here asks for \`${name}\`` };
        if (!["granted", "denied", "prompt"].includes(state)) return { error: `prefer permission.${name}: granted, denied or prompt` };
      }
      const group = req.extension ?? {};
      for (const [key, v] of Object.entries(group)) {
        const row = byKey.get(key);
        const sources = [...byKey.values()].filter(r => !r.send).map(r => r.key);
        if (!row) return { error: `prefer extension: no source \`${key}\` in this app; its module sources are ${sources.join(", ") || "none"}` };
        if (row.send) return { error: `prefer extension: \`${key}\` is a send, which the module answers` };
        const p = row.sub.permission, field = p && isStatus(row) ? p.field : null;
        if (field && v && typeof v === "object" && field in v) return { error: `prefer extension: \`${key}\`'s \`${field}\` is set by \`prefer permission.${p.name}\`` };
        if (v !== "clear" && !errorCode(v)) {
          try { typed(row, JSON.stringify(field ? { ...v, [field]: "prompt" } : v)); } catch (e) { return { error: `prefer extension: ${e.message}` }; }
        }
      }
      for (const [name, state] of Object.entries(req.permission ?? {})) setPermission(name, state);
      for (const [key, v] of Object.entries(group)) {
        if (v === "clear") preferred.delete(key); else preferred.set(key, v);
        say(`prefer extension ${key}`);
      }
      settleValues();
      if (Object.keys(group).length) reask(Object.keys(group));
      return { extension: Object.fromEntries(preferred), permission: Object.fromEntries(permission) };
    },
  };
}

/** HTML's activation-triggering input events, and `click`: what a person's activation is. */
const ACTIVATING = new Set(["click", "keydown", "mousedown", "pointerdown", "pointerup", "touchend"]);

/** The code of an error answer, `{"error": "POSITION_UNAVAILABLE"}`, with a `message` beside it or not. */
function errorCode(json) {
  if (json === null || typeof json !== "object" || Array.isArray(json)) return null;
  const keys = Object.keys(json).filter(k => k !== "message");
  return keys.length === 1 && typeof json.error === "string" ? json.error : null;
}

/** A Contract call's options record (its fields in order) as the object an
 * implementation reads: a `none` left out, so an omitted option stays the platform's (D2). */
function object(value, type) {
  if (value === null || value === undefined) return undefined;
  if (Array.isArray(type)) return type[0] === "?" ? object(value, type[1]) : type[0] === "[" ? value.map(v => object(v, type[1])) : value;
  if (!type || typeof type !== "object" || !Array.isArray(value)) return value;
  const out = {};
  Object.keys(type).forEach((f, i) => { const v = object(value[i], type[f]); if (v !== undefined) out[f] = v; });
  return out;
}

/** `json` with every option field it leaves out `null`, as data code reads a value of `type`. */
function complete(json, type) {
  if (errorCode(json) || json === null || typeof json !== "object") return json;
  if (Array.isArray(type)) return type[0] === "[" && Array.isArray(json) ? json.map(x => complete(x, type[1])) : type[0] === "?" ? complete(json, type[1]) : json;
  if (typeof type !== "object") return json;
  const out = { ...json };
  for (const [f, t] of Object.entries(type)) out[f] = out[f] === undefined ? (Array.isArray(t) && t[0] === "?" ? null : out[f]) : complete(out[f], t);
  return out;
}

/** Set `path` (`coords.accuracy`) in `v` to `filler` where it is absent. */
function fill(v, path, filler) {
  const parts = path.split(".");
  let at = v;
  for (let i = 0; i < parts.length; i++) {
    if (at === null || typeof at !== "object") return;
    if (i === parts.length - 1) { if (at[parts[i]] === undefined) at[parts[i]] = filler; return; }
    at = at[parts[i]] ??= {};
  }
}

/** `json` as the runtime holds a value of `type` (a record is its fields in
 * order): an option field it leaves out is none, as an omitted word is. */
function value(json, type, at) {
  if (typeof type === "string") {
    const ok = type === "n" ? typeof json === "number" && isFinite(json) : type === "b" ? typeof json === "boolean"
      : type === "s" ? typeof json === "string" : json === null;
    if (!ok) throw new Failed(`${at}: expected ${{ n: "a number", b: "a bool", s: "a string" }[type] ?? "null"}`, "shape");
    return json;
  }
  if (Array.isArray(type)) {
    if (type[0] === "?") return json == null ? null : value(json, type[1], at);
    if (type[0] === "e") {
      if (!type[1].includes(json)) throw new Failed(`${at}: ${JSON.stringify(json)} is not one of ${type[1].join(", ")}`, "shape");
      return json;
    }
    if (!Array.isArray(json)) throw new Failed(`${at}: expected an array`, "shape");
    return json.map((x, i) => value(x, type[1], `${at}[${i}]`));
  }
  if (json === null || typeof json !== "object" || Array.isArray(json)) throw new Failed(`${at}: expected an object`, "shape");
  for (const k of Object.keys(json)) if (!Object.hasOwn(type, k)) throw new Failed(`${at}: field \`${k}\` is not in the shape`, "shape");
  return Object.entries(type).map(([f, t]) => {
    if (json[f] === undefined && Array.isArray(t) && t[0] === "?") return null;
    if (json[f] === undefined) throw new Failed(`${at}: field \`${f}\` is missing`, "shape");
    return value(json[f], t, `${at}.${f}`);
  });
}
