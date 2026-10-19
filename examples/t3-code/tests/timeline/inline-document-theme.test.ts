import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { inlineDocumentEvent, inlineDocumentKey, inlineDocumentView, inlineDocumentsPosted, inlineReference, inlineTheme, syncInlineDocuments } from '../../src/timeline/inline-documents';

const native = { available: true } as Native;
function fixture(kind: 'html' | 'mcp') {
  const client = new T3Client();
  client.threadId = 'thread'; client.environmentId = 'env'; client.origin = 'http://127.0.0.1:1'; client.generation = 1;
  const item = { id: 'item', type: 'dynamic_tool', status: 'completed', toolName: kind === 'html' ? 'html_render' : 'weather.forecast',
    output: kind === 'html' ? { htmlRender: { attachmentId: 'doc', title: 'Report', height: 320 } } : { t3McpApp: { attachmentId: 'doc', server: 'weather', tool: 'forecast', resourceUri: 'ui://weather' } } };
  client.thread = { projection: { thread: { id: 'thread' }, visibleTurnItems: [{ sourceThreadId: 'thread', sourceItemId: 'item', item }] } };
  let mints = 0;
  client.rpc = async () => { mints++; return { relativeUrl: '/api/assets/doc', expiresAt: 10_000_000 }; };
  const ref = inlineReference(item)!, key = inlineDocumentKey(client, 'thread', 'item', ref);
  const view = () => inlineDocumentView(client, key, ref);
  const config = () => JSON.parse(decodeURIComponent(view().src.split('#')[1]!));
  const receive = (message: unknown) => inlineDocumentEvent(client, key, 'message', JSON.stringify({ t3Document: config().token, message }), native);
  const drainPosts = async () => {
    const messages: Record<string, unknown>[] = [];
    while (view().posts.length) {
      const post = view().posts[0]!; messages.push(JSON.parse(post.text).message);
      await inlineDocumentEvent(client, key, 'ack', post.key, native);
    }
    return messages;
  };
  return { client, view, config, receive, drainPosts, mintCount: () => mints };
}

test('document system themes use the resolved host scheme while explicit appearance wins', () => {
  const { client } = fixture('html');
  expect(inlineTheme(client, 'light').appearance).toBe('light');
  expect(inlineTheme(client, 'dark').appearance).toBe('dark');
  client.local.deviceSettings.appearanceMode = 'dark';
  expect(inlineTheme(client, 'light').appearance).toBe('dark');
  client.local.deviceSettings.appearanceMode = 'light';
  expect(inlineTheme(client, 'dark').appearance).toBe('light');
});

for (const kind of ['html', 'mcp'] as const) test(`${kind} live theme and font updates retain the mounted document and signed URL`, async () => {
  const { client, view, receive, drainPosts, mintCount } = fixture(kind);
  await syncInlineDocuments(client, native, 0, 'light');
  const src = view().src;
  if (kind === 'mcp') {
    await receive({ jsonrpc: '2.0', id: 1, method: 'ui/initialize', params: {} });
    expect(await drainPosts()).toMatchObject([{ result: { hostContext: { theme: 'light' } } }]);
    await receive({ jsonrpc: '2.0', method: 'ui/notifications/initialized' });
  }
  await receive({ jsonrpc: '2.0', method: 't3/adapter-ready' });
  await drainPosts();
  client.local.clientSettings.fontFamilySans = 'Georgia';
  await syncInlineDocuments(client, native, 1, 'dark');
  const messages = await drainPosts();
  const theme = messages.find(message => message.method === 'ui/notifications/host-context-changed');
  expect(theme).toMatchObject({ params: { theme: 'dark' } });
  expect(JSON.stringify(theme)).toContain('Georgia');
  expect(view().src).toBe(src);
  expect(mintCount()).toBe(1);
});

for (const kind of ['html', 'mcp'] as const) test(`${kind}: a theme change during the attachments read counts as posted, so the snapshot carrying it is read again`, async () => {
  const { client, receive, drainPosts } = fixture(kind);
  await syncInlineDocuments(client, native, 0, 'light');
  if (kind === 'mcp') {
    await receive({ jsonrpc: '2.0', id: 1, method: 'ui/initialize', params: {} });
    await receive({ jsonrpc: '2.0', method: 'ui/notifications/initialized' });
  }
  await receive({ jsonrpc: '2.0', method: 't3/adapter-ready' });
  await drainPosts();
  await syncInlineDocuments(client, native, 1, 'light');
  const before = inlineDocumentsPosted(client);
  await syncInlineDocuments(client, native, 2, 'light');
  expect(inlineDocumentsPosted(client)).toBe(before);
  await syncInlineDocuments(client, native, 3, 'dark');
  expect(inlineDocumentsPosted(client)).toBeGreaterThan(before);
  expect((await drainPosts()).some(message => JSON.stringify(message).includes('"dark"'))).toBe(true);
});
