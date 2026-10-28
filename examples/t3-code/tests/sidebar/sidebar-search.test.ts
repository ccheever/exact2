// The sidebar's thread search against T3 Code 7c3b3b3605ef (MIT, LICENSE-T3): Sidebar.logic.ts searchSidebarThreads
// ranks title, branch and pull request matches as ⌘K ranks them with picks off, then lists the threads found only for
// their messages; Sidebar.tsx ranks every connected environment's message matches together (rankMessageMatches) and
// keeps shown rows in place (nextShownOrder), lets the highlight follow its thread, and opens find on a message row's
// match (threadFindStore.ts findForMessageMatch); useThreadSearch asks each environment and shows what has landed.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { obj, initialShell, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { sidebarLocal, sidebarOpened, sidebarSelecting } from '../../src/sidebar/sidebar-commands';
import { findForMessageMatch, runThreadSearch, searchMatch } from '../../src/sidebar/sidebar-search';
import { setRuntimeClock } from '../../src/sidebar/sidebar-state';
import { sidebarSnapshot } from '../../src/sidebar/sidebar-view';
import { threadFindState, threadFindTick, threadFindView } from '../../src/timeline/thread-find-runtime';

beforeEach(() => setRuntimeClock(() => Date.now()));
afterEach(() => { setRuntimeClock(() => Number.NaN); fleet.entries.clear(); });

const NOW = Date.parse('2026-10-27T12:00:00.000Z');
const ago = (ms: number) => new Date(NOW - ms).toISOString();
const DAY = 86_400_000;
const KEY = 'https://second.example\nenv-search-b';
const row = (id: string, title: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title, status: 'idle', latestRunId: null,
  createdAt: ago(40 * DAY), updatedAt: ago(40 * DAY), latestUserMessageAt: ago(40 * DAY), archivedAt: null, settledOverride: null, settledAt: null,
  modelSelection: { instanceId: 'codex', model: 'm' }, lineage: { relationshipToParent: null }, ...extra });
const match = (threadId: string, snippet: string, extra: Obj = {}): Obj => ({ threadId, projectId: 'p1', source: 'assistant', snippet, messageCreatedAt: ago(DAY), quality: 1, hits: 1, ...extra });

function background(threads: Obj[]): FleetEntry {
  return { key: KEY, origin: 'https://second.example', environmentId: 'env-search-b', phase: 'connected', message: '', traceId: '',
    generation: 2, synchronized: 2, lastEvent: 0, subscriptions: {}, config: { environment: { environmentId: 'env-search-b', label: 'Second', capabilities: {} } },
    shell: { ...initialShell(), projects: [{ id: 'p1', title: 'second', workspaceRoot: '/b' }], threads }, scopes: [], error: '', requested: true };
}

/**
 * A client focused on env-search-a. `here` answers its searches; `there` env-search-b's, which wait for `release` when
 * `gated`. `asked` holds every search sent, with its environment.
 */
function focused(threads: Obj[], here: Obj[] = [], there: Obj[] = [], gated = false) {
  const asked: Obj[] = [], findCalls: Obj[] = [];
  let release = () => {};
  const gate = gated ? new Promise<void>(resolve => { release = resolve; }) : Promise.resolve();
  const client = {
    shell: { projects: [{ id: 'p1', title: 'first', workspaceRoot: '/a' }], threads, sequence: 1 },
    config: { environment: { capabilities: {} }, providers: [], keybindings: [], threadFind: true },
    environmentId: 'env-search-a', threadId: '', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, generation: 1, presentation: {},
    thread: { sequence: 1, hasMore: false, projection: { visibleTurnItems: [] } }, projection: { visibleTurnItems: [] },
    local: { drafts: {}, selections: {}, snapshotDrafts: {}, snapshotReleases: [], deviceSettings: { timestampFormat: '24-hour' }, clientSettings: {}, sidebarWidth: 256 },
    projectGroups() { return [{ key: 'g1', name: 'first', members: [{ id: 'p1' }] }]; },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `c${index}`),
      request: async (method: string, payload: Obj) => {
        if (method === 'orchestration.searchThreads') { asked.push({ environmentId: 'env-search-a', ...payload }); return { matches: here }; }
        return {};
      },
      call: async () => ({}),
    }),
    rpc: async (_native: Native, method: string, payload: Obj) => { findCalls.push({ method, payload }); return { snapshotSequence: 1, totalMatches: 0, activeIndex: 0, match: null }; },
  } as unknown as T3Client;
  const native = { available: true, watch() {}, later: async (request: unknown) => {
    const sent = obj(request);
    if (sent.fleet === KEY && sent.method === 'orchestration.searchThreads') {
      asked.push({ environmentId: 'env-search-b', ...obj(sent.payload) });
      await gate;
      return { ok: true, generation: 2, value: { matches: there } };
    }
    return { ok: true, generation: Number(sent.generation) || 2, value: {} };
  } } as unknown as Native;
  return { client, native, asked, findCalls, release: () => release() };
}
const helpers = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2).toUpperCase(), projectInk: 'ink', projectSurface: 'surface' }),
  providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
const type = (client: T3Client, query: string) => { (client as unknown as { query: string }).query = query; };
const view = (client: T3Client) => sidebarSnapshot(client, NOW, helpers);
const ids = (client: T3Client) => view(client).threads.map(thread => thread.id);
const landing = () => { let count = 0; const landed = () => { count++; }; return { landed, count: () => count }; };
const until = async (done: () => boolean) => { for (let tries = 0; tries < 100 && !done(); tries++) await new Promise(resolve => setTimeout(resolve, 1)); };

describe('title, branch and pull request matches rank as ⌘K ranks them', () => {
  test('a whole word leads a match inside a word; among whole words, recent use then opens, and a ⌘K pick does not count', () => {
    const { client } = focused([
      row('mid', 'Researching options', { latestUserMessageAt: ago(60_000) }),
      row('old', 'Fix search box', { opens: { openedAt: ago(30 * DAY), score: 0, picks: [{ query: 'search', at: NOW - 1000 }] } }),
      row('opened', 'Ship search box', { opens: { openedAt: NOW - 60_000, score: 1, picks: [] } }),
      row('recent', 'Tune search box', { latestUserMessageAt: ago(3_600_000) }),
      row('none', 'Release notes'),
    ]);
    type(client, 'search');
    expect(ids(client)).toEqual(['recent', 'opened', 'old', 'mid']);
  });

  test('a branch matches as well as a title, and a project\'s name lists none of its threads', () => {
    const { client } = focused([row('branch', 'Untitled work', { branch: 'feature/sidebar-search' }), row('plain', 'Something else')]);
    type(client, 'sidebar');
    expect(ids(client)).toEqual(['branch']);
    type(client, 'first');
    expect(ids(client)).toEqual([]);
  });
});

describe('message matches from every environment', () => {
  const threads = () => [row('title', 'Deploy checklist'), row('mine', 'Weekly notes')];
  const theirs = [row('theirs', 'Release log')];
  const here = [match('mine', 'We deploy on Fridays', { messageId: 'm-mine' }), match('title', 'deploy steps')];
  const there = [match('theirs', 'deploy deploy deploy', { source: 'user', hits: 20, messageId: 'm-theirs' })];

  test('follow the title matches, ranked together; rows shown keep their places while a slower environment answers', async () => {
    fleet.entries.set(KEY, background(theirs));
    const slow = focused(threads(), here, there, true), landed = landing();
    type(slow.client, 'deploy');
    const search = runThreadSearch(slow.client, slow.native, 'deploy', landed.landed);
    await until(() => landed.count() === 1);
    expect(ids(slow.client)).toEqual(['title', 'mine']);
    slow.release(); await search;
    expect(ids(slow.client)).toEqual(['title', 'mine', 'fleet:env-search-b:theirs']);
    // Both at once: the thread that keeps using the word outranks a passing mention.
    const fast = focused(threads(), here, there);
    type(fast.client, 'deploy');
    await runThreadSearch(fast.client, fast.native, 'deploy');
    expect(ids(fast.client)).toEqual(['title', 'fleet:env-search-b:theirs', 'mine']);
    expect(fast.asked).toEqual([{ environmentId: 'env-search-a', query: 'deploy' }, { environmentId: 'env-search-b', query: 'deploy' }]);
  });

  test('only a row listed for its message shows it, the words of the query it was found for in semibold', async () => {
    const { client, native } = focused(threads(), here);
    type(client, 'deploy');
    await runThreadSearch(client, native, 'deploy');
    const rows = view(client).threads;
    expect(rows.find(entry => entry.id === 'title')!.matchLabel).toBe('');
    const mine = rows.find(entry => entry.id === 'mine')!;
    expect(mine.matchLabel).toBe('Agent:');
    expect(mine.matchParts.map(part => [part.text, part.hit])).toEqual([['We ', false], ['deploy', true], [' on Fridays', false]]);
    // A refined query keeps those rows until its own answer lands; they still mark only what they were found for.
    type(client, 'deploy fridays');
    void runThreadSearch(client, native, 'deploy fridays');
    const refined = view(client).threads.find(entry => entry.id === 'mine')!;
    expect(refined.matchParts.filter(part => part.hit).map(part => part.text)).toEqual(['deploy']);
  });

  test('"Searching thread messages…" while an environment has not answered and nothing is listed', async () => {
    fleet.entries.set(KEY, background(theirs));
    const { client, native, release } = focused(threads(), [], [], true);
    type(client, 'zebra');
    const search = runThreadSearch(client, native, 'zebra');
    await new Promise(resolve => setTimeout(resolve, 1));
    expect(view(client).sidebar).toMatchObject({ searching: true, searchPending: true });
    release(); await search;
    expect(view(client).sidebar).toMatchObject({ searching: true, searchPending: false });
    expect(view(client).threads).toEqual([]);
  });

  test('a query without a word of three letters asks no server; a pasted wall of text asks by its start', async () => {
    fleet.entries.set(KEY, background(theirs));
    const { client, native, asked } = focused(threads(), here, there);
    await runThreadSearch(client, native, 'de');
    expect(asked).toEqual([]);
    await runThreadSearch(client, native, 'x'.repeat(250));
    expect(asked.map(entry => `${String(entry.environmentId)}:${String(entry.query).length}`)).toEqual(['env-search-a:200', 'env-search-b:200']);
  });

  test('a replaced query\'s answers neither repaint the list nor show; the current query\'s each repaint it', async () => {
    fleet.entries.set(KEY, background(theirs));
    const { client, native, release } = focused(threads(), here, there, true);
    const replaced = landing(), current = landing();
    type(client, 'deploy');
    const first = runThreadSearch(client, native, 'deploy', replaced.landed);
    type(client, 'fridays');
    const second = runThreadSearch(client, native, 'fridays', current.landed);
    release(); await Promise.all([first, second]);
    expect(replaced.count()).toBe(0);
    expect(current.count()).toBe(2);
    type(client, 'deploy');
    expect(ids(client)).toEqual(['title']); // only the title: nothing was found for "deploy" itself
  });
});

describe('the highlight', () => {
  test('follows its thread when a row lands above it; a new query starts at the top; the pointer moves it', async () => {
    const { client, native } = focused([row('title', 'Deploy checklist'), row('mine', 'Weekly notes')], [match('mine', 'deploy now')]);
    type(client, 'deploy');
    await runThreadSearch(client, native, 'deploy');
    view(client);
    await sidebarLocal(client, native, 'search-key', '', 'ArrowDown');
    expect(view(client).sidebar.searchSelectedId).toBe('mine');
    client.shell.threads = [row('new', 'Deploy', { latestUserMessageAt: ago(1000) }), ...client.shell.threads];
    expect(ids(client)).toEqual(['new', 'title', 'mine']);
    expect(view(client).sidebar.searchSelectedId).toBe('mine');
    await sidebarLocal(client, native, 'search-hover', 'title', '');
    expect(view(client).sidebar.searchSelectedId).toBe('title');
    await sidebarLocal(client, native, 'search-hover', 'zzz', '');
    expect(view(client).sidebar.searchSelectedId).toBe('title');
    type(client, 'deploy ');
    expect(view(client).sidebar.searchSelectedId).toBe('new');
  });
});

describe('opening a message row finds its match', () => {
  const threads = () => [row('open', 'Open thread'), row('mine', 'Weekly notes'), row('other', 'Other notes')];

  test('at the matched message, the query as the snippet writes it, once the thread shows', async () => {
    const { client, native, findCalls } = focused(threads(), [match('mine', 'We Deploy on Fridays', { messageId: 'm-mine' })]);
    type(client, 'deploy');
    await runThreadSearch(client, native, 'deploy');
    view(client);
    expect(await sidebarSelecting(client, native, 'mine', 'search')).toBe(false);
    expect(client.query).toBe('');
    Object.assign(client, { threadId: 'mine' });
    sidebarOpened(client, native);
    expect(threadFindView(client)).toMatchObject({ open: true, query: 'Deploy' });
    expect(threadFindState(client).model.start).toEqual({ entryId: 'm-mine', occurrence: 0 });
    await threadFindTick(client, native, NOW);
    expect(findCalls).toEqual([{ method: 'orchestration.searchThread', payload: { threadId: 'mine', query: 'Deploy', skills: [], start: { entryId: 'm-mine', occurrence: 0 } } }]);
  });

  test('another environment\'s row finds it after the reconnect; another thread shown first drops it; a title row opens no find', async () => {
    fleet.entries.set(KEY, background([row('theirs', 'Release log')]));
    const { client, native } = focused(threads(), [match('mine', 'deploy here')], [match('theirs', 'deploy there', { messageId: 'm-theirs' })]);
    Object.assign(client, { threadId: 'open' });
    type(client, 'deploy');
    await runThreadSearch(client, native, 'deploy');
    view(client);
    await sidebarSelecting(client, native, 'fleet:env-search-b:theirs', 'search');
    sidebarOpened(client, native); // still the thread it was picked from, while the reconnect runs
    expect(threadFindView(client).open).toBe(false);
    Object.assign(client, { environmentId: 'env-search-b', threadId: 'theirs', generation: 2 });
    sidebarOpened(client, native);
    expect(threadFindView(client)).toMatchObject({ open: true, query: 'deploy' });
    expect(threadFindState(client).model.start).toEqual({ entryId: 'm-theirs', occurrence: 0 });

    Object.assign(client, { environmentId: 'env-search-a', threadId: 'open', generation: 3 });
    type(client, 'deploy');
    await runThreadSearch(client, native, 'deploy');
    view(client);
    await sidebarSelecting(client, native, 'mine', 'search');
    Object.assign(client, { threadId: 'other' });
    sidebarOpened(client, native);
    Object.assign(client, { threadId: 'mine' });
    sidebarOpened(client, native);
    expect(threadFindView(client).open).toBe(false);
    type(client, 'open');
    view(client);
    await sidebarSelecting(client, native, 'open', 'search');
    Object.assign(client, { threadId: 'open' });
    sidebarOpened(client, native);
    expect(threadFindView(client).open).toBe(false);
  });

  test('a server without thread find opens none', async () => {
    const { client, native } = focused(threads(), [match('mine', 'deploy here')]);
    (client.config as Obj).threadFind = false;
    type(client, 'deploy');
    await runThreadSearch(client, native, 'deploy');
    view(client);
    await sidebarSelecting(client, native, 'mine', 'search');
    Object.assign(client, { threadId: 'mine' });
    sidebarOpened(client, native);
    expect(threadFindView(client).open).toBe(false);
  });
});

describe('the excerpt (ThreadSearchMatchExcerpt)', () => {
  test('"You:" in the info ink for the user, "Agent:" in the success ink for the agent; every hit of the query in semibold', () => {
    expect(searchMatch({ source: 'user', snippet: 'fork here too' }, 'fork')).toMatchObject({ matchLabel: 'You:', matchUser: true });
    const agent = searchMatch({ source: 'assistant', snippet: '# Fixture complete\n\nThe GENUINE backend, genuine output.' }, 'genuine');
    expect(agent).toMatchObject({ matchLabel: 'Agent:', matchUser: false });
    expect(agent.matchParts.map(part => [part.text, part.hit])).toEqual([
      ['# Fixture complete The ', false], ['GENUINE', true], [' backend, ', false], ['genuine', true], [' output.', false]]);
  });

  test('no match, or one without a snippet, shows no excerpt', () => {
    expect(searchMatch(undefined, 'needle').matchLabel).toBe('');
    expect(searchMatch({ source: 'user', snippet: '  ' }, 'needle').matchLabel).toBe('');
  });
});

describe('findForMessageMatch', () => {
  test('the whole query when the snippet holds it as typed, else its longest word, as the snippet writes it', () => {
    expect(findForMessageMatch({ snippet: 'then vp  run dev again', messageId: 'm' }, 'vp run dev')).toEqual({ query: 'vp  run dev', start: { entryId: 'm', occurrence: 0 } });
    expect(findForMessageMatch({ snippet: 'the Sidebar moved' }, 'fix sidebar')).toEqual({ query: 'Sidebar' });
    expect(findForMessageMatch({ snippet: 'Cafeteria near the Café' }, 'cafe')).toEqual({ query: 'Café' });
    expect(findForMessageMatch({ snippet: 'It’s done' }, "it's done")).toEqual({ query: 'It’s done' });
  });
});
