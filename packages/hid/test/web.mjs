// @exact/hid's web conformance suite in Chromium (LLP 1133.037 §16 stage 1), over a fake `navigator.hid`
// (`packages/serial/test/chromium.mjs`): the chooser asked with the app's filters and every pad usage excluded (D11),
// a pad never offered, a lease per open on one shared open, input reports as a batch (the latest first, then every
// report since, `dropped` past `history`), output and feature reports, an unplug, and `forget`.
//
// usage: bun packages/hid/test/web.mjs
import { checker, open } from '../../serial/test/chromium.mjs';

const fake = () => {
  class Device extends EventTarget {
    constructor(vendorId, productId, productName, collections) {
      super();
      Object.assign(this, { vendorId, productId, productName, collections, opened: false, opens: 0, sent: [], forgotten: false });
    }
    async open() { this.opened = true; this.opens++; }
    async close() { this.opened = false; }
    async forget() { this.forgotten = true; }
    async sendReport(reportId, data) { this.sent.push(['output', reportId, [...data]]); }
    async sendFeatureReport(reportId, data) { this.sent.push(['feature', reportId, [...data]]); }
    async receiveFeatureReport(reportId) {
      if (reportId !== 1) throw new DOMException('no such report', 'NotAllowedError');
      return new DataView(Uint8Array.of(1, 0x63, 0xff).buffer);
    }
    report(reportId, bytes) {
      const e = new Event('inputreport');
      Object.assign(e, { reportId, data: new DataView(Uint8Array.from(bytes).buffer) });
      this.dispatchEvent(e);
    }
  }
  const light = new Device(0x27b8, 0x01ed, 'blink(1) mk3', [{ usagePage: 0xff00, usage: 1, featureReports: [{ reportId: 1 }], inputReports: [], outputReports: [] }]);
  const pad = new Device(0x054c, 0x0ce6, 'DualSense', [{ usagePage: 1, usage: 5, inputReports: [{ reportId: 1 }] }]);
  const granted = new Set([pad]);
  const hid = new EventTarget();
  hid.requestDevice = async (options) => {
    globalThis.__asked = options;
    const match = [light, pad].filter((d) => options.filters.some((f) => (f.vendorId ?? d.vendorId) === d.vendorId) && !options.exclusionFilters.some((x) => d.collections.some((c) => c.usagePage === x.usagePage && c.usage === x.usage)));
    match.forEach((d) => granted.add(d));
    return match;
  };
  hid.getDevices = async () => [...granted].filter((d) => !d.forgotten);
  Object.defineProperty(navigator, 'hid', { value: hid });
  Object.assign(globalThis, { __light: light, __pad: pad });
};

const { check, failed } = checker('web');
const h = await open('hid', fake);
const b64 = (bytes) => Buffer.from(bytes).toString('base64');

const chosen = await h.call('requestDevice', { filters: [{ vendorId: 0x27b8, productId: null }], exclusionFilters: null });
const asked = await h.eval(() => __asked);
check('the chooser excludes every pad usage beside the app\'s filters', asked.exclusionFilters.length === 3 && asked.exclusionFilters.every((x) => x.usagePage === 1) && JSON.stringify(asked.filters) === JSON.stringify([{ vendorId: 0x27b8 }]), JSON.stringify(asked));
check('the chosen light is its record', chosen.length === 1 && chosen[0].productName === 'blink(1) mk3' && chosen[0].collections[0].featureReports[0].reportId === 1, JSON.stringify(chosen));
check('a pad the browser remembers is never offered', (await h.call('getDevices')).map((d) => d.productName).join() === 'blink(1) mk3');
const device = chosen[0].id;
const a = await h.call('open', { device }), b = await h.call('open', { device });
check('two opens are two leases on one open', a.connection !== b.connection && (await h.eval(() => __light.opens)) === 1);

for (const n of [1, 2, 3]) await h.eval((n) => __light.report(2, [n]), n);
const first = await h.call('inputReports', { connection: a.connection });
check('a reader\'s first answer is the latest report', first.reports.length === 1 && first.reports[0].data === b64([3]), JSON.stringify(first));
for (const n of [4, 5, 6, 7]) await h.eval((n) => __light.report(2, [n]), n);
const next = await h.call('inputReports', { connection: a.connection });
check('the next is every report since', next.reports.map((r) => r.data).join() === [4, 5, 6, 7].map((n) => b64([n])).join() && next.dropped === 0, JSON.stringify(next));
await h.call('inputReports', { connection: a.connection, history: 2 });
for (const n of [8, 9, 10]) await h.eval((n) => __light.report(2, [n]), n);
const bounded = await h.call('inputReports', { connection: a.connection, history: 2 });
check('past history, the rest is counted dropped', bounded.reports.length === 2 && bounded.dropped === 1, JSON.stringify(bounded));

await h.call('sendFeatureReport', { connection: a.connection, reportId: 1, data: b64([0x63, 0xff, 0, 0, 0, 30, 0]) });
await h.call('sendReport', { connection: b.connection, reportId: 0, data: b64([1]) });
check('feature and output reports reach the device', JSON.stringify(await h.eval(() => __light.sent)) === JSON.stringify([['feature', 1, [0x63, 0xff, 0, 0, 0, 30, 0]], ['output', 0, [1]]]));
check('a feature report reads back', (await h.call('receiveFeatureReport', { connection: a.connection, reportId: 1 })).data === b64([1, 0x63, 0xff]));
check('a report the device lacks fails NotAllowedError', (await h.call('receiveFeatureReport', { connection: a.connection, reportId: 9 })).error === 'NotAllowedError');

await h.call('close', { connection: a.connection });
check('the device stays open while a lease does', (await h.eval(() => __light.opened)) === true);
await h.eval(() => { const e = new Event('disconnect'); e.device = __light; navigator.hid.dispatchEvent(e); });
const gone = await h.call('inputReports', { connection: b.connection });
check('unplugged, a reader reads interrupted and a send fails NetworkError', gone.interrupted === true && (await h.call('sendReport', { connection: b.connection, reportId: 0, data: b64([1]) })).error === 'NetworkError', JSON.stringify(gone));
await h.call('forget', { device });
check('forget forgets the device', (await h.eval(() => __light.forgotten)) === true);

await h.close();
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
