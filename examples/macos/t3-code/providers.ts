// Settings → Providers: T3's ProviderSettingsPanel projected for Contract, and
// the provider-instance writes it offers. Every write reads fresh server
// settings first and changes only the selected instance (atomic mutation).
import { obj, str, arr, num, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pushToast } from './toast';
import { accentHsv } from './settings-b-accent';
import type { T3Client } from './client';
import { favoriteSlugs, groupModels, instancePrefs, runModelPrefOp, MODEL_PREF_OPS } from './settings-b-models';
import { DRIVERS, driverMeta, instanceEnabled, sameValue, versionLabel, providerSummary, versionAdvisory, checkedLabel,
  slugifyLabel, validateInstanceId, deriveAvailableInstanceId, type Driver, type DriverField } from './providers-meta';

export interface ProviderHost {
  config: Obj; ready: boolean; writable: boolean; local: { favoriteModels: string[] };
  rpc(native: Native, method: string, payload: Obj, write?: boolean): Promise<Obj>;
}
type Row = { id: string; instance: Obj; driver: string; isDefault: boolean; isDirty: boolean };
export type AcpAgent = { id: string; name: string; description: string; link: string; icon: string; version: string; distribution: string; added: boolean };
const ENV_NAME = /^[a-zA-Z_][a-zA-Z0-9_]*$/;
const settingsOf = (host: ProviderHost) => obj(host.config.settings);
const liveProviders = (host: ProviderHost) => arr(host.config.providers);

/** Rows in T3's order: each driver's default slot, then its custom instances, then unknown drivers. */
export function providerRows(settings: Obj, live: Obj[], target = ''): Row[] {
  const instances = obj(settings.providerInstances), legacy = obj(settings.providers), rows: Row[] = [];
  const visible = DRIVERS.filter(driver => driver.id !== 'cursor' || live.some(provider => provider.instanceId === 'cursor'));
  for (const driver of visible) {
    if (driver.defaultSlot) {
      const explicit = instances[driver.id] && typeof instances[driver.id] === 'object' ? obj(instances[driver.id]) : undefined;
      const legacyConfig = legacy[driver.id] && typeof legacy[driver.id] === 'object' ? obj(legacy[driver.id]) : undefined;
      const synthesized = legacyConfig ? (() => { const { enabled, ...rest } = legacyConfig; return { driver: driver.id, enabled, config: rest }; })() : undefined;
      const effective = explicit ?? synthesized;
      if (effective) {
        const isDirty = explicit !== undefined || !sameValue(legacyConfig, driver.legacyDefault);
        if (driver.id === 'codex' || driver.id === 'claudeAgent' || isDirty || instanceEnabled(effective) || target === driver.id) {
          rows.push({ id: driver.id, instance: effective, driver: driver.id, isDefault: true, isDirty });
        }
      }
    }
    for (const [id, entry] of Object.entries(instances)) {
      if (id !== driver.id && str(obj(entry).driver) === driver.id) rows.push({ id, instance: obj(entry), driver: driver.id, isDefault: false, isDirty: false });
    }
  }
  for (const [id, entry] of Object.entries(instances)) {
    if (!driverMeta(str(obj(entry).driver)) || !visible.some(driver => driver.id === str(obj(entry).driver))) {
      if (!rows.some(row => row.id === id)) rows.push({ id, instance: obj(entry), driver: str(obj(entry).driver), isDefault: DRIVERS.some(driver => driver.id === id), isDirty: false });
    }
  }
  return rows;
}

const hash = (text: string) => { let value = 5381; for (let i = 0; i < text.length; i++) value = ((value << 5) + value + text.charCodeAt(i)) | 0; return (value >>> 0).toString(36); };
const initials = (name: string) => name.split(/\s+/).filter(Boolean).slice(0, 2).map(part => part[0]!.toUpperCase()).join('') || '?';
function rowStatus(row: Row, provider: Obj | undefined) {
  const enabled = instanceEnabled(row.instance);
  const statusKey = enabled ? (['ready', 'warning', 'error', 'disabled'].includes(str(provider?.status)) ? str(provider?.status) : 'warning') : 'disabled';
  const summary = enabled ? providerSummary(provider) : { headline: 'Disabled', detail: '' };
  const compatibility = str(obj(provider?.compatibilityAdvisory).status);
  const incompatible = enabled && ['graceful', 'unsupported', 'broken'].includes(compatibility);
  const needsAttention = statusKey === 'warning' || statusKey === 'error';
  const inlineDetail = incompatible ? (compatibility === 'broken' ? 'Incompatible' : compatibility === 'unsupported' ? 'Unsupported' : 'Limited support') : summary.detail;
  const auth = obj(provider?.auth), authenticated = enabled && auth.status === 'authenticated';
  return { enabled, statusKey, summary, needsAttention, inlineDetail, authenticated, authEmail: str(auth.email).trim(), authLabel: authenticated ? str(auth.label) || str(auth.type) : '' };
}

const fieldRow = (instanceId: string, field: DriverField, value: string, extra: { placeholder?: string; redacted?: boolean; clearable?: boolean } = {}, first = false) => ({
  rowKey: `${instanceId}:${field.key}:${value}`, key: field.key, first, label: field.label, description: field.description,
  placeholder: extra.placeholder ?? field.placeholder, control: field.control as string, value, redacted: extra.redacted === true, clearable: extra.clearable === true,
  options: (field.options || []).map(option => ({ value: option.value, label: option.label, selected: option.value === (value || field.options![0]!.value) })),
});

function capabilityLabels(model: Obj): string {
  const descriptors = arr(obj(model.capabilities).optionDescriptors), labels: string[] = [];
  if (descriptors.some(item => item.id === 'fastMode' || (item.id === 'serviceTier' && item.type === 'select' && arr(item.options).some(option => option.id === 'fast' || option.label === 'Fast')))) labels.push('Fast mode');
  if (descriptors.some(item => item.id === 'thinking')) labels.push('Thinking');
  if (descriptors.some(item => item.type === 'select' && ['reasoningEffort', 'effort', 'reasoning', 'variant'].includes(str(item.id)))) labels.push('Reasoning');
  return labels.join(' · ');
}

// Environment rows that are being edited but not yet valid stay on this device
// until they can be published (T3's ProviderEnvironmentSection draft rows).
const envDrafts = new Map<string, Obj[]>();
function envRows(instanceId: string, instance: Obj, dedicated: Set<string>): Obj[] {
  const published = arr(instance.environment).filter(variable => !dedicated.has(str(variable.name)));
  const draft = envDrafts.get(instanceId);
  return draft ?? published.map(variable => ({ name: str(variable.name), value: str(variable.value), sensitive: variable.sensitive === true, ...(variable.valueRedacted === true ? { valueRedacted: true } : {}) }));
}

function editorFor(host: ProviderHost, row: Row, live: Obj[]) {
  const meta = driverMeta(row.driver), provider = live.find(candidate => candidate.instanceId === row.id);
  const status = rowStatus(row, provider), config = obj(row.instance.config);
  const advisory = versionAdvisory(provider, status.enabled);
  const displayName = str(row.instance.displayName).trim() || meta?.label || row.driver;
  const dedicated = new Set((meta?.env || []).map(field => field.key));
  const variables = envRows(row.id, row.instance, dedicated);
  const custom = row.driver === 'antigravity' ? [] : arr(config.customModels).map(entry => typeof entry === 'string' ? { slug: entry, name: entry } : obj(entry));
  const liveModels = arr(provider?.models).filter(model => model.isCustom !== true);
  const models = [...liveModels, ...custom.map(entry => ({ slug: str(entry.slug), name: str(entry.name) || str(entry.slug), isCustom: true, capabilities: entry.capabilities ?? arr(provider?.models).find(model => model.slug === entry.slug)?.capabilities ?? null }))];
  // ProviderModelsSection: favorites, visible, then hidden; order and visibility are device preferences (settings-b-models.ts).
  const favorites = favoriteSlugs(host.local, row.id), prefs = instancePrefs(host.local, row.id), hiddenSet = new Set(prefs.hiddenModels);
  const display = groupModels<Obj & { slug: string; isCustom: boolean }>(models.map(model => ({ ...(model as Obj), slug: str(model.slug), isCustom: model.isCustom === true })), favorites, hiddenSet, prefs.modelOrder);
  const favoriteCount = display.filter(entry => entry.group === 'favorite').length;
  const hiddenCount = display.filter(entry => !entry.model.isCustom && hiddenSet.has(entry.model.slug)).length;
  const builtIn = display.filter(entry => !entry.model.isCustom);
  return {
    id: row.id, key: row.id, driver: row.driver, title: displayName,
    nameRows: [{ key: `${row.id}:${str(row.instance.displayName)}` }], // the accent picker keeps its popover open across commits
    modelBlocks: [{ key: `${row.id}:${hash(JSON.stringify(config.customModels ?? null))}` }],
    displayName: str(row.instance.displayName), placeholder: meta?.label || 'Instance label', accent: str(row.instance.accentColor), ...accentHsv(str(row.instance.accentColor)),
    version: versionLabel(provider?.version), advisory: advisory ? (advisory.warning ? 'warning' : 'update') : '', advisoryTitle: advisory?.title || '',
    advisoryDetail: advisory?.detail || '', advisoryCommand: advisory?.command || '', ...providerUpdateAction(provider, advisory),
    statusLead: status.authenticated && status.authEmail ? `Authenticated as ${status.authEmail}${status.authLabel ? ` · ${status.authLabel}` : ''}` : status.summary.headline,
    statusDetail: status.inlineDetail ? `· ${status.inlineDetail}` : '', dot: status.needsAttention ? status.statusKey : '',
    canDelete: !row.isDefault, canReset: row.isDefault && row.isDirty, known: meta !== undefined,
    showSetup: (meta?.env.length || 0) > 0 || (row.driver === 'cursor' && obj(provider?.setup).canAuthenticate === false),
    setupNote: row.driver === 'cursor' && obj(provider?.setup).canAuthenticate === false ? "Using CURSOR_API_KEY. Remove it from this provider's environment to use browser sign-in." : '',
    envFields: (meta?.env || []).map(field => {
      const variable = arr(row.instance.environment).find(entry => entry.name === field.key);
      const redacted = variable?.valueRedacted === true;
      return fieldRow(row.id, field, redacted ? '' : str(variable?.value), { placeholder: redacted ? 'Stored secret - enter a new value to replace' : field.placeholder, redacted, clearable: variable !== undefined });
    }),
    fields: (meta?.fields || []).map((field, index) => fieldRow(row.id, field, str(config[field.key]), {}, index === 0)),
    variables: variables.map((variable, index) => ({ rowKey: `${row.id}:env:${index}:${str(variable.name)}:${variable.valueRedacted === true ? '' : str(variable.value)}:${variable.sensitive === true}:${variable.valueRedacted === true}`,
      index, name: str(variable.name), value: variable.valueRedacted === true ? '' : str(variable.value), sensitive: variable.sensitive === true,
      redacted: variable.valueRedacted === true, label: str(variable.name) || String(index + 1) })),
    hasVariables: variables.length > 0,
    models: display.map(({ model, group }, index) => {
      const previous = display[index - 1], next = display[index + 1], starts = !previous || previous.group !== group;
      const hidden = !model.isCustom && hiddenSet.has(model.slug);
      return { rowKey: `${row.id}:model:${model.slug}`, slug: model.slug, name: str(model.name) || model.slug, showSlug: str(model.name) !== model.slug && str(model.name) !== '',
        custom: model.isCustom, favorite: group === 'favorite', capabilities: capabilityLabels(obj(model)),
        groupLabel: !starts ? '' : group === 'hidden' ? 'Hidden from picker' : favoriteCount > 0 ? (group === 'favorite' ? 'Favorites' : 'All') : '', firstGroup: index === 0,
        hidden, moves: !hidden, canUp: previous?.group === group, canDown: next?.group === group,
        pickerTip: model.isCustom ? 'Custom models are always shown in the picker' : hidden ? 'Hidden from picker' : 'Shown in picker' };
    }),
    modelSummary: `${models.length} model${models.length === 1 ? '' : 's'}${favoriteCount > 0 ? ` · ${favoriteCount} favorite${favoriteCount === 1 ? '' : 's'}` : ''}${hiddenCount > 0 ? ` · ${hiddenCount} hidden` : ''}`,
    bulkLabel: builtIn.length ? (builtIn.every(entry => hiddenSet.has(entry.model.slug)) ? 'Enable all' : 'Disable all') : '',
    modelPlaceholder: meta?.modelPlaceholder || 'model-slug', canAddModel: row.driver !== 'antigravity',
  };
}

/**
 * ProviderInstanceCard's version action: "Install <recommended>" when the
 * compatibility advisory names a version the server can install, else
 * "Update now" when the server reports an update candidate.
 */
export function providerUpdateAction(provider: Obj | undefined, advisory: { title: string } | null): { advisoryAction: string; advisoryTarget: string } {
  const none = { advisoryAction: '', advisoryTarget: '' };
  if (!provider || !advisory) return none;
  const compatibility = obj(provider.compatibilityAdvisory), version = obj(provider.versionAdvisory), target = str(compatibility.recommendedVersion);
  if (target) return str(compatibility.message) && version.canInstallVersion === true ? { advisoryAction: `Install ${versionLabel(target) || target}`, advisoryTarget: target } : none;
  return version.canUpdate === true && str(version.status) === 'behind_latest' ? { advisoryAction: 'Update now', advisoryTarget: '' } : none;
}

const PRESET_HEALTH: Record<string, number> = { performance: 60, balanced: 300, 'battery-saver': 900 };
export function healthInterval(settings: Obj): { seconds: number; preset: number; base: string } {
  const background = obj(settings.backgroundActivity), profile = str(background.profile, 'balanced');
  const base = profile === 'custom' ? str(background.baseProfile, 'balanced') : profile;
  const preset = PRESET_HEALTH[base] ?? 300, override = obj(background.overrides).providerHealthRefreshInterval;
  const millis = profile === 'custom' && typeof override === 'number' ? override : preset * 1000;
  return { seconds: Math.round(millis / 1000), preset, base };
}

/** The Providers route's whole projection. */
export function providerPage(host: ProviderHost, selectedId: string, nowMs: number) {
  if (!host.ready) return { available: false, writable: false, message: 'Connect an environment to set up its providers.', checked: '', selectedId: '', rows: [], editors: [], emptyEditor: '', healthSeconds: '300', healthDown: '270', healthUp: '330', healthCustom: false, cursorUsage: false, macos: false, hubs: [] };
  const settings = settingsOf(host), live = liveProviders(host), rows = providerRows(settings, live, selectedId);
  const selected = rows.find(row => row.id === selectedId) ?? rows[0];
  const latest = live.reduce((value, provider) => str(provider.checkedAt) > value ? str(provider.checkedAt) : value, '');
  const health = healthInterval(settings);
  return {
    available: true, writable: host.writable, message: host.writable ? '' : "This session can view this environment's providers but can't change their settings.",
    checked: live.length ? checkedLabel(latest, nowMs) : 'Not checked yet', selectedId: selected?.id || '',
    rows: rows.map((row, index) => {
      const provider = live.find(candidate => candidate.instanceId === row.id), status = rowStatus(row, provider);
      const name = str(row.instance.displayName).trim() || driverMeta(row.driver)?.label || row.driver;
      const advisory = versionAdvisory(provider, status.enabled);
      return { id: row.id, first: index === 0, name, driver: row.driver, enabled: status.enabled, selected: row.id === selected?.id, version: versionLabel(provider?.version),
        advisory: advisory ? (advisory.warning ? 'warning' : 'update') : '', advisoryTitle: advisory?.title || '',
        status: `${status.summary.headline}${status.needsAttention && status.inlineDetail ? ` · ${status.inlineDetail}` : ''}`,
        dot: status.needsAttention ? status.statusKey : '', accent: str(row.instance.accentColor), badge: str(row.instance.accentColor) ? initials(name) : '' };
    }),
    editors: selected ? [editorFor(host, selected, live)] : [], emptyEditor: selected ? '' : 'No providers configured.',
    healthSeconds: String(health.seconds), healthDown: String(Math.max(0, health.seconds - 30)), healthUp: String(health.seconds + 30), healthCustom: health.seconds !== health.preset, cursorUsage: settings.cursorKeychainUsageEnabled === true,
    macos: str(obj(obj(host.config.environment).platform).os) === 'darwin' || !str(obj(obj(host.config.environment).platform).os),
    hubs: Object.entries(obj(settings.usageLimitSources)).map(([id, entry], index) => {
      const source = obj(entry), label = str(source.label).trim() || str(source.url);
      return { id, first: index === 0, label, description: `CLI Proxy${source.enabled === false ? ' · Disabled' : ''}${label !== str(source.url) ? ` · ${str(source.url)}` : ''}` };
    }),
  };
}

function existingIds(settings: Obj): Set<string> {
  const ids = new Set(['codex', 'claudeAgent', ...Object.keys(obj(settings.providerInstances))]);
  for (const [kind, config] of Object.entries(obj(settings.providers))) if (!sameValue(config, driverMeta(kind)?.legacyDefault)) ids.add(kind);
  return ids;
}
// The ACP Registry agent the wizard prepared, for this wizard opening only.
let acpPrepared: { serial: number; agent: AcpAgent } | null = null;
let acpLastAgents: AcpAgent[] = [];
let wizardSerial = -1;
function wizardIdentity(settings: Obj, driverId: string, labelSet: boolean, label: string, idSet: boolean, id: string) {
  const meta = driverMeta(driverId) ?? DRIVERS[0]!, existing = existingIds(settings), isAcp = meta.id === 'acpRegistry';
  const prepared = isAcp && acpPrepared?.serial === wizardSerial ? acpPrepared.agent : null;
  const shownLabel = labelSet ? label : (isAcp ? str(prepared?.name) : meta.label);
  const instanceId = idSet ? id : deriveAvailableInstanceId(candidate => !candidate.trim() || candidate === meta.label
    ? (isAcp ? `${meta.id}_custom` : meta.id) : (slugifyLabel(candidate) ? `${meta.id}_${slugifyLabel(candidate)}` : ''), shownLabel, existing);
  return { meta, existing, shownLabel, instanceId, error: validateInstanceId(instanceId, existing) };
}

/** The Add provider wizard's derived identity and the selected driver's config fields. */
export function providerWizard(host: ProviderHost, open: boolean, serial: number, driverId: string, labelSet: boolean, label: string, idSet: boolean, id: string) {
  if (serial !== wizardSerial) { wizardSerial = serial; acpPrepared = null; }
  const identity = wizardIdentity(settingsOf(host), driverId, labelSet, label, idSet, id);
  const prepared = identity.meta.id === 'acpRegistry' && acpPrepared?.serial === serial ? acpPrepared.agent : null;
  return {
    sessions: open ? [{ key: `wizard:${serial}` }] : [], prepared: prepared ? [prepared] : [],
    drivers: DRIVERS.filter(driver => driver.id !== 'acpRegistry').map(driver => ({ id: driver.id, label: driver.label, selected: driver.id === identity.meta.id })),
    driver: identity.meta.id, driverLabel: identity.meta.label, label: identity.shownLabel, instanceId: identity.instanceId, idError: identity.error,
    preview: identity.shownLabel.trim() || `${identity.meta.label} Workspace`, idPlaceholder: `${identity.meta.id}_work`,
    fields: identity.meta.fields.map((field, index) => ({ ...fieldRow('wizard', field, ''), index })),
    environment: str(obj(host.config.environment).label) || 'this environment',
  };
}

/** ACP Registry search (read scope); an empty query is the compact compatible catalog. */
export async function acpRegistry(host: ProviderHost, native: Native | null | undefined, query: string, open: boolean, configured: string[]) {
  const empty = { ready: false, status: '', error: '', agents: [] as AcpAgent[] };
  if (!open || !native?.available || !host.ready) return empty;
  try {
    const result = await host.rpc(native, 'server.searchAcpRegistry', { query: query.trim().slice(0, 120) });
    const agents: AcpAgent[] = arr(result.agents).map(agent => ({ id: str(agent.id), name: str(agent.name), description: str(agent.description),
      link: str(agent.website) || str(agent.repository), icon: str(agent.icon), version: str(agent.version), distribution: str(agent.distribution),
      added: configured.includes(str(agent.id)) }));
    acpLastAgents = agents;
    return { ready: true, error: '', agents, status: `${agents.length} compatible ${agents.length === 1 ? 'agent' : 'agents'} found.` };
  } catch (error) { return { ...empty, ready: true, error: error instanceof Error ? error.message : 'The ACP Registry search failed.' }; }
}

async function freshSettings(host: ProviderHost, native: Native) { return host.rpc(native, 'server.getSettings', {}); }
async function refreshConfig(host: ProviderHost, native: Native) { host.config = await host.rpc(native, 'server.getConfig', {}); }
async function upsert(host: ProviderHost, native: Native, row: Row, next: Obj, settings: Obj, extra: Obj = {}) {
  const legacyDefault = row.isDefault ? driverMeta(row.driver)?.legacyDefault : undefined;
  const patch = { ...(legacyDefault ? { providers: { ...obj(settings.providers), [row.driver]: legacyDefault } } : {}), ...extra };
  await host.rpc(native, 'server.updateSettings', { patch, providerInstanceMutation: { operation: 'upsert', instanceId: row.id, instance: next } }, true);
}
const withKey = (base: Obj, key: string, value: unknown) => { const next: Obj = { ...base }; if (value === undefined) delete next[key]; else next[key] = value as Obj[string]; return next; };

// Server failures the reference reports as toasts (ProviderSettingsPanel,
// AddProviderInstanceDialog); validation stays with the form.
const FAILURE_TITLES: Record<string, string> = { 'provider-remove': 'Could not delete provider instance', 'provider-reset': 'Could not reset provider instance',
  'provider-add': 'Could not add provider instance', 'provider-create': 'Could not add provider instance' };
const TOASTED = ['provider-name', 'provider-display', 'provider-enabled', 'provider-accent', 'provider-field', 'provider-env-field', 'provider-env-remove', 'provider-env-name',
  'provider-env-value', 'provider-env-sensitive', 'provider-model-add', 'provider-model-remove', 'provider-model-rename', ...Object.keys(FAILURE_TITLES)];
const toastOf = (host: ProviderHost) => host as unknown as T3Client;

/** One provider-settings write. `value` is the JSON the Contract action built through app.ts. */
export async function runProviderOp(host: ProviderHost, native: Native, op: string, id: string, value: string): Promise<string> {
  try { return await providerOp(host, native, op, id, value); }
  catch (error) {
    if (!TOASTED.includes(op) || !(error instanceof ClientError) || ['client', 'stale', 'superseded'].includes(error.kind)) throw error;
    pushToast(toastOf(host), { kind: 'error', title: FAILURE_TITLES[op] ?? 'Could not update provider instance', description: error.message || 'The settings update failed.' });
    return '';
  }
}

async function providerOp(host: ProviderHost, native: Native, op: string, id: string, value: string): Promise<string> {
  const input = value.startsWith('{') ? obj(JSON.parse(value)) : { value };
  if (op === 'provider-refresh') { await host.rpc(native, 'server.refreshProviders', { refreshModels: true }); await refreshConfig(host, native); return ''; }
  if (op === 'provider-health' || op === 'provider-cursor-usage') {
    const settings = await freshSettings(host, native);
    if (op === 'provider-cursor-usage') { await host.rpc(native, 'server.updateSettings', { patch: { cursorKeychainUsageEnabled: input.value === 'true' } }, true); await host.rpc(native, 'server.refreshProviders', {}); }
    else {
      const health = healthInterval(settings), seconds = input.value === 'reset' ? health.preset : Math.max(0, Math.round(Number(input.value)));
      if (!Number.isFinite(seconds)) throw new ClientError('Enter a number of seconds.');
      const overrides = { ...obj(obj(settings.backgroundActivity).overrides) };
      if (seconds === health.preset) delete overrides.providerHealthRefreshInterval; else overrides.providerHealthRefreshInterval = seconds * 1000;
      const background = Object.keys(overrides).length ? { schemaVersion: 1, profile: 'custom', baseProfile: health.base, overrides } : { schemaVersion: 1, profile: health.base, overrides: {} };
      await host.rpc(native, 'server.updateSettings', { patch: { backgroundActivity: background } }, true);
    }
    await refreshConfig(host, native); return '';
  }
  if (op === 'provider-add' || op === 'provider-create') return createInstance(host, native, op, id, input);
  if (op === 'provider-update' || op === 'provider-copy-command') {
    const provider = liveProviders(host).find(candidate => candidate.instanceId === id);
    if (!provider) throw new ClientError('That provider instance is no longer available.');
    const row = providerRows(settingsOf(host), liveProviders(host), id).find(candidate => candidate.id === id);
    const name = str(row?.instance.displayName).trim() || driverMeta(str(provider.driver))?.label || str(provider.driver);
    if (op === 'provider-copy-command') {
      const command = versionAdvisory(provider, true)?.command || '';
      try {
        if (!command) throw new ClientError('This provider has no update command.');
        const copied = await native.later({ op: 'copyText', text: command }) as Obj;
        if (copied?.ok !== true) throw new ClientError('Could not copy the command.');
        pushToast(toastOf(host), { kind: 'success', title: `${name} update command copied`, description: 'Run it in a terminal when you are ready to update.' });
      } catch (error) { pushToast(toastOf(host), { kind: 'error', title: `Could not copy ${name} update command`, description: error instanceof Error ? error.message : '', stacked: true }); }
      return '';
    }
    const label = driverMeta(str(provider.driver))?.label || str(provider.driver);
    try {
      await host.rpc(native, 'server.updateProvider', { provider: str(provider.driver), instanceId: id, ...(input.value ? { targetVersion: str(input.value) } : {}) }, true);
      await host.rpc(native, 'server.refreshProviders', {});
      await refreshConfig(host, native);
    } catch (error) { pushToast(toastOf(host), { kind: 'error', title: `Could not update ${label}`, description: error instanceof Error ? error.message : 'The provider update command could not be started.', stacked: true }); }
    return '';
  }
  if (MODEL_PREF_OPS.includes(op)) {
    // Device-only: the Models section's switches, arrows and bulk toggle.
    const provider = liveProviders(host).find(candidate => candidate.instanceId === id);
    const row = providerRows(settingsOf(host), liveProviders(host), id).find(candidate => candidate.id === id);
    const custom = row ? arr(obj(row.instance.config).customModels).map(entry => typeof entry === 'string' ? entry : str(obj(entry).slug)) : [];
    const models = [...arr(provider?.models).filter(model => model.isCustom !== true).map(model => ({ slug: str(model.slug), isCustom: false })),
      ...custom.map(slug => ({ slug, isCustom: true }))];
    runModelPrefOp(host.local, op, id, str(input.key), str(input.value), models);
    return '';
  }
  if (op === 'provider-chatgpt') {
    // AddCodexAccountDialog: one managed Codex instance per ChatGPT account.
    // Its runtime install and browser sign-in are the server's managed setup.
    const name = str(input.key).trim();
    if (!name) throw new ClientError('Enter an account name.');
    const settings = await freshSettings(host, native);
    const instanceId = deriveAvailableInstanceId(() => `codex_chatgpt_${slugifyLabel(name) || 'account'}`, name, existingIds(settings));
    await host.rpc(native, 'server.updateSettings', { patch: {}, providerInstanceMutation: { operation: 'create', instanceId,
      instance: { driver: 'codex', displayName: `ChatGPT - ${name}`, enabled: true, config: { enabled: true, setupMode: 'managed' } } } }, true);
    await refreshConfig(host, native); return '';
  }
  if (op === 'provider-hub-add' || op === 'provider-hub-remove') {
    if (op === 'provider-hub-add') {
      const url = str(input.key).trim(), key = str(input.value).trim();
      if (!url || !key) throw new ClientError('Enter the hub URL and management key.');
      let parsed: URL;
      try { parsed = new URL(url); } catch { throw new ClientError('Enter a valid hub URL.'); }
      const slug = parsed.host.toLowerCase().replace(/[^a-z0-9.-]+/g, '-').replace(/^-+|-+$/g, '');
      await host.rpc(native, 'server.updateSettings', { patch: { usageLimitSources: { [`cliproxy-${slug || 'hub'}`]: { kind: 'cliproxy', ...(id.trim() ? { label: id.trim() } : {}), url, managementKey: key, enabled: true } } } }, true);
    } else await host.rpc(native, 'server.updateSettings', { patch: { usageLimitSources: { [str(input.key)]: null } } }, true);
    await refreshConfig(host, native); return '';
  }
  if (op === 'acp-prepare') {
    const agentId = str(input.key || input.agentId).trim();
    if (!agentId) throw new ClientError('Select an ACP or configure one manually.');
    const result = await host.rpc(native, 'server.prepareAcpRegistryAgent', { agentId }, true);
    const agent: AcpAgent = acpLastAgents.find(candidate => candidate.id === agentId) ?? { id: agentId, name: str(input.value) || agentId, description: '', link: '', icon: '', version: '', distribution: '', added: false };
    acpPrepared = { serial: wizardSerial, agent: { ...agent, id: str(result.agentId) || agentId, version: str(result.version) || str(agent.version), distribution: str(result.distribution) || str(agent.distribution) } };
    return '';
  }
  const settings = await freshSettings(host, native);
  const row = providerRows(settings, liveProviders(host), id).find(candidate => candidate.id === id);
  if (!row) throw new ClientError('That provider instance is no longer available.');
  const instance = row.instance, config = obj(instance.config), meta = driverMeta(row.driver);
  if (op === 'provider-remove') {
    if (row.isDefault) throw new ClientError('Built-in provider slots can be reset, not deleted.');
    await host.rpc(native, 'server.updateSettings', { patch: {}, providerInstanceMutation: { operation: 'remove', instanceId: id } }, true);
    envDrafts.delete(id);
    if (row.driver === 'acpRegistry' && str(config.agentId)) {
      try { await host.rpc(native, 'server.uninstallAcpRegistryManagedBinary', { agentId: str(config.agentId) }, true); }
      catch (error) {
        await refreshConfig(host, native);
        pushToast(toastOf(host), { kind: 'warning', title: 'Provider deleted, but managed files remain', description: error instanceof Error ? error.message : 'Managed binary cleanup failed.' });
        return '';
      }
    }
  } else if (op === 'provider-reset') {
    if (!row.isDefault || !meta?.legacyDefault) throw new ClientError('Only built-in provider slots can be reset.');
    await host.rpc(native, 'server.updateSettings', { patch: { providers: { ...obj(settings.providers), [row.driver]: meta.legacyDefault } }, providerInstanceMutation: { operation: 'remove', instanceId: id } }, true);
    envDrafts.delete(id);
  } else {
    let next: Obj = instance, extra: Obj = {};
    if (op === 'provider-name' || op === 'provider-display') {
      const name = str(input.value).trim();
      if (op === 'provider-name' && !name) throw new ClientError('Provider name cannot be empty.');
      if (name === str(instance.displayName)) return '';
      next = withKey(instance, 'displayName', name || undefined);
    } else if (op === 'provider-enabled') {
      if (!['true', 'false'].includes(str(input.value))) throw new ClientError('Choose enabled or disabled.');
      next = { ...instance, enabled: input.value === 'true' };
      const textGeneration = obj(settings.textGenerationModelSelection);
      if (input.value === 'false' && str(textGeneration.instanceId) === id) extra = { textGenerationModelSelection: { instanceId: 'codex', model: 'gpt-6-luna', options: [{ id: 'reasoningEffort', value: 'low' }] } };
    } else if (op === 'provider-accent') {
      const color = str(input.value).trim();
      if (color && !/^#[0-9a-fA-F]{6}$/.test(color)) throw new ClientError('Choose a six-digit hex color.');
      if (color === str(instance.accentColor)) return '';
      next = withKey(instance, 'accentColor', color || undefined);
    } else if (op === 'provider-field') {
      const field = meta?.fields.find(candidate => candidate.key === str(input.key));
      if (!field) throw new ClientError('That setting is not available for this provider.');
      const text = str(input.value).trim();
      if (text === str(config[field.key]).trim()) return '';
      next = { ...instance, config: text || field.persist ? { ...config, [field.key]: text } : withKey(config, field.key, undefined) };
    } else if (op === 'provider-env-field') {
      const field = meta?.env.find(candidate => candidate.key === str(input.key));
      if (!field) throw new ClientError('That variable is not available for this provider.');
      const text = str(input.value).trim(), others = arr(instance.environment).filter(variable => variable.name !== field.key);
      const environment = text ? [...arr(instance.environment).map(variable => variable.name === field.key ? { name: field.key, value: text, sensitive: true } : variable),
        ...(arr(instance.environment).some(variable => variable.name === field.key) ? [] : [{ name: field.key, value: text, sensitive: true }])] : others;
      next = withKey(instance, 'environment', environment.length ? environment : undefined);
    } else if (op.startsWith('provider-env-')) {
      const dedicated = new Set((meta?.env || []).map(field => field.key));
      const rows = envRows(id, instance, dedicated).map(entry => ({ ...entry }));
      const index = /^\d+$/.test(str(input.key)) ? Number(input.key) : -1;
      if (op === 'provider-env-add') { rows.push({ name: '', value: '', sensitive: true }); envDrafts.set(id, rows); return ''; }
      if (index < 0 || index >= rows.length) throw new ClientError('That variable is no longer available.');
      if (op === 'provider-env-remove') rows.splice(index, 1);
      else if (op === 'provider-env-name') rows[index]!.name = str(input.value).trim();
      else if (op === 'provider-env-value') { if (str(input.value) !== str(rows[index]!.value) || rows[index]!.valueRedacted === true) { rows[index]!.value = str(input.value); rows[index]!.valueRedacted = false; } }
      else if (op === 'provider-env-sensitive') { const current = rows[index]!; current.sensitive = input.value === 'true'; if (current.sensitive !== true && current.valueRedacted === true) current.valueRedacted = false; }
      else throw new ClientError(`Unknown action: ${op}`);
      envDrafts.set(id, rows);
      const invalid = rows.some(entry => !ENV_NAME.test(str(entry.name)) && (str(entry.name) || str(entry.value) || entry.sensitive !== true || entry.valueRedacted !== undefined));
      if (invalid) return str(rows[index]?.name) && !ENV_NAME.test(str(rows[index]?.name)) ? 'Use letters, digits and underscores for variable names, starting with a letter or underscore.' : '';
      const keep = arr(instance.environment).filter(variable => dedicated.has(str(variable.name)));
      const published = rows.filter(entry => ENV_NAME.test(str(entry.name))).map(entry => ({ name: str(entry.name), value: str(entry.value), sensitive: entry.sensitive === true, ...(entry.valueRedacted === true ? { valueRedacted: true } : {}) }));
      const environment = [...keep, ...published];
      if (sameValue(environment, arr(instance.environment))) { await refreshConfig(host, native); return ''; }
      next = withKey(instance, 'environment', environment.length ? environment : undefined);
      await upsert(host, native, row, next, settings);
      envDrafts.delete(id); await refreshConfig(host, native); return '';
    } else if (op === 'provider-model-add' || op === 'provider-model-remove' || op === 'provider-model-rename') {
      if (row.driver === 'antigravity') throw new ClientError('Antigravity models come from the provider.');
      const slug = str(input.key || input.slug).trim(), custom = arr(config.customModels).map(entry => typeof entry === 'string' ? { slug: entry, name: entry } : obj(entry));
      if (op === 'provider-model-add') {
        if (!slug) throw new ClientError('Enter a model slug.');
        if (arr(liveProviders(host).find(provider => provider.instanceId === id)?.models).some(model => model.isCustom !== true && model.slug === slug)) throw new ClientError('That model is already built in.');
        if (slug.length > 256) throw new ClientError('Model slugs must be 256 characters or less.');
        if (custom.some(entry => entry.slug === slug)) throw new ClientError('That custom model is already saved.');
        custom.push({ slug, name: slug, capabilities: null });
      } else if (op === 'provider-model-remove') {
        if (!custom.some(entry => entry.slug === slug)) throw new ClientError('That custom model is no longer saved.');
        custom.splice(custom.findIndex(entry => entry.slug === slug), 1);
      } else {
        const entry = custom.find(candidate => candidate.slug === slug);
        if (!entry) throw new ClientError('That custom model is no longer saved.');
        entry.name = str(input.value ?? input.name).trim() || slug;
      }
      const stored = row.driver === 'acpRegistry' ? custom.map(entry => str(entry.slug)) : custom.map(entry => ({ slug: str(entry.slug), name: str(entry.name), ...(entry.capabilities ? { capabilities: entry.capabilities } : {}) }));
      next = { ...instance, config: { ...config, customModels: stored } };
    } else throw new ClientError(`Unknown action: ${op}`);
    await upsert(host, native, row, next, settings, extra);
  }
  await refreshConfig(host, native);
  return '';
}

async function createInstance(host: ProviderHost, native: Native, op: string, id: string, input: Obj): Promise<string> {
  const settings = await freshSettings(host, native);
  const driver = str(input.driver), meta = driverMeta(driver);
  if (!meta) throw new ClientError('Choose an available driver advertised by this environment.');
  const identity = op === 'provider-add' ? wizardIdentity(settings, driver, true, str(input.label), id !== '', id) : null;
  const instanceId = identity ? identity.instanceId : id;
  const error = identity ? identity.error : (!/^[a-zA-Z][a-zA-Z0-9_-]{0,63}$/.test(id) ? 'Use an instance ID starting with a letter, followed by letters, digits, dash or underscore (maximum 64 characters).'
    : (id in obj(settings.providerInstances) || liveProviders(host).some(provider => provider.instanceId === id)) ? 'That provider instance ID already exists.' : '');
  if (error) throw new ClientError(error);
  const label = str(op === 'provider-add' ? input.label : input.name).trim();
  if (op === 'provider-create' && !label) throw new ClientError('Provider name cannot be empty.');
  const values = op === 'provider-add' ? obj(input.fields) : { binaryPath: input.binaryPath, homePath: input.homePath };
  const config: Obj = {};
  for (const field of meta.fields) { const text = str(values[field.key]).trim(); if (text) config[field.key] = text; }
  const prepared = driver === 'acpRegistry' && acpPrepared?.serial === wizardSerial ? acpPrepared.agent : null;
  if (prepared && !str(config.agentId)) { config.agentId = str(prepared.id); if (/^https:\/\/cdn\.agentclientprotocol\.com\//.test(str(prepared.icon))) config.registryIconUrl = str(prepared.icon); }
  if (driver === 'acpRegistry' && !str(config.agentId)) throw new ClientError('Select an ACP or configure one manually.');
  if (driver === 'acpRegistry') config.distribution = 'auto';
  if (driver === 'codex') config.setupMode = 'existing';
  const accent = str(input.accentColor).trim();
  if (accent && !/^#[0-9a-fA-F]{6}$/.test(accent)) throw new ClientError('Choose a six-digit hex color.');
  const instance = { driver, enabled: true, ...(label ? { displayName: label } : {}), ...(accent ? { accentColor: accent } : {}), ...(Object.keys(config).length ? { config } : {}) };
  await host.rpc(native, 'server.updateSettings', { patch: {}, providerInstanceMutation: { operation: 'create', instanceId, instance } }, true);
  await refreshConfig(host, native);
  // An ACP Registry instance continues to its sign-in step instead (AddProviderInstanceDialog).
  if (driver !== 'acpRegistry') pushToast(toastOf(host), { kind: 'success', title: 'Provider instance added', description: `${meta.label} instance '${instanceId}' was added.` });
  return '';
}

export const PROVIDER_OPS = ['provider-create', 'provider-add', 'provider-name', 'provider-display', 'provider-enabled', 'provider-remove', 'provider-reset',
  'provider-accent', 'provider-field', 'provider-env-field', 'provider-env-add', 'provider-env-name', 'provider-env-value', 'provider-env-sensitive', 'provider-env-remove', 'provider-model-add',
  'provider-model-remove', 'provider-model-rename', 'provider-hub-add', 'provider-hub-remove', 'provider-chatgpt', 'provider-refresh', 'provider-health', 'provider-cursor-usage', 'acp-prepare', 'provider-update', 'provider-copy-command', ...MODEL_PREF_OPS];
export type { Driver };

/** The wizard's positional config drafts (f0…f4) as the driver's named fields. */
export function providerFieldValues(driver: string, values: string[]): Obj {
  const fields = driverMeta(driver)?.fields || [];
  return Object.fromEntries(fields.map((field, index) => [field.key, values[index] ?? '']).filter(([, value]) => value !== ''));
}
