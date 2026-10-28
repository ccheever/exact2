// A module of devices a person plugs in, under the agent's substitute on the JS target (LLP 1133.037 §11), as the
// runner's `modules/devices.rs`: the interface's `devices` names the agent word listing the devices, the handle kind,
// whether an open is exclusive and the record's fields; each name's `device.role` says what it does. The web has no
// attached list (`attachedPorts` is refused by its `unsupported`) and no permission at open, so neither is here.

const b64 = bytes => btoa(String.fromCharCode(...bytes));
const unb64 = text => { try { return Uint8Array.from(atob(text), c => c.charCodeAt(0)); } catch { return null; } };

/** `at` with each `{path}` replaced by the call's value there. */
const template = (at, args) => at.replace(/\{([^}]+)\}/g, (_, path) => String(path.split(".").reduce((v, k) => v?.[k], args)));

/** `env`: {byKey, preferred, say, clock, reask, polled(row)}. */
export function devices(env) {
  const { byKey, preferred, say, clock } = env;
  // The remembered devices outlive a reload, as the browser's chosen ones do: kept in the page's storage.
  const KEPT = 'exact.devices.remembered';
  const remembered = new Map(), connections = new Map(), writes = new Map(), sequences = new Map();
  try { for (const [e, ids] of Object.entries(JSON.parse(globalThis.localStorage?.getItem(KEPT) ?? '{}'))) remembered.set(e, ids); } catch { /* none kept */ }
  const keep = () => { try { globalThis.localStorage?.setItem(KEPT, JSON.stringify(Object.fromEntries(remembered))); } catch { /* not kept */ } };
  let next = 0;
  const extensionOf = row => row.key.slice(0, row.key.indexOf("."));
  const declaredOf = row => row.key.slice(row.key.indexOf(".") + 1);
  const roleOf = row => row.sub.device?.role ? row.sub.device : null;
  const fixtures = row => preferred.get(`${extensionOf(row)}.${row.sub.devices?.word ?? "devices"}`) ?? [];
  const fixture = (row, id) => fixtures(row).find(d => d.id === id);
  const rowOf = extension => [...byKey.values()].find(r => extensionOf(r) === extension && r.sub.devices);
  const record = (row, f) => {
    const out = {}, d = row.sub.devices ?? {};
    for (const field of d.record ?? []) { const v = f[field] ?? d.fill?.[field]; if (v != null) out[field] = v; }
    return out;
  };
  const changed = extension => env.reask([...byKey.values()].filter(r => extensionOf(r) === extension && r.live && !r.send).map(r => r.key));
  const remember = (extension, id) => { const l = remembered.get(extension) ?? []; if (!l.includes(id)) l.push(id); remembered.set(extension, l); keep(); };
  const live = c => { const f = fixture(rowOf(c.extension), c.device); return f && f.connected !== false; };
  const connectionOf = args => {
    const c = connections.get(args.connection);
    if (!c) return { error: { error: "InvalidStateError", message: "no such connection" } };
    if (!c.open) return { error: { error: "InvalidStateError", message: "closed" } };
    if (!live(c)) return { error: { error: "NetworkError", message: "the device is not connected" } };
    return { c, f: fixture(rowOf(c.extension), c.device) };
  };
  const claimRule = (row, d, args, c) => {
    const unclaimed = { error: "InvalidStateError", message: "the interface is not claimed" };
    if (d.claims === true && [...connections].some(([h, o]) => h !== args.connection && o.open && o.extension === c.extension && o.device === c.device && o.claimed.has(args.interfaceNumber))) {
      return { error: "NetworkError", message: "claimed by another connection" };
    }
    if (d.claimed && !c.claimed.has(args[d.claimed])) return unclaimed;
    if (d.recipient) { const s = args[d.recipient]; if (s?.recipient === "interface" && !c.claimed.has(s.index & 0xff)) return unclaimed; }
    if (typeof args.endpointNumber === "number" && declaredOf(row) !== "clearHalt" && c.claimed.size === 0) return unclaimed;
    return null;
  };
  const open = (row, d, args) => {
    const by = d.by ?? "device", id = args[by], extension = extensionOf(row), f = fixture(row, id);
    if (!f) return { error: "NotFoundError" };
    if (f.connected === false) return { error: "NetworkError" };
    if (row.sub.devices?.exclusive && [...connections.values()].some(c => c.open && c.extension === extension && c.device === id)) {
      return { error: "InvalidStateError", message: "already open" };
    }
    const handle = `${row.sub.devices?.kind ?? "device"}-${++next}`;
    const options = d.options ? { ...d.options } : null;
    if (options) for (const [k, v] of Object.entries(args)) if (k !== by && v != null) options[k] = v;
    connections.set(handle, { extension, device: id, open: true, opened: clock.now, options, configuration: null, claimed: new Set(), delivered: 0, position: 0, ended: false, readers: new Map() });
    remember(extension, id);
    say(`${extension}: opened ${id} as ${handle}`);
    changed(extension);
    const out = { connection: handle, [by]: record(row, f) };
    if (options) out.options = options;
    return out;
  };
  const closed = c => { c.open = false; c.claimed.clear(); };
  const answer = (row, d, args) => {
    const extension = extensionOf(row);
    switch (d.role) {
      case "remembered": return (remembered.get(extension) ?? []).map(id => { const f = fixture(row, id); return { ...(row.sub.devices?.fill ?? {}), ...(f ? record(row, f) : { id }), connected: !!f && f.connected !== false }; });
      case "attached": return fixtures(row).filter(f => f.connected !== false).map(f => record(row, f));
      case "connection": {
        // USB's configuration and claims besides, where `devices.connection` names them.
        const extra = row.sub.devices?.connection ?? [], c = connections.get(args.connection), here = !!c && live(c);
        const out = { open: !!c && c.open && here, connected: here };
        if (c?.options) out.options = c.options;
        if (extra.includes('claimed')) out.claimed = c ? [...c.claimed] : [];
        if (extra.includes('configuration') && c?.configuration != null) out.configuration = c.configuration;
        return out;
      }
      case "write": {
        const { c, error } = connectionOf(args);
        if (error) return error;
        const refused = claimRule(row, d, args, c);
        if (refused) return refused;
        const entry = { name: declaredOf(row), device: c.device, at: clock.now };
        for (const [k, v] of Object.entries(args)) if (v != null) entry[k] = v;
        writes.set(extension, [...(writes.get(extension) ?? []), entry]);
        if (d.sets) c.configuration = args.configurationValue;
        if (d.claims === true) c.claimed.add(args.interfaceNumber);
        if (d.claims === false) c.claimed.delete(args.interfaceNumber);
        say(`${extension}: ${declaredOf(row)} to ${c.device}`);
        changed(extension);
        return d.answer === "written" ? { status: "ok", bytesWritten: (unb64(args.data ?? "") ?? []).length } : null;
      }
      case "keyed": {
        const { c, f, error } = connectionOf(args);
        if (error) return error;
        const refused = claimRule(row, d, args, c);
        if (refused) return refused;
        const path = template(d.at ?? "", args), at = path.split(".").reduce((v, k) => v?.[k], f);
        let value = at;
        if (Array.isArray(at) && at.length) {
          const seen = sequences.get(extension) ?? new Map(), key = `${c.device} ${path}`, n = seen.get(key) ?? 0;
          value = at[Math.min(n, at.length - 1)];
          seen.set(key, n + 1);
          sequences.set(extension, seen);
        } else if (at == null) return d.missing;
        return d.wrap && (value === null || typeof value !== "object") ? { [d.wrap]: value } : value;
      }
      case "stream": {
        const c = connections.get(args.connection);
        if (!c) return { error: "InvalidStateError", message: "no such connection" };
        if (c.ended) return { error: "InvalidStateError", message: "the stream has ended" };
        const f = fixture(row, c.device), unplugged = !f || f.connected === false;
        const events = f?.[d.events ?? "input"] ?? [], since = clock.now - c.opened, bytes = [], errors = [];
        while (c.delivered < events.length && (events[c.delivered].at ?? 0) <= since) {
          const e = events[c.delivered++], data = e.data != null ? unb64(e.data) : null;
          if (data) { c.position += data.length; bytes.push(...data); }
          if (e.error) errors.push({ at: c.position, code: e.error });
        }
        const done = !c.open || unplugged;
        if (unplugged && c.open) errors.push({ at: c.position, code: "NetworkError" });
        c.ended = done;
        env.polled(row);
        return { data: b64(bytes), errors, done, position: c.position };
      }
      case "reports": {
        const c = connections.get(args.connection);
        if (!c) return { error: "InvalidStateError", message: "no such connection" };
        const f = fixture(row, c.device), since = clock.now - c.opened;
        const arrived = (f?.[d.events ?? "inputReports"] ?? []).filter(e => (e.at ?? 0) <= since && (args.reportId == null || e.reportId === args.reportId));
        const reader = JSON.stringify(Object.fromEntries(Object.entries(args).filter(([k, v]) => k !== "after" && v != null).sort()));
        const history = args.history ?? d.history ?? 16, first = !c.readers.has(reader);
        let fresh = first ? arrived.slice(-1) : arrived.slice(c.readers.get(reader)), dropped = 0;
        if (!first && fresh.length > history) { dropped = fresh.length - history; fresh = fresh.slice(dropped); }
        c.readers.set(reader, arrived.length);
        env.polled(row);
        return { reports: fresh.map(e => ({ reportId: e.reportId ?? 0, data: e.data ?? "", at: c.opened + (e.at ?? 0) })), dropped, interrupted: !f || f.connected === false };
      }
      case "close": {
        const c = connections.get(args.connection);
        if (!c || !c.open) return { error: "InvalidStateError" };
        closed(c);
        say(`${extension}: closed ${args.connection}`);
        changed(extension);
        return null;
      }
      case "forget": {
        const id = args.port ?? args.device;
        remembered.set(extension, (remembered.get(extension) ?? []).filter(x => x !== id));
        keep();
        for (const c of connections.values()) if (c.extension === extension && c.device === id) closed(c);
        say(`${extension}: forgot ${id}`);
        changed(extension);
        return null;
      }
    }
    return { error: "NotSupportedError" };
  };
  return {
    role: roleOf,
    /** What a device name answers now: `{now}`, `{hold: "device"}` for the chooser, or null for another name. */
    decide(row, args) {
      const d = roleOf(row);
      if (!d) return null;
      if (d.role === "request") return { hold: "device" };
      return { now: d.role === "open" ? open(row, d, args) : answer(row, d, args) };
    },
    /** A send that answers nothing: null when done, or its refusal; undefined for another name. */
    effect(row, args) { const d = roleOf(row); return d ? answer(row, d, args) : undefined; },
    /** The chooser's ticket summary and taps. */
    ticket(h) {
      const ids = fixtures(h.row).filter(f => f.connected !== false).map(f => f.id);
      return { args: { request: declaredOf(h.row), origin: "app", options: h.args ?? {}, devices: ids }, choices: ids };
    },
    /** The chooser answered: the chosen record (a list of it, `many`), or `cancel`'s answer. */
    chosen(h, choice) {
      const d = roleOf(h.row);
      if (choice === "cancel") return d.cancel;
      const f = fixture(h.row, choice);
      remember(extensionOf(h.row), choice);
      changed(extensionOf(h.row));
      const r = record(h.row, f);
      return d.many ? [r] : r;
    },
    /** A preferred agent word that lists an extension's devices: its sources asked again. */
    preferred(key) {
      const [extension, word] = [key.slice(0, key.indexOf(".")), key.slice(key.indexOf(".") + 1)];
      if ([...byKey.values()].some(r => extensionOf(r) === extension && r.sub.devices?.word === word)) changed(extension);
    },
    /** `state.extensions.<module>`'s device fields. */
    state(extension, e) {
      if (remembered.has(extension)) e.remembered = remembered.get(extension);
      const list = [...connections].filter(([, c]) => c.extension === extension).map(([handle, c]) => ({ handle, device: c.device, open: c.open, claimed: [...c.claimed], delivered: c.delivered, position: c.position, readers: Object.fromEntries(c.readers) }));
      if (list.length) e.connections = list;
      if (writes.has(extension)) e.writes = writes.get(extension);
      if (sequences.has(extension)) e.sequences = Object.fromEntries(sequences.get(extension));
    },
  };
}
