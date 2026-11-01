import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { arr, obj, str, type Obj } from '../../src/client/domain';
import { Backend, storage } from '../support/client-fixture';
import { promptLinkView } from '../../src/prompt-links/prompt-link-handler';
import { readPairingAppLink } from '../../src/connections/remote';
import { toasts } from '../../src/shell/toast';
import { paletteCommand } from '../../src/palette/palette-commands';
import { chatLocal } from '../../src/timeline/timeline-presentation';
import { fleet, environmentKey, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { surfaceStore } from '../../src/panel/panel-surfaces';
import { primary, type PrimaryTarget } from '../../src/connections/local-primary';

class LinkBackend extends Backend {
  previewRefusal = '';
  failIds = false;
  override status() { return { ...super.status(), environmentId: str(obj(this.config.environment).environmentId) }; }
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'connect' && request.origin === 'http://remote.test') {
      this.config = { ...this.config, environment: { ...obj(this.config.environment), environmentId: 'env2' } };
      this.shell = { snapshotSequence: 10, projects: [{ id: 'remote-project', title: 'Remote', workspaceRoot: '/remote' }], threads: [{ id: 'remote-thread', projectId: 'remote-project', title: 'Remote thread' }] };
      this.details['remote-thread'] = { ...this.details.t1, projection: { ...obj(this.details.t1.projection), thread: { ...obj(obj(this.details.t1.projection).thread), id: 'remote-thread', projectId: 'remote-project' } } };
    }
    if (request.op === 'request' && request.method === 'projectClone.start') {
      this.calls.push(request);
      const payload = obj(request.payload);
      this.shell.projects = [...arr(this.shell.projects), { id: payload.projectId, title: payload.title, workspaceRoot: payload.destinationPath,
        repositoryIdentity: { canonicalKey: 'github.com/missing/repo' } }];
      return this.good({ projectId: payload.projectId });
    }
    if (request.op === 'request' && request.method === 'projects.ensureScratch') {
      this.calls.push(request);
      this.shell.projects = [...arr(this.shell.projects).filter(project => project.id !== 'scratch'), { id: 'scratch', title: 'Scratch', workspaceRoot: '/scratch' }];
      return this.good({ projectId: 'scratch' });
    }
    if (request.op === 'pairEnvironment') {
      this.calls.push(request);
      if (request.credential === 'spent') return this.bad('The environment credential is invalid.');
      return this.good({ origin: request.origin, environmentId: 'env-paired', label: 'Paired' });
    }
    if (request.op === 'ids' && this.failIds) return this.bad('no ids');
    if (request.op === 'request' && request.method === 'preview.list') return this.good({ sessions: [], serverEpoch: 'epoch-1', revision: 1 });
    if (request.op === 'request' && request.method === 'preview.open') {
      this.calls.push(request);
      if (this.previewRefusal) return this.bad(this.previewRefusal);
      const payload = obj(request.payload);
      return this.good({ threadId: payload.threadId, tabId: `tab-${++this.serial}`, navStatus: { _tag: 'Loading', url: str(payload.url), title: '' }, canGoBack: false, canGoForward: false, updatedAt: '2026-10-21T00:00:00.000Z' });
    }
    if (request.op === 'request' && request.method === 'assets.createUrl') { this.calls.push(request); return this.good({ relativeUrl: '/api/assets/token/page.html' }); }
    if (request.op === 'browserSync') return this.good({});
    if (request.op === 'request' && request.method === 'git.preparePullRequestThread') {
      this.calls.push(request);
      return this.good({ branch: 'pr-7', worktreePath: '/worktrees/pr-7' });
    }
    return super.later(input);
  }
}
async function setup() {
  const client = new T3Client(), native = new LinkBackend(), disk = storage();
  native.config.scratchWorkspaceRoot = '/scratch';
  obj(arr(native.shell.projects)[0]).repositoryIdentity = { canonicalKey: 'github.com/me/repo' };
  await client.refresh(native, disk.files);
  const command = (op: string, id = '', value = '') => client.command(op, id, value, 0, native, disk.files);
  const open = (url: string) => command('shelllocal:prompt-link-open', '', url);
  return { client, native, disk, command, open };
}
function expectNotSent(native: Backend): void {
  expect(native.calls.filter(call => call.op === 'request').map(call => call.method)).not.toContain('orchestration.launchThread');
  expect(native.committed.some(payload => payload.type === 'message.dispatch')).toBe(false);
}

test('a plain link prepares No project, removes hidden characters, and never sends a message', async () => {
  const { client, native, open, disk } = await setup();
  const result = await open(`t3code://new?prompt=${encodeURIComponent('Read\u200b this first')}`);
  expect(result.message).toBe('sidebar:new-thread');
  expect(client.projectId).toBe('scratch');
  expect(client.threadId).toBe('');
  expect(client.draft).toBe('Read this first');
  expect(obj(disk.data().drafts)['env1:new:scratch']).toBe('Read this first');
  expect(toasts(client).some(toast => toast.kind === 'warning' && toast.timeoutMs === 0)).toBe(true);
  expectNotSent(native);
});

test('a repository link stages its provider/model while preserving an unfinished draft until Replace draft', async () => {
  const { client, native, open, command } = await setup();
  client.local.drafts['env1:new:p1'] = 'My work';
  await open('t3code://new?prompt=Linked+work&repo=me/repo&provider=codex&model=model-b');
  expect(client.projectId).toBe('p1');
  expect(client.draft).toBe('My work');
  expect([client.providerId, client.modelId]).toEqual(['codex-personal', 'model-b']);
  const notice = toasts(client).find(toast => toast.title === 'Linked prompt ready')!;
  await command('shelllocal:prompt-link-replace', String(notice.id));
  expect(client.draft).toBe('Linked work');
  expectNotSent(native);
});

test('a repository link opened from a thread lands in the project\'s new-thread draft, not that thread', async () => {
  const { client, native, disk, open, command } = await setup();
  await command('select-thread', 't1');
  await client.refresh(native, disk.files);
  expect(client.threadId).toBe('t1');
  await open('t3code://new?prompt=Linked+work&repo=me/repo');
  expect(client).toMatchObject({ projectId: 'p1', threadId: '' });
  expect(client.local.drafts['env1:new:p1']).toBe('Linked work');
  expect(client.local.drafts['env1:t1'] ?? '').toBe('');
  expectNotSent(native);
});

test('a missing repository requires a clone or No project choice and unavailable providers explain the fallback', async () => {
  const { client, native, open, command } = await setup();
  await open('t3code://new?prompt=Read+me&repo=missing/repo&provider=cursor');
  expect(promptLinkView(client)).toMatchObject({ show: true, clone: true, withoutProject: true, title: "You don't have missing/repo" });
  expect(native.calls.some(call => call.method === 'projectClone.start')).toBe(false);
  await command('shelllocal:prompt-link-action', 'without-project');
  expect(client.projectId).toBe('scratch');
  expect(client.draft).toBe('Read me');
  expect(toasts(client).at(-1)?.description).toContain('cursor, which is not set up here, so your default model is used.');
  expect(promptLinkView(client).show).toBe(false);
  expectNotSent(native);
});

test('thread links and matching PR links open an existing thread and leave their prompts unsent', async () => {
  const { client, native, open, disk } = await setup();
  await open('t3code://threads/env1/t1?prompt=Review+this');
  expect(client.threadId).toBe('t1');
  expect(client.draft).toBe('Review this');
  await client.refresh(native, disk.files);
  Object.assign(client.shell.threads.find(thread => thread.id === 't2')!, { updatedAt: '2026-10-18', pullRequests: [{ host: 'github.com', repository: 'me/repo', number: 7 }] });
  await open('t3code://new?pr=me/repo/7&prompt=Review+PR');
  expect(client.threadId).toBe('t2');
  expect(client.draft).toBe('Review PR');
  expect(native.calls.some(call => call.method === 'git.preparePullRequestThread')).toBe(false);
  expectNotSent(native);
});

test('a new PR checkout cannot run a setup script until the reader chooses Check out', async () => {
  const { client, native, open, command } = await setup();
  await open('t3code://new?pr=me/repo/7&prompt=Review+this+PR');
  expect(promptLinkView(client)).toMatchObject({ checkout: true, title: 'Check out me/repo#7?' });
  expect(native.calls.some(call => call.method === 'git.preparePullRequestThread')).toBe(false);
  await command('shelllocal:prompt-link-action', 'checkout');
  expect(native.calls.filter(call => call.method === 'git.preparePullRequestThread')).toHaveLength(1);
  expect(client.draft).toContain('Review this PR');
  expect(promptLinkView(client).show).toBe(false);
  expectNotSent(native);
});

test('cloning a missing repository happens only after its explicit action and opens an unsent draft', async () => {
  const { client, native, open, command } = await setup();
  client.config.newProjectsRoot = '/projects';
  obj(obj(client.config.environment).capabilities).projectCloneTracking = true;
  await open('t3code://new?prompt=Cloned+draft&repo=missing/repo');
  expect(native.calls.some(call => call.method === 'projectClone.start')).toBe(false);
  await command('shelllocal:prompt-link-action', 'clone');
  expect(native.calls.find(call => call.method === 'projectClone.start')?.payload).toMatchObject({ remoteUrl: 'https://github.com/missing/repo.git', destinationPath: '/projects/repo' });
  expect(client.draft).toBe('Cloned draft');
  expect(promptLinkView(client).show).toBe(false);
  expectNotSent(native);
});

test('a thread link switches to its connected background environment before opening the draft', async () => {
  const { client, native, open } = await setup();
  const key = environmentKey('http://remote.test', 'env2');
  const entry: FleetEntry = { key, origin: 'http://remote.test', environmentId: 'env2', phase: 'connected', message: '', traceId: '', generation: 1,
    synchronized: 1, lastEvent: 0, subscriptions: {}, config: { environment: { environmentId: 'env2' } },
    shell: { sequence: 1, projects: [{ id: 'remote-project', title: 'Remote', workspaceRoot: '/remote' }], threads: [{ id: 'remote-thread', projectId: 'remote-project', title: 'Remote thread' }] },
    scopes: ['orchestration:read', 'orchestration:operate'], error: '', requested: true };
  fleet.entries.set(key, entry);
  try {
    await open('t3code://threads/env2/remote-thread?prompt=Remote+draft');
    expect(promptLinkView(client).show).toBe(false);
    expect(client.environmentId).toBe('env2');
    expect(client.threadId).toBe('remote-thread');
    expect(client.local.drafts['env2:remote-thread']).toBe('Remote draft');
    expectNotSent(native);
  } finally { fleet.entries.delete(key); }
});

test('invalid and hidden-only prompts stay out of drafts', async () => {
  const { client, native, open } = await setup();
  await open('t3code://new?prompt=%E2%80%8B');
  expect(promptLinkView(client).description).toContain('only hidden characters');
  expect(client.draft).toBe('');
  await open('t3code://new?prompt=hello&repo=not-a-repo');
  expect(promptLinkView(client).description).toContain('repository T3 Code cannot read');
  expectNotSent(native);
});

test('palette and user-message actions copy shareable links through the native clipboard', async () => {
  const { client, native, command, disk } = await setup();
  await command('draft', '', 'Draft prompt');
  await paletteCommand(client, native, disk.files, 'copy-prompt-link', 'context', '');
  const draftLink = new URL(str(native.calls.findLast(call => call.op === 'copyText')?.text));
  expect(draftLink.origin).toBe('https://app.t3.codes');
  expect(draftLink.searchParams.get('prompt')).toBe('Draft prompt');
  expect(draftLink.searchParams.get('repo')).toBe('me/repo');
  await command('select-thread', 't1');
  client.projection.visibleTurnItems = [{ sourceThreadId: 't1', sourceItemId: 'user', item: { type: 'user_message', text: 'Message prompt' } }];
  await chatLocal(client, native, 'copy-prompt-link', JSON.stringify(['t1', 'user']), '', disk.files);
  expect(new URL(str(native.calls.findLast(call => call.op === 'copyText')?.text)).searchParams.get('prompt')).toBe('Message prompt');
  expectNotSent(native);
});

test('a server that withholds Scratch reports the unavailable draft without creating a project or sending', async () => {
  const { client, native, open } = await setup();
  delete client.config.scratchWorkspaceRoot;
  await open('t3code://new?prompt=Read+this');
  expect(promptLinkView(client)).toMatchObject({ show: true, title: 'Could not open the link',
    description: 'None of your machines can start a thread without a project right now.' });
  expect(native.calls.some(call => call.method === 'projects.ensureScratch')).toBe(false);
  expectNotSent(native);
});

test('without a recent or primary Scratch machine, multiple offerings require a choice rather than an arbitrary write', async () => {
  const { client, native, open } = await setup();
  delete client.config.scratchWorkspaceRoot;
  client.shell.threads = [];
  const keys: string[] = [];
  for (const id of ['remote-a', 'remote-b']) {
    const key = environmentKey(`http://${id}.test`, id); keys.push(key);
    fleet.entries.set(key, { key, origin: `http://${id}.test`, environmentId: id, phase: 'connected', message: '', traceId: '', generation: 1,
      synchronized: 1, lastEvent: 0, subscriptions: {}, config: { scratchWorkspaceRoot: '/scratch', environment: { environmentId: id } },
      shell: { sequence: 1, projects: [], threads: [] }, scopes: ['orchestration:read', 'orchestration:operate'], error: '', requested: true });
  }
  try {
    await open('t3code://new?prompt=Read+this');
    expect(promptLinkView(client).description).toBe('None of your machines can start a thread without a project right now.');
    expect(native.calls.some(call => call.method === 'projects.ensureScratch')).toBe(false);
    expectNotSent(native);
  } finally { for (const key of keys) fleet.entries.delete(key); }
});

test('a link without a project starts on This machine even when another machine was active more recently', async () => {
  const { client, native, open } = await setup();
  primary.target = { environmentId: 'env1', httpBaseUrl: 'http://127.0.0.1:3773' } as PrimaryTarget;
  const key = environmentKey('http://remote-a.test', 'remote-a');
  fleet.entries.set(key, { key, origin: 'http://remote-a.test', environmentId: 'remote-a', phase: 'connected', message: '', traceId: '', generation: 1,
    synchronized: 1, lastEvent: 0, subscriptions: {}, config: { scratchWorkspaceRoot: '/scratch', environment: { environmentId: 'remote-a' } },
    shell: { sequence: 1, projects: [{ id: 'rp', title: 'Remote', workspaceRoot: '/remote' }], threads: [{ id: 'rt', projectId: 'rp', title: 'Latest', updatedAt: '2999-01-01T00:00:00.000Z' }] },
    scopes: ['orchestration:read', 'orchestration:operate'], error: '', requested: true });
  try {
    expect((await open('t3code://new?prompt=Read+this')).message).toBe('sidebar:new-thread');
    expect(native.calls.some(call => call.method === 'projects.ensureScratch')).toBe(true);
    expect([client.environmentId, client.projectId]).toEqual(['env1', 'scratch']);
  } finally { fleet.entries.delete(key); primary.target = null; }
});

const previewOpens = (native: Backend) => native.calls.filter(call => call.method === 'preview.open').map(call => obj(call.payload));
const panelOf = (client: T3Client, threadId: string) => surfaceStore(client).panels.get(`env1:${threadId}`);

test('a web link opens a thread without a project with its Browser panel maximized; the next one joins that panel', async () => {
  const { client, native, open } = await setup();
  const first = await open('https://example.com/docs');
  expect(first.message).toBe('sidebar:new-thread');
  expect(client.projectId).toBe('scratch');
  const [opened] = previewOpens(native);
  expect(opened).toMatchObject({ url: 'https://example.com/docs' });
  const threadId = str(opened!.threadId);
  const panel = panelOf(client, threadId)!;
  expect([panel.visible, panel.maximized, panel.active.startsWith('browser:')]).toEqual([true, true, true]);
  // The thread on screen shows a Browser tab now, so the next link opens beside it, not in another new thread.
  expect((await open('http://localhost:5173/')).message).toBe('');
  expect(native.calls.filter(call => call.method === 'projects.ensureScratch')).toHaveLength(1);
  expect(previewOpens(native).at(-1)).toMatchObject({ threadId, url: 'http://localhost:5173/' });
  expect(promptLinkView(client).show).toBe(false);
  expectNotSent(native);
});

// DesktopWebLinkCoordinator: a pull request link a connected project reads opens the pull request view.
test('a pull request link for a connected project opens the Pull Requests page on it; other links open the browser', async () => {
  const { client, native, open } = await setup();
  obj(arr(native.shell.projects)[0]).repositoryIdentity = { provider: 'github', canonicalKey: 'github.com/me/repo', owner: 'me', name: 'repo', displayName: 'me/repo' };
  await client.refresh(native, storage().files);
  client.config = { ...client.config, environment: { ...obj(client.config.environment), capabilities: { ...obj(obj(client.config.environment).capabilities), pullRequests: true } } };
  const opened = await open('https://github.com/me/repo/pull/7');
  expect(opened.message.startsWith('sidebar:pr-page:')).toBe(true);
  expect(JSON.parse(opened.message.slice('sidebar:pr-page:'.length))).toMatchObject({ repository: 'me/repo', number: 7 });
  expect(previewOpens(native)).toHaveLength(0);
  // A repository no project reads, and the setting off, go to the browser panel.
  expect((await open('https://github.com/someone/else/pull/3')).message).toBe('sidebar:new-thread');
  (client.local as { clientSettings?: Obj }).clientSettings = { pullRequestLinksOpenInApp: false };
  expect((await open('https://github.com/me/repo/pull/7')).message.startsWith('sidebar:pr-page:')).toBe(false);
  expectNotSent(native);
});

test('an HTML file opens from its signed asset URL, and a link the browser refuses is a toast that names it', async () => {
  const { client, native, open } = await setup();
  primary.target = { environmentId: 'env1', httpBaseUrl: 'http://127.0.0.1:3773' } as PrimaryTarget;
  try {
    await open('file:///Users/me/My%20Page.html');
    expect(native.calls.find(call => call.method === 'assets.createUrl')?.payload).toMatchObject({ resource: { _tag: 'media-file', path: '/Users/me/My Page.html' } });
    expect(previewOpens(native).at(-1)?.url).toBe('http://127.0.0.1:3773/api/assets/token/page.html');
    native.previewRefusal = 'Preview is unavailable.';
    await open('https://example.com/');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not open the link', description: 'Preview is unavailable.' });
    expect(promptLinkView(client).show).toBe(false);
  } finally { primary.target = null; }
});

test('without This machine an HTML file opens nowhere: no other server is asked to serve it', async () => {
  const { client, native, open } = await setup();
  await open('file:///Users/me/page.html');
  expect(toasts(client).at(-1)).toMatchObject({ title: 'Could not open the link', description: 'file:///Users/me/page.html' });
  expect(native.calls.some(call => call.method === 'projects.ensureScratch' || call.method === 'assets.createUrl')).toBe(false);
});

test('a web link that cannot open its new thread is a toast, not the prompt-link dialog', async () => {
  const { client, native, open } = await setup();
  native.failIds = true;
  await open('https://example.com/');
  expect(toasts(client).at(-1)).toMatchObject({ title: 'Could not open the link' });
  expect(promptLinkView(client).show).toBe(false);
  expect(previewOpens(native)).toHaveLength(0);
});

test('a web link leaves an open prompt-link dialog open', async () => {
  const { client, open } = await setup();
  await open('t3code://new?prompt=Read+me&repo=missing/repo');
  expect(promptLinkView(client)).toMatchObject({ show: true, title: "You don't have missing/repo" });
  await open('https://example.com/');
  expect(promptLinkView(client)).toMatchObject({ show: true, title: "You don't have missing/repo" });
});

test('readPairingAppLink reads t3code://pair on any channel scheme and nothing else', () => {
  const pairingUrl = 'https://box.example.com/pair#token=abc';
  expect(readPairingAppLink(`t3code://pair?pairingUrl=${encodeURIComponent(pairingUrl)}`)).toBe(pairingUrl);
  expect(readPairingAppLink(`t3code-dev://pair/?pairingUrl=${encodeURIComponent(pairingUrl)}`)).toBe(pairingUrl);
  expect(readPairingAppLink('t3code://pair/extra?pairingUrl=x')).toBeNull();
  expect(readPairingAppLink('t3code://pair')).toBeNull();
  expect(readPairingAppLink('https://pair?pairingUrl=x')).toBeNull();
  expect(readPairingAppLink('t3code://new?prompt=x')).toBeNull();
});

test('a t3code://pair link asks to add the backend, spends its code only once confirmed, and is not paired twice', async () => {
  const { client, native, command, open } = await setup();
  const link = (url: string) => `t3code://pair?pairingUrl=${encodeURIComponent(url)}`;
  const pairs = () => native.calls.filter(call => call.op === 'pairEnvironment');
  await open(link('http://box.example.com:3773/pair#token=code-1'));
  expect(promptLinkView(client)).toMatchObject({ show: false, pairing: 'Add the backend at http://box.example.com:3773?' });
  await command('shelllocal:prompt-link-action', 'pair-cancel');
  expect(promptLinkView(client).pairing).toBe('');
  expect(pairs()).toHaveLength(0);
  await open(link('http://box.example.com:3773/pair#token=code-1'));
  await command('shelllocal:prompt-link-action', 'pair-confirm');
  expect(pairs().map(call => [call.origin, call.credential])).toEqual([['http://box.example.com:3773', 'code-1']]);
  expect(promptLinkView(client).pairing).toBe('');
  expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Backend added', description: 'The environment is saved and will reconnect on app startup.' });
  // A repeated click asks nothing: its one-time code is spent.
  await open(link('http://box.example.com:3773/pair#token=code-1'));
  expect(promptLinkView(client).pairing).toBe('');
  // A refused code and a link without a token are the "Could not add backend" toast.
  await open(link('http://box.example.com:3773/pair#token=spent'));
  await command('shelllocal:prompt-link-action', 'pair-confirm');
  expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not add backend', description: 'The environment credential is invalid.' });
  await open(link('http://box.example.com:3773/pair'));
  expect(promptLinkView(client).pairing).toBe('');
  expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not add backend', description: 'Pairing URL is missing its token.' });
});

