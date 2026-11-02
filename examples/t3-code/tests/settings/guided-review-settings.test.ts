import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from '../../src/client/domain';
import { DEFAULTS, inheritance, scopedPatch, sourceControlRows } from '../../src/settings/source-control-view';
import { isProjectScopedSettingKey, planScopedSettingsClear, planScopedSettingsPatch, resolveProjectSettings,
  resolveScopedSettingsTargets, scopedSettingsAreMixed, scopedSettingsSource } from '../../src/settings/scoped-settings-plan';
import { resolveSettingsScope, type ScopeGroup } from '../../src/settings/settings-scope';
import { contractComponent } from '../support/contract-action-fixture';

function environment(id: string, settings: Obj) {
  return { environmentId: id, label: id, connection: { phase: 'connected' },
    serverConfig: { settings, environment: { capabilities: { projectSettingsOverrides: true } } } };
}
const first = environment('first', { reviewGuideInstructions: 'Read service interfaces first.', reviewGuidesEnabled: true,
  projectSettingsOverrides: { p1: { reviewGuideInstructions: 'Read errors.', branchNamePrefix: 'feature' } } });
const second = environment('second', { reviewGuideInstructions: 'Review behavior.', reviewGuidesEnabled: false,
  projectSettingsOverrides: { p2: { defaultAutoPull: true } } });
const environments = [first, second];
const group: ScopeGroup = { projectKey: 'logical-project', displayName: 'Project', memberProjects: [
  { environmentId: 'first', id: 'p1', workspaceRoot: '/one', physicalProjectKey: 'first:/one' },
  { environmentId: 'second', id: 'p2', workspaceRoot: '/two', physicalProjectKey: 'second:/two' },
] };
const project = resolveSettingsScope({ project: group.projectKey }, [group], environments);
const instructionsRow = (settings: Obj, projectId = '', writable = true) => sourceControlRows(settings, projectId, 'Local', [], writable).writing.find(row => row.key === 'reviewGuideInstructions');

describe('review guide instructions settings', () => {
  test('trims surrounding whitespace, preserves multiline contents, and resets to an empty string', () => {
    expect(DEFAULTS.reviewGuideInstructions).toBe('');
    expect(scopedPatch({}, '', 'reviewGuideInstructions', ' \nRead contracts.\n\nKeep <literal> text.\t '))
      .toEqual({ reviewGuideInstructions: 'Read contracts.\n\nKeep <literal> text.' });
    expect(scopedPatch({}, '', 'reviewGuideInstructions', '  \n\t ')).toEqual({ reviewGuideInstructions: '' });
    expect(scopedPatch({ reviewGuideInstructions: 'Custom' }, '', 'reviewGuideInstructions', '__default__')).toEqual({ reviewGuideInstructions: '' });
  });

  test('project writes preserve sibling overrides and inherit restores the environment value', () => {
    const settings: Obj = { reviewGuidesEnabled: true, reviewGuideInstructions: 'Environment instructions',
      projectSettingsOverrides: { p: { reviewGuideInstructions: 'Previous', branchNamePrefix: 't3' }, other: { reviewGuideInstructions: 'Other' } } };
    expect(scopedPatch(settings, 'p', 'reviewGuideInstructions', ' New instructions ')).toEqual({ projectSettingsOverrides: { p: { reviewGuideInstructions: 'New instructions', branchNamePrefix: 't3' } } });
    const cleared = scopedPatch(settings, 'p', 'reviewGuideInstructions', '__inherit__');
    expect(cleared).toEqual({ projectSettingsOverrides: { p: { branchNamePrefix: 't3' } } });
    const resolved = resolveProjectSettings({ ...settings, projectSettingsOverrides: { ...obj(settings.projectSettingsOverrides), ...obj(cleared.projectSettingsOverrides) } }, 'p');
    expect(resolved.settings.reviewGuideInstructions).toBe('Environment instructions');
    expect(resolved.sources.reviewGuideInstructions).toBe('environment');
    expect(obj(settings.projectSettingsOverrides).other).toEqual({ reviewGuideInstructions: 'Other' });
    expect(scopedPatch({ projectSettingsOverrides: { p: { reviewGuideInstructions: 'Only override' } } }, 'p', 'reviewGuideInstructions', '__inherit__'))
      .toEqual({ projectSettingsOverrides: { p: null } });
  });

  test('projects can explicitly clear instructions while environment and unrelated project defaults stay intact', () => {
    const plan = planScopedSettingsPatch(project, environments, { reviewGuideInstructions: '' });
    expect(plan.unavailableReason).toBeNull();
    expect(plan.serverWrites.map(write => [write.environmentId, write.patch])).toEqual([
      ['first', { projectSettingsOverrides: { p1: { reviewGuideInstructions: '', branchNamePrefix: 'feature' } } }],
      ['second', { projectSettingsOverrides: { p2: { reviewGuideInstructions: '', defaultAutoPull: true } } }],
    ]);
    expect(first.serverConfig.settings.reviewGuideInstructions).toBe('Read service interfaces first.');
    expect(second.serverConfig.settings.reviewGuideInstructions).toBe('Review behavior.');
    expect(planScopedSettingsClear(project, environments, ['reviewGuideInstructions']).serverWrites.map(write => write.patch)).toEqual([
      { projectSettingsOverrides: { p1: { branchNamePrefix: 'feature' } } },
      { projectSettingsOverrides: { p2: { defaultAutoPull: true } } },
    ]);
  });

  test('resolves mixed instructions and inheritance across the selected project checkouts', () => {
    expect(isProjectScopedSettingKey('reviewGuideInstructions')).toBe(true);
    const targets = resolveScopedSettingsTargets(project, environments);
    expect(targets.map(target => [target.environmentId, target.projectId, target.settings.reviewGuideInstructions, target.sources.reviewGuideInstructions]))
      .toEqual([['first', 'p1', 'Read errors.', 'project'], ['second', 'p2', 'Review behavior.', 'environment']]);
    expect(scopedSettingsAreMixed(targets, ['reviewGuideInstructions'])).toBe(true);
    expect(scopedSettingsSource(targets, ['reviewGuideInstructions'])).toBe('mixed');
    expect(inheritance(first.serverConfig.settings, 'p1', 'reviewGuideInstructions', 'First').state).toBe('overridden');
    expect(inheritance(second.serverConfig.settings, 'p2', 'reviewGuideInstructions', 'Second').summary).toBe('Inherited from Second');
  });

  test('shows instructions and their project reset only when the environment enables guides', () => {
    expect(instructionsRow({ reviewGuideInstructions: 'Stored while disabled' })).toBeUndefined();
    expect(instructionsRow({ reviewGuidesEnabled: false, projectSettingsOverrides: { p: { reviewGuidesEnabled: true, reviewGuideInstructions: 'Project' } } }, 'p')).toBeUndefined();
    expect(instructionsRow({ reviewGuidesEnabled: true, reviewGuideInstructions: '' })).toMatchObject({ kind: 'textarea', value: '', reset: '' });
    expect(instructionsRow({ reviewGuidesEnabled: true, reviewGuideInstructions: 'Custom' })).toMatchObject({ reset: 'key=reviewGuideInstructions&value=__default__' });
    expect(instructionsRow(first.serverConfig.settings, 'p1')).toMatchObject({ value: 'Read errors.', reset: 'key=reviewGuideInstructions&value=__inherit__', state: 'overridden' });
    expect(instructionsRow({ reviewGuidesEnabled: true, reviewGuideInstructions: 'Inherited', projectSettingsOverrides: { p: { reviewGuidesEnabled: false } } }, 'p'))
      .toMatchObject({ value: 'Inherited', reset: '', state: 'inherited' });
    expect(instructionsRow({ reviewGuidesEnabled: true }, '', false)?.disabled).toBe(true);
  });

  test('keeps the guides enable flag environment-only in the UI and both patch paths', () => {
    expect(DEFAULTS.reviewGuidesEnabled).toBe(false);
    expect(isProjectScopedSettingKey('reviewGuidesEnabled')).toBe(false);
    expect(sourceControlRows(first.serverConfig.settings, 'p1', 'First', [], true).writing.find(row => row.key === 'reviewGuidesEnabled'))
      .toMatchObject({ checked: true, disabled: true });
    expect(() => scopedPatch({}, 'p1', 'reviewGuidesEnabled', 'true')).toThrow('environment-wide');
    expect(planScopedSettingsPatch(project, environments, { reviewGuidesEnabled: true }))
      .toMatchObject({ serverWrites: [], unavailableReason: 'This setting is environment-wide and cannot be overridden by a project.' });
    const named = resolveSettingsScope({ machine: 'second' }, [group], environments);
    expect(planScopedSettingsPatch(named, environments, { reviewGuidesEnabled: true }).serverWrites.map(write => [write.environmentId, write.patch]))
      .toEqual([['second', { reviewGuidesEnabled: true }]]);
  });
});

describe('review guide textarea commits', () => {
  const textarea = contractComponent('settings-source-control.contract', 'ScopedTextArea');
  function fixture(mixed: boolean, value: string) {
    const calls: unknown[][] = [];
    const scope = { value, mixed, placeholder: '', label: 'Review guide instructions', fieldKey: 'reviewGuideInstructions', busy: false,
      restInput: (...args: unknown[]) => calls.push(args) };
    return { calls, scope, state: textarea.state(scope) };
  }
  test('a mixed textarea starts empty and an untouched blur never replaces the different saved values', () => {
    const { calls, scope, state } = fixture(true, 'Representative instructions');
    expect(state.draft).toBe('');
    textarea.call('commit', state, scope, ['']);
    expect(calls).toEqual([]);
  });
  test('a changed mixed value saves its trimmed text once and clears the edited flag', () => {
    const { calls, scope, state } = fixture(true, 'Representative instructions');
    textarea.call('edit', state, scope, [' \nRead tests first.\n ']);
    textarea.call('commit', state, scope, [' \nRead tests first.\n ']);
    textarea.call('commit', state, scope, ['Read tests first.']);
    expect(calls).toEqual([['scoped', 'key=reviewGuideInstructions&value=', 'Read tests first.']]);
  });
  test('an ordinary unchanged value, including padding added by an edit, sends no write', () => {
    const { calls, scope, state } = fixture(false, 'Read tests first.');
    textarea.call('commit', state, scope, ['Read tests first.']);
    textarea.call('edit', state, scope, [' Read tests first.  ']);
    textarea.call('commit', state, scope, [' Read tests first.  ']);
    expect(calls).toEqual([]);
  });
  test('an explicitly edited empty mixed value clears every selected target, even when the representative was already empty', () => {
    const { calls, scope, state } = fixture(true, '');
    textarea.call('edit', state, scope, [' ']);
    textarea.call('commit', state, scope, [' ']);
    expect(calls).toEqual([['scoped', 'key=reviewGuideInstructions&value=', '']]);
  });
});
