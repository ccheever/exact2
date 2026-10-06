import { expect, test } from 'bun:test';
import { heroProjects, heroHeadline, landingKind, mostRecentProjectId, heroCarry, heroLand, defaultPick, isScratchProject, threadActivity } from './pages-home';
import type { Obj, Shell } from './domain';

const shell = (projects: Obj[], threads: Obj[] = []): Shell => ({ projects, threads, sequence: 1 });
const providers = [{ instanceId: 'codex', driver: 'codex', status: 'ready', enabled: true, installed: true, auth: { status: 'authenticated' },
  models: [{ slug: 'gpt-a', isDefault: true }, { slug: 'gpt-b' }] }];

test('project activity follows the latest user message, then updates, ignoring archived threads', () => {
  expect(threadActivity({ latestUserMessageAt: '2026-10-01T00:00:00Z', updatedAt: '2026-10-03T00:00:00Z' })).toBe(Date.parse('2026-10-01T00:00:00Z'));
  expect(threadActivity({ updatedAt: '2026-10-03T00:00:00Z' })).toBe(Date.parse('2026-10-03T00:00:00Z'));
  const value = shell([{ id: 'a', title: 'Alpha', updatedAt: '2026-01-01T00:00:00Z' }, { id: 'b', title: 'Beta' }, { id: 'c', title: 'Gamma', updatedAt: '2026-09-01T00:00:00Z' }], [
    { id: 't1', projectId: 'b', latestUserMessageAt: '2026-10-02T00:00:00Z' },
    { id: 't2', projectId: 'a', latestUserMessageAt: '2026-10-03T00:00:00Z', archivedAt: '2026-10-03T01:00:00Z' },
  ]);
  expect(mostRecentProjectId(value)).toBe('b');
  expect(mostRecentProjectId(shell([]))).toBe('');
});

test('the hero menu puts the open project first and leaves the Scratch project to "No project"', () => {
  const client = { projectId: 'b', config: { scratchWorkspaceRoot: '/data/scratch/' }, shell: shell([
    { id: 'a', title: 'Parity fixture', workspaceRoot: '/w/a', updatedAt: '2026-10-03T00:00:00Z' },
    { id: 'b', title: 'Single checkout two', workspaceRoot: '/w/b', updatedAt: '2026-10-01T00:00:00Z' },
    { id: 's', title: 'Scratch', workspaceRoot: '/data/scratch', updatedAt: '2026-10-04T00:00:00Z' },
  ]) };
  const rows = heroProjects(client as never);
  expect(rows.map(row => [row.id, row.selected])).toEqual([['b', true], ['a', false]]);
  expect(rows[1]).toMatchObject({ name: 'Parity fixture', mark: 'PF' });
  expect(isScratchProject({ workspaceRoot: '/data/scratch' }, '/data/scratch/')).toBe(true);
});

test('headline and landing follow DraftHeroHeadline and the index route', () => {
  expect(heroHeadline({ scratchDraft: true, resolved: true, canChoose: true })).toBe('work');
  expect(heroHeadline({ scratchDraft: false, resolved: true, canChoose: true })).toBe('build');
  expect(heroHeadline({ scratchDraft: false, resolved: false, canChoose: true })).toBe('start');
  expect(heroHeadline({ scratchDraft: false, resolved: false, canChoose: false })).toBe('add');
  const base = { connected: true, ready: true, savedEnvironments: 1, connecting: false, projects: 2, projectId: 'a', threadId: '' };
  expect(landingKind(base)).toBe('');
  expect(landingKind({ ...base, connected: false, savedEnvironments: 0 })).toBe('no-environment');
  expect(landingKind({ ...base, connected: false, savedEnvironments: 0, connecting: true })).toBe('offline');
  expect(landingKind({ ...base, connected: false })).toBe('offline');
  expect(landingKind({ ...base, projects: 0, projectId: '' })).toBe('no-projects');
  expect(landingKind({ ...base, projectId: '' })).toBe('start-error');
});

function draftClient(over: Obj = {}) {
  return { ready: true, writable: true, threadId: '', projectId: 'a', environmentId: 'env', get draftKey() { return `env:new:${this.projectId}`; },
    providerId: 'codex', modelId: 'gpt-a', modelOptions: [] as Obj[], runtimeMode: 'approval-required', interactionMode: 'default',
    config: { providers, settings: {} }, local: { drafts: { 'env:new:a': 'build the thing' } as Record<string, string>, snapshotDrafts: {} as Record<string, Obj[]> },
    shell: shell([{ id: 'a', title: 'A' }, { id: 'b', title: 'B' }]), ...over };
}

test('retargeting a draft carries its prompt; the new project default applies unless a model was picked', async () => {
  const client = draftClient();
  const carry = await heroCarry(client as never, 'project', 'b', async () => '');
  expect(carry).toMatchObject({ projectId: 'b', text: 'build the thing', explicit: null });
  client.projectId = 'b';
  heroLand(client as never, carry);
  expect(client.local.drafts).toEqual({ 'env:new:b': 'build the thing' });

  const picked = draftClient({ modelId: 'gpt-b' });
  const explicit = await heroCarry(picked as never, 'project', 'b', async () => '');
  expect(explicit.explicit).toMatchObject({ providerId: 'codex', modelId: 'gpt-b' });
  picked.projectId = 'b'; picked.modelId = 'gpt-a';
  heroLand(picked as never, explicit);
  expect(picked.modelId).toBe('gpt-b');

  const same = draftClient({ modelId: 'gpt-a' });
  const kept = await heroCarry(same as never, 'project', 'a', async () => '');
  same.modelId = 'gpt-b';
  heroLand(same as never, kept);
  expect([same.modelId, same.local.drafts['env:new:a']]).toEqual(['gpt-a', 'build the thing']);
});

test('a sticky model counts as the default, not as an explicit pick', () => {
  const client = draftClient({ local: { drafts: {}, snapshotDrafts: {}, composerControls: { stickyProvider: 'codex', stickyByProvider: { codex: { model: 'gpt-b', options: [] } }, stickyOptionsByModel: {}, staged: {}, wokeSeen: {} } } });
  expect(defaultPick(client as never, 'a')).toEqual({ providerId: 'codex', modelId: 'gpt-b' });
});

test('retargeting refuses started threads, vanished projects and servers without Scratch', async () => {
  await expect(heroCarry(draftClient({ threadId: 't1' }) as never, 'project', 'b', async () => '')).rejects.toThrow('Choose a project from a new thread.');
  await expect(heroCarry(draftClient() as never, 'project', 'gone', async () => '')).rejects.toThrow('That project is no longer available.');
  await expect(heroCarry(draftClient() as never, 'scratch', '', async () => 's')).rejects.toThrow('cannot start threads without a project');
  const scratch = draftClient({ config: { providers, settings: {}, scratchWorkspaceRoot: '/s' } });
  scratch.shell = shell([{ id: 'a', title: 'A' }, { id: 's', title: 'Scratch', workspaceRoot: '/s' }]);
  expect((await heroCarry(scratch as never, 'scratch', '', async () => 's')).projectId).toBe('s');
  const retry = await heroCarry(draftClient() as never, 'retry', '', async () => '');
  expect(retry).toMatchObject({ projectId: 'a', fromKey: '' });
});
