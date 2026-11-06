import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { makeDocumentRuntime, drainDocumentRuntime, disposeDocumentRuntime } from '../../src/timeline/inline-document-host';
import { htmlRenderTheme } from '../../src/timeline/html-render';
import { STANDARD } from '../../src/appearance/settings-appearance-editor';

const ticket = (name: string) => ({ available: true, name }) as unknown as Native;
function fixture(readOnly: boolean) {
  const client = new T3Client(); client.threadId = 'conversation';
  const calls: { native: Native; method: string; payload: unknown; write: boolean }[] = [], sent: unknown[] = [];
  let owned = true;
  let replied!: () => void;
  const reply = new Promise<void>(resolve => { replied = resolve; });
  client.rpc = async (native, method, payload, write = false) => { calls.push({ native, method, payload, write }); return method === 'mcpApps.toolInfo' ? { callable: true, readOnly } : { content: [{ type: 'text', text: 'done' }] }; };
  const runtime = makeDocumentRuntime(client, { app: { attachmentId: 'a', server: 'weather', tool: 'forecast', resourceUri: 'ui://weather' }, sourceThreadId: 'source', conversationThreadId: 'conversation', itemId: 'item',
    post: value => { sent.push(value); replied(); }, theme: () => htmlRenderTheme(STANDARD.dark, 'dark'), owned: () => owned, width: () => 728, viewportHeight: () => 500, canExpand: () => true, height: () => {} });
  return { client, runtime, calls, sent, reply, loseOwner: () => { owned = false; }, leave: () => { owned = false; disposeDocumentRuntime(runtime); } };
}
const call = { jsonrpc: '2.0', id: 3, method: 'tools/call', params: { name: 'forecast', arguments: { city: 'Oslo' } } };

test('read-only MCP calls use the source item and the Native ticket of the current drain', async () => {
  const { runtime, calls, sent, reply } = fixture(true), native = ticket('read');
  runtime.host.receive(call);
  expect(calls).toEqual([]);
  await drainDocumentRuntime(runtime, native, 100);
  expect(calls.map(value => value.method)).toEqual(['mcpApps.toolInfo', 'mcpApps.callTool']);
  expect(calls.every(value => value.native === native)).toBe(true);
  expect(calls[1]?.write).toBe(false);
  expect(calls[1]?.payload).toEqual({ threadId: 'source', itemId: 'item', name: 'forecast', arguments: { city: 'Oslo' } });
  await reply;
  expect(sent).toContainEqual({ jsonrpc: '2.0', id: 3, result: { content: [{ type: 'text', text: 'done' }] } });
});

test('a mutating MCP call waits for approval and uses the approving answer ticket', async () => {
  const { runtime, calls } = fixture(false), before = ticket('before'), approving = ticket('approving');
  runtime.fullscreen = true;
  runtime.host.receive(call);
  await drainDocumentRuntime(runtime, before, 100);
  expect(runtime.fullscreen).toBe(false);
  expect(calls).toHaveLength(1);
  expect(runtime.permission?.text).toContain('Allow weather to run forecast?');
  const permission = runtime.permission!; runtime.permission = null; permission.settle(true);
  await drainDocumentRuntime(runtime, approving, 101);
  expect(calls).toHaveLength(2);
  expect(calls[1]?.native).toBe(approving);
  expect(calls[1]?.write).toBe(true);
});

test('declining or leaving a document never runs its queued mutating tool', async () => {
  const declined = fixture(false);
  declined.runtime.host.receive(call);
  await drainDocumentRuntime(declined.runtime, ticket('read'), 100);
  declined.runtime.permission!.settle(false); declined.runtime.permission = null;
  await drainDocumentRuntime(declined.runtime, ticket('decline'), 101);
  expect(declined.calls).toHaveLength(1);
  const closed = fixture(false);
  closed.runtime.host.receive(call);
  await drainDocumentRuntime(closed.runtime, ticket('read'), 100);
  closed.leave();
  await drainDocumentRuntime(closed.runtime, ticket('stale'), 101);
  expect(closed.calls).toHaveLength(1);
  expect(closed.sent).toEqual([]);
});

test('losing ownership during a read cancels later jobs and an existing approval', async () => {
  const { client, runtime, calls, sent, loseOwner } = fixture(false);
  runtime.host.receive(call);
  await drainDocumentRuntime(runtime, ticket('permission'), 100);
  expect(runtime.permission).not.toBeNull();
  let release!: () => void, started!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  const firstStarted = new Promise<void>(resolve => { started = resolve; });
  const rpc = client.rpc;
  client.rpc = async (native, method, payload, write) => {
    const result = await rpc(native, method, payload, write);
    if (method === 'mcpApps.readResource') { started(); await gate; }
    return result;
  };
  for (const id of [4, 5]) runtime.host.receive({ jsonrpc: '2.0', id, method: 'resources/read', params: { uri: `ui://weather/${id}` } });
  const draining = drainDocumentRuntime(runtime, ticket('old-connection'), 101);
  await firstStarted;
  loseOwner(); release();
  await draining;
  expect(calls.filter(value => value.method === 'mcpApps.readResource')).toHaveLength(1);
  expect(runtime.permission).toBeNull();
  expect(runtime.closed).toBe(true);
  expect(sent).toEqual([]);
});

test('an inline app asking to close is torn down first; full screen only steps back inline', async () => {
  const initialize = (runtime: ReturnType<typeof fixture>['runtime']) => {
    runtime.host.receive({ jsonrpc: '2.0', id: 'init', method: 'ui/initialize', params: { appCapabilities: {} } });
    runtime.host.receive({ jsonrpc: '2.0', method: 'ui/notifications/initialized' });
  };
  const request = { jsonrpc: '2.0', method: 'ui/notifications/request-teardown' };
  const teardowns = (sent: unknown[]) => sent.filter(value => (value as { method?: string }).method === 'ui/resource-teardown') as { id: string }[];

  const expanded = fixture(true);
  initialize(expanded.runtime);
  expanded.runtime.fullscreen = true;
  expanded.runtime.host.receive(request);
  expect(expanded.runtime.fullscreen).toBe(false);
  expect(expanded.runtime.closed).toBe(false);
  expect(teardowns(expanded.sent)).toEqual([]);

  const answered = fixture(true);
  initialize(answered.runtime);
  answered.runtime.host.receive(request);
  const [teardown] = teardowns(answered.sent);
  expect(teardown).toBeDefined();
  expect(answered.runtime.closed).toBe(false);
  answered.runtime.host.receive({ jsonrpc: '2.0', id: teardown!.id, result: {} });
  await Promise.resolve();
  expect(answered.runtime.closed).toBe(true);

  const silent = fixture(true);
  initialize(silent.runtime);
  silent.runtime.host.receive(request);
  expect(teardowns(silent.sent)).toHaveLength(1);
  await drainDocumentRuntime(silent.runtime, ticket('early'), 1999);
  expect(silent.runtime.closed).toBe(false);
  await drainDocumentRuntime(silent.runtime, ticket('late'), 2000);
  await Promise.resolve();
  expect(silent.runtime.closed).toBe(true);
});
