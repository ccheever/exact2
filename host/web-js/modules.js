// A native module's sources and sends on the JS target (LLP 1133.002 D13,
// §6; LLP 1124 D7, D9), substituted as the runner's `modules.rs` does
// natively, by what each name's `exact.json` declares: a permission whose
// state is `prompt` until `prefer permission.<name>` or a ticket sets it;
// a source's preferred value (`prefer extension`) or its default; a send
// answered from its `from` source's preferred value, else held under its
// `ticket` for the agent (`type @t …`). A held request is a promise the
// agent's answer settles. Imported by an app's module only when its plan
// names a module or places a package's view (views.js).
//
// Every call is admitted first (LLP 1133.002 D14, as the runner's
// `modules/admit.rs`): the grant lines its name or arguments list, its
// `origin` and its `foreground`. Outside the agent the bound implementation
// then answers; under it, an interface that is its own substitute
// (`implementation` in its declaration) is answered by its implementation,
// told `agent: true`, behind the permission and tickets here, with the
// agent's preferred value laid over each answer; every other by the
// substitute. A call's origin is `app`, `tool` from a handler a module
// called back (`invoke`, LLP 1133.016 D9), `device` from an event a package
// emitted (LLP 1133.004 D6), or `wake` while a wake's work runs (LLP
// 1133.017 D5, wakes.js).
//
// A send that answers nothing (type `u`) is a host command of the source's
// name, run once its commit stands (LLP 1133.004 D5), and its declared list,
// value and admission answer under the agent (§7). An event the root hears
// is raised by the host (an activation the implementation binds, `isTrusted`
// only) or emitted (the device's origin), and heard in order, once, each as
// its own commit (D6).
import { data, Resources, commit, R, Failed, nextTicket, say, clock, Clocked, After, time, Store, Hosts, navigateRoot, locationOf, inflight, Origin } from "./rt.js";
import { reportTime } from "./navigation.js";
import { packageViews } from "./views.js";
import { packagePictures } from "./pictures.js";
import { appGrantSet, coversPath } from "./admission.js";
import { samples, groupOf } from "./samples.js";
import { wakes, REFUSED } from "./wakes.js";
import { reach } from "./reach.js";

// `rows`: source → [extension.name, send, substitute, type, request, options' type, live];
// `bound`: extension → a loader of its web implementation, where the app binds one
// (LLP 1133.001 D4), which answers outside the agent;
// `views`: a package view's tag → [extension, displayed, binding props] (views.js);
// `events`: extension.event → [its action, taken when heard; activation | emit | wake; hold; its parameters' types; its
// name; a wake's budget].
export function modules(rows, bound = {}, views = {}, events = {}) {
  const byKey = new Map(), preferred = new Map(), permission = new Map(), held = [];
  // A permission declared for other targets is no permission here (`targets`, LLP 1133.026 D19 item 2).
  const onWeb = p => !Array.isArray(p?.targets) || p.targets.includes("web");
  const here = sub => {
    if (!sub || typeof sub !== "object") return sub;
    const out = { ...sub };
    if (out.permission && !onWeb(out.permission)) delete out.permission;
    if (Array.isArray(out.permissionList)) out.permissionList = out.permissionList.filter(onWeb);
    return out;
  };
  for (const [source, [key, send, sub, type, request, options, live]] of Object.entries(rows)) byKey.set(key, { source, key, send, sub: here(sub), type, request, options, live });
  const sampled = samples(t => now() - clock.now + t);
  // The origin of the commit running now (LLP 1133.002 D12): the device's while an emitted event's handler runs, and,
  // since data code and an answer's commit cannot say which request they follow, while any request such a handler
  // made is still in flight (`deviceBase`, the in-flight count before it): the least privileged origin.
  let origin = "app", deviceBase = null, wk = null;
  const originNow = () => deviceBase !== null ? "device" : wk?.active() ? "wake" : origin;
  Origin.now = originNow;
  // What a commit of a wake's origin may not run (LLP 1133.017 D6): journaled, never run.
  Origin.refused = (name, from) => from === "wake" && REFUSED.has(name) && (say(`${name}: refused: during a wake`), true);
  After.push(() => { if (deviceBase !== null && inflight.n <= deviceBase) deviceBase = null; });
  const asks = (row, send) => row.request === "always" ? send : row.request === "when-prompt" ? !send : false;
  // A request's own timeout, the option its declaration's `timeout` names: from a Contract call's record, or data code's object.
  const timeoutOf = (row, args) => {
    const name = row.sub.timeout, at = name && row.options ? Object.keys(row.options).indexOf(name) : -1;
    const v = Array.isArray(args?.[0]) ? args[0][at] : args?.[name];
    return typeof v === "number" && v >= 0 ? v : null;
  };
  // A value ticket that would wait `timeout=0` fails `TIMEOUT` at once (D2).
  const decideTimed = (row, send, timeout, args) => { const d = decide(row, send, args); return d.hold === "value" && timeout === 0 ? { now: { error: "TIMEOUT" } } : d; };
  // Value tickets whose timeout has passed on the agent's clock fail `TIMEOUT`, retired (§6).
  Clocked.push(() => { for (const h of held.filter(h => h.deadline != null && h.deadline <= clock.now)) { say(`device position ${h.ticket}: timed out`); release(h, { error: "TIMEOUT" }); } });
  const sibling = (row, declared) => declared && byKey.get(`${row.key.slice(0, row.key.indexOf("."))}.${declared}`);
  // A word an interface declares for its implementation's agent path alone (`agent`, LLP 1133.012 §9), by key
  // (`speech-recognition.heard`): its type, where the key is no name of the interface.
  const rowOf = key => [...byKey.values()].find(r => r.key.slice(0, r.key.indexOf(".")) === key.slice(0, key.indexOf(".")));
  const agentWord = key => !byKey.has(key) && key.includes(".") ? rowOf(key)?.sub.agent?.[key.slice(key.indexOf(".") + 1)] : undefined;
  const wordFits = (ty, v) => ty === "string" ? typeof v === "string" : ty === "number" ? typeof v === "number" : ty === "bool" ? typeof v === "boolean"
    : ty.endsWith("[]") ? Array.isArray(v) && v.every(x => wordFits(ty.slice(0, -2), x)) : true;
  const isStatus = row => row.sub.permission?.status === row.key.slice(row.key.indexOf(".") + 1);
  // A permission no platform asks starts as its declaration says (`initial`, LLP 1133.017 D14).
  const stateOf = name => permission.get(name) ?? [...byKey.values()].flatMap(permissionsOf).find(p => p.name === name)?.initial ?? "prompt";
  const now = () => reportTime(clock.now)[0] + clock.now; // the agent clock's date, as `exactTime` reads it
  const filled = (row, v) => {
    if (errorCode(v)) return v;
    const out = structuredClone(v);
    for (const [path, filler] of Object.entries(row.sub.fill ?? {})) fill(out, path, filler === "now" ? now() : filler);
    return out;
  };
  const prefers = row => preferred.has(row.key) ? filled(row, preferred.get(row.key)) : undefined;
  // The web's own facts over a source's (`web`: local authentication's `later` cell, LLP 1133.010 D15). A default has
  // the web's restrictions laid over it (`targets.web`). An interface that is its own substitute takes a partial
  // preferred value, laid over the default (LLP 1133.009 D10 item 11), as does a `partial` source (LLP 1133.011 §9),
  // as the runner's `value_of`.
  const restricted = row => row.sub.default && row.sub.targets?.web ? overlay(structuredClone(row.sub.default), row.sub.targets.web) : row.sub.default;
  const preferredOver = row => {
    const p = prefers(row), d = restricted(row);
    const partial = row.sub.partial || (row.sub.implementation && d && typeof d === "object");
    return partial && p && !errorCode(p) ? overlay(structuredClone(d ?? {}), p) : p;
  };
  const valueOf = row => row.sub.index === "entries" ? rc.value(row) : row.sub.listed ? wk.listed(row.sub.listed) : row.sub.web ? { ...(preferredOver(row) ?? restricted(row) ?? {}), ...row.sub.web } : preferredOver(row) ?? restricted(row);
  // What the permission decides of a call now: its status's answer, a denial or a ticket; null when it allows it.
  // The permission a call asks: its declared one, or, where the interface declares several and the name's grants
  // are a line per argument (`permissionBy`), the one of the arguments it was given that settles the call now: a
  // denied one, else one still to ask, else the first (LLP 1133.008 D6, LLP 1133.009 D3), as the runner's `admit.rs`.
  const permissionOf = (row, args) => {
    const by = row.sub.permissionBy;
    if (!by) return row.sub.permission;
    const asked = Object.entries(by).filter(([a]) => given(args, a)).map(([, p]) => p);
    return asked.find(p => stateOf(p.name) === "denied") ?? asked.find(p => stateOf(p.name) === "prompt") ?? asked[0] ?? row.sub.permission;
  };
  const permissionsOf = row => [row.sub.permission, ...Object.values(row.sub.permissionBy ?? {})].filter(Boolean);
  const permissionNamed = (row, name) => permissionsOf(row).find(p => p.name === name);
  // A name admitted with no permission is never held or refused, and a request answers the status in every state but
  // `prompt` (`admission`, LLP 1133.004 §7).
  const decidePermission = (row, send, p = row.sub.permission) => {
    if (!p || row.sub.admission === "none") return null;
    const state = stateOf(p.name);
    if (!send && isStatus(row)) return { now: { ...(valueOf(row) ?? {}), [p.field]: row.sub.web?.[p.field] ?? state } };
    if (state === "denied" && row.sub.admission !== "request") return { now: { error: p.denied } };
    return state === "prompt" && asks(row, send) ? { hold: "permission" } : null;
  };
  const decide = (row, send, args = {}) => {
    // A target's own restriction holds under the agent too (§6): the web has no precision request.
    if (row.sub.unsupported?.includes("web")) return { now: { error: "NotSupportedError" } };
    const p = decidePermission(row, send);
    if (p) return p;
    if (send && row.sub.ceremony) return ceremonies.decide(row, args);
    if (send && row.sub.records) return { now: null }; // a signal the substitute records (LLP 1133.011 §9)
    if (send && row.sub.shortcuts === "pin") return { now: rc.pin(row, args) };
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
  const capability = h => h.waits === "permission" ? "permission" : h.waits === "raise" ? h.row.sub.raise.ticket : h.waits === "ceremony" ? h.row.sub.ceremony.ticket
    : h.waits === "choose" ? h.row.sub.choose.ticket : h.row.sub.ticket;
  // `forward`: the implementation's call, made once the ticket allows it.
  // A newer ask of the same reader or mutation supersedes its held one, whatever answers the newer (§6: retired).
  // A forwarded call let go is cancelled at its implementation (LLP 1133.016 D15 item 2).
  const supersede = (row, name) => {
    for (const h of held.filter(h => h.name === name)) { retire(h); say(`device ${capability(h)} ${h.ticket} retired`); h.reject(new Failed(`${row.key}: superseded`)); }
    const pending = forwarded.get(name);
    if (pending) { forwarded.delete(name); say(`cancel ${pending.key} request ${pending.request}: its call was let go`); pending.impl.cancel(pending.request); }
  };
  const forwarded = new Map();
  let requests = 0;
  // `args` choose the permission the call waits on (`permissionOf`), kept by name.
  // `kept`: whether data code's call is still wanted (`ts-data.js`), which a ceremony's reply reads (D11).
  const hold = (row, waits, name, json = false, timeout = null, forward = null, args = {}, kept = () => true) => {
    // Ceremonies follow D9 instead (`ceremonies.begins`): two mutations' calls of one function are two ceremonies.
    if (waits !== "ceremony") supersede(row, name);
    const queued = waits === "ceremony" && ceremonies.begins(row);
    return new Promise((resolve, reject) => {
      const h = { ticket: queued ? null : nextTicket(), row, waits, name, send: row.send, resolve, reject, json, timeout, forward, args, kept, queued, permission: permissionOf(row, args)?.name, deadline: waits === "value" && timeout != null ? clock.now + timeout : null };
      held.push(h);
      if (!queued) say(`device ${capability(h)} ${h.ticket} held (agent)`);
      if (waits === "ceremony") ceremonies.record(h, queued ? "queued" : "held");
    });
  };
  const retire = h => { const i = held.indexOf(h); if (i >= 0) held.splice(i, 1); };
  const release = (h, json) => {
    retire(h);
    json = ceremonies.ended(h, json);
    if (h.json) return h.resolve(complete(json, h.row.type));
    try { h.resolve(answer(h.row, json)); } catch (e) { h.reject(e); }
  };
  // `sheet`: a sheet's answer, which reaches the implementation as the call's `preferred` (`{tap}`, `{typed}`).
  const releaseForward = (h, sheet) => { retire(h); h.forward(sheet).then(json => release(h, json), e => h.reject(e)); };
  // Ceremonies (LLP 1133.011 §9), as the runner's `modules/ceremony.rs`: a grant matched by an argument's value, the
  // status rules (`unless`), one sheet at a time, the `passkey` ticket's args and answers, the withheld paths, a
  // dropped registration disposed of (D11), and the list `state.extensions` shows.
  const at = (v, path) => path.split(".").reduce((x, k) => x?.[k], v);
  const listed = [];
  const ceremonies = {
    grants(row, args) {
      const lines = (data.grants ?? "").split("\n").map(l => l.trim().split(/\s+/));
      for (const [path, spec] of Object.entries(row.sub.nameGrants ?? {})) {
        if (!spec?.line) continue;
        const code = spec.refuse ?? "NotAllowedError";
        // An omitted RP ID is the page's effective domain, which must be granted too (D6).
        const value = at(args, path) ?? (spec.omitted === "page" && typeof location === "object" ? location.hostname : undefined);
        if (typeof value !== "string") return { error: code, message: `${path} is omitted, and here no page's host stands for it` };
        if (!lines.some(([l, t]) => l === spec.line && t === value)) return { error: code, message: `grant-missing (${path}=${value} needs \`${spec.line} ${value}\`)` };
      }
      return null;
    },
    rpId(row, args) {
      const path = Object.entries(row.sub.nameGrants ?? {}).find(([, spec]) => spec?.matches === "target")?.[0];
      return (path && at(args, path)) ?? (typeof location === "object" ? location.hostname : null);
    },
    decide(row, args) {
      const c = row.sub.ceremony, status = valueOf(sibling(row, c.status)) ?? {};
      for (const rule of c.unless ?? []) {
        const applies = !rule.when || Object.entries(rule.when).every(([k, v]) => args?.[k] === v);
        if (applies && status[rule.field] === rule.is) return { now: { error: rule.error, message: `\`${rule.field}\` is ${rule.is} on web` } };
      }
      return { hold: "ceremony" };
    },
    args(h) {
      const c = h.row.sub.ceremony, declared = h.row.key.slice(h.row.key.indexOf(".") + 1);
      const client = c.clientOrigin?.web === "page" && typeof location === "object" ? location.origin : null;
      return { request: declared, ceremony: declared, mediation: h.args?.mediation ?? "optional", rpId: ceremonies.rpId(h.row, h.args), clientOrigin: client, options: c.options ? h.args?.[c.options] ?? null : null, origin: "app" };
    },
    // A held conditional ceremony ends with `AbortError` before another is held; `true` while a modal one is held (D9).
    begins(row) {
      const ext = extensionOf(row), ours = h => h.waits === "ceremony" && !h.queued && extensionOf(h.row) === ext;
      for (const h of held.filter(h => ours(h) && h.args?.mediation === "conditional")) {
        say(`device ${capability(h)} ${h.ticket} retired`);
        release(h, { error: "AbortError", message: "a newer ceremony replaced this conditional one" });
      }
      return held.some(ours);
    },
    promote() {
      if (held.some(h => h.waits === "ceremony" && !h.queued)) return;
      const next = held.find(h => h.queued);
      if (!next) return;
      next.queued = false; next.ticket = nextTicket();
      say(`device ${capability(next)} ${next.ticket} held (agent)`);
    },
    record(h, outcome) {
      const entry = { name: h.row.key.slice(h.row.key.indexOf(".") + 1), mediation: h.args?.mediation ?? null, rpId: ceremonies.rpId(h.row, h.args), origin: h.row.sub.ceremony ? ceremonies.args(h).clientOrigin : null, outcome };
      const i = listed.findIndex(([x]) => x === h);
      if (i >= 0) listed[i][1] = entry; else listed.push([h, entry]);
      if (listed.length > 16) listed.shift();
    },
    // The answer as it leaves: no withheld path; a registration whose call is no longer wanted disposed of (D11).
    ended(h, json) {
      if (json && typeof json === "object" && !errorCode(json)) {
        json = structuredClone(json);
        for (const path of h.row.sub.withhold ?? []) {
          const parts = path.split("."), last = parts.pop(), parent = parts.length ? at(json, parts.join(".")) : json;
          if (parent && typeof parent === "object") delete parent[last];
        }
      }
      if (h.waits !== "ceremony") return json;
      if (!h.kept()) json = ceremonies.dropped(h.row, json);
      ceremonies.record(h, errorCode(json) ?? "answered");
      if (!h.queued) ceremonies.promote();
      return json;
    },
    // A reply its call no longer wants: its registration disposed of, and the call ended with `AbortError`, so its
    // data code, which runs on, never has the response (D11).
    dropped(row, json) {
      const ids = row.sub.handles && !errorCode(json) ? Object.keys(row.sub.handles).flatMap(path => handlesAt(json, path)) : [];
      if (ids.length) {
        say(`dispose ${extensionOf(row)} ${ids.join(", ")}: its reply was dropped`);
        implementation(row)?.then(impl => impl?.dispose?.(ids));
      }
      return row.sub.ceremony ? { error: "AbortError", message: "the call was let go" } : json;
    },
  };
  const reask = keys => {
    commit(() => { const sources = new Set([...keys].map(k => byKey.get(k).source)); for (const r of Resources) if (sources.has(r.source)) R(r); }, "module");
    for (const key of keys) globalThis.exact?.announceModule?.(key); // data code's readers
  };
  // What answers a name: its implementation as in production (outside the agent), its implementation told it runs
  // under the agent (an interface that is its own substitute), or the substitute.
  const extensionOf = row => row.key.slice(0, row.key.indexOf("."));
  const route = row => !bound[extensionOf(row)] ? "substitute" : !clock.agent ? "real" : row.sub.implementation ? "agent" : "substitute";
  // The bound implementation, loaded on first use (LLP 1133.002 D15), with its context (LLP 1133.009 D10 items 2-6):
  // `changed`, `agent`, the session's clock (`now`, the agent's under the agent), whether the page is in the
  // foreground (`foreground()`; `foreground(shown)` on the implementation is told each change), the journal (`log`),
  // the track table (`tracks`), admitted paths' files (`files`) and the audio session (`audioSession`). The web has
  // no session end an implementation could finish in.
  const loaded = new Map(), chunks = new Map(), live = new Set(), ready = new Map();
  // Under the agent the drive's page fact (`prefer visibility-state`, agent.js) is the foreground; the machine's never is.
  let agentShown = true;
  const shown = () => clock.agent ? agentShown : typeof document !== "object" || document.visibilityState !== "hidden";
  // The session's track table (LLP 1133.009 D10 item 6): tracks and streams by id, each track with the
  // implementation that opened it, its `owner`, through which a consuming implementation subscribes.
  const table = new Map(), holds = new Set();
  const tracks = {
    // An exclusive hold on a device for a recognizer that opens it itself (LLP 1133.012 D8, D12 item 4): granted
    // only while no track of `kind` has a running capture (its owner's `leased(id)`), and while it lasts the owner
    // opens no capture of that kind. Its release, or a throw `NotReadableError`.
    hold(kind) {
      if ([...table.values()].some(e => e.kind === kind && e.owner?.leased?.(e.id))) throw Object.assign(new Error(`a ${kind} capture runs`), { code: "NotReadableError" });
      const h = { kind };
      holds.add(h);
      return () => { holds.delete(h); };
    },
    held: kind => [...holds].some(h => h.kind === kind),
    add: entry => { table.set(entry.id, entry); },
    lookup: id => table.get(id) ?? null,
    remove: id => { table.delete(id); },
    all: () => [...table.values()],
    /** `consumer` ({live(source), muted(reason), unmuted(), ended()}) joins track `id`; how = {start, origin}. */
    subscribe(id, consumer, how = {}) {
      const entry = table.get(id);
      if (!entry?.owner?.subscribe) throw Object.assign(new Error(`no track ${id}`), { code: "NotFoundError" });
      return entry.owner.subscribe(id, consumer, how);
    },
  };
  // An admitted path's file (LLP 1133.009 D10 item 4), in the store `storage.fs` reads: the app's, or the drive's
  // scratch store under the agent; none when the drive names none.
  let store = null;
  const fileStore = () => store ??= import("./storage-environment.js").then(env => {
    const key = env.storageKey(data.appId);
    if (key == null) throw Object.assign(new Error(env.agentStorageRefusal), { code: "agent" });
    return import(new URL("./storage-fs.js", import.meta.url).href).then(m => m.createFileStore(key));
  });
  const files = {
    // A file's first write makes its directory, as a recorder's writer needs it.
    write: (path, bytes) => fileStore().then(async s => { await s.mkdir(path.slice(0, path.lastIndexOf("/"))); return s.writeFile(path, bytes); }),
    append: (path, bytes) => fileStore().then(s => s.appendFile(path, bytes)),
    read: path => fileStore().then(s => s.readFile(path)),
    // A directory a handle fills (LLP 1133.012 D12 item 7): emptied, made, and listed.
    remove: path => fileStore().then(s => s.rm(path)),
    mkdir: path => fileStore().then(s => s.mkdir(path)),
    list: path => fileStore().then(s => s.readdir(path)).catch(() => []),
  };
  // The page's Audio Session (W3C Audio Session; LLP 1133.009 D7, LLP 1133.012 D4): `play-and-record` while a
  // capture's claim is held, else the strongest spoken type claimed (`playback` over `transient-solo` over
  // `transient`), else what the page had.
  const claims = new Set(), spoken = ["transient", "transient-solo", "playback"];
  let before = null;
  const session = () => typeof navigator === "object" ? navigator.audioSession : undefined;
  const compose = () => {
    const s = session();
    if (!s) return;
    const types = [...claims].map(c => c.type);
    const want = types.includes("play-and-record") ? "play-and-record" : spoken.filter(t => types.includes(t)).at(-1);
    if (want && before == null) before = s.type;
    if (want) s.type = want;
    else if (before != null) { s.type = before; before = null; }
  };
  const audioSession = {
    /** A capture's claim, or speech's with `{type}`, one of the spoken types. */
    claim(how = {}) {
      const claim = { type: spoken.includes(how.type) ? how.type : "play-and-record", release: () => { claims.delete(claim); compose(); } };
      claims.add(claim);
      compose();
      return claim;
    },
    /** The type in force: the page's, or the strongest claimed where the page has no Audio Session. */
    type() {
      const types = [...claims].map(c => c.type);
      return session()?.type ?? (types.includes("play-and-record") ? "play-and-record" : spoken.filter(t => types.includes(t)).at(-1) ?? "auto");
    },
  };
  // The extension's chunk, and the session's one instance of its implementation, which its names and its views share
  // (LLP 1133.008 D8 item 1).
  const chunk = extension => { if (!chunks.has(extension)) chunks.set(extension, bound[extension]()); return chunks.get(extension); };
  const instance = extension => {
    if (!bound[extension]) return null;
    if (!loaded.has(extension)) loaded.set(extension, chunk(extension).then(m => {
      const context = { changed: key => byKey.has(key) && reask([key]), agent: clock.agent, now: () => { time(); return clock.now; }, foreground: shown, log: say, tracks, files, audioSession,
        revokePicture: source => pictures?.revokePicture(source), pictureChanged: source => pictures?.pictureChanged(source), invoke: invoker(extension),
        // A package's own raising has the device's origin; a person's is the host's to give (LLP 1133.004 D6).
        emit: (name, args) => raise(`${extension}.${name}`, args, "device"),
        // The web's wake is the page's own (LLP 1133.017 D8): an implementation raises a declared wake event, and is
        // answered the invocation's outcome (`completed`, `expired`, `refused`, `unheard`).
        wakes: { raise: (name, args, how = {}) => wk.raise(`${extension}.${name}`, args, { hidden: !shown(), ...how }) },
        // Only an event that is the web's user activation (HTML's activation-triggering input events, and `click`), and
        // trusted, is a person's; any other type binds nothing.
        activation: (target, type, name, args, { navigate } = {}) => !ACTIVATING.has(type) ? say(`${extension}.${name}: a ${type} is no person's activation; nothing is bound`) : target.addEventListener(type, e => {
          if (!e.isTrusted) return say(`${extension}.${name}: an untrusted ${type} raises nothing`);
          try { globalThis.focus?.(); } catch {}
          if (navigate) navigateRoot(locationOf(navigate)); else raise(`${extension}.${name}`, args, "app");
        }) };
      const impl = m.implementation?.(context) ?? null;
      if (clock.agent) impl?.preferred?.(preferredOf(extension));
      ready.set(extension, impl);
      return impl;
    }));
    return loaded.get(extension);
  };
  const implementation = row => route(row) === "substitute" ? null : instance(extensionOf(row));
  // A send made inside the press (`"gesture": true`, LLP 1133.013 D12 item 5): its implementation is loaded once
  // the page is up, so the send's handler runs in the input event's dispatch, where a browser's gesture holds.
  const gestures = new Set([...byKey.values()].filter(r => r.send && r.sub.gesture && route(r) === "real").map(extensionOf));
  if (gestures.size) setTimeout(() => { for (const extension of gestures) instance(extension); });
  packageViews(views, chunk, instance);
  // A package's `img` scheme (LLP 1133.019 D11 item 1): admitted as the runner's `modules/pictures.rs` admits it on
  // target `web` — the app declares one of its grant lines, an implementation is bound, and under the agent its
  // permission is granted and the app declares that permission's lines — then drawn by its implementation (pictures.js).
  const schemes = {}, imageRows = {}, pictureCounts = new Map();
  for (const row of byKey.values()) {
    const scheme = row.sub.images?.scheme;
    if (typeof scheme === "string" && !imageRows[extensionOf(row)]) { schemes[scheme] = extensionOf(row); imageRows[extensionOf(row)] = row; }
  }
  const pictureDecision = row => {
    const images = row.sub.images, lines = (data.grants ?? "").split("\n").map(l => l.trim().split(/\s+/)[0]);
    if (!images.grants.some(l => lines.includes(l))) return `grant-missing (needs ${images.grants.join(" or ")})`;
    const r = route(row);
    if (r === "substitute") return "no implementation bound";
    const name = images.permission;
    if (typeof name !== "string" || r === "real") return null;
    const entries = [row.sub.permission, ...(row.sub.permissionList ?? []), ...Object.values(row.sub.permissionBy ?? {})].flat();
    const read = entries.filter(p => p?.name === name).flatMap(p => p.grants ?? []).some(l => lines.includes(l));
    if (!read) return `grant-missing (permission ${name})`;
    return stateOf(name) === "granted" ? null : `permission ${name} is ${stateOf(name)}`;
  };
  const admitPicture = extension => {
    const why = pictureDecision(imageRows[extension]), c = pictureCounts.get(extension) ?? { admitted: 0, refused: 0 };
    if (why) { c.refused++; c.refusal = why; } else { c.admitted++; delete c.refusal; }
    pictureCounts.set(extension, c);
    return why;
  };
  const pictures = Object.keys(schemes).length ? packagePictures(schemes, admitPicture, instance, extension => pictureDecision(imageRows[extension])) : null;
  const tell = () => { sampled.visibility(!shown()); for (const impl of loaded.values()) impl.then(i => i?.foreground?.(shown())); };
  if (typeof addEventListener === "function") addEventListener("visibilitychange", tell);
  (globalThis.exact ??= {}).foreground = visible => { if (agentShown !== visible) { agentShown = visible; tell(); } };
  // The agent's preferred values for an extension's names, by name.
  const preferredOf = extension => Object.fromEntries([...preferred].filter(([k]) => k.startsWith(`${extension}.`)).map(([k, v]) => [k.slice(extension.length + 1), v]));
  // Under the agent, the preferred value laid over the implementation's answer field by field (LLP 1133.009 D10 item 11).
  // An implementation that lays it itself (`"overlays": "implementation"`) is told it instead (LLP 1133.016 §9).
  const overlaid = (row, json) => {
    if (route(row) !== "agent" || errorCode(json) || !preferred.has(row.key) || row.sub.overlays === "implementation") return json;
    const p = preferred.get(row.key);
    return errorCode(p) ? p : overlay(structuredClone(json), p);
  };
  // A Contract reader's (`reader`, its resource's name) arguments, by live source: once the implementation has a
  // reader's new arguments, the group no reader asks with any longer is released, `releaseArgs(name, args)` (LLP
  // 1133.009 D10 item 12). A group data code asks for is released only with the whole name.
  const readers = new Map(), groups = new Map();
  // `how`: the call's origin, the reader or mutation that asked (`name`), whose newer call cancels this one, and a held
  // sheet's answer (`sheet`, LLP 1133.018 D13 item 4). The implementation is told the origin and the request's number,
  // which its `cancel(request)` is given, and under the agent the sheet's answer or else the preferred value.
  const viaImplementation = (row, args, reader, how = {}) => implementation(row)?.then(async impl => {
    const name = row.key.slice(row.key.indexOf(".") + 1);
    // An extension with no web implementation reads its declared default, as the runner answers a target with none
    // (LLP 1133.028 D17); its sends refuse.
    if (impl === null && !row.send) return valueOf(row) ?? { error: "NotSupportedError" };
    if (!impl?.[name]) return { error: "NotSupportedError" };
    // A call not the app's that would ask is refused while the platform's state is `prompt` (LLP 1133.002 D12).
    const p = row.sub.permission;
    if ((how.origin ?? "app") !== "app" && p?.status && row.sub.admission !== "none" && asks(row, row.send) && impl[p.status]) {
      const status = await impl[p.status]({});
      if (status?.[p.field] === "prompt") { say(`${row.key}: refused, person-required (${how.origin} origin)`); return { error: p.denied }; }
    }
    if (row.live) live.add(row.key);
    const request = ++requests, asker = how.name ?? reader;
    if (row.send && asker && impl.cancel) forwarded.set(asker, { key: row.key, request, impl });
    const told = { origin: how.origin ?? "app", request, ...(route(row) === "agent" ? { preferred: how.sheet ?? preferred.get(row.key) } : {}) };
    const answered = Promise.resolve(impl[name](args ?? {}, told)).finally(() => { if (forwarded.get(asker)?.request === request) forwarded.delete(asker); });
    if (row.live && row.sub.poll) {
      polls.set(row.key, clock.now);
      asking.add(row.key);
      // An error (an id the session does not hold) is not polled again until a reader asks anew.
      answered.then(json => { if (errorCode(json)) failed.add(row.key); else failed.delete(row.key); }, () => failed.add(row.key))
        .finally(() => { asking.delete(row.key); if (!clock.agent) setTimeout(pollCheck, periodOf(row) ?? 0); });
    }
    if (row.live && reader) {
      const by = readers.get(row.key) ?? new Map(), told = groups.get(row.key) ?? new Map(), group = groupOf(args);
      const { after, ...asked } = args ?? {};
      readers.set(row.key, by.set(reader, group));
      groups.set(row.key, told.set(group, asked));
      const current = new Set(by.values());
      for (const [group, was] of [...told]) if (!current.has(group)) { told.delete(group); say(`release ${row.key} ${group}: no reader asks with it`); impl.releaseArgs?.(name, was); }
    }
    return answered.then(json => { handled(row, args, json); polled(row, args, json); return overlaid(row, json); });
  });
  // ---- a send that answers nothing, and its declared substitute (LLP 1133.004 D5, §7)
  const lists = new Map(), setValues = new Map(), effects = [];
  const rc = reach({ say, announce: keys => reask(keys), preferred: key => preferred.get(key) });
  const listOf = row => [...byKey.values()].filter(r => extensionOf(r) === extensionOf(row) && r.sub.list).map(r => r.sub.list.state);
  const applyEffect = (row, args) => {
    const reached = rc.effect(row, args, [...byKey.values()]);
    if (reached !== undefined) return reached;
    const name = row.key.split(".")[1], sub = row.sub;
    if (sub.list) {
      // A registration's kind keys it beside its key (LLP 1133.017 §10).
      const entries = lists.get(sub.list.state) ?? [], value = args[sub.list.key] ?? null, ref = nextTicket(), kind = sub.list.kind;
      const same = e => e.fields[sub.list.key] === value && e.fields.kind === kind, replaced = value == null ? null : entries.find(same)?.ref;
      const kept = value == null ? entries : entries.filter(e => !same(e));
      kept.push({ ref, row, fields: { ...Object.fromEntries(Object.keys(row.options ?? {}).map(k => [k, null])), ...args, ...(kind ? { kind } : {}) } });
      lists.set(sub.list.state, kept);
      say(`${name}: listed${typeof value === "string" ? ` (${value})` : ""} @${ref}`);
      if (kind) wk.registered(ref, replaced);
      listedChanged(sub.list.state);
    }
    // `"unlist": key`, or `{key, kind}` for the entries of one kind.
    const unlist = typeof sub.unlist === "object" ? sub.unlist?.key : sub.unlist, unkind = sub.unlist?.kind;
    if (unlist) {
      for (const state of listOf(row)) { lists.set(state, (lists.get(state) ?? []).filter(e => e.fields[unlist] !== args[unlist] || (unkind != null && e.fields.kind !== unkind))); listedChanged(state); }
      say(`${name}: unlisted`);
    }
    if (sub.set) {
      const from = sub.set.from, given = from ? args[from] : undefined;
      const value = !from ? (sub.set.value ?? null) : given == null ? (sub.set.absent ?? null) : given === 0 ? (sub.set.zero ?? null) : given;
      setValues.set(`${extensionOf(row)}/${sub.set.state}`, value);
      say(`${name}: ${sub.set.state} ${JSON.stringify(value)}`);
    }
    return null;
  };
  /** One call: done (`null`), refused (`{error}`), or held on the permission ticket until it is answered. */
  const effect = (row, args, from = originNow()) => {
    const before = origin;
    if (from === "device") origin = "device";
    try { return effectNow(row, args, from); } finally { origin = before; }
  };
  const effectNow = (row, args, from) => {
    const name = row.key.split(".")[1];
    const byValue = ceremonies.grants(row, args);
    if (byValue) { say(`${row.key}: refused, ${byValue.message}`); return Promise.resolve(byValue); }
    // A target's own restriction holds under the agent too: the web has no periodic sync (LLP 1133.017 D17).
    if (row.sub.unsupported?.includes("web") && route(row) !== "real") { say(`${name}: refused: NotSupportedError`); return Promise.resolve({ error: "NotSupportedError" }); }
    const why = admit(row, args, from);
    if (why) { say(`${row.key}: refused, ${why}`); return Promise.resolve(why.startsWith("person-required") ? { error: "NotAllowedError", message: "person-required" } : { error: "NotAllowedError" }); }
    const real = route(row) === "real" ? viaImplementation(row, args, null, { origin: from }) : null;
    if (real) return real.then(r => { if (errorCode(r)) say(`${name}: refused: ${errorCode(r)}`); return errorCode(r) ? r : null; });
    const p = row.sub.permission, state = p && row.sub.admission !== "none" ? stateOf(p.name) : null;
    if (state === "denied") { say(`${name}: refused: ${p.denied}`); return Promise.resolve({ error: p.denied }); }
    if (state === "prompt" && asks(row, true)) {
      // Only a person answers a prompt (LLP 1133.002 D12).
      if (from !== "app") { say(`${row.key}: refused, person-required (${from} origin)`); return Promise.resolve({ error: p.denied }); }
      return new Promise(resolve => { const h = { ticket: nextTicket(), row, args, resolve }; effects.push(h); say(`device permission ${h.ticket} held (agent)`); });
    }
    return Promise.resolve(applyEffect(row, args));
  };
  const settleEffects = (name, state) => {
    for (const h of effects.filter(h => h.row.sub.permission?.name === name)) {
      if (state === "prompt") continue;
      effects.splice(effects.indexOf(h), 1);
      if (state === "granted") h.resolve(applyEffect(h.row, h.args));
      else { say(`${h.row.key.split(".")[1]}: refused: ${h.row.sub.permission.denied}`); h.resolve({ error: h.row.sub.permission.denied }); }
    }
  };
  for (const row of byKey.values()) if (row.send && row.sub.effect) Hosts[row.source] = Object.assign((from, record) => { effect(row, object(record, row.options) ?? {}, from); }, { origin: true });
  const delivered = e => { const at = e.row.sub.list.at, t = at ? e.fields[at] : null; return t == null || t <= now(); };
  // A source declared `"listed"` answers from a list: asked again when it changes.
  const listedChanged = state => { const keys = [...byKey.values()].filter(r => r.sub.listed?.state === state).map(r => r.key); if (keys.length) reask(keys); };
  wk = wakes({ events, say, value, lists, changed: listedChanged, toEpoch: ms => reportTime(ms)[0] + ms,
    onLine: () => (globalThis.exact?.page?.read?.().online ?? globalThis.navigator?.onLine) !== false,
    run: f => { const before = origin; origin = "wake"; try { f(); } finally { origin = before; } } });
  // ---- events (LLP 1133.004 D6)
  const unheard = [];
  const raise = (key, args, from) => {
    const declared = events[key];
    if (!declared) return say(`${key}: no handler hears it`);
    const [, raised, bound] = declared;
    if (raised === "wake") return say(`${key}: a wake raises it, at the platform's call`);
    unheard.push({ key, args: Array.isArray(args) ? args : [], origin: raised === "activation" ? from : "device" });
    // Each event's own bound: past it, its own oldest is dropped.
    while (unheard.filter(q => q.key === key).length > bound) { const old = unheard.splice(unheard.findIndex(q => q.key === key), 1)[0]; say(`${key}: dropped (${typeof old.args[0] === "string" ? old.args[0] : JSON.stringify(old.args[0] ?? null)})`); }
    say(`${key}: raised`);
    queueMicrotask(hear);
  };
  const hear = () => {
    while (unheard.length) {
      const { key, args, origin: from } = unheard.shift(), [action, , , types, name] = events[key];
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
    raise(`${extensionOf(row)}.${tap.event}`, args, "app");
    return { ref, event: tap.event, delivery: "substituted" };
  };
  // The handlers a handle's work may call back (LLP 1133.016 D15 item 1): those its call's `handlers` path lists
  // (`tools[].name`), or those of the handle its `handlersFrom` option names. A call back for any other is refused.
  const handlers = new Map();
  const handled = (row, args, json) => {
    if (errorCode(json) || !(row.sub.handlers || row.sub.handlersFrom)) return;
    const names = row.sub.handlers ? handlesAt(args, row.sub.handlers) : [...(handlers.get(args?.[row.sub.handlersFrom]) ?? [])];
    for (const id of Object.keys(row.sub.handles ?? {}).flatMap(path => handlesAt(json, path))) handlers.set(id, new Set(names));
  };
  // `invoke(handle, handler, args)`: the data module's handler run for the handle's work, as `tool` (the web has no
  // wake to narrow to); its answer as JSON text, `{text}`, or `{error, message}`.
  const invoker = extension => (handle, handler, args) => {
    const at = `${extension}: ${handler} for ${handle}`;
    if (!handlers.get(handle)?.has(handler)) { say(`${at}: refused, no call made for it lists the handler`); return Promise.resolve({ error: "NotAllowedError" }); }
    const started = clock.now;
    say(`${at}: run, tool:model:${handle}`);
    let run;
    try { run = data.tool?.(handler, args ?? {}, Store, "tool"); } catch (e) { run = Promise.reject(e); }
    if (!run) { say(`${at}: refused, the data module's \`tools\` has no \`${handler}\``); return Promise.resolve({ error: "NotFoundError" }); }
    return run.then(v => {
      const text = JSON.stringify(v ?? null);
      say(`${at}: answered ${new TextEncoder().encode(text).length} bytes in ${clock.now - started} ms`);
      return { text };
    }, e => {
      say(`${at}: failed: ${e?.message ?? e}`);
      return { error: typeof e?.code === "string" ? e.code : "OperationError", message: String(e?.message ?? e) };
    });
  };
  // A poll that ends with its answer (`poll.while`, LLP 1133.016 D15 item 3): whether each argument group's last
  // answer is one the declaration polls on. While no reader's is, the clock does not ask; an announcement does.
  const within = new Map();
  const polled = (row, args, json) => {
    const w = row.sub.poll?.while;
    if (!w || !row.live) return;
    const by = within.get(row.key) ?? within.set(row.key, new Map()).get(row.key);
    by.set(groupOf(args), !errorCode(json) && Object.entries(w).every(([field, values]) => values.includes(json?.[field])));
  };
  const pollEnded = row => {
    const by = within.get(row.key), groups = [...(readers.get(row.key)?.values() ?? [])];
    return !!row.sub.poll?.while && !!by && groups.length > 0 && groups.every(g => by.get(g) === false);
  };
  // Polled live sources (LLP 1133.009 D10 item 7, as the runner's `modules/poll.rs`): asked again each period of the
  // option `poll` names (or its default) while a reader reads them, once after an advance that crossed a period, and
  // on the live clock once a period. No plan timer: a poll never stops a jump, which waits for the ask it made
  // (`polling`). At most one ask of a source is in flight.
  const polls = new Map(), asking = new Set(), failed = new Set();
  const periodOf = row => {
    const option = typeof row.sub.poll === "string" ? row.sub.poll : row.sub.poll?.option;
    // A reader that names no handle (every handle option empty) reads nothing a poll would change.
    const handles = row.sub.handleOptions ?? [], all = [...(readers.get(row.key)?.values() ?? [])].map(a => JSON.parse(a));
    const polling = all.filter(a => !handles.length || handles.some(h => typeof a[h] === "string" && a[h] !== ""));
    if (all.length && !polling.length) return null;
    const asked = polling.map(a => a[option]).filter(ms => typeof ms === "number" && ms > 0);
    return asked.length ? Math.min(...asked) : row.sub.poll?.default > 0 ? row.sub.poll.default : null;
  };
  const pollCheck = () => {
    time(); // on the live clock, the wall's time: a page with no timer moves no clock
    const due = [...polls].filter(([key, last]) => {
      const row = byKey.get(key), period = periodOf(row);
      return period != null && (route(row) !== "substitute" || sampled.sampled(row)) && !asking.has(key) && !failed.has(key) && !pollEnded(row) && live.has(key) && clock.now - last >= period;
    }).map(([key]) => key);
    for (const key of due) { polls.set(key, clock.now); say(`poll ${key}`); }
    if (due.length) reask(due);
  };
  let pollQueued = false;
  Clocked.push(() => { if (!pollQueued && polls.size) { pollQueued = true; queueMicrotask(() => { pollQueued = false; pollCheck(); }); } });
  // Admission (LLP 1133.002 D14): why the call is refused, or null.
  const admit = (row, args, origin) => {
    const sub = row.sub, lines = (data.grants ?? "").split("\n").map(l => l.trim().split(/\s+/)[0]);
    const granted = v => typeof v === "string" ? lines.includes(v) : Array.isArray(v) ? v.some(l => lines.includes(l)) : true;
    const g = sub.nameGrants;
    // An empty list needs no line (a picker's, LLP 1133.018 D4).
    if (Array.isArray(g) && g.length && !granted(g)) return `grant-missing (${g.join(" or ")})`;
    if (g && !Array.isArray(g) && typeof g === "object") {
      for (const [arg, line] of Object.entries(g)) {
        if (given(args, arg) && !granted(line)) return `grant-missing (${arg}= needs ${[line].flat().join(" or ")})`;
      }
    }
    // A path option is the app's own file, within its `fs.read` or `fs.write` lines (LLP 1133.009 D10 item 4).
    for (const [option, path] of Object.entries(sub.paths ?? {})) {
      const given = args?.[option], capability = `fs.${path.access}`;
      if (typeof given === "string" && !(appPath(given) && coversPath(appGrantSet(), capability, given))) return `grant-missing (${option}=${given} needs ${capability})`;
    }
    const applies = !sub.when || Object.entries(sub.when).every(([k, v]) => args?.[k] === v);
    // `{whenAny, is: "app"}`: only a call giving one of those options is held to `app` (LLP 1133.012 D12 item 5).
    const appOnly = sub.origin?.whenAny ? sub.origin.whenAny.some(a => given(args, a)) : sub.origin === "app";
    if (applies && appOnly && origin !== "app") return `person-required (${origin} origin)`;
    if (applies && sub.foreground && !shown()) return "not in the foreground";
    return null;
  };
  // Admitted, then what answers: `{now}`, `{hold}`, or `{forward}` (the implementation).
  const admitted = (row, args, origin, send, timeout) => {
    // A grant matched by the argument's value fails with its own code (`SecurityError` for an RP ID, LLP 1133.011 D6).
    const byValue = ceremonies.grants(row, args);
    if (byValue) { say(`${row.key}: refused, ${byValue.message}`); return { now: byValue }; }
    const why = admit(row, args, origin);
    if (why) { say(`${row.key}: refused, ${why}`); return { now: why.startsWith("person-required") ? { error: "NotAllowedError", message: "person-required" } : { error: "NotAllowedError" } }; }
    const r = route(row);
    if (r === "real") return { forward: true };
    const p = permissionOf(row, args);
    // Under the agent: the web's own restriction, then the permission's ticket, a sheet's or the raise's (LLP 1133.018
    // D13 item 4); else the implementation.
    const agentDecision = () => row.sub.unsupported?.includes("web") ? { now: { error: "NotSupportedError" } }
      : decidePermission(row, send, p) ?? (send && row.sub.choose ? { hold: "choose" } : null) ?? (send && row.sub.raise ? decide(row, send, args) : null) ?? { forward: true };
    let d = r === "agent" ? agentDecision() : decideTimed(row, send, timeout, args);
    // A name whose grants are per argument asks permission only for an argument it was given (LLP 1133.009 D3).
    const g = row.sub.nameGrants, asks = !g || Array.isArray(g) || Object.keys(g).some(a => given(args, a));
    if (d.hold === "permission" && !asks) d = r === "agent" ? { forward: true } : { now: { error: "TypeError" } };
    // Only a person answers a prompt (D12).
    if (d.hold === "permission" && origin !== "app") { say(`${row.key}: refused, person-required (${origin} origin)`); return { now: { error: p.denied } }; }
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
      polls.delete(key);
      sampled.release(key);
      implementation(row)?.then(impl => impl?.release?.(key.slice(key.indexOf(".") + 1)));
    }
  });
  const reserved = (data.reserved ??= {});
  // Handles (LLP 1133.009 D6, D10 item 10): a reply naming the objects a call made is the call's until delivered. A
  // reply to a call its reader or mutation asked again before it landed is dropped, and its implementation told to
  // dispose of them. (The runner also disposes of a refused commit's; here no commit refuses a module's answer.)
  const asked = new Map();
  const disposeDropped = (row, json, name, n) => {
    if (asked.get(name) === n || !row.sub.handles || errorCode(json)) return;
    const ids = Object.keys(row.sub.handles).flatMap(path => handlesAt(json, path));
    if (!ids.length) return;
    say(`dispose ${extensionOf(row)} ${ids.join(", ")}: its reply was dropped`);
    implementation(row)?.then(impl => impl?.dispose?.(ids));
  };
  // A sampled source's reader (LLP 1133.013 D5): the implementation is passed the reader's cursor as `after`; the
  // substitute answers the reader's grid. Either answer moves the cursor.
  const sampledAnswer = (row, d, args, reader) => {
    if (d.forward) { const after = sampled.cursor(row, reader); if (after !== undefined) args.after = after; return d; }
    if (d.hold || errorCode(d.now) || route(row) !== "substitute") return d;
    // Polled as an implementation's reader is: each period's ask answers that period's samples.
    live.add(row.key);
    readers.set(row.key, (readers.get(row.key) ?? new Map()).set(reader, groupOf(args)));
    polls.set(row.key, clock.now);
    const json = sampled.substitute(row, args, reader);
    sampled.answered(row, reader, json);
    return { now: json };
  };
  for (const row of byKey.values()) {
    reserved[row.source] = (_, args, name) => {
      supersede(row, name);
      const given = sampled.clamp(row, args?.[0] !== undefined ? object(args[0], row.options) : {}, say);
      const timeout = row.send ? timeoutOf(row, args) : null, reader = row.send ? null : name;
      const origin = originNow();
      let d = admitted(row, given, origin, row.send, timeout);
      const n = (asked.get(name) ?? 0) + 1;
      asked.set(name, n);
      if (reader && sampled.sampled(row)) d = sampledAnswer(row, d, given, reader);
      if (d.forward && row.send && row.sub.gesture && ready.has(extensionOf(row))) {
        const impl = ready.get(extensionOf(row)), declared = row.key.slice(row.key.indexOf(".") + 1);
        if (impl?.[declared]) return Promise.resolve(impl[declared](given, { origin, request: ++requests })).then(json => { handled(row, given, json); return answer(row, json); });
      }
      if (d.forward) return viaImplementation(row, given, reader, { name, origin }).then(json => { disposeDropped(row, json, name, n); if (reader) sampled.answered(row, reader, json); return answer(row, json); });
      if (d.hold) return hold(row, d.hold, name, false, timeout, route(row) === "agent" ? sheet => viaImplementation(row, given, reader, { name, sheet, origin }) : null, given);
      // A send refused now fails its mutation, as on native, rather than the action that sent it.
      return row.send && errorCode(d.now) ? Promise.resolve().then(() => answer(row, d.now)) : answer(row, d.now);
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
      if (h.permission !== name) continue;
      if (state === "denied") { release(h, { error: permissionNamed(h.row, name).denied }); continue; }
      if (state !== "granted" || h.waits !== "permission") continue;
      // A call that needs another permission too asks it next (`getUserMedia(audio=true, video=true)`).
      const next = permissionOf(h.row, h.args);
      if (next && next.name !== name) {
        const d = decidePermission(h.row, h.send, next);
        if (d?.now) { release(h, d.now); continue; }
        if (d?.hold) { h.permission = next.name; h.ticket = nextTicket(); say(`device ${capability(h)} ${h.ticket} held (agent)`); continue; }
      }
      if (h.forward) { releaseForward(h); continue; }
      const d = decideTimed(h.row, h.send, h.timeout);
      if (d.hold) { h.waits = d.hold; h.ticket = nextTicket(); h.deadline = d.hold === "value" && h.timeout != null ? clock.now + h.timeout : null; say(`device ${capability(h)} ${h.ticket} held (agent)`); } else release(h, d.now);
    }
    settleEffects(name, state);
    reask([...byKey.values()].filter(r => permissionNamed(r, name)).map(r => r.key));
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
    // A polled source's ask in flight: what a jump waits for (agent.js).
    polling: () => asking.size > 0,
    // `state.extensions` (LLP 1133.002 §6), as the runner's `extensions_json` writes it.
    state() {
      const out = {};
      for (const row of byKey.values()) {
        const [extension, declared] = [row.key.slice(0, row.key.indexOf(".")), row.key.slice(row.key.indexOf(".") + 1)];
        const e = out[extension] ??= { bound: !clock.agent && !!bound[extension], preferred: {}, readers: {}, held: [] };
        if (route(row) === "agent") e.agent = true;
        // Each permission by name; `permission` too where it has one.
        for (const p of permissionsOf(row)) (e.permissions ??= {})[p.name] = stateOf(p.name);
        const names = Object.keys(e.permissions ?? {});
        if (names.length === 1) e.permission = e.permissions[names[0]]; else delete e.permission;
        if (preferred.has(row.key)) e.preferred[declared] = preferred.get(row.key);
        if (!row.send) e.readers[declared] = [...new Set([...Resources.filter(r => r.source === row.source || r.watches?.includes(row.key)).map(r => r.name), ...(globalThis.exact?.moduleReaders?.(row.key) ?? [])])];
        for (const h of held.filter(h => h.row === row && !h.queued)) e.held.push({ ticket: h.ticket, name: declared, origin: "app", waits: h.waits });
        for (const h of effects.filter(h => h.row === row)) e.held.push({ ticket: h.ticket, name: declared, origin: "app", waits: "permission" });
      }
      for (const q of unheard) {
        const extension = q.key.split(".")[0], e = out[extension] ??= { bound: false, preferred: {}, readers: {}, held: [] };
        (e.queued ??= []).push({ event: q.key.split(".")[1], args: q.args, origin: q.origin });
      }
      // What a loaded implementation discloses of its own (`state()`: a language model's sessions, LLP 1133.016 §9).
      for (const [extension, e] of Object.entries(out)) {
        Object.assign(e, sampled.state(extension));
        rc.state([...byKey.values()].filter(r => extensionOf(r) === extension), e);
        const own = ready.get(extension)?.state?.();
        if (own) e.implementation = own;
      }
      for (const [h, entry] of listed) (out[extensionOf(h.row)].ceremonies ??= []).push(entry);
      for (const [extension, c] of pictureCounts) if (out[extension]) out[extension].pictures = { ...c };
      return out;
    },
    // A package's emit, as its context raises it: the device's (the tests' way in, the agent binding no implementation).
    raiseForTests: (key, args = []) => raise(key, args, "device"),
    // Each declared list and value, by its state name (§7), for the agent's top-level state.
    declared() {
      const out = {};
      for (const row of byKey.values()) {
        if (row.sub.list) out[row.sub.list.state] = (lists.get(row.sub.list.state) ?? []).map(e => ({ ...e.fields, delivered: delivered(e), ref: e.ref, ...wk.fields(e) }));
        if (row.sub.set) out[row.sub.set.state] = setValues.get(`${extensionOf(row)}/${row.sub.set.state}`) ?? null;
      }
      return out;
    },
    // A relaunched page's launch flag (`--notification '<entry>'`): the entry as its tap would deliver it.
    // `state.wake` (LLP 1133.017 §10).
    wake: () => wk.state(),
    launch(req) {
      // `--wake '<entry>'`: a registration a cold wake runs for.
      if (req.launch === "wake") {
        let entry;
        try { entry = typeof req.entry === "string" ? JSON.parse(req.entry) : req.entry; } catch (e) { return { error: `--wake: not JSON: ${e.message}` }; }
        const row = [...byKey.values()].find(r => r.sub.list?.kind === entry?.kind && r.sub.list?.tap?.as === "wake");
        return row ? wk.launch(row, entry, nextTicket()) : { error: `--wake: no module of this app lists registrations of kind \`${entry?.kind}\`` };
      }
      const row = [...byKey.values()].find(r => r.sub.list?.launch === req.launch);
      if (!row) return { error: `--${req.launch}: no module of this app lists entries a launch hands over` };
      let entry;
      try { entry = typeof req.entry === "string" ? JSON.parse(req.entry) : req.entry; } catch (e) { return { error: `--${req.launch}: not JSON: ${e.message}` }; }
      say(`launch --${req.launch}`);
      return activate(row, entry ?? {}, 0);
    },
    // A call from data code (ts-data.js `__exact_module`): JSON, as the native runtime answers it.
    // `kept`: whether the answer that made the call still wants its reply; a registration it no longer does is disposed of (D11).
    call(body, kept = () => true) {
      const row = byKey.get(`${body.extension}.${body.source ?? body.send}`);
      if (!row || row.send !== !!body.send) return Promise.resolve({ error: "NotSupportedError" });
      const args = sampled.clamp(row, { ...(body.args ?? {}) }, say);
      const timeout = row.send ? timeoutOf(row, args) : null, reader = !row.send && sampled.sampled(row) ? `data ${groupOf(args)}` : null;
      const origin = body.origin === "tool" ? "tool" : originNow();
      if (row.sub.effect) return effect(row, args, origin);
      let d = admitted(row, args, origin, row.send, timeout);
      if (reader) d = sampledAnswer(row, d, args, reader);
      if (d.forward) return viaImplementation(row, args, null, { origin }).then(json => { if (reader) sampled.answered(row, reader, json); return complete(kept() ? json : ceremonies.dropped(row, json), row.type); });
      if (d.hold) return hold(row, d.hold, row.key, true, timeout, route(row) === "agent" ? sheet => viaImplementation(row, args, null, { origin, sheet }) : null, args, kept);
      say(`native ${row.key} (${route(row) === "substitute" ? "substituted" : "refused"})`);
      if (row.sub.ceremony || row.sub.records) ceremonies.record({ row, args }, errorCode(d.now) ?? "recorded");
      return Promise.resolve(complete(d.now, row.type));
    },
    holds: () => [...held.filter(h => !h.queued).map(h => ({ name: h.name, ticket: h.ticket, device: { capability: capability(h),
      args: h.waits === "permission" ? { capability: h.permission, request: h.row.key.split(".")[1], origin: "app" } : h.waits === "ceremony" ? ceremonies.args(h)
        // A sheet shows what the call asked (the properties, the event).
        : h.waits === "choose" ? { request: h.row.key.split(".")[1], origin: "app", options: h.args ?? {} } : { request: h.row.key.split(".")[1] } } })),
      ...effects.map(h => ({ name: h.row.key, ticket: h.ticket, device: { capability: "permission", args: { capability: h.row.sub.permission.name, request: h.row.key.split(".")[1], origin: "app" } } }))],
    // `tap @t allow|deny|cancel`, `type @t <value>`, `tap @ref` on a listed entry; undefined for a ticket that is not a module's.
    answer(req) {
      const entry = req.op === "tap" && [...lists.values()].flat().find(e => e.ref === req.ticket);
      if (entry) {
        if (!delivered(entry)) return { error: `@${req.ticket} is not delivered yet: advance the clock past its time` };
        // A registration's tap is a wake, and leaves it listed.
        if (entry.row.sub.list.tap?.as === "wake") return wk.tap(entry);
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
      if (h.waits === "ceremony") {
        const c = h.row.sub.ceremony;
        if (req.op === "tap") {
          const choices = Object.keys(c.choices ?? {});
          if (req.choice !== "cancel" && !choices.includes(req.choice)) return { error: `@${h.ticket} (${cap}): tap takes ${["cancel", ...choices].join(" | ")}, not ${req.choice}` };
          say(`device ${cap} ${h.ticket} ${req.choice === "cancel" ? "cancelled" : `answered: ${req.choice}`}`);
          release(h, (req.choice === "cancel" ? c.cancel : c.choices[req.choice]) ?? { error: "NotAllowedError" });
          return { ticket: req.ticket, capability: cap, answered: req.choice, delivery: "substituted" };
        }
        let v;
        try { v = JSON.parse(req.text); } catch {}
        if (!v || typeof v !== "object" || Array.isArray(v) || Object.entries(c.answer ?? {}).some(([k, x]) => v[k] !== x)) {
          return { error: `@${h.ticket}: ${h.row.key}: type the response's JSON, an object with ${JSON.stringify(c.answer ?? {})} (packages/passkeys/test/authenticator.mjs makes one from the ticket's args)` };
        }
        say(`device ${cap} ${h.ticket} answered: a value`);
        release(h, v);
        return { ticket: req.ticket, capability: cap, answered: "value", delivery: "substituted" };
      }
      // A sheet (`choose`, LLP 1133.018 D13 item 4), as the runner's `modules/permits.rs`: a tap of `cancel` or one of
      // its `choices`, or typed ids (`c2,c5`), more than one only where the call set the option `many` names; the
      // implementation answers, told the answer as the call's `preferred`.
      if (h.waits === "choose") {
        const c = h.row.sub.choose, choices = c.choices ?? [], ext = extensionOf(h.row);
        let sheet;
        if (req.op === "tap") {
          if (req.choice !== "cancel" && !choices.includes(req.choice)) return { error: `@${h.ticket} (${cap}): tap takes ${["cancel", ...choices].join(" | ")}, not ${req.choice}` };
          say(`device ${cap} ${h.ticket} ${req.choice === "cancel" ? "cancelled" : `answered: ${req.choice}`}`);
          sheet = { tap: req.choice };
        } else {
          if (c.typed === false) return { error: `@${h.ticket} (${cap}): answered by tap, not type` };
          let ids;
          try { ids = JSON.parse(req.text); } catch {}
          if (!Array.isArray(ids)) ids = String(req.text).split(",").map(s => s.trim()).filter(Boolean);
          if (c.many && ids.length > 1 && h.args?.[c.many] !== true) return { error: `@${h.ticket}: one id, not ${ids.length}: the call did not set \`${c.many}\`` };
          say(`device ${cap} ${h.ticket} answered: a value`);
          if (c.word) {
            preferred.set(`${ext}.${c.word}`, ids);
            say(`${ext}.${c.word}: chosen by the agent`);
            loaded.get(ext)?.then(impl => impl?.preferred?.(preferredOf(ext)));
          }
          sheet = { typed: ids };
        }
        if (h.forward) releaseForward(h, sheet); else release(h, {});
        return { ticket: req.ticket, capability: cap, answered: sheet.tap ?? "value", delivery: "substituted" };
      }
      if (h.waits === "permission") {
        if (req.op !== "tap") return { error: `@${h.ticket} (permission): answered by tap, not type` };
        if (!["allow", "deny", "cancel"].includes(req.choice)) return { error: `@${h.ticket} (permission): tap takes cancel | allow | deny, not ${req.choice}` };
        say(`device permission ${h.ticket} ${req.choice === "cancel" ? "cancelled" : `answered: ${req.choice}`}`);
        if (req.choice === "allow") setPermission(h.permission, "granted");
        else if (req.choice === "deny") setPermission(h.permission, "denied");
        else release(h, { error: permissionNamed(h.row, h.permission).denied });
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
        if (![...byKey.values()].some(r => permissionNamed(r, name))) return { error: `prefer permission: no module here asks for \`${name}\`` };
        if (!["granted", "denied", "prompt"].includes(state)) return { error: `prefer permission.${name}: granted, denied or prompt` };
      }
      const group = req.extension ?? {};
      for (const [key, v] of Object.entries(group)) {
        const word = agentWord(key);
        if (word) { if (v !== "clear" && !wordFits(word, v)) return { error: `prefer extension: \`${key}\` is a ${word}` }; continue; }
        const row = byKey.get(key);
        const sources = [...byKey.values()].filter(r => !r.send).map(r => r.key);
        if (!row) return { error: `prefer extension: no source \`${key}\` in this app; its module sources are ${sources.join(", ") || "none"}` };
        if (row.send) return { error: `prefer extension: \`${key}\` is a send, which the module answers` };
        const p = row.sub.permission, field = p && isStatus(row) ? p.field : null;
        if (field && v && typeof v === "object" && field in v) return { error: `prefer extension: \`${key}\`'s \`${field}\` is set by \`prefer permission.${p.name}\`` };
        // An implementation's own substitute takes a partial overlay (LLP 1133.009 D10 item 11); a declared one, a value.
        if (v !== "clear" && !errorCode(v)) {
          try {
            if (row.sub.implementation || row.sub.partial) partial(v, row.type, row.key);
            else if (sampled.sampled(row)) sampled.check(row, v, (reading, key) => value(filled(row, reading), row.type, key));
            else typed(row, JSON.stringify(field ? { ...v, [field]: stateOf(p.name) } : v));
          } catch (e) { return { error: `prefer extension: ${e.message}` }; }
        }
      }
      for (const [name, state] of Object.entries(req.permission ?? {})) setPermission(name, state);
      for (const [key, v] of Object.entries(group)) {
        if (v === "clear") preferred.delete(key); else preferred.set(key, v);
        if (byKey.has(key)) sampled.prefer(byKey.get(key), v);
        say(`prefer extension ${key}`);
      }
      // An implementation bound under the agent is told its names' preferred values (LLP 1133.008 D8 item 9).
      for (const extension of new Set(Object.keys(group).map(k => byKey.get(k) ?? rowOf(k)).filter(r => r && route(r) === "agent").map(extensionOf))) {
        loaded.get(extension)?.then(impl => impl?.preferred?.(preferredOf(extension)));
      }
      settleValues();
      const keys = Object.keys(group).filter(k => byKey.has(k));
      if (keys.length) reask(keys);
      return { extension: Object.fromEntries(preferred), permission: Object.fromEntries(permission) };
    },
  };
}

/** Whether a call gave option `arg`: set and not `false`; the grant key `"*"` is every call's. */
const given = (args, arg) => arg === "*" || (args?.[arg] != null && args[arg] !== false);

/** `app:/<root>/<file…>` with no empty, `.` or `..` segment (the runner's `plain_app_path`). */
const appPath = p => p.startsWith("app:/") && p.slice(5).split("/").length >= 2 && p.slice(5).split("/").every(s => s && s !== "." && s !== ".." && !s.includes("\0"));

/** HTML's activation-triggering input events, and `click`: what a person's activation is. */
const ACTIVATING = new Set(["click", "keydown", "mousedown", "pointerdown", "pointerup", "touchend"]);

/** The code of an error answer, `{"error": "POSITION_UNAVAILABLE"}`, which may say more in a `message`, and each
 * platform's own code beside it (`apple`, `android`; LLP 1133.011 D1), as the runner's `error_code`. */
function errorCode(json) {
  if (json === null || typeof json !== "object" || Array.isArray(json)) return null;
  const keys = Object.keys(json);
  return typeof json.error === "string" && keys.every(k => ["error", "message", "apple", "android"].includes(k)) ? json.error : null;
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

/** The strings at `path` in `v`: `a.b`, and `a[].b` for each element of the list `a`. */
function handlesAt(v, path) {
  const [head, ...rest] = path.split("."), each = head.endsWith("[]"), at = v?.[each ? head.slice(0, -2) : head];
  const items = each ? (Array.isArray(at) ? at : []) : [at];
  return items.flatMap(x => rest.length ? handlesAt(x, rest.join(".")) : typeof x === "string" ? [x] : []);
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
