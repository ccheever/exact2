import { describe, expect, test } from 'bun:test';
import { groupLabel, projectDisplayNames, heroGroups } from './r6-polish-groups';
import { alertClip, anchorFrame, textMeasure, SIDEBAR_PROBES } from './r6-polish-measure';
import { cardScripts } from './r6-polish-scripts';
import { rowTips, bubbleWidth } from './r3-sidebar-tips';
import { highlight } from './timeline-highlight';
import { probeKey } from './r5-composer-menus';
import { EnvironmentFleet } from './settings-b-fleet';

const repo = (title: string, root: string, extra: Record<string, unknown> = {}) => ({ id: title + root, title, workspaceRoot: root,
  repositoryIdentity: { canonicalKey: 'git.example.invalid/acme/shared', displayName: 'acme/shared', name: 'shared', owner: 'acme', rootPath: root }, ...extra });

describe('repository display names (deriveProjectGroupLabel)', () => {
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
  test('list markers take the heading ink; bold and code spans after them keep theirs', () => {
    expect(highlight('- one\n  * two\n3. **three** `x`', 'md')).toEqual([
      // lane r12-render: Shiki's markdown grammar (r12-render-highlight.ts); 'tag' and 'flag' paint as 'heading' and 'bold' did,
      // and a code span's backticks are punctuation (#636363) around its green text.
      { text: '-', cls: 'tag' }, { text: ' one\n  ', cls: '' }, { text: '*', cls: 'tag' }, { text: ' two\n', cls: '' },
      { text: '3.', cls: 'tag' }, { text: ' ', cls: '' }, { text: '**three**', cls: 'flag' }, { text: ' ', cls: '' }, { text: '`', cls: 'punct' }, { text: 'x', cls: 'str' }, { text: '`', cls: 'punct' }]);
    expect(highlight('**not a list**', 'md')).toEqual([{ text: '**not a list**', cls: 'flag' }]);
  });
});
