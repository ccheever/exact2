// T3 Code 9c161462c9 (MIT; see LICENSE-T3): Add project starts on the environment
// (CommandPalette.tsx openAddProjectFlow, buildEnvironmentItem, startAddProjectSourceSelection),
// and every later step acts on the chosen one. Project rows name their own environment
// (projectEnvironmentLocationById) and draw their member's monogram (ProjectFavicon).
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { initialShell, obj, str, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { addProjectView } from '../../src/palette/palette-add';
import { paletteCommand } from '../../src/palette/palette-commands';
import { commandView, rootActions } from '../../src/palette/palette';
import { paletteView } from '../../src/palette/palette-view';
import { environmentOptionLabel } from '../../src/palette/palette-environments';
import { fleet, environmentKey, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { primary, type PrimaryTarget } from '../../src/connections/local-primary';
import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
import { showPendingFind } from '../../src/sidebar/sidebar-search';
import { threadFindState, threadFindView } from '../../src/timeline/thread-find-runtime';

const PRIMARY = environmentKey('http://127.0.0.1:16601', 'env-primary');
const THIRD = environmentKey('http://127.0.0.1:16602', 'env-third');
afterEach(() => { fleet.entries.clear(); fleet.saved = []; primary.target = null; });

function entry(key: string, environmentId: string, label: string, config: Obj = {}, projects: Obj[] = []): FleetEntry {
  return { key, origin: key.split('\n')[0]!, environmentId, phase: 'connected', message: '', traceId: '', generation: 2, synchronized: 2, lastEvent: 0, subscriptions: {},
    config: { environment: { environmentId, label }, settings: {}, ...config }, shell: { ...initialShell(), projects }, scopes: [...AUTH_STANDARD_CLIENT_SCOPES], error: '', requested: true };
}
/** Focused on Lane Second; This machine (the primary) and Lane Third are background environments. */
function threeEnvironments(): T3Client {
  primary.target = { environmentId: 'env-primary', httpBaseUrl: 'http://127.0.0.1:16601' } as PrimaryTarget;
  fleet.entries.set(PRIMARY, { ...entry(PRIMARY, 'env-primary', "Daehyeon's MacBook Pro", { environment: { environmentId: 'env-primary', label: "Daehyeon's MacBook Pro", platform: { machine: 'laptop' } } }), primary: true });
  fleet.entries.set(THIRD, entry(THIRD, 'env-third', 'Lane Third', { newProjectsRoot: '/third/projects' }, [{ id: 'p9', title: 'present', workspaceRoot: '/third/present' }]));
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env-second', origin: 'http://127.0.0.1:16580', scopes: [...AUTH_STANDARD_CLIENT_SCOPES] });
  c.config = { environment: { label: 'Lane Second', capabilities: { serverResolvedCommandContext: true } }, settings: {}, keybindings: [] };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'agent-edits', workspaceRoot: '/second/agent-edits' }], threads: [] };
  c.projectId = 'p1';
  return c;
}
/** Answers per transport: the focused connection's (no `fleet`) and each background one's, by key. */
class Fake implements Native {
  available = true; calls: Obj[] = []; replies: Record<string, Record<string, unknown>> = {};
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const generation = Number(request.generation) || 1;
    if (request.op === 'ids') return { ok: true, generation, value: Array.from({ length: Number(request.count) }, (_, i) => `id-${i}`) };
    const value = this.replies[str(request.fleet)]?.[str(request.method)];
    return { ok: true, generation, value: value ?? {} };
  }
}
const files = (): Files => ({ fs: { async mkdir() {}, async readFile() { throw new Error('none'); }, async atomicWriteFile() {} } });
const authenticated = { sourceControlProviders: [{ kind: 'github', status: 'available', auth: { status: 'authenticated', account: 'fixture' } }] };

describe('Add project: the Environments step', () => {
  test('lists every connected environment, This device first, then by label', async () => {
    const view = await addProjectView(threeEnvironments(), new Fake(), { page: 'add-project', query: '', highlighted: false });
    expect(view.rows.map(row => [row.header, row.title, row.description, row.arg])).toEqual([
      ['Environments', "Daehyeon's MacBook Pro", 'This device', 'add-project/env:env-primary'],
      ['', 'Lane Second', 'env-second', 'add-project/env:env-second'],
      ['', 'Lane Third', 'env-third', 'add-project/env:env-third'],
    ]);
    expect(view.rows.map(row => [row.icon, row.project])).toEqual([['machine', 'laptop'], ['machine', 'server'], ['machine', 'server']]);
    expect(view.back).toBe(true);
  });

  test('a background environment that is still connecting is not listed', async () => {
    const c = threeEnvironments();
    fleet.entries.get(THIRD)!.synchronized = 1;
    const view = await addProjectView(c, new Fake(), { page: 'add-project', query: '', highlighted: false });
    expect(view.rows.map(row => row.title)).toEqual(["Daehyeon's MacBook Pro", 'Lane Second']);
  });

  test('the chosen environment\'s discovery sets the Sources; Back returns to the Environments', async () => {
    const c = threeEnvironments(), native = new Fake();
    native.replies[THIRD] = { 'server.discoverSourceControl': authenticated };
    const second = await addProjectView(c, native, { page: 'add-project/env:env-second', query: '', highlighted: false });
    expect(second.parent).toBe('add-project');
    expect(second.rows.map(row => row.title)).toEqual(['Local folder', 'Git URL', 'Azure DevOps repository', 'Bitbucket repository', 'Forgejo / Gitea repository',
      'GitCafe repository', 'GitHub repository', 'GitLab repository']);
    expect(second.rows.slice(2).every(row => row.kind === 'disabled' && row.badge === 'Setup Required')).toBe(true);
    const third = await addProjectView(c, native, { page: 'add-project/env:env-third', query: '', highlighted: false });
    expect(third.rows.map(row => [row.title, row.kind])).toEqual([['New project', 'action'], ['Local folder', 'action'], ['Git URL', 'action'], ['GitHub repository', 'action'],
      ['Azure DevOps repository', 'disabled'], ['Bitbucket repository', 'disabled'], ['Forgejo / Gitea repository', 'disabled'], ['GitCafe repository', 'disabled'], ['GitLab repository', 'disabled']]);
    expect(third.rows[1]!.arg).toBe('add-project/env:env-third/browse');
    const asked = native.calls.filter(call => call.method === 'server.discoverSourceControl').map(call => str(call.fleet));
    expect(asked).toEqual(['', THIRD]);
  });

  test('browsing and adding a folder go to the chosen environment, which then opens the project', async () => {
    const c = threeEnvironments(), native = new Fake();
    native.replies[THIRD] = { 'filesystem.browse': { parentPath: '/third/', entries: [{ name: 'tool', fullPath: '/third/tool' }] } };
    const view = await addProjectView(c, native, { page: 'add-project/env:env-third/browse', query: '/third/', highlighted: false });
    expect(view.rows.map(row => row.title)).toEqual(['..', 'tool']);
    expect(view.footerAction).toBe('');
    const result = await paletteCommand(c, native, files(), 'add-path', '/third/tool', 'add-project/env:env-third/browse');
    const create = native.calls.find(call => call.method === 'projects.mutate')!;
    expect([create.fleet, obj(create.payload).type, obj(create.payload).workspaceRoot]).toEqual([THIRD, 'project.create', '/third/tool']);
    expect(result).toMatchObject({ close: true, project: 'fleet:env-third:id-1' });
    // A folder the environment already holds opens that project.
    expect(await paletteCommand(c, native, files(), 'add-path', '/third/present', 'add-project/env:env-third/browse')).toMatchObject({ project: 'fleet:env-third:p9' });
  });

  test('the primary\'s folder can open in Finder', async () => {
    const view = await addProjectView(threeEnvironments(), new Fake(), { page: 'add-project/env:env-primary/browse', query: '~/', highlighted: false });
    expect(view.footerAction).toBe('Open in Finder');
  });

  test('with one environment Add project is its Sources; with none the action opens Settings › Connections', async () => {
    const c = threeEnvironments();
    fleet.entries.clear(); primary.target = null;
    expect((await addProjectView(c, new Fake(), { page: 'add-project', query: '', highlighted: false })).rows[0]!.header).toBe('Sources');
    const add = (client: T3Client) => rootActions(client).find(item => item.row.key === 'add-project')!.row;
    expect(add(c)).toMatchObject({ op: 'page', arg: 'add-project' });
    c.connection = 'disconnected';
    expect(add(c)).toMatchObject({ op: 'settings', arg: 'connections' });
  });

  test('Environments and Sources, where nothing matches the query, list the root\'s results', async () => {
    const c = threeEnvironments(), native = new Fake();
    // The groups shown, each with its first row.
    const shown = async (page: string, query: string) => (await paletteView(c, native, [true, 'command', page, query, '', false, 0, 'dark'])).rows
      .filter(row => row.header).map(row => [row.header, row.title]);
    expect(await shown('add-project', 'third')).toEqual([['Environments', 'Lane Third']]);
    expect(await shown('add-project', 'usage')).toEqual([['Actions', 'Open usage'], ['Settings', 'Usage providers']]);
    expect(await shown('add-project/env:env-second', 'git url')).toEqual([['Sources', 'Git URL']]);
    expect(await shown('add-project/env:env-second', 'usage')).toEqual([['Actions', 'Open usage'], ['Settings', 'Usage providers']]);
  });

  test('resolveEnvironmentOptionLabel: a generic primary label reads "This device"', () => {
    expect(environmentOptionLabel(true, 'env-primary', 'Local')).toBe('This device');
    expect(environmentOptionLabel(true, 'env-primary', "Daehyeon's MacBook Pro")).toBe("Daehyeon's MacBook Pro");
    expect(environmentOptionLabel(false, 'env-third', '', '')).toBe('env-third');
  });
});

// CommandPalette.tsx newProjectMachineGroup, switchNewProjectEnvironment and showExistingProjectSources.
describe('New project: its Environments', () => {
  /** threeEnvironments, with This device able to make a project too (Lane Third already can). */
  function twoCanCreate(): T3Client {
    const c = threeEnvironments(), primaryEntry = fleet.entries.get(PRIMARY)!;
    fleet.entries.set(PRIMARY, { ...primaryEntry, config: { ...primaryEntry.config, newProjectsRoot: '/primary/projects' } });
    return c;
  }
  const page = 'add-project/env:env-third/new-project';
  const shown = (view: { rows: { header: string; title: string; description: string; trailing: string }[] }) => view.rows.map(row => [row.header, row.title, row.description, row.trailing]);

  test('lists those that can make a project, the chosen one checked; choosing another moves the project there', async () => {
    const c = twoCanCreate(), native = new Fake();
    native.replies[THIRD] = { 'server.discoverSourceControl': authenticated };
    const view = await addProjectView(c, native, { page, query: 'Demo', highlighted: false });
    expect(shown(view)).toEqual([
      ['Environments', "Daehyeon's MacBook Pro", 'This device', ''],
      ['', 'Lane Third', 'env-third', 'check'],
      ['Options', 'Create private repository on GitHub', 'fixture/demo', ''],
      ['', 'Add existing project', 'Open a folder or clone a repository', 'chevron'],
    ]);
    expect(view.rows[0]).toMatchObject({ icon: 'machine', project: 'laptop', op: 'flow', arg: 'new-project-environment', arg2: 'env-primary' });
    expect([view.contextDescription, view.count]).toEqual(['Creates /third/projects/demo', 4]);
    // This device: its folder, its own discovery (no publish host is ready there), the check; the name and the option stay.
    await paletteCommand(c, native, files(), 'toggle-publish', '', page);
    expect(await paletteCommand(c, native, files(), 'new-project-environment', 'env-primary', page)).toMatchObject({ ok: true, close: false });
    const moved = await addProjectView(c, native, { page, query: 'Demo', highlighted: false });
    expect(shown(moved).map(([, title, , trailing]) => [title, trailing])).toEqual([["Daehyeon's MacBook Pro", 'check'], ['Lane Third', ''], ['Add existing project', 'chevron']]);
    expect(moved.contextDescription).toBe('Creates /primary/projects/demo');
    expect(native.calls.filter(call => call.method === 'server.discoverSourceControl').map(call => str(call.fleet))).toEqual([THIRD, PRIMARY]);
    // Create makes it there, unpublished: no host is ready on This device (publishTo null).
    native.replies[PRIMARY] = { 'projects.createNew': { workspaceRoot: '/primary/projects/demo', projectId: 'pd' } };
    expect(await paletteCommand(c, native, files(), 'new-project', 'Demo', page)).toMatchObject({ close: true, project: 'fleet:env-primary:pd' });
    const create = native.calls.find(call => call.method === 'projects.createNew')!;
    expect([create.fleet, obj(create.payload).name]).toEqual([PRIMARY, 'Demo']);
    expect(native.calls.some(call => call.method === 'sourceControl.publishRepository')).toBe(false);
  });

  test('Add existing project opens the chosen environment\'s Sources, the one it came from while that is still the choice', async () => {
    const c = twoCanCreate(), native = new Fake();
    const existing = async (on: string) => (await addProjectView(c, native, { page: on, query: '', highlighted: false })).rows.find(row => row.title === 'Add existing project')!.arg;
    expect(await existing(page)).toBe('add-project/env:env-third');
    await paletteCommand(c, native, files(), 'new-project-environment', 'env-primary', page);
    expect(await existing(page)).toBe('add-project/env:env-primary');
    // From the root, New project starts on the first that can make one (openNewProjectFlow), and its Sources open on
    // their own: Back returns to the root.
    expect(await existing('new-project')).toBe('env:env-primary');
    const sources = await paletteView(c, native, [true, 'command', 'env:env-primary', '', '', false, 0, 'dark']);
    expect([sources.parent, sources.rows[0]!.header, sources.rows[0]!.title, sources.rows[0]!.arg]).toEqual(['', 'Sources', 'New project', 'env:env-primary/new-project']);
    expect(await existing('env:env-primary/new-project')).toBe('env:env-primary');
    await paletteCommand(c, native, files(), 'new-project-environment', 'env-third', 'env:env-primary/new-project');
    expect(await existing('env:env-primary/new-project')).toBe('env:env-third');
  });

  test('with one that can make a project there is no Environments, unless the chosen one no longer can', async () => {
    const c = threeEnvironments(), native = new Fake();
    expect(shown(await addProjectView(c, native, { page, query: 'Demo', highlighted: false })).map(([header, title]) => [header, title])).toEqual([['', 'Add existing project']]);
    // Lane Second has no folder for new projects: Lane Third is offered, unchecked, and nothing says where it goes.
    const elsewhere = await addProjectView(c, native, { page: 'add-project/env:env-second/new-project', query: 'Demo', highlighted: false });
    expect(shown(elsewhere).map(([header, title, , trailing]) => [header, title, trailing])).toEqual([['Environments', 'Lane Third', ''], ['', 'Add existing project', 'chevron']]);
    expect(elsewhere.contextDescription).toBe('');
  });

  test('the choice lasts while its page is shown: another page, or the palette closing, forgets it', async () => {
    const c = twoCanCreate(), native = new Fake();
    const checked = async (args: unknown[]) => (await paletteView(c, native, args)).rows.find(row => row.trailing === 'check')?.title;
    const onPage = [true, 'command', page, 'Demo', '', false, 0, 'dark'];
    await paletteCommand(c, native, files(), 'new-project-environment', 'env-primary', page);
    expect(await checked(onPage)).toBe("Daehyeon's MacBook Pro");
    await paletteView(c, native, [true, 'command', 'add-project/env:env-third', '', '', false, 0, 'dark']);
    expect(await checked(onPage)).toBe('Lane Third');
    await paletteCommand(c, native, files(), 'new-project-environment', 'env-primary', page);
    await paletteView(c, native, [false, 'command', page, 'Demo', '', false, 0, 'dark']);
    expect(await checked(onPage)).toBe('Lane Third');
  });

  test('the root offers New project when any connected environment can make one', () => {
    const c = threeEnvironments(), offered = () => rootActions(c).some(item => item.row.key === 'new-project');
    expect(offered()).toBe(true); // Lane Third can; the focused Lane Second cannot
    fleet.entries.delete(THIRD);
    expect(offered()).toBe(false);
  });
});

// CommandPaletteThreadItems.tsx useConnectedThreadSearch and the message items' run (openThreadFindOnQuery).
describe('⌘K message results from every connected environment', () => {
  const NOW = Date.parse('2026-10-28T12:00:00.000Z'), at = (hours: number) => new Date(NOW - hours * 3_600_000).toISOString();
  /** Lane Second (focused) and Lane Third each hold a thread whose messages say "lakebed"; a third thread is named for it. */
  function searching(): { c: T3Client; native: Fake } {
    const c = threeEnvironments(), native = new Fake();
    c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'agent-edits', workspaceRoot: '/second/agent-edits' }],
      threads: [{ id: 't2', projectId: 'p1', title: 'Second notes', updatedAt: at(2) }, { id: 't3', projectId: 'p1', title: 'Lakebed survey', updatedAt: at(3) }] };
    fleet.entries.get(THIRD)!.shell.threads = [{ id: 't9', projectId: 'p9', title: 'Deploy log', updatedAt: at(1), lastVisitedAt: at(1) }];
    const match = (threadId: string, snippet: string, extra: Obj = {}) => ({ threadId, messageId: `m-${threadId}`, source: 'user', snippet, quality: 1, hits: 1, messageCreatedAt: at(4), ...extra });
    native.replies[''] = { 'orchestration.searchThreads': { matches: [match('t2', 'the lakebed is dry'), match('t3', 'lakebed again')] } };
    native.replies[THIRD] = { 'orchestration.searchThreads': { matches: [match('t9', 'Lakebed deploy finished', { source: 'assistant', hits: 3 })] } };
    return { c, native };
  }
  const view = (c: T3Client, native: Fake) => paletteView(c, native, [true, 'command', '', 'lakebed', '', false, NOW, 'dark']);

  test('are searched on each environment\'s own transport and ranked together', async () => {
    const { c, native } = searching();
    const rows = (await view(c, native)).rows;
    expect(native.calls.filter(call => call.method === 'orchestration.searchThreads').map(call => [str(call.fleet), obj(call.payload).query])).toEqual([['', 'lakebed'], [PRIMARY, 'lakebed'], [THIRD, 'lakebed']]);
    // Lane Third's thread is listed with its environment; "Lakebed survey" matches by its title, so it is no message row.
    expect(rows.filter(row => row.key.startsWith('threads-messages:')).map(row => [row.arg, row.title, row.matchLabel, row.context])).toEqual([
      ['fleet:env-third:t9', 'Deploy log', 'Agent:', 'present · Lane Third'],
      ['t2', 'Second notes', 'You:', 'agent-edits · Lane Second'],
    ]);
    expect(rows.find(row => row.key.startsWith('threads-search:'))!.arg).toBe('t3');
  });

  test('a message row opens its thread in its environment, tells that server of the pick, and finds the message once it shows', async () => {
    const { c, native } = searching();
    await view(c, native);
    expect(await paletteCommand(c, native, files(), 'thread', 'fleet:env-third:t9', `${NOW} lakebed`)).toMatchObject({ close: true, thread: 'fleet:env-third:t9' });
    const visit = native.calls.find(call => call.method === 'orchestration.dispatchCommand')!;
    expect([visit.fleet, visit.deliver]).toEqual([THIRD, 'palette-visit:id-0']);
    expect(obj(visit.payload)).toMatchObject({ type: 'thread.visit', threadId: 't9', opened: true, searchQuery: 'lakebed' });
    showPendingFind(c); // still Lane Second's, while the reconnect runs
    expect(threadFindView(c).open).toBe(false);
    Object.assign(c, { environmentId: 'env-third', threadId: 't9', generation: 2 });
    c.config = { ...c.config, threadFind: true };
    showPendingFind(c);
    expect(threadFindView(c)).toMatchObject({ open: true, query: 'Lakebed' });
    expect(threadFindState(c).model.start).toEqual({ entryId: 'm-t9', occurrence: 0 });
  });

  test('a title row, or a thread picked where the root query is not shown, opens no find', async () => {
    const { c, native } = searching();
    c.config = { ...c.config, threadFind: true };
    await view(c, native);
    await paletteCommand(c, native, files(), 'thread', 't3', `${NOW} lakebed`);
    Object.assign(c, { threadId: 't3' });
    showPendingFind(c);
    expect(threadFindView(c).open).toBe(false);
    // A message row's thread picked from an Add project page's fallback, whose picks carry no query.
    await paletteCommand(c, native, files(), 'thread', 't2', `${NOW} `);
    Object.assign(c, { threadId: 't2' });
    showPendingFind(c);
    expect(threadFindView(c).open).toBe(false);
  });
});

describe('palette project rows', () => {
  test('name their environment beside the path and draw the member\'s monogram', () => {
    const c = threeEnvironments();
    const identity = { canonicalKey: 'github.com/t3-fixture/shared-notes', displayName: 't3-fixture/shared-notes', name: 'shared-notes' };
    c.shell = { sequence: 1, projects: [
      { id: 'p1', title: 'shared-notes', workspaceRoot: '/second/shared-notes', repositoryIdentity: { ...identity, rootPath: '/second/shared-notes' } },
      { id: 'p3', title: 'shared-notes-review', workspaceRoot: '/second/shared-notes-review', repositoryIdentity: { ...identity, rootPath: '/second/shared-notes-review' } },
      { id: 'p2', title: 'agent-edits', workspaceRoot: '/second/agent-edits' }], threads: [] };
    const view = commandView(c, { page: 'new-thread-in', query: '', now: 0, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false });
    const shared = view.rows.find(row => row.arg === 'p1')!;
    expect([shared.title, shared.project, shared.env, shared.envMachine, shared.description]).toEqual(['t3-fixture/shared-notes', 'SN', 'Lane Second', 'server', '/second/shared-notes']);
    expect(view.rows.find(row => row.arg === 'p2')).toMatchObject({ env: 'Lane Second', project: 'AE' });
    // The Projects search names a grouped project's environments instead of its path (ProjectSearchDescription).
    const search = commandView(c, { page: '', query: 'shared', now: 0, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false });
    expect(search.rows.find(row => row.key === 'projects-search:project:p1')).toMatchObject({ project: 'SN', env: 'Lane Second', envMachine: '', description: '' });
  });

  test('the primary\'s projects read "Local", with no machine glyph', () => {
    const c = threeEnvironments();
    fleet.entries.set(PRIMARY, { ...fleet.entries.get(PRIMARY)!, shell: { ...initialShell(), projects: [{ id: 'pm', title: 'mine', workspaceRoot: '/me/mine' }] } });
    const view = commandView(c, { page: 'new-thread-in', query: '', now: 0, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false });
    expect(view.rows.find(row => row.arg === 'fleet:env-primary:pm')).toMatchObject({ env: 'Local', envMachine: '', description: '/me/mine' });
  });
});
