// T3 Code 1e2ecbd975: the Pull Requests list keeps its rows and its place when the app
// becomes active again and across a live Light/Dark switch; Link to thread opens its own picker
// dialog (PullRequestThreadLinks display="picker"); the checkout dialog's field selects its text on open in
// Contract (PullRequestThreadDialog's `focus(); select()`). The behavior itself is checked by driving the
// app on macOS.
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { ClientError, type Native } from '../../src/client/protocol';
import { fleet, EnvironmentFleet } from '../../src/connections/settings-b-fleet';
import { prLocal, pullRequestsPage, type PrInput } from '../../src/pull-requests/pages-prs';
import { environmentQueries } from '../../src/pull-requests/pages-pr-paging';
import { threadPickerView } from '../../src/pull-requests/pages-pr-links';
import { contractComponent, value } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';


const source = async (file: string) => appText(file);

// ── A return to the app keeps the list ──────────────────────────────────────

const NOW = Date.parse('2026-10-14T03:00:00Z');
const GH = 'env-gh', ORIGIN = 'http://127.0.0.1:16543';
const KEY = `${ORIGIN}\n${GH}`;
const rows = Array.from({ length: 30 }, (_, index) => ({ projectId: 'p-gh', host: 'github.com', repository: 'owner/playground', number: 144 - index,
  title: `Change ${144 - index}`, state: 'open', isDraft: false, author: { login: 'owner' }, updatedAt: '2026-10-13T11:00:00Z', additions: 1, deletions: 0 }));

/**
 * The transport as the fleet sees one background environment (the GitHub lane added from Settings › Connections): its
 * status, the bootstrap, and the inbox. `foreground()` is the window's return (transportSignal "foreground"): a `_retryDue` on the shell
 * stream, as the transport appends when the app becomes active (tests/transport/transport-session.test.ts).
 */
function backgroundTransport() {
  let serial = 0, seq = 0, generation = 1;
  let subscriptionFailure: Error | undefined;
  let supersedeAt: 'subscribe' | 'ack' | undefined;
  const inbox: Obj[] = [], subscribes: Obj[] = [];
  const subscriptions: Record<string, string> = {};
  const answer = (value: unknown) => ({ ok: true, generation, value });
  const native = {
    available: true, watch() {},
    later: async (request: Obj) => {
      if (request.op === 'environments') return answer({ saved: [{ origin: ORIGIN, environmentId: GH, label: 'GitHub lane', enabled: true }] });
      if (request.fleet !== KEY) return answer({});
      switch (request.op) {
        case 'status': return answer({ state: 'connected', environmentId: GH });
        case 'http': return answer(request.path === '/api/orchestration/shell'
          ? { snapshotSequence: 5, projects: [{ id: 'p-gh', title: 'playground', workspaceRoot: '/lane/playground', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/owner/playground', displayName: 'owner/playground' } }], threads: [] }
          : { scopes: ['orchestration:read'] });
        case 'request': return answer({ environment: { label: 'GitHub lane', capabilities: { pullRequests: true } } });
        case 'subscribe': {
          if (subscriptionFailure) { const error = subscriptionFailure; subscriptionFailure = undefined; throw error; }
          const id = `${generation}-${++serial}`;
          subscribes.push({ key: request.key, payload: request.payload, id });
          subscriptions[String(request.key)] = id;
          if (supersedeAt === 'subscribe') { supersedeAt = undefined; throw new ClientError('The answer was let go.', 'superseded'); }
          return answer({ id });
        }
        case 'events': return answer({ events: inbox.filter(event => Number(event.seq) > Number(request.after)), latest: seq });
        case 'ack': {
          for (let index = inbox.length - 1; index >= 0; index--) if (Number(inbox[index]!.seq) <= Number(request.through)) inbox.splice(index, 1);
          if (supersedeAt === 'ack') { supersedeAt = undefined; throw new ClientError('The answer was let go.', 'superseded'); }
          return answer({ latest: seq });
        }
        default: return answer({});
      }
    },
  } as unknown as Native;
  const emit = (key: string, value: Obj) => inbox.push({ seq: ++seq, generation, key, subscriptionId: subscriptions[key], value });
  const foreground = () => emit('shell', { _retryDue: true });
  return { native, subscribes, foreground, emit, subscriptions,
    failSubscription: (error: Error) => { subscriptionFailure = error; },
    supersedeAfter: (op: 'subscribe' | 'ack') => { supersedeAt = op; },
    reconnect: () => { generation++; seq = 0; inbox.length = 0; } };
}

/** The window's focused connection (This machine: one project, no pull requests) and the page's reads on every server. */
function focusedClient() {
  const listed: string[] = [];
  const client = {
    environmentId: 'env', origin: 'http://127.0.0.1:16542', connection: 'connected', statusMessage: '', scopes: [], ready: true, generation: 1, revision: 0, local: {} as Obj,
    config: { environment: { label: 'This machine', capabilities: { pullRequests: true } } },
    shell: { projects: [{ id: 'p1', title: 'work', repositoryIdentity: null }], threads: [], sequence: 0 },
    rpc: async (_native: unknown, method: string, payload: Obj) => {
      if (method === 'pullRequests.list') { listed.push(String(payload.environmentId ?? 'env')); return payload.environmentId === GH ? { entries: rows, viewers: {} } : { entries: [], viewers: {} }; }
      if (method === 'pullRequests.listStats') return { stats: [] };
      return {};
    },
    restAccess: () => ({ call: async () => ({ id: '1-1' }) }),
  } as unknown as T3Client & { ready: boolean };
  client.projectGroups = () => client.shell.projects.map(project => ({ key: String(project.id), name: String(project.title ?? ''), members: [project] }));
  const native = { available: true, watch: () => {}, later: async () => ({ ok: true, generation: 1, value: {} }) } as unknown as Native;
  const page = (input: Partial<PrInput> & { returns?: number } = {}) => pullRequestsPage(client, native, { open: true, refresh: 0, now: NOW, selected: '', query: '', typed: false, ...input });
  return { client, page, listed };
}
const numbers = (view: Awaited<ReturnType<typeof pullRequestsPage>>) => view.groups.flatMap(group => group.rows.map(row => row.number));

afterEach(() => { fleet.entries.clear(); fleet.saved = []; });

describe('The Pull Requests list keeps its rows when the app becomes active again', () => {
  test('a background environment\'s shell resubscribes from its cursor on `_retryDue`, staying synchronized with its projects', async () => {
    const source = new EnvironmentFleet(), lane = backgroundTransport();
    const focused = { origin: 'http://127.0.0.1:16542', environmentId: 'env', connection: 'connected' };
    await source.sync(lane.native, focused);
    const entry = source.entries.get(KEY)!;
    expect([entry.phase, entry.synchronized, entry.generation, entry.shell.projects.length]).toEqual(['connected', 1, 1, 1]);
    const firstShell = entry.subscriptions.shell;
    lane.foreground();
    await source.sync(lane.native, focused);
    // Before: the marker went to applyShell as a snapshot with no sequence, which threw (synchronized -1), and the
    // environment left every surface until a fresh bootstrap.
    expect([entry.synchronized, entry.generation, entry.shell.projects.map(project => project.id)]).toEqual([1, 1, ['p-gh']]);
    const again = lane.subscribes.filter(call => call.key === 'shell');
    expect(again.map(call => call.payload)).toEqual([{ afterSequence: 5 }, { afterSequence: 5 }]);
    expect(entry.subscriptions.shell).not.toBe(firstShell);
    expect(lane.subscribes.filter(call => call.key === 'config')).toHaveLength(1); // the config stream had nothing due
  });

  test('a superseded foreground resubscription preserves the snapshot; an actual failure asks for a bootstrap', async () => {
    for (const cancelled of [true, false]) {
      const source = new EnvironmentFleet(), lane = backgroundTransport();
      const focused = { origin: 'http://127.0.0.1:16542', environmentId: 'env', connection: 'connected' };
      await source.sync(lane.native, focused);
      const entry = source.entries.get(KEY)!, shell = entry.shell;
      lane.foreground();
      const error = cancelled ? new ClientError('The answer was let go.', 'superseded') : new Error('Subscribe refused.');
      lane.failSubscription(error);
      if (cancelled) await expect(source.sync(lane.native, focused)).rejects.toThrow(error.message);
      else await source.sync(lane.native, focused);
      expect(entry.shell).toBe(shell);
      expect(entry.synchronized).toBe(cancelled ? entry.generation : -1);
      expect(entry.busy).toBe(false);
    }
  });

  test.each(['subscribe', 'ack'] as const)('a retry survives a superseded native %s and resumes live shell events', async op => {
    const source = new EnvironmentFleet(), lane = backgroundTransport();
    const focused = { origin: 'http://127.0.0.1:16542', environmentId: 'env', connection: 'connected' };
    await source.sync(lane.native, focused);
    const entry = source.entries.get(KEY)!, shell = entry.shell;
    lane.foreground();
    lane.supersedeAfter(op);
    await expect(source.sync(lane.native, focused)).rejects.toThrow('The answer was let go.');
    expect(entry.shell).toBe(shell);
    expect(entry.synchronized).toBe(entry.generation);
    // Native already replaced the subscription or removed the retry marker from its inbox.
    // A later answer must retry anyway and adopt the current native subscription id.
    await source.sync(lane.native, focused);
    expect(lane.subscribes.filter(call => call.key === 'shell')).toHaveLength(op === 'subscribe' ? 3 : 2);
    expect(entry.subscriptions.shell).toBe(lane.subscriptions.shell);
    lane.emit('shell', { kind: 'project.updated', sequence: 6, project: { ...entry.shell.projects[0], title: 'Renamed after recovery' } });
    await source.sync(lane.native, focused);
    expect(entry.shell.sequence).toBe(6);
    expect(entry.shell.projects[0]!.title).toBe('Renamed after recovery');
    lane.foreground();
    await source.sync(lane.native, focused);
    expect(lane.subscribes.at(-1)!.payload).toEqual({ afterSequence: 6 });
  });

  test('a new generation replaces pending foreground retries with its bootstrap subscriptions', async () => {
    const source = new EnvironmentFleet(), lane = backgroundTransport();
    const focused = { origin: 'http://127.0.0.1:16542', environmentId: 'env', connection: 'connected' };
    await source.sync(lane.native, focused);
    lane.emit('config', { _retryDue: true });
    lane.foreground();
    lane.supersedeAfter('ack');
    await expect(source.sync(lane.native, focused)).rejects.toThrow('The answer was let go.');
    lane.reconnect();
    await source.sync(lane.native, focused);
    const entry = source.entries.get(KEY)!;
    expect([entry.generation, entry.synchronized]).toEqual([2, 2]);
    expect(lane.subscribes.map(call => call.key).filter(key => key === 'config' || key === 'shell')).toEqual(['config', 'shell', 'config', 'shell']);
    expect(entry.subscriptions).toMatchObject({ config: lane.subscriptions.config, shell: lane.subscriptions.shell });
  });

  test('the page reads the same servers after the return, and its rows never empty', async () => {
    const lane = backgroundTransport();
    const { client, page, listed } = focusedClient();
    await fleet.sync(lane.native, client as unknown as { origin: string; environmentId: string; connection: string });
    const before = await page();
    expect(environmentQueries(client, '').map(query => query.environmentId)).toEqual(['env', GH]);
    expect(numbers(before).slice(0, 3)).toEqual([144, 143, 142]);
    expect(before.empty).toBe('');
    // The app becomes active: the transport marks the background shell due, the fleet drains it, the window's `focus`
    // asks the list again (app.contract windowReturned, 30 s after the last read).
    lane.foreground();
    await fleet.sync(lane.native, client as unknown as { origin: string; environmentId: string; connection: string });
    expect(environmentQueries(client, '').map(query => query.environmentId)).toEqual(['env', GH]);
    const after = await page({ now: NOW + 30_000, returns: 1 });
    expect(numbers(after)).toEqual(numbers(before));
    expect(after.empty).toBe(''); // not "No pull requests"
    expect(listed.filter(id => id === GH)).toHaveLength(2); // the return read the list again, on both servers
  });

  test('while the focused connection resynchronizes (its shell waits for the completion marker), the page keeps the rows it drew', async () => {
    const { client, page } = focusedClient();
    client.rpc = (async (_native: unknown, method: string) => method === 'pullRequests.list' ? { entries: rows.map(row => ({ ...row, projectId: 'p1' })), viewers: {} } : { stats: [] }) as T3Client['rpc'];
    const drawn = await page();
    expect(numbers(drawn)).toHaveLength(30);
    client.ready = false; // T3Client.retryStreams: shellLive is false until the resubscribed shell says `synchronized`
    const resyncing = await page({ now: NOW + 30_000, returns: 1 });
    expect(numbers(resyncing)).toEqual(numbers(drawn));
    expect([resyncing.loading, resyncing.empty]).toEqual([false, '']);
    client.ready = true;
    expect(numbers(await page({ now: NOW + 31_000, returns: 1 }))).toEqual(numbers(drawn));
    // Another server (a different focus) never shows these rows.
    (client as { environmentId: string }).environmentId = 'other';
    client.ready = false;
    expect(numbers(await page({ now: NOW + 32_000 }))).toEqual([]);
  });
});

// ── Link to thread's own dialog ─────────────────────────────────────────────

describe('"Link to thread" opens its own picker dialog (PullRequestThreadLinks display="picker")', () => {
  test('the palette page `pr-link-thread` is the picker panel, its rows and texts the reference ThreadPicker\'s', () => {
    const view = threadPickerView({ shell: { threads: [], projects: [] } } as unknown as T3Client, '', false);
    expect(view).toMatchObject({ page: 'pr-link-thread', panel: 'picker', label: 'Choose a thread', placeholder: 'Search threads or projects...', testId: 'pull-request-thread-picker', autoHighlight: true });
  });

  test('the picker keeps its first enabled row highlighted on open and after a query edit', async () => {
    const expression = (await source('app.contract')).split('  derive paletteHighlight = ')[1]!.split('\n')[0]!;
    const highlight = (panel: string, count: number, cursor: number) => value(expression,
      { palette: { panel, count, autoHighlight: true }, paletteCursor: cursor, min: Math.min });
    expect(highlight('picker', 2, -2)).toBe(0);
    expect(highlight('picker', 2, 1)).toBe(1);
    expect(highlight('picker', 0, -2)).toBe(-1);
    expect(highlight('list', 2, -2)).toBe(-1); // Preserve the command palette's explicit no-highlight state.
  });
});

// ── Select on open in Contract ──────────────────────────────────────────────

describe('The checkout dialog\'s field focuses and selects its reference as it opens, in Contract', () => {
  test('FieldOpenSelect focuses then selects once per opening, preserving later caret changes', () => {
    const field = contractComponent('dialog-trap.contract', 'FieldOpenSelect');
    const state = field.state(), calls: unknown[][] = [];
    const scope = { target: 'reference', length: '한글🙂'.length,
      focus: (id: string) => calls.push(['focus', id]),
      setSelectionRange: (id: string, start: number, end: number) => calls.push(['select', id, start, end]) };
    field.call('laidOut', state, scope, [0, 0]);
    expect(calls).toEqual([]);
    field.call('laidOut', state, scope, [1, 1]);
    expect(calls).toEqual([['focus', 'reference'], ['select', 'reference', 0, 4]]);
    calls.length = 0;
    field.call('laidOut', state, scope, [1, 1]);
    expect(calls).toEqual([]); // The field's caret is now owned by its editor.
    field.call('laidOut', state, scope, [0, 0]);
    field.call('laidOut', state, scope, [1, 1]);
    expect(calls).toEqual([['focus', 'reference'], ['select', 'reference', 0, 4]]);
  });
});

// ── The provider menu after a failed list, and the Server filter ───────────

describe('The Pull Requests filters name the hosts and servers the page knows', () => {
  test('an empty server offers Add project without a list request and keeps the known provider options', async () => {
    const lane = backgroundTransport();
    const { client, page, listed } = focusedClient();
    client.shell.projects = [];
    await fleet.sync(lane.native, client as unknown as { origin: string; environmentId: string; connection: string });
    const all = await page();
    expect(numbers(all).slice(0, 2)).toEqual([144, 143]);
    for (let pass = 0; pass < 2; pass++) {
      prLocal(client, 'server', 'env'); listed.length = 0;
      const empty = await page({ refresh: pass * 2 + 1 });
      expect([empty.hasProjects, empty.empty, empty.emptyAction, empty.error]).toEqual([false, 'No projects in this workspace', 'add-project', '']);
      expect(empty.hosts.map(host => host.label)).toEqual(['All', 'GitHub']);
      expect(listed).toEqual([]);
      prLocal(client, 'server', '');
      expect(numbers(await page({ refresh: pass * 2 + 2 })).slice(0, 2)).toEqual([144, 143]);
    }
  });
  test('with the list failed, the provider menu still has All and each host the projects expect', async () => {
    const lane = backgroundTransport();
    const { client, page } = focusedClient();
    await fleet.sync(lane.native, client as unknown as { origin: string; environmentId: string; connection: string });
    client.rpc = (async (_native: unknown, method: string) => {
      if (method === 'pullRequests.list') throw new ClientError('Pull request operation list failed: Could not reach GitHub at github.com.');
      return {};
    }) as unknown as T3Client['rpc'];
    const view = await page();
    expect([view.empty, view.emptyDetail]).toEqual(['Could not load pull requests', 'Pull request operation list failed: Could not reach GitHub at github.com.']);
    expect(view.hosts.map(host => [host.value, host.label, host.selected])).toEqual([['', 'All', true], ['github.com', 'GitHub', false]]);
  });
  test('two servers that read pull requests give Filters a Server row; choosing one reads only that server', async () => {
    const lane = backgroundTransport();
    const { client, page, listed } = focusedClient();
    await fleet.sync(lane.native, client as unknown as { origin: string; environmentId: string; connection: string });
    const all = await page();
    expect(all.servers.map(server => [server.value, server.label, server.selected])).toEqual([['', 'All servers', true], ['env', 'This machine', false], [GH, 'GitHub lane', false]]);
    expect([all.serverLabel, all.filterBadge]).toEqual(['All servers', '']);
    prLocal(client, 'project', 'p1');
    prLocal(client, 'server', GH);
    expect(environmentQueries(client, '', GH).map(query => query.environmentId)).toEqual([GH]);
    listed.length = 0;
    const scoped = await page({ refresh: 1 });
    expect(listed).toEqual([GH]);
    expect([scoped.server, scoped.serverLabel, scoped.filterBadge, scoped.projectId]).toEqual([GH, 'GitHub lane', '1', '']);
    expect(numbers(scoped).slice(0, 2)).toEqual([144, 143]);
    // A project brings its own server back (the focused one's projects are the ones listed).
    prLocal(client, 'project', 'p1');
    expect((await page({ refresh: 2 })).server).toBe('env');
    // A chosen server that is not connected reads nothing, not every other server (usePullRequestScope).
    prLocal(client, 'server', GH);
    for (const entry of fleet.entries.values()) entry.phase = 'connecting';
    expect(environmentQueries(client, '', GH)).toEqual([]);
  });
  test('one server shows no Server row (serverOptions.length > 2)', async () => {
    const body = await source('src/pull-requests/pages-prs.contract');
    expect(body).toContain('derive serverShown = page.servers.length > 2');
    expect(body).toMatch(/if serverShown\n[^\n]+role="separator"\n[^\n]+testId="pr-filter-server"/);
    const { page } = focusedClient();
    expect((await page()).servers.map(server => server.label)).toEqual(['All servers', 'This machine']);
  });
});
