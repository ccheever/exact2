// @exact/usb's web conformance suite in Chromium (LLP 1133.037 §16 stage 1), over a fake `navigator.usb`
// (`packages/serial/test/chromium.mjs`): the chooser asked with the app's filters, the record's configurations, a
// lease per open on one shared open, a claim held by another connection refused `NetworkError`, control and bulk
// transfers in both directions with their status, the device closing with its last lease, and `forget`.
//
// usage: bun packages/usb/test/web.mjs
import { checker, open } from '../../serial/test/chromium.mjs';

const fake = () => {
  const alternate = { alternateSetting: 0, interfaceClass: 0xfe, interfaceSubclass: 1, interfaceProtocol: 2, interfaceName: '@Internal Flash  /0x08000000/04*016Kg', endpoints: [{ endpointNumber: 1, direction: 'in', type: 'bulk', packetSize: 64 }] };
  const configuration = { configurationValue: 1, configurationName: null, interfaces: [{ interfaceNumber: 0, alternates: [alternate], alternate, claimed: false }] };
  const device = {
    usbVersionMajor: 2, usbVersionMinor: 0, usbVersionSubminor: 0, deviceClass: 0, deviceSubclass: 0, deviceProtocol: 0,
    vendorId: 0x0483, productId: 0xdf11, deviceVersionMajor: 2, deviceVersionMinor: 0, deviceVersionSubminor: 0,
    manufacturerName: 'STMicroelectronics', productName: 'STM32 BOOTLOADER', serialNumber: '3254335A3330',
    configuration, configurations: [configuration], opened: false, opens: 0, closes: 0, log: [], forgotten: false,
    async open() { this.opened = true; this.opens++; },
    async close() { this.opened = false; this.closes++; },
    async forget() { this.forgotten = true; },
    async selectConfiguration(v) { this.log.push(['configuration', v]); },
    async claimInterface(n) { configuration.interfaces[0].claimed = true; this.log.push(['claim', n]); },
    async releaseInterface(n) { configuration.interfaces[0].claimed = false; this.log.push(['release', n]); },
    async selectAlternateInterface(n, a) { this.log.push(['alternate', n, a]); },
    async controlTransferIn(setup, length) {
      this.log.push(['in', setup.request, length]);
      return setup.request === 3 ? { status: 'ok', data: new DataView(Uint8Array.of(0, 5, 0, 0, 2, 0).buffer) } : { status: 'stall' };
    },
    async controlTransferOut(setup, data) { this.log.push(['out', setup.request, [...data]]); return { status: 'ok', bytesWritten: data.length }; },
    async transferIn(endpoint, length) { this.log.push(['bulk in', endpoint, length]); return { status: 'ok', data: new DataView(Uint8Array.of(7, 8).buffer) }; },
    async transferOut(endpoint, data) { this.log.push(['bulk out', endpoint, [...data]]); return { status: 'ok', bytesWritten: data.length }; },
    async reset() { this.log.push(['reset']); },
    async clearHalt(direction, endpoint) { this.log.push(['halt', direction, endpoint]); },
  };
  const usb = new EventTarget();
  usb.requestDevice = async (options) => { globalThis.__asked = options; if (!options.filters.some((f) => f.classCode === 0xfe)) throw new DOMException('none chosen', 'NotFoundError'); return device; };
  usb.getDevices = async () => (device.forgotten ? [] : [device]);
  Object.defineProperty(navigator, 'usb', { value: usb });
  globalThis.__device = device;
};

const { check, failed } = checker('web');
const u = await open('usb', fake);
const b64 = (bytes) => Buffer.from(bytes).toString('base64');

check('a filter that matches nothing fails NotFoundError', (await u.call('requestDevice', { filters: [{ vendorId: 1 }] })).error === 'NotFoundError');
const dfu = { vendorId: null, productId: null, classCode: 0xfe, subclassCode: 1, protocolCode: null, serialNumber: null };
const board = await u.call('requestDevice', { filters: [dfu] });
check('the chooser takes the filter without its empty fields', JSON.stringify((await u.eval(() => __asked)).filters) === JSON.stringify([{ classCode: 0xfe, subclassCode: 1 }]));
check('the record carries the configurations and the DFU alternate', board.productName === 'STM32 BOOTLOADER' && board.configurations[0].interfaces[0].alternates[0].interfaceSubclass === 1 && board.configuration === 1, JSON.stringify(board).slice(0, 300));
const a = await u.call('open', { device: board.id }), b = await u.call('open', { device: board.id });
check('two opens are two leases on one open', a.connection !== b.connection && (await u.eval(() => __device.opens)) === 1);
await u.call('claimInterface', { connection: a.connection, interfaceNumber: 0 });
check('a claim another connection holds fails NetworkError', (await u.call('claimInterface', { connection: b.connection, interfaceNumber: 0 })).error === 'NetworkError');
check('an alternate on an unclaimed interface fails InvalidStateError', (await u.call('selectAlternateInterface', { connection: b.connection, interfaceNumber: 0, alternateSetting: 0 })).error === 'InvalidStateError');
await u.call('selectAlternateInterface', { connection: a.connection, interfaceNumber: 0, alternateSetting: 0 });
const setup = { requestType: 'class', recipient: 'interface', request: 3, value: 0, index: 0 };
const status = await u.call('controlTransferIn', { connection: a.connection, setup, length: 6 });
check('a control transfer in answers its status and bytes', status.status === 'ok' && status.data === b64([0, 5, 0, 0, 2, 0]), JSON.stringify(status));
check('a stall answers stall', (await u.call('controlTransferIn', { connection: a.connection, setup: { ...setup, request: 9 }, length: 1 })).status === 'stall');
check('a control transfer out answers the bytes written', (await u.call('controlTransferOut', { connection: a.connection, setup: { ...setup, request: 1 }, data: b64([0x21, 0, 0, 0, 8]) })).bytesWritten === 5);
check('bulk in and out', (await u.call('transferIn', { connection: a.connection, endpointNumber: 1, length: 64 })).data === b64([7, 8]) && (await u.call('transferOut', { connection: a.connection, endpointNumber: 2, data: b64([1, 2, 3]) })).bytesWritten === 3);
check('the connection reads its claims', JSON.stringify((await u.call('usbConnection', { connection: a.connection })).claimed) === '[0]');
await u.call('close', { connection: a.connection });
check('closing releases its claims; the device stays open for the other lease', (await u.eval(() => __device.log.some((l) => l[0] === 'release'))) && (await u.eval(() => __device.closes)) === 0);
await u.call('close', { connection: b.connection });
check('the device closes with its last lease', (await u.eval(() => __device.closes)) === 1);
await u.call('forget', { device: board.id });
check('forget forgets the device', (await u.eval(() => __device.forgotten)) === true && (await u.call('getDevices')).length === 0);

await u.close();
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
