// Settings → Project view model (reference ProjectsSettings.tsx and
// ProjectSettingsPanel.tsx, its Pin and Archive rows at 2359643cff and its Checkouts at 5d3d78ab93): the selected project group's name, icon, pin,
// archive, new-thread defaults, checkouts and removal copy, over every environment's project groups
// (projects-fleet.ts). Group key and checkout come from the settings scope
// (settings-core's resolveScope), so this never guesses a target.
import { arr, obj, projectGroupOrganization, str, type Obj } from '../client/domain';
import { canPinProjectGroup } from '../sidebar/project-pin';
import type { T3Client } from '../client/client';
import { projectIdentity } from '../client/presentation';
import { inheritance, type ScopedRow } from './source-control-view';
import { projectActions, resolveScripts, overridesScripts } from './settings-b-actions';
import { iconFields, blankIconFields } from './settings-b-icons';
import type { Native } from '../client/protocol';
import { environmentClient, environmentLabel, memberRef, memberThreads, settingsGroups } from './projects-fleet';
import { settingsScopeEnvironmentLabels } from './settings-scope';
import { scopeNoticeGroups, type ScopeNoticeGroup } from './settings-scope-notice';
import { scopeEnvironments, scopeProjectGroups } from './settings-scope-sources';
import { isScratch } from '../composer/scratch-machine';
import { projectPath } from '../client/client-shared';

type Choice = { value: string; label: string; selected: boolean };
type Checkout = { id: string; title: string; path: string; groupingLabel: string; groupingValue: string; groupingOptions: Choice[] };

// PROJECT_GROUPING_MODE_LABELS (sidebarProjectGrouping.ts).
const GROUPING_LABELS: Record<string, string> = { repository: 'Group by repository', repository_path: 'Group by repository path', separate: 'Keep separate' };
/**
 * CheckoutGroupingSelect: the checkout's own grouping rule (this device's override by its physical key), else
 * "Default (<the global mode>)". The option values are the `grouping-override` op's (client-ops-sidebar.ts).
 */
function checkoutRow(client: T3Client, member: Obj): Checkout {
  const { environmentId } = memberRef(client, str(member.id));
  const local = client.local as { groupingMode?: string; groupingOverrides?: Record<string, string> };
  const selection = local.groupingOverrides?.[`${environmentId}:${projectPath(member.workspaceRoot)}`] ?? 'inherit';
  const label = environmentLabel(client, environmentId);
  return { id: str(member.id), title: str(member.title), path: str(member.workspaceRoot),
    groupingLabel: `Grouping rule for ${str(member.workspaceRoot)} on ${label ? `${label} (${environmentId})` : environmentId}`,
    groupingValue: selection === 'inherit' ? `Default (${GROUPING_LABELS[local.groupingMode ?? 'repository'] ?? GROUPING_LABELS.repository})` : GROUPING_LABELS[selection] ?? selection,
    groupingOptions: [['inherit', 'Use global default'], ...Object.entries(GROUPING_LABELS)].map(([value, text]) => ({ value: value!, label: text!, selected: value === selection })) };
}

// ProjectSettingsPanel.logic.ts: the No project (Scratch) project's own name and icon, which its resets restore.
export const SCRATCH_PROJECT_TITLE = 'No project';
const SCRATCH_PROJECT_ICON = { kind: 'lucide', name: 'message-square-dashed', color: 'gray' };
/** A member is the Scratch project of its own environment (isScratchProject: its workspace root is the Scratch folder). */
export function memberIsScratch(client: T3Client, member: Obj): boolean {
  const home = environmentClient(client, memberRef(client, str(member.id)).environmentId);
  return isScratch(member, str(home.config.scratchWorkspaceRoot).trim());
}
/** defaultProjectIcon: no favicon, and the Scratch project's dashed message icon or none. */
export const defaultProjectIcon = (scratch: boolean) => ({ faviconPath: null, projectIcon: scratch ? SCRATCH_PROJECT_ICON : null });
/** hasDefaultProjectIcon: the member shows the icon a reset would give it. */
export function hasDefaultProjectIcon(member: Obj, scratch: boolean): boolean {
  if (member.faviconPath != null) return false;
  const icon = member.projectIcon == null ? null : obj(member.projectIcon);
  if (!scratch) return icon === null;
  return icon?.kind === 'lucide' && icon.name === SCRATCH_PROJECT_ICON.name && icon.color === SCRATCH_PROJECT_ICON.color;
}
const blank: ScopedRow = { key: '', kind: 'select', title: '', description: '', checked: false, value: '', valueLabel: '', options: [], placeholder: '', disabled: true, first: false, summary: '', state: '', layers: [], layerGroups: [], reset: '', status: '', child: '', mixed: false, control: '', resetLabel: '', driver: '', icon: '', badge: '', accent: '', traitsLabel: '', traits: [], target: '', managed: '' };

function iconDescription(project: Obj): string {
  const icon = obj(project.projectIcon);
  if (icon.kind === 'lucide') return `${str(icon.name)} · ${str(icon.color)}`;
  if (icon.kind === 'monogram') return `${str(icon.text)} · ${str(icon.color)}`;
  if (icon.kind === 'emoji') return str(icon.emoji);
  return str(project.faviconPath, 'Automatic');
}

export function projectsPage(client: T3Client, projectKey: string, checkout: string, active: boolean) {
  const empty = { ready: false, selected: false, message: '', noticeGroups: [] as ScopeNoticeGroup[], confirmTitle: '', confirmDescription: '', key: '', name: '', mark: '', ink: '', surface: '', icon: 'Automatic', iconCustom: false, nameResettable: false,
    members: [] as (Checkout & { environment: string; first: boolean })[], checkoutGroups: [] as { id: string; label: string; secondary: string; members: Checkout[]; first: boolean }[], multipleCheckouts: false, hasOther: false, threads: 0,
    workspace: blank, removeTitle: '', removeDescription: '', removeLabel: '', confirm: '', removeTarget: '', names: [] as string[], organize: false, pinned: false, archived: false };
  if (!active) return empty;
  const groups = settingsGroups(client);
  if (!client.ready) return { ...empty, ready: true, message: 'Connect an environment to manage its projects.' };
  // ProjectsSettings: SettingsScopeNotice target="project".
  if (!projectKey) return { ...empty, ready: true, noticeGroups: groups.length ? scopeNoticeGroups('project', scopeProjectGroups(client), scopeEnvironments(client)) : [],
    message: 'Choose a project to manage its name, icon, checkouts and actions.' };
  const group = groups.find(candidate => candidate.key === projectKey);
  // A stale project or checkout is explained once, by the scope sentence's notice (settings-core).
  if (!group) return { ...empty, ready: true, message: groups.length ? '' : 'Add a project from the sidebar to configure it here.' };
  const members = group.members.filter(member => !checkout || member.id === checkout);
  if (!members.length) return { ...empty, ready: true, message: '' };
  const representative = members[0], hasOther = members.length < group.members.length;
  const threads = members.reduce((count, member) => count + memberThreads(client, str(member.id)).length, 0);
  // The representative's own environment holds its settings and takes its writes.
  const ref = memberRef(client, str(representative.id)), home = environmentClient(client, ref.environmentId);
  const settings = obj(home.config.settings), projectId = ref.projectId;
  const environment = environmentLabel(client, ref.environmentId);
  const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
  // The Model row is General's (settings-core's ProviderModelPicker + TraitsPicker row, settingsCore.projectModel).
  const envMode = str(override.defaultThreadEnvMode ?? settings.defaultThreadEnvMode, 'local');
  const identity = projectIdentity(group.name);
  const whole = members.length === group.members.length && !hasOther;
  const kind = hasOther || !whole ? 'checkout' : 'project';
  const machines = scopeEnvironments(client), checkouts = new Map<string, Checkout[]>();
  for (const member of members) {
    const environmentId = memberRef(client, str(member.id)).environmentId;
    checkouts.set(environmentId, [...(checkouts.get(environmentId) ?? []), checkoutRow(client, member)]);
  }
  const checkoutGroups = checkouts.size > 1 ? [...checkouts].map(([id, entries], index) => {
    const environment = machines.find(machine => machine.environmentId === id);
    const labels = environment ? settingsScopeEnvironmentLabels(environment, machines) : { primary: environmentLabel(client, id), secondary: id };
    return { id, label: labels.primary, secondary: labels.secondary, members: entries.map((member, index) => ({ ...member, environment: labels.primary, first: index === 0 })), first: index === 0 };
  }) : [];
  const single = members.length === 1 ? representative : undefined;
  // Pin and archive belong to the whole project, even when one machine is picked.
  const organization = projectGroupOrganization(group.members);
  return { ...empty, ready: true, selected: true, key: group.key, organize: canPinProjectGroup(client, group.members), pinned: organization.pinned, archived: organization.archived, name: group.name, mark: identity.projectMark, ink: identity.projectInk, surface: identity.projectSurface,
    icon: iconDescription(representative), iconCustom: group.members.some(member => !hasDefaultProjectIcon(member, memberIsScratch(client, member))),
    // canResetScratchTitle: every member is a Scratch project and one of them was renamed.
    nameResettable: group.members.every(member => memberIsScratch(client, member)) && group.members.some(member => str(member.title) !== SCRATCH_PROJECT_TITLE),
    // Checkouts lists a lone checkout too, for its grouping rule; Remove needs another checkout to keep.
    members: members.map((member, index) => ({ ...checkoutRow(client, member), environment: environmentLabel(client, memberRef(client, str(member.id)).environmentId), first: index === 0 })),
    multipleCheckouts: members.length > 1, hasOther, threads, checkoutGroups,
    workspace: { ...blank, ...inheritance(settings, projectId, 'defaultThreadEnvMode', environment), key: 'defaultThreadEnvMode', kind: 'select', title: 'Workspace', description: 'Where new threads in this project start.', disabled: !home.writable,
      value: envMode, valueLabel: envMode === 'worktree' ? 'New worktree' : 'Current checkout', options: [{ value: 'local', label: 'Current checkout', selected: envMode === 'local' }, { value: 'worktree', label: 'New worktree', selected: envMode === 'worktree' }],
      reset: Object.prototype.hasOwnProperty.call(override, 'defaultThreadEnvMode') ? 'key=defaultThreadEnvMode&value=__inherit__' : '' },
    removeTitle: hasOther ? 'Remove checkout' : group.members.length > 1 ? 'Remove this project everywhere' : 'Remove project',
    removeDescription: hasOther ? "Deletes the selected machine's checkout entries and their threads. Other machines and files on disk are not touched."
      : group.members.length > 1 ? `Deletes all ${group.members.length} checkout entries and their threads on every machine. Files on disk are not touched.` : 'Deletes the project entry and its threads. Files on disk are not touched.',
    removeLabel: hasOther ? 'Remove checkout' : group.members.length > 1 ? 'Remove all entries' : 'Remove project',
    // removeMembers' confirmation copy, one line per paragraph.
    confirm: [threads > 0 ? `Remove ${kind} "${single ? str(single.title) : group.name}" and delete its ${threads} thread${threads === 1 ? '' : 's'}?` : `Remove ${kind} "${single ? str(single.title) : group.name}"?`,
      ...(single ? [`Path: ${str(single.workspaceRoot)}`, `Environment: ${environment}`] : [`This removes ${members.length} grouped project entries.`]),
      threads > 0 ? 'Those threads and any archived threads are deleted from T3 Code. Provider sessions are kept.' : 'Any archived threads are deleted from T3 Code. Provider sessions are kept.',
      whole ? 'This removes only the project entries, not the files on disk.' : 'Other entries in this grouped project are unaffected.', 'This action cannot be undone.'].join('\n'),
    removeTarget: hasOther ? str(representative.id) : '', names: [group.name] };
}

/** removeMembers' confirm message split as ConfirmDialogHost does: the question line titles it. */
export function confirmCopy(message: string): { title: string; description: string } {
  const lines = message.trim().split('\n');
  const question = lines.findIndex(line => line.trim().endsWith('?'));
  if (question >= 0) return { title: lines[question].trim(), description: lines.filter((_, index) => index !== question).join('\n').trim() };
  return { title: 'Confirm action', description: message.trim() };
}

const blankAction = { id: '', first: false, name: '', command: '', icon: 'play', setup: false, settle: false, wait: false, preview: false, previewUrl: '', autoOpen: false, keybinding: '', shortcut: '' };
/** The Actions section: ProjectActionsSettings for the scope's checkouts. */
async function actionsSection(client: T3Client, native: Native | null | undefined, projectKey: string, checkout: string, page: ReturnType<typeof projectsPage>) {
  const group = page.selected ? settingsGroups(client).find(candidate => candidate.key === projectKey) : undefined;
  const members = group ? group.members.filter(member => !checkout || member.id === checkout) : [];
  const empty = { actionScope: '', actionsMixed: false, actionsWritable: false, t3Invalid: false, actions: [] as (typeof blankAction)[], importable: [] as { key: string; name: string; command: string; icon: string; payload: string }[],
    actionsRow: { ...blank, key: 'defaultProjectScripts', kind: 'actions', title: 'Actions', description: "Commands that run in this project's checkout or its worktree, with optional shortcuts.", first: true, reset: '', status: '' }, blankActions: [blankAction] };
  if (!members.length) return empty;
  // The actions of the representative's environment, over its members there (their own project ids, as its settings key them).
  const home = memberRef(client, str(members[0]!.id)).environmentId, view = environmentClient(client, home, native);
  const local = members.filter(member => memberRef(client, str(member.id)).environmentId === home).map(member => ({ ...member, id: memberRef(client, str(member.id)).projectId }));
  const listed = await projectActions(view, native, local);
  const settings = obj(view.config.settings), environment = environmentLabel(client, home);
  const overridden = local.some(member => overridesScripts(settings, str(member.id)));
  const count = (list: unknown[]) => `${list.length} action${list.length === 1 ? '' : 's'}`;
  const own = resolveScripts(settings, local[0]!);
  return { ...empty, actionScope: members.map(member => str(member.id)).join(','), actionsMixed: listed.actionsMixed, t3Invalid: listed.t3Invalid,
    actionsWritable: view.writable && obj(obj(view.config.environment).capabilities).projectSettingsOverrides === true,
    actions: listed.actions.map(action => { const edit = obj(JSON.parse(action.edit)); return { id: action.id, first: action.first, name: action.name, command: action.command, icon: action.icon, setup: action.setup, settle: action.settle,
      wait: edit.waitForSetup === true, preview: action.preview, previewUrl: str(edit.previewUrl), autoOpen: edit.autoOpenPreview === true, keybinding: str(edit.keybinding), shortcut: action.shortcut }; }),
    importable: listed.importable.map(entry => { const raw = obj(JSON.parse(entry.payload));
      return { key: entry.key, name: entry.name, command: entry.command, icon: entry.icon,
        payload: new URLSearchParams({ name: str(raw.name), command: str(raw.command), icon: str(raw.icon), setup: String(raw.runOnWorktreeCreate === true), settle: String(raw.runOnSettle === true), wait: String(raw.waitForSetup === true), keybinding: '', preview: str(raw.previewUrl), auto: String(raw.autoOpenPreview === true) }).toString() }; }),
    actionsRow: { ...empty.actionsRow, summary: overridden ? 'Overridden for this project' : `Inherited from ${environment}`, state: overridden ? 'overridden' : 'inherited',
      layers: [{ key: 'project', label: 'Project', value: overridden ? count(own) : 'Inherits', effective: overridden, set: overridden },
        { key: 'environment', label: 'Environment', value: arr(settings.defaultProjectScripts).length ? count(arr(settings.defaultProjectScripts)) : 'Inherits', effective: !overridden && arr(settings.defaultProjectScripts).length > 0, set: arr(settings.defaultProjectScripts).length > 0 },
        { key: 'built-in', label: 'Default', value: 'None', effective: !overridden && !arr(settings.defaultProjectScripts).length, set: true }],
      reset: overridden ? 'reset' : '' } };
}

/** The page with the confirmation already split for the dialog. */
export async function projectsView(client: T3Client, projectKey: string, checkout: string, active: boolean, native?: Native | null) {
  const page = projectsPage(client, projectKey, checkout, active);
  const copy = page.confirm ? confirmCopy(page.confirm) : { title: '', description: '' };
  const group = page.selected ? settingsGroups(client).find(candidate => candidate.key === projectKey) : undefined;
  // The icon is read in its representative's environment (its favicon is that server's); its writes reach every member's.
  const home = group?.members[0] ? environmentClient(client, memberRef(client, str(group.members[0].id)).environmentId, native) : client;
  const icons = group ? await iconFields(home, native, group.name, group.members) : blankIconFields; // settings-b-icons.ts
  return { ...page, confirmTitle: copy.title, confirmDescription: copy.description, ...(await actionsSection(client, native, projectKey, checkout, page)), ...icons };
}
