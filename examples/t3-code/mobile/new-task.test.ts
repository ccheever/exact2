import { noteNow } from './shared/composer-controls';
import { liveEvent } from './shared/live-streams';
import { mobileNewTaskCloneObserve } from './new-task-clone';
import { mobileLayoutFacts } from './root-presentation';
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTask } from './new-task';
import { mobileNewTaskFileSnapshot, mobileNewTaskFileRead } from './new-task-file';
import { mobileAppLink } from './navigation-links';
import { arr, obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { draftContext, patchDraftContext } from './shared/composer-controls-branch';
import { branchState } from './shared/r4-git-branch';
import { mobileComposerTarget } from './composer-target';
import { mobileComposerSettings, mobileComposerSettingsAction } from './composer-settings';
import { mobileNewTaskFlowView as view, mobileNewTaskFlowAction as act, mobileNewTaskFlowOwns as owns, mobileNewTaskRoute } from './new-task-flow';
import { mobileNewTaskDraftChoicesUpdate, mobileNewTaskDraftLookup, mobileNewTaskDraftCreate, mobileNewTaskDraftBind } from './mobile-new-task-drafts';
import { mobileNewTaskLaunchPrepare, mobileNewTaskLaunchPendingOwned } from './mobile-new-task-launch';
import { mobileVoiceAction, mobileVoiceBlocksSubmission, mobileVoiceSnapshot } from './voice-data';
import { resetVoiceInputGlobalsForTests } from './voice-controller';
import { contractFns } from '../tests/support/contract-action-fixture';

async function fixture() {
  const client = new MobileDraftClient(), fleet = new EnvironmentFleet(), calls: Obj[] = [];
  const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('not saved'); }, async atomicWriteFile() {} } };
  await client.command('dismiss-error', '', '', 0, { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: {} }; } }, files);
  Object.assign(client, { environmentId: 'env', origin: 'https://example.test', projectId: 'a', connection: 'connected',
    configLive: true, shellLive: true, shellLoaded: true, threadLive: true, scopes: ['orchestration:operate'] });
  noteNow(client, 1791420000000);
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, providers: [] };
  client.shell.projects = [{ id: 'a', title: 'A', workspaceRoot: '/a' }, { id: 'b', title: 'B', workspaceRoot: '/b' }];
  client.local.drafts['env:new:a'] = 'A original'; client.local.drafts['env:new:b'] = 'B original';
  let serial = 0;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'mobileOutbox') return { ok: true, generation: client.generation, value: request.action === 'read'
      ? { ownerEpoch: 'epoch', sequenceFloor: 0, complete: true, errors: [], records: [], outcomes: [], mutations: [], revisions: {}, tokens: {}, transfers: [] }
      : { complete: true, fingerprint: null, claims: [] } };
    const value = request.op === 'ids' ? Array.from({length: Number(request.count)}, () => `id-${++serial}`) : request.op === 'http' ? { authenticated: true, permissions: ['source-control:write'] }
      : request.method === 'vcs.switchRef' ? { refName: 'normalized-branch' } : {};
    return { ok: true, generation: client.generation, value };
  } };
  const snapshot = (location: string, visit = 'visit', session = 'flow', ready = true, groupingMode = 'repository') => view(session, visit, location, true, ready, client, fleet, groupingMode);
  const action = (owner: string, kind: string, id = '', value = '', visit = 'visit') => act(owner, visit, kind, id, value, native, files, client, fleet);
  return { client, fleet, files, native, calls, snapshot, action };
}


for (const choice of [
  { name: 'current checkout', mode: 'local', branch: 'main', origin: false, path: '', label: 'main' },
  { name: 'current checkout with origin enabled', mode: 'local', branch: 'feature/mobile', origin: true, path: '', label: 'feature/mobile' },
  { name: 'existing worktree', mode: 'local', branch: 'feature/mobile', origin: true, path: '/a-worktree', label: 'feature/mobile' },
  { name: 'local worktree base', mode: 'worktree', branch: 'main', origin: false, path: '', label: 'From main' },
  { name: 'origin worktree base', mode: 'worktree', branch: 'main', origin: true, path: '', label: 'From origin/main' },
  { name: 'remote-named local checkout', mode: 'local', branch: 'origin/topic', origin: true, path: '', label: 'origin/topic' },
  { name: 'remote-named worktree base', mode: 'worktree', branch: 'origin/topic', origin: false, path: '', label: 'From origin/topic' },
  { name: 'remote-named origin worktree base', mode: 'worktree', branch: 'origin/topic', origin: true, path: '', label: 'From origin/origin/topic' },
  { name: 'empty local branch', mode: 'local', branch: '', origin: false, path: '', label: 'Choose branch' },
  { name: 'empty worktree base', mode: 'worktree', branch: '', origin: true, path: '', label: 'Choose branch' },
]) {
  test(`New Task branch control labels ${choice.name}`, async () => {
    const f = await fixture(), chooser = f.snapshot('/new');
    expect(await f.action(chooser.owner, 'project', '["env","a"]')).toMatchObject({ message: '' });
    expect(f.client.draftKey.startsWith('new-task:')).toBe(true);
    const context = { envMode: choice.mode, branch: choice.branch, worktreePath: choice.path };
    patchDraftContext(f.client, context);
    branchState(f.client).origin.set(f.client.draftKey, choice.origin);
    const before = { ...draftContext(f.client) }, calls = f.calls.length;
    expect(mobileNewTask('', f.client, f.fleet).branchLabel).toBe(choice.label);
    expect(draftContext(f.client)).toEqual(before);
    expect(f.calls).toHaveLength(calls);
  });
}

test('Choose project never adopts the prior thread or even a sole project automatically', async () => {
  const f = await fixture(); f.client.threadId = 'old-thread'; f.client.shell.projects = f.client.shell.projects.slice(0, 1);
  const chooser = f.snapshot('/new');
  expect(chooser).toMatchObject({ chooser: true, title: 'Choose project', ready: false, needsPrepare: false });
  expect(f.client.threadId).toBe('old-thread'); expect(f.client.local.drafts['env:new:a']).toBe('A original');
  expect(f.calls).toHaveLength(0);
});

for (const source of ['fresh', 'explicit', 'saved']) {
  test(`Change Project returns to the ${source} Draft with its identity and unsent text`, async () => {
    const f = await fixture();
    const independent = mobileNewTaskDraftCreate(f.client, { id: 'independent', environmentId: 'env', projectId: 'b',
      origin: f.client.origin, createdAt: '2026-10-08T00:00:00Z' });
    f.client.local.drafts[independent.key] = 'Another unsent task for B';
    const initial = f.snapshot('/new', 'chooser');
    expect(await f.action(initial.owner, 'project', '["env","a"]', '', 'chooser'))
      .toMatchObject({ message: '', nextLocation: '/new/draft' });
    const key = f.client.draftKey;
    const location = source === 'explicit' ? '/new/draft?environmentId=env&projectId=a'
      : source === 'saved' ? `/new/draft?draftId=${encodeURIComponent(key)}` : '/new/draft';
    let draft = f.snapshot(location, 'draft');
    if (draft.needsPrepare) {
      expect(await f.action(draft.owner, 'prepare', '', '', 'draft')).toMatchObject({ message: '' });
      draft = f.snapshot(location, 'draft');
    }
    expect(draft.ready).toBe(true);
    expect(await f.action(draft.owner, 'draft', draft.draftOwner, 'Unsent task for A', 'draft'))
      .toMatchObject({ message: '' });
    const picker = '/new?draftRoute=draft';
    let calls = f.calls.length;
    const choose = f.snapshot(picker, 'change');
    expect(choose).toMatchObject({ chooser: true, ready: false });
    expect(owns(draft.owner, 'draft', f.client)).toBe(false);
    expect(f.snapshot(location, 'draft')).toMatchObject({ ready: true, needsPrepare: false });
    expect(f.calls).toHaveLength(calls);
    expect(f.client.draft).toBe('Unsent task for A');
    expect(f.client.projectId).toBe('a');

    f.snapshot(picker, 'change-2');
    expect(await f.action(draft.owner, 'project', '["env","b"]', '', 'change-2'))
      .toMatchObject({ message: '', nextLocation: '@back', requestRoute: 'change-2' });
    expect(f.client.draftKey).toBe(key);
    expect(mobileNewTaskDraftLookup(f.client, key)).toMatchObject({ environmentId: 'env', projectId: 'b' });
    expect(f.snapshot(location, 'draft')).toMatchObject({ ready: true, needsPrepare: false });
    expect(f.client.projectId).toBe('b');
    expect(f.client.draft).toBe('Unsent task for A');
    expect(f.client.local.drafts['env:new:a']).toBe('A original');
    expect(f.client.local.drafts['env:new:b']).toBe('B original');
    expect(f.client.local.drafts[independent.key]).toBe('Another unsent task for B');
    expect(mobileNewTaskDraftLookup(f.client, independent.key)?.projectId).toBe('b');
    expect(f.calls.filter(call => call.op === 'ids')).toHaveLength(1);
    expect(f.calls.some(call => call.method === 'orchestration.launchThread')).toBe(false);
    calls = f.calls.length;
    expect((await f.action(draft.owner, 'project', '["env","a"]', '', 'change-2')).message)
      .toContain('route changed');
    expect(f.calls).toHaveLength(calls);
  });
}

test('a project chooser cannot return to an unrelated or replaced Draft request', async () => {
  for (const replaced of [false, true]) {
    const f = await fixture(), chooser = f.snapshot('/new', 'chooser');
    await f.action(chooser.owner, 'project', '["env","a"]', '', 'chooser');
    f.snapshot('/new/draft', 'draft');
    const key = f.client.draftKey;
    if (replaced) f.snapshot('/new/draft?environmentId=env&projectId=b', 'draft');
    const changed = f.snapshot(`/new?draftRoute=${replaced ? 'draft' : 'other'}`, 'change');
    const calls = f.calls.length;
    expect((await f.action(changed.owner, 'project', '["env","b"]', '', 'change')).message)
      .toContain('original draft');
    expect(f.client.projectId).toBe('a');
    expect(f.client.draftKey).toBe(key);
    expect(f.calls).toHaveLength(calls);
  }
});

test('only the original New Task entry presents a sheet; Change Project stays in its stack', () => {
  const fns = contractFns('mobile/mobile-root-state.contract') as Record<string, (...args: unknown[]) => unknown>;
  const presentation = (name: string, route: string, session = 'original', split = false) => {
    const read = (fn: string) => fns[fn]!(name, route, session, split);
    return [read('mobileRouteFormSheet'), read('mobileRoutePresentation'), read('mobileRouteDetent')];
  };
  expect(presentation('newTask', 'original')).toEqual(['new-task', 'modal', 'large']);
  expect(presentation('newTask', 'change-project')).toEqual(['', '', '']);
  expect(presentation('newTaskDraft', 'draft')).toEqual(['', '', '']);
  expect(presentation('newTask', 'original', 'original', true)).toEqual(['', '', '']);
  expect(presentation('newTask', 'change-project', 'original', true)).toEqual(['', '', '']);
  expect(presentation('settings', 'settings')).toEqual(['settings', 'modal', '']);
  expect(presentation('settings', 'settings', 'original', true)).toEqual(['', '', '']);
  for (const name of ['threadModel', 'newTaskModel']) expect(presentation(name, name)).toEqual(['composer', 'modal', 'large']);
  expect(presentation('threadQueue', 'queue')).toEqual(['', 'modal', 'medium large']);
  for (const name of ['connect', 'connections']) expect(presentation(name, name)).toEqual(['true', 'modal', '']);
  for (const name of ['gitOverview', 'threadTerminalCapture']) expect(presentation(name, name)).toEqual(['', 'modal', '']);
  for (const name of ['homeArrange', 'threadBrowser', 'threadDevices', 'settingsLegal']) expect(presentation(name, name)).toEqual(['', 'fullscreen', '']);
});

test('New Task replies pop a retained chooser and preserve other navigation outcomes', () => {
  const fns = contractFns('mobile/mobile-root-state.contract') as Record<string, (...args: unknown[]) => unknown>;
  const navigation = (operation: string, nextLocation = '', message = '', submitted = false) =>
    fns.taskReplyNavigation!({ revision: 1, requestRoute: 'chooser', environmentId: 'env', projectId: 'a', threadId: '',
      alertTitle: '', nextLocation, message, submitted }, operation);
  expect(navigation('project', '@back')).toBe('back');
  expect(navigation('scratch', '@back')).toBe('back');
  expect(navigation('project', '/new/draft')).toBe('push');
  expect(navigation('project', '@back', 'Choose an available project')).toBe('');
  expect(navigation('environment')).toBe('back');
  expect(navigation('branch')).toBe('back');
  expect(navigation('send', '', '', true)).toBe('thread');
  expect(navigation('pending-close', '@close')).toBe('close');
  expect(navigation('clone-remove', '/')).toBe('home');
  expect(navigation('draft')).toBe('');
});

test('bare draft selects the sole project through a fresh independent identity', async () => {
  const f = await fixture(); f.client.threadId = 'old-thread'; f.client.shell.projects = f.client.shell.projects.slice(0, 1);
  const draft = f.snapshot('/new/draft');
  expect(draft).toMatchObject({ status: 'prepare', needsPrepare: true, ready: false, nextLocation: '' });
  expect(f.calls).toHaveLength(0); expect(f.client.threadId).toBe('old-thread');
  expect(await f.action(draft.owner, 'prepare')).toMatchObject({ message: '', nextLocation: '' });
  expect(f.snapshot('/new/draft')).toMatchObject({ ready: true, needsPrepare: false });
  const first = f.client.draftKey;
  expect(first).toBe('new-task:id-1'); expect(f.client.draft).toBe(''); expect(f.client.threadId).toBe('');
  expect(f.client.local.drafts['env:new:a']).toBe('A original');
  await f.action(draft.owner, 'prepare'); expect(f.client.draftKey).toBe(first);
  const next = f.snapshot('/new/draft', 'next', 'other-flow');
  expect(next.needsPrepare).toBe(true);
  expect(await f.action(next.owner, 'prepare', '', '', 'next')).toMatchObject({ message: '' });
  expect(f.client.draftKey).toBe('new-task:id-2');
  expect(f.calls.filter(call => call.op === 'ids')).toHaveLength(2);
});

test('bare draft waits for catalog and grouping preferences before selecting', async () => {
  const f = await fixture(); f.client.shell.projects = f.client.shell.projects.slice(0, 1);
  expect(f.snapshot('/new/draft', 'visit', 'flow', false)).toMatchObject({ status: 'loading', needsPrepare: false, nextLocation: '' });
  expect(f.snapshot('/new/draft', 'visit', 'flow', true, '')).toMatchObject({ status: 'loading', needsPrepare: false, nextLocation: '' });
  expect(f.calls).toHaveLength(0);
  const ready = f.snapshot('/new/draft'); expect(ready.needsPrepare).toBe(true);
  expect(await f.action(ready.owner, 'prepare')).toMatchObject({ message: '' });
  expect(f.snapshot('/new/draft').ready).toBe(true);
});

test('bare draft counts logical project groups and uses their representative', async () => {
  for (const mode of ['repository', 'separate']) {
    const f = await fixture();
    for (const project of f.client.shell.projects) project.repositoryIdentity = { canonicalKey: 'same-repository' };
    const draft = f.snapshot('/new/draft', 'visit', 'flow', true, mode);
    if (mode === 'repository') {
      expect(draft).toMatchObject({ needsPrepare: true, nextLocation: '' });
      expect(await f.action(draft.owner, 'prepare')).toMatchObject({ message: '' });
      expect(f.client.projectId).toBe('a'); expect(f.client.draftKey).toBe('new-task:id-1');
    } else {
      expect(draft).toMatchObject({ needsPrepare: false, nextLocation: '/new', ready: false });
      expect(f.calls).toHaveLength(0);
    }
  }
  const multiple = await fixture();
  expect(multiple.snapshot('/new/draft')).toMatchObject({ needsPrepare: false, nextLocation: '/new', ready: false });
  const empty = await fixture(); empty.client.shell.projects = [];
  expect(empty.snapshot('/new/draft')).toMatchObject({ needsPrepare: false, nextLocation: '/new', ready: false });
});

test('bare draft selection never repairs explicit or partial query identities', async () => {
  const f = await fixture(); f.client.shell.projects = f.client.shell.projects.slice(0, 1);
  for (const query of ['environmentId=env&projectId=missing', 'projectId=missing', 'environmentId=env', 'projectId=', 'draftId=missing', 'branch=main', 'worktreePath=%2Fwork']) {
    expect(f.snapshot(`/new/draft?${query}`)).toMatchObject({ needsPrepare: false, nextLocation: '/new', ready: false });
  }
  expect(f.calls).toHaveLength(0); expect(f.client.local.drafts['env:new:a']).toBe('A original');
});

test('bare draft selection rechecks the sole project while allocating its identity', async () => {
  const f = await fixture(); f.client.shell.projects = f.client.shell.projects.slice(0, 1);
  let finish!: (value: unknown) => void, entered!: () => void;
  const started = new Promise<void>(resolve => { entered = resolve; }), original = f.native.later;
  f.native.later = async input => obj(input).op === 'ids'
    ? (entered(), await new Promise(resolve => { finish = resolve; })) : original(input);
  const draft = f.snapshot('/new/draft'), preparing = f.action(draft.owner, 'prepare');
  await started; f.client.shell.projects.push({ id: 'b', title: 'B', workspaceRoot: '/b' });
  finish({ ok: true, generation: f.client.generation, value: ['stale-id'] });
  await expect(preparing).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.local.drafts['new-task:stale-id']).toBeUndefined();
  expect(f.snapshot('/new/draft')).toMatchObject({ needsPrepare: false, ready: false, nextLocation: '/new' });
});

test('explicit project selection keeps existing shared drafts and enables only that flow', async () => {
  const f = await fixture(), chooser = f.snapshot('/new');
  expect(await f.action(chooser.owner, 'project', '["env","b"]')).toMatchObject({ message: '', nextLocation: '/new/draft', submitted: false });
  const draft = f.snapshot('/new/draft', 'draft');
  expect(draft.ready).toBe(true); expect(owns(draft.owner, 'draft', f.client)).toBe(true);
  expect(f.client.projectId).toBe('b'); expect(f.client.draft).toBe(''); expect(f.client.local.drafts['env:new:a']).toBe('A original');
  expect(f.snapshot('/new/draft', 'fresh', 'other-flow')).toMatchObject({ ready: false, nextLocation: '/new' });
});

test('settings preparation retains presentation owner without admitting stale or unready draft actions', async () => {
  const f = await fixture(), chooser = f.snapshot('/new');
  expect(chooser.draftOwner).toBe('');
  await f.action(chooser.owner, 'project', '["env","a"]');
  const draft = f.snapshot('/new/draft', 'draft'), contentOwner = mobileComposerTarget(f.client).owner;
  expect(draft).toMatchObject({ ready: true, draftOwner: contentOwner });
  expect(contentOwner).not.toBe('');
  let writes = 0; f.files.fs.atomicWriteFile = async () => { writes++; };
  for (const visit of ['model-1', 'model-2']) {
    const opening = f.snapshot('/new/draft/settings', visit);
    expect(opening).toMatchObject({ ready: false, needsPrepare: true, draftOwner: contentOwner });
    expect(owns(opening.owner, 'draft', f.client)).toBe(false);
    expect(owns(opening.owner, visit, f.client)).toBe(false);
    expect((await f.action(opening.owner, 'draft', '', 'stale', 'draft')).message).toContain('route changed');
    expect((await f.action(opening.owner, 'draft', '', 'unready', visit)).message).toContain('ready');
    expect(writes).toBe(0);
    expect(await f.action(opening.owner, 'prepare', '', '', visit)).toMatchObject({ message: '' });
    expect(f.snapshot('/new/draft/settings', visit)).toMatchObject({ ready: true, draftOwner: contentOwner });
    expect(owns(opening.owner, visit, f.client)).toBe(true);
    const nested = f.snapshot('/new/draft/settings/runtime', `${visit}-runtime`);
    expect(nested).toMatchObject({ ready: true, draftOwner: contentOwner });
    expect(owns(nested.owner, visit, f.client)).toBe(false);
    expect(f.snapshot('/new/draft/settings', visit)).toMatchObject({ ready: true, draftOwner: contentOwner });
    await mobileComposerSettingsAction('cancel', '', '', f.native, f.files, f.client);
    expect(f.snapshot('/new/draft', 'draft')).toMatchObject({ ready: true, draftOwner: contentOwner });
    expect(owns(draft.owner, 'draft', f.client)).toBe(true);
  }
  expect(f.client.local.drafts['env:new:a']).toBe('A original');
  expect(f.client.local.drafts['env:new:b']).toBe('B original'); expect(writes).toBe(0);
});

test('prepared New Task unchanged Done remains a write-free dismissal after disconnect', async () => {
  const f = await fixture(), chooser = f.snapshot('/new');
  await f.action(chooser.owner, 'project', '["env","a"]');
  const draft = f.snapshot('/new/draft', 'draft'), contentOwner = draft.draftOwner;
  const opening = f.snapshot('/new/draft/settings', 'model');
  expect(await f.action(opening.owner, 'prepare', '', '', 'model')).toMatchObject({ message: '' });
  expect(f.snapshot('/new/draft/settings', 'model')).toMatchObject({ ready: true, draftOwner: contentOwner });
  f.client.connection = 'disconnected'; f.client.scopes = [];
  expect(f.snapshot('/new/draft/settings', 'model', 'flow', false)).toMatchObject({ ready: true, draftOwner: contentOwner });
  expect(owns(opening.owner, 'model', f.client)).toBe(true);
  const draftText = f.client.draft;
  const calls = f.calls.length; let fileCalls = 0;
  f.files.fs.mkdir = async () => { fileCalls++; };
  f.files.fs.readFile = async () => { fileCalls++; throw new Error('No read is needed'); };
  f.files.fs.atomicWriteFile = async () => { fileCalls++; };
  expect(mobileComposerSettings('', '', false, f.client)).toMatchObject({ open: true, canEdit: false, canSave: true, saveLabel: 'Done' });
  expect(await mobileComposerSettingsAction('save', '', '', f.native, f.files, f.client)).toMatchObject({ closed: true, message: '' });
  expect(mobileComposerSettings('', '', false, f.client).open).toBe(false);
  expect(f.client.draft).toBe(draftText);
  expect(f.client.local.drafts['env:new:a']).toBe('A original'); expect(f.client.local.drafts['env:new:b']).toBe('B original');
  expect(f.calls).toHaveLength(calls); expect(fileCalls).toBe(0);
});

test('presentation owner disappears when selected draft proof is lost', async () => {
  const changes: Array<(client: T3Client) => void> = [
    client => { client.projectId = 'b'; }, client => { client.environmentId = 'elsewhere'; },
    client => { client.generation++; }, client => { client.threadEpoch++; },
    client => { client.threadId = 'thread'; }, client => { client.shell.projects = []; },
    client => { client.shell.projects[0].archivedAt = '2026-10-07T00:00:00Z'; },
  ];
  for (const change of changes) {
    const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
    const draft = f.snapshot('/new/draft', 'draft'); expect(draft.draftOwner).not.toBe('');
    change(f.client);
    expect(f.snapshot('/new/draft/settings', 'model')).toMatchObject({ draftOwner: '', ready: false });
    expect(owns(draft.owner, 'draft', f.client)).toBe(false);
    expect((await f.action(draft.owner, 'draft', '', 'blocked', 'model')).message).toContain('ready');
    expect(f.client.local.drafts['env:new:a']).toBe('A original');
  }
});

test('inactive, replacement and unselected flows never inherit a presentation owner', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const draft = f.snapshot('/new/draft', 'draft'); expect(draft.draftOwner).not.toBe('');
  expect(view('flow', 'gone', '/', false, true, f.client, f.fleet)).toMatchObject({ draftOwner: '', ready: false });
  expect(owns(draft.owner, 'draft', f.client)).toBe(false);
  expect(f.snapshot('/new/draft', 'returned')).toMatchObject({ draftOwner: '', ready: false });
  const replacement = f.snapshot('/new/draft/settings', 'model', 'replacement');
  expect(replacement).toMatchObject({ draftOwner: '', ready: false });
  expect(owns(replacement.owner, 'model', f.client)).toBe(false);
  const unloaded = new T3Client();
  expect(view('unloaded', 'draft', '/new/draft', true, true, unloaded, f.fleet)).toMatchObject({ status: 'loading', draftOwner: '', ready: false });
});

test('direct context query initializes the actual project once and does not undo later picks', async () => {
  const f = await fixture(), url = '/new/draft/environment?environmentId=env&projectId=b';
  const first = f.snapshot(url); expect(first.needsPrepare).toBe(true);
  expect(await f.action(first.owner, 'prepare')).toMatchObject({ message: '', nextLocation: '' });
  expect(f.snapshot(url)).toMatchObject({ ready: true, title: 'Environment', needsPrepare: false });
  expect(await f.action(first.owner, 'project', '["env","a"]')).toMatchObject({ message: '' });
  expect(f.snapshot(url).ready).toBe(true); expect(f.client.projectId).toBe('a');
  f.snapshot('/new/draft/branch', 'child');
  expect(f.snapshot(url).needsPrepare).toBe(false); expect(f.client.projectId).toBe('a');
  expect(f.snapshot('/new/draft/environment?environmentId=env&projectId=a').needsPrepare).toBe(true);
});

test('unknown direct identity waits for catalog, then returns chooser without selecting a fallback', async () => {
  const f = await fixture(), url = '/new/draft?environmentId=env&projectId=deleted';
  expect(f.snapshot(url, 'visit', 'flow', false).status).toBe('loading');
  expect(f.snapshot(url)).toMatchObject({ status: 'pick', ready: false, nextLocation: '/new' });
  expect(f.client.projectId).toBe('a'); expect(f.calls).toHaveLength(0);
  expect(f.snapshot('/new/draft/branch?projectId=b')).toMatchObject({ status: 'pick', nextLocation: '/new' });
});

test('unknown draft and unavailable share identity never fall through to an existing project', async () => {
  const f = await fixture();
  for (const key of ['draftId', 'incomingShareId', 'cloning']) {
    expect(f.snapshot(`/new/draft?environmentId=env&projectId=b&${key}=real-id`)).toMatchObject({ status: 'pick', ready: false, needsPrepare: false, nextLocation: '/new' });
  }
  const pendingURL = '/new/draft?environmentId=env&projectId=b&pendingTaskId=missing';
  const pending = f.snapshot(pendingURL);
  expect(pending).toMatchObject({ ready: false, needsPrepare: true, pendingEditor: true });
  expect((await f.action(pending.owner, 'prepare')).message).not.toBe('');
  expect(f.snapshot(pendingURL)).toMatchObject({ ready: false, needsPrepare: false, status: 'retained' });
  expect(f.client.projectId).toBe('a'); expect(mobileNewTaskRoute('/new/draft?environmentId=one&environmentId=two&projectId=a').environmentId).toBe('one');
});

test('branch links dispatch real checkout before accepting normalized branch context', async () => {
  const f = await fixture(), flow = f.snapshot('/new/draft?environmentId=env&projectId=b&branch=feature');
  expect(await f.action(flow.owner, 'prepare')).toMatchObject({ message: '' });
  expect(f.calls.find(call => call.method === 'vcs.switchRef')?.payload).toEqual({ cwd: '/b', refName: 'feature' });
  expect(draftContext(f.client)).toEqual({ envMode: 'local', branch: 'normalized-branch', worktreePath: '' });
  expect(f.client.draft).toBe(''); expect(f.snapshot('/new/draft?environmentId=env&projectId=b&branch=feature').ready).toBe(true);
});

test('checkout permission loss refuses the draft and preserves its previous context', async () => {
  const f = await fixture(), original = f.native.later;
  f.native.later = async input => obj(input).op === 'http' ? { ok: true, generation: f.client.generation, value: { authenticated: true, permissions: [] } } : original(input);
  const url = '/new/draft?environmentId=env&projectId=b&branch=feature', flow = f.snapshot(url);
  expect((await f.action(flow.owner, 'prepare')).message).toContain('cannot check out');
  expect(f.calls.some(call => call.method === 'vcs.switchRef')).toBe(false);
  expect(draftContext(f.client).branch).toBe(''); expect(f.snapshot(url).ready).toBe(false);
});

test('leaving during selection prevents late flow adoption without erasing any draft', async () => {
  const f = await fixture(); let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  const original = f.native.later;
  f.native.later = async input => { if (obj(input).op === 'unsubscribe') { entered(); await gate; } return original(input); };
  const first = f.snapshot('/new'), pending = f.action(first.owner, 'project', '["env","b"]');
  await started; view('flow', 'gone', '/', false, true, f.client, f.fleet); release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot('/new/draft', 'returned').ready).toBe(false);
  expect(f.client.local.drafts['env:new:a']).toBe('A original'); expect(f.client.local.drafts['env:new:b']).toBe('B original');
});

test('a dispatched checkout blocks another flow until it settles and cannot patch a replacement draft', async () => {
  const f = await fixture(); let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  const original = f.native.later;
  f.native.later = async input => { if (obj(input).method === 'vcs.switchRef') { entered(); await gate; } return original(input); };
  const first = f.snapshot('/new/draft?environmentId=env&projectId=b&branch=feature');
  const pending = f.action(first.owner, 'prepare'); await started;
  const replacement = f.snapshot('/new/draft?environmentId=env&projectId=a', 'new', 'other-flow');
  expect(replacement.busy).toBe(true); expect(replacement.needsPrepare).toBe(false);
  expect((await act(replacement.owner, 'new', 'prepare', '', '', f.native, f.files, f.client, f.fleet)).message).toContain('Wait');
  release(); await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(draftContext(f.client).branch).toBe('');
  expect(f.snapshot('/new/draft?environmentId=env&projectId=a', 'new', 'other-flow').needsPrepare).toBe(true);
});

test('draft edits retain their captured project while persistence overlaps and ownership changes', async () => {
  const f = await fixture(), first = f.snapshot('/new'); await f.action(first.owner, 'project', '["env","a"]');
  const draft = f.snapshot('/new/draft'), key = f.client.draftKey; let release!: () => void, entered!: () => void, writes = 0;
  const started = new Promise<void>(resolve => { entered = resolve; }), gate = new Promise<void>(resolve => { release = resolve; });
  f.files.fs.atomicWriteFile = async () => { if (++writes === 1) { entered(); await gate; } };
  const edit = f.action(draft.owner, 'draft', '', 'A changed'); await started;
  const latest = f.action(draft.owner, 'draft', '', 'A latest'); await latest;
  f.client.projectId = 'b'; release(); await edit;
  expect(f.client.local.drafts[key]).toBe('A latest'); expect(f.client.local.drafts['env:new:a']).toBe('A original'); expect(f.client.draft).toBe('A latest');
  expect(owns(draft.owner, 'visit', f.client)).toBe(false);
});

test('a same-route external selection while unsubscribe is pending cannot become the clicked project flow', async () => {
  const f = await fixture(); let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  const original = f.native.later;
  f.native.later = async input => { if (obj(input).op === 'unsubscribe') { entered(); await gate; } return original(input); };
  const first = f.snapshot('/new'), pending = f.action(first.owner, 'project', '["env","b"]'); await started;
  f.client.projectId = 'a'; f.client.threadEpoch++; release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot('/new/draft').ready).toBe(false); expect(f.client.draft).toBe('A original');
});

test('attachment and settings child routes preserve the chosen draft owner', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  for (const path of ['/new/draft/attachments/file-id', '/new/draft/files/src%2Fa.ts']) {
    const child = f.snapshot(path, path); expect(child.ready).toBe(true); expect(owns(child.owner, path, f.client)).toBe(true);
  }
});

import { mobileNewTaskChooser } from './new-task';
test('Choose project reuses repository grouping, searches member paths and picks a real preferred-environment member', async () => {
  const f = await fixture();
  const repositoryIdentity = { canonicalKey: 'github.com/acme/repo', name: 'repo', displayName: 'acme/repo', rootPath: '/a' };
  f.client.shell.projects = [{ id: 'a', title: 'repo', workspaceRoot: '/a', repositoryIdentity },
    { id: 'b', title: 'repo', workspaceRoot: '/checkout/work', repositoryIdentity }];
  const grouped = mobileNewTaskChooser('checkout', 'repository', f.client, f.fleet);
  expect(grouped.projects).toHaveLength(1);
  expect(grouped.projects[0]).toMatchObject({ id: '["env","a"]', title: 'acme/repo', subtitle: '2 workspaces',
    favicon: '', faviconTarget: { environmentId: 'env', cwd: '/a', faviconPath: '', key: '["env","/a",null]' } });
  expect(f.calls).toHaveLength(0);
  f.client.shell.projects[0]!.projectIcon = { kind: 'emoji', emoji: '🌲' };
  expect(mobileNewTaskChooser('', 'repository', f.client, f.fleet).projects[0]).toMatchObject({ id: '["env","a"]', iconKind: 'emoji', iconText: '🌲', faviconTarget: { key: '' } });
  expect(mobileNewTaskChooser('', 'separate', f.client, f.fleet).projects).toHaveLength(2);
  expect(mobileNewTaskChooser('absent', 'repository', f.client, f.fleet)).toMatchObject({ projects: [], emptyTitle: 'No matching projects' });
});


test('unprepared explicit identity and direct settings children cannot operate the previous draft', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const draft = f.snapshot('/new/draft'); expect(owns(draft.owner, 'visit', f.client)).toBe(true);
  const changed = f.snapshot('/new/draft?environmentId=env&projectId=b');
  expect(changed.needsPrepare).toBe(true); expect(changed.draftOwner).toBe(draft.draftOwner); expect(owns(changed.owner, 'visit', f.client)).toBe(false);
  expect((await f.action(changed.owner, 'draft', '', 'wrong owner')).message).toContain('ready');
  expect(f.client.local.drafts['env:new:a']).toBe('A original');
  const settings = f.snapshot('/new/draft/settings/runtime', 'runtime');
  expect(settings.needsPrepare).toBe(true); expect(settings.draftOwner).toBe(draft.draftOwner); expect(owns(settings.owner, 'runtime', f.client)).toBe(false);
  await f.action(settings.owner, 'prepare', '', '', 'runtime');
  expect(f.snapshot('/new/draft/settings/runtime', 'runtime').ready).toBe(true);
});

// No project uses the real shared ensureScratch + shell decoder, never a synthetic chooser row.
import { mobileNewTask, mobileNewTaskChooser, mobileNewTaskPrepare } from './new-task';
import { mobileScratchTarget, mobileOpenScratch, mobileMoveScratch } from './new-task-scratch';
import { patchDraftContext } from './shared/composer-controls-branch';
function scratchNative(f: Awaited<ReturnType<typeof fixture>>, options: { grant?: boolean; missing?: boolean } = {}) {
  f.client.config.scratchWorkspaceRoot = '/scratch';
  const original = f.native.later;
  f.native.later = async input => {
    const request = obj(input);
    if (request.path === '/api/auth/session') { f.calls.push(request); return { ok: true, generation: f.client.generation,
      value: { authenticated: true, permissions: options.grant === false ? [] : ['orchestration:operate'] } }; }
    if (request.method === 'projects.ensureScratch') { f.calls.push(request); return { ok: true, generation: f.client.generation, value: { projectId: 'scratch' } }; }
    if (request.path === '/api/orchestration/shell') { f.calls.push(request); return { ok: true, generation: f.client.generation,
      value: { snapshotSequence: 1, projects: [...f.client.shell.projects, ...(options.missing ? [] : [{ id: 'scratch', title: 'Scratch', workspaceRoot: '/scratch' }])], threads: [] } }; }
    return original(input);
  };
}

test('No project uses actual shared creation and shell admission; previous and scratch drafts survive', async () => {
  const f = await fixture(); scratchNative(f);
  f.client.local.drafts['env:new:scratch'] = 'Existing scratch';
  patchDraftContext(f.client, { envMode: 'worktree', branch: 'stale', worktreePath: '/old' }, 'env:new:scratch');
  const chooser = f.snapshot('/new'), data = mobileNewTaskChooser('', 'repository', f.client, f.fleet);
  expect(data.canStartScratch).toBe(true); expect(data.projects.map(row => row.projectId)).toEqual(['a', 'b']);
  expect(await f.action(chooser.owner, 'scratch', data.scratchTarget)).toMatchObject({ nextLocation: '/new/draft', message: '', projectId: 'scratch' });
  expect(f.snapshot('/new/draft', 'draft').ready).toBe(true); expect(f.client.draft).toBe('');
  expect(f.client.local.drafts['env:new:a']).toBe('A original');
  expect(draftContext(f.client)).toEqual({ envMode: 'local', branch: '', worktreePath: '' });
  expect(f.calls.filter(call => call.method === 'projects.ensureScratch')).toHaveLength(1);
  expect(f.calls.filter(call => call.op === 'timelineSleep')).toHaveLength(1);
  expect(f.calls.some(call => call.path === '/api/orchestration/shell')).toBe(true);
  expect(mobileNewTaskChooser('', 'repository', f.client, f.fleet).projects.some(row => row.projectId === 'scratch')).toBe(false);
  const before = f.calls.length; await mobileNewTaskPrepare('', f.native, f.client); expect(f.calls.length).toBe(before);
  for (const kind of ['workspace', 'origin', 'branch']) expect((await f.action(chooser.owner, kind, '', 'worktree', 'draft')).message).toContain('without a project');
});

test('scratch eligibility excludes reconnecting cached shells and permission loss writes nothing', async () => {
  const f = await fixture(); scratchNative(f, { grant: false });
  const chooser = f.snapshot('/new'), key = mobileScratchTarget(f.client, f.fleet);
  f.client.connection = 'reconnecting'; expect(mobileScratchTarget(f.client, f.fleet)).toBe('');
  expect((await f.action(chooser.owner, 'scratch', key)).message).toContain('no longer available');
  f.client.connection = 'connected';
  expect((await f.action(chooser.owner, 'scratch', key)).message).toContain('cannot create');
  expect(f.calls.some(call => call.method === 'projects.ensureScratch')).toBe(false); expect(f.client.projectId).toBe('a');
});

test('missing shell project is bounded, does not select a guessed project, and retry is explicit', async () => {
  const f = await fixture(); scratchNative(f, { missing: true });
  const chooser = f.snapshot('/new');
  expect((await f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet))).message).toContain('has not reached');
  expect(f.calls.filter(call => call.op === 'timelineSleep')).toHaveLength(20);
  expect(f.calls.filter(call => call.method === 'projects.ensureScratch')).toHaveLength(1); expect(f.client.projectId).toBe('a');
});

test('leaving during ensureScratch never selects late project or starts another request', async () => {
  const f = await fixture(); scratchNative(f); const original = f.native.later;
  let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  f.native.later = async input => { if (obj(input).method === 'projects.ensureScratch') { entered(); await gate; } return original(input); };
  const chooser = f.snapshot('/new'), pending = f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet));
  await started;
  expect((await f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet))).message).toContain('Wait');
  view('flow', 'gone', '/', false, true, f.client, f.fleet); release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.projectId).toBe('a'); expect(f.calls.some(call => call.path === '/api/orchestration/shell')).toBe(false);
});

test('an aborted scratch polling answer stays superseded despite shared optional-wait catches', async () => {
  const f = await fixture(); scratchNative(f); const original = f.native.later;
  f.native.later = async input => { if (obj(input).op === 'timelineSleep') throw { name: 'FetchError', kind: 'Aborted' }; return original(input); };
  const chooser = f.snapshot('/new');
  await expect(f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet))).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.projectId).toBe('a'); expect(f.calls.some(call => call.path === '/api/orchestration/shell')).toBe(false);
});

test('scratch explicit links ignore obsolete worktree settings and preserve the saved draft', async () => {
  const f = await fixture(); scratchNative(f);
  f.client.shell.projects.push({ id: 'scratch', title: 'Scratch', workspaceRoot: '/scratch' });
  f.client.local.drafts['env:new:scratch'] = 'Keep me';
  const url = '/new/draft?environmentId=env&projectId=scratch&branch=old&worktreePath=%2Fold', flow = f.snapshot(url);
  expect((await f.action(flow.owner, 'prepare')).message).toBe('');
  expect(f.snapshot(url).ready).toBe(true); expect(f.client.draft).toBe('');
  expect(draftContext(f.client)).toEqual({ envMode: 'local', branch: '', worktreePath: '' });
  expect(f.calls.some(call => call.method === 'vcs.switchRef')).toBe(false);
});

test('remote scratch addresses actual fleet generation and adopts only that shell', async () => {
  const f = await fixture();
  const entry = { key: 'remote-key', origin: 'https://remote.test', environmentId: 'remote', phase: 'connected' as const,
    message: '', traceId: '', generation: 7, synchronized: 7, lastEvent: 0, subscriptions: {},
    config: { scratchWorkspaceRoot: '/remote-scratch' }, shell: { ...f.client.shell, projects: [] }, scopes: [], error: '', requested: true };
  f.fleet.entries.set(entry.key, entry);
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); f.calls.push(request);
    const value = request.path === '/api/auth/session' ? { authenticated: true, permissions: ['orchestration:operate'] }
      : request.method === 'projects.ensureScratch' ? { projectId: 'remote-scratch' }
        : request.path === '/api/orchestration/shell' ? { snapshotSequence: 1, projects: [{ id: 'remote-scratch', workspaceRoot: '/remote-scratch' }], threads: [] } : {};
    return { ok: true, generation: request.generation ?? 0, value };
  } };
  const target = mobileScratchTarget(f.client, f.fleet);
  expect(await mobileOpenScratch(target, native, f.client, f.fleet, () => true)).toEqual({ environmentId: 'remote', projectId: 'remote-scratch' });
  for (const call of f.calls.filter(call => call.op === 'http' || call.op === 'request')) expect(call).toMatchObject({ fleet: 'remote-key', generation: 7 });
  expect(entry.shell.projects[0]?.id).toBe('remote-scratch'); expect(f.client.projectId).toBe('a');
  expect(f.client.shell.projects.some(project => project.id === 'remote-scratch')).toBe(false);
});

test('changing selected project while scratch authorization waits prevents the create write', async () => {
  const f = await fixture(); scratchNative(f); const original = f.native.later;
  let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  f.native.later = async input => { if (obj(input).path === '/api/auth/session') { entered(); await gate; } return original(input); };
  const chooser = f.snapshot('/new'), pending = f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet));
  await started; f.client.projectId = 'b'; release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.some(call => call.method === 'projects.ensureScratch')).toBe(false);
  expect(f.client.projectId).toBe('b'); expect(f.client.draft).toBe('B original');
});

test('scratch branch routes redirect and scratch environment changes cannot strand attachment owners', async () => {
  const f = await fixture(); scratchNative(f); const chooser = f.snapshot('/new');
  await f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet));
  expect(f.snapshot('/new/draft/branch', 'branch')).toMatchObject({ ready: false, nextLocation: '/new/draft' });
  f.snapshot('/new/draft/environment', 'environment');
  f.client.local.snapshotDrafts[f.client.draftKey] = [{ id: 'owned-file', name: 'keep.txt', type: 'file' }];
  const before = f.calls.length;
  expect((await f.action(chooser.owner, 'environment', 'other', '', 'environment')).message).toContain('no longer available');
  expect(f.calls.slice(before).map(call => [call.op, call.action])).toEqual([['mobileOutbox', 'read'], ['mobileOutbox', 'transferLookup']]);
  expect(f.client.snapshotDrafts[0]?.id).toBe('owned-file');
});

test('scratch Run on attempts the actual connection without treating legacy content as a collision', async () => {
  const f = await fixture(); scratchNative(f); const chooser = f.snapshot('/new');
  await f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet));
  f.client.local.drafts[f.client.draftKey] = 'Source draft';
  const entry = { key: 'remote-key', origin: 'https://remote.test', environmentId: 'remote', phase: 'connected' as const,
    message: '', traceId: '', generation: 7, synchronized: 7, lastEvent: 0, subscriptions: {}, config: { scratchWorkspaceRoot: '/remote-scratch' },
    shell: { ...f.client.shell, projects: [{ id: 'remote-scratch', workspaceRoot: '/remote-scratch' }] }, scopes: [], error: '', requested: true };
  f.fleet.entries.set(entry.key, entry); f.client.local.drafts['remote:new:remote-scratch'] = 'Destination draft';
  const original = f.native.later;
  f.native.later = async input => {
    const request = obj(input);
    if (request.fleet === 'remote-key') { f.calls.push(request); return { ok: true, generation: 7,
      value: request.path === '/api/auth/session' ? { authenticated: true, permissions: ['orchestration:operate'] } : { projectId: 'remote-scratch' } }; }
    return original(input);
  };
  f.snapshot('/new/draft/environment', 'environment');
  expect((await f.action(chooser.owner, 'environment', 'remote', '', 'environment')).message).not.toContain('already has a saved draft');
  expect(f.calls.some(call => call.method === 'projects.ensureScratch' && call.fleet === 'remote-key')).toBe(true);
  expect(f.calls.some(call => call.op === 'connect')).toBe(true);
  expect(f.client.draft).toBe('Source draft'); expect(f.client.local.drafts['remote:new:remote-scratch']).toBe('Destination draft');
  expect(f.client.environmentId).toBe('env');
});

test('scratch shared move reduces before any command preamble and restores on route loss during fleet stop', async () => {
  const f = await fixture(); scratchNative(f);
  f.client.projectId = 'scratch'; f.client.shell.projects.push({ id: 'scratch', workspaceRoot: '/scratch' });
  f.client.local.drafts[f.client.draftKey] = 'Keep source';
  const entry = { key: 'remote-key', origin: 'https://remote.test', environmentId: 'remote', phase: 'connected' as const,
    message: '', traceId: '', generation: 7, synchronized: 7, lastEvent: 0, subscriptions: {}, config: { scratchWorkspaceRoot: '/remote-scratch' },
    shell: { ...f.client.shell, projects: [{ id: 'remote-scratch', workspaceRoot: '/remote-scratch' }] }, scopes: [], error: '', requested: true };
  f.fleet.entries.set(entry.key, entry);
  let active = true, release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); f.calls.push(request); if (request.op === 'fleetStop') await gate;
    return { ok: true, generation: f.client.generation, value: {} };
  } };
  const pending = mobileMoveScratch('remote', native, f.client, f.fleet, () => active);
  expect(f.client.local.drafts['remote:new:remote-scratch']).toBe('Keep source');
  expect(f.client.local.drafts['env:new:scratch']).toBeUndefined();
  expect(f.calls.map(call => call.op)).toEqual(['fleetStop']);
  active = false; release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.some(call => call.op === 'connect')).toBe(false);
  expect(f.client.local.drafts['env:new:scratch']).toBe('Keep source');
  expect(f.client.local.drafts['remote:new:remote-scratch']).toBeUndefined();
});

async function scratchRollbackFixture(connectFailure = false) {
  const f = await fixture(); scratchNative(f);
  f.client.projectId = 'scratch'; f.client.shell.projects.push({ id: 'scratch', workspaceRoot: '/scratch' });
  const from = 'env:new:scratch', to = 'remote:new:remote-scratch';
  f.client.local.drafts[from] = 'Original source';
  const context = { envMode: 'local', branch: '', worktreePath: '' };
  f.client.local.composerControls.contexts[from] = context;
  const entry = { key: 'remote-key', origin: 'https://remote.test', environmentId: 'remote', phase: 'connected' as const,
    message: '', traceId: '', generation: 7, synchronized: 7, lastEvent: 0, subscriptions: {}, config: { scratchWorkspaceRoot: '/remote-scratch' },
    shell: { ...f.client.shell, projects: [{ id: 'remote-scratch', workspaceRoot: '/remote-scratch' }] }, scopes: [], error: '', requested: true };
  f.fleet.entries.set(entry.key, entry);
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); f.calls.push(request); if (request.op === 'fleetStop') await gate;
    if (request.op === 'connect' && connectFailure) return { ok: false, generation: f.client.generation, error: { kind: 'denied', message: 'Connect refused' } };
    return { ok: true, generation: f.client.generation, value: {} };
  } };
  const pending = mobileMoveScratch('remote', native, f.client, f.fleet, () => true);
  return { ...f, from, to, context, pending, release };
}

test('scratch rollback restores captured slots when another project becomes selected', async () => {
  const f = await scratchRollbackFixture();
  f.client.projectId = 'b'; f.release();
  await expect(f.pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.local.drafts[f.from]).toBe('Original source'); expect(f.client.local.drafts[f.to]).toBeUndefined();
  expect(f.client.local.composerControls.contexts[f.from]).toBe(f.context);
  expect(f.client.local.composerControls.contexts[f.to]).toBeUndefined();
  expect(f.client.local.selections.remote).toBeUndefined(); expect(f.client.draft).toBe('B original');
  expect(f.calls.some(call => call.op === 'connect')).toBe(false);
});

test('scratch captured-slot rollback preserves later source and destination text/context/selection writes', async () => {
  const f = await scratchRollbackFixture();
  f.client.projectId = 'b';
  f.client.local.drafts[f.from] = 'New source edit'; f.client.local.drafts[f.to] = 'New destination edit';
  const context = { envMode: 'local', branch: 'newer', worktreePath: '' };
  f.client.local.composerControls.contexts[f.to] = context;
  f.client.local.selections.remote = { projectId: 'newer-project', threadId: 'newer-thread' };
  f.release(); await expect(f.pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.local.drafts[f.from]).toBe('New source edit'); expect(f.client.local.drafts[f.to]).toBe('New destination edit');
  expect(f.client.local.composerControls.contexts[f.to]).toBe(context);
  expect(f.client.local.composerControls.contexts[f.from]).toBe(f.context);
  expect(f.client.local.selections.remote).toEqual({ projectId: 'newer-project', threadId: 'newer-thread' });
  expect(f.client.draft).toBe('B original');
});

test('a refused Connect reply also rolls back by captured cells without clobbering later edits', async () => {
  const f = await scratchRollbackFixture(true);
  f.client.local.drafts[f.from] = 'Edited source during wait';
  f.client.local.drafts[f.to] = 'Edited destination during wait';
  const context = { envMode: 'local', branch: 'later', worktreePath: '/later' };
  f.client.local.composerControls.contexts[f.from] = context;
  const selected = { projectId: 'other', threadId: '' }; f.client.local.selections.remote = selected;
  f.release(); await expect(f.pending).rejects.toMatchObject({ kind: 'denied', message: 'Connect refused' });
  expect(f.client.local.drafts[f.from]).toBe('Edited source during wait');
  expect(f.client.local.drafts[f.to]).toBe('Edited destination during wait');
  expect(f.client.local.composerControls.contexts[f.from]).toBe(context);
  expect(f.client.local.selections.remote).toBe(selected);
});


test('New Task keyboard offset uses an explicit docked guide fact, never focus-like values', () => {
  expect(mobileLayoutFacts('').keyboardDocked).toBe(false);
  expect(mobileLayoutFacts('{"keyboardDocked":true}').keyboardDocked).toBe(true);
  for (const value of [false, null, 1, "true", {}, []]) {
    expect(mobileLayoutFacts(JSON.stringify({ keyboardDocked: value })).keyboardDocked).toBe(false);
  }
  expect(mobileLayoutFacts('{"safeTop":-1,"safeBottom":34,"keyboardDocked":true,"liquidGlass":true}'))
    .toEqual({ safeTop: 0, safeBottom: 34, liquidGlass: true, keyboardDocked: true, displayScale: 1, windowHeight: 0 });
  expect(mobileLayoutFacts('{"focused":true,"height":300}').keyboardDocked).toBe(false);
});

test('window height stays unavailable until an attached native window publishes a positive finite size', () => {
  expect(mobileLayoutFacts('').windowHeight).toBe(0);
  expect(mobileLayoutFacts('not-json').windowHeight).toBe(0);
  for (const value of [undefined, null, false, true, 0, -1, '1133', {}, []]) {
    expect(mobileLayoutFacts(JSON.stringify({ windowHeight: value })).windowHeight).toBe(0);
  }
  expect(mobileLayoutFacts('{"windowHeight":1e999}').windowHeight).toBe(0);
  expect(mobileLayoutFacts('{"height":748,"keyboardDocked":true}').windowHeight).toBe(0);
  expect(mobileLayoutFacts('{"windowHeight":1133,"height":748,"keyboardDocked":true}'))
    .toMatchObject({ windowHeight: 1133, keyboardDocked: true });
  expect(mobileLayoutFacts('{"windowHeight":744}').windowHeight).toBe(744);
});

function modelConfig(client: T3Client) {
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, settings: { projectSettingsFolded: true, defaultModelSelection: null,
    providerInstances: { codex: { driver: 'codex', enabled: true, config: {} } },
    projectSettingsOverrides: { a: { defaultModelSelection: { instanceId: 'codex', model: 'configured', options: [{ id: 'reasoning', value: 'high' }] } } } },
    providers: [{ instanceId: 'codex', driver: 'codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' },
      models: [{ slug: 'catalog', name: 'Catalog', isDefault: true }] }] };
}

test('actual New Task flow preserves configured noncatalog model and options through default recalculation', async () => {
  const f = await fixture(); modelConfig(f.client);
  f.client.local.composerControls.stickyProvider = 'codex';
  f.client.local.composerControls.stickyByProvider.codex = { model: 'catalog', options: [] };
  const chooser = f.snapshot('/new');
  expect(await f.action(chooser.owner, 'project', '["env","a"]')).toMatchObject({ message: '', submitted: false });
  f.client.local.drafts[f.client.draftKey] = 'Fresh content';
  expect(f.client.providerId).toBe('codex'); expect(f.client.modelId).toBe('configured');
  expect(f.client.modelOptions).toEqual([{ id: 'reasoning', value: 'high' }]);
  expect(mobileNewTask('', f.client, f.fleet).composer).toMatchObject({ modelLabel: 'configured', canSend: true });
  f.client.chooseDefaults();
  expect(f.client.modelId).toBe('configured'); expect(f.client.modelOptions).toEqual([{ id: 'reasoning', value: 'high' }]);
  expect(f.snapshot('/new/draft', 'draft').ready).toBe(true);
  expect(f.client.draft).toBe('Fresh content');
  expect(f.calls.some(call => ['orchestration.launchThread', 'orchestration.dispatchCommand'].includes(String(call.method)))).toBe(false);
  f.client.scopes = [];
  expect(mobileNewTask('', f.client, f.fleet).composer.canSend).toBe(false);
});

test('fresh defaults respect folded projects, explicit null and provider disable flags', async () => {
  const f = await fixture(); modelConfig(f.client);
  const settings = obj(f.client.config.settings), overrides = obj(settings.projectSettingsOverrides);
  f.client.shell.projects[0]!.defaultModelSelection = { instanceId: 'codex', model: 'legacy-project' };
  settings.defaultModelSelection = { instanceId: 'codex', model: 'environment' };
  overrides.a = {}; f.client.chooseDefaults(); expect(f.client.modelId).toBe('environment');
  settings.projectSettingsFolded = false;
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('legacy-project');
  overrides.a = { defaultModelSelection: null };
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('catalog');
  overrides.a = { defaultModelSelection: undefined };
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('environment');
  overrides.a = { defaultModelSelection: { instanceId: 'codex', model: 'project' } };
  obj(obj(settings.providerInstances).codex).config = { enabled: false };
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('environment');
});

test('sticky noncatalog choices retain options after implicit legacy defaults fall through', async () => {
  const f = await fixture(); modelConfig(f.client);
  const provider = obj((f.client.config.providers as Obj[])[0]);
  (provider.models as Obj[]).push({ slug: 'configured', name: 'Legacy', isLegacy: true });
  f.client.local.composerControls.stickyProvider = 'codex';
  f.client.local.composerControls.stickyByProvider.codex = { model: 'remembered', options: [{ id: 'fast', value: true }] };
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('remembered');
  expect(f.client.modelOptions).toEqual([{ id: 'fast', value: true }]);
  provider.auth = { status: 'unauthenticated' };
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('');
  expect(mobileNewTask('', f.client, f.fleet).composer).toMatchObject({ modelLabel: 'Choose model', canSend: false });
});

test('Antigravity keeps unavailable configured choice while send stays blocked', async () => {
  const f = await fixture(); modelConfig(f.client);
  obj(obj(f.client.config.settings).projectSettingsOverrides).a = { defaultModelSelection: { instanceId: 'anti', model: 'retained' } };
  obj(obj(f.client.config.settings).providerInstances).anti = { driver: 'antigravity', enabled: true };
  f.client.chooseDefaults(); expect(f.client.providerId).toBe('anti'); expect(f.client.modelId).toBe('retained');
  expect(mobileNewTask('', f.client, f.fleet).composer).toMatchObject({ modelLabel: 'retained', modelUnavailable: true, canSend: false });
});

test('automatic choice follows catalog order and defaults without treating model display as send readiness', async () => {
  const f = await fixture(); modelConfig(f.client);
  obj(obj(f.client.config.settings).projectSettingsOverrides).a = {};
  const provider = obj((f.client.config.providers as Obj[])[0]);
  provider.status = 'error'; provider.models = [{ slug: 'first', name: 'First' }, { slug: 'default', name: 'Default', isDefault: true }];
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('default');
  expect(mobileNewTask('', f.client, f.fleet).composer.canSend).toBe(false);
  provider.models = [{ slug: 'first', name: 'First' }];
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('first');
  provider.enabled = false; f.client.chooseDefaults(); expect(f.client.modelId).toBe('');
});


const effort = [{ id: 'effort', label: 'Reasoning effort', type: 'select', currentValue: 'medium',
  options: [{ id: 'medium', label: 'Medium' }, { id: 'high', label: 'High' }] }];
function twoProviders(client: T3Client) {
  const ready = { enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, supportedRuntimeModes: ['approval-required', 'full-access'] };
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, settings: { projectSettingsFolded: true, defaultModelSelection: null,
    providerInstances: { codex: { driver: 'codex', enabled: true, config: {} }, claude: { driver: 'claudeAgent', enabled: true, config: {} } }, projectSettingsOverrides: {} },
    providers: [{ instanceId: 'codex', driver: 'codex', ...ready, models: [{ slug: 'gpt-a', name: 'GPT A', isDefault: true }, { slug: 'gpt-b', name: 'GPT B' }] },
      { instanceId: 'claude', driver: 'claudeAgent', ...ready, models: [{ slug: 'opus', name: 'Opus', isDefault: true },
        { slug: 'sonnet', name: 'Sonnet', capabilities: { optionDescriptors: effort } }] }] };
}
const sticky = (client: T3Client) => { const prefs = client.local.composerControls;
  return { provider: prefs.stickyProvider, choice: prefs.stickyByProvider[prefs.stickyProvider] }; };

test('existing-thread Save and option changes keep the New Task sticky pick while remembering per-model options', async () => {
  const f = await fixture(); twoProviders(f.client);
  const settings = (kind: string, id = '', value = '') => mobileComposerSettingsAction(kind, id, value, f.native, f.files, f.client);
  const chooser = f.snapshot('/new');
  expect(await f.action(chooser.owner, 'project', '["env","a"]')).toMatchObject({ message: '' });
  expect(f.client.modelId).toBe('gpt-a');
  await settings('open'); await settings('pick', 'codex', 'gpt-b');
  expect(await settings('save')).toMatchObject({ closed: true, message: '' });
  expect(sticky(f.client)).toEqual({ provider: 'codex', choice: { model: 'gpt-b', options: [] } });
  // The same client opens an existing thread and changes its model and options.
  f.client.threadId = 'thread';
  f.client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
    projection: { thread: { id: 'thread', projectId: 'a', modelSelection: { instanceId: 'codex', model: 'gpt-a' } }, runs: [], turnItems: [] } };
  f.client.providerId = 'codex'; f.client.modelId = 'gpt-a'; f.client.modelOptions = [];
  const dispatched = () => f.calls.filter(call => call.method === 'orchestration.dispatchCommand').map(call => obj(obj(call.payload).modelSelection));
  await settings('open'); await settings('pick', 'claude', 'sonnet'); await settings('option', 'effort', 'high');
  expect(await settings('save')).toMatchObject({ closed: true, message: '' });
  await settings('open'); expect((await settings('option', 'effort', 'medium')).message).toBe(''); await settings('cancel');
  const medium = [{ id: 'effort', value: 'medium' }];
  expect({ dispatched: dispatched(), thread: f.client.local.composerControls.staged[f.client.draftKey], sticky: sticky(f.client),
    memory: f.client.local.composerControls.stickyOptionsByModel.claude?.sonnet })
    .toEqual({ dispatched: [], thread: { providerId: 'claude', modelId: 'sonnet', options: medium },
      sticky: { provider: 'codex', choice: { model: 'gpt-b', options: [] } }, memory: medium });
  // Back in New Task, the default is the earlier New Task pick; a later pick of the thread model brings back its options.
  f.client.threadId = ''; f.client.thread = null;
  const next = f.snapshot('/new', 'again', 'again-flow');
  expect(await f.action(next.owner, 'project', '["env","a"]', '', 'again')).toMatchObject({ message: '' });
  expect({ provider: f.client.providerId, model: f.client.modelId }).toEqual({ provider: 'codex', model: 'gpt-b' });
  await settings('open'); await settings('pick', 'claude', 'sonnet');
  expect(await settings('save')).toMatchObject({ closed: true, message: '' });
  expect({ options: f.client.modelOptions, sticky: sticky(f.client) }).toEqual({ options: medium, sticky: { provider: 'claude', choice: { model: 'sonnet', options: medium } } });
});

test('New Task option-only change sticks the current model with its options, not an older provider sticky model', async () => {
  const f = await fixture(); twoProviders(f.client);
  const overrides = obj(obj(f.client.config.settings).projectSettingsOverrides);
  overrides.a = { defaultModelSelection: { instanceId: 'claude', model: 'sonnet' } };
  const prefs = f.client.local.composerControls;
  prefs.stickyProvider = 'codex';
  prefs.stickyByProvider = { codex: { model: 'gpt-a', options: [] }, claude: { model: 'opus', options: [] } };
  const chooser = f.snapshot('/new');
  expect(await f.action(chooser.owner, 'project', '["env","a"]')).toMatchObject({ message: '' });
  expect({ provider: f.client.providerId, model: f.client.modelId }).toEqual({ provider: 'claude', model: 'sonnet' });
  await mobileComposerSettingsAction('open', '', '', f.native, f.files, f.client);
  expect((await mobileComposerSettingsAction('option', 'effort', 'high', f.native, f.files, f.client)).message).toBe('');
  const high = [{ id: 'effort', value: 'high' }];
  expect({ options: f.client.modelOptions, sticky: sticky(f.client), memory: prefs.stickyOptionsByModel.claude?.sonnet })
    .toEqual({ options: high, sticky: { provider: 'claude', choice: { model: 'sonnet', options: high } }, memory: high });
  overrides.a = {};
  const next = f.snapshot('/new', 'again', 'again-flow');
  expect(await f.action(next.owner, 'project', '["env","a"]', '', 'again')).toMatchObject({ message: '' });
  expect({ model: f.client.modelId, options: f.client.modelOptions }).toEqual({ model: 'sonnet', options: high });
});

test('draft file reads use explicit worktree cwd and shared source rendering', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/src%2Fa.ts?environmentId=env&cwd=%2Fworktree&projectName=Worktree&line=2';
  const flow = f.snapshot(location, 'file'), calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: f.client.generation, value: request.op === 'http'
      ? { authenticated: true, permissions: ['filesystem:read'] } : { contents: 'one\r\n\ttwo', truncated: true } };
  } };
  const file = await mobileNewTaskFileRead('src/a.ts', location, 'file', flow.owner, false, native, f.client);
  expect(file).toMatchObject({ title: 'a.ts', subtitle: 'Worktree · src', contents: 'one\n\ttwo', truncated: true, initialRowId: 'source-line:1' });
  expect(file.rows[1]).toMatchObject({ text: '    two', selected: true });
  expect(calls.find(call => call.method === 'projects.readFile')?.payload).toEqual({ cwd: '/worktree', relativePath: 'src/a.ts' });
  expect(f.client.projectId).toBe('a'); expect(f.client.threadId).toBe(''); expect(f.client.draft).toBe('');
});

test('draft file permission downgrade clears content and cross-environment routes never read', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts', flow = f.snapshot(location, 'file'); let allowed = true, reads = 0;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); if (request.method) reads++;
    return { ok: true, generation: f.client.generation, value: request.op === 'http'
      ? { authenticated: true, permissions: allowed ? ['filesystem:read'] : [], scopes: ['filesystem:read'] } : { contents: 'private' } };
  } };
  expect((await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, native, f.client)).contents).toBe('private');
  allowed = false;
  const denied = await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, native, f.client);
  expect(denied.contents).toBe(''); expect(denied.error).toContain('cannot read'); expect(reads).toBe(1);
  const foreign = location + '?environmentId=other&cwd=/private'; f.snapshot(foreign, 'foreign');
  await mobileNewTaskFileRead('a.ts', foreign, 'foreign', flow.owner, false, native, f.client);
  expect(reads).toBe(1);
});

test('draft file replies cannot cross a route change or a newer request', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts', flow = f.snapshot(location, 'file'); let release!: (reply: unknown) => void;
  const slow: Native = { available: true, watch() {}, async later() { return new Promise(resolve => { release = resolve; }); } };
  const old = mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, slow, f.client);
  f.snapshot('/new/draft', 'draft');
  release({ ok: true, generation: f.client.generation, value: { authenticated: true, permissions: ['filesystem:read'] } });
  await expect(old).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client).contents).toBe('');
  f.snapshot(location, 'file');
  const older = mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, slow, f.client);
  const ready: Native = { available: true, watch() {}, async later(input) {
    return { ok: true, generation: f.client.generation, value: obj(input).op === 'http'
      ? { authenticated: true, permissions: ['filesystem:read'] } : { contents: 'newest' } };
  } };
  await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, ready, f.client);
  release({ ok: true, generation: f.client.generation, value: { authenticated: true, permissions: ['filesystem:read'] } });
  await expect(older).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client).contents).toBe('newest');
});


test('draft file query replacement invalidates the old read without clearing the current cache', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const oldLocation = '/new/draft/files/a.ts?cwd=/old', location = '/new/draft/files/a.ts?cwd=/new';
  const flow = f.snapshot(oldLocation, 'file'); let release!: (reply: unknown) => void;
  const slow: Native = { available: true, watch() {}, async later() { return new Promise(resolve => { release = resolve; }); } };
  const pending = mobileNewTaskFileRead('a.ts', oldLocation, 'file', flow.owner, false, slow, f.client);
  f.snapshot(location, 'file');
  const ready: Native = { available: true, watch() {}, async later(input) {
    return { ok: true, generation: f.client.generation, value: obj(input).op === 'http'
      ? { authenticated: true, permissions: ['filesystem:read'] } : { contents: 'current workspace' } };
  } };
  await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, ready, f.client);
  release({ ok: true, generation: f.client.generation, value: { authenticated: true, permissions: ['filesystem:read'] } });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  await expect(mobileNewTaskFileRead('a.ts', oldLocation, 'file', flow.owner, false, ready, f.client)).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileNewTaskFileSnapshot('a.ts', oldLocation, 'file', flow.owner, false, f.client).contents).toBe('');
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client).contents).toBe('current workspace');
  expect(f.client.draft).toBe('');
});


function fileNative(f: Awaited<ReturnType<typeof fixture>>, calls: Obj[], content = 'answer') {
  return { available: true, watch() {}, async later(input: unknown) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: f.client.generation, value: request.op === 'http'
      ? { authenticated: true, permissions: ['filesystem:read'] } : { contents: content } };
  } };
}
test('draft file legal raw question mark must not erase a later environmentId', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = mobileAppLink('/new/draft/files/a.ts?cwd=/work?tree&environmentId=other', 'file').location;
  const flow = f.snapshot(location, 'file'), calls: Obj[] = [];
  expect(new URLSearchParams(location.slice(location.indexOf('?') + 1)).get('environmentId')).toBe('other');
  await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, fileNative(f, calls), f.client);
  expect(calls.filter(call => call.method === 'projects.readFile')).toHaveLength(0);
});
test('draft file valid trailing space in Unix cwd must survive route decoding', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts?cwd=%2Fwork%20&environmentId=env', flow = f.snapshot(location, 'file'), calls: Obj[] = [];
  await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, fileNative(f, calls), f.client);
  expect(calls.find(call => call.method === 'projects.readFile')?.payload).toEqual({ cwd: '/work ', relativePath: 'a.ts' });
});
test('draft file read RPC completion after route exit is refused', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts', flow = f.snapshot(location, 'file');
  let release!: (reply: unknown) => void, reached!: () => void;
  const requested = new Promise<void>(resolve => { reached = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    if (obj(input).op === 'http') return {ok: true, generation: f.client.generation, value: {authenticated: true, permissions: ['filesystem:read']}};
    reached(); return new Promise(resolve => { release = resolve; });
  }};
  const reading = mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, native, f.client);
  await requested; f.snapshot('/new/draft', 'draft');
  release({ok: true, generation: f.client.generation, value: {contents: 'old'}});
  await expect(reading).rejects.toMatchObject({kind: 'superseded'});
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client).contents).toBe('');
});
test('draft file read RPC completion after reconnect is refused', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts', flow = f.snapshot(location, 'file');
  let release!: (reply: unknown) => void, reached!: () => void;
  const requested = new Promise<void>(resolve => { reached = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    if (obj(input).op === 'http') return {ok: true, generation: f.client.generation, value: {authenticated: true, permissions: ['filesystem:read']}};
    reached(); return new Promise(resolve => { release = resolve; });
  }};
  const reading = mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, native, f.client);
  await requested; f.client.generation++;
  release({ok: true, generation: f.client.generation - 1, value: {contents: 'old'}});
  await expect(reading).rejects.toMatchObject({kind: 'superseded'});
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client).contents).toBe('');
});
test('draft file runtime abandonment produces no content or sticky spinner', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts', flow = f.snapshot(location, 'file');
  const native: Native = {available: true, watch() {}, async later() {throw {name:'FetchError',kind:'Aborted'};}};
  await expect(mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, native, f.client)).rejects.toMatchObject({kind:'superseded'});
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client)).toMatchObject({contents:'', loading:false, error:''});
});


test('Add Project routes retain the containing draft while refusing composer actions', async () => {
  const f = await fixture(), chooser = f.snapshot('/new');
  await f.action(chooser.owner, 'project', '["env","b"]');
  const draft = f.snapshot('/new/draft', 'draft');
  for (const suffix of ['', '/repository', '/destination', '/local', '/new']) {
    const added = f.snapshot(`/new/add-project${suffix}?environmentId=env`, 'add');
    expect(added).toMatchObject({ owner: draft.owner, status: 'add-project', ready: false, needsPrepare: false, nextLocation: '', draftOwner: draft.draftOwner });
    expect(owns(draft.owner, 'add', f.client)).toBe(false);
    const calls = f.calls.length;
    for (const kind of ['draft', 'project', 'scratch', 'prepare']) {
      expect((await f.action(added.owner, kind, '["env","a"]', 'changed', 'add')).message).toContain('Return to the new task');
    }
    expect(f.calls).toHaveLength(calls);
    expect(f.client.projectId).toBe('b'); expect(f.client.draft).toBe('');
    expect(f.snapshot('/new/draft', 'draft')).toMatchObject({ owner: draft.owner, ready: true, draftOwner: draft.draftOwner });
  }
  expect(mobileNewTaskRoute('/new/add-project/unknown').context).toBe('');
});


test('chooser Add Project uses connection availability and retains unfiltered project presence', async () => {
  const f = await fixture();
  expect(mobileNewTask('unmatched', f.client, f.fleet)).toMatchObject({ canAddProject: true, hasProjects: true, projects: [] });
  f.client.connection = 'disconnected';
  expect(mobileNewTask('', f.client, f.fleet).canAddProject).toBe(false);
  f.client.shell.projects = [];
  expect(mobileNewTask('', f.client, f.fleet)).toMatchObject({ canAddProject: false, hasProjects: false });
});


test('initial clone draft admits route but guards actual Send until authoritative stream answer', async () => {
  const f = await fixture();
  f.client.config = { ...f.client.config, environment: { capabilities: { projectCloneTracking: true, serverResolvedCommandContext: true } } };
  const location = '/new/draft?environmentId=env&projectId=a&cloning=1';
  expect(mobileNewTaskRoute(location).unsupported).toBe('');
  let flow = f.snapshot(location);
  await f.action(flow.owner, 'prepare'); flow = f.snapshot(location);
  expect(flow.ready).toBe(true);
  mobileNewTaskCloneObserve(flow.owner, 'visit', location, true, f.client);
  expect(mobileNewTask('', f.client, f.fleet).composer).toMatchObject({ canSend: false, blockedReason: 'Cloning repository' });
  f.calls.length = 0;
  expect(await f.action(flow.owner, 'send')).toMatchObject({ message: 'Cloning repository', submitted: false });
  expect(f.calls).toHaveLength(0); expect(f.client.draft).toBe('');
  liveEvent(f.client, { key: 'project-clones', subscriptionId: 'clone-1', value: [{ projectId: 'a', phase: 'failed' }] });
  expect(await f.action(flow.owner, 'send')).toMatchObject({ message: 'Repository not cloned', submitted: false });
  expect(f.calls).toHaveLength(0);
  liveEvent(f.client, { key: 'project-clones', subscriptionId: 'clone-1', value: [] });
  expect(mobileNewTask('', f.client, f.fleet).composer.blockedReason).not.toBe('Cloning repository');
});


test('acknowledged clone removal survives shell fallback without racing the draft redirect', async () => {
  for (const leave of [false, true, 'same-visit']) {
    const f = await fixture(), location = '/new/draft?environmentId=env&projectId=a&cloning=1';
    f.client.config = { environment: { capabilities: { projectCloneTracking: true, serverResolvedCommandContext: true } }, providers: [] };
    let flow = f.snapshot(location); await f.action(flow.owner, 'prepare'); flow = f.snapshot(location);
    liveEvent(f.client, { key: 'project-clones', subscriptionId: 'clone-1', value: [{ projectId: 'a', phase: 'failed' }] });
    const previous = f.native.later;
    f.native.later = async input => {
      const request = obj(input);
      if (request.path === '/api/auth/session') return { ok: true, generation: f.client.generation,
        value: { authenticated: true, permissions: ['orchestration:operate'] } };
      if (request.op === 'ids') return { ok: true, generation: f.client.generation, value: ['delete-project'] };
      if (request.method === 'projects.mutate') {
        f.client.shell.projects = f.client.shell.projects.filter(project => project.id !== 'a'); f.client.projectId = 'b';
        const during = f.snapshot(leave ? '/new' : location, leave === true ? 'other' : 'visit');
        if (!leave) expect(during).toMatchObject({ busy: true, nextLocation: '' });
        mobileNewTaskCloneObserve(during.owner, during.requestRoute, leave ? '/new' : location, during.ready || during.busy, f.client);
        return { ok: true, generation: f.client.generation, value: {} };
      }
      return previous(input);
    };
    const removed = await f.action(flow.owner, 'clone-remove', 'a', 'env');
    expect(removed).toMatchObject({ nextLocation: leave ? '' : '/', message: '' });
    // Exact mutation result records are closed. The helper-only removed flag must not leak.
    expect(Object.keys(removed).sort()).toEqual(['alertTitle', 'environmentId', 'message', 'nextLocation', 'projectId', 'requestRoute', 'revision', 'submitted', 'threadId']);
    expect(f.client.local.drafts['env:new:a']).toBe('A original');
  }
});


import { mobileNewTaskAction } from './new-task';
import { mobileNewTaskDraftPresentation, mobileNewTaskDraftSelectedBranch } from './mobile-new-task-drafts';
test('actual branch picker distinguishes untouched checkout from selecting that same branch', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const original = f.native.later;
  f.native.later = async input => {
    const call = obj(input);
    if (call.method === 'vcs.refreshStatus') return { ok: true, generation: f.client.generation, value: { isRepo: true, refName: 'main' } };
    if (call.method === 'vcs.listRefs') return { ok: true, generation: f.client.generation, value: { refs: [{ name: 'main', current: true, isDefault: true }], total: 1, nextCursor: null } };
    return original(input);
  };
  await mobileNewTaskPrepare('', f.native, f.client);
  expect(draftContext(f.client).branch).toBe('main');
  expect(mobileNewTaskDraftSelectedBranch(mobileNewTaskDraftPresentation(f.client, f.client.draftKey)!)).toBeNull();
  expect((await mobileNewTaskAction('branch', 'main', '', f.native, f.files, f.client, f.fleet)).message).toBe('');
  expect(mobileNewTaskDraftSelectedBranch(mobileNewTaskDraftPresentation(f.client, f.client.draftKey)!)).toBe('main');
  await mobileNewTaskPrepare('', f.native, f.client);
  expect(mobileNewTaskDraftSelectedBranch(mobileNewTaskDraftPresentation(f.client, f.client.draftKey)!)).toBe('main');
});

const localInteractionPreferences = { ready: true, serialized: JSON.stringify({ ready: true, planModeEnabled: true }) };
async function localInteractionDraft() {
  const f = await fixture();
  f.client.config.providers = [{ instanceId: 'provider', driver: 'codex', enabled: true, installed: true, status: 'ready',
    showInteractionModeToggle: true, models: [{ slug: 'model', name: 'Model', isDefault: true }] }];
  const chooser = f.snapshot('/new');
  expect((await f.action(chooser.owner, 'project', '["env","a"]')).message).toBe('');
  const flow = f.snapshot('/new/draft');
  expect(flow.ready).toBe(true); expect(flow.draftOwner).toBe(mobileComposerTarget(f.client).owner);
  expect(f.client).toBeInstanceOf(MobileDraftClient);
  return { ...f, flow };
}

test('NewTask local interaction projects the supported ordinary draft mode', async () => {
  const f = await localInteractionDraft(), calls = f.calls.length;
  const before = JSON.stringify([f.client.interactionMode, mobileNewTaskDraftPresentation(f.client, f.client.draftKey)?.choices]);
  const data = mobileNewTask('', f.client, f.fleet, localInteractionPreferences);
  expect({ enabled: obj(data).planModeEnabled ?? null, mode: obj(data).interactionMode ?? null,
    changed: JSON.stringify([f.client.interactionMode, mobileNewTaskDraftPresentation(f.client, f.client.draftKey)?.choices]) !== before,
    calls: f.calls.slice(calls) })
    .toEqual({ enabled: true, mode: 'default', changed: false, calls: [] });
});

test('NewTask local interaction changes a ready ordinary draft', async () => {
  const f = await localInteractionDraft(), calls = f.calls.length;
  const response = await act(f.flow.owner, 'visit', 'interaction', f.flow.draftOwner, 'plan', f.native, f.files,
    f.client, f.fleet, 1791420000000, localInteractionPreferences);
  expect({ message: response.message, mode: f.client.interactionMode,
    stored: mobileNewTaskDraftPresentation(f.client, f.client.draftKey)?.choices?.interactionMode ?? null,
    calls: f.calls.slice(calls) }).toEqual({ message: '', mode: 'plan', stored: 'plan', calls: [] });
});

async function localInteractionStorage() {
  const f = await localInteractionDraft(), writes: Obj[] = [];
  let disk = '';
  f.files.fs.atomicWriteFile = async (_path, bytes) => { disk = new TextDecoder().decode(bytes); writes.push(obj(JSON.parse(disk))); };
  await f.client.persist(f.files); writes.length = 0; f.calls.length = 0;
  const choose = (value: string, preferences = localInteractionPreferences, blocked = false) => act(f.flow.owner, 'visit', 'interaction', f.flow.draftOwner,
    value, f.native, f.files, f.client, f.fleet, 1791420000000, preferences, blocked);
  return { ...f, writes, choose, disk: () => obj(JSON.parse(disk)) };
}
const localInteractionChoice = (document: Obj, key: string) => obj(obj(obj(obj(document.mobileNewTaskDrafts).records)[key]).choices).interactionMode;

for (const row of [
  { name: 'loaded on', ready: true, saved: { ready: true, planModeEnabled: true }, provider: true, enabled: true },
  { name: 'loaded off', ready: true, saved: { ready: true, planModeEnabled: false }, provider: true, enabled: false },
  { name: 'loading ignores stale true', ready: false, saved: { ready: true, planModeEnabled: true }, provider: true, enabled: false },
  { name: 'malformed JSON', ready: true, saved: null, provider: true, enabled: false },
  { name: 'invalid preference value', ready: true, saved: { ready: true, planModeEnabled: 'true' }, provider: true, enabled: false },
  { name: 'failed inner snapshot', ready: true, saved: { ready: false, planModeEnabled: true }, provider: true, enabled: false },
  { name: 'provider hides modes', ready: true, saved: { ready: true, planModeEnabled: true }, provider: false, enabled: false },
  { name: 'no selected provider status', ready: true, saved: { ready: true, planModeEnabled: true }, provider: null, enabled: true },
]) test(`NewTask local interaction preference/provider gate: ${row.name}`, async () => {
  const f = await localInteractionStorage(), key = f.client.draftKey;
  expect(mobileNewTaskDraftChoicesUpdate(f.client, key, { interactionMode: 'plan' })).toBe(true);
  f.client.interactionMode = 'plan'; f.client.local.deviceSettings.planModeEnabled = true;
  if (row.provider === null) f.client.config.providers = [];
  else arr(f.client.config.providers)[0]!.showInteractionModeToggle = row.provider;
  const preferences = { ready: row.ready, serialized: row.saved === null ? '{' : JSON.stringify(row.saved) };
  const data = mobileNewTask('', f.client, f.fleet, preferences);
  expect({ enabled: data.planModeEnabled, mode: data.interactionMode, stored: mobileNewTaskDraftLookup(f.client, key)?.choices?.interactionMode,
    clientMode: f.client.interactionMode, writes: f.writes.length, calls: f.calls })
    .toEqual({ enabled: row.enabled, mode: row.enabled ? 'plan' : 'default', stored: 'plan', clientMode: 'plan', writes: 0, calls: [] });
  if (!row.enabled) { expect((await f.choose('default', preferences)).message).not.toBe(''); expect(f.writes).toEqual([]); }
  else { expect((await f.choose('default', preferences)).message).toBe(''); expect(localInteractionChoice(f.writes.at(-1)!, key)).toBe('default'); }
});

for (const offline of [false, true]) test(`NewTask local interaction persists both choices without server authority: offline=${offline}`, async () => {
  const f = await localInteractionStorage(), key = f.client.draftKey;
  const other = mobileNewTaskDraftCreate(f.client, { id: 'other-mode-draft', environmentId: 'env', projectId: 'b', origin: f.client.origin, createdAt: '2026-10-09T00:00:00.000Z' });
  const before = JSON.stringify({ text: f.client.draft, model: [f.client.providerId, f.client.modelId, f.client.modelOptions], runtime: f.client.runtimeMode,
    workspace: draftContext(f.client), other: mobileNewTaskDraftLookup(f.client, other.key) });
  f.client.scopes = ['orchestration:read']; if (offline) f.client.connection = 'disconnected';
  expect((await f.choose('plan')).message).toBe(''); expect(localInteractionChoice(f.writes[0]!, key)).toBe('plan');
  expect((await f.choose('default')).message).toBe(''); expect(localInteractionChoice(f.writes.at(-1)!, key)).toBe('default');
  expect(JSON.stringify({ text: f.client.draft, model: [f.client.providerId, f.client.modelId, f.client.modelOptions], runtime: f.client.runtimeMode,
    workspace: draftContext(f.client), other: mobileNewTaskDraftLookup(f.client, other.key) })).toBe(before);
  const restart = new MobileDraftClient(), disk = f.disk();
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(JSON.stringify(disk)).buffer; }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch() {}, async later(input) { if (obj(input).op !== 'devicePresentation') throw Error('Unexpected restart request'); return { ok: true, generation: 0, value: {} }; } };
  const handles = mobileDraftRecoveryHandles(restart, native, files);
  expect((await restart.command('dismiss-error', '', '', 0, handles.native, handles.storage)).message).toBe('');
  expect(mobileNewTaskDraftLookup(restart, key)?.choices?.interactionMode).toBe('default');
  expect(f.calls).toEqual([]);
});

test('NewTask local interaction rejects imports, stale visits, unready routes and wrong content owners', async () => {
  const f = await localInteractionStorage(), key = f.client.draftKey;
  expect((await f.choose('plan', localInteractionPreferences, true)).message).not.toBe('');
  expect((await f.choose('other')).message).not.toBe('');
  await expect(act(f.flow.owner, 'visit', 'interaction', 'wrong-owner', 'plan', f.native, f.files, f.client, f.fleet, 0, localInteractionPreferences))
    .rejects.toMatchObject({ kind: 'superseded' });
  f.snapshot('/new/draft', 'new-visit');
  expect((await f.choose('plan')).message).toContain('route changed');
  const unready = f.snapshot('/new/draft?environmentId=env&projectId=a&pendingTaskId=missing', 'unready');
  expect(unready.ready).toBe(false);
  expect((await act(unready.owner, 'unready', 'interaction', f.flow.draftOwner, 'plan', f.native, f.files, f.client, f.fleet, 0, localInteractionPreferences)).message).not.toBe('');
  expect(mobileNewTaskDraftLookup(f.client, key)?.choices?.interactionMode).toBeUndefined(); expect(f.writes).toEqual([]); expect(f.calls).toEqual([]);
});

test('NewTask local interaction retains an accepted choice when storage rejects', async () => {
  const f = await localInteractionStorage(); f.files.fs.atomicWriteFile = async () => { throw Error('mode storage failed'); };
  expect((await f.choose('plan')).message).toBe('mode storage failed');
  expect([f.client.interactionMode, mobileNewTaskDraftLookup(f.client, f.client.draftKey)?.choices?.interactionMode]).toEqual(['plan', 'plan']);
  expect(mobileNewTask('', f.client, f.fleet, localInteractionPreferences).interactionError).toBe('mode storage failed'); expect(f.calls).toEqual([]);
});

for (const firstFails of [false, true]) test(`NewTask local interaction accepts opposite intents while the first save waits: firstFails=${firstFails}`, async () => {
  const f = await localInteractionStorage(), key = f.client.draftKey, serial: string[] = [];
  let entered!: () => void, release!: () => void;
  const started = new Promise<void>(done => { entered = done; }), held = new Promise<void>(done => { release = done; });
  let count = 0, queue = Promise.resolve();
  f.files.fs.atomicWriteFile = (_path, bytes) => {
    const mode = String(localInteractionChoice(obj(JSON.parse(new TextDecoder().decode(bytes))), key));
    const position = ++count;
    const writing = queue.then(async () => {
      if (position === 1) { entered(); await held; if (firstFails) throw Error('old mode save failed'); }
      serial.push(mode);
    });
    queue = writing.catch(() => {}); return writing;
  };
  const first = f.choose('plan'); await started;
  expect(owns(f.flow.owner, 'visit', f.client)).toBe(true); expect(f.client.interactionMode).toBe('plan');
  const second = f.choose('default'); expect(f.client.interactionMode).toBe('default'); release();
  const secondResult = await second, firstResult = await first;
  expect([secondResult.message, firstResult.message, f.client.interactionMode, mobileNewTaskDraftLookup(f.client, key)?.choices?.interactionMode])
    .toEqual(['', '', 'default', 'default']);
  expect(serial).toEqual(firstFails ? ['default'] : ['plan', 'default']);
  expect(mobileNewTask('', f.client, f.fleet, localInteractionPreferences).interactionError).toBe(''); expect(f.calls).toEqual([]);
});

test('NewTask local interaction ignores an older save rejection after the newer choice succeeds', async () => {
  const f = await localInteractionStorage(); let entered!: () => void, release!: () => void, count = 0;
  const started = new Promise<void>(done => { entered = done; }), held = new Promise<void>(done => { release = done; });
  f.files.fs.atomicWriteFile = async () => { if (++count === 1) { entered(); await held; throw Error('late old rejection'); } };
  const first = f.choose('plan'); await started;
  expect((await f.choose('default')).message).toBe(''); release(); expect((await first).message).toBe('');
  expect(f.client.interactionMode).toBe('default'); expect(mobileNewTask('', f.client, f.fleet, localInteractionPreferences).interactionError).toBe('');
});

test('NewTask local interaction completion cannot rewrite a successor draft or its errors', async () => {
  const f = await localInteractionStorage(), originalKey = f.client.draftKey;
  let entered!: () => void, release!: () => void;
  const started = new Promise<void>(done => { entered = done; }), held = new Promise<void>(done => { release = done; });
  f.files.fs.atomicWriteFile = async () => { entered(); await held; throw Error('departed mode save failed'); };
  const first = f.choose('plan'); await started;
  const other = mobileNewTaskDraftCreate(f.client, { id: 'mode-successor', environmentId: 'env', projectId: 'a', origin: f.client.origin, createdAt: '2026-10-10T00:00:00.000Z' });
  expect(mobileNewTaskDraftBind(f.client, other.key, 'successor')).toBe(true); f.client.interactionMode = 'default';
  f.snapshot('/new', 'successor', 'successor-flow'); release();
  expect((await first).message).toBe(''); expect(f.client.interactionMode).toBe('default');
  expect(mobileNewTaskDraftLookup(f.client, originalKey)?.choices?.interactionMode).toBe('plan');
  expect(mobileNewTaskDraftLookup(f.client, other.key)?.choices).toBeNull();
  expect(mobileNewTask('', f.client, f.fleet, localInteractionPreferences).interactionError).toBe('');
});

test('NewTask local interaction works in restored and scratch drafts without changing their destination', async () => {
  const f = await localInteractionStorage(), key = f.client.draftKey;
  const location = `/new/draft?draftId=${encodeURIComponent(key)}`, restoring = f.snapshot(location, 'restored');
  expect((await f.action(restoring.owner, 'prepare', '', '', 'restored')).message).toBe('');
  const restored = f.snapshot(location, 'restored'); expect(restored.ready).toBe(true);
  f.client.config.scratchWorkspaceRoot = '/a'; expect(mobileNewTask('', f.client, f.fleet).scratch).toBe(true);
  const before = JSON.stringify([f.client.environmentId, f.client.projectId, f.client.draftKey, draftContext(f.client)]), calls = f.calls.length;
  expect((await act(restored.owner, 'restored', 'interaction', restored.draftOwner, 'plan', f.native, f.files, f.client, f.fleet, 0, localInteractionPreferences)).message).toBe('');
  expect(JSON.stringify([f.client.environmentId, f.client.projectId, f.client.draftKey, draftContext(f.client)])).toBe(before); expect(f.calls.slice(calls)).toEqual([]);
});

test('NewTask local interaction admits the bound next draft while a validated captured launch waits', async () => {
  const f = await localInteractionStorage(), originalKey = f.client.draftKey;
  f.client.local.drafts[originalKey] = 'captured task';
  const pending = { method: 'orchestration.launchThread', payload: { commandId: 'captured-command', threadId: 'captured-thread', projectId: 'a', initialMessage: { text: 'captured task', attachments: [] } },
    description: 'Captured launch', threadId: 'captured-thread', text: 'captured task', uncertain: true };
  expect(mobileNewTaskLaunchPrepare(f.client, pending)).not.toBeNull(); f.client.local.pending.env = pending;
  expect(mobileNewTaskLaunchPendingOwned(f.client)).toBe(true);
  const choosing = f.snapshot('/new', 'next', 'next-flow');
  expect((await f.action(choosing.owner, 'project', '["env","b"]', '', 'next')).message).toBe('');
  const next = f.snapshot('/new/draft', 'next', 'next-flow'); expect(next.ready).toBe(true);
  const before = JSON.stringify(f.client.pending), calls = f.calls.length; f.client.busy = true;
  expect((await act(next.owner, 'next', 'interaction', next.draftOwner, 'plan', f.native, f.files, f.client, f.fleet, 0, localInteractionPreferences)).message).toBe('');
  expect(f.client.draftKey).not.toBe(originalKey); expect(f.client.interactionMode).toBe('plan');
  expect(JSON.stringify(f.client.pending)).toBe(before); expect(f.calls.slice(calls)).toEqual([]);
});

test('NewTask local interaction stays local while another draft owns a recording', async () => {
  resetVoiceInputGlobalsForTests();
  const f = await localInteractionStorage(), recordingKey = f.client.draftKey, voiceCalls: Obj[] = [];
  const native: Native = { ...f.native, async later(input) {
    const request = obj(input);
    if (request.op !== 'mobileVoice') return f.native.later(input);
    voiceCalls.push(request);
    const value = request.action === 'status' ? { available: true, locale: 'en-US', event: 0, levels: [], elapsed: 0 }
      : request.action === 'permission' ? { granted: true, canAskAgain: true }
      : request.action === 'prepare' ? { locale: 'en-US' }
      : request.action === 'recorder-prepare' || request.action === 'stop' ? { uri: 'file:///new-task-mode-fixture.m4a' } : {};
    return { ok: true, generation: f.client.generation, value };
  } };
  try {
    const started = await mobileVoiceAction('start', 0, 0, 'Recording draft A', recordingKey, native, f.files, f.client);
    expect(started.message).toBe(''); expect(started.data).toMatchObject({ phase: 'recording', owner: recordingKey, busy: true, focused: true });
    const chooser = f.snapshot('/new', 'recording-next', 'recording-next-flow');
    expect((await f.action(chooser.owner, 'project', '["env","b"]', '', 'recording-next')).message).toBe('');
    const ready = f.snapshot('/new/draft', 'recording-next', 'recording-next-flow');
    expect(ready.ready).toBe(true); expect(f.client.draftKey).not.toBe(recordingKey);
    expect(mobileVoiceBlocksSubmission(f.client)).toBe(true);
    const voice = mobileVoiceSnapshot(f.client.draftKey, f.client);
    expect(voice).toMatchObject({ phase: 'recording', owner: recordingKey, busy: true, focused: false, globalVisible: true });
    const recording = JSON.stringify(mobileNewTaskDraftPresentation(f.client, recordingKey));
    const requests = f.calls.length, voiceRequests = voiceCalls.length, writes = f.writes.length, key = f.client.draftKey;
    expect(mobileNewTask('', f.client, f.fleet, localInteractionPreferences).planModeEnabled).toBe(true);
    expect((await act(ready.owner, 'recording-next', 'interaction', ready.draftOwner, 'plan', native, f.files,
      f.client, f.fleet, 0, localInteractionPreferences)).message).toBe('');
    expect(f.client.interactionMode).toBe('plan'); expect(localInteractionChoice(f.writes.at(-1)!, key)).toBe('plan'); expect(f.writes.length - writes).toBe(1);
    expect(JSON.stringify(mobileNewTaskDraftPresentation(f.client, recordingKey))).toBe(recording);
    expect(mobileVoiceSnapshot(key, f.client)).toEqual(voice);
    expect(f.calls.slice(requests)).toEqual([]); expect(voiceCalls.slice(voiceRequests)).toEqual([]);
  } finally {
    try { await mobileVoiceAction('cancel', 0, 0, 'Recording draft A', recordingKey, native, f.files, f.client); }
    finally { resetVoiceInputGlobalsForTests(); }
  }
});
