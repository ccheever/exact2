import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { obj, type Obj } from '../../src/client/domain';
import { inlineDocumentEvent, inlineDocumentKey, inlineDocumentView, inlineReference, syncInlineDocuments } from '../../src/timeline/inline-documents';
import { timelineEntries } from '../../src/timeline/timeline-rows';
import { transcriptRows } from '../../src/timeline/timeline-presentation';

const native = { available: true } as Native;
const html = { attachmentId: 'a', title: 'Report', height: 400 };
const item = { id: 'i', type: 'dynamic_tool', status: 'completed', toolName: 'html_render', output: { htmlRender: html } };
const ref = inlineReference(item)!;
function fixture(renderItem: Obj = item) {
  const client = new T3Client(); client.threadId = 't'; client.environmentId = 'env'; client.generation = 1; client.origin = 'http://127.0.0.1:1';
  client.thread = { projection: { thread: { id: 't' }, visibleTurnItems: [{ sourceThreadId: 't', sourceItemId: 'i', item: renderItem }] } };
  return client;
}

test('completed render tools become standalone timeline rows instead of grouped tool output', () => {
  const client = fixture();
  expect(timelineEntries({ rows: obj(client.thread?.projection).visibleTurnItems, attempts: [], nodes: [] })).toMatchObject([{ kind: 'document', document: { kind: 'html', reference: html } }]);
  expect(transcriptRows(client)).toMatchObject([{ kind: 'inline-document', inlineDocuments: [{ title: 'Report', src: '' }] }]);
  expect(inlineReference({ ...item, status: 'running' })).toBeNull();
});

test('signed URLs are chosen once and retain the mounted document through refreshes', async () => {
  const client = fixture(), calls: unknown[] = [];
  client.rpc = async (_native, method, payload) => { calls.push({ method, payload }); return { relativeUrl: '/api/assets/a?bucket=1', expiresAt: 100 }; };
  const key = inlineDocumentKey(client, 't', 'i', ref);
  await syncInlineDocuments(client, native, 0);
  const initial = inlineDocumentView(client, key, ref);
  expect(initial.src).toStartWith('assets/inline-document/index.html#');
  const config = JSON.parse(decodeURIComponent(initial.src.split('#')[1]!));
  expect(config.src).toStartWith('http://127.0.0.1:1/api/assets/a?bucket=1#t3-theme=');
  await syncInlineDocuments(client, native, 1_000_000);
  expect(calls).toHaveLength(1);
  expect(inlineDocumentView(client, key, ref).src).toBe(initial.src);
  inlineDocumentEvent(client, key, 'loaded', '');
  inlineDocumentEvent(client, key, 'message', JSON.stringify({ t3Document: config.token, message: { jsonrpc: '2.0', method: 'ui/notifications/size-changed', params: { height: 250 } } }));
  expect(inlineDocumentView(client, key, ref)).toMatchObject({ ready: true, height: 250 });
});

test('a failed URL mint remains an error until an explicit retry and never spins on refresh', async () => {
  const client = fixture(); let calls = 0;
  client.rpc = async () => { calls++; throw new Error('offline'); };
  const key = inlineDocumentKey(client, 't', 'i', ref);
  await syncInlineDocuments(client, native, 0);
  await syncInlineDocuments(client, native, 1);
  expect(calls).toBe(1);
  expect(inlineDocumentView(client, key, ref).error).toBe('offline');
  inlineDocumentEvent(client, key, 'retry', '');
  client.rpc = async () => ({ relativeUrl: '/api/assets/retry', expiresAt: 10_000_000 });
  await syncInlineDocuments(client, native, 2);
  expect(inlineDocumentView(client, key, ref)).toMatchObject({ error: '', ready: false });
  expect(decodeURIComponent(inlineDocumentView(client, key, ref).src)).toContain('/api/assets/retry');
});

test('a reply from the previous connection never publishes into the replacement frame', async () => {
  const client = fixture(); let resolve!: (value: unknown) => void;
  client.rpc = () => new Promise(done => { resolve = done; });
  const pending = syncInlineDocuments(client, native, 0);
  client.generation++;
  resolve({ relativeUrl: '/api/assets/stale', expiresAt: 10_000_000 });
  await pending;
  const key = inlineDocumentKey(client, 't', 'i', ref);
  expect(inlineDocumentView(client, key, ref).src).toBe('');
  client.rpc = async () => ({ relativeUrl: '/api/assets/new', expiresAt: 10_000_000 });
  await syncInlineDocuments(client, native, 0);
  expect(decodeURIComponent(inlineDocumentView(client, key, ref).src)).toContain('/api/assets/new');
});


test('expanding and restoring a document keeps its source and ignores fullscreen height reports', async () => {
  const client = fixture();
  let mints = 0;
  client.rpc = async () => { mints++; return { relativeUrl: '/api/assets/resident', expiresAt: 10_000_000 }; };
  const key = inlineDocumentKey(client, 't', 'i', ref);
  await syncInlineDocuments(client, native, 0);
  const initial = inlineDocumentView(client, key, ref);
  const config = JSON.parse(decodeURIComponent(initial.src.split('#')[1]!));
  await inlineDocumentEvent(client, key, 'fullscreen', 'open');
  await inlineDocumentEvent(client, key, 'measure', '1280|840');
  await inlineDocumentEvent(client, key, 'message', JSON.stringify({ t3Document: config.token, message: { jsonrpc: '2.0', method: 'ui/notifications/size-changed', params: { height: 840 } } }));
  expect(inlineDocumentView(client, key, ref)).toMatchObject({ fullscreen: true, src: initial.src, height: initial.height });
  await inlineDocumentEvent(client, key, 'fullscreen', 'close');
  await syncInlineDocuments(client, native, 1);
  expect(inlineDocumentView(client, key, ref)).toMatchObject({ fullscreen: false, src: initial.src, height: initial.height });
  expect(mints).toBe(1);
});


test('inline width selects responsive measurements and is retained while fullscreen', async () => {
  const responsiveItem = { ...item, output: { htmlRender: { ...html, height: 600, heights: [[364, 560], [728, 200]] } } };
  const client = fixture(responsiveItem), responsiveRef = inlineReference(responsiveItem)!;
  client.rpc = async () => ({ relativeUrl: '/api/assets/responsive', expiresAt: 10_000_000 });
  const key = inlineDocumentKey(client, 't', 'i', responsiveRef);
  await syncInlineDocuments(client, native, 0);
  expect(inlineDocumentView(client, key, responsiveRef).height).toBe(200);
  await inlineDocumentEvent(client, key, 'measure', '364|560');
  const inline = inlineDocumentView(client, key, responsiveRef);
  expect(inline.height).toBe(560);
  await inlineDocumentEvent(client, key, 'fullscreen', 'open');
  await inlineDocumentEvent(client, key, 'measure', '1280|840');
  expect(inlineDocumentView(client, key, responsiveRef)).toMatchObject({ height: 560 });
  await inlineDocumentEvent(client, key, 'fullscreen', 'close');
  expect(inlineDocumentView(client, key, responsiveRef).height).toBe(560);
});

test('MCP fullscreen publishes the measured guest viewport and retains its document on restore', async () => {
  const appItem = { ...item, toolName: 'weather.forecast', output: { t3McpApp: { attachmentId: 'mcp', server: 'weather', tool: 'forecast', resourceUri: 'ui://weather' } } };
  const client = fixture(appItem), appRef = inlineReference(appItem)!;
  let mints = 0;
  client.rpc = async () => { mints++; return { relativeUrl: '/api/assets/mcp', expiresAt: 10_000_000 }; };
  const key = inlineDocumentKey(client, 't', 'i', appRef);
  await syncInlineDocuments(client, native, 0);
  const initial = inlineDocumentView(client, key, appRef);
  expect(initial).toMatchObject({ title: 'forecast', server: 'weather' });
  const config = JSON.parse(decodeURIComponent(initial.src.split('#')[1]!));
  const message = (value: unknown) => inlineDocumentEvent(client, key, 'message', JSON.stringify({ t3Document: config.token, message: value }));
  await inlineDocumentEvent(client, key, 'measure', '728|320');
  await message({ jsonrpc: '2.0', id: 'init', method: 'ui/initialize', params: { appCapabilities: {} } });
  await message({ jsonrpc: '2.0', method: 'ui/notifications/initialized' });
  const context = () => inlineDocumentView(client, key, appRef).posts.map(post => JSON.parse(post.text).message)
    .filter(value => value.method === 'ui/notifications/host-context-changed').reduce((value, message) => ({ ...value, ...message.params }), {});
  await inlineDocumentEvent(client, key, 'fullscreen', 'open');
  await inlineDocumentEvent(client, key, 'measure', '1280|800');
  expect(context()).toMatchObject({ displayMode: 'fullscreen', containerDimensions: { width: 1280, height: 800 } });
  await message({ jsonrpc: '2.0', method: 'ui/notifications/size-changed', params: { height: 800 } });
  expect(inlineDocumentView(client, key, appRef)).toMatchObject({ fullscreen: true, height: initial.height, src: initial.src });
  await inlineDocumentEvent(client, key, 'fullscreen', 'close');
  await inlineDocumentEvent(client, key, 'measure', '728|320');
  expect(context()).toMatchObject({ displayMode: 'inline', containerDimensions: { width: 728, maxHeight: 2000 } });
  expect(inlineDocumentView(client, key, appRef)).toMatchObject({ fullscreen: false, height: initial.height, src: initial.src });
  expect(mints).toBe(1);
});

test('an MCP app opens full screen only right after input in its own frame (McpAppFrame)', async () => {
  const appItem = { ...item, toolName: 'weather.forecast', output: { t3McpApp: { attachmentId: 'mcp', server: 'weather', tool: 'forecast', resourceUri: 'ui://weather' } } };
  const client = fixture(appItem), appRef = inlineReference(appItem)!;
  client.rpc = async () => ({ relativeUrl: '/api/assets/mcp', expiresAt: 10_000_000 });
  const key = inlineDocumentKey(client, 't', 'i', appRef);
  await syncInlineDocuments(client, native, 0);
  const config = JSON.parse(decodeURIComponent(inlineDocumentView(client, key, appRef).src.split('#')[1]!));
  const message = (value: unknown, activation?: unknown) => inlineDocumentEvent(client, key, 'message', JSON.stringify({ t3Document: config.token, message: value, ...(activation ? { activation } : {}) }));
  await message({ jsonrpc: '2.0', id: 'init', method: 'ui/initialize', params: { appCapabilities: { availableDisplayModes: ['inline', 'fullscreen'] } } });
  await message({ jsonrpc: '2.0', method: 'ui/notifications/initialized' });
  const request = { jsonrpc: '2.0', method: 'ui/request-display-mode', params: { mode: 'fullscreen' } };
  // On load, from a timer, or with the focus but no tap or key press in the frame: the app stays in the thread.
  await message({ ...request, id: 1 });
  await message({ ...request, id: 2 }, { focused: true, active: false });
  await message({ ...request, id: 3 }, { focused: false, active: true });
  expect(inlineDocumentView(client, key, appRef).fullscreen).toBe(false);
  // Right after a tap in the frame it opens.
  await message({ ...request, id: 4 }, { focused: true, active: true });
  expect(inlineDocumentView(client, key, appRef).fullscreen).toBe(true);
});

for (const identity of ['generation', 'environment', 'origin', 'thread'] as const) {
  test(`MCP queued reads stay with their ${identity} identity while the first read awaits`, async () => {
    const appItem = { ...item, toolName: 'weather.forecast', output: { t3McpApp: { attachmentId: 'mcp', server: 'weather', tool: 'forecast', resourceUri: 'ui://weather' } } };
    const client = fixture(appItem), appRef = inlineReference(appItem)!;
    let release!: () => void, started!: () => void;
    const gate = new Promise<void>(resolve => { release = resolve; });
    const firstStarted = new Promise<void>(resolve => { started = resolve; });
    const reads: { origin: string; payload: unknown }[] = [];
    client.rpc = async (_native, method, payload) => {
      if (method === 'assets.createUrl') return { relativeUrl: '/api/assets/mcp', expiresAt: 10_000_000 };
      if (method === 'mcpApps.readResource') { reads.push({ origin: client.origin, payload }); if (reads.length === 1) { started(); await gate; } return { contents: [] }; }
      return {};
    };
    const key = inlineDocumentKey(client, 't', 'i', appRef);
    await syncInlineDocuments(client, native, 0);
    const initial = inlineDocumentView(client, key, appRef);
    const config = JSON.parse(decodeURIComponent(initial.src.split('#')[1]!));
    const message = (id: number) => JSON.stringify({ t3Document: config.token, message: { jsonrpc: '2.0', id, method: 'resources/read', params: { uri: `ui://weather/${id}` } } });
    const draining = inlineDocumentEvent(client, key, 'message', message(1), native);
    await firstStarted;
    await inlineDocumentEvent(client, key, 'message', message(2));
    if (identity === 'generation') client.generation++;
    else if (identity === 'environment') client.environmentId = 'replacement';
    else if (identity === 'origin') client.origin = 'http://127.0.0.1:2';
    else client.threadId = 'replacement';
    release(); await draining;
    expect(reads).toEqual([{ origin: 'http://127.0.0.1:1', payload: { threadId: 't', itemId: 'i', uri: 'ui://weather/1' } }]);
  });
}

test('a suspended cached MCP drain cannot mint the next document on a replacement connection', async () => {
  const appItem = { ...item, toolName: 'weather.forecast', output: { t3McpApp: { attachmentId: 'mcp', server: 'weather', tool: 'forecast', resourceUri: 'ui://weather' } } };
  const client = fixture(appItem), appRef = inlineReference(appItem)!;
  let release!: () => void, started!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  const firstStarted = new Promise<void>(resolve => { started = resolve; });
  const mints: string[] = [];
  client.rpc = async (_native, method) => {
    if (method === 'assets.createUrl') { mints.push(client.origin); return { relativeUrl: '/api/assets/document', expiresAt: 10_000_000 }; }
    if (method === 'mcpApps.readResource') { started(); await gate; return { contents: [] }; }
    return {};
  };
  const key = inlineDocumentKey(client, 't', 'i', appRef);
  await syncInlineDocuments(client, native, 0);
  const config = JSON.parse(decodeURIComponent(inlineDocumentView(client, key, appRef).src.split('#')[1]!));
  await inlineDocumentEvent(client, key, 'message', JSON.stringify({ t3Document: config.token, message: { jsonrpc: '2.0', id: 1, method: 'resources/read', params: { uri: 'ui://weather/current' } } }));
  client.thread!.projection.visibleTurnItems = [{ sourceThreadId: 't', item: appItem }, { sourceThreadId: 't', item: { ...item, id: 'next' } }];
  const syncing = syncInlineDocuments(client, native, 1);
  await firstStarted;
  client.origin = 'http://127.0.0.1:2';
  release(); await syncing;
  expect(mints).toEqual(['http://127.0.0.1:1']);
});
