// WebSocket tickets name the route they open (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3: packages/client-runtime/
// src/authorization/remote.ts issueWebSocketTicketWithFallback, state/deviceHubAccess.ts): an RPC socket's ticket
// mints at `/api/auth/websocket-ticket/rpc`, and at the bare path on a server that predates ticket routes (it answers
// 404); a media ticket (the device hub, the Browser's preview stream) mints at `/media` only.
import { describe, expect, test } from 'bun:test';
import { Drive, FakeServer, pairing } from '../support/transport-fixture';

const ORIGIN = 'http://127.0.0.1:4001';
type Obj = Record<string, unknown>;
const tickets = (server: FakeServer) => server.requests.filter(request => request.path.startsWith('/api/auth/websocket-ticket'))
  .map(request => `${request.method} ${request.path}`);

async function paired(ticketRoutes: boolean) {
  const server = new FakeServer(ORIGIN);
  server.ticketRoutes = ticketRoutes;
  const drive = new Drive(server);
  expect((await drive.op(pairing())).ok).toBe(true);
  return { drive, server };
}

describe('a 0.1.4 server', () => {
  test('the RPC socket and the terminal page mint at /rpc, the device hub and the preview stream at /media', async () => {
    const { drive, server } = await paired(true);
    expect(tickets(server)).toEqual(['POST /api/auth/websocket-ticket/rpc']);
    expect((await drive.op({ op: 'deviceHubAccess' })).ok).toBe(true);
    expect((await drive.op({ op: 'previewStreamAccess' })).ok).toBe(true);
    expect(tickets(server)).toEqual(['POST /api/auth/websocket-ticket/rpc', 'POST /api/auth/websocket-ticket/media', 'POST /api/auth/websocket-ticket/media']);
    const terminal = await drive.transport.focused.terminalSocketAccess();
    expect(terminal.ok && new URL(String((terminal.value as Obj).url)).searchParams.get('wsTicket')).toBeTruthy();
    expect(tickets(server).at(-1)).toBe('POST /api/auth/websocket-ticket/rpc');
  });
});

describe('a server before ticket routes', () => {
  test('an RPC ticket falls back to the bare path after its 404; a media ticket does not', async () => {
    const { drive, server } = await paired(false);
    expect(tickets(server)).toEqual(['POST /api/auth/websocket-ticket/rpc', 'POST /api/auth/websocket-ticket']);
    expect((await drive.transport.focused.terminalSocketAccess()).ok).toBe(true);
    expect(tickets(server).slice(-2)).toEqual(['POST /api/auth/websocket-ticket/rpc', 'POST /api/auth/websocket-ticket']);
    // As in the reference, the device hub and the preview stream do not open.
    const hub = await drive.op({ op: 'deviceHubAccess' });
    expect([hub.ok, (hub.error as Obj).kind]).toEqual([false, 'HTTP']);
    expect((await drive.op({ op: 'previewStreamAccess' })).ok).toBe(false);
    expect(tickets(server).slice(-2)).toEqual(['POST /api/auth/websocket-ticket/media', 'POST /api/auth/websocket-ticket/media']);
  });
});
