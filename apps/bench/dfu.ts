// USB DFU 1.1 (USB-IF, "Device Firmware Upgrade", 2004) over control transfers to the DFU interface, and ST's DfuSe
// extension (AN3156) for an STM32's system bootloader: an image written in `wTransferSize` blocks, each followed by
// GET_STATUS until the device is idle again, then the zero-length DNLOAD that starts manifestation (LLP 1133.037 §12.2).
// The transport is `DfuIo`, so the state machine is tested in Bun against a fake device (`dfu.test.ts`). Data code has
// no timers, so the update is a generator that yields each `bwPollTimeout` wait to its driver: `update` sleeps through
// them, and Bench's Contract waits each one with a task between steps.

/** DFU 1.1 §3: the class requests, by `bRequest`. */
export const DNLOAD = 1, GETSTATUS = 3, CLRSTATUS = 4, ABORT = 6;

/** DFU 1.1 §6.1.2's `bState`. */
export const State = {
  appIDLE: 0, appDETACH: 1, dfuIDLE: 2, dfuDNLOAD_SYNC: 3, dfuDNBUSY: 4, dfuDNLOAD_IDLE: 5, dfuMANIFEST_SYNC: 6,
  dfuMANIFEST: 7, dfuMANIFEST_WAIT_RESET: 8, dfuUPLOAD_IDLE: 9, dfuERROR: 10,
} as const;

/** DFU 1.1 §6.1.2's `bStatus`, by value. */
const STATUS = ['OK', 'errTARGET', 'errFILE', 'errWRITE', 'errERASE', 'errCHECK_ERASED', 'errPROG', 'errVERIFY',
  'errADDRESS', 'errNOTDONE', 'errFIRMWARE', 'errVENDOR', 'errUSBR', 'errPOR', 'errUNKNOWN', 'errSTALLEDPKT'];

export type DfuStatus = { status: number; pollTimeout: number; state: number };

/** The device's side: class requests to the DFU interface. `controlIn` answers the bytes, or null for a stall. */
export interface DfuIo {
  controlIn(request: number, value: number, length: number): Promise<Uint8Array | null>;
  controlOut(request: number, value: number, data: Uint8Array): Promise<boolean>;
}

/** Where an update stands when it yields: the wait it needs before its next request, and its progress. */
export type DfuStep = { wait: number; written: number; total: number };

export class DfuError extends Error {
  constructor(message: string, readonly status?: string) { super(message); }
}

export function parseStatus(bytes: Uint8Array): DfuStatus {
  if (bytes.length < 6) throw new DfuError(`GET_STATUS answered ${bytes.length} bytes, not 6`);
  return { status: bytes[0], pollTimeout: bytes[1] | (bytes[2] << 8) | (bytes[3] << 16), state: bytes[4] };
}

export function statusName(status: number): string {
  return STATUS[status] ?? `status ${status}`;
}

async function getStatus(io: DfuIo): Promise<DfuStatus> {
  const bytes = await io.controlIn(GETSTATUS, 0, 6);
  if (!bytes) throw new DfuError('GET_STATUS stalled');
  return parseStatus(bytes);
}

type Progress = { written: number; total: number };

/** GET_STATUS, yielding `bwPollTimeout` while the device is busy, until it leaves `busy`. A device in dfuERROR is
 * cleared (CLR_STATUS) and the update fails with its status. */
async function* settle(io: DfuIo, busy: number[], at: Progress): AsyncGenerator<DfuStep, DfuStatus> {
  for (let tries = 0; tries < 10_000; tries++) {
    const s = await getStatus(io);
    if (s.state === State.dfuERROR || s.status !== 0) {
      await io.controlOut(CLRSTATUS, 0, new Uint8Array());
      throw new DfuError(`the board reported ${statusName(s.status)}`, statusName(s.status));
    }
    if (!busy.includes(s.state)) return s;
    if (s.pollTimeout > 0) yield { wait: s.pollTimeout, ...at };
  }
  throw new DfuError('the board stayed busy');
}

async function* download(io: DfuIo, block: number, data: Uint8Array, at: Progress): AsyncGenerator<DfuStep, DfuStatus> {
  if (!(await io.controlOut(DNLOAD, block, data))) throw new DfuError(`DNLOAD of block ${block} stalled`);
  return yield* settle(io, [State.dfuDNBUSY, State.dfuDNLOAD_SYNC], at);
}

/** A DfuSe command (AN3156 §6.1): sent as block 0. */
function command(code: number, address?: number): Uint8Array {
  if (address === undefined) return Uint8Array.of(code);
  return Uint8Array.of(code, address & 0xff, (address >>> 8) & 0xff, (address >>> 16) & 0xff, (address >>> 24) & 0xff);
}

export type UpdateOptions = {
  transferSize: number;
  /** DfuSe: the flash address the image starts at, and the size of each page to erase before writing it. */
  dfuse?: { address: number; pageSize: number };
  progress?: (written: number, total: number) => void;
};

/** Write `image` to the board, block by block, then manifest it, sleeping through each wait. Resolves once the board
 * has taken the image; the board then leaves DFU mode, or waits for a reset (`dfuMANIFEST-WAIT-RESET`). */
export async function update(io: DfuIo, image: Uint8Array, options: UpdateOptions, sleep: (ms: number) => Promise<void>): Promise<void> {
  const steps = updating(io, image, options);
  for (let next = await steps.next(); !next.done; next = await steps.next()) {
    options.progress?.(next.value.written, next.value.total);
    if (next.value.wait > 0) await sleep(next.value.wait);
  }
}

/** The update as steps: each yield is a wait the board asked for, or a block written (wait 0). */
export async function* updating(io: DfuIo, image: Uint8Array, options: UpdateOptions): AsyncGenerator<DfuStep, void> {
  const size = options.transferSize;
  if (!(size > 0)) throw new DfuError('the interface names no transfer size');
  // A board left in an error or mid-download by an earlier attempt starts again from idle (DFU 1.1 §6.1.5).
  const first = await getStatus(io);
  if (first.state === State.dfuERROR) await io.controlOut(CLRSTATUS, 0, new Uint8Array());
  else if (first.state !== State.dfuIDLE) await io.controlOut(ABORT, 0, new Uint8Array());
  let block = 0;
  const dfuse = options.dfuse;
  if (dfuse) {
    // Erase each page the image covers, then point at its start; data blocks are numbered from 2 (AN3156 §6.1).
      const at: Progress = { written: 0, total: image.length };
    for (let page = 0; page < image.length; page += dfuse.pageSize) yield* download(io, 0, command(0x41, dfuse.address + page), at);
    yield* download(io, 0, command(0x21, dfuse.address), at);
    block = 2;
  }
  const at: Progress = { written: 0, total: image.length };
  for (let from = 0; from < image.length; from += size, block++) {
    yield* download(io, block, image.subarray(from, Math.min(from + size, image.length)), at);
    at.written = Math.min(from + size, image.length);
    yield { wait: 0, ...at };
  }
  // The zero-length DNLOAD ends the download; the board manifests the image (DFU 1.1 §7).
  if (!(await io.controlOut(DNLOAD, block, new Uint8Array()))) throw new DfuError('the final DNLOAD stalled');
  let done: DfuStatus | null = null;
  try {
    done = yield* settle(io, [State.dfuMANIFEST_SYNC, State.dfuMANIFEST], at);
  } catch (e) {
    // A DfuSe bootloader jumps to the new image during manifestation and stops answering.
    if (!(dfuse && e instanceof DfuError && !e.status)) throw e;
  }
  if (done && ![State.dfuIDLE, State.dfuMANIFEST_WAIT_RESET].includes(done.state as 2 | 8)) {
    throw new DfuError(`the board ended in state ${done.state}`);
  }
}
