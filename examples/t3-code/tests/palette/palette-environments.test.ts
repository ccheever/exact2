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
import { environmentOptionLabel } from '../../src/palette/palette-environments';
import { fleet, environmentKey, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { primary, type PrimaryTarget } from '../../src/connections/local-primary';
import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';

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

  test('resolveEnvironmentOptionLabel: a generic primary label reads "This device"', () => {
    expect(environmentOptionLabel(true, 'env-primary', 'Local')).toBe('This device');
    expect(environmentOptionLabel(true, 'env-primary', "Daehyeon's MacBook Pro")).toBe("Daehyeon's MacBook Pro");
    expect(environmentOptionLabel(false, 'env-third', '', '')).toBe('env-third');
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
