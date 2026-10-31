// Adding a project to another machine, against T3 Code 74541bab17 (MIT, see LICENSE-T3):
// lib/addProjectToMachine.ts and its test, hooks/useAddProjectTargets.ts, AddProjectToMachineDialog.tsx,
// the silenced clone toasts of ProjectCloneToastCoordinator.tsx and ChatView's file-copy banner.
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { initialShell, obj, type Obj } from '../../src/client/domain';
import { liveFleetEvent, liveFleetPass, PROJECT_CLONES_KEY } from '../../src/client/live-streams';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { addProjectTargets, cloneUrlForProtocol, cloneUrlProtocol, defaultSelection, fileCopyPass, isDefaultCopiedFile, isProjectCloneToastSilenced, projectFileCopy, projectRemoteUrl,
  projectSubdirectory } from '../../src/shell/add-project-machine';
import { addMachineLocal, addMachineView, splitForMiddleTruncate, submitAddMachine } from '../../src/shell/add-project-dialog';
import { projectCloneBanner, projectCloneSendBlockReason, syncCloneToasts, type CloneToast, type ToastPort, type TrackedToast } from '../../src/shell/project-clones';
import { environmentView, stripRunOn } from '../../src/details/git-env';
import { runConnectionOp, CONNECTION_OPS } from '../../src/connections/connections';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';

describe('cloneUrlForProtocol (addProjectToMachine.test.ts)', () => {
  test('switches between scp-style SSH and HTTPS', () => {
    expect(cloneUrlForProtocol('git@github.com:pingdotgg/t3code.git', 'https')).toBe('https://github.com/pingdotgg/t3code.git');
    expect(cloneUrlForProtocol('https://github.com/pingdotgg/t3code.git', 'ssh')).toBe('git@github.com:pingdotgg/t3code.git');
    expect(cloneUrlForProtocol('ssh://git@gitlab.com/group/sub/repo.git', 'https')).toBe('https://gitlab.com/group/sub/repo.git');
  });
  test('keeps the URL as is for its own protocol and drops HTTPS credentials on switch', () => {
    expect(cloneUrlForProtocol('https://token@github.com/a/b.git', 'https')).toBe('https://token@github.com/a/b.git');
    expect(cloneUrlForProtocol('https://token@github.com/a/b.git', 'ssh')).toBe('git@github.com:a/b.git');
  });
  test('refuses rewrites that would guess wrong', () => {
    expect(cloneUrlForProtocol('ssh://git@host.example:2222/a/b.git', 'https')).toBeNull();
    expect(cloneUrlForProtocol('https://host.example:8443/a/b.git', 'ssh')).toBeNull();
    expect(cloneUrlForProtocol('https://host.example:8443/a/b.git', 'https')).toBe('https://host.example:8443/a/b.git');
    expect(cloneUrlForProtocol('/srv/git/repo.git', 'https')).toBeNull();
    expect(cloneUrlProtocol('file:///srv/git/repo.git')).toBeNull();
  });
});

const REMOTE = 'https://github.com/acme/widgets.git';
const identity = (root = '/repo') => ({ canonicalKey: 'github.com/acme/widgets', rootPath: root, displayName: 'acme/widgets', locator: { remoteUrl: REMOTE } });
const files = [
  { path: '.env', byteLength: 40 }, { path: 'apps/web/.env.local', byteLength: 20 }, { path: 'big/.env', byteLength: 2 * 1024 * 1024 }, { path: 'notes.log', byteLength: 7 },
];

type Call = { method: string; payload: Obj; fleet: string };
/** The focused environment "Laptop" (env, project p1 at /repo/app inside the repository /repo) and the bridge every environment answers through. */
function harness(options: { listing?: boolean; writeFails?: string } = {}) {
  const calls: Call[] = [];
  const answer = (method: string, payload: Obj): Obj => {
    if (method === 'projects.listIgnoredFiles') return { files, truncated: false };
    if (method === 'projects.readFile') return { contents: `contents of ${String(payload.relativePath)}`, truncated: false };
    if (method === 'projectClone.start') return { cwd: '/home/studio/widgets' };
    return {};
  };
  const client = {
    environmentId: 'env', origin: 'http://127.0.0.1:16960', threadId: '', projectId: 'p1', draftKey: 'env:new:p1', draft: 'carry me', ready: true, connection: 'connected', generation: 3, presentation: {},
    config: { environment: { label: 'Laptop', capabilities: { projectCloneTracking: true, ignoredFileListing: options.listing !== false } }, settings: {} },
    local: { groupingMode: 'repository', groupingOverrides: {}, drafts: { 'env:new:p1': 'carry me' } as Record<string, string>, selections: {} as Record<string, { projectId: string; threadId: string }>,
      composerControls: { contexts: { 'env:new:p1': { envMode: 'worktree', branch: 'main', worktreePath: '' } } } },
    shell: { ...initialShell(), projects: [{ id: 'p1', title: 'widgets', workspaceRoot: '/repo/app', repositoryIdentity: identity() }], threads: [] },
    projection: {},
    restAccess: () => ({
      request: async (method: string, payload: Obj) => { calls.push({ method, payload, fleet: '' }); return answer(method, payload); },
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `new-${index}`),
    }),
  };
  const native = {
    available: true, watch: () => {},
    later: async (input: unknown) => {
      const request = obj(input), key = String(request.fleet ?? '');
      calls.push({ method: String(request.method ?? request.op), payload: request, fleet: key });
      if (request.op === 'request' && request.method === 'projects.writeFile' && options.writeFails === String(obj(request.payload).relativePath))
        return { ok: false, generation: 5, error: { kind: 'transport', message: 'disk full' } };
      if (request.op === 'request') return { ok: true, generation: 5, value: answer(String(request.method), obj(request.payload)) };
      if (request.op === 'connect') return { ok: true, generation: 9, value: { state: 'connecting', origin: request.origin, environmentId: 'env2', message: '' } };
      return { ok: true, generation: 5, value: {} };
    },
  } as unknown as Native;
  return { client: client as unknown as T3Client & typeof client, native, calls };
}

/** A synchronized background environment in the window's fleet; `project` makes it hold the same repository. */
async function machine(id: string, label: string, over: Partial<FleetEntry> & { project?: boolean } = {}): Promise<FleetEntry> {
  const { project, ...rest } = over;
  const entry: FleetEntry = {
    key: `http://127.0.0.1:16521\n${id}`, origin: 'http://127.0.0.1:16521', environmentId: id, phase: 'connected', message: '', traceId: '',
    generation: 5, synchronized: 5, lastEvent: 0, subscriptions: {}, scopes: ['orchestration:read', 'source-control:write'], error: '', requested: false,
    config: { environment: { label, capabilities: { projectCloneTracking: true } }, settings: { addProjectBaseDirectory: '~/code' } },
    shell: { ...initialShell(), projects: project ? [{ id: `q-${id}`, title: 'widgets', workspaceRoot: '/elsewhere/widgets', repositoryIdentity: identity('/elsewhere/widgets') }] : [] }, ...rest,
  };
  fleet.entries.set(entry.key, entry);
  let serial = 0;
  await liveFleetPass(async () => ({ id: `5-${++serial}` }), entry);
  return entry;
}
const clones = (entry: FleetEntry, serial: number, value: Obj[]) => liveFleetEvent(entry, { key: PROJECT_CLONES_KEY, generation: 5, subscriptionId: `5-${serial}`, value });
afterEach(() => { fleet.entries.clear(); resetPrimary(); });

describe('target selection (useAddProjectTargets)', () => {
  test('connected machines without the project that can clone it, by label; a project without a remote gets none', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client } = harness();
    await machine('zeta', 'Zeta');
    await machine('alpha', 'Alpha');
    await machine('has', 'Has it', { project: true });
    await machine('readonly', 'Read only', { scopes: ['orchestration:read'] });
    await machine('old', 'Old server', { config: { environment: { label: 'Old server', capabilities: {} } } });
    await machine('syncing', 'Syncing', { synchronized: 4 });
    await machine('down', 'Down', { phase: 'error' });
    expect(addProjectTargets(client).map(target => [target.environmentId, target.label])).toEqual([['alpha', 'Alpha'], ['zeta', 'Zeta']]);
    client.shell.projects[0]!.repositoryIdentity = { ...identity(), locator: { remoteUrl: '' } };
    expect(addProjectTargets(client)).toEqual([]);
  });

  test('a project on one machine still gets the Run on menu, holding this machine and "Add to another machine"; a started thread does not', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client } = harness();
    expect(stripRunOn(client).envShow).toBe(false);
    await machine('studio', 'Studio');
    const strip = stripRunOn(client);
    expect([strip.envShow, strip.envOptions.map(option => option.id), strip.envAdd.map(option => option.label)]).toEqual([true, ['env'], ['Studio']]);
    expect(environmentView(client)).toMatchObject({ envPick: true, envAdd: [{ id: 'studio' }] });
    const started = { ...client, threadId: 't1', projection: { messages: [{ id: 'm1' }] } } as unknown as T3Client;
    expect(environmentView(started)).toMatchObject({ envPick: false, envAdd: [] });
  });
});

describe('the dialog (AddProjectToMachineDialog)', () => {
  test('lists the ignored files with the env files picked, large files left out; All and None; the machine picked from the submenu', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client, native, calls } = harness();
    await machine('alpha', 'Alpha');
    await machine('studio', 'Studio');
    await addMachineLocal(client, native, 'open', '', 'studio');
    expect(calls.find(call => call.method === 'projects.listIgnoredFiles')?.payload).toEqual({ cwd: '/repo/app' });
    let view = addMachineView(client);
    expect(view).toMatchObject({ open: true, description: 'Clone acme/widgets and bring the local files git does not track.', sourceLabel: 'Laptop', targetLabel: 'Studio',
      destination: '~/code/widgets', protocolShown: true, protocol: 'https', remoteUrl: REMOTE, folderLabel: 'Folder on Studio', fileSummary: '+ 2 files',
      countLabel: '2 of 4 · 60 B', allLabel: 'All', submitLabel: 'Clone to Studio', canSubmit: true });
    expect(view.targets.map(target => [target.label, target.selected])).toEqual([['Alpha', false], ['Studio', true]]);
    expect(view.files.map(file => [file.path, file.size, file.checked, file.disabled])).toEqual([
      ['.env', '40 B', true, false], ['apps/web/.env.local', '20 B', true, false], ['big/.env', 'Over 1 MB', false, true], ['notes.log', '7 B', false, false]]);
    await addMachineLocal(client, native, 'all', '', '');
    expect(addMachineView(client)).toMatchObject({ countLabel: '3 of 4 · 67 B', allLabel: 'None', fileSummary: '+ 3 files' });
    await addMachineLocal(client, native, 'all', '', '');
    expect(addMachineView(client)).toMatchObject({ countLabel: '0 of 4', allLabel: 'All', fileSummary: 'no files' });
    await addMachineLocal(client, native, 'file', 'big/.env', 'true'); // over the cap: never picked
    await addMachineLocal(client, native, 'file', 'notes.log', 'true');
    expect(addMachineView(client).files.filter(file => file.checked).map(file => file.path)).toEqual(['notes.log']);
    await addMachineLocal(client, native, 'protocol', '', 'ssh');
    await addMachineLocal(client, native, 'target', '', 'alpha');
    view = addMachineView(client);
    expect([view.remoteUrl, view.targetLabel, view.destination]).toEqual(['git@github.com:acme/widgets.git', 'Alpha', '~/code/widgets']);
    await addMachineLocal(client, native, 'close', '', '');
    expect(addMachineView(client).open).toBe(false);
  });

  test('a server without ignoredFileListing offers no files; the folder must be named', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client, native, calls } = harness({ listing: false });
    await machine('studio', 'Studio');
    await addMachineLocal(client, native, 'open', '', '');
    expect(calls.some(call => call.method === 'projects.listIgnoredFiles')).toBe(false);
    expect(addMachineView(client)).toMatchObject({ canListFiles: false, fileSummary: 'no files', canSubmit: true });
    await addMachineLocal(client, native, 'path', '', '  ');
    expect(await submitAddMachine(client, native)).toEqual({ status: null, generation: -1 });
    expect(addMachineView(client).error).toBe('Choose a folder on the other machine.');
  });

  test('Return in the folder field before the ignored files load starts nothing (the button is disabled then)', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client, native, calls } = harness();
    await machine('studio', 'Studio');
    let release!: () => void;
    const gate = new Promise<void>(resolve => { release = resolve; });
    const access = client.restAccess();
    Object.assign(client, { restAccess: () => ({ ...access, request: async (method: string, payload: Obj) => { if (method === 'projects.listIgnoredFiles') await gate; return access.request(method, payload); } }) });
    const opening = addMachineLocal(client, native, 'open', '', 'studio');
    await Promise.resolve();
    expect(addMachineView(client)).toMatchObject({ canSubmit: false, filesNote: 'Finding ignored files…' });
    expect(await submitAddMachine(client, native)).toEqual({ status: null, generation: -1 });
    expect(calls.some(call => call.method === 'projectClone.start' || call.method === 'projects.readFile')).toBe(false);
    release(); await opening;
    expect(addMachineView(client).canSubmit).toBe(true);
  });

  test('paths truncate in the middle, keeping the last folder (MiddleTruncate)', () => {
    expect(splitForMiddleTruncate('/Users/Shared/fixtures/envs/second/repos/empty-project')).toEqual({ head: '/Users/Shared/fixtures/envs/second/repos/', tail: 'empty-project' });
    expect(splitForMiddleTruncate('/repo/a-very-long-folder-name-indeed')).toEqual({ head: '/repo/a-very-long-folder-name-', tail: 'indeed' });
    expect(splitForMiddleTruncate('~/widgets')).toBeNull();
  });

  test('defaults: env files by name, subdirectories below the repository root', () => {
    expect(['.env', 'a/.env.production', '.envrc', '.dev.vars', '.environment', 'env', '.env/'].map(isDefaultCopiedFile)).toEqual([true, true, true, true, false, false, false]);
    expect([...defaultSelection(files)]).toEqual(['.env', 'apps/web/.env.local']);
    expect(projectSubdirectory('/repo', '/repo/app')).toBe('app');
    expect(projectSubdirectory('/repo/', '/repo')).toBe('');
    expect(projectSubdirectory('/repo', '/repository')).toBe('');
    expect(projectSubdirectory(undefined, '/repo/app')).toBe('');
  });
});

describe('adding the project (addProjectToMachine)', () => {
  test('Clone reads the picked files, starts the clone there, moves the draft and the window, then copies once the clone is done', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client, native, calls } = harness();
    const studio = await machine('studio', 'Studio');
    expect(CONNECTION_OPS).toContain('add-machine-submit');
    await addMachineLocal(client, native, 'open', '', 'studio');
    const result = await runConnectionOp(native, 'add-machine-submit', '', '', true, client);
    expect(result).toMatchObject({ status: { environmentId: 'env2' }, generation: 9 });
    expect(calls.filter(call => call.method === 'projects.readFile').map(call => call.payload)).toEqual([
      { cwd: '/repo/app', relativePath: '.env' }, { cwd: '/repo/app', relativePath: 'apps/web/.env.local' }]);
    const start = calls.find(call => call.method === 'projectClone.start')!;
    expect([start.fleet, obj(start.payload.payload)]).toEqual([studio.key, expect.objectContaining({ projectId: 'new-0', title: 'widgets', remoteUrl: REMOTE, destinationPath: '~/code/widgets' })]);
    expect(addMachineView(client).open).toBe(false);
    expect(client.local.drafts).toEqual({ 'studio:new:new-0': 'carry me' });
    expect(client.local.selections.studio).toEqual({ projectId: 'new-0', threadId: '' });
    expect(isProjectCloneToastSilenced(client, 'studio', 'new-0')).toBe(true);
    expect(projectFileCopy(client, 'studio', 'new-0')).toMatchObject({ phase: 'copying' });
    expect(projectCloneSendBlockReason(null, { phase: 'copying' })).toBe('Copying files');

    // The window moved there; its clone list now names the clone.
    fleet.entries.set(studio.key, studio);
    clones(studio, 2, [{ projectId: 'new-0', destinationPath: '/home/studio/widgets', phase: 'running', stage: 'receiving' }]);
    await fileCopyPass(client, native);
    expect(calls.some(call => call.method === 'projects.writeFile')).toBe(false);
    clones(studio, 2, [{ projectId: 'new-0', destinationPath: '/home/studio/widgets', phase: 'done', stage: 'checkout' }]);
    await fileCopyPass(client, native);
    // The project sits at app/ below its repository, so the files land at the same place in the clone.
    expect(calls.filter(call => call.method === 'projects.writeFile').map(call => [call.fleet, obj(call.payload.payload)])).toEqual([
      [studio.key, { cwd: '/home/studio/widgets', relativePath: 'app/.env', contents: 'contents of .env' }],
      [studio.key, { cwd: '/home/studio/widgets', relativePath: 'app/apps/web/.env.local', contents: 'contents of apps/web/.env.local' }]]);
    expect(projectFileCopy(client, 'studio', 'new-0')).toBeNull();
  });

  test('a pass while the copy is writing starts nothing; the copy ends once', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client, native, calls } = harness();
    const studio = await machine('studio', 'Studio');
    await addMachineLocal(client, native, 'open', '', 'studio');
    await submitAddMachine(client, native);
    fleet.entries.set(studio.key, studio);
    clones(studio, 2, [{ projectId: 'new-0', phase: 'done' }]);
    let release!: () => void, reachedWrite!: () => void;
    const gate = new Promise<void>(resolve => { release = resolve; });
    const writeStarted = new Promise<void>(resolve => { reachedWrite = resolve; });
    const slow = { ...native, later: async (input: unknown) => { if (obj(input).method === 'projects.writeFile') { reachedWrite(); await gate; } return native.later(input); } } as Native;
    const first = fileCopyPass(client, slow);
    await writeStarted;
    const second = fileCopyPass(client, slow); // the shell's next pass, with the first write in flight
    expect(projectFileCopy(client, 'studio', 'new-0')).toMatchObject({ phase: 'copying', writing: true });
    release();
    await Promise.all([first, second]);
    expect(calls.filter(call => call.method === 'projects.writeFile').map(call => obj(call.payload.payload).relativePath)).toEqual(['app/.env', 'app/apps/web/.env.local']);
    expect(projectFileCopy(client, 'studio', 'new-0')).toBeNull();
  });

  test('a copy that fails shows its banner until dismissed; a clone that goes away drops its copy', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client, native } = harness({ writeFails: 'app/.env' });
    const studio = await machine('studio', 'Studio');
    await addMachineLocal(client, native, 'open', '', 'studio');
    await submitAddMachine(client, native);
    fleet.entries.set(studio.key, studio);
    clones(studio, 2, [{ projectId: 'new-0', phase: 'done' }]);
    await fileCopyPass(client, native);
    const copy = projectFileCopy(client, 'studio', 'new-0');
    expect(copy).toEqual({ phase: 'failed', title: 'Could not copy 1 file to Studio', failures: ['.env: disk full'] });
    expect(projectCloneBanner(null, copy, 'new-0')).toMatchObject({ id: 'project-file-copy:new-0', variant: 'error', title: 'Could not copy 1 file to Studio',
      description: '.env: disk full', dismiss: 'shelllocal:add-machine-copy-dismiss', action: '' });
    expect(projectCloneSendBlockReason(null, copy)).toBe('');
    const there = Object.assign(client, { environmentId: 'studio', projectId: 'new-0' });
    await addMachineLocal(there, native, 'copy-dismiss', '', '');
    expect(projectFileCopy(client, 'studio', 'new-0')).toBeNull();
  });

  test('closing the dialog while it starts still lands on the clone; a window that cannot follow puts the draft back', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client, native } = harness();
    await machine('studio', 'Studio');
    await addMachineLocal(client, native, 'open', '', 'studio');
    let release!: () => void, reachedStart!: () => void;
    const gate = new Promise<void>(resolve => { release = resolve; });
    const cloneStarted = new Promise<void>(resolve => { reachedStart = resolve; });
    const slow = { ...native, later: async (input: unknown) => { if (obj(input).method === 'projectClone.start') { reachedStart(); await gate; } return native.later(input); } } as Native;
    const submitting = submitAddMachine(client, slow);
    await cloneStarted;
    expect(addMachineView(client).submitLabel).toBe('Starting…');
    await addMachineLocal(client, native, 'close', '', '');
    expect(addMachineView(client).open).toBe(false);
    release();
    expect(await submitting).toMatchObject({ generation: 9 });
    expect(client.local.drafts).toEqual({ 'studio:new:new-0': 'carry me' });

    // The window cannot move: the draft and the machine's selection go back.
    const { client: other, native: otherNative } = harness();
    const entry = await machine('studio', 'Studio');
    await addMachineLocal(other, otherNative, 'open', '', 'studio');
    const refused = { ...otherNative, later: async (input: unknown) => obj(input).op === 'connect'
      ? { ok: false, generation: 5, error: { kind: 'transport', message: 'unreachable' } } : otherNative.later(input) } as Native;
    await expect(submitAddMachine(other, refused)).rejects.toThrow('unreachable');
    expect(other.local.drafts).toEqual({ 'env:new:p1': 'carry me' });
    expect((other.local.composerControls as { contexts: Record<string, Obj> }).contexts['env:new:p1']).toMatchObject({ envMode: 'worktree', branch: 'main' });
    expect(other.local.selections.studio).toBeUndefined();
    void entry;
  });

  test('a clone that leaves the list after it was seen drops its copy; one never seen keeps waiting', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client, native, calls } = harness();
    const studio = await machine('studio', 'Studio');
    await addMachineLocal(client, native, 'open', '', 'studio');
    await submitAddMachine(client, native);
    fleet.entries.set(studio.key, studio);
    clones(studio, 2, []);
    await fileCopyPass(client, native);
    expect(projectFileCopy(client, 'studio', 'new-0')).toMatchObject({ phase: 'copying' });
    clones(studio, 2, [{ projectId: 'new-0', phase: 'failed', error: 'boom' }]);
    await fileCopyPass(client, native);
    expect(projectFileCopy(client, 'studio', 'new-0')).toMatchObject({ phase: 'copying' });
    clones(studio, 2, []); // Remove project
    await fileCopyPass(client, native);
    expect(projectFileCopy(client, 'studio', 'new-0')).toBeNull();
    expect(calls.some(call => call.method === 'projects.writeFile')).toBe(false);
  });

  test('a clone that does not start keeps the dialog open with its error', async () => {
    primaryAt('http://127.0.0.1:16960', 'env');
    const { client, native } = harness();
    await machine('studio', 'Studio');
    await addMachineLocal(client, native, 'open', '', '');
    const failing = { ...native, later: async (input: unknown) => obj(input).method === 'projectClone.start'
      ? { ok: false, generation: 5, error: { kind: 'transport', message: 'Destination exists' } } : native.later(input) } as Native;
    expect(await submitAddMachine(client, failing)).toEqual({ status: null, generation: -1 });
    expect(addMachineView(client)).toMatchObject({ open: true, error: 'Destination exists', submitting: false, canSubmit: true });
    expect(projectFileCopy(client, 'studio', 'new-0')).toBeNull();
    expect(client.local.drafts).toEqual({ 'env:new:p1': 'carry me' });
  });
});

describe('silenced clone toasts (ProjectCloneToastCoordinator)', () => {
  const clone = (over: Obj = {}): Obj => ({ projectId: 'p1', destinationPath: '/work/pr-demo', repository: null, phase: 'running', stage: 'receiving', percent: 5, detail: null, error: null, ...over });
  function fakePort() {
    const log: string[] = []; let next = 1;
    const port: ToastPort = { add: (key, toast: CloneToast) => { log.push(`add ${toast.kind} ${toast.title}`); return next++; }, update: (id, toast) => { log.push(`update ${id} ${toast.title}`); },
      close: id => { log.push(`close ${id}`); }, live: () => true };
    return { port, log };
  }
  test('a silenced clone shows neither progress nor success, but a failure still toasts', () => {
    const { port, log } = fakePort(), tracked = new Map<string, TrackedToast>(), silenced = new Set(['p1']);
    syncCloneToasts(tracked, [clone()], 'env', '', port, silenced);
    syncCloneToasts(tracked, [clone({ phase: 'done' })], 'env', '', port, silenced);
    syncCloneToasts(tracked, [clone({ phase: 'failed', error: 'boom' })], 'env', '', port, silenced);
    expect(log).toEqual(['add error Failed to clone pr-demo']);
  });
});

// AddProjectToMachineDialog.tsx at 7c3b3b3605ef (#22492): a fork's own remote is what the target clones.
test('the target clones a fork\'s own remote, not the upstream it resolved to', () => {
  const fork = { repositoryIdentity: { locator: { remoteUrl: 'https://github.com/upstream/app.git' }, origin: { canonicalKey: 'github.com/me/app', remoteUrl: 'git@github.com:me/app.git' } } };
  expect(projectRemoteUrl(fork)).toBe('git@github.com:me/app.git');
  // An older server sends no origin remote.
  expect(projectRemoteUrl({ repositoryIdentity: { locator: { remoteUrl: 'https://github.com/upstream/app.git' }, origin: { canonicalKey: 'github.com/upstream/app' } } })).toBe('https://github.com/upstream/app.git');
});
