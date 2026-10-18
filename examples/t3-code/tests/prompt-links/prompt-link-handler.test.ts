import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { arr, obj, str, type Obj } from '../../src/client/domain';
import { Backend, storage } from '../support/client-fixture';
import { promptLinkView } from '../../src/prompt-links/prompt-link-handler';
import { toasts } from '../../src/shell/toast';
import { paletteCommand } from '../../src/palette/palette-commands';
import { chatLocal } from '../../src/timeline/timeline-presentation';
import { fleet, environmentKey, type FleetEntry } from '../../src/connections/settings-b-fleet';

class LinkBackend extends Backend {
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
