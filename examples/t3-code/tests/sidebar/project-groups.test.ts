import { describe, expect, test } from 'bun:test';
import { groupLabel, projectDisplayNames, heroGroups } from '../../src/sidebar/project-groups';
import { alertClip, anchorFrame, textMeasure, SIDEBAR_PROBES } from '../../src/shared/laid-out-widths';
import { cardScripts } from '../../src/details/project-scripts';
import { rowTips, bubbleWidth } from '../../src/sidebar/sidebar-tips';
import { highlighted } from '../support/highlights-fixture';
import { probeKey } from '../../src/composer/composer-menus';
import { EnvironmentFleet } from '../../src/connections/settings-b-fleet';
import { fleet } from '../../src/connections/settings-b-fleet';
import { T3Client } from '../../src/client/client';
import { NO_PROJECT_SCOPE, partition, projectScopes } from '../../src/sidebar/sidebar-view';
import { sidebarPrefs } from '../../src/sidebar/sidebar-state';

const repo = (title: string, root: string, extra: Record<string, unknown> = {}) => ({ id: title + root, title, workspaceRoot: root,
  repositoryIdentity: { canonicalKey: 'git.example.invalid/acme/shared', displayName: 'acme/shared', name: 'shared', owner: 'acme', rootPath: root }, ...extra });

describe('repository display names (deriveProjectGroupLabel)', () => {
  test('a fork follows its origin, while repository canonical identity remains available for PR routing', () => {
    const client = new T3Client(); client.environmentId = 'env-a';
    const upstream = repo('shared', '/upstream', { id: 'upstream' });
    const fork = repo('shared', '/fork', { id: 'fork', repositoryIdentity: { ...upstream.repositoryIdentity, origin: { canonicalKey: 'git.example.invalid/me/shared', displayName: 'me/shared' } } });
    client.shell.projects = [upstream, fork, { ...fork, id: 'fork-two', workspaceRoot: '/fork-two' }];
    expect(client.projectGroups().map(group => [group.key, group.name, group.members.length])).toEqual([
      ['git.example.invalid/acme/shared', 'shared', 1], ['git.example.invalid/me/shared', 'me/shared', 2],
    ]);
    expect(fork.repositoryIdentity.canonicalKey).toBe('git.example.invalid/acme/shared');
  });
  test('No project merges scratch filters across environments without merging their mutable projects', () => {
    const client = new T3Client(); client.environmentId = 'env-a'; client.config = {};
    client.shell.projects = [{ id: 'a', title: 'Scratch A', workspaceRoot: '/a', isScratch: true }, { id: 'repo', title: 'Named', workspaceRoot: '/repo' }];
    client.shell.threads = [{ id: 'local', projectId: 'a', status: 'idle' }];
    const key = 'panel-scope-fixture';
    fleet.entries.set(key, { key, origin: 'http://fixture.invalid', environmentId: 'env-b', phase: 'connected', message: '', traceId: '', generation: 1, synchronized: 1, lastEvent: 0,
      subscriptions: {}, config: {}, shell: { projects: [{ id: 'b', title: 'Scratch B', workspaceRoot: '/b', isScratch: true }], threads: [{ id: 'remote', projectId: 'b', status: 'idle' }] } as never, scopes: [], error: '', requested: true });
    try {
      sidebarPrefs(client).scope = 'env-a:/a';
      const scratch = projectScopes(client).find(group => group.key === NO_PROJECT_SCOPE)!;
      expect(scratch.name).toBe('No project'); expect([...scratch.ids]).toEqual(['a', 'fleet:env-b:b']);
      expect(sidebarPrefs(client).scope).toBe(NO_PROJECT_SCOPE);
      expect(partition(client, 0).active.map(thread => thread.id).sort()).toEqual(['fleet:env-b:remote', 'local']);
      expect(client.projectGroups()).toHaveLength(2); expect(client.shell.projects).toHaveLength(2);
    } finally { fleet.entries.delete(key); }
  });
  test('a shared title that is only the repository name reads as the display name', () => {
    expect(groupLabel([repo('shared', '/a/shared'), repo('shared', '/b/shared')])).toBe('acme/shared');
    expect(groupLabel([repo('Payments', '/a/shared'), repo('Payments', '/b/shared')])).toBe('Payments');
    expect(groupLabel([repo('shared', '/a/shared'), repo('shared-2', '/b/shared-2')])).toBe('acme/shared');
    expect(groupLabel([repo('shared', '/a/shared')])).toBe('shared');
  });
  test('checkouts group across the focused environment and connected background environments', () => {
    const fleet = new EnvironmentFleet();
    const local = { environmentId: 'env-a', shell: { projects: [repo('shared', '/a/shared', { id: 'p1' }), { id: 'p2', title: 'demo', workspaceRoot: '/a/demo' }] }, local: { groupingMode: 'repository', groupingOverrides: {} } };
    expect(projectDisplayNames(local, fleet).get('p1')).toBe('shared');
    fleet.entries.set('b', { key: 'b', origin: 'http://b', environmentId: 'env-b', phase: 'connected', message: '', traceId: '', generation: 1, synchronized: 1, lastEvent: 0,
      subscriptions: {}, config: {}, shell: { projects: [repo('shared', '/b/shared', { id: 'q1' })], threads: [] } as never, scopes: [], error: '', requested: true });
    const names = projectDisplayNames(local, fleet);
    expect([names.get('p1'), names.get('fleet:env-b:q1'), names.get('p2')]).toEqual(['acme/shared', 'acme/shared', 'demo']);
    // "Keep separate" for the local checkout splits the group again.
    const separate = { ...local, local: { groupingMode: 'repository', groupingOverrides: { 'env-a:/a/shared': 'separate' } } };
    expect(projectDisplayNames(separate, fleet).get('p1')).toBe('shared');
  });
  test('the hero lists each logical project once, under its group name', () => {
    const client = { environmentId: 'env-a', revision: 1, shell: { projects: [repo('shared', '/a/shared', { id: 'p1' }), repo('shared', '/b/shared', { id: 'p2' })] }, local: { groupingMode: 'repository', groupingOverrides: {} } };
    expect(heroGroups(client, [{ id: 'p2', name: 'shared' }, { id: 'p1', name: 'shared' }])).toEqual([{ id: 'p2', name: 'acme/shared' }]);
  });
});

describe('measured placements', () => {
  test('the AlertStack wrapper is clipped to its banners, and not before both are laid out', () => {
    expect(alertClip({ anchors: {} })).toBe('none');
    expect(alertClip({ anchors: { alerts: [0, 900], 'alert:provider': [100, 700] } })).toBe("path('M 100 0 H 800 V 4000 H 100 Z')");
    expect(alertClip({ anchors: { alerts: [0, 900], 'alert:provider': [100, 700], 'alert:error': [50, 800] } })).toBe("path('M 50 0 H 850 V 4000 H 50 Z')");
    expect(alertClip({ anchors: { alerts: [0, 500], 'alert:error': [50, 800] } })).toBe('none');
  });
  test('row tooltips place by the probed label widths once they report', () => {
    expect(SIDEBAR_PROBES.map(entry => entry.text)).toEqual(['Settle', 'Woke', 'Unsent draft']);
    const presentation = { anchors: { [probeKey('Settle', 12, 400)]: [0, 40], [probeKey('Woke', 12, 500)]: [0, 32], [probeKey('Unsent draft', 12, 400)]: [0, 70.5] } };
    const measure = textMeasure(presentation);
    expect(measure('Settle', 12, 400)).toBe(40);
    expect(measure('Missing', 12, 400)).toBeNull();
    expect(bubbleWidth('Unsent draft', measure)).toBe(88.5);
    const facts = { card: true, draft: false, pinned: false, canPin: true, woke: false, canSnooze: false, canSettle: true, canWake: false, canUnsettle: false };
    const settle = rowTips(facts, measure).find(tip => tip.name === 'settle')!;
    expect(settle.x).toBe(14 + (6 + 14 + 4 + 40 + 6) / 2);
    expect(anchorFrame({ anchors: { 'crumb:src': [61.5, 30] } }, 'crumb:src')).toEqual([61.5, 30]);
  });
});

describe('the card script row (ProjectScriptsControl, panel)', () => {
  test('the primary script is the first that is not a setup script; setup scripts read "(setup)" in the menu', () => {
    const settings = { projectSettingsOverrides: { p1: { defaultProjectScripts: [
      { id: 'setup', name: 'Setup', command: 'npm ci', icon: 'configure', runOnWorktreeCreate: true },
      { id: 'test', name: 'Test', command: 'npm test', icon: 'test', runOnWorktreeCreate: false }] } } };
    expect(cardScripts(settings, { id: 'p1' })).toEqual({ scriptName: 'Test', scriptIcon: 'test',
      scripts: [{ id: 'setup', label: 'Setup (setup)', icon: 'configure' }, { id: 'test', label: 'Test', icon: 'test' }] });
    const only = { projectSettingsOverrides: { p1: { defaultProjectScripts: [{ id: 'setup', name: 'Setup', command: 'sh setup.sh', icon: 'play', runOnWorktreeCreate: true }] } } };
    expect(cardScripts(only, { id: 'p1' }).scriptName).toBe('Setup');
    expect(cardScripts({}, { id: 'p1' }).scriptName).toBe('');
  });
});

describe('Markdown source colour', () => {
  test('list markers are punctuation grey; bold and code spans after them keep their styles', () => {
    // @pierre/highlights' markdown lexer: punctuation.list_marker, emphasis.strong (weight 700), a code span a string.
    expect(highlighted('- one\n  * two\n3. **three** `x`', 'md')).toEqual([
      { text: '- ', cls: 'punct' }, { text: 'one\n', cls: '' }, { text: '  ', cls: '' }, { text: '* ', cls: 'punct' }, { text: 'two\n', cls: '' },
      { text: '3. ', cls: 'punct' }, { text: '**three**', cls: 'strong' }, { text: ' ', cls: '' }, { text: '`x`', cls: 'str' }]);
    expect(highlighted('**not a list**', 'md')).toEqual([{ text: '**not a list**', cls: 'strong' }]);
  });
});
