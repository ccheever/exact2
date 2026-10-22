import { expect, test } from 'bun:test';
import { mobileProjectFaviconTarget } from './mobile-project-favicon';
import { mobileHomeProjects, projectMobileHome, type HomeSource } from './home';
import { mobileNewTaskProjectIcon } from './new-task';
import { initialShell, type Obj } from './shared/domain';
import type { MobilePendingTask } from './mobile-outbox-presentation';

const now = Date.parse('2026-10-09T12:00:00Z');
const source = (environmentId: string, project: Obj): HomeSource => ({ environmentId, label: environmentId,
  focused: environmentId === 'focused', connected: false, machine: 'laptop', config: {},
  shell: { ...initialShell(), projects: [project], threads: [{ id: 'same', projectId: 'p', title: 'Thread', archivedAt: null }] } });

test('surface target preserves exact path identity and suppresses absent projects, roots and overrides', () => {
  expect(mobileProjectFaviconTarget('env', { workspaceRoot: '/a/../repo ', faviconPath: ' icon.svg' })).toEqual({
    environmentId: 'env', cwd: '/a/../repo ', faviconPath: ' icon.svg', key: '["env","/a/../repo "," icon.svg"]' });
  for (const project of [undefined, {}, { workspaceRoot: '' }, ...['emoji', 'monogram', 'lucide'].map(kind => ({ workspaceRoot: '/repo', projectIcon: { kind } }))]) {
    expect(mobileProjectFaviconTarget('env', project)).toEqual({ key: '', environmentId: '', cwd: '', faviconPath: '' });
  }
});

test('Home and sidebar projections use each thread, draft and pending task project even offline', () => {
  const sources = [source('focused', { id: 'p', title: 'Focused', workspaceRoot: '/focus' }),
    source('background', { id: 'p', title: 'Background', workspaceRoot: '/background', faviconPath: 'bg.svg' })];
  const pending: MobilePendingTask = { owner: 'pending-owner', status: 'pending', canRetry: false, reason: '', record: {
    schemaVersion: 1, origin: 'https://background.test', environmentId: 'background', threadId: 'queued', messageId: 'm', commandId: 'c',
    text: 'Queued', attachments: [], createdAt: new Date(now).toISOString(),
    creation: { projectId: 'p', projectTitle: 'Saved', workspaceMode: 'local', branch: '', worktreePath: null } } };
  const drafts = [{ key: 'new-task:draft', environmentId: 'background', projectId: 'p', origin: 'https://background.test',
    createdAt: new Date(now).toISOString(), text: 'Draft', images: [], files: [], workspace: { branch: '' } }];
  const projected = projectMobileHome(sources, now, { drafts, pendingTasks: [pending] });
  expect(projected.items).toHaveLength(4);
  for (const row of projected.items) {
    expect(row.projectId).toBe('p'); expect(row.favicon).toBe('');
    expect(row.faviconTarget.environmentId).toBe(row.environmentId);
    expect(row.faviconTarget.cwd).toBe(row.environmentId === 'focused' ? '/focus' : '/background');
  }
  const orphaned = projectMobileHome([], now, { drafts, pendingTasks: [pending] });
  expect(orphaned.items).toHaveLength(2);
  expect(orphaned.items.every(row => row.faviconTarget.key === '')).toBe(true);
});

test('chooser icon resolves the grouped representative instead of a preferred selection member', () => {
  const repositoryIdentity = { canonicalKey: 'github.com/team/repo', name: 'repo', displayName: 'team/repo', rootPath: '/repo' };
  const sources = [source('background', { id: 'p', title: 'repo', workspaceRoot: '/representative', faviconPath: 'custom.svg', repositoryIdentity }),
    source('focused', { id: 'p', title: 'repo', workspaceRoot: '/preferred-selection', repositoryIdentity })];
  const scope = mobileHomeProjects(sources, {})[0]!;
  expect(scope.projectKeys).toEqual(['background:p', 'focused:p']);
  expect(mobileNewTaskProjectIcon(scope, sources).faviconTarget).toMatchObject({ environmentId: 'background', cwd: '/representative', faviconPath: 'custom.svg' });
  sources[0]!.shell.projects[0]!.projectIcon = { kind: 'emoji', emoji: '🌲' };
  expect(mobileNewTaskProjectIcon(scope, sources)).toMatchObject({ iconKind: 'emoji', iconText: '🌲', faviconTarget: { key: '' } });
  expect(sources[1]!.shell.projects[0]!.projectIcon).toBeUndefined();
});
