// The stream fixture's data (LLP 1133.003 §9): the T3 Code device player's
// framing (`packages/client-runtime/src/device/stream.ts` at 74541bab17),
// parsed in a stream's mapper. serve-sim's body is envelopes: a `u32`
// big-endian length, a tag (1 the `avcC` description, 2 a key frame, 3 a
// delta, 4 a JPEG seed) and the payload, at arbitrary read boundaries.
// serve-emu's WebSocket sends one frame a message: a 16-byte header (magic
// `SEMU`, version 1, a key flag, a `u64` presentation time) over Annex-B.
// Each frame goes to `__exact_gate_decode`, the gate's stub decode until
// `@exact/webcodecs` exists, which hosts install under the agent only:
// natively it copies the bytes into the host; on the web it feeds the
// browser's `VideoDecoder`. Outside the agent the frames are parsed only.
import type { Answer } from './app.contract.d.ts';

export const appId = 'com.exact.streamfixture';
const SERVER = '127.0.0.1:4340';
export const grants = [`net.fetch http://${SERVER}`, `net.websocket ws://${SERVER}`].join('\n');

type Config = { codec: string; description?: Uint8Array; optimizeForLatency: boolean };
declare const __exact_gate_decode: ((track: string, type: 'key' | 'delta', timestamp: number, data: Uint8Array, config?: Config) => void) | undefined;
function decode(track: string, type: 'key' | 'delta', timestamp: number, data: Uint8Array, config?: Config) {
  if (typeof __exact_gate_decode === 'function') __exact_gate_decode(track, type, timestamp, data, config);
}

const FRAME_US = 16_667;
const LIVE = { live: true, failed: '' };
const IDLE = { live: false, failed: '' };
type Event = { type: string; data: ArrayBuffer | string; message?: string };

const hex = (byte: number) => byte.toString(16).padStart(2, '0');
// The codec string from SPS bytes 1–3 (profile, constraints, level).
const codecOf = (sps: Uint8Array) => (sps.length < 4 ? 'avc1.42E01E' : `avc1.${hex(sps[1])}${hex(sps[2])}${hex(sps[3])}`);

// serve-sim's envelopes, as the reference's `AvccDemuxer` reads them: bytes
// join a buffer, and each complete envelope is handed on before the rest is
// moved to the front.
function avcc(track: string) {
  let buffer = new Uint8Array(64 * 1024), length = 0, config: Config | null = null, keyed = false, frame = 0;
  return (e: Event) => {
    if (e.type === 'error') return { live: false, failed: e.message ?? 'ended' };
    if (!(e.data instanceof ArrayBuffer)) return LIVE;
    const bytes = new Uint8Array(e.data);
    if (length + bytes.length > buffer.length) {
      let capacity = buffer.length;
      while (capacity < length + bytes.length) capacity *= 2;
      const grown = new Uint8Array(capacity);
      grown.set(buffer.subarray(0, length));
      buffer = grown;
    }
    buffer.set(bytes, length);
    length += bytes.length;
    let offset = 0;
    while (length - offset >= 4) {
      const size = new DataView(buffer.buffer, offset, 4).getUint32(0, false);
      if (length - offset - 4 < size) break;
      const tag = buffer[offset + 4], payload = buffer.subarray(offset + 5, offset + 4 + size);
      if (tag === 1) config = { codec: codecOf(payload), description: payload.slice(), optimizeForLatency: true };
      else if (tag === 2 && config) { keyed = true; decode(track, 'key', frame++ * FRAME_US, payload, config); }
      else if (tag === 3 && keyed) decode(track, 'delta', frame++ * FRAME_US, payload);
      offset += 4 + size;
    }
    if (offset > 0) { buffer.copyWithin(0, offset, length); length -= offset; }
    return LIVE;
  };
}

// The first SPS of an Annex-B access unit, for the codec string.
function spsOf(unit: Uint8Array): Uint8Array | null {
  for (let i = 0; i + 3 < unit.length; i++) {
    if (unit[i] === 0 && unit[i + 1] === 0 && unit[i + 2] === 1 && (unit[i + 3] & 0x1f) === 7) return unit.subarray(i + 3);
  }
  return null;
}

// serve-emu's frames: decoded with no description, so a key frame carries
// its parameter sets.
function semu(track: string) {
  let config: Config | null = null;
  return (e: Event) => {
    if (e.type === 'error' || e.type === 'close') return { live: false, failed: e.message ?? 'closed' };
    if (!(e.data instanceof ArrayBuffer) || e.data.byteLength <= 16) return LIVE;
    const view = new DataView(e.data, 0, 16);
    if (view.getUint32(0, false) !== 0x53454d55 || view.getUint8(4) !== 1) return LIVE;
    const key = (view.getUint8(5) & 1) !== 0, unit = new Uint8Array(e.data, 16);
    const timestamp = view.getUint32(8, false) * 2 ** 32 + view.getUint32(12, false);
    if (key) {
      const sps = spsOf(unit);
      if (sps) config = { codec: codecOf(sps), optimizeForLatency: true };
    }
    if (config) decode(track, key ? 'key' : 'delta', timestamp, unit, key ? config : undefined);
    return LIVE;
  };
}

function screen(mode: string, track: string) {
  if (mode === 'android') return fetch(`ws://${SERVER}/android`, { exactStream: semu(track) } as RequestInit);
  if (mode === 'ios' || mode === 'ios-split') {
    const split = mode === 'ios-split' ? '?split=1024' : '';
    return fetch(`http://${SERVER}/ios/stream.avcc${split}`, { exactStream: avcc(track) } as RequestInit);
  }
  return IDLE;
}

export const answer: Answer = (source, args) => {
  switch (source) {
    case 'screen': return screen(String(args[0]), String(args[1])) as never;
    default: throw new Error(`unknown source ${source}`);
  }
};
