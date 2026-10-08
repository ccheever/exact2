// Provider settings upkeep, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): the version advisory
// popover of apps/web/src/components/settings/ProviderInstanceCard.tsx:772-870 (one node for the
// list row and the editor), providerStatus.ts getProviderVersionAdvisoryPresentation,
// ProviderSettingsPanel.tsx:723-767 and :1162-1186 (runProviderUpdate, onInstallRecommended,
// onRunUpdate, isUpdating), ProviderUpdatesAction.tsx:24-112 ("Update all" on every connected
// environment, one toast) and the `upkeep:` ops that drive them, the ACP section
// (acp-sessions.ts) and the custom model editor (custom-model-editor.ts).
// Changes: pending updates are kept here per connection (the reference's
// updatingProviderInstanceIds and ProviderUpdatesAction's isPending); an update runs inside its
// op, sent through the `providerUpdateRun` queue mutation, so the button shows "Updating" from
// this state and from the server's `updateState` while the rest of Settings stays usable.
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';
import { versionLabel } from './providers-meta';
import { fleet } from './settings-b-fleet';
import {
  canOneClickUpdateProviderCandidate, collectProviderUpdateCandidates, failure, getProviderUpdateRunToastView, isProviderSettingsUpdateCandidate,
  isProviderUpdateActive, providerDisplayName, providersOf, success, type ProviderUpdateRun,
} from './provider-updates';
import { fleetTarget, focusedTarget, primaryTarget, runLaunchUpdates, UPDATE_TIMEOUT_SECONDS, type UpdateTarget } from './provider-update-notify';
import { acpOp, acpSectionView, urlAuthView, type AcpHost } from './acp-sessions';
import { customModelEditorOp, customModelEditorView, readCustomModelEntries, storedCustomModels } from './custom-model-editor';
import { wakeShell } from './r10-connect-timing';
import { upsertInstance, type ProviderHost, type providerPage } from './providers';
import { FleetSetupHost, hostClient } from './codex-fleet-host'; // Settings › Providers on a background environment (providers-scope.ts)

export interface UpkeepHost extends AcpHost, ProviderHost {
  ready: boolean; generation?: number; environmentId?: string;
  call?(native: Native, request: unknown, expected?: number, write?: boolean): Promise<Obj>;
}

// --- The version advisory popover --------------------------------------------------------------

const COMPATIBILITY_TITLES: Record<string, string> = { graceful: 'Limited support', unsupported: 'Unsupported version', broken: 'Known broken version' };
/** getProviderVersionAdvisoryPresentation. */
export function versionAdvisoryPresentation(advisory: Obj | undefined, compatibility: Obj | undefined, showCompatibility = true) {
  const latestIsIncompatible = compatibility?.latestVersionStatus === 'broken' || compatibility?.latestVersionStatus === 'unsupported';
  const status = str(compatibility?.status);
  if (showCompatibility && compatibility && (status === 'graceful' || status === 'unsupported' || status === 'broken')) {
    const targetVersion = str(compatibility.recommendedVersion) || null;
    const recommendation = versionLabel(targetVersion) || str(compatibility.recommendedRange);
    return { title: COMPATIBILITY_TITLES[status]!, detail: str(compatibility.message) || (recommendation ? `Use ${recommendation} for full support.` : 'Update for full support.'),
      updateCommand: targetVersion || latestIsIncompatible ? null : str(advisory?.updateCommand) || null, emphasis: status === 'graceful' ? 'normal' : 'strong', targetVersion };
  }
  if (!advisory || !str(advisory.status) || advisory.status === 'current' || advisory.status === 'unknown' || latestIsIncompatible) return null;
  const latest = versionLabel(advisory.latestVersion);
  return { title: 'Update available', detail: str(advisory.message) || (latest ? `Update available: install ${latest}.` : 'Update available: install the latest provider version.'),
    updateCommand: str(advisory.updateCommand) || null, emphasis: 'normal', targetVersion: null as string | null };
}

const pendingUpdates = new WeakMap<object, Set<string>>();
const pendingOf = (host: object) => { let set = pendingUpdates.get(host); if (!set) pendingUpdates.set(host, set = new Set()); return set; };

/**
 * versionAdvisoryNode for one instance: the same popover from the list row (`mode` "list") and
 * the editor header. `show` false draws nothing.
 */
export function advisoryView(host: UpkeepHost, instanceId: string, provider: Obj | undefined, enabled: boolean, mode: 'list' | 'editor') {
  const compatibility = enabled ? obj(provider?.compatibilityAdvisory) : undefined;
  const presentation = provider ? versionAdvisoryPresentation(provider.versionAdvisory ? obj(provider.versionAdvisory) : undefined, provider.compatibilityAdvisory ? obj(provider.compatibilityAdvisory) : undefined, enabled) : null;
  const readOnly = !host.writable;
  const hasCompatibilityWarning = compatibility !== undefined && !!str(compatibility.status) && compatibility.status !== 'supported' && compatibility.status !== 'unknown';
  const canInstallRecommended = !readOnly && !!provider && !!str(obj(provider.compatibilityAdvisory).message) && !!str(obj(provider.compatibilityAdvisory).recommendedVersion)
    && obj(provider.versionAdvisory).canInstallVersion === true;
  const canRunUpdate = !readOnly && !!provider && isProviderSettingsUpdateCandidate(provider);
  const hasAction = !!presentation && (presentation.targetVersion ? canInstallRecommended : canRunUpdate);
  const updating = pendingOf(host).has(instanceId) || isProviderUpdateActive(provider);
  const command = presentation?.updateCommand ?? '';
  return {
    show: presentation !== null, mode, popId: `provider-advisory-${mode}-${instanceId}`, instanceId,
    label: presentation ? `${presentation.title} — view details` : '', title: presentation?.title ?? '', detail: presentation?.detail ?? '',
    strong: presentation?.emphasis === 'strong', warning: hasCompatibilityWarning,
    action: !hasAction ? '' : updating ? 'Updating' : presentation!.targetVersion ? `Install ${versionLabel(presentation!.targetVersion)}` : 'Update now',
    target: presentation?.targetVersion ?? '', updating, command, divider: hasAction && command !== '',
  };
}

// --- Update all (ProviderUpdatesAction) --------------------------------------------------------

const updateAllPending = new WeakMap<object, boolean>();
/** Every connected environment: the focused one, then the background ones (the reference's useEnvironments()). */
export function updateTargets(host: UpkeepHost, native: Native | null): UpdateTarget[] {
  const targets: UpdateTarget[] = [];
  if (host.ready && host.call) targets.push(focusedTarget(host as unknown as T3Client, native));
  for (const entry of fleet.entries.values()) {
    if (entry.phase !== 'connected' || !Object.keys(entry.config).length || entry.environmentId === host.environmentId) continue;
    const saved = fleet.saved.find(candidate => str(candidate.environmentId) === entry.environmentId);
    targets.push(fleetTarget(entry, native, str(saved?.label)));
  }
  return targets;
}
type Machine = { key: string; label: string; target: UpdateTarget; candidates: Obj[] };
function machinesOf(targets: readonly UpdateTarget[]): Machine[] {
  return targets.flatMap(target => {
    const candidates = collectProviderUpdateCandidates(target.providers).filter(candidate => canOneClickUpdateProviderCandidate(candidate, target.providers));
    return candidates.length > 0 ? [{ key: target.key, label: target.label, target, candidates }] : [];
  });
}
/** The Providers header's "Update all": hidden with nothing to update unless a run is in flight. */
export function updateAllView(host: UpkeepHost, targets: readonly UpdateTarget[] = updateTargets(host, null)) {
  const machines = machinesOf(targets), pending = updateAllPending.get(host) === true;
  return { show: machines.length > 0 || pending, pending, label: pending ? 'Updating…' : 'Update all',
    tip: machines.map(machine => `${machine.label}: ${machine.candidates.map(candidate => providerDisplayName(str(candidate.driver))).join(', ')}`).join('\n') };
}

/** handleUpdate: every outdated provider on every connected machine at once, then one toast. */
export async function runUpdateAll(host: UpkeepHost, targets: readonly UpdateTarget[]): Promise<void> {
  if (updateAllPending.get(host) === true) return;
  const machines = machinesOf(targets);
  if (machines.length === 0) return;
  updateAllPending.set(host, true);
  try {
    const runs = await Promise.all(machines.flatMap(({ label, target, candidates }) => candidates.map(async (candidate): Promise<ProviderUpdateRun> => {
      let result: ProviderUpdateRun['result'];
      try { result = success({ providers: providersOf(await target.request('server.updateProvider', { provider: str(candidate.driver), instanceId: str(candidate.instanceId) })) }); }
      catch (error) { result = failure(error, letGo(error)); }
      return { machineLabel: label, driver: str(candidate.driver), instanceId: str(candidate.instanceId), result };
    })));
    const view = getProviderUpdateRunToastView(runs);
    if (view) pushToast(hostClient(host), { kind: view.type === 'success' ? 'success' : view.type === 'error' ? 'error' : view.type === 'loading' ? 'loading' : 'warning', title: view.title, description: view.description, stacked: true });
  } finally { updateAllPending.set(host, false); }
}

/** runProviderUpdate: one instance on this environment, "Update now" or "Install <recommended>". */
export async function runProviderUpdate(host: UpkeepHost, target: UpdateTarget, instanceId: string, targetVersion: string): Promise<void> {
  const provider = arr(host.config.providers).find(candidate => candidate.instanceId === instanceId);
  if (!provider || !host.writable) return;
  const pending = pendingOf(host);
  if (pending.has(instanceId)) return;
  const view = advisoryView(host, instanceId, provider, provider.enabled !== false, 'editor');
  if (!view.action || view.updating || (view.target || '') !== targetVersion) return;
  pending.add(instanceId);
  try { await target.request('server.updateProvider', { provider: str(provider.driver), instanceId, ...(targetVersion ? { targetVersion } : {}) }); }
  catch (error) {
    if (!letGo(error)) pushToast(hostClient(host), { kind: 'error', title: `Could not update ${providerDisplayName(str(provider.driver))}`,
      description: error instanceof Error && error.message ? error.message : 'The provider update command could not be started.', stacked: true });
  } finally { pending.delete(instanceId); }
}

// --- The editor's upkeep fields ----------------------------------------------------------------

/** The upkeep parts of one instance editor: the URL auth block, the ACP section and the open custom model editor. */
export function editorUpkeep(host: UpkeepHost, instanceId: string, driver: string, provider: Obj | undefined) {
  return { urlAuth: urlAuthView(host, provider), acp: acpSectionView(host, instanceId, provider),
    modelEditor: customModelEditorView(host, instanceId, driver, arr(provider?.models)) };
}

/**
 * The Providers page with its upkeep fields: each row's and the editor's advisory popover, Update all, the editor's upkeep parts.
 * `all` is the window's client: Update all covers every connected environment whichever one the page shows.
 */
export function withUpkeep(host: UpkeepHost, page: ReturnType<typeof providerPage>, all: UpkeepHost = host) {
  const live = arr(host.config.providers), providerOf = (id: string) => live.find(candidate => candidate.instanceId === id);
  const editors = page.editors.map(editor => editorUpkeep(host, editor.id, editor.driver, providerOf(editor.id)));
  return {
    // An open custom model editor owns Escape (its Cancel), so Settings' Back gives it up (app-settings.contract).
    ...page, updateAll: updateAllView(all), escapeOwned: editors.some(upkeep => upkeep.modelEditor.length > 0),
    rows: page.rows.map(row => ({ ...row, update: advisoryView(host, row.id, providerOf(row.id), row.enabled, 'list') })),
    editors: page.editors.map((editor, index) => {
      const upkeep = editors[index]!, editing = upkeep.modelEditor[0]?.slug ?? '';
      return { ...editor, update: advisoryView(host, editor.id, providerOf(editor.id), editor.enabled, 'editor'), upkeep,
        models: editor.models.map(model => ({ ...model, editing: model.custom && model.slug === editing })) };
    }),
  };
}

// --- The ops -----------------------------------------------------------------------------------

/** Copy update command (useCopyToClipboard with the reference's toasts). */
async function copyCommand(host: UpkeepHost, native: Native, instanceId: string): Promise<void> {
  const provider = arr(host.config.providers).find(candidate => candidate.instanceId === instanceId);
  const instance = obj(obj(obj(host.config.settings).providerInstances)[instanceId]);
  const name = str(instance.displayName).trim() || providerDisplayName(str(provider?.driver ?? instance.driver));
  const command = provider ? advisoryView(host, instanceId, provider, provider.enabled !== false, 'editor').command : '';
  try {
    if (!command) throw new ClientError('This provider has no update command.');
    const copied = obj(await native.later({ op: 'copyText', text: command }));
    if (copied.ok !== true) throw new ClientError('Could not copy the command.');
    pushToast(hostClient(host), { kind: 'success', title: `${name} update command copied`, description: 'Run it in a terminal when you are ready to update.' });
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(hostClient(host), { kind: 'error', title: `Could not copy ${name} update command`, description: error instanceof Error ? error.message : '', stacked: true });
  }
}

/** The custom model editor's Save (handleSaveEdit): the edited entry replaces its slug in `config.customModels`, and the editor closes. */
async function saveCustomModel(host: UpkeepHost, native: Native, instanceId: string, next: ReturnType<typeof customModelEditorOp>): Promise<void> {
  if (!next || !host.writable) return;
  customModelEditorOp(host, host, 'saved', instanceId, '', '');
  try {
    await upsertInstance(host, native, instanceId, (instance, driver) => {
      const config = obj(instance.config);
      const entries = readCustomModelEntries(config.customModels).map(entry => entry.slug === next.slug ? next : entry);
      return { ...instance, config: { ...config, customModels: storedCustomModels(driver, entries) } };
    });
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(hostClient(host), { kind: 'error', title: 'Could not update provider instance', description: error instanceof Error && error.message ? error.message : 'The settings update failed.' });
  }
}

/**
 * One `upkeep:<what>` op from the Providers page or a toast. `id` is the instance (or the launch
 * prompt's key), `field` and `value` as each op names them.
 */
export async function providerUpkeepOp(host: UpkeepHost, native: Native, op: string, id: string, field: string, value: string): Promise<void> {
  const what = op.slice('upkeep:'.length);
  const toast = (entry: { kind: 'success' | 'error' | 'warning'; title: string; description?: string }) => pushToast(hostClient(host), entry);
  try {
    if (what === 'update') await runProviderUpdate(host, host instanceof FleetSetupHost ? fleetTarget(host.entry, native) : focusedTarget(host as unknown as T3Client, native), id, value);
    else if (what === 'update-all') await runUpdateAll(host, updateTargets(host, native));
    else if (what === 'update-launch') await runLaunchUpdates(host as unknown as T3Client, id, primaryTarget(host as unknown as T3Client, native));
    else if (what === 'copy-command') await copyCommand(host, native, id);
    else if (what.startsWith('acp-')) await acpOp(host, native, what.slice(4), id, field, value, toast);
    else if (what.startsWith('cm-')) {
      const next = customModelEditorOp(host, host, what.slice(3), id, field, value);
      if (what === 'cm-save') await saveCustomModel(host, native, id, next);
    } else throw new ClientError(`Unknown action: ${op}`);
  } finally {
    // A toast pushed here shows now, not at the shell's next tick.
    await wakeShell(native);
  }
}

export { UPDATE_TIMEOUT_SECONDS };
