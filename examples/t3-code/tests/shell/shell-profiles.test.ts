// shell.ts's answer reports this client's browser profiles to a server browser it can operate (BrowserProfileReporter).
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { shellView } from '../../src/shell/shell';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';

const T0 = Date.UTC(2026, 9, 5, 14, 0, 0);
function setup() {
  const client = new T3Client(), rpcs: string[] = [];
  Object.assign(client, { environmentId: 'env', threadId: 't1', generation: 1, connection: 'connected', configLive: true, shellLive: true, threadLive: true,
    loaded: true, scopes: ['preview:operate'], config: { environment: { capabilities: { serverBrowser: true } } } });
  client.rpc = (async (_native: Native, method: string) => { rpcs.push(method); return {}; }) as T3Client['rpc'];
  const files = { fs: { async mkdir() {}, async readFile(): Promise<ArrayBuffer> { throw new Error('missing'); }, async atomicWriteFile() {} } };
  const native = { available: true, watch() {}, later: async (input: unknown) => {
    const request = input as Obj;
    if (request.op === 'notifyStatus') return { ok: true, generation: 1, value: { active: true, authorization: 'unknown', agent: false, opened: '', openedThread: '' } };
    return { ok: true, generation: 1, value: {} };
  } } as unknown as Native;
  const view = (now: number) => shellView(client, native, files as never, now, '', false);
  return { rpcs, view };
}

describe('the shell answer', () => {
  test('reports the browser profiles once to the server browser it operates', async () => {
    const { rpcs, view } = setup();
    await view(T0);
    await view(T0 + 1_000);
    expect(rpcs.filter(method => method === 'preview.reportProfiles')).toHaveLength(1);
  });
});
