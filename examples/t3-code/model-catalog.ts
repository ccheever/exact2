// T3's model picker catalog: ModelPickerContent.tsx (rail, filtering, legacy
// section), modelOrdering.ts, modelPickerSearch.ts and shared searchRanking.ts.
// Browsing and searching never change the composer's selection; only a chosen
// row commits one (`command("model", …)`).
import { arr, str, type Obj } from './domain';
import { providerLock, matchesLock } from './composer-controls-commands';
import { applyPickerPrefs } from './settings-b-models'; // settings-b: hidden models and saved order
import { fanoutSelections, fanoutSupported } from './r3-composer-controls-fanout';
import type { T3Client } from './client';
import { pickerReady, pickerOptions, pickerSetupEntries, shouldOfferModelPickerSetup } from './provider-picker-setup'; // provider-sign-in-and-install
import { usesChatGptSharing } from './chatgpt-plan';
import { composerPair } from './composer-provider-selection'; // composer-provider-state-and-details: the checked row

type Badge = (provider: Obj | undefined, providers: Obj[]) => { providerBadge: string; providerBadgeColor: string };
type Item = { id: string; name: string; shortName: string; subProvider: string; providerId: string; providerName: string;
  driver: string; favorite: boolean; legacy: boolean; isNew: boolean; order: number };

export { pickerReady };

const normalize = (value: string) => value.trim().toLowerCase();
function subsequence(value: string, query: string): number | null {
  let at = 0, first = -1, previous = -1, gaps = 0;
  for (let index = 0; index < value.length; index += 1) {
    if (value[index] !== query[at]) continue;
    if (first === -1) first = index;
    if (previous !== -1) gaps += index - previous - 1;
    previous = index; at += 1;
    if (at === query.length) return first * 2 + gaps * 3 + (index - first + 1 - query.length) + Math.min(64, value.length - query.length);
  }
  return null;
}
function scoreField(value: string, token: string, base: number): number | null {
  if (!value || !token) return null;
  const penalty = Math.min(64, Math.max(0, value.length - token.length));
  if (value === token) return base;
  if (value.startsWith(token)) return base + 2 + penalty;
  const boundary = [' ', '-', '_', '/'].map(marker => { const at = value.indexOf(marker + token); return at < 0 ? -1 : at + marker.length; })
    .filter(at => at >= 0);
  if (boundary.length) return base + 4 + Math.min(...boundary) * 2 + penalty;
  const includes = value.indexOf(token);
  if (includes >= 0) return base + 6 + includes * 2 + penalty;
  const fuzzy = token.length >= 3 ? subsequence(value, token) : null;
  return fuzzy === null ? null : base + 100 + fuzzy;
}
const searchText = (item: Item) => normalize([item.name, item.shortName, item.subProvider, item.driver, item.providerName].filter(Boolean).join(' '));
/** scoreModelPickerSearch: every token must match a field; favorites get a 24-point boost. */
export function scoreModel(item: Item, query: string): number | null {
  const tokens = normalize(query).split(/\s+/u).filter(Boolean);
  if (!tokens.length) return 0;
  const fields = [item.name, ...(item.shortName ? [item.shortName] : []), ...(item.subProvider ? [item.subProvider] : []),
    item.driver, item.providerName].map(normalize).concat(searchText(item));
  let score = 0;
  for (const token of tokens) {
    const scores = fields.map((field, index) => scoreField(field, token, index * 10)).filter((value): value is number => value !== null);
    if (!scores.length) return null;
    score += Math.min(...scores);
  }
  return item.favorite ? score - 24 : score;
}
function display(item: Item): string {
  const name = item.shortName || item.name, qualifier = item.subProvider.trim();
  if (!qualifier) return name;
  const pattern = new RegExp(`^${qualifier.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?:\\s*[.:/-]\\s*|\\s+)`, 'iu');
  return name.replace(pattern, '').trim() || name;
}

export function favoriteKeys(favorites: string[]): Set<string> {
  const keys = new Set<string>();
  for (const entry of favorites) {
    try { const [provider, model] = JSON.parse(entry) as unknown[]; keys.add(`${String(provider)}:${String(model)}`); } catch { /* ignored */ }
  }
  return keys;
}

/**
 * A settings selection target (settings-model-picker.ts: ProviderModelPicker with
 * lockedProvider null and getModelDisabledReason): no thread lock or fan-out, and a
 * model some selected target cannot honor stays listed, disabled, with its reason.
 * `setup: false` is a picker without onOpenProviderSetup (Scheduled Tasks' Model): an
 * instance that needs setup is neither selectable nor offered in the footer.
 */
export type PickerTarget = { reason: (instanceId: string, model: string) => string; setup?: boolean };

/**
 * ModelPickerSidebar describeUnavailableInstance and its context-disabled tooltip: a rail
 * button's tooltip and, for one that is not ready or locked out, its accessible name.
 */
export function railLabel(provider: Obj, locked: boolean): string {
  const name = str(provider.displayName, str(provider.driver));
  if (!pickerReady(provider)) {
    if (provider.enabled !== true || provider.status === 'disabled') return `${name} — Disabled in settings.`;
    const kind = provider.status === 'error' ? 'Unavailable' : provider.status === 'warning' ? 'Limited' : 'Not ready';
    const message = str(provider.message).trim();
    return message ? `${name} — ${kind}. ${message}` : `${name} — ${kind}.`;
  }
  return locked ? `${name} is unavailable in this thread. Start a new thread to switch providers.` : name;
}

/**
 * adjacentModelPickerProvider: ⇧⌘↓/⇧⌘↑ step through Favorites and the rail's selectable
 * instances only (picker-ready, or not ready but reachable for setup; never a locked-out one),
 * wrapping; from a choice that is not in that list, down goes to Favorites and up to the last.
 */
export function adjacentPickerProvider(catalog: { provider: string; providers: { id: string; selectable: boolean }[] }, direction: 1 | -1): string {
  const rail = ['favorites', ...catalog.providers.filter(entry => entry.selectable).map(entry => entry.id)];
  const at = rail.indexOf(catalog.provider);
  return rail[at < 0 ? (direction === 1 ? 0 : rail.length - 1) : (at + direction + rail.length) % rail.length]!;
}

/**
 * modelJumpCommandByKey: the first nine rows a person can choose (no disabled reason) take
 * modelPicker.jump.1–9, in search results and Favorites too; `label` names each command's
 * shortcut ("⌘1", shortcutLabelForCommand) and `chord` its aria-keyshortcuts ("Meta+1").
 */
export function withJumpLabels<C extends { models: { kind: string; reason: string; jump: string; jumpKey: string }[] }>(catalog: C, shortcut: (command: string) => { label: string; chord: string }): C {
  let ordinal = 0;
  const models = catalog.models.map(row => {
    if (row.kind !== 'model' || row.reason || ordinal >= 9) return row;
    ordinal += 1;
    const { label, chord } = shortcut(`modelPicker.jump.${ordinal}`);
    return { ...row, jump: label, jumpKey: chord };
  });
  return { ...catalog, models };
}

/**
 * `requested` is the rail choice ("" until the reader picks one: favorites
 * when any exist, else the active instance). Legacy models follow their
 * collapsible header row (`legacy`), as `legacy-model` rows the view shows
 * only while expanded (from the start when the active model is one of them:
 * `legacyDefault`); keyboard highlight is the view's.
 */
export function pickerCatalog(client: { config: Obj; local: { favoriteModels: string[] }; providerId: string; modelId: string; threadId?: string; projection?: Obj },
  requested: string, query: string, badge: Badge, target?: PickerTarget) {
  const providers = arr(client.config.providers);
  // The composer's picker checks the instance and model the composer shows (activeInstanceId and model are
  // ChatComposer's resolved selection, composer-provider-selection.ts); a settings target keeps its own value.
  const current = target ? { providerId: client.providerId, modelId: client.modelId } : composerPair(client);
  // A started thread offers only its driver's (and account group's) models (ModelPickerContent lockedProvider).
  const lock = target ? null : providerLock(client);
  const keys = favoriteKeys(client.local.favoriteModels);
  const instanceOrder = new Map(providers.map((provider, index) => [str(provider.instanceId), index]));
  const items: Item[] = providers.filter(provider => pickerReady(provider) && matchesLock(provider, lock)).flatMap(provider => applyPickerPrefs(client.local, str(provider.instanceId), arr(provider.models)).map((model, order) => {
    const id = str(model.slug), providerId = str(provider.instanceId);
    return { id, name: str(model.name, id), shortName: str(model.shortName), subProvider: str(model.subProvider), providerId,
      providerName: str(provider.displayName, str(provider.driver)), driver: str(provider.driver), favorite: keys.has(`${providerId}:${id}`),
      legacy: model.isLegacy === true, isNew: model.badge === 'new', order };
  }));
  // An active instance that needs setup opens selected, so its footer offers the setup (ModelPickerContent:245-264).
  const active = providers.find(provider => provider.instanceId === current.providerId);
  const activeNeedsSetup = target?.setup !== false && !!active && shouldOfferModelPickerSetup(active, pickerOptions(active));
  const selected = requested || (activeNeedsSetup || client.local.favoriteModels.length === 0 ? current.providerId : 'favorites');
  const searching = normalize(query) !== '';
  const original = (item: Item) => (instanceOrder.get(item.providerId) ?? 0) * 10000 + item.order;
  let list: Item[];
  if (searching) {
    list = items.map(item => ({ item, score: scoreModel(item, query), tie: searchText(item) }))
      .filter((entry): entry is { item: Item; score: number; tie: string } => entry.score !== null)
      .sort((a, b) => a.score - b.score || Number(b.item.favorite) - Number(a.item.favorite) || a.tie.localeCompare(b.tie))
      .map(entry => entry.item);
  } else if (selected === 'favorites') {
    list = items.filter(item => item.favorite).sort((a, b) => original(a) - original(b));
  } else {
    list = items.filter(item => item.providerId === selected)
      .sort((a, b) => Number(b.favorite) - Number(a.favorite) || original(a) - original(b));
  }
  const legacy = !searching && selected !== 'favorites' ? list.filter(item => item.legacy) : [];
  // The unsearched view's rows (legacy header included): a search keeps the height it had (a6aebbe2e0).
  const rest = selected === 'favorites' ? items.filter(item => item.favorite) : items.filter(item => item.providerId === selected);
  const restLegacy = selected !== 'favorites' ? rest.filter(item => item.legacy).length : 0;
  const ordered: Array<Item | null> = legacy.length ? [...list.filter(item => !item.legacy), null, ...legacy] : list;
  // Several models chosen for a new thread (r3-composer-controls-fanout.ts): each chosen row is selected and checked.
  const fan = target ? null : fanoutSelections(client as unknown as T3Client);
  const fanKeys = new Set((fan ?? []).map(selection => `${selection.instanceId}:${selection.model}`));
  const row = (item: Item, index: number, kind: string) => ({ key: `${item.providerId}:${item.id}`, kind, id: item.id, providerId: item.providerId,
    name: display(item), label: item.subProvider ? `${item.providerName} · ${item.subProvider}` : item.providerName, driver: item.driver,
    favorite: item.favorite, selected: fan ? fanKeys.has(`${item.providerId}:${item.id}`) : item.providerId === current.providerId && item.id === current.modelId, isNew: item.isNew,
    index, highlighted: false, expanded: false, checked: !!fan && fanKeys.has(`${item.providerId}:${item.id}`), reason: target?.reason(item.providerId, item.id) ?? '', jump: '', jumpKey: '' });
  const rows = ordered.map((item, index) => item === null
    ? { key: `legacy:${selected}`, kind: 'legacy', id: '', providerId: selected, name: 'Legacy models', label: `${legacy.length} models`,
      driver: '', favorite: false, selected: false, isNew: false, index, highlighted: false, expanded: false, checked: false, reason: '', jump: '', jumpKey: '' }
    : row(item, index, legacy.length && item.legacy ? 'legacy-model' : 'model'));
  // The rail's instance badges count only its own (enabled) entries, as ModelPickerSidebar does.
  // Locked-out instances follow the compatible ones, disabled.
  const enabled = providers.filter(provider => provider.enabled === true);
  const railOrder = lock ? [...enabled.filter(provider => matchesLock(provider, lock)), ...enabled.filter(provider => !matchesLock(provider, lock))] : enabled;
  // An instance that needs setup stays selectable though it is not ready (selectableUnavailableInstanceIds),
  // only where the picker offers setup (onOpenProviderSetup).
  const setup = target?.setup !== false;
  const rail = railOrder.map((provider, index) => ({ id: str(provider.instanceId), index: index + 1,
    name: str(provider.displayName, str(provider.driver)), driver: str(provider.driver), ready: pickerReady(provider) && matchesLock(provider, lock),
    selectable: (pickerReady(provider) || (setup && shouldOfferModelPickerSetup(provider, pickerOptions(provider)))) && matchesLock(provider, lock),
    label: railLabel(provider, !matchesLock(provider, lock)), selected: str(provider.instanceId) === selected, ...badge(provider, enabled) }));
  const at = rail.findIndex(entry => entry.selected);
  // expandedLegacyInstances starts with the active instance when its model is a legacy one.
  const legacyDefault = selected === current.providerId && legacy.some(item => item.id === current.modelId);
  return { searching, provider: selected, favoritesSelected: selected === 'favorites', count: rows.length, legacyCount: legacy.length, legacyDefault,
    restCount: rest.length + (restLegacy ? 1 : 0), restLegacyCount: restLegacy,
    highlight: -1, highlightKind: '', highlightId: '', highlightProvider: '', models: rows, providers: rail,
    railIndex: selected === 'favorites' ? 0 : at < 0 ? -1 : at + 1,
    setup: setup ? pickerSetupEntries(enabled.filter(provider => matchesLock(provider, lock)), selected, searching, list.length) : [],
    // Combobox autoHighlight ('input-change'): a search highlights its first row a person can choose.
    firstEnabled: rows.findIndex(entry => entry.kind === 'model' && !entry.reason),
    // ProviderModelPicker onToggleModel (a new thread's draft that can start several models): Shift adds a model and keeps the picker open.
    multiple: !target && fanoutSupported(client as unknown as T3Client),
    // ChatGptSharingControl: the active instance (the composer's) shares a ChatGPT plan (managed-codex-chatgpt).
    chatgptSharing: usesChatGptSharing(providers.find(provider => provider.instanceId === current.providerId)) };
}
