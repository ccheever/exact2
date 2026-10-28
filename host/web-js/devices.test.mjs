// devices.js, the JS target's substitute for serial ports, HID and USB devices (LLP 1133.037 §11), against the
// packages' own declarations: the same answers the runner's `modules/devices.rs` gives (contract/cli/tests/it/devices.rs).
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { devices } from './devices.js';

const declared = name => JSON.parse(readFileSync(new URL(`../../packages/${name}/exact.json`, import.meta.url), 'utf8'));

/** Each name's row as the bake writes it: the interface's `devices` copied into its substitute. */
function rows(...names) {
  const byKey = new Map();
  for (const name of names) {
    const i = declared(name);
    for (const n of i.names) {
      const declaredName = n.source ?? n.send;
      byKey.set(`${name}.${declaredName}`, { key: `${name}.${declaredName}`, send: !!n.send, live: !!n.live, sub: { ...(n.substitute ?? {}), devices: i.devices } });
    }
  }
  return byKey;
}

function world() {
  const byKey = rows('serial', 'hid', 'usb'), preferred = new Map(), asked = [], clock = { now: 0 };
  const dv = devices({ byKey, preferred, say: () => {}, clock, reask: keys => asked.push(...keys), polled: () => {} });
  const row = key => byKey.get(key);
  const call = (key, args = {}) => { const d = dv.decide(row(key), args); return d?.hold ? d : d?.now; };
  const effect = (key, args) => dv.effect(row(key), args);
  return { dv, preferred, clock, asked, row, call, effect };
}

const b64 = s => btoa(s);

test('a port streams each byte once, with errors at their offsets, and ends unplugged', () => {
  const w = world();
  w.preferred.set('serial.ports', [{ id: 's3', name: 'USB JTAG/serial debug unit', input: [{ at: 0, data: b64('boot\r\n') }, { at: 200, data: b64('caf') }, { at: 300, error: 'BreakError' }, { at: 400, data: b64('e\r\n') }] }]);
  w.dv.preferred('serial.ports');
  expect(w.asked).toContain('serial.getPorts');
  const opened = w.call('serial.open', { port: 's3', baudRate: 115200 });
  expect(opened.connection).toBe('serial-1');
  expect(opened.options.dataBits).toBe(8);
  expect(w.call('serial.open', { port: 's3', baudRate: 115200 }).error).toBe('InvalidStateError');
  const read = () => w.call('serial.serialInput', { connection: 'serial-1' });
  expect(atob(read().data)).toBe('boot\r\n');
  expect(read().data).toBe('');
  w.clock.now = 450;
  const next = read();
  expect(atob(next.data)).toBe('cafe\r\n');
  expect(next.errors).toEqual([{ at: 9, code: 'BreakError' }]);
  expect(w.effect('serial.write', { connection: 'serial-1', data: b64('help\r\n') })).toBe(null);
  w.preferred.set('serial.ports', [{ id: 's3', connected: false }]);
  const end = read();
  expect(end.done).toBe(true);
  expect(end.errors[0].code).toBe('NetworkError');
  expect(read().error).toBe('InvalidStateError');
  expect(w.call('serial.getPorts')).toEqual([{ id: 's3', name: '', connected: false, granted: true }]);
});

test('the chooser offers the preferred devices and remembers the one chosen', () => {
  const w = world();
  w.preferred.set('hid.devices', [{ id: 'b1', vendorId: 10168, productId: 493, productName: 'blink(1) mk3' }]);
  const held = w.call('hid.requestDevice', { filters: [] });
  expect(held.hold).toBe('device');
  const h = { row: w.row('hid.requestDevice'), args: {} };
  expect(w.dv.ticket(h).choices).toEqual(['b1']);
  expect(w.dv.chosen(h, 'cancel')).toEqual([]);
  expect(w.dv.chosen(h, 'b1')[0].productName).toBe('blink(1) mk3');
  expect(w.call('hid.getDevices').map(d => d.id)).toEqual(['b1']);
});

test('HID reports are a batch, and a USB control key answers its list in turn', () => {
  const w = world();
  w.preferred.set('hid.devices', [{ id: 'b1', vendorId: 1, productId: 2, inputReports: [{ at: 0, reportId: 1, data: 'AQ==' }, { at: 10, reportId: 1, data: 'Ag==' }, { at: 300, reportId: 1, data: 'Aw==' }] }]);
  const c = w.call('hid.open', { device: 'b1' }).connection;
  w.clock.now = 100;
  expect(w.call('hid.inputReports', { connection: c }).reports.map(r => r.data)).toEqual(['Ag==']);
  w.clock.now = 400;
  expect(w.call('hid.inputReports', { connection: c }).reports.map(r => r.data)).toEqual(['Aw==']);
  w.preferred.set('usb.devices', [{ id: 'stm', vendorId: 1155, productId: 57105, control: { 'in class interface 3 0 0': [{ status: 'ok', data: 'AA==' }, { status: 'ok', data: 'AQ==' }] } }]);
  const u = w.call('usb.open', { device: 'stm' }).connection;
  const setup = { requestType: 'class', recipient: 'interface', request: 3, value: 0, index: 0 };
  expect(w.call('usb.controlTransferIn', { connection: u, setup, length: 6 }).error).toBe('InvalidStateError');
  expect(w.effect('usb.claimInterface', { connection: u, interfaceNumber: 0 })).toBe(null);
  expect([1, 2, 3].map(() => w.call('usb.controlTransferIn', { connection: u, setup, length: 6 }).data)).toEqual(['AA==', 'AQ==', 'AQ==']);
  expect(w.call('usb.controlTransferOut', { connection: u, setup, data: 'AAEC' })).toEqual({ status: 'ok', bytesWritten: 3 });
});
