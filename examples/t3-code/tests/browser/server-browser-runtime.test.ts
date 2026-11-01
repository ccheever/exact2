import { expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { adoptAppFiles } from '../../src/media/save-file';
import { noteNow } from '../../src/composer/composer-controls';
import { prepareServerStream, serverStreamSource, serverStreamInput, serverStreamFrame, serverStreamUrl, streamPagePoint, drainServerNotices, retainServerStreams, serverStreamDecode, serverStreamPainted } from '../../src/browser/server-browser-runtime';
import type { SocketEvent, SocketHandle } from '../../src/transport/transport-session';
const native = { available: true } as Native;
function fixture() {
  let tickets = 0, ids = 0;
  const files = new Map<string, Uint8Array>();
  adoptAppFiles({ fs: { mkdir: async () => {}, writeFile: async (path: string, bytes: Uint8Array) => { files.set(path, bytes); }, readFile: async (path: string) => files.get(path)?.buffer ?? new ArrayBuffer(0), rm: async (path: string) => { for (const key of files.keys()) if (key === path || key.startsWith(path + '/')) files.delete(key); } } });
  const client = { generation: 1, connection: 'connected', async ids() { return [`frame-store-${++ids}`]; }, async call() { return { wsBase: 'ws://127.0.0.1/api/preview-stream', ticket: `ticket-${++tickets}` }; } } as unknown as T3Client;
  const sent: Record<string, unknown>[] = [], socket: SocketHandle = { readyState: 1, bufferedAmount: 0, send(text) { sent.push(JSON.parse(text)); }, close() {} };
  noteNow(client, 1000);
  return { client, sent, socket, files, get tickets() { return tickets; } };
}
function withSocket(client: T3Client, id: string, socket: SocketHandle, run: (event: (value: SocketEvent) => void) => void) {
  const original = globalThis.fetch;
  try {
    let callback: ((event: SocketEvent, socket?: SocketHandle) => unknown) | undefined;
    globalThis.fetch = ((_url: unknown, options: unknown) => { callback = (options as { exactStream: typeof callback }).exactStream; return Promise.resolve(new Response()); }) as typeof fetch;
    serverStreamSource(client, id, serverStreamUrl(client, id));
    run(value => { callback!(value, socket); });
  } finally { globalThis.fetch = original; }
}
test('page coordinates exclude letterboxing and preserve CSS viewport units', () => {
  expect(streamPagePoint(200, 200, 800, 400, 1200, 800)).toEqual({ x: 200, y: 400 });
  expect(streamPagePoint(20, 200, 800, 400, 1200, 800)).toBeNull();
});
test('one active tab owns the stream, input requires its controller and resizing grows the image cap', async () => {
  const run = fixture(); await prepareServerStream(run.client, native, 'one', 'thread', 'tab');
  const first = serverStreamUrl(run.client, 'one');
  expect(new URL(first).searchParams.get('wsTicket')).toBe('ticket-1');
  await prepareServerStream(run.client, native, 'one', 'thread', 'tab'); expect(run.tickets).toBe(1);
  withSocket(run.client, 'one', run.socket, event => {
    event({ type: 'open' }); event({ type: 'message', data: JSON.stringify({ type: 'viewport', width: 1200, height: 800 }) });
    event({ type: 'message', data: JSON.stringify({ type: 'control', canOperate: true, controller: 'agent', generation: 1, dialog: null }) });
    serverStreamInput(run.client, 'one', 'key', { key: 'Enter', code: 'Enter' }); expect(run.sent).toEqual([]);
    serverStreamInput(run.client, 'one', 'copy', {}); expect(run.sent).toEqual([]); // only the controller's copy reaches the page
    serverStreamInput(run.client, 'one', 'takeControl', {}); expect(run.sent.at(-1)).toEqual({ type: 'takeControl' });
    event({ type: 'message', data: JSON.stringify({ type: 'control', canOperate: true, controller: 'you', generation: 2, dialog: null }) });
    serverStreamInput(run.client, 'one', 'mouse', { phase: 'down', buttons: 1, x: 200, y: 200, width: 800, height: 400 });
    expect(run.sent.at(-1)).toMatchObject({ type: 'mouse', action: 'down', button: 'left', x: 200, y: 400, clickCount: 1 });
    serverStreamInput(run.client, 'one', 'key', { key: 'Enter', code: 'Enter', phase: 'down' });
    expect(run.sent.at(-1)).toMatchObject({ type: 'key', action: 'down', keyCode: 13, text: '\r' });
    // handleClipboard: the menu bar's Copy and Cut arrive as Control+C and Control+X (modifiers 2).
    const before = run.sent.length;
    serverStreamInput(run.client, 'one', 'copy', {}); serverStreamInput(run.client, 'one', 'cut', {});
    expect(run.sent.slice(before)).toEqual([
      { type: 'key', action: 'down', key: 'c', code: 'KeyC', keyCode: 67, modifiers: 2 }, { type: 'key', action: 'up', key: 'c', code: 'KeyC', keyCode: 67, modifiers: 2 },
      { type: 'key', action: 'down', key: 'x', code: 'KeyX', keyCode: 88, modifiers: 2 }, { type: 'key', action: 'up', key: 'x', code: 'KeyX', keyCode: 88, modifiers: 2 },
    ]);
    serverStreamInput(run.client, 'one', 'back', {}); expect(run.sent.at(-1)).toEqual({ type: 'history', delta: -1 });
    serverStreamInput(run.client, 'one', 'forward', {}); expect(run.sent.at(-1)).toEqual({ type: 'history', delta: 1 });
    serverStreamInput(run.client, 'one', 'resize', { width: 1600, height: 1000 });
    expect(run.sent.at(-1)).toEqual({ type: 'resize', width: 1600, height: 1000 });
  });
  await prepareServerStream(run.client, native, 'one', 'thread', 'tab');
  expect(new URL(serverStreamUrl(run.client, 'one')).searchParams.get('maxWidth')).toBe('3200');
  await prepareServerStream(run.client, native, '', '', ''); expect(serverStreamUrl(run.client, 'one')).toBe(''); expect(serverStreamFrame(run.client, 'one').image).toBe('');
});
test('a delayed access reply cannot restore a hidden tab or an older connection', async () => {
  const run = fixture(); let finish!: (access: Record<string, unknown>) => void;
  run.client.call = () => new Promise(resolve => { finish = resolve; });
  const first = prepareServerStream(run.client, native, 'one', 'thread', 'tab');
  await Promise.resolve();
  await prepareServerStream(run.client, native, '', '', ''); finish({ wsBase: 'ws://127.0.0.1/api/preview-stream', ticket: 'old' }); await first;
  expect(serverStreamUrl(run.client, 'one')).toBe('');
});

test('retry delays use the maintenance clock after an idle page, and notices stop that clock when drained', async () => {
  const run = fixture(); await prepareServerStream(run.client, native, 'one', 'thread', 'tab');
  const first = serverStreamUrl(run.client, 'one');
  withSocket(run.client, 'one', run.socket, event => { event({ type: 'open' }); event({ type: 'close', code: 1001 }); });
  expect(serverStreamFrame(run.client, 'one').attention).toBe(true);
  noteNow(run.client, 10000); await prepareServerStream(run.client, native, 'one', 'thread', 'tab'); expect(run.tickets).toBe(1);
  noteNow(run.client, 10499); await prepareServerStream(run.client, native, 'one', 'thread', 'tab'); expect(run.tickets).toBe(1);
  noteNow(run.client, 10500); await prepareServerStream(run.client, native, 'one', 'thread', 'tab'); expect(run.tickets).toBe(2);
  expect(serverStreamUrl(run.client, 'one')).not.toBe(first);
  withSocket(run.client, 'one', run.socket, event => { event({ type: 'open' }); event({ type: 'message', data: JSON.stringify({ type: 'popup', tabId: 'new-tab' }) }); });
  const popups: string[] = []; expect(serverStreamFrame(run.client, 'one').attention).toBe(true);
  await drainServerNotices(run.client, native, 'one', tab => popups.push(tab));
  expect(popups).toEqual(['new-tab']); expect(serverStreamFrame(run.client, 'one').attention).toBe(false);
});

test('a panel and a floating tab have independent sockets and permission gates', async () => {
  const run = fixture(); await prepareServerStream(run.client, native, 'panel', 'thread', 'tab-one'); await prepareServerStream(run.client, native, 'mini', 'thread', 'tab-two');
  await retainServerStreams(run.client, ['panel', 'mini']);
  withSocket(run.client, 'panel', run.socket, event => { event({ type: 'open' }); event({ type: 'message', data: JSON.stringify({ type: 'control', canOperate: true, controller: 'you', generation: 1, dialog: null }) }); });
  withSocket(run.client, 'mini', run.socket, event => { event({ type: 'open' }); event({ type: 'message', data: JSON.stringify({ type: 'control', canOperate: false, controller: 'agent', generation: 1, dialog: null }) }); });
  expect(serverStreamFrame(run.client, 'panel').controlled).toBe(true); expect(serverStreamFrame(run.client, 'mini').controlled).toBe(false);
  serverStreamInput(run.client, 'mini', 'text', { text: 'blocked' }); expect(run.sent).toHaveLength(0);
  serverStreamInput(run.client, 'panel', 'text', { text: 'allowed' }); expect(run.sent).toEqual([{ type: 'text', text: 'allowed' }]);
  await retainServerStreams(run.client, ['mini']); expect(serverStreamUrl(run.client, 'panel')).toBe(''); expect(serverStreamUrl(run.client, 'mini')).not.toBe('');
});

test('large JPEGs use app files, retain only the latest queued frame and remove retired files', async () => {
  const run = fixture(); await prepareServerStream(run.client, native, 'one', 'thread', 'tab');
  const large = new Uint8Array(2 * 1024 * 1024); large[0] = 255; large[1] = 216;
  withSocket(run.client, 'one', run.socket, event => { event({ type: 'open' }); event({ type: 'message', data: large.buffer }); });
  const first = await serverStreamDecode(run.client, 'one'); expect(first.image).toStartWith('app:/tmp/browser-frames/');
  expect(run.files.get(first.image)?.byteLength).toBe(large.byteLength); expect(run.files.size).toBe(1);
  withSocket(run.client, 'one', run.socket, event => { event({ type: 'message', data: new Uint8Array([1, 2]).buffer }); event({ type: 'message', data: new Uint8Array([3, 4]).buffer }); });
  await serverStreamPainted(run.client, 'one', first.image);
  const second = await serverStreamDecode(run.client, 'one'); expect([...run.files.get(second.image)!]).toEqual([3, 4]); expect(run.files.size).toBe(2);
  await serverStreamPainted(run.client, 'one', second.image); expect(run.files.size).toBe(1);
  await retainServerStreams(run.client, []); expect(run.files.size).toBe(0);
  expect(run.sent.filter(frame => frame.type === 'ack')).toHaveLength(3);
});

test('the refused state\'s Reconnect clears the refusal and asks for new stream access', async () => {
  const run = fixture(); await prepareServerStream(run.client, native, 'one', 'thread', 'tab');
  for (let attempt = 0; attempt < 3; attempt++) {
    // The access is asked for again after the backoff (the first prepare after a close starts its clock).
    if (attempt) for (const at of [100000 * attempt, 100000 * attempt + 50000]) { noteNow(run.client, at); await prepareServerStream(run.client, native, 'one', 'thread', 'tab'); }
    withSocket(run.client, 'one', run.socket, event => { event({ type: 'close', code: 4401 }); });
  }
  expect(serverStreamFrame(run.client, 'one').refused).toBe(true);
  const tickets = run.tickets;
  noteNow(run.client, 900000); await prepareServerStream(run.client, native, 'one', 'thread', 'tab'); expect(run.tickets).toBe(tickets);
  serverStreamInput(run.client, 'one', 'retry', {});
  expect(serverStreamFrame(run.client, 'one').refused).toBe(false);
  await prepareServerStream(run.client, native, 'one', 'thread', 'tab'); expect(run.tickets).toBe(tickets + 1);
});
