import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

// Uses the already-built exact-web-js compiler. The fixture and peer are
// private to this test; all input goes through the ordinary browser driver.
test('JS target socket mutations echo ordered frames and obey commits and resource lifetime', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/socket-test-'));
  const received = [], connections = new Set();
  let counted = 0;
  const peer = Bun.serve({ hostname: '127.0.0.1', port: 0,
    fetch(req, server) {
      if (new URL(req.url).pathname === '/count') { counted++; return new Response('counted', { headers: { 'access-control-allow-origin': '*' } }); }
      if (new URL(req.url).pathname === '/body') return req.text().then(body => new Response('body:' + body, { headers: { 'access-control-allow-origin': '*' } }));
      if (new URL(req.url).pathname === '/events') return new Response(new ReadableStream({
        start(controller) { controller.enqueue(new TextEncoder().encode('data: initial\n\n')); },
      }), { headers: { 'content-type': 'text/event-stream', 'access-control-allow-origin': '*' } });
      return server.upgrade(req) ? undefined : new Response('websocket only');
    },
    websocket: {
      open(ws) { connections.add(ws); },
      message(ws, data) {
        // A large frame fills the browser's buffer; it is counted, not echoed.
        if (typeof data === 'string' && data.startsWith('xx')) { received.push(`x${data.length}`); return; }
        received.push(typeof data === 'string' ? data : [...data]); ws.send(data);
      },
      close(ws) { connections.delete(ws); },
    },
  });
  const url = `ws://127.0.0.1:${peer.port}`;
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Socket Test', app: { id: 'test.socket', name: 'Socket Test' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), `component SocketTest
  resource feed = feed(1) as shape string else empty()
  resource other = feed(2) as shape string else empty()
  resource denied = denied() as shape string else empty()
  resource cancelSocket = cancellable("socket") as shape string else empty()
  resource cancelEvents = cancellable("events") as shape string else empty()
  mutation sent as shape string queue
  mutation second as shape string
  action write(mode: string)
    send sent = write(mode)
  action rollback
    send sent = write("rollback")
    send second = refuse()
  action replace
    send sent = write("forgotten")
    refresh feed
  action twice
    send sent = write("first")
    send sent = write("second")
  view
    column
      text feed testId="feed"
      text other testId="other"
      text denied testId="denied"
      button press=write("cancel") testId="cancel"
        text "cancel"
      button press=write("burst") testId="burst"
        text "send"
      button press=write("throw") testId="throw"
        text "throw"
      button press=write("async") testId="async"
        text "async"
      button press=write("shape") testId="shape"
        text "shape"
      button press=write("overflow") testId="overflow"
        text "overflow"
      button press=rollback testId="rollback"
        text "rollback"
      button press=twice testId="twice"
        text "twice"
      button press=replace testId="replace"
        text "replace"
      button press=write("stale") testId="stale"
        text "stale"
      button press=write("close") testId="close"
        text "close"
      button press=write("probe") testId="probe"
        text "probe"
      button press=write("read") testId="read"
        text "read"
      button press=write("big") testId="big"
        text "big"
      button press=write("sendfetch") testId="sendfetch"
        text "send then fetch"
      button press=write("fetchsend") testId="fetchsend"
        text "fetch then send"
      button press=write("captured") testId="captured"
        text "captured"
`);
  writeFileSync(resolve(dir, 'app.ts'), `export const appId = 'test.socket';
export const grants = ${JSON.stringify(`net.websocket ${url}\nnet.fetch http://127.0.0.1:${peer.port}`)};
const sockets = new Map(), controllers = new Map(); let previous: any;
export function answer(name: string, args: any): any {
  if (name === 'cancellable') {
    const kind = args[0], controller = new AbortController();
    controllers.set(kind, controller);
    return fetch(kind === 'socket' ? ${JSON.stringify(url)} : ${JSON.stringify(`http://127.0.0.1:${peer.port}/events`)}, {
      signal: controller.signal,
      exactStream: (event: any) => event.type === 'error' ? event.type + ':' + event.kind : event.type === 'open' ? 'open' : event.data,
    });
  }
  if (name === 'feed') {
    const id = args[0]; let log = '';
    return fetch(${JSON.stringify(url)}, { exactStream(event: any, socket?: any) {
      if (event.type === 'open') {
        if (id === 1) previous = sockets.get(id);
        sockets.set(id, socket);
        try { socket.send('mapper'); return 'mapper unexpectedly sent'; } catch {}
        return 'open';
      }
      if (event.type === 'close') return event.code + ':' + event.reason + ':' + event.wasClean;
      if (event.type === 'error') return event.kind + ':' + event.message;
      log += (typeof event.data === 'string' ? event.data : [...new Uint8Array(event.data)].join(',')) + '|';
      return log;
    }});
  }
  if (name === 'denied') return fetch('ws://localhost:' + ${peer.port}, { exactStream: (e: any) => e.kind });
  if (name === 'refuse') throw Object.assign(new Error('deliberate commit refusal'), { refuse: true });
  const mode = args[0], s = mode === 'stale' ? previous : sockets.get(1), t = sockets.get(2);
  const counter = ${JSON.stringify(`http://127.0.0.1:${peer.port}/count`)};
  if (mode === 'probe') { const before = t.bufferedAmount; t.send('abc'); return before + ':' + t.bufferedAmount; }
  if (mode === 'read') return 'read:' + t.bufferedAmount;
  if (mode === 'big') { t.send('x'.repeat(900000)); return 'big'; }
  if (mode === 'sendfetch') { t.send('a'); fetch(counter).catch(() => {}); return 'x'; }
  if (mode === 'captured') {
    const url = new URL(${JSON.stringify(`http://127.0.0.1:${peer.port}/body`)}), init = Object.create({ method: 'POST' });
    init.body = new URLSearchParams('was=original');
    const sent = fetch(url, init);
    url.pathname = '/count'; init.body.set('was', 'changed');
    return sent.then((r: any) => r.text());
  }
  if (mode === 'fetchsend') { fetch(counter).catch(() => {}); try { t.send('a'); } catch {} return 'x'; }
  if (mode === 'cancel') { for (const c of controllers.values()) c.abort(); return 'cancelled'; }
  if (mode === 'close') { s.close(4001, 'finished'); return 'closed'; }
  if (mode === 'burst') {
    for (const frame of ['subscribe', 'Ack', 'Ping']) s.send(frame);
    const bytes = new Uint8Array([8, 9]); s.send(bytes); bytes[0] = 99;
    return 'sent';
  }
  if (mode === 'overflow') { for (let i = 0; i < 257; i++) s.send('too many'); return 'bad'; }
  s.send(mode);
  if (mode === 'throw') throw new Error('source failed');
  if (mode === 'async') return Promise.resolve('bad');
  if (mode === 'shape') return 123;
  return mode;
}
`);
  let browser, server;
  try {
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'socket-test', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      return new Response(Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1))));
    } });
    browser = await open({ host: 'web', url: server.url.href, webDist: resolve(dir, 'dist') });
    const until = async predicate => {
      const deadline = performance.now() + 15000;
      for (;;) {
        const state = await browser.state();
        if (predicate(state)) return state;
        if (performance.now() > deadline) throw new Error('socket state deadline: ' + JSON.stringify(state) + '\n' + (await browser.logs()).lines.join('\n'));
        await new Promise(r => setTimeout(r, 50));
      }
    };
    await browser.clock('data');
    const ready = await until(s => s.resources.feed === 'open' && s.resources.other === 'open');
    expect(ready.resources.denied).toBe('Refused');
    await until(s => s.resources.cancelSocket === 'open' && s.resources.cancelEvents === 'initial');
    await browser.tap('cancel');
    const cancelled = await until(s => !s.streams.some(x => ['cancelSocket', 'cancelEvents'].includes(x.name)));
    expect([cancelled.resources.cancelSocket, cancelled.resources.cancelEvents]).toEqual(['error:Aborted', 'error:Aborted']);
    expect(cancelled.streams.filter(s => s.readyState === 1)).toHaveLength(2);
    const oracle = JSON.parse(await browser.carrier.evaluate(`new Promise((resolve, reject) => {
      const ws = new WebSocket(${JSON.stringify(url)}), messages = [];
      ws.binaryType = 'arraybuffer';
      let early; try { ws.send('early'); } catch (e) { early = e.name; }
      ws.onopen = () => { for (const x of ['subscribe', 'Ack', 'Ping']) ws.send(x); ws.send(new Uint8Array([8, 9])); };
      ws.onmessage = e => {
        messages.push(typeof e.data === 'string' ? e.data : [...new Uint8Array(e.data)]);
        if (messages.length === 4) ws.close(4001, 'finished');
      };
      ws.onerror = () => reject(new Error('oracle socket failed'));
      ws.onclose = e => resolve(JSON.stringify({ messages, early, close: [e.code, e.reason, e.wasClean], buffered: ws.bufferedAmount }));
    })`));
    expect(oracle.early).toBe('InvalidStateError');
    expect(oracle.close).toEqual([4001, 'finished', true]);
    expect(oracle.buffered).toBe(0);
    received.length = 0;
    for (const mode of ['throw', 'async', 'shape', 'overflow', 'rollback']) await browser.tap(mode);
    await browser.tap('burst');
    const echoed = await until(s => s.resources.feed === 'subscribe|Ack|Ping|8,9|');
    expect(received).toEqual(oracle.messages);
    expect(echoed.resources.other).toBe('open');
    expect(echoed.streams.find(s => s.name === 'feed')).toMatchObject({ sent: 4, bufferedAmount: 0, coalesced: 0, grant: `net.websocket ${url}` });
    await browser.tap('twice');
    await browser.clock('settle');
    await until(s => s.resources.feed === 'subscribe|Ack|Ping|8,9|first|second|');
    oracle.messages.push('first', 'second');
    expect(received).toEqual(oracle.messages);
    await browser.tap('replace');
    await until(s => s.resources.feed === 'open' && s.streams.find(s => s.name === 'feed')?.sent === 0 && connections.size === 2);
    await browser.tap('stale');
    await browser.tap('close');
    const closed = await until(s => s.resources.feed === '4001:finished:true');
    expect(closed.streams.map(s => s.name)).toEqual(['other']);
    expect(received).toEqual(oracle.messages);
    const logs = (await browser.logs()).lines.join('\n');
    expect(logs).toContain('deliberate commit refusal');
    expect(logs).toContain('socket send:');
    expect(logs).toContain('socket close: 4001 finished');
    expect(logs).toContain('socket mutation exceeds 256');
    expect(logs).toContain('the socket is not open');
    // bufferedAmount counts what the mutation has staged.
    await browser.tap('probe');
    await until(s => JSON.stringify(s.slots.sent).includes('0:3') && s.resources.other === 'abc|');
    // A full buffer drains while the module sends nothing; a later turn
    // reads the lower count.
    await browser.tap('big');
    await until(s => received.includes('x900000'));
    let read;
    for (let i = 0; i < 40 && read !== 'read:0'; i++) {
      await browser.tap('read');
      read = (await until(s => JSON.stringify(s.slots.sent).includes('read:'))).slots.sent;
      read = String(read?.value ?? read);
    }
    expect(read).toBe('read:0');
    // A mutation cannot both send and fetch, in either order: no frame, no request.
    const before = received.length;
    await browser.tap('sendfetch');
    await browser.tap('fetchsend');
    await browser.clock('settle');
    await new Promise(r => setTimeout(r, 300));
    expect(received.length).toBe(before);
    expect(counted).toBe(0);
    const refused = (await browser.logs()).lines.join('\n');
    expect(refused.match(/a mutation cannot both send on a socket and fetch/g)?.length).toBe(2);
    // A mutation's fetch sends the request as it was when fetch was called.
    await browser.tap('captured');
    await until(s => JSON.stringify(s.slots.sent).includes('body:was=original'));
    expect(counted).toBe(0);
  } finally {
    await browser?.close(); server?.stop(true); peer.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 180000);
