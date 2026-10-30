// The thread details card's Servers section (ThreadServersPanel.tsx and portDiscoveryState.ts useThreadDiscoveredPorts
// at 6d450a4284): the servers this thread's terminals and agent started, their source line, when Stop is offered, a ⌘-press
// to the system browser, and Stop through preview.stopLocalServer with the reference's failure toast.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { ClientError } from '../../src/client/protocol';
import { DETAILS_DISCOVERED_SERVERS_KEY, discoveredServersEvent } from '../../src/browser/browser-targets';
import { threadServerLocal, threadServersView, watchThreadServers } from '../../src/details/thread-servers';
import { toasts } from '../../src/shell/toast';
import { externalOpensForTests, resetExternalOpensForTests } from '../../src/client/external-open';

const native = {} as Native;
function fixture(options: { stop?: boolean; scopes?: string[]; stopError?: string } = {}) {
  const calls: Record<string, unknown>[] = [], stops: { pid: number; port: number }[] = [];
  const client = {
    generation: 1, connection: 'connected', ready: true, environmentId: 'env', threadId: 't1', origin: 'http://127.0.0.1:16720',
    scopes: options.scopes ?? ['preview:operate', 'terminal:operate'], config: { environment: { capabilities: { localServerStop: options.stop ?? true } } },
    restAccess: () => ({ call: async (request: Record<string, unknown>) => { calls.push(request); return request.op === 'subscribe' ? { id: 'sub-1' } : {}; } }),
    stopLocalServer: async (_native: Native, input: { pid: number; port: number }) => { stops.push(input); if (options.stopError) throw new ClientError(options.stopError); },
  } as unknown as T3Client;
  const servers = (list: object[]) => discoveredServersEvent(client, { key: DETAILS_DISCOVERED_SERVERS_KEY, subscriptionId: 'sub-1', value: { servers: list, scannedAt: 'now' } });
  return { client, calls, stops, servers };
}
const server = (port: number, extra: object) => ({ host: 'localhost', port, url: `http://localhost:${port}/`, processName: 'bun', pid: port, terminal: null, ...extra });

describe('the servers this thread started (useThreadDiscoveredPorts)', () => {
  test('its terminal\'s and its agent\'s, each with its source; another thread\'s are left out', async () => {
    const f = fixture();
    await watchThreadServers(f.client, native, true);
    expect(f.calls).toEqual([{ op: 'subscribe', key: DETAILS_DISCOVERED_SERVERS_KEY, method: 'subscribeDiscoveredLocalServers', payload: {} }]);
    f.servers([server(5173, { agent: { threadId: 't1' } }), server(3000, { terminal: { threadId: 't1', terminalId: 'term' }, processName: null }), server(4000, { agent: { threadId: 't2' } })]);
    const view = threadServersView(f.client, 't1');
    expect(view.rows.map(row => [row.label, row.source, row.stoppable])).toEqual([['localhost:5173', 'Agent · bun', true], ['localhost:3000', 'Terminal', true]]);
    expect(threadServersView(f.client, 't3').show).toBe(false);
    await watchThreadServers(f.client, native, false);
    expect(f.calls.at(-1)).toEqual({ op: 'unsubscribe', key: DETAILS_DISCOVERED_SERVERS_KEY });
  });
  test('Stop needs the environment\'s localServerStop, terminal:operate and a process id', async () => {
    for (const [options, pid, stoppable] of [[{ stop: false }, 1, false], [{ scopes: ['preview:operate'] }, 1, false], [{}, null, false], [{}, 1, true]] as const) {
      const f = fixture(options);
      await watchThreadServers(f.client, native, true);
      f.servers([server(5173, { pid, agent: { threadId: 't1' } })]);
      expect(threadServersView(f.client, 't1').rows[0]!.stoppable).toBe(stoppable);
    }
  });
});

describe('a row\'s presses (ThreadServersPanel open and stop)', () => {
  test('⌘-press opens the server in the system browser', async () => {
    resetExternalOpensForTests();
    const f = fixture();
    await watchThreadServers(f.client, native, true);
    f.servers([server(5173, { agent: { threadId: 't1' } })]);
    await threadServerLocal(f.client, native, 'external', '5173:5173');
    expect(externalOpensForTests()).toEqual(['http://localhost:5173/']);
  });
  test('Stop sends the process and port; a refusal is the "Could not stop server" toast with the reference\'s sentence', async () => {
    const ok = fixture();
    await watchThreadServers(ok.client, native, true);
    ok.servers([server(5173, { agent: { threadId: 't1' } })]);
    await threadServerLocal(ok.client, native, 'stop', '5173:5173');
    expect(ok.stops).toEqual([{ pid: 5173, port: 5173 }]);
    const failing = fixture({ stopError: 'This server is no longer running.' });
    await watchThreadServers(failing.client, native, true);
    failing.servers([server(5173, { agent: { threadId: 't1' } })]);
    await threadServerLocal(failing.client, native, 'stop', '5173:5173');
    expect(toasts(failing.client).map(toast => [toast.title, toast.description])).toEqual([['Could not stop server', 'This server is no longer running.']]);
    expect(threadServersView(failing.client, 't1').busy).toBe(false);
  });
});
