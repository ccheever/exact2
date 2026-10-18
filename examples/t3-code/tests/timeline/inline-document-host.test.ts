import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { makeDocumentRuntime, drainDocumentRuntime, disposeDocumentRuntime } from '../../src/timeline/inline-document-host';
import { htmlRenderTheme } from '../../src/timeline/html-render';
import { STANDARD } from '../../src/appearance/settings-appearance-editor';

const ticket = (name: string) => ({ available: true, name }) as unknown as Native;
function fixture(readOnly: boolean) {
  const client = new T3Client(); client.threadId = 'conversation';
  const calls: { native: Native; method: string; payload: unknown }[] = [], sent: unknown[] = [];
  let owned = true;
  let replied!: () => void;
  const reply = new Promise<void>(resolve => { replied = resolve; });
  client.rpc = async (native, method, payload) => { calls.push({ native, method, payload }); return method === 'mcpApps.toolInfo' ? { callable: true, readOnly } : { content: [{ type: 'text', text: 'done' }] }; };
  const runtime = makeDocumentRuntime(client, { app: { attachmentId: 'a', server: 'weather', tool: 'forecast', resourceUri: 'ui://weather' }, sourceThreadId: 'source', conversationThreadId: 'conversation', itemId: 'item',
    post: value => { sent.push(value); replied(); }, theme: () => htmlRenderTheme(STANDARD.dark, 'dark'), owned: () => owned, width: () => 728, viewportHeight: () => 500, canExpand: () => true, height: () => {} });
  return { runtime, calls, sent, reply, leave: () => { owned = false; disposeDocumentRuntime(runtime); } };
}
const call = { jsonrpc: '2.0', id: 3, method: 'tools/call', params: { name: 'forecast', arguments: { city: 'Oslo' } } };

test('read-only MCP calls use the source item and the Native ticket of the current drain', async () => {
  const { runtime, calls, sent, reply } = fixture(true), native = ticket('read');
  runtime.host.receive(call);
  expect(calls).toEqual([]);
  await drainDocumentRuntime(runtime, native, 100);
  expect(calls.map(value => value.method)).toEqual(['mcpApps.toolInfo', 'mcpApps.callTool']);
  expect(calls.every(value => value.native === native)).toBe(true);
  expect(calls[1]?.payload).toEqual({ threadId: 'source', itemId: 'item', name: 'forecast', arguments: { city: 'Oslo' } });
  await reply;
  expect(sent).toContainEqual({ jsonrpc: '2.0', id: 3, result: { content: [{ type: 'text', text: 'done' }] } });
});

test('a mutating MCP call waits for approval and uses the approving answer ticket', async () => {
  const { runtime, calls } = fixture(false), before = ticket('before'), approving = ticket('approving');
  runtime.host.receive(call);
  await drainDocumentRuntime(runtime, before, 100);
  expect(calls).toHaveLength(1);
  expect(runtime.permission?.text).toContain('Allow weather to run forecast?');
  const permission = runtime.permission!; runtime.permission = null; permission.settle(true);
  await drainDocumentRuntime(runtime, approving, 101);
  expect(calls).toHaveLength(2);
  expect(calls[1]?.native).toBe(approving);
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
