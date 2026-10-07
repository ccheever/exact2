// Pinned mobile ThreadSettingsSheet/modelOptions/thread-settings-options at365aa87982.
// @ref llp/1106.005-composer-and-transcript.decision.md#settings-ownership
import { mobileClient, mobileCommand } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, providerAvailable, type Native, type Files } from './shared/protocol';
import { favoriteKeys } from './shared/model-catalog';
import { applyOptionChoice, lockedProviderReason } from './shared/composer-controls-commands';
import { rememberOptions } from './shared/composer-controls';
import { resolvedCurrent, optionLabel, reportedSelection, type Selection } from './shared/r3-composer-controls-model';
import { runtimeModes } from './shared/composer-presentation';
import { letGo } from './shared/let-go';
import { mobileProviderIconURL } from './environment-detail';

export interface ComposerCatalogRow { key: string; kind: string; id: string; providerId: string; label: string; subtitle: string;
  driver: string; iconUrl: string; selected: boolean; disabled: boolean; reason: string; favorite: boolean; legacy: boolean;
  isDefault: boolean; collapsed: boolean; count: number; first: boolean; last: boolean }
export interface ComposerChoice { id: string; label: string; description: string; selected: boolean; disabled: boolean; last: boolean }
export interface ComposerOption { id: string; label: string; value: string; boolean: boolean; on: boolean; choices: ComposerChoice[] }
export interface ComposerSettingsSnapshot { revision: number; open: boolean; busy: boolean; canEdit: boolean; canSave: boolean;
  saveLabel: string; query: string; filter: string; showLegacy: boolean; hasLegacy: boolean; empty: string; pending: boolean;
  models: ComposerCatalogRow[]; filters: ComposerChoice[]; options: ComposerOption[]; runtimes: ComposerChoice[]; runtimeLabel: string;
  error: string; environmentId: string; threadId: string; }
export interface ComposerActionResult { revision: number; message: string; closed: boolean; environmentId: string; threadId: string }
interface SettingsSession { owner: string; applied: string; pending: Selection | null; expanded: Set<string>; open: boolean; busy: boolean; error: string }
const sessions = new WeakMap<T3Client, SettingsSession>();
const owner = (client: T3Client) => JSON.stringify([client.generation, client.draftKey]);
const applied = (client: T3Client) => JSON.stringify([client.providerId, client.modelId]);
function session(client: T3Client): SettingsSession {
  let state = sessions.get(client);
  if (!state || state.owner !== owner(client)) {
    state = { owner: owner(client), applied: applied(client), pending: null, expanded: new Set(), open: false, busy: false, error: '' };
    sessions.set(client, state);
  }
  return state;
}
function providerLabel(provider: Obj): string {
  return str(provider.displayName) || ({ codex: 'Codex', claudeAgent: 'Claude', pi: 'Pi' }[str(provider.driver)] ?? str(provider.instanceId));
}
function catalog(client: T3Client) {
  const entries: { key: string; provider: Obj; model: Obj; unavailable: boolean }[] = [];
  for (const provider of arr(client.config.providers)) {
    // The mobile catalog retains configured Antigravity selection, even if setup becomes unavailable.
    if (provider.enabled !== true || provider.installed !== true || obj(provider.auth).status === 'unauthenticated'
      || provider.driver === 'antigravity' && provider.availability === 'unavailable') continue;
    for (const model of arr(provider.models)) entries.push({ key: `${str(provider.instanceId)}:${str(model.slug)}`, provider, model, unavailable: !providerAvailable(provider) });
  }
  const key = `${client.providerId}:${client.modelId}`;
  if (client.providerId && client.modelId && !entries.some(entry => entry.key === key)) {
    const provider = arr(client.config.providers).find(provider => provider.instanceId === client.providerId)
      ?? { instanceId: client.providerId, ...obj(obj(obj(client.config.settings).providerInstances)[client.providerId]) };
    const model = arr(provider.models).find(model => model.slug === client.modelId) ?? { slug: client.modelId, name: client.modelId };
    entries.push({ key, provider, model, unavailable: true });
  }
  return entries;
}
const selectedOptions = (client: T3Client, state: SettingsSession): Selection => state.pending ?? { instanceId: client.providerId, model: client.modelId, options: client.modelOptions };
function descriptorRows(client: T3Client, state: SettingsSession): Obj[] {
  const selected = selectedOptions(client, state), provider = arr(client.config.providers).find(provider => provider.instanceId === selected.instanceId);
  const model = arr(provider?.models).find(model => model.slug === selected.model);
  return arr(obj(model?.capabilities).optionDescriptors).filter(descriptor => ['select', 'boolean'].includes(str(descriptor.type)));
}
function editable(client: T3Client) { return client.writable && !client.pending && !client.busy; }
function selectionReason(client: T3Client, provider: Obj, model: Obj): string {
  if (!providerAvailable(provider)) return 'Set up this provider on web or desktop, or select another model.';
  const locked = lockedProviderReason(client, str(provider.instanceId));
  if (locked) return locked;
  if (client.threadId && provider.requiresNewThreadForModelChange === true && (provider.instanceId !== client.providerId || model.slug !== client.modelId)) return 'Start a new thread to change this model.';
  return '';
}

/** The catalog is mobile presentation over authoritative config; search never changes selected model. */
export function mobileComposerSettings(query = '', filter = '', showLegacy = false, client: T3Client = mobileClient): ComposerSettingsSnapshot {
  const state = session(client), selected = selectedOptions(client, state), entries = catalog(client), favorites = favoriteKeys(client.local.favoriteModels);
  const canEdit = editable(client) && !state.busy, needle = query.trim().toLocaleLowerCase();
  const groups = new Map<string, typeof entries>();
  for (const entry of entries) { const id = str(entry.provider.instanceId), group = groups.get(id) ?? []; group.push(entry); groups.set(id, group); }
  const models: ComposerCatalogRow[] = [], filters: ComposerChoice[] = [
    { id: '', label: 'All providers', description: '', selected: !filter, disabled: false, last: false },
    { id: '@favorites', label: 'Favorites', description: '', selected: filter === '@favorites', disabled: false, last: false }];
  for (const [id, group] of groups) {
    const provider = group[0]!.provider, label = providerLabel(provider), driver = str(provider.driver), iconUrl = mobileProviderIconURL(provider.iconUrl);
    filters.push({ id, label, description: '', selected: filter === id, disabled: false, last: false });
    if (filter && filter !== '@favorites' && id !== filter) continue;
    const visible = group.filter(entry => (showLegacy || favorites.has(entry.key) || entry.model.isLegacy !== true || entry.model.slug === selected.model && id === selected.instanceId)
      && (filter !== '@favorites' || favorites.has(entry.key)) && (!needle || [str(entry.model.name), str(entry.model.subProvider), str(entry.model.slug), label].some(value => value.toLocaleLowerCase().includes(needle))))
      .sort((a, b) => Number(favorites.has(b.key)) - Number(favorites.has(a.key)));
    if (!visible.length) continue;
    const defaultExpanded = ['claudeAgent', 'codex', 'antigravity'].includes(driver) || id === selected.instanceId;
    const collapsed = !needle && !filter && (defaultExpanded ? state.expanded.has(id) : !state.expanded.has(id));
    const base = { providerId: id, driver, iconUrl, subtitle: '', selected: false, disabled: false, reason: '', favorite: false,
      legacy: false, isDefault: false, collapsed, count: visible.length, first: false, last: false };
    models.push({ ...base, key: `provider:${id}`, kind: 'provider', id, label });
    if (!collapsed) visible.forEach((entry, index) => {
      const reason = selectionReason(client, provider, entry.model);
      models.push({ ...base, key: entry.key, kind: 'model', id: str(entry.model.slug), label: str(entry.model.name, str(entry.model.slug)),
        subtitle: str(entry.model.subProvider), selected: selected.instanceId === id && selected.model === entry.model.slug,
        disabled: !canEdit || !!reason || entry.unavailable, reason, favorite: favorites.has(entry.key), legacy: entry.model.isLegacy === true,
        isDefault: entry.model.isDefault === true, first: index === 0, last: index === visible.length - 1 });
    });
  }
  filters.forEach((row, index) => { row.last = index === filters.length - 1; });
  const reported = state.pending ? null : reportedSelection(client.projection);
  const options = descriptorRows(client, state).map(descriptor => {
    const current = resolvedCurrent(descriptor, selected.options), injected = Array.isArray(descriptor.promptInjectedValues) ? descriptor.promptInjectedValues : [];
    const choices = arr(descriptor.options).filter(option => option.id !== 'ultracode' && !injected.includes(option.id ?? null)).map(option => ({ id: str(option.id), label: str(option.label),
      description: str(option.description), selected: option.id === current, disabled: !canEdit, last: false }));
    choices.forEach((row, index) => { row.last = index === choices.length - 1; });
    return { id: str(descriptor.id), label: str(descriptor.label), value: optionLabel(descriptor, current, selected, reported), boolean: descriptor.type === 'boolean', on: current === true, choices };
  });
  const provider = arr(client.config.providers).find(provider => provider.instanceId === selected.instanceId), supported = provider?.supportedRuntimeModes;
  const available = runtimeModes.filter(mode => !Array.isArray(supported) || !supported.length || supported.includes(mode.mode));
  const runtime = available.some(mode => mode.mode === client.runtimeMode) ? client.runtimeMode : available[0]?.mode ?? client.runtimeMode;
  const runtimes = available.map((mode, index) => ({ id: mode.mode, label: mode.label, description: mode.description, selected: mode.mode === runtime, disabled: !canEdit, last: index === available.length - 1 }));
  const candidate = state.pending && entries.find(entry => entry.provider.instanceId === state.pending!.instanceId && entry.model.slug === state.pending!.model);
  return { revision: client.revision, open: state.open, busy: state.busy, canEdit, canSave: canEdit && (!state.pending || !!candidate && !candidate.unavailable && !selectionReason(client, candidate.provider, candidate.model)),
    saveLabel: state.pending ? 'Save' : 'Done', query, filter, showLegacy, hasLegacy: entries.some(entry => entry.model.isLegacy === true),
    empty: models.length ? '' : needle ? 'No matching models' : 'No models available', pending: !!state.pending, models, filters, options, runtimes,
    runtimeLabel: runtimes.find(row => row.selected)?.label ?? '', error: state.error, environmentId: client.environmentId, threadId: client.threadId };
}

/** Only UI staging is local. Save is one shared model command carrying all staged options. */
export async function mobileComposerSettingsAction(kind: string, id: string, value: string, native: Native | null | undefined, storage: Files,
  client: T3Client = mobileClient): Promise<ComposerActionResult> {
  const state = session(client), result = (message = '', closed = false) => ({ revision: client.revision, message, closed, environmentId: client.environmentId, threadId: client.threadId });
  const command = (op: string, id = '', value = '') => client === mobileClient ? mobileCommand([op, id, value], native, storage) : client.command(op, id, value, 0, native, storage);
  if (state.busy) return result('Wait for the current settings change to finish.');
  try {
    state.error = '';
    if (kind === 'open') { state.open = true; state.pending = null; state.applied = applied(client); state.expanded.clear(); return result(); }
    if (kind === 'cancel') { state.open = false; state.pending = null; return result('', true); }
    if (kind === 'toggle-provider') { if (!state.expanded.delete(id)) state.expanded.add(id); return result(); }
    if (kind === 'favorite') { const response = await command('favorite-model', id, value); state.error = response.message; return result(response.message); }
    if (!editable(client)) throw new ClientError('Wait for a writable connection to synchronize before changing settings.');
    if (kind === 'pick') {
      const entry = catalog(client).find(entry => entry.provider.instanceId === id && entry.model.slug === value);
      if (!entry || entry.unavailable) throw new ClientError('Set up this provider on web or desktop, or select another model.');
      const reason = selectionReason(client, entry.provider, entry.model); if (reason) throw new ClientError(reason);
      if (id === client.providerId && value === client.modelId) state.pending = null;
      else if (state.pending?.instanceId !== id || state.pending.model !== value) state.pending = { instanceId: id, model: value,
        options: (client.local.composerControls.stickyOptionsByModel[id]?.[value] ?? []).map(option => ({ ...option })) };
      return result();
    }
    if (kind === 'option') {
      const descriptor = descriptorRows(client, state).find(descriptor => descriptor.id === id);
      if (value === 'ultracode' || Array.isArray(descriptor?.promptInjectedValues) && descriptor.promptInjectedValues.includes(value))
        throw new ClientError('That option is not offered in the mobile picker.');
    }
    if (kind === 'option' && state.pending) {
      state.pending = { ...state.pending, options: applyOptionChoice(descriptorRows(client, state), state.pending.options, id, value) }; return result();
    }
    state.busy = true;
    if (kind === 'save') {
      if (!state.open) throw new ClientError('The settings session changed. Reopen settings.');
      if (state.owner !== owner(client) || state.applied !== applied(client)) throw new ClientError('The draft or model changed while settings were open. Reopen settings.');
      if (state.pending) {
        const pending = state.pending, entry = catalog(client).find(entry => entry.provider.instanceId === pending.instanceId && entry.model.slug === pending.model);
        if (!entry || entry.unavailable) throw new ClientError('Set up this provider on web or desktop, or select another model.');
        const reason = selectionReason(client, entry.provider, entry.model); if (reason) throw new ClientError(reason);
        // Shared rememberModel reads this memory when its single model-selection command is built.
        rememberOptions(client, pending.instanceId, pending.model, pending.options);
        const response = await command('model', pending.model, pending.instanceId); if (response.message) throw new ClientError(response.message);
        if (state.owner !== owner(client)) throw new ClientError('The draft changed while settings were saving.');
        const supported = entry.provider.supportedRuntimeModes;
        if (Array.isArray(supported) && supported.length && !supported.includes(client.runtimeMode)) {
          const mode = runtimeModes.find(mode => supported.includes(mode.mode));
          if (mode) { const next = await command('runtime', '', mode.mode); if (next.message) throw new ClientError(next.message); }
        }
      }
      state.pending = null; state.open = false; return result('', true);
    }
    if (kind !== 'option' && kind !== 'runtime') throw new ClientError('Unknown composer settings action.');
    const response = await command(kind === 'option' ? 'model-option' : 'runtime', id, value);
    if (response.message) throw new ClientError(response.message);
    return result();
  } catch (error) { if (letGo(error)) throw error; state.error = error instanceof Error ? error.message : 'Could not change thread settings.'; return result(state.error); }
  finally { state.busy = false; client.revision++; }
}
