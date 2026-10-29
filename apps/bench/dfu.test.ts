// `dfu.ts` against a fake device that follows DFU 1.1's state machine (§6.1.2, appendix A), and DfuSe's commands.
import { expect, test } from 'bun:test';
import { ABORT, CLRSTATUS, DNLOAD, DfuError, GETSTATUS, State, update, type DfuIo } from './dfu';

type Sent = { request: number; value: number; data: number[] };

/** A board in DFU mode: each DNLOAD makes it busy for one GET_STATUS, then idle; `failAt` answers errVERIFY at the
 * GET_STATUS after that many data blocks. Flash is what the blocks wrote, at DfuSe's address when it has one. */
function board({ failAt, start = State.dfuIDLE, dfuse = false }: { failAt?: number; start?: number; dfuse?: boolean } = {}) {
  let state = start, status = 0, blocks = 0, pointer = 0;
  const sent: Sent[] = [], flash = new Map<number, number>(), erased: number[] = [], slept: number[] = [];
  const sleep = async (ms: number) => { slept.push(ms); };
  const io: DfuIo = {
    async controlIn(request, _value, length) {
      if (request !== GETSTATUS || length !== 6) return null;
      if (failAt !== undefined && blocks === failAt && state === State.dfuDNBUSY) { state = State.dfuERROR; status = 7; }
      const answer = Uint8Array.of(status, 5, 0, 0, state, 0);
      if (state === State.dfuDNBUSY) state = State.dfuDNLOAD_IDLE;
      else if (state === State.dfuMANIFEST) state = State.dfuIDLE;
      return answer;
    },
    async controlOut(request, value, data) {
      sent.push({ request, value, data: [...data] });
      if (request === CLRSTATUS || request === ABORT) { state = State.dfuIDLE; status = 0; return true; }
      if (request !== DNLOAD) return false;
      if (data.length === 0) { state = State.dfuMANIFEST; return true; }
      if (dfuse && value === 0) {
        const address = data[1] | (data[2] << 8) | (data[3] << 16) | (data[4] << 24);
        if (data[0] === 0x41) erased.push(address);
        if (data[0] === 0x21) pointer = address;
      } else {
        const base = dfuse ? pointer + (value - 2) * 4 : value * 4;
        data.forEach((b, i) => flash.set(base + i, b));
        blocks++;
      }
      state = State.dfuDNBUSY;
      return true;
    },
  };
  return { io, sent, flash, erased, slept, sleep };
}

const image = Uint8Array.from({ length: 10 }, (_, i) => 0xa0 + i);

test('a DFU 1.1 update writes each block, ends with a zero-length DNLOAD and reports progress', async () => {
  const b = board(), progress: number[] = [];
  await update(b.io, image, { transferSize: 4, progress: (n) => { if (progress.at(-1) !== n) progress.push(n); } }, b.sleep);
  expect(progress).toEqual([0, 4, 8, 10]);
  expect(b.sent.map((s) => [s.request, s.value, s.data.length])).toEqual([[DNLOAD, 0, 4], [DNLOAD, 1, 4], [DNLOAD, 2, 2], [DNLOAD, 3, 0]]);
  expect([...b.flash.keys()].sort((x, y) => x - y).map((k) => b.flash.get(k))).toEqual([...image]);
  // Each busy answer's bwPollTimeout was waited.
  expect(b.slept.length).toBeGreaterThan(0);
});

test('a DfuSe update erases each page and sets the address before the blocks, numbered from 2', async () => {
  const b = board({ dfuse: true });
  await update(b.io, image, { transferSize: 4, dfuse: { address: 0x0800_0000, pageSize: 8 } }, b.sleep);
  expect(b.erased).toEqual([0x0800_0000, 0x0800_0008]);
  expect(b.sent.slice(0, 3).map((s) => s.data[0])).toEqual([0x41, 0x41, 0x21]);
  expect(b.sent.slice(3).map((s) => s.value)).toEqual([2, 3, 4, 5]);
  expect(b.flash.get(0x0800_0000)).toBe(0xa0);
  expect(b.flash.get(0x0800_0009)).toBe(0xa9);
});

test('errVERIFY after the third block fails the update and clears the status', async () => {
  const b = board({ failAt: 3 });
  const failed = await update(b.io, Uint8Array.from({ length: 20 }), { transferSize: 4 }, b.sleep).catch((e) => e);
  expect(failed).toBeInstanceOf(DfuError);
  expect((failed as DfuError).status).toBe('errVERIFY');
  expect(b.sent.at(-1)?.request).toBe(CLRSTATUS);
  expect(b.sent.filter((s) => s.request === DNLOAD).length).toBe(3);
});

test('a board left in an error starts again from idle', async () => {
  const b = board({ start: State.dfuERROR });
  await update(b.io, image, { transferSize: 8 }, b.sleep);
  expect(b.sent[0].request).toBe(CLRSTATUS);
});
