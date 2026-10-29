// @exact/usb on the web (LLP 1133.037 D13): WebUSB (https://wicg.github.io/webusb/), the browser's chooser and its
// remembered devices, in the interface's shapes. The page runtime (host/web-js/modules.js) calls it outside the agent.
// A device is opened once and each `open` is a lease on it (D5); a claim belongs to the connection that made it, and
// another connection's claim of the same interface fails `NetworkError`, as a second tab's does.

const b64 = bytes => { let s = ''; for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000)); return btoa(s); };
const unb64 = text => Uint8Array.from(atob(text ?? ''), c => c.charCodeAt(0));
const failure = (e, fallback = 'NetworkError') => ({ error: typeof e?.name === 'string' && e.name !== 'Error' ? e.name : fallback, message: String(e?.message ?? e) });
const clean = f => Object.fromEntries(Object.entries(f ?? {}).filter(([, v]) => v != null));

export function implementation({ changed }) {
  const usb = typeof navigator === 'object' ? navigator.usb : undefined;
  const ids = new Map(), devices = new Map(), connections = new Map(), leases = new Map();
  let next = 0, nextConnection = 0;
  const idOf = d => { if (!ids.has(d)) { const id = `usb-device-${++next}`; ids.set(d, id); devices.set(id, d); } return ids.get(d); };
  const record = d => ({
    id: idOf(d),
    usbVersionMajor: d.usbVersionMajor, usbVersionMinor: d.usbVersionMinor, usbVersionSubminor: d.usbVersionSubminor,
    deviceClass: d.deviceClass, deviceSubclass: d.deviceSubclass, deviceProtocol: d.deviceProtocol,
    vendorId: d.vendorId, productId: d.productId,
    deviceVersionMajor: d.deviceVersionMajor, deviceVersionMinor: d.deviceVersionMinor, deviceVersionSubminor: d.deviceVersionSubminor,
    manufacturerName: d.manufacturerName ?? '', productName: d.productName ?? '', serialNumber: d.serialNumber ?? null,
    configuration: d.configuration?.configurationValue ?? null,
    configurations: (d.configurations ?? []).map(c => ({
      configurationValue: c.configurationValue, configurationName: c.configurationName ?? null,
      interfaces: c.interfaces.map(i => ({
        interfaceNumber: i.interfaceNumber,
        alternates: i.alternates.map(a => ({
          alternateSetting: a.alternateSetting, interfaceClass: a.interfaceClass, interfaceSubclass: a.interfaceSubclass,
          interfaceProtocol: a.interfaceProtocol, interfaceName: a.interfaceName ?? null,
          endpoints: a.endpoints.map(e => ({ endpointNumber: e.endpointNumber, direction: e.direction, type: e.type, packetSize: e.packetSize })),
        })),
      })),
    })),
    connected: true, granted: true,
  });
  const announce = () => { changed('usb.getDevices'); changed('usb.usbConnection'); };
  if (usb?.addEventListener) {
    usb.addEventListener('connect', announce);
    usb.addEventListener('disconnect', e => {
      for (const c of connections.values()) if (c.device === e.device) { c.unplugged = true; c.open = false; }
      leases.delete(e.device);
      announce();
    });
  }
  const connection = args => {
    const c = connections.get(args?.connection);
    if (!c) return { error: { error: 'InvalidStateError', message: 'no such connection' } };
    if (!c.open) return { error: { error: c.unplugged ? 'NetworkError' : 'InvalidStateError' } };
    return { c };
  };
  const claimedElsewhere = (c, n) => [...connections.values()].some(o => o !== c && o.open && o.device === c.device && o.claimed.has(n));
  const call = async (args, run) => {
    const { c, error } = connection(args);
    if (error) return error;
    try { return (await run(c)) ?? null; } catch (e) { return failure(e); }
  };
  const result = r => ({ status: r.status, ...(r.data ? { data: b64(new Uint8Array(r.data.buffer, r.data.byteOffset, r.data.byteLength)) } : {}), ...(r.bytesWritten != null ? { bytesWritten: r.bytesWritten } : {}) });

  return {
    usbStatus() { return { available: !!usb, attachedList: false, state: usb ? 'granted' : 'denied' }; },
    async requestDevice(args) {
      if (!usb) return { error: 'NotSupportedError' };
      try {
        const d = await usb.requestDevice({ filters: (args?.filters ?? []).map(clean), exclusionFilters: (args?.exclusionFilters ?? []).map(clean) });
        changed('usb.getDevices');
        return record(d);
      } catch (e) {
        return failure(e, 'NotFoundError');
      }
    },
    async getDevices() { return usb ? (await usb.getDevices()).map(record) : []; },
    attachedDevices() { return { error: 'NotSupportedError' }; },
    async open(args) {
      const d = devices.get(args?.device);
      if (!d) return { error: 'NotFoundError' };
      try { if (!d.opened) await d.open(); } catch (e) { return failure(e); }
      leases.set(d, (leases.get(d) ?? 0) + 1);
      const handle = `usb-${++nextConnection}`;
      connections.set(handle, { device: d, open: true, unplugged: false, claimed: new Set() });
      announce();
      return { connection: handle, device: record(d) };
    },
    usbConnection(args) {
      const c = connections.get(args?.connection);
      return c ? { open: c.open, connected: !c.unplugged, configuration: c.device.configuration?.configurationValue ?? null, claimed: [...c.claimed] } : { open: false, connected: false, configuration: null, claimed: [] };
    },
    selectConfiguration: args => call(args, c => c.device.selectConfiguration(args.configurationValue)),
    claimInterface: args => call(args, async c => {
      if (claimedElsewhere(c, args.interfaceNumber)) return { error: 'NetworkError', message: 'claimed by another connection' };
      if (!c.device.configuration?.interfaces.find(i => i.interfaceNumber === args.interfaceNumber)?.claimed) await c.device.claimInterface(args.interfaceNumber);
      c.claimed.add(args.interfaceNumber);
      announce();
    }),
    releaseInterface: args => call(args, async c => {
      if (!c.claimed.has(args.interfaceNumber)) return { error: 'InvalidStateError' };
      await c.device.releaseInterface(args.interfaceNumber);
      c.claimed.delete(args.interfaceNumber);
      announce();
    }),
    selectAlternateInterface: args => call(args, c => c.claimed.has(args.interfaceNumber) ? c.device.selectAlternateInterface(args.interfaceNumber, args.alternateSetting) : { error: 'InvalidStateError' }),
    controlTransferIn: args => call(args, async c => result(await c.device.controlTransferIn(args.setup, args.length))),
    controlTransferOut: args => call(args, async c => result(await c.device.controlTransferOut(args.setup, unb64(args.data)))),
    transferIn: args => call(args, async c => result(await c.device.transferIn(args.endpointNumber, args.length))),
    transferOut: args => call(args, async c => result(await c.device.transferOut(args.endpointNumber, unb64(args.data)))),
    clearHalt: args => call(args, c => c.device.clearHalt(args.direction, args.endpointNumber)),
    reset: args => call(args, c => c.device.reset()),
    async close(args) {
      const c = connections.get(args?.connection);
      if (!c || !c.open) return { error: 'InvalidStateError' };
      c.open = false;
      for (const n of c.claimed) await c.device.releaseInterface(n).catch(() => {});
      c.claimed.clear();
      const left = (leases.get(c.device) ?? 1) - 1;
      leases.set(c.device, left);
      // The device closes with its last lease (D5).
      if (left === 0) { leases.delete(c.device); await c.device.close().catch(() => {}); }
      announce();
      return null;
    },
    async forget(args) {
      const d = devices.get(args?.device);
      if (!d) return null;
      for (const c of connections.values()) if (c.device === d) c.open = false;
      leases.delete(d);
      try { await d.forget?.(); } catch (e) { return failure(e); }
      announce();
      return null;
    },
    release() {},
  };
}
