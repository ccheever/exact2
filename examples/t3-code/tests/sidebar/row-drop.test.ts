// A sidebar row's files and actions on its own environment (T3 Code 6d450a4284, MIT, see LICENSE-T3): files dropped
// on a row (sidebarPendingFileDropStore and ChatView's consumePendingFileDrop; the files' bytes held in memory,
// app-files-fixture.ts), "New thread on <branch>" (handleNewThread with the branch), the composer taking the focus on a
// thread change, and Mark unread's silent failures on this and a background environment.
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { ClientError, type Files, type Native } from '../../src/client/protocol';
import { toasts } from '../../src/shell/toast';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { sidebarPrefs } from '../../src/sidebar/sidebar-state';
import { sidebarCommand } from '../../src/sidebar/sidebar-commands';
import { draftContext } from '../../src/composer/composer-controls-branch';
import { draftFiles } from '../../src/composer/composer-editor-files';
import { clearRowDrops, queueRowDrop, rowDropKeys, takeRowDrops } from '../../src/sidebar/row-drop';
import { memoryFiles, png, text } from '../support/app-files-fixture';
import { STAGED } from '../../src/composer/composer-attach-stage';
import { threadOps } from '../../src/client/client-ops-threads';
import type { OpOut } from '../../src/client/client-ops';
import { terminalUiStore } from '../../src/terminal/terminal-ui-state';
import { acceptComposerEdit, observeComposer } from '../support/composer-field-fixture';

const at = (minute: number) => `2026-03-09T10:${String(minute).padStart(2, '0')}:00.000Z`;
const files: Files = { fs: { async mkdir() {}, async atomicWriteFile() {}, async readFile() { return new ArrayBuffer(0); } } };
const obj = (value: unknown): Obj => (value && typeof value === 'object' ? value as Obj : {});
const thread = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null, activeProviderThreadId: null,
  pendingRuntimeRequest: null, createdAt: at(0), updatedAt: at(0), archivedAt: null, settledOverride: null, lineage: { relationshipToParent: null }, ...extra });
const repo = { provider: 'github', canonicalKey: 'github.com/acme/app', displayName: 'acme/app' };
const PR = 'https://github.com/acme/app/pull/7';
const linked = (url = PR) => [{ host: 'github.com', repository: 'acme/app', number: 7, url, source: 'manual', linkedAt: at(1), stack: null,
  snapshot: { state: 'open', title: 'Ship it', headBranch: 'feat', baseBranch: 'main', isDraft: false, updatedAt: at(1), syncedAt: at(1) } }];

interface Rig { client: T3Client; native: Native; ops: Obj[]; rpcs: { method: string; payload: Obj }[]; opened: string[]; drafts: string[] }
/** A real client on `local` (thread-1 open unless said), its server answering preview and dispatch calls, its module recording ops. */
function rig(options: { threads?: Obj[]; projects?: Obj[]; threadId?: string; settings?: Obj; failOpen?: boolean; caps?: Obj; presentation?: Obj; editorApplies?: boolean } = {}): Rig {
  const client = new T3Client(), ops: Obj[] = [], rpcs: { method: string; payload: Obj }[] = [], opened: string[] = [], drafts: string[] = [];
  let ids = 0;
  Object.assign(client, { loaded: true, connection: 'connected', origin: 'http://127.0.0.1:16562', configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate'], presentation: options.presentation ?? {} });
  client.environmentId = 'local'; client.threadId = options.threadId ?? 'thread-1'; client.projectId = 'p1';
  client.shell = { projects: options.projects ?? [{ id: 'p1', title: 'App', workspaceRoot: '/work/app', repositoryIdentity: repo, createdAt: at(0), updatedAt: at(0) }], threads: options.threads ?? [thread('thread-1'), thread('thread-2', { pullRequests: linked() })], sequence: 1 };
  client.config = { environment: { label: 'This Mac', capabilities: { pullRequests: true, threadPullRequests: true, threadVisitedTracking: true, serverResolvedCommandContext: true, ...(options.caps ?? {}) } }, settings: {}, providers: [], keybindings: [] };
  client.local.clientSettings = { ...client.local.clientSettings, browserLinkTarget: 'system', confirmThreadDelete: true, ...(options.settings ?? {}) } as typeof client.local.clientSettings;
  client.request = async (_native, method, payload) => {
    rpcs.push({ method, payload });
    if (method === 'preview.list') return { sessions: [], serverEpoch: 'epoch-1', revision: 1 };
    if (method === 'preview.open') {
      if (options.failOpen) throw new Error('The preview server refused.');
      return { threadId: String(payload.threadId), tabId: 'tab-1', navStatus: { _tag: 'Loading', url: String(payload.url), title: '' }, canGoBack: false, canGoForward: false, updatedAt: at(2) };
    }
    return {};
  };
  Object.assign(client, {
    async openSelected(_native: Native, id: string) { opened.push(id); client.threadId = id; },
    async openDraft(_native: Native, projectId: string) { drafts.push(projectId); client.threadId = ''; client.projectId = projectId; },
    adoptStatus(value: Obj) { ops.push({ op: 'adopted', value }); },
  });
  const native: Native = { available: true, watch() {}, async later(request) {
    const op = obj(request); ops.push(op);
    if (op.op === 'ids') return { ok: true, generation: 0, value: Array.from({ length: Number(op.count) }, () => `00000000-0000-4000-8000-${String(ids++).padStart(12, '0')}`) };
    if (op.op === 'editorInsert') return { ok: true, generation: 0, value: { applied: options.editorApplies === true && acceptComposerEdit(client, op) } };
    if (op.op === 'subscribe') return { ok: true, generation: Number(op.generation ?? 0), value: { id: `sub-${ops.length}` } };
    const generation = op.fleet ? 3 : Number(op.generation ?? 0);
    return { ok: true, generation, value: op.op === 'connect' ? { state: 'connecting' } : {} };
  } };
  return { client, native, ops, rpcs, opened, drafts };
}
const opsOf = (ops: Obj[], name: string) => ops.filter(op => op.op === name);
/** A row's drop (sidebar-row.contract filesDropped): `files` are the drop's `doc:` handles, which the app's storage holds. */
const dropOn = (client: T3Client, key: string, files: string[], thread = key.split(':').at(-1)!, open = false) =>
  queueRowDrop(client, `${open ? '1' : '0'}\t${thread}\t${key}`, { files, picked: [], asText: false, target: '' });
const fleetRequests = (ops: Obj[]) => ops.filter(op => op.fleet === 'remote' && op.op === 'request').map(op => ({ method: op.method, payload: obj(op.payload) }));

/** A background environment the fleet keeps connected: Studio, with the shared repository and a solo project. */
function background(threads: Obj[] = [], projects?: Obj[]): FleetEntry {
  const entry = { key: 'remote', origin: 'http://studio.local:3773', environmentId: 'env-remote', phase: 'connected', message: '', traceId: '', generation: 3, synchronized: 3, lastEvent: 0,
    subscriptions: {}, config: { environment: { label: 'Studio', capabilities: { threadVisitedTracking: true } }, settings: {} },
    shell: { projects: projects ?? [{ id: 'r1', title: 'solo', workspaceRoot: '/srv/solo', createdAt: at(0), updatedAt: at(0) }], threads, sequence: 1 }, scopes: [], error: '', requested: false } as unknown as FleetEntry;
  fleet.entries.set('remote', entry);
  return entry;
}
afterEach(() => { fleet.entries.clear(); });

describe('files dropped on a row (consumePendingFileDrop, addDroppedFiles)', () => {
  test('a drop opens a row that is not the open thread\'s; folders alone queue nothing', async () => {
    const { client } = rig({ threadId: 'thread-1' });
    memoryFiles({ 'doc:/1/notes.txt': text('notes'), 'doc:/2/src': 'folder' });
    expect(await dropOn(client, 'fleet:local:thread-2', ['doc:/2/src'])).toBe('');
    expect(await dropOn(client, 'fleet:local:thread-2', ['doc:/1/notes.txt', 'doc:/2/src'])).toBe('open:thread-2');
    expect(await dropOn(client, 'fleet:local:thread-1', ['doc:/1/notes.txt'], 'thread-1', true)).toBe('');
    expect(rowDropKeys(client)).toEqual(['fleet:local:thread-2', 'fleet:local:thread-1']);
  });
  test('once the dropped-on thread is open, its queued files join its composer: images to the shelf, files as chips', async () => {
    const { client, native, ops } = rig({ threadId: 'thread-2', editorApplies: true });
    observeComposer(client, '');
    memoryFiles({ 'doc:/1/notes.txt': text('notes'), 'doc:/2/shot.png': png(4, 3), 'doc:/3/layers.psd': text('8BPS') });
    await dropOn(client, 'fleet:local:thread-9', ['doc:/1/notes.txt']);
    await dropOn(client, 'fleet:local:thread-2', ['doc:/1/notes.txt', 'doc:/2/shot.png', 'doc:/3/layers.psd']);
    expect(await takeRowDrops(client, native)).toBe(true);
    expect(draftFiles(client.local).map(file => [file.name, file.draftKey])).toEqual([['notes.txt', 'local:thread-2']]);
    expect(client.local.snapshotDrafts['local:thread-2']?.map(image => image.name)).toEqual(['shot.png']);
    expect(opsOf(ops, 'editorInsert')[0]).toMatchObject({ owner: client.snapshotOwner });
    expect(client.local.drafts['local:thread-2']).toContain('notes.txt');
    expect(client.error).toBe("'layers.psd' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.");
    expect(rowDropKeys(client)).toEqual(['fleet:local:thread-9']);
  });
  test('a pending question that takes no attachments refuses a row drop, as any attachment (addDroppedFiles)', async () => {
    const { client, native } = rig({ threadId: 'thread-2' });
    // The open thread's projection, with a question that has no custom answer (requests.ts requestPresentation).
    Object.assign(client, { thread: { projection: { runtimeRequests: [{ id: 'q', kind: 'user_input', status: 'pending', responseCapability: { type: 'live' } }],
      turnItems: [{ id: 'q-item', type: 'user_input_request', requestId: 'q', questions: [{ id: 'one', question: 'Pick one', allowCustomAnswer: false, options: [{ label: 'A' }] }] }] } } });
    const { store } = memoryFiles({ 'doc:/1/notes.txt': text('notes') });
    await dropOn(client, 'fleet:local:thread-2', ['doc:/1/notes.txt']);
    expect(await takeRowDrops(client, native)).toBe(false);
    expect(toasts(client).map(toast => toast.title)).toEqual(['This question cannot accept attachments.']);
    expect([...store.keys()].filter(path => path.startsWith(STAGED))).toEqual([]);
    expect(rowDropKeys(client)).toEqual([]);
  });
  test('nothing queued for the open thread stages nothing', async () => {
    const { client, native } = rig({ threadId: 'thread-1' });
    const { store } = memoryFiles({ 'doc:/1/notes.txt': text('notes') });
    await dropOn(client, 'fleet:local:thread-2', ['doc:/1/notes.txt']);
    expect(await takeRowDrops(client, native)).toBe(false);
    expect([...store.keys()].filter(path => path.startsWith(STAGED))).toEqual([]);
  });
});

describe('"New thread on <branch>" carries the branch (handleNewThread with branch, worktreePath, envMode)', () => {
  test('a worktree thread\'s draft reuses its worktree; a plain one takes its branch on the local checkout', async () => {
    const { client, native, drafts } = rig({ threads: [thread('thread-1'), thread('wt', { branch: 'feature/x', worktreePath: '/work/.t3/wt' }), thread('local', { branch: 'fix/y' })] });
    expect(await sidebarCommand(client, native, files, 'menu-choice', 'wt', 'new-thread-on-branch')).toBe('sidebar:new-thread');
    expect(drafts).toEqual(['p1']);
    expect(draftContext(client)).toEqual({ envMode: 'worktree', branch: 'feature/x', worktreePath: '/work/.t3/wt' });
    expect(await sidebarCommand(client, native, files, 'menu-choice', 'local', 'new-thread-on-branch')).toBe('sidebar:new-thread');
    expect(draftContext(client)).toEqual({ envMode: 'local', branch: 'fix/y', worktreePath: '' });
  });
});

describe('a thread change focuses the composer (ChatView\'s effect on activeThread.id)', () => {
  const select = async (client: T3Client, native: Native, id: string, how = 'click') => {
    const out = { message: '', id: '', value: '' } as OpOut;
    await threadOps.call(client, 'select-thread', id, how, 0, native, files, out);
    return out.message;
  };
  test('a row press that opens another thread focuses the composer; the open thread\'s own row and a ⌘-click do not', async () => {
    const { client, native } = rig();
    expect(await select(client, native, 'thread-2')).toBe('focus:composer');
    expect(await select(client, native, 'thread-2')).toBe(''); // no thread change: the focus stays on the row
    const held = rig();
    expect(await select(held.client, held.native, 'thread-2', 'click:meta')).toBe(''); // a selection toggle opens nothing
    expect(await select(client, native, 'gone')).toBe(''); // a thread the shell does not hold: the empty state, no composer
  });
  test('not while the opened thread\'s terminal drawer is open, which owns the keys', async () => {
    const { client, native } = rig();
    terminalUiStore(client).getState().setTerminalOpen({ environmentId: 'local', threadId: 'thread-2' }, true);
    expect(await select(client, native, 'thread-2')).toBe('');
  });
});

describe('drops whose thread did not open (handleThreadFileDrop)', () => {
  test('a drop queued for a thread that is not open goes once another thread opens; one whose thread opens is taken', async () => {
    const { client, native } = rig({ threadId: 'thread-1' });
    memoryFiles({ 'doc:/1/notes.txt': text('notes') });
    await dropOn(client, 'fleet:local:thread-3', ['doc:/1/notes.txt']);
    expect(await takeRowDrops(client, native)).toBe(false); // its open is still on the way
    expect(rowDropKeys(client)).toEqual(['fleet:local:thread-3']);
    client.threadId = 'thread-2'; // the open found the app busy, or another thread opened first
    expect(await takeRowDrops(client, native)).toBe(false);
    expect(rowDropKeys(client)).toEqual([]);
    const landed = rig({ threadId: 'thread-1' });
    await dropOn(landed.client, 'fleet:local:thread-3', ['doc:/1/notes.txt']);
    await takeRowDrops(landed.client, landed.native);
    landed.client.threadId = 'thread-3';
    expect(await takeRowDrops(landed.client, landed.native)).toBe(true);
    expect(draftFiles(landed.client.local).map(file => file.name)).toEqual(['notes.txt']);
  });
  test('a press whose thread fails to open lets the row\'s drop go', async () => {
    const { client, native } = rig();
    memoryFiles({ 'doc:/1/notes.txt': text('notes') });
    await dropOn(client, 'fleet:env-gone:t1', ['doc:/1/notes.txt']);
    const out = { message: '', id: '', value: '' } as OpOut;
    await expect(threadOps.call(client, 'select-thread', 'fleet:env-gone:t1', 'click', 0, native, files, out)).rejects.toThrow('That environment is no longer connected.');
    expect(rowDropKeys(client)).toEqual([]);
  });

});

describe('Mark unread uses the reference silent failure policy', () => {
  test('a server refusal for a local thread with no completed run stays silent, while archive failures still toast', async () => {
    const { client, native } = rig();
    client.request = async () => { throw new Error('No completed run'); };
    await sidebarCommand(client, native, files, 'menu-choice', 'thread-1', 'mark-unread');
    expect(toasts(client)).toEqual([]);
    expect(sidebarPrefs(client).visited['thread-1']).toBeUndefined();
    await sidebarCommand(client, native, files, 'archive', 'thread-1', '');
    expect(toasts(client).at(-1)?.title).toBe('Failed to archive thread');
  });
  test('a background server refusal stays silent; a superseded mutation still propagates', async () => {
    const { client, native } = rig();
    background([thread('t9', { projectId: 'r1' })]);
    let failure: Error = new Error('No completed run');
    const original = native.later.bind(native);
    native.later = async request => obj(request).op === 'request' ? Promise.reject(failure) : original(request);
    await sidebarCommand(client, native, files, 'menu-choice', 'fleet:env-remote:t9', 'mark-unread');
    expect(toasts(client)).toEqual([]);
    expect(sidebarPrefs(client).visited['fleet:env-remote:t9']).toBeUndefined();
    failure = new ClientError('The answer was let go.', 'superseded');
    await expect(sidebarCommand(client, native, files, 'menu-choice', 'fleet:env-remote:t9', 'mark-unread')).rejects.toBe(failure);
  });
  test('the untracked fallback makes only completed local/background threads unread', async () => {
    const { client, native, rpcs } = rig({ caps: { threadVisitedTracking: false },
      threads: [thread('thread-1'), thread('done', { latestRunId: 'run', latestRunCompletedAt: at(3), status: 'completed' })] });
    const entry = background([thread('empty', { projectId: 'r1' }), thread('done', { projectId: 'r1', latestRunId: 'run', latestRunCompletedAt: at(3), status: 'completed' })]);
    obj(obj(entry.config.environment).capabilities).threadVisitedTracking = false;
    for (const id of ['thread-1', 'done', 'fleet:env-remote:empty', 'fleet:env-remote:done']) await sidebarCommand(client, native, files, 'menu-choice', id, 'mark-unread');
    expect(sidebarPrefs(client).visited).toMatchObject({ done: '2026-03-09T10:02:59.999Z', 'fleet:env-remote:done': '2026-03-09T10:02:59.999Z' });
    expect(sidebarPrefs(client).visited['thread-1']).toBeUndefined();
    expect(sidebarPrefs(client).visited['fleet:env-remote:empty']).toBeUndefined();
    expect(toasts(client)).toEqual([]);
    expect(rpcs).toEqual([]);
  });
});

describe('row drop ownership during an environment handoff', () => {
  test('a queued background drop waits for synchronization and attaches once in the destination, preserving file order', async () => {
    const { client, native, ops } = rig();
    memoryFiles({ 'doc:/1/notes.txt': text('notes'), 'doc:/2/other.txt': text('other') });
    await dropOn(client, 'fleet:env-remote:thread-1', ['doc:/1/notes.txt', 'doc:/2/other.txt']);
    expect(await takeRowDrops(client, native)).toBe(false); // same raw thread ID on the current environment
    T3Client.prototype.adoptStatus.call(client, { state: 'connected', origin: 'http://studio.local:3773', environmentId: 'env-remote', message: '', presentation: client.presentation }, 1);
    client.threadId = 'thread-1'; client.projectId = 'r1';
    expect(await takeRowDrops(client, native)).toBe(false); // selected, but shell/thread/config not ready
    expect(rowDropKeys(client)).toEqual(['fleet:env-remote:thread-1']);
    Object.assign(client, { configLive: true, shellLive: true, threadLive: true });
    const owner = client.snapshotOwner;
    expect(await takeRowDrops(client, native)).toBe(true);
    expect(draftFiles(client.local).map(file => [file.name, file.draftKey])).toEqual([['notes.txt', 'env-remote:thread-1'], ['other.txt', 'env-remote:thread-1']]);
    expect(opsOf(ops, 'editorInsert')[0]?.owner).toBe(owner);
    expect(client.local.drafts['local:thread-1']).toBeUndefined();
    expect(await takeRowDrops(client, native)).toBe(false);
  });
  test('a failed row press clears its queue', async () => {
    const { client, native } = rig();
    memoryFiles({ 'doc:/1/notes.txt': text('notes') });
    await dropOn(client, 'fleet:env-remote:gone', ['doc:/1/notes.txt']);
    await clearRowDrops(client, native, ['fleet:env-remote:gone']);
    expect(rowDropKeys(client)).toEqual([]);
  });
  test('navigation while the files are staged releases them instead of attaching to the new draft', async () => {
    const { client, native, ops } = rig();
    const { fs, released } = memoryFiles({ 'doc:/1/notes.txt': text('notes') });
    await dropOn(client, 'fleet:local:thread-1', ['doc:/1/notes.txt'], 'thread-1', true);
    const stat = fs.stat;
    fs.stat = async (path: string) => { client.threadId = 'thread-2'; return stat(path); };
    expect(await takeRowDrops(client, native)).toBe(false);
    expect(draftFiles(client.local)).toEqual([]);
    expect(opsOf(ops, 'editorInsert')).toEqual([]);
    expect(released()).toHaveLength(1);
  });
  test('a drop abandoned for another thread cannot attach on a later visit to it', async () => {
    const { client, native } = rig();
    memoryFiles({ 'doc:/1/abandoned.txt': text('gone'), 'doc:/2/kept.txt': text('kept') });
    await dropOn(client, 'fleet:local:abandoned', ['doc:/1/abandoned.txt']);
    await takeRowDrops(client, native);
    await dropOn(client, 'fleet:local:thread-2', ['doc:/2/kept.txt']);
    client.threadId = 'thread-2';
    expect(await takeRowDrops(client, native)).toBe(true);
    client.threadId = 'abandoned';
    expect(await takeRowDrops(client, native)).toBe(false);
    expect(rowDropKeys(client)).toEqual([]);
    expect(draftFiles(client.local).map(file => file.name)).toEqual(['kept.txt']);
  });
  test('an image-only drop preserves its original source and shelf after editor navigation refuses insertion', async () => {
    const { client, native, ops } = rig();
    memoryFiles({ 'doc:/1/shot.png': png(4, 3) });
    await dropOn(client, 'fleet:local:thread-1', ['doc:/1/shot.png'], 'thread-1', true);
    client.local.drafts['local:thread-1'] = 'Keep this image here';
    const original = native.later.bind(native), owner = client.snapshotOwner;
    let release: (() => void) | undefined;
    let entered: (() => void) | undefined;
    const inserting = new Promise<void>(resolve => { entered = resolve; });
    native.later = async request => {
      const op = obj(request);
      if (op.op === 'editorInsert') await new Promise<void>(resolve => { release = resolve; entered!(); });
      return original(request);
    };
    const taking = takeRowDrops(client, native);
    await inserting;
    expect(release).toBeDefined();
    client.threadId = 'thread-2';
    release!();
    expect(await taking).toBe(true);
    expect(client.local.snapshotDrafts['local:thread-1']?.map(image => image.name)).toEqual(['shot.png']);
    expect(client.local.drafts['local:thread-1']).toBe('Keep this image here');
    expect(client.local.drafts['local:thread-2']).toBeUndefined();
    expect(client.local.snapshotDrafts['local:thread-2']).toBeUndefined();
    expect(client.error).toBe('');
    expect(opsOf(ops, 'editorInsert')[0]?.owner).toBe(owner);
  });
});
