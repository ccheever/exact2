// Lane settings-b: per-instance model visibility and order (ProviderModelsSection
// groupModelsForDisplay / nextHiddenModelsForBulkToggle, modelOrdering.ts
// sortModelsForProviderInstance, modelSelection.ts applyInstanceModelPreferences).
// "Favorites, visibility, and ordering are saved on this device": the
// preferences live beside favorites in T3Client.local and the picker honors them.
import { obj, str, type Obj } from './domain';
import { ClientError } from './protocol';

export type ModelPrefs = Record<string, { hiddenModels: string[]; modelOrder: string[] }>;
type Local = { favoriteModels: string[] };
const strings = (value: unknown) => Array.isArray(value) ? [...new Set(value.filter((entry): entry is string => typeof entry === 'string' && entry.trim().length > 0))].slice(0, 500) : [];

export function decodeModelPrefs(value: unknown): ModelPrefs {
  const prefs: ModelPrefs = {};
  for (const [instanceId, entry] of Object.entries(obj(value)).slice(0, 200)) {
    const hiddenModels = strings(obj(entry).hiddenModels), modelOrder = strings(obj(entry).modelOrder);
    if (hiddenModels.length || modelOrder.length) prefs[instanceId] = { hiddenModels, modelOrder };
  }
  return prefs;
}
/** The device preferences on T3Client.local (persisted with the rest of it). */
export function modelPrefs(local: object): ModelPrefs {
  return (local as { providerModelPreferences?: ModelPrefs }).providerModelPreferences ?? {};
}
/** T3Client.load: carry the saved preferences into the fresh local object. */
export function adoptModelPrefs(next: object, saved: Obj): void {
  (next as { providerModelPreferences?: ModelPrefs }).providerModelPreferences = decodeModelPrefs(saved.providerModelPreferences);
}
export function instancePrefs(local: object, instanceId: string) {
  return modelPrefs(local)[instanceId] ?? { hiddenModels: [], modelOrder: [] };
}
function setInstancePrefs(local: object, instanceId: string, hiddenModels: string[], modelOrder: string[]) {
  const next = { ...modelPrefs(local) }, hidden = strings(hiddenModels), order = strings(modelOrder);
  if (hidden.length || order.length) next[instanceId] = { hiddenModels: hidden, modelOrder: order }; else delete next[instanceId];
  (local as { providerModelPreferences?: ModelPrefs }).providerModelPreferences = next;
}
export function favoriteSlugs(local: Local, instanceId: string): Set<string> {
  return new Set(local.favoriteModels.map(key => { try { const [instance, slug] = JSON.parse(key) as unknown[]; return instance === instanceId ? str(slug) : ''; } catch { return ''; } }).filter(Boolean));
}

/** sortModelsForProviderInstance: favorites first (when grouped), then saved order, then server order. */
export function sortModels<T extends { slug: string }>(models: T[], order: string[], favorites?: Set<string>): T[] {
  const rank = new Map(order.map((slug, index) => [slug, index])), original = new Map(models.map((model, index) => [model.slug, index]));
  const at = (map: Map<string, number>, slug: string) => map.get(slug) ?? Number.POSITIVE_INFINITY;
  return models.slice().sort((a, b) => (favorites ? Number(favorites.has(b.slug)) - Number(favorites.has(a.slug)) : 0)
    || (at(rank, a.slug) - at(rank, b.slug) || 0) || (at(original, a.slug) - at(original, b.slug) || 0));
}
export type ModelGroup = 'favorite' | 'visible' | 'hidden';
/** groupModelsForDisplay: favorites, then visible, then hidden (hidden sink). */
export function groupModels<T extends { slug: string; isCustom: boolean }>(models: T[], favorites: Set<string>, hidden: Set<string>, order: string[]): { model: T; group: ModelGroup }[] {
  const sorted = sortModels(models, order, favorites);
  const isHidden = (model: T) => !model.isCustom && hidden.has(model.slug);
  return [...sorted.filter(model => favorites.has(model.slug)).map(model => ({ model, group: 'favorite' as ModelGroup })),
    ...sorted.filter(model => !favorites.has(model.slug) && !isHidden(model)).map(model => ({ model, group: 'visible' as ModelGroup })),
    ...sorted.filter(model => !favorites.has(model.slug) && isHidden(model)).map(model => ({ model, group: 'hidden' as ModelGroup }))];
}
export function bulkHidden(models: { slug: string; isCustom: boolean }[], hidden: string[]): string[] {
  const builtIn = models.filter(model => !model.isCustom).map(model => model.slug), set = new Set(builtIn);
  return builtIn.every(slug => hidden.includes(slug)) ? hidden.filter(slug => !set.has(slug)) : [...new Set([...hidden, ...builtIn])];
}

/** The picker's view of one instance's models: hidden built-ins dropped, saved order applied. */
export function applyPickerPrefs(local: object, instanceId: string, models: Obj[]): Obj[] {
  const prefs = instancePrefs(local, instanceId), hidden = new Set(prefs.hiddenModels);
  const kept = models.filter(model => model.isCustom === true || !hidden.has(str(model.slug))).map(model => ({ model, slug: str(model.slug) }));
  return sortModels(kept, prefs.modelOrder).map(entry => entry.model);
}

export const MODEL_PREF_OPS = ['provider-model-hide', 'provider-model-move', 'provider-model-bulk'];
/**
 * One device-only model preference change. `models` is the instance's full
 * list (built-in and custom, `isCustom` set) as the Models section shows it.
 */
export function runModelPrefOp(local: Local, op: string, instanceId: string, slug: string, value: string, models: { slug: string; isCustom: boolean }[]): void {
  const prefs = instancePrefs(local, instanceId), favorites = favoriteSlugs(local, instanceId);
  if (op === 'provider-model-bulk') { setInstancePrefs(local, instanceId, bulkHidden(models, prefs.hiddenModels), prefs.modelOrder); return; }
  const model = models.find(entry => entry.slug === slug);
  if (!model) throw new ClientError('That model is no longer advertised by T3.');
  if (op === 'provider-model-hide') {
    if (model.isCustom) throw new ClientError('Custom models are always shown in the picker.');
    const hide = value === 'true';
    if (hide === prefs.hiddenModels.includes(slug)) return;
    setInstancePrefs(local, instanceId, hide ? [...prefs.hiddenModels, slug] : prefs.hiddenModels.filter(entry => entry !== slug), prefs.modelOrder);
    return;
  }
  if (op === 'provider-model-move') {
    const display = groupModels(models, favorites, new Set(prefs.hiddenModels), prefs.modelOrder);
    const index = display.findIndex(entry => entry.model.slug === slug), next = index + (value === 'up' ? -1 : 1);
    if (index < 0 || next < 0 || next >= display.length || display[index]!.group !== display[next]!.group) return;
    const order = display.map(entry => entry.model.slug);
    [order[index], order[next]] = [order[next]!, order[index]!];
    setInstancePrefs(local, instanceId, prefs.hiddenModels, order);
    return;
  }
  throw new ClientError(`Unknown action: ${op}`);
}
