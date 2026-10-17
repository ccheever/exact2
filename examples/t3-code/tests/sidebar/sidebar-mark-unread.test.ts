// Mark unread is fire and forget everywhere (useThreadActions.ts useMarkThreadUnread: useAtomCommand(threadEnvironment.markUnread,
// { reportFailure: false })), so the server's refusal of a thread with no completed run shows no toast and no banner, from
// the flat row menu, the multi-selection menu, the legacy menus, the thread's More actions menu and a background environment's row.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { obj, initialShell, type Obj } from '../../src/client/domain';
import { ClientError, type Files, type Native } from '../../src/client/protocol';
import { toasts } from '../../src/shell/toast';
import { sidebarCommand } from '../../src/sidebar/sidebar-commands';
import { sidebarPrefs, sidebarSession, setRuntimeClock } from '../../src/sidebar/sidebar-state';
import { shellCommand } from '../../src/shell/shell-commands';
import { fleet } from '../../src/connections/settings-b-fleet';

beforeEach(() => setRuntimeClock(() => Date.now()));
afterEach(() => { setRuntimeClock(() => Number.NaN); fleet.entries.clear(); });

const NOW = Date.now();
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const REFUSAL = 'Thread verify-timeline has no completed run to mark unread.';
const CAPS = { threadSettlement: true, threadSnooze: true, threadPinning: true, threadVisitedTracking: true };
const row = (id: string, projectId = 'p1', extra: Obj = {}): Obj => ({ id, projectId, title: `Thread ${id}`, status: 'idle', latestRunId: null,
  createdAt: iso(-60_000), updatedAt: iso(-60_000), archivedAt: null, settledOverride: null, settledAt: null,
  modelSelection: { instanceId: 'codex', model: 'm' }, lineage: { relationshipToParent: null }, ...extra });
const KEY = 'https://second.example\nenv-b';

/** A client on env-a whose server, and env-b's, refuse thread.mark-unread as a server does for a thread with no completed run. */
function refusing(threads: Obj[], options: { caps?: Obj; picks?: string[]; settings?: Obj } = {}) {
  const sent: string[] = [], picks = [...(options.picks ?? [])];
  const client = {
    shell: { projects: [{ id: 'p1', title: 'shared-notes', workspaceRoot: '/a' }], threads, sequence: 1 },
    config: { environment: { capabilities: options.caps ?? CAPS }, providers: [], keybindings: [] },
    environmentId: 'env-a', threadId: '', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, presentation: {}, error: '',
    local: { drafts: {}, selections: {}, snapshotDrafts: {}, snapshotReleases: [], composerControls: { contexts: {} }, deviceSettings: { timestampFormat: '24-hour' },
      clientSettings: { confirmThreadArchive: false, confirmThreadDelete: true, confirmThreadUnpin: false, sidebarWorkingShelfEnabled: false, ...(options.settings ?? {}) }, sidebarWidth: 256 },
    projectGroups() { return [{ key: 'g1', name: 'shared-notes', members: [{ id: 'p1' }] }]; },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `c${index}`),
      request: async (method: string, payload: Obj) => {
        if (method === 'orchestration.dispatchCommand') { sent.push(`env-a:${String(payload.threadId)}`); throw new ClientError(REFUSAL, 'server'); }
        return {};
      },
      dispatch: async () => { throw new Error('Mark unread must not go through the write journal'); },
      call: async (request: Obj) => request.op === 'sidebarMenu' ? { id: picks.shift() ?? null } : {},
    }),
  } as unknown as T3Client & { error: string };
  const native = { available: true, watch() {}, later: async (request: unknown) => {
    const message = obj(request), payload = obj(message.payload);
    if (message.fleet === KEY && message.method === 'orchestration.dispatchCommand') {
      sent.push(`env-b:${String(payload.threadId)}`);
      return { ok: false, generation: 2, error: { message: REFUSAL, kind: 'server' } };
    }
    return { ok: true, generation: Number(message.generation) || 2, value: {} };
  } } as unknown as Native;
  return { client, native, sent };
}
const files = {} as Files;

describe('Mark unread refused by the server', () => {
  test('the row menu, the multi-selection menu and the thread menu stay silent', async () => {
    const { client, native, sent } = refusing([row('a'), row('b')], { picks: ['mark-unread'] });
    await sidebarCommand(client, native, files, 'menu-choice', 'a', 'mark-unread');
    sidebarSession(client).selection = ['a', 'b'];
    await sidebarCommand(client, native, files, 'menu', 'a', 'row');
    expect(await shellCommand(client, native, files, 'mark-unread', 'b', '')).toBe('');
    expect(sent).toEqual(['env-a:a', 'env-a:a', 'env-a:b', 'env-a:b']);
    expect(sidebarSession(client).selection).toEqual([]);
    expect(toasts(client)).toEqual([]);
    expect(client.error).toBe('');
  });

  test('the legacy menus stay silent', async () => {
    const { client, native, sent } = refusing([row('a'), row('b')], { picks: ['mark-unread', 'mark-unread'] });
    await sidebarCommand(client, native, files, 'legacy-thread-menu', 'a', '');
    sidebarSession(client).selection = ['a', 'b'];
    await sidebarCommand(client, native, files, 'legacy-thread-menu', 'a', '');
    expect(sent).toEqual(['env-a:a', 'env-a:a', 'env-a:b']);
    expect(toasts(client)).toEqual([]);
  });

  test('a background environment\'s row stays silent', async () => {
    fleet.entries.set(KEY, { key: KEY, origin: 'https://second.example', environmentId: 'env-b', phase: 'connected', message: '', traceId: '',
      generation: 2, synchronized: 2, lastEvent: 0, subscriptions: {}, config: { environment: { environmentId: 'env-b', label: 'Lane Second', capabilities: CAPS } },
      shell: { ...initialShell(), projects: [{ id: 'pb', title: 'background-only', workspaceRoot: '/b' }], threads: [row('tb', 'pb')] }, scopes: [], error: '', requested: true });
    const { client, native, sent } = refusing([row('a')]);
    await sidebarCommand(client, native, files, 'menu-choice', 'fleet:env-b:tb', 'mark-unread');
    expect(sent).toEqual(['env-b:tb']);
    expect(toasts(client)).toEqual([]);
  });

  test('without visited tracking, the thread menu marks this device\'s watermark as the sidebar does', async () => {
    const completedAt = iso(-30_000);
    const { client, native, sent } = refusing([row('a', 'p1', { latestRunId: 'r1', latestRunCompletedAt: completedAt }), row('b')], { caps: { threadPinning: true } });
    await shellCommand(client, native, files, 'mark-unread', 'a', '');
    await shellCommand(client, native, files, 'mark-unread', 'b', ''); // no completed run: nothing to rewind
    expect(sent).toEqual([]);
    expect(sidebarPrefs(client).visited).toEqual({ a: new Date(Date.parse(completedAt) - 1).toISOString() });
  });
});
