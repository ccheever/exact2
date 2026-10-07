// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/scoped-settings.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
import { obj, str, type Json, type Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';

// Names and values from the pinned ServerSettings/ProjectSettingsOverrides.
// These writes control server worktree creation, merge defaults, generated prose
// and provider tool access, rather than merely saving a disconnected UI form.
const definitions = [
  { key: 'defaultAutoPull', route: 'source-control', label: 'Default automatic pull', description: 'Automatically pull remote changes for new worktrees.', choices: [['false', 'Off'], ['true', 'On']] },
  { key: 'pullRequestMergeMethod', route: 'source-control', label: 'Default pull request merge method', description: 'Last selected uses the method most recently chosen on this device.', choices: [['null', 'Last selected'], ['"merge"', 'Merge'], ['"squash"', 'Squash'], ['"rebase"', 'Rebase']] },
  { key: 'branchNamingMode', route: 'source-control', label: 'Worktree branch naming', description: 'Choose how branches for new worktrees are named.', choices: [['"static"', 'Static prefix'], ['"semantic"', 'Semantic'], ['"custom"', 'Custom instructions']] },
  { key: 'branchNamePrefix', route: 'source-control', label: 'Branch prefix', description: 'Prefix for branches created with static naming.', text: true, when: 'static' },
  { key: 'branchNameInstructions', route: 'source-control', label: 'Branch naming instructions', description: 'Instructions used to generate worktree branch names.', text: true, when: 'custom' },
  { key: 'sourceControlWritingStyle.mode', route: 'source-control', label: 'Source control writing style', description: 'Style for generated change descriptions and change requests.', choices: [['"repo_conventions"', 'Repository conventions'], ['"conventional_commits"', 'Conventional Commits'], ['"custom"', 'Custom instructions']] },
  { key: 'sourceControlWritingStyle.customInstructions', route: 'source-control', label: 'Writing instructions', description: 'Instructions for generated change descriptions and change requests.', text: true, when: 'writing-custom' },
  { key: 'sourceControlWritingStyle.followChangeRequestTemplates', route: 'source-control', label: 'Follow change request templates', description: 'Use the repository template when generating change requests.', choices: [['false', 'Off'], ['true', 'On']] },
  { key: 'enableAgentBrowserAccess', route: 'integrations', label: 'Agent browser access', description: 'Allow providers to use browser tools. Your own browser access is unaffected.', choices: [['false', 'Off'], ['true', 'On']] },
  { key: 'enableAgentDeviceAccess', route: 'integrations', label: 'Agent device access', description: 'Allow providers to drive simulators and emulators when the device hub is enabled.', choices: [['false', 'Off'], ['true', 'On']] },
  { key: 'defaultThreadEnvMode', route: 'general', label: 'Default workspace', description: 'Workspace for new threads. Existing threads keep their workspace.', choices: [['"local"', 'Local'], ['"worktree"', 'Worktree']] },
  { key: 'newWorktreesStartFromOrigin', route: 'general', label: 'Start new worktrees from origin', description: 'Base new worktrees on the remote branch.', choices: [['false', 'Off'], ['true', 'On']] },
  { key: 'worktreeSubmodules', route: 'general', label: 'Worktree submodules', description: 'Initialize submodules in new worktrees.', choices: [['"recursive"', 'Recursive'], ['"top-level"', 'Top level'], ['"none"', 'None']] },
  { key: 'responseStreamingMode', route: 'general', label: 'Response streaming', description: 'Stream assistant responses by turn or paragraph.', choices: [['"turn"', 'Turn'], ['"paragraph"', 'Paragraph']] },
  { key: 'continueThreadsAfterServerUpdate', route: 'general', label: 'Continue threads after server update', description: 'Resume eligible active threads after an environment update.', choices: [['false', 'Off'], ['true', 'On']] },
];
export function scopedSearchControls(settings: Obj, projectId: string, capabilities: Obj) {
  if (projectId && capabilities.projectSettingsOverrides !== true) return [];
  const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
  const effective: Obj = { ...settings, ...override, sourceControlWritingStyle: { ...obj(settings.sourceControlWritingStyle), ...obj(override.sourceControlWritingStyle) } };
  return definitions.filter(item => (!item.when || item.when === (item.key.startsWith('sourceControl') ? `writing-${obj(effective.sourceControlWritingStyle).mode}` : effective.branchNamingMode))).map(item => [item.route, item.text ? `scoped-input-${item.key}` : `scoped-${item.key}-0`, item.label, item.description] as [string, string, string, string]);
}
export function scopedSettingPatch(settings: Obj, projectId: string, key: string, value: Json | undefined, inherit: boolean): Obj {
  const definition = definitions.find(item => item.key === key || (inherit && !key.includes('.') && item.key.startsWith(key + '.')));
  if (!definition) throw new Error('Unsupported scoped setting.');
  if (!inherit && (definition.text ? typeof value !== 'string' : !definition.choices?.some(([candidate]) => candidate === JSON.stringify(value)))) throw new Error('Choose a supported setting value.');
  if (inherit && key.includes('.')) throw new Error('Use the writing-settings group reset to clear the whole override.');
  if (inherit && !projectId) throw new Error('Choose a project to clear its override.');
  const [root, field] = key.split('.');
  const current = projectId ? { ...obj(obj(settings.projectSettingsOverrides)[projectId]) } : { ...settings };
  if (inherit) delete current[root];
  else if (field) current[root] = { ...obj(settings[root]), ...obj(current[root]), [field]: typeof value === 'string' ? value.trim() : value };
  else current[root] = typeof value === 'string' ? value.trim() : value;
  return projectId ? { projectSettingsOverrides: { [projectId]: Object.keys(current).length ? current : null } } : { [root]: current[root] };
}
export async function scopedControls(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, route: string, active: boolean) {
  const empty = { available: false, error: '', project: projectId !== '', scope: `${environmentId}:${projectId}`, rows: [] as { key: string; label: string; description: string; value: string; text: boolean; overridden: boolean; inheritKey: string; inheritLabel: string; disabled: boolean; choices: { id: string; label: string; value: string; selected: boolean }[] }[] };
  if (!active) return empty;
  try {
    if (!native?.available || !client.ready || environmentId !== client.environmentId || (projectId && !client.shell.projects.some(project => project.id === projectId))) throw new Error('This scope is unavailable. Choose a connected environment and an existing checkout.');
    const config = await client.readSettings(native, 'server.getConfig');
    const capabilities = obj(obj(config.environment).capabilities);
    if (projectId && capabilities.projectSettingsOverrides !== true) throw new Error('Update the selected environment to configure project overrides.');
    const settings = await client.readSettings(native, 'server.getSettings');
    const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
    const effective: Obj = { ...settings, ...override, sourceControlWritingStyle: { ...obj(settings.sourceControlWritingStyle), ...obj(override.sourceControlWritingStyle) } };
    const rows = definitions.filter(item => item.route === route && (!item.when || item.when === (item.key.startsWith('sourceControl') ? `writing-${obj(effective.sourceControlWritingStyle).mode}` : effective.branchNamingMode))).map(item => {
      const [root, field] = item.key.split('.');
      const value = field ? obj(effective[root])[field] : effective[root];
      return { key: item.key, label: item.label, description: item.description, value: item.text ? str(value) : JSON.stringify(value ?? null), text: item.text === true, overridden: Object.prototype.hasOwnProperty.call(override, root), inheritKey: !field ? root : item.key.endsWith('.mode') ? root : '', inheritLabel: field ? 'Use environment writing settings' : 'Use environment setting', disabled: !client.writable || !(root in settings) || (item.key === 'continueThreadsAfterServerUpdate' && capabilities.threadRestartContinuation !== true) || (item.key === 'enableAgentDeviceAccess' && settings.enableDeviceSupport !== true), choices: (item.choices || []).map(([value, label], index) => ({ id: String(index), label, value, selected: value === JSON.stringify(field ? obj(effective[root])[field] : effective[root]) })) };
    });
    return { ...empty, available: true, rows };
  } catch (error) { return { ...empty, error: error instanceof Error ? error.message : 'Could not load settings.' }; }
}
