import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { Backend, storage } from '../support/client-fixture';
import { obj, str, type Obj } from '../../src/client/domain';
import { commandOutputView, syncCommandOutputs } from '../../src/timeline/command-output';
import { setTurnItemOpen } from '../../src/timeline/timeline-item-fetch';
import { prepareTimeline } from '../../src/timeline/timeline-prepare';
import { letGoAware } from '../../src/client/let-go';
import { noteNow } from '../../src/composer/composer-controls';

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
test('a cancelled timeline preparation leaves no stream error or cooldown for the next answer', async () => {
  const { native, client, disk, item, row } = await connected();
  setTurnItemOpen(client, row, true); noteNow(client, 1000);
  const later = native.later.bind(native); let attempts = 0;
  native.later = async request => {
    const call = obj(request);
    if (call.op === 'subscribe' && call.method === 'orchestration.subscribeCommandOutput' && ++attempts === 1) {
      throw Object.assign(new Error('the answer was let go before this reply; the request may already have been sent'), { name: 'FetchError', kind: 'Aborted' });
    }
    return later(request);
  };
  await expect(prepareTimeline(client, letGoAware(native))).rejects.toMatchObject({ kind: 'superseded' });
  expect(commandOutputView(client, row, item)).toEqual({ value: null, error: '' });
  await prepareTimeline(client, letGoAware(native));
  expect(attempts).toBe(2); expect(subscriptions(native)).toHaveLength(1);
  const key = str(subscriptions(native)[0]!.key);
  native.emit(key, { kind: 'replace', text: 'Running parser checks\nProgress 50%\n', truncated: false, running: true });
  await client.refresh(native, disk);
  expect(commandOutputView(client, row, item)).toMatchObject({ error: '', value: { output: { text: 'Running parser checks\nProgress 50%\n' } } });
});
test('a real subscription failure keeps its error and cooldown until a later successful stream', async () => {
  const { native, client, disk, item, row } = await connected();
  setTurnItemOpen(client, row, true); noteNow(client, 1000);
  const later = native.later.bind(native); let attempts = 0;
  native.later = async request => {
    const call = obj(request);
    if (call.op === 'subscribe' && call.method === 'orchestration.subscribeCommandOutput' && ++attempts === 1) {
      throw Object.assign(new Error('The network connection was lost.'), { name: 'FetchError', kind: 'Network' });
    }
    return later(request);
  };
  await prepareTimeline(client, letGoAware(native));
  expect(commandOutputView(client, row, item)?.error).toBe("Couldn't load output: Reconnect to try again.");
  noteNow(client, 1999); await prepareTimeline(client, letGoAware(native)); expect(attempts).toBe(1);
  noteNow(client, 2000); await prepareTimeline(client, letGoAware(native)); expect(attempts).toBe(2);
  const key = str(subscriptions(native)[0]!.key);
  native.emit(key, { kind: 'replace', text: 'Recovered output', truncated: false, running: false }); await client.refresh(native, disk);
  expect(commandOutputView(client, row, item)).toMatchObject({ error: '', value: { running: false, output: { text: 'Recovered output' } } });
});
