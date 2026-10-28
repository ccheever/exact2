// @exact/serial's web conformance suite in Chromium (LLP 1133.037 §16 stage 1), over a fake `navigator.serial`
// (`chromium.mjs`): the filters the chooser is asked with, `getPorts`, `open`'s options and a second open, one reader
// that loses nothing with its backpressure (1 MB pushed while the reader is slow), a non-fatal error at its offset and
// the next `readable` taken, writes in order, signals, an unplug ending the stream, and `forget`.
//
// usage: bun packages/serial/test/web.mjs
import { checker, open } from './chromium.mjs';

const fake = () => {
  const chunk = 1024;
  class Port extends EventTarget {
    constructor(info) {
      super();
      Object.assign(this, { info, connected: true, readable: null, writable: null, written: [], signals: [], pulled: 0, queue: [], wake: null, options: null, forgotten: false });
    }
    getInfo() { return this.info; }
    streams() {
      const p = this;
      p.readable = new ReadableStream({
        start(c) { p.controller = c; },
        async pull(c) {
          while (!p.queue.length) await new Promise((r) => { p.wake = r; });
          const piece = p.queue.shift();
          p.pulled += piece.length;
          c.enqueue(piece);
        },
      }, { highWaterMark: 0 });
      p.writable = new WritableStream({ write(c) { p.written.push(...c); } });
    }
    async open(options) {
      if (this.options) throw new DOMException('already open', 'InvalidStateError');
      if (!(options.baudRate > 0)) throw new TypeError('baudRate');
      this.options = options;
      this.streams();
    }
    async close() { this.options = null; this.readable = null; this.writable = null; }
    async setSignals(s) { this.signals.push(s); }
    async getSignals() { return { dataCarrierDetect: true, clearToSend: false, ringIndicator: false, dataSetReady: true }; }
    async forget() { this.forgotten = true; }
    /** The device sends `bytes`, in pieces the size of a read. */
    feed(bytes) {
      for (let i = 0; i < bytes.length; i += chunk) this.queue.push(bytes.slice(i, i + chunk));
      this.wake?.();
    }
    /** A non-fatal error: this readable errors and the port gives a new one (Web Serial's readable steps). */
    fail(name) { const old = this.controller; this.streams(); old.error(new DOMException(name, name)); }
    /** Unplugged: the stream errors fatally and the port disconnects. */
    unplug() {
      const old = this.controller;
      this.readable = null;
      this.connected = false;
      old.error(new DOMException('gone', 'NetworkError'));
      const e = new Event('disconnect');
      Object.defineProperty(e, 'target', { value: this });
      navigator.serial.dispatchEvent(e);
    }
  }
  const ports = [new Port({ usbVendorId: 0x303a, usbProductId: 0x1001 }), new Port({ usbVendorId: 0x0403, usbProductId: 0x6001 })];
  const granted = new Set();
  const serial = new EventTarget();
  serial.requestPort = async (options = {}) => {
    globalThis.__asked = options;
    const match = ports.find((p) => !(options.filters ?? []).length || options.filters.some((f) => (f.usbVendorId ?? p.info.usbVendorId) === p.info.usbVendorId && (f.usbProductId ?? p.info.usbProductId) === p.info.usbProductId));
    if (!match) throw new DOMException('none chosen', 'NotFoundError');
    granted.add(match);
    return match;
  };
  serial.getPorts = async () => [...granted].filter((p) => !p.forgotten);
  Object.defineProperty(navigator, 'serial', { value: serial });
  globalThis.__ports = ports;
};

const { check, failed } = checker('web');
const s = await open('serial', fake);
const b64 = (bytes) => Buffer.from(bytes).toString('base64');
const bytesOf = (text) => Buffer.from(text, 'base64');

check('the status reads available, with no attached list', JSON.stringify(await s.call('serialStatus')) === JSON.stringify({ available: true, attachedList: false, state: 'granted' }));
const none = await s.call('requestPort', { filters: [{ usbVendorId: 0x1234, usbProductId: null }] });
check('a filter that matches nothing fails NotFoundError, the filter passed without its empty fields', none.error === 'NotFoundError' && JSON.stringify(await s.eval(() => __asked)) === JSON.stringify({ filters: [{ usbVendorId: 0x1234 }] }), JSON.stringify(none));
const port = await s.call('requestPort', { filters: [{ usbVendorId: 0x303a }] });
check('the chosen port is its record', port.usbVendorId === 0x303a && port.usbProductId === 0x1001 && port.connected === true, JSON.stringify(port));
check('getPorts lists it', (await s.call('getPorts')).map((p) => p.id).join() === port.id);
check('the attached list is refused on the web', (await s.call('attachedPorts')).error === 'NotSupportedError');

const opened = await s.call('open', { port: port.id, baudRate: 115200 });
check('open answers a connection and the options with their defaults', opened.connection && opened.options.dataBits === 8 && opened.options.parity === 'none' && opened.options.bufferSize === 255, JSON.stringify(opened));
check('a second open fails InvalidStateError', (await s.call('open', { port: port.id, baudRate: 9600 })).error === 'InvalidStateError');
const connection = opened.connection;

// D8: a megabyte while the reader does not read. The implementation reads until the undelivered bytes reach the
// bound (two seconds at 115,200 baud: 23,040 bytes), then the rest waits in the stream.
const total = 1 << 20;
const data = Uint8Array.from({ length: total }, (_, i) => (i * 7) & 0xff);
await s.eval(([d]) => __ports[0].feed(Uint8Array.from(d)), [Array.from(data)]);
await Bun.sleep(300);
const pulled = await s.eval(() => __ports[0].pulled);
check('backpressure: reading stops at the bound while nothing is delivered', pulled >= 23_040 && pulled <= 23_040 + 1024, `${pulled} bytes read`);
const got = [];
for (let i = 0; i < 400 && got.length < total; i++) {
  const r = await s.call('serialInput', { connection });
  got.push(...bytesOf(r.data));
  if (!r.data) await Bun.sleep(5);
}
check('every byte arrives, in order', got.length === total && got.every((b, i) => b === data[i]), `${got.length} of ${total}`);

// D9: a break between two pieces is an entry at its offset, and reading goes on with the next readable.
await s.eval(() => __ports[0].feed(new TextEncoder().encode('before')));
await Bun.sleep(50);
await s.eval(() => __ports[0].fail('BreakError'));
await Bun.sleep(50);
await s.eval(() => __ports[0].feed(new TextEncoder().encode('after')));
await Bun.sleep(100);
let text = '', errors = [];
for (let i = 0; i < 5; i++) {
  const r = await s.call('serialInput', { connection });
  text += bytesOf(r.data).toString();
  errors.push(...r.errors);
  await Bun.sleep(20);
}
check('a break is marked at its offset and the next readable is read', text === 'beforeafter' && errors.length === 1 && errors[0].code === 'BreakError' && errors[0].at === total + 6, JSON.stringify({ text, errors }));

await s.call('write', { connection, data: b64(Buffer.from('help\r\n')) });
await s.call('write', { connection, data: b64(Buffer.from('reset\r\n')) });
check('writes go out in order', Buffer.from(await s.eval(() => __ports[0].written)).toString() === 'help\r\nreset\r\n');
await s.call('setSignals', { connection, requestToSend: true, dataTerminalReady: null });
check('signals are the call\'s, the omitted ones left out', JSON.stringify(await s.eval(() => __ports[0].signals)) === JSON.stringify([{ requestToSend: true }]));
check('getSignals reads the port\'s', (await s.call('getSignals', { connection })).dataSetReady === true);

await s.eval(() => __ports[0].unplug());
await Bun.sleep(100);
let end;
for (let i = 0; i < 5 && !end?.done; i++) end = await s.call('serialInput', { connection });
check('an unplug ends the stream with NetworkError', end.done === true && (end.errors ?? []).some((e) => e.code === 'NetworkError'), JSON.stringify(end));
check('the next read fails InvalidStateError', (await s.call('serialInput', { connection })).error === 'InvalidStateError');
check('the connection reads not connected', (await s.call('serialConnection', { connection })).connected === false);

const other = await s.call('requestPort', { filters: [{ usbVendorId: 0x0403 }] });
await s.call('forget', { port: other.id });
check('forget forgets the port', (await s.eval(() => __ports[1].forgotten)) === true && !(await s.call('getPorts')).some((p) => p.id === other.id));
check('the readers of the list were asked again', (await s.eval(() => __changed)).includes('serial.getPorts'));

await s.close();
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
