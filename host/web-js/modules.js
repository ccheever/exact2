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
// Every call is admitted first (LLP 1133.002 D14, as the runner's
// `modules/admit.rs`): the grant lines its name or arguments list, its
// `origin` and its `foreground`. Outside the agent the bound implementation
// then answers; under it, an interface that is its own substitute
// (`implementation` in its declaration) is answered by its implementation,
// told `agent: true`, behind the permission and tickets here, with the
// agent's preferred value laid over each answer; every other by the
// substitute. The web has no tool or wake door, so every call's origin is
// `app`.
import { data, Resources, commit, R, Failed, nextTicket, say, clock, Clocked, After } from "./rt.js";
import { reportTime } from "./navigation.js";

// `rows`: source → [extension.name, send, substitute, type, request, options' type, live];
// `bound`: extension → a loader of its web implementation, where the app binds one
// (LLP 1133.001 D4), which answers outside the agent.
export function modules(rows, bound = {}) {
  const byKey = new Map(), preferred = new Map(), permission = new Map(), held = [];
  for (const [source, [key, send, sub, type, request, options, live]] of Object.entries(rows)) byKey.set(key, { source, key, send, sub, type, request, options, live });
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
  // What the permission decides of a call now: its status's answer, a denial or a ticket; null when it allows it.
  const decidePermission = (row, send) => {
    const p = row.sub.permission;
    if (!p) return null;
    const state = stateOf(p.name);
    if (!send && isStatus(row)) return { now: { ...(valueOf(row) ?? {}), [p.field]: state } };
    if (state === "denied") return { now: { error: p.denied } };
    const asks = row.request === "always" ? send : row.request === "when-prompt" ? !send : false;
    return state === "prompt" && asks ? { hold: "permission" } : null;
  };
  const decide = (row, send) => {
    // A target's own restriction holds under the agent too (§6): the web has no precision request.
    if (row.sub.unsupported?.includes("web")) return { now: { error: "NotSupportedError" } };
    const p = decidePermission(row, send);
    if (p) return p;
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
  // `forward`: the implementation's call, made once the ticket allows it.
  const hold = (row, waits, name, json = false, timeout = null, forward = null) => {
    // A newer ask of the same reader or mutation supersedes its held one (§6: retired).
    for (const h of held.filter(h => h.name === name)) { retire(h); h.reject(new Failed(`${row.key}: superseded`)); }
    return new Promise((resolve, reject) => {
      const h = { ticket: nextTicket(), row, waits, name, send: row.send, resolve, reject, json, timeout, forward, deadline: waits === "value" && timeout != null ? clock.now + timeout : null };
      held.push(h);
      say(`device ${capability(h)} ${h.ticket} held (agent)`);
    });
  };
  const retire = h => { const i = held.indexOf(h); if (i >= 0) held.splice(i, 1); };
  const release = (h, json) => { retire(h); if (h.json) return h.resolve(complete(json, h.row.type)); try { h.resolve(answer(h.row, json)); } catch (e) { h.reject(e); } };
  const releaseForward = h => { retire(h); h.forward().then(json => release(h, json), e => h.reject(e)); };
  const reask = keys => {
    commit(() => { const sources = new Set([...keys].map(k => byKey.get(k).source)); for (const r of Resources) if (sources.has(r.source)) R(r); }, "module");
    for (const key of keys) globalThis.exact?.announceModule?.(key); // data code's readers
  };
  // What answers a name: its implementation as in production (outside the agent), its implementation told it runs
  // under the agent (an interface that is its own substitute), or the substitute.
  const extensionOf = row => row.key.slice(0, row.key.indexOf("."));
  const route = row => !bound[extensionOf(row)] ? "substitute" : !clock.agent ? "real" : row.sub.implementation ? "agent" : "substitute";
  // The bound implementation, loaded on first use (LLP 1133.002 D15), with its context (LLP 1133.009 D10 items 2, 3):
  // `changed`, `agent`, the session's clock (`now`, the agent's under the agent) and whether the page is in the
  // foreground (`foreground()`; `foreground(shown)` on the implementation is told each change). The web has no
  // session end an implementation could finish in.
  const loaded = new Map(), live = new Set();
  const shown = () => typeof document !== "object" || document.visibilityState !== "hidden";
  const implementation = row => {
    const extension = extensionOf(row);
    if (route(row) === "substitute") return null;
    if (!loaded.has(extension)) loaded.set(extension, bound[extension]().then(m => {
      const context = { changed: key => byKey.has(key) && reask([key]), agent: clock.agent, now: () => clock.now, foreground: shown };
      const impl = m.implementation?.(context) ?? null;
      if (clock.agent) impl?.preferred?.(preferredOf(extension));
      return impl;
    }));
    return loaded.get(extension);
  };
  if (typeof addEventListener === "function") {
    addEventListener("visibilitychange", () => { for (const impl of loaded.values()) impl.then(i => i?.foreground?.(shown())); });
  }
  // The agent's preferred values for an extension's names, by name.
  const preferredOf = extension => Object.fromEntries([...preferred].filter(([k]) => k.startsWith(`${extension}.`)).map(([k, v]) => [k.slice(extension.length + 1), v]));
  // Under the agent, the preferred value laid over the implementation's answer field by field (LLP 1133.009 D10 item 11).
  const overlaid = (row, json) => {
    if (route(row) !== "agent" || errorCode(json) || !preferred.has(row.key)) return json;
    const p = preferred.get(row.key);
    return errorCode(p) ? p : overlay(structuredClone(json), p);
  };
  // A Contract reader's (`reader`, its resource's name) arguments, by live source: once the implementation has a
  // reader's new arguments, the group no reader asks with any longer is released, `releaseArgs(name, args)` (LLP
  // 1133.009 D10 item 12). A group data code asks for is released only with the whole name.
  const readers = new Map(), groups = new Map();
  const viaImplementation = (row, args, reader) => implementation(row)?.then(impl => {
    const name = row.key.slice(row.key.indexOf(".") + 1);
    if (!impl?.[name]) return { error: "NotSupportedError" };
    if (row.live) live.add(row.key);
    const answered = Promise.resolve(impl[name](args ?? {}, route(row) === "agent" ? { preferred: preferred.get(row.key) } : undefined));
    if (row.live && reader) {
      const by = readers.get(row.key) ?? new Map(), told = groups.get(row.key) ?? new Map();
      readers.set(row.key, by.set(reader, JSON.stringify(args ?? {})));
      groups.set(row.key, told.set(JSON.stringify(args ?? {}), args ?? {}));
      const current = new Set(by.values());
      for (const [group, was] of [...told]) if (!current.has(group)) { told.delete(group); say(`release ${row.key} ${group}: no reader asks with it`); impl.releaseArgs?.(name, was); }
    }
    return answered.then(json => overlaid(row, json));
  });
  // Admission (LLP 1133.002 D14): why the call is refused, or null.
  const admit = (row, args, origin) => {
    const sub = row.sub, lines = (data.grants ?? "").split("\n").map(l => l.trim().split(/\s+/)[0]);
    const granted = v => typeof v === "string" ? lines.includes(v) : Array.isArray(v) ? v.some(l => lines.includes(l)) : true;
    const g = sub.nameGrants;
    if (Array.isArray(g) && !granted(g)) return `grant-missing (${g.join(" or ")})`;
    if (g && !Array.isArray(g) && typeof g === "object") {
      for (const [arg, line] of Object.entries(g)) {
        const v = args?.[arg];
        if (v != null && v !== false && !granted(line)) return `grant-missing (${arg}= needs ${[line].flat().join(" or ")})`;
      }
    }
    const applies = !sub.when || Object.entries(sub.when).every(([k, v]) => args?.[k] === v);
    if (applies && sub.origin === "app" && origin !== "app") return `person-required (${origin} origin)`;
    if (applies && sub.foreground && typeof document === "object" && document.visibilityState === "hidden") return "not in the foreground";
    return null;
  };
  // Admitted, then what answers: `{now}`, `{hold}`, or `{forward}` (the implementation).
  const admitted = (row, args, origin, send, timeout) => {
    const why = admit(row, args, origin);
    if (why) { say(`${row.key}: refused, ${why}`); return { now: { error: "NotAllowedError" } }; }
    const r = route(row);
    if (r === "real") return { forward: true };
    const d = r === "agent" ? decidePermission(row, send) ?? { forward: true } : decideTimed(row, send, timeout);
    // Only a person answers a prompt (D12).
    if (d.hold === "permission" && origin !== "app") { say(`${row.key}: refused, person-required (${origin} origin)`); return { now: { error: row.sub.permission.denied } }; }
    return d;
  };
  // The last reader of a live source gone, its acquisition stops (D5).
  After.push(() => {
    for (const key of [...live]) {
      const row = byKey.get(key);
      if (Resources.some(r => r.source === row.source) || (globalThis.exact?.moduleReaders?.(key) ?? []).length) continue;
      live.delete(key);
      readers.delete(key);
      groups.delete(key);
      implementation(row)?.then(impl => impl?.release?.(key.slice(key.indexOf(".") + 1)));
    }
  });
  const reserved = (data.reserved ??= {});
  for (const row of byKey.values()) {
    reserved[row.source] = (_, args, name) => {
      const given = args?.[0] !== undefined ? object(args[0], row.options) : {};
      const timeout = row.send ? timeoutOf(row, args) : null, d = admitted(row, given, "app", row.send, timeout);
      const reader = row.send ? null : name;
      if (d.forward) return viaImplementation(row, given, reader).then(json => answer(row, json));
      if (d.hold) return hold(row, d.hold, name, false, timeout, route(row) === "agent" ? () => viaImplementation(row, given, reader) : null);
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
      if (h.forward) { releaseForward(h); continue; }
      const d = decideTimed(h.row, h.send, h.timeout);
      if (d.hold) { h.waits = d.hold; h.ticket = nextTicket(); h.deadline = d.hold === "value" && h.timeout != null ? clock.now + h.timeout : null; say(`device ${capability(h)} ${h.ticket} held (agent)`); } else release(h, d.now);
    }
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
  (globalThis.exact ??= {}).extensions = {
    // `state.extensions` (LLP 1133.002 §6), as the runner's `extensions_json` writes it.
    state() {
      const out = {};
      for (const row of byKey.values()) {
        const [extension, declared] = [row.key.slice(0, row.key.indexOf(".")), row.key.slice(row.key.indexOf(".") + 1)];
        const e = out[extension] ??= { bound: !clock.agent && !!bound[extension], preferred: {}, readers: {}, held: [] };
        if (route(row) === "agent") e.agent = true;
        if (row.sub.permission) e.permission = stateOf(row.sub.permission.name);
        if (preferred.has(row.key)) e.preferred[declared] = preferred.get(row.key);
        if (!row.send) e.readers[declared] = [...new Set([...Resources.filter(r => r.source === row.source || r.watches?.includes(row.key)).map(r => r.name), ...(globalThis.exact?.moduleReaders?.(row.key) ?? [])])];
        for (const h of held.filter(h => h.row === row)) e.held.push({ ticket: h.ticket, name: declared, origin: "app", waits: h.waits });
      }
      return out;
    },
    // A call from data code (ts-data.js `__exact_module`): JSON, as the native runtime answers it.
    call(body) {
      const row = byKey.get(`${body.extension}.${body.source ?? body.send}`);
      if (!row || row.send !== !!body.send) return Promise.resolve({ error: "NotSupportedError" });
      const timeout = row.send ? timeoutOf(row, body.args) : null, d = admitted(row, body.args ?? {}, "app", row.send, timeout);
      if (d.forward) return viaImplementation(row, body.args).then(json => complete(json, row.type));
      if (d.hold) return hold(row, d.hold, row.key, true, timeout, route(row) === "agent" ? () => viaImplementation(row, body.args) : null);
      say(`native ${row.key} (${route(row) === "substitute" ? "substituted" : "refused"})`);
      return Promise.resolve(complete(d.now, row.type));
    },
    holds: () => held.map(h => ({ name: h.name, ticket: h.ticket, device: { capability: capability(h), args: h.waits === "permission" ? { capability: h.row.sub.permission.name, request: h.row.key.split(".")[1], origin: "app" } : { request: h.row.key.split(".")[1] } } })),
    // `tap @t allow|deny|cancel`, `type @t <value>`; undefined for a ticket that is not a module's.
    answer(req) {
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
        // An implementation's own substitute takes a partial overlay (LLP 1133.009 D10 item 11); a declared one, a value.
        if (v !== "clear" && !errorCode(v)) {
          try { if (row.sub.implementation) partial(v, row.type, row.key); else typed(row, JSON.stringify(field ? { ...v, [field]: "prompt" } : v)); } catch (e) { return { error: `prefer extension: ${e.message}` }; }
        }
      }
      for (const [name, state] of Object.entries(req.permission ?? {})) setPermission(name, state);
      for (const [key, v] of Object.entries(group)) {
        if (v === "clear") preferred.delete(key); else preferred.set(key, v);
        say(`prefer extension ${key}`);
      }
      // An implementation bound under the agent is told its names' preferred values (LLP 1133.008 D8 item 9).
      for (const extension of new Set(Object.keys(group).map(k => byKey.get(k)).filter(r => route(r) === "agent").map(extensionOf))) {
        loaded.get(extension)?.then(impl => impl?.preferred?.(preferredOf(extension)));
      }
      settleValues();
      if (Object.keys(group).length) reask(Object.keys(group));
      return { extension: Object.fromEntries(preferred), permission: Object.fromEntries(permission) };
    },
  };
}

/** The code of an error answer, `{"error": "POSITION_UNAVAILABLE"}`. */
function errorCode(json) {
  if (json === null || typeof json !== "object" || Array.isArray(json)) return null;
  const keys = Object.keys(json);
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

/** `over`'s fields laid over `v`'s, record by record. */
function overlay(v, over) {
  if (v === null || typeof v !== "object" || Array.isArray(v) || over === null || typeof over !== "object" || Array.isArray(over)) return over;
  for (const [k, x] of Object.entries(over)) v[k] = overlay(v[k], x);
  return v;
}

/** `v`, a partial overlay of a value of `type`: each field it writes checked, record by record. */
function partial(v, type, at) {
  if (v !== null && typeof v === "object" && !Array.isArray(v) && type && typeof type === "object" && !Array.isArray(type)) {
    for (const [k, x] of Object.entries(v)) {
      if (!Object.hasOwn(type, k)) throw new Failed(`${at}: field \`${k}\` is not in the shape`, "shape");
      partial(x, type[k], `${at}.${k}`);
    }
    return;
  }
  if (Array.isArray(type) && type[0] === "?") { if (v != null) partial(v, type[1], at); return; }
  value(v, type, at);
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
