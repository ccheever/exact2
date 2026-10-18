import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { Backend, storage } from '../support/client-fixture';
import { obj, str, type Obj } from '../../src/client/domain';
import { commandOutputView, syncCommandOutputs } from '../../src/timeline/command-output';
import { setTurnItemOpen } from '../../src/timeline/timeline-item-fetch';

async function connected() {
  const native = new Backend(), client = new T3Client(), disk = storage().files;
  obj(native.config.environment).capabilities = { serverResolvedCommandContext: true, commandOutputStreaming: true };
  const projection = obj(native.details.t1.projection);
  const item = { id: 'cmd', threadId: 't1', type: 'command_execution', runId: null, nodeId: null, ordinal: 1, status: 'running', input: 'bun test parser' };
  projection.turnItems = [item]; projection.visibleTurnItems = [{ sourceThreadId: 't1', sourceItemId: 'cmd', position: 0, visibility: 'local', item }];
  await client.refresh(native, disk); await client.command('select-thread', 't1', '', 0, native, disk); await client.refresh(native, disk);
  return { native, client, disk, item, row: JSON.stringify(['t1', 'cmd']) };
}
const subscriptions = (native: Backend) => native.calls.filter(call => call.op === 'subscribe' && call.method === 'orchestration.subscribeCommandOutput');
test('only expanded commands subscribe, stream through the actual client batch, then unsubscribe on collapse', async () => {
  const { native, client, disk, item, row } = await connected();
  await syncCommandOutputs(client, native); expect(subscriptions(native)).toHaveLength(0);
  setTurnItemOpen(client, row, true); await syncCommandOutputs(client, native); await syncCommandOutputs(client, native);
  expect(subscriptions(native)).toHaveLength(1);
  const call = subscriptions(native)[0]!; expect(call.payload).toEqual({ threadId: 't1', itemId: 'cmd' });
  const key = str(call.key);
  native.emit(key, { kind: 'replace', text: 'line one\n', truncated: false, running: true });
  await client.refresh(native, disk);
  expect(commandOutputView(client, row, item)?.value?.output.text).toBe('line one\n');
  native.emit(key, { kind: 'append', text: 'line two\n', truncated: false, running: false });
  await client.refresh(native, disk);
  expect(commandOutputView(client, row, item)?.value).toMatchObject({ running: false, output: { text: 'line one\nline two\n' } });
  setTurnItemOpen(client, row, false); await syncCommandOutputs(client, native);
  expect(native.calls.some(call => call.op === 'unsubscribe' && call.key === key)).toBe(true);
  native.emit(key, { kind: 'append', text: 'late', truncated: false, running: true }); await client.refresh(native, disk);
  expect(commandOutputView(client, row, item)?.value).toBeNull();
});
test('navigation unsubscribes old commands and rejects their late events', async () => {
  const { native, client, disk, item, row } = await connected();
  setTurnItemOpen(client, row, true); await syncCommandOutputs(client, native);
  const key = str(subscriptions(native)[0]!.key);
  await client.command('select-thread', 't2', '', 0, native, disk); await client.refresh(native, disk); await syncCommandOutputs(client, native);
  expect(native.calls.some(call => call.op === 'unsubscribe' && call.key === key)).toBe(true);
  native.emit(key, { kind: 'replace', text: 'wrong thread', truncated: false, running: true }); await client.refresh(native, disk);
  expect(commandOutputView(client, row, item)?.value).toBeNull();
});
test('older servers retain fetched final output without opening an unsupported stream', async () => {
  const { native, client, item, row } = await connected();
  obj(client.config.environment).capabilities = {} satisfies Obj;
  setTurnItemOpen(client, row, true); await syncCommandOutputs(client, native);
  expect(subscriptions(native)).toHaveLength(0); expect(commandOutputView(client, row, item)).toBeNull();
});
