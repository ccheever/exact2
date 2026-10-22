#!/usr/bin/env bun
// The stream fixture's server (LLP 1133.003 §9): it replays `record.swift`'s
// H.264 at 60 fps in a loop, as the T3 Code device player receives it.
//
//   bun apps/stream-fixture/serve.mjs [--port 4340]
//   GET /ios/stream.avcc[?split=<bytes>]  serve-sim's body: the description,
//                                         then a key frame or delta envelope
//                                         every 16.7 ms, each written whole,
//                                         or in writes of at most <bytes>
//   GET /android (WebSocket)              serve-emu's: a SEMU binary message
//                                         every 16.7 ms
//   GET /stats                            what each connection was sent
//
// The recordings are made on first use (`xcrun swift record.swift`) into
// `target/stream-fixture/`, which no commit carries.
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const args = process.argv.slice(2);
const port = Number(args[args.indexOf('--port') + 1] || 4340) || 4340;
const ROOT = resolve(import.meta.dirname, '../..');
const DIR = resolve(ROOT, 'target/stream-fixture');
const FRAME_MS = 1000 / 60;

if (!existsSync(resolve(DIR, 'ios.avcc')) || !existsSync(resolve(DIR, 'android.semu'))) {
  const made = Bun.spawnSync(['xcrun', 'swift', '-O', resolve(import.meta.dirname, 'record.swift'), DIR], { stdout: 'inherit', stderr: 'inherit' });
  if (made.exitCode !== 0) throw new Error('record.swift failed: the recordings need macOS and VideoToolbox');
}

// Each recording as its pieces: a `u32` big-endian length before each.
function pieces(bytes, lengthCovers) {
  const out = [];
  for (let at = 0; at + 4 <= bytes.length;) {
    const n = bytes.readUInt32BE(at);
    out.push(lengthCovers ? bytes.subarray(at, at + 4 + n) : bytes.subarray(at + 4, at + 4 + n));
    at += 4 + n;
  }
  return out;
}
// serve-sim's envelopes are sent as they are; the first is the description.
const [description, ...envelopes] = pieces(readFileSync(resolve(DIR, 'ios.avcc')), true);
const messages = pieces(readFileSync(resolve(DIR, 'android.semu')), false);

const connections = [];
// Calls `send(i)` for frame i at its time from now, until `stop()`.
function paced(send) {
  const start = performance.now();
  let i = 0, timer = null, stopped = false;
  const tick = () => {
    if (stopped) return;
    while (performance.now() >= start + i * FRAME_MS) send(i++);
    timer = setTimeout(tick, Math.max(0, start + i * FRAME_MS - performance.now()));
  };
  tick();
  return () => { stopped = true; clearTimeout(timer); };
}

function avcc(url) {
  const split = Number(url.searchParams.get('split')) || 0;
  const stats = { kind: 'ios', split, opened: Date.now(), frames: 0, bytes: 0, writes: 0, closed: null };
  connections.push(stats);
  let stop = () => {};
  const body = new ReadableStream({
    type: 'direct',
    async pull(controller) {
      const write = (bytes) => {
        for (let at = 0; at < bytes.length; at += split || bytes.length) {
          controller.write(bytes.subarray(at, split ? at + split : bytes.length));
          controller.flush();
          stats.writes++;
        }
        stats.bytes += bytes.length;
      };
      write(description);
      await new Promise((done) => {
        stop = paced((i) => {
          try { write(envelopes[i % envelopes.length]); stats.frames++; } catch { stop(); done(); }
        });
      });
    },
    cancel() { stop(); stats.closed = Date.now(); },
  });
  return new Response(body, { headers: { 'content-type': 'application/octet-stream', 'cache-control': 'no-store', 'access-control-allow-origin': '*' } });
}

const server = Bun.serve({
  port,
  hostname: '127.0.0.1',
  fetch(req, srv) {
    const url = new URL(req.url);
    if (url.pathname === '/ios/stream.avcc') return avcc(url);
    if (url.pathname === '/android' && srv.upgrade(req, { data: { kind: 'android', opened: Date.now(), frames: 0, bytes: 0, closed: null } })) return;
    if (url.pathname === '/stats') return Response.json(connections.map(({ stop, ...c }) => c));
    if (url.pathname === '/reset') { connections.length = 0; return Response.json({ ok: true }); }
    return new Response('not found', { status: 404 });
  },
  websocket: {
    open(ws) {
      const stats = ws.data;
      connections.push(stats);
      stats.stop = paced((i) => {
        const m = Buffer.from(messages[i % messages.length]);
        m.writeBigUInt64BE(BigInt(Math.round(i * FRAME_MS * 1000)), 8);
        ws.sendBinary(m);
        stats.frames++; stats.bytes += m.length;
      });
    },
    close(ws) { ws.data.stop?.(); ws.data.closed = Date.now(); },
    message() {},
  },
});
console.log(`stream fixture on http://127.0.0.1:${server.port} (${envelopes.length} iOS and ${messages.length} Android frames, looped)`);
