// Bench (LLP 1133.037 §12.2): a serial monitor for a development board, and a firmware updater for a board in USB
// DFU mode. The bytes are this module's: the log's lines (`serial.ts`) and DFU's requests (`dfu.ts`); Contract reads
// what they make of them.
import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';
import {
  attachedDevices, claimInterface, close as closeDevice, controlTransferIn, controlTransferOut, getDevices,
  open as openDevice, selectAlternateInterface, usbStatus,
} from '@exact/usb';
import { DfuError, updating, type DfuIo, type DfuStep } from './dfu';
import { attachedPorts, getPorts } from '@exact/serial';
import { clearLog, decodeBase64, encodeBase64, sendLine, serialLog } from './serial';

export const appId = 'com.exact.bench';
export const grants = 'device.serial\ndevice.usb\nfs.read app:/tmp/picked';

type Job = { steps: AsyncGenerator<DfuStep, void>; connection: string; step: DfuStep };
const jobs = new Map<string, Job>();
let made = 0;

/** The DFU interface of a device: the application-specific class's DFU subclass (USB class codes, 0xFE/0x01). */
type Device = Awaited<ReturnType<typeof getDevices>>[number];

function dfuInterface(device: Device) {
  for (const configuration of device.configurations) {
    for (const i of configuration.interfaces) {
      const alternate = i.alternates.find((a) => a.interfaceClass === 0xfe && a.interfaceSubclass === 0x01);
      if (alternate) return { configuration, number: i.interfaceNumber, alternate };
    }
  }
  return null;
}

/** DfuSe's flash layout from the alternate setting's name (AN3156 §4: `@Internal Flash  /0x08000000/04*016Kg,…`):
 * the start address and the first sector's size, which every erased page is taken to be. */
export function dfuseLayout(name: string | null | undefined): { address: number; pageSize: number } | undefined {
  const m = /\/0x([0-9a-fA-F]+)\/(\d+)\*(\d+)([KM]?)/.exec(name ?? '');
  if (!m) return undefined;
  const unit = m[4] === 'M' ? 1024 * 1024 : m[4] === 'K' ? 1024 : 1;
  return { address: parseInt(m[1], 16), pageSize: Number(m[3]) * unit };
}

/** The DFU functional descriptor's `wTransferSize` (DFU 1.1 §4.1.3), read from the configuration descriptor; 1,024
 * where the device does not answer it. */
async function transferSize(connection: string): Promise<number> {
  const read = await controlTransferIn({ connection, setup: { requestType: 'standard', recipient: 'device', request: 6, value: 0x0200, index: 0 }, length: 512 });
  if (read.status === 'ok') {
    const bytes = decodeBase64(read.data);
    for (let at = 0; at + 1 < bytes.length && bytes[at] > 0; at += bytes[at]) {
      if (bytes[at + 1] === 0x21 && bytes[at] >= 7) return bytes[at + 5] | (bytes[at + 6] << 8);
    }
  }
  return 1024;
}

type Started = Result<'startUpdate'>;

const DFU = { vendorId: null, productId: null, classCode: 0xfe, subclassCode: 0x01, protocolCode: null, serialNumber: null };

/** The DFU boards plugged in where the platform lists them (D3), else those the person chose in the browser. A
 * Contract read of `attachedDevices` would fail on the web, so the choice between the two is made here. */
async function boardList(attachedList: boolean): Promise<Device[]> {
  return attachedList ? attachedDevices({ filters: [DFU] }) : (await getDevices({})).filter((d) => dfuInterface(d));
}

/** The serial ports the same way: plugged in now, else remembered. */
async function portList(attachedList: boolean) {
  return attachedList ? attachedPorts({}) : getPorts({});
}

/** The board `id` names. */
async function boardNamed(id: string): Promise<Device | undefined> {
  const { attachedList } = await usbStatus({});
  return (await boardList(attachedList)).find((d) => d.id === id);
}

/** Open the board, claim its DFU interface and read the image; the update then goes on step by step. */
async function startUpdate(id: string, path: string, storage: Storage): Promise<Started> {
  const failed = (error: string): Started => ({ job: '', total: 0, error });
  const device = await boardNamed(id);
  if (!device) return failed('That board is no longer plugged in.');
  const found = dfuInterface(device);
  if (!found) return failed('This device has no DFU interface.');
  let image: Uint8Array;
  try {
    image = new Uint8Array(await storage.fs.readFile(path));
  } catch {
    return failed('Bench could not read that file.');
  }
  if (image.length === 0) return failed('That file is empty.');
  let connection = '';
  try {
    connection = (await openDevice({ device: device.id })).connection;
    await claimInterface({ connection, interfaceNumber: found.number });
    await selectAlternateInterface({ connection, interfaceNumber: found.number, alternateSetting: found.alternate.alternateSetting });
    const io: DfuIo = {
      async controlIn(request, value, length) {
        const r = await controlTransferIn({ connection, setup: { requestType: 'class', recipient: 'interface', request, value, index: found.number }, length });
        return r.status === 'ok' ? decodeBase64(r.data) : null;
      },
      async controlOut(request, value, data) {
        const r = await controlTransferOut({ connection, setup: { requestType: 'class', recipient: 'interface', request, value, index: found.number }, data: encodeBase64(data) });
        return r.status === 'ok';
      },
    };
    const options = { transferSize: await transferSize(connection), dfuse: dfuseLayout(found.alternate.interfaceName) };
    const job = `dfu-${++made}`;
    jobs.set(job, { steps: updating(io, image, options), connection, step: { wait: 0, written: 0, total: image.length } });
    return { job, total: image.length, error: '' };
  } catch (e) {
    if (connection) await closeDevice({ connection }).catch(() => {});
    return failed(`The board refused the update: ${(e as Error).message ?? e}`);
  }
}

type Stepped = Result<'continueUpdate'>;

/** The update's next step: written bytes and a wait for Contract to keep, until it is done or fails. */
async function continueUpdate(job: string): Promise<Stepped> {
  const j = jobs.get(job);
  if (!j) return { job, written: 0, total: 0, wait: 0, done: true, error: 'The update was interrupted.' };
  const end = async (error: string): Promise<Stepped> => {
    jobs.delete(job);
    await closeDevice({ connection: j.connection }).catch(() => {});
    return { job, written: j.step.written, total: j.step.total, wait: 0, done: true, error };
  };
  try {
    const next = await j.steps.next();
    if (next.done) return end('');
    j.step = next.value;
    return { job, ...next.value, done: false, error: '' };
  } catch (e) {
    return end(e instanceof DfuError ? `The update failed: ${e.message}.` : `The update failed: ${(e as Error).message ?? e}`);
  }
}

const sources: Sources = {
  portList: ([attachedList]) => portList(attachedList),
  boardList: ([attachedList]) => boardList(attachedList),
  serialLog: ([connection]) => serialLog(connection),
  clearLog: ([connection]) => clearLog(connection),
  sendLine: ([connection, text, ending]) => sendLine(connection, text, ending),
  startUpdate: ([device, path], _store, storage) => startUpdate(device, path, storage),
  continueUpdate: ([job]) => continueUpdate(job),
};
export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
