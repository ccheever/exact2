// @exact/hid on the web (LLP 1133.037 D10–D12): WebHID (https://wicg.github.io/webhid/), the browser's chooser and
// its remembered devices, in the interface's shapes. The page runtime (host/web-js/modules.js) calls it outside the
// agent.
//
// - Game controllers are `@exact/gamepad`'s (D11): the chooser excludes the pad usages, and a device holding a pad
//   collection is never offered. WebHID applies its own blocklist.
// - Input reports are a batch (D12): each connection keeps its recent reports, and each reader its cursor; the first
//   answer is the latest, each later one every report since, at most `history`, `dropped` counting the rest.

/** Generic Desktop's joystick, gamepad and multi-axis controller (HID Usage Tables §4). */
export const PADS = [0x04, 0x05, 0x08].map(usage => ({ usagePage: 0x01, usage }));
const KEPT = 1024;

const b64 = bytes => btoa(String.fromCharCode(...bytes));
const unb64 = text => Uint8Array.from(atob(text), c => c.charCodeAt(0));
const failure = (e, fallback = 'NetworkError') => ({ error: typeof e?.name === 'string' && e.name !== 'Error' ? e.name : fallback, message: String(e?.message ?? e) });
const clean = f => Object.fromEntries(Object.entries(f ?? {}).filter(([, v]) => v != null));

/** Whether a device holds a game controller's collection. */
export const isPad = device => (device.collections ?? []).some(c => PADS.some(p => p.usagePage === c.usagePage && p.usage === c.usage));

export function implementation({ changed }) {
  const hid = typeof navigator === 'object' ? navigator.hid : undefined;
  const ids = new Map(), devices = new Map(), connections = new Map();
  let next = 0, nextConnection = 0;
  const idOf = d => { if (!ids.has(d)) { const id = `hid-device-${++next}`; ids.set(d, id); devices.set(id, d); } return ids.get(d); };
  const reports = list => (list ?? []).map(r => ({ reportId: r.reportId }));
  const record = d => ({
    id: idOf(d), vendorId: d.vendorId, productId: d.productId, productName: d.productName ?? '',
    collections: (d.collections ?? []).map(c => ({ usagePage: c.usagePage, usage: c.usage, type: c.type ?? null, inputReports: reports(c.inputReports), outputReports: reports(c.outputReports), featureReports: reports(c.featureReports) })),
    connected: true, granted: true,
  });
  const announce = () => { changed('hid.getDevices'); changed('hid.hidConnection'); };
  if (hid?.addEventListener) {
    hid.addEventListener('connect', announce);
    hid.addEventListener('disconnect', e => {
      for (const c of connections.values()) if (c.device === e.device) { c.unplugged = true; c.open = false; }
      announce();
      changed('hid.inputReports');
    });
  }
  const connection = args => {
    const c = connections.get(args?.connection);
    if (!c) return { error: { error: 'InvalidStateError', message: 'no such connection' } };
    if (!c.open) return { error: { error: c.unplugged ? 'NetworkError' : 'InvalidStateError' } };
    return { c };
  };
  /** The shared open of a device and its report log: one per device, a lease per connection (D5). */
  const shared = new Map();
  async function lease(d) {
    let s = shared.get(d);
    if (!s) {
      if (!d.opened) await d.open();
      s = { log: [], sequence: 0, leases: 0 };
      d.addEventListener('inputreport', e => {
        s.log.push({ sequence: ++s.sequence, reportId: e.reportId, data: b64(new Uint8Array(e.data.buffer, e.data.byteOffset, e.data.byteLength)), at: performance.now() });
        if (s.log.length > KEPT) s.log.shift();
        changed('hid.inputReports');
      });
      shared.set(d, s);
    }
    s.leases++;
    return s;
  }

  return {
    hidStatus() { return { available: !!hid, attachedList: false, state: hid ? 'granted' : 'denied' }; },
    async requestDevice(args) {
      if (!hid) return { error: 'NotSupportedError' };
      try {
        const chosen = await hid.requestDevice({ filters: (args?.filters ?? []).map(clean), exclusionFilters: [...(args?.exclusionFilters ?? []).map(clean), ...PADS] });
        changed('hid.getDevices');
        return chosen.filter(d => !isPad(d)).map(record);
      } catch (e) {
        return failure(e, 'NotFoundError');
      }
    },
    async getDevices() { return hid ? (await hid.getDevices()).filter(d => !isPad(d)).map(record) : []; },
    attachedDevices() { return { error: 'NotSupportedError' }; },
    async open(args) {
      const d = devices.get(args?.device);
      if (!d || isPad(d)) return { error: 'NotFoundError' };
      let s;
      try { s = await lease(d); } catch (e) { return failure(e); }
      const handle = `hid-${++nextConnection}`;
      connections.set(handle, { device: d, shared: s, open: true, unplugged: false, readers: new Map() });
      announce();
      return { connection: handle, device: record(d) };
    },
    hidConnection(args) {
      const c = connections.get(args?.connection);
      return c ? { open: c.open, connected: !c.unplugged } : { open: false, connected: false };
    },
    inputReports(args) {
      const c = connections.get(args?.connection);
      if (!c) return { error: 'InvalidStateError', message: 'no such connection' };
      const reader = JSON.stringify([args.reportId ?? null, args.history ?? null, args.interval ?? null]);
      const history = Math.min(1024, args.history ?? 16), log = c.shared.log.filter(r => args.reportId == null || r.reportId === args.reportId);
      const cursor = c.readers.get(reader);
      let fresh = cursor === undefined ? log.slice(-1) : log.filter(r => r.sequence > cursor), dropped = 0;
      if (fresh.length > history) { dropped = fresh.length - history; fresh = fresh.slice(dropped); }
      c.readers.set(reader, c.shared.sequence);
      return { reports: fresh.map(({ reportId, data, at }) => ({ reportId, data, at })), dropped, interrupted: c.unplugged };
    },
    async sendReport(args) {
      const { c, error } = connection(args);
      if (error) return error;
      try { await c.device.sendReport(args.reportId, unb64(args.data)); return null; } catch (e) { return failure(e, 'NotAllowedError'); }
    },
    async sendFeatureReport(args) {
      const { c, error } = connection(args);
      if (error) return error;
      try { await c.device.sendFeatureReport(args.reportId, unb64(args.data)); return null; } catch (e) { return failure(e, 'NotAllowedError'); }
    },
    async receiveFeatureReport(args) {
      const { c, error } = connection(args);
      if (error) return error;
      try {
        const view = await c.device.receiveFeatureReport(args.reportId);
        return { data: b64(new Uint8Array(view.buffer, view.byteOffset, view.byteLength)) };
      } catch (e) {
        return failure(e, 'NotAllowedError');
      }
    },
    async close(args) {
      const c = connections.get(args?.connection);
      if (!c || !c.open) return { error: 'InvalidStateError' };
      c.open = false;
      // The device closes with its last lease.
      if (--c.shared.leases === 0) { shared.delete(c.device); try { await c.device.close(); } catch { /* gone */ } }
      announce();
      return null;
    },
    async forget(args) {
      const d = devices.get(args?.device);
      if (!d) return null;
      for (const c of connections.values()) if (c.device === d) c.open = false;
      shared.delete(d);
      try { await d.forget?.(); } catch (e) { return failure(e); }
      announce();
      return null;
    },
    release() {},
  };
}
