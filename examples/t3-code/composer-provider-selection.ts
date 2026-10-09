// The composer's provider and model, decided as T3 Code decides them (MIT; see
// LICENSE-T3). Sources: apps/web/src/providerInstances.ts
// (deriveProviderInstanceEntries, sortProviderInstanceEntries,
// isProviderInstancePickerReady, resolveSelectableProviderInstanceEntry),
// components/ChatView.logic.ts (resolveComposerProviderSelection),
// components/chat/ChatComposer.tsx (noProviderAvailable, showProviderUnavailable,
// providerSetupInstanceId), modelSelection.ts (getAppModelOptionsForInstance,
// resolveAppModelSelection, resolveAppModelSelectionForInstance), providerModels.ts
// (getDefaultServerModel), composerDraftStore.ts (deriveEffectiveComposerModelState)
// and packages/contracts/src/model.ts (DEFAULT_MODEL_BY_PROVIDER).
//
// One rule for the placeholder, the control row, the model trigger and the send:
// an instance the thread or draft asked for stays selected while it is enabled and
// not "unavailable", whatever its probe status (a signed-out Codex keeps its picker);
// otherwise a ready instance, then one whose probe did not fail. The clone reads
// `enabled` from the provider snapshot (the server reconciles it with settings), as
// every other provider list here does.
import { arr, obj, str, type Obj } from './domain';
import { resolveSelectableModel } from './r3-composer-controls-model';
import { providerLock } from './composer-controls-commands';
import { applyPickerPrefs } from './settings-b-models';

export const NO_PROVIDER_INSTANCE_ID = 't3code_no_provider';
export const DEFAULT_MODEL = 'gpt-6-astra';
export const ANTIGRAVITY_DEFAULT_MODEL = 'antigravity-default';
export const DEFAULT_MODEL_BY_PROVIDER: Record<string, string> = {
  codex: DEFAULT_MODEL, claudeAgent: 'claude-fable-5-1', cursor: 'auto', grok: 'grok-build', acpRegistry: 'default',
  pi: 'default', opencode: 'openai/gpt-5', antigravity: ANTIGRAVITY_DEFAULT_MODEL,
};

export type ProviderEntry = {
  instanceId: string; driverKind: string; continuationGroupKey: string; enabled: boolean; installed: boolean;
  status: string; isDefault: boolean; isAvailable: boolean; snapshot: Obj; models: Obj[];
};

/** deriveProviderInstanceEntries + sortProviderInstanceEntries: one entry per instance, each driver's default first. */
export function providerEntries(providers: Obj[]): ProviderEntry[] {
  const entries = providers.map(snapshot => ({
    instanceId: str(snapshot.instanceId), driverKind: str(snapshot.driver), continuationGroupKey: str(obj(snapshot.continuation).groupKey),
    enabled: snapshot.enabled === true, installed: snapshot.installed === true, status: str(snapshot.status),
    // defaultInstanceIdForDriver: the default instance's id is its driver's slug.
    isDefault: str(snapshot.instanceId) === str(snapshot.driver), isAvailable: snapshot.availability !== 'unavailable',
    snapshot, models: arr(snapshot.models),
  }));
  const byKind = new Map<string, ProviderEntry[]>();
  for (const entry of entries) byKind.set(entry.driverKind, [...byKind.get(entry.driverKind) ?? [], entry]);
  return [...byKind.values()].flatMap(bucket => [...bucket.filter(entry => entry.isDefault), ...bucket.filter(entry => !entry.isDefault)]);
}

/** isProviderInstancePickerReady. */
export const isProviderInstancePickerReady = (entry: ProviderEntry): boolean => entry.enabled && entry.isAvailable && entry.status === 'ready';
const isSelectable = (entry: ProviderEntry): boolean => entry.enabled && entry.isAvailable;

/**
 * resolveSelectableProviderInstanceEntry: the requested instance while it is
 * enabled and available; else a ready one, else one whose probe did not fail. An
 * errored instance is kept only when it was asked for, never invented as a default.
 */
export function resolveSelectableProviderInstanceEntry(entries: ProviderEntry[], instanceId: string | undefined): ProviderEntry | undefined {
  if (instanceId !== undefined) {
    const requested = entries.find(entry => entry.instanceId === instanceId);
    if (requested && isSelectable(requested)) return requested;
  }
  return entries.find(isProviderInstancePickerReady) ?? entries.find(entry => isSelectable(entry) && entry.status !== 'error');
}

/** resolveComposerProviderSelection: the same enabled instance for the composer, provider status and chat actions. */
export function resolveComposerProviderSelection(input: { entries: ProviderEntry[]; candidateInstanceIds: Array<string | null | undefined>;
  lockedProvider: string | null; lockedInstanceId: string | null | undefined }) {
  const requestedInstanceId = input.candidateInstanceIds.find((candidate): candidate is string => !!candidate && candidate !== NO_PROVIDER_INSTANCE_ID);
  const requestedDriverKind = input.lockedProvider ?? input.entries.find(entry => entry.instanceId === requestedInstanceId)?.driverKind
    ?? input.entries[0]?.driverKind ?? 'unconfigured';
  const lockedContinuationGroupKey = input.lockedProvider ? (input.entries.find(entry => entry.instanceId === input.lockedInstanceId)?.continuationGroupKey || null) : null;
  // Missing metadata must not move Antigravity history into another Google profile.
  const requiresExactInstance = input.lockedProvider === 'antigravity' && !!input.lockedInstanceId && lockedContinuationGroupKey === null;
  const compatible = input.entries.filter(entry => (!input.lockedProvider || entry.driverKind === input.lockedProvider)
    && (!lockedContinuationGroupKey || entry.continuationGroupKey === lockedContinuationGroupKey)
    && (!requiresExactInstance || entry.instanceId === input.lockedInstanceId));
  const selectedProviderEntry = input.candidateInstanceIds
    .map(candidate => compatible.find(entry => entry.instanceId === candidate && entry.enabled && entry.isAvailable))
    .find(entry => entry !== undefined)
    ?? resolveSelectableProviderInstanceEntry(compatible.filter(entry => entry.driverKind === requestedDriverKind), undefined)
    ?? resolveSelectableProviderInstanceEntry(compatible, undefined);
  const unavailableProviderInstanceId = selectedProviderEntry ? undefined
    : input.lockedProvider ? (input.lockedInstanceId || requestedInstanceId) : requestedInstanceId;
  return { selectedProviderEntry, requestedDriverKind, lockedContinuationGroupKey, unavailableProviderInstanceId };
}

// ── Models ─────────────────────────────────────────────────────────────────

type Prefs = object;
/** readCustomModelEntries: strings or { slug, name } records, trimmed and deduplicated. */
function customEntries(value: unknown): Obj[] {
  const seen = new Set<string>(), out: Obj[] = [];
  for (const raw of Array.isArray(value) ? value as unknown[] : []) {
    const record = typeof raw === 'string' ? { slug: raw } : raw && typeof raw === 'object' ? raw as Obj : null;
    const slug = str(record?.slug).trim();
    if (!slug || seen.has(slug)) continue;
    seen.add(slug);
    out.push({ slug, name: str(record?.name).trim() || slug, isCustom: true });
  }
  return out;
}
/** readInstanceCustomModels: the instance's own list, else (a default instance only) the legacy per-driver one. */
function instanceCustomModels(settings: Obj, instanceId: string, driver: string): Obj[] {
  if (driver === 'antigravity') return [];
  const config = obj(obj(obj(settings.providerInstances)[instanceId]).config);
  if (Array.isArray(config.customModels)) return customEntries(config.customModels);
  return instanceId === driver ? customEntries(obj(obj(settings.providers)[driver]).customModels) : [];
}
/** getAppModelOptionsForInstance: the instance's built-in models, its custom models, the device's hidden models and order. */
export function instanceModelOptions(local: Prefs, settings: Obj, entry: ProviderEntry, selectedModel?: string | null): Obj[] {
  const options = entry.models.filter(model => model.isCustom !== true);
  const seen = new Set(options.map(model => str(model.slug)));
  for (const custom of instanceCustomModels(settings, entry.instanceId, entry.driverKind).slice(0, 32)) {
    if (str(custom.slug).length > 256 || seen.has(str(custom.slug))) continue;
    seen.add(str(custom.slug)); options.push(custom);
  }
  const kept = applyPickerPrefs(local, entry.instanceId, options);
  // appendUnavailableDynamicModelSelection: an OpenCode or Antigravity model the catalog dropped stays as an unavailable row.
  const slug = str(selectedModel).trim();
  if ((entry.driverKind !== 'opencode' && entry.driverKind !== 'antigravity') || !slug) return kept;
  if (entry.driverKind === 'antigravity' && slug === ANTIGRAVITY_DEFAULT_MODEL) return kept;
  if (resolveSelectableModel(entry.driverKind, slug, entry.models) || kept.some(option => option.slug === slug)) return kept;
  if (options.some(option => option.slug === slug)) return kept; // hidden by the device: the preference stands
  return [...kept, { slug, name: slug, isCustom: false, isUnavailable: true }];
}

/** getDefaultServerModel: the default instance's default built-in model, else the driver's fallback. */
export function defaultServerModel(providers: Obj[], driver: string): string {
  const models = arr(providers.find(provider => provider.instanceId === driver)?.models);
  return str(models.find(model => model.isDefault === true && model.isCustom !== true)?.slug)
    || str(models.find(model => model.isCustom !== true)?.slug) || str(models[0]?.slug) || DEFAULT_MODEL_BY_PROVIDER[driver] || DEFAULT_MODEL;
}

/** resolveAppModelSelection: the driver's default instance (else the first enabled one) and its options, else its default model. */
export function resolveAppModelSelection(local: Prefs, settings: Obj, providers: Obj[], driver: string, selectedModel: string | null | undefined): string {
  const requested = providers.find(provider => provider.instanceId === driver);
  const resolved = requested?.enabled === true ? str(requested.driver) : str(providers.find(provider => provider.enabled === true)?.driver, 'codex');
  const entry = providerEntries(providers).find(candidate => candidate.instanceId === resolved);
  const options = entry ? instanceModelOptions(local, settings, entry, selectedModel) : [];
  return resolveSelectableModel(resolved, selectedModel, options) || defaultServerModel(providers, resolved);
}

/** resolveAppModelSelectionForInstance: the model when the instance offers it, else (OpenCode, Antigravity) the kept slug, else its default. */
export function resolveAppModelSelectionForInstance(local: Prefs, settings: Obj, providers: Obj[], instanceId: string, selectedModel: string | null | undefined,
  preserveUnavailableSelection = false): string | null {
  const entry = providerEntries(providers).find(candidate => candidate.instanceId === instanceId);
  if (!entry) return null;
  const options = instanceModelOptions(local, settings, entry, preserveUnavailableSelection ? selectedModel : null);
  const resolved = resolveSelectableModel(entry.driverKind, selectedModel, options);
  if (resolved) return resolved;
  const kept = str(selectedModel).trim();
  if (preserveUnavailableSelection && (entry.driverKind === 'opencode' || entry.driverKind === 'antigravity') && kept
    && !resolveSelectableModel(entry.driverKind, kept, entry.models) && (entry.driverKind !== 'antigravity' || kept !== ANTIGRAVITY_DEFAULT_MODEL)) return kept;
  return str(options.find(option => option.isDefault === true)?.slug) || str(options[0]?.slug) || null;
}

// ── The composer's selection ───────────────────────────────────────────────

/** What the composer reads; a test client may leave the thread and shell out. */
export type SelectionSource = {
  config: Obj; providerId: string; modelId: string; projectId?: string; threadId?: string; projection?: Obj;
  shell?: { projects: Obj[] }; local?: object; configLive?: boolean;
};
/** hasProviderSetup (ProviderStatusBanner.tsx): the instance can be signed in or installed from Settings. */
export const hasProviderSetup = (provider: Obj): boolean => provider.driver === 'antigravity' || obj(provider.setup).canAuthenticate === true || obj(provider.setup).canInstall === true;

/** The project's default selection (the project override, the project, then the server's default). */
function projectDefaultSelection(client: SelectionSource): Obj {
  const settings = obj(client.config.settings), projectId = client.projectId ?? '';
  const project = client.shell?.projects.find(entry => entry.id === projectId);
  const override = obj(obj(obj(settings.projectSettingsOverrides)[projectId]).defaultModelSelection);
  return str(override.instanceId) ? override : str(obj(project?.defaultModelSelection).instanceId) ? obj(project?.defaultModelSelection) : obj(settings.defaultModelSelection);
}

/**
 * ChatComposer's model state: the selected instance (resolveComposerProviderSelection over the
 * composer's choice, the thread's runtime, the thread's and the project's selections), the model
 * it sends (deriveEffectiveComposerModelState) and the no-provider chrome. `fanout` is
 * multipleModelSelections !== null.
 */
export function composerSelection(client: SelectionSource, fanout = false) {
  const providers = arr(client.config.providers), settings = obj(client.config.settings), local = client.local ?? {};
  const entries = providerEntries(providers);
  const projection = client.threadId ? obj(client.projection) : {};
  const thread = obj(projection.thread), threadSelection = obj(thread.modelSelection);
  // deriveThreadRuntime is null until the thread has a run or a provider thread.
  const runtimeInstance = client.threadId && (arr(projection.runs).length > 0 || !!str(thread.activeProviderThreadId)) ? str(thread.providerInstanceId) : '';
  const projectSelection = projectDefaultSelection(client);
  const lock = client.threadId && client.projection ? providerLock({ threadId: client.threadId, projection: client.projection, config: client.config }) : null;
  const lockedInstanceId = runtimeInstance || str(threadSelection.instanceId);
  const { selectedProviderEntry: entry, requestedDriverKind, unavailableProviderInstanceId } = resolveComposerProviderSelection({ entries,
    candidateInstanceIds: [client.providerId, runtimeInstance, str(threadSelection.instanceId), str(projectSelection.instanceId)],
    lockedProvider: lock?.driver ?? null, lockedInstanceId });
  const noProviderAvailable = !entry && !fanout;
  // Before the catalog arrives every thread resolves to "no provider"; only the chrome waits (providerCatalogPending).
  const catalogKnown = client.configLive === true || providers.length > 0;
  const showProviderUnavailable = noProviderAvailable && catalogKnown;
  const providerSetupInstanceId = noProviderAvailable
    ? unavailableProviderInstanceId ?? (lock === null ? entries.find(candidate => hasProviderSetup(candidate.snapshot))?.instanceId : undefined) ?? ''
    : '';
  const instanceId = entry?.instanceId ?? NO_PROVIDER_INSTANCE_ID, driver = entry?.driverKind ?? requestedDriverKind;
  // The composer's own choice (the draft's modelSelectionByProvider) wins for its instance; else the thread's, then the project's model.
  const own = client.providerId === instanceId && client.modelId ? client.modelId : '';
  let model: string;
  if (own) {
    model = resolveAppModelSelectionForInstance(local, settings, providers, instanceId, own, true)
      ?? (driver === 'antigravity' ? '' : resolveAppModelSelection(local, settings, providers, driver, own));
  } else {
    const candidate = str(threadSelection.model) || str(projectSelection.model) || null;
    const preserve = !!entry && str(threadSelection.instanceId) === instanceId;
    model = (entry ? resolveAppModelSelectionForInstance(local, settings, providers, instanceId, candidate, preserve) : null)
      ?? (driver === 'antigravity' && entry ? '' : null) ?? resolveAppModelSelection(local, settings, providers, driver, candidate);
  }
  return { entry, provider: entry?.snapshot, instanceId: entry ? instanceId : '', driver, model: entry ? model : '',
    noProviderAvailable, showProviderUnavailable, providerSetupInstanceId };
}
