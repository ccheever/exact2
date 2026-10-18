// Environment-owned model preferences, T3 Code 9c161462c9 (MIT; LICENSE-T3):
// shared/serverSettings.ts resolveModelPreferences/modelPreferencesMigrationPatch and
// web/hooks/useSettings.ts desired-state favorite mutations. Legacy device values seed only
// unset server lists; an explicit empty server list always wins.
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import type { ProviderHost } from './providers';
import { decodeModelPrefs, runModelPrefOp } from './settings-b-models';

type Host = Pick<ProviderHost, 'config' | 'local' | 'rpc'>;
export function legacyFavorites(local: object): Obj[] {
  const keys = (local as { favoriteModels?: string[] }).favoriteModels ?? [];
  return keys.flatMap(key => { try {
    const pair: unknown = JSON.parse(key);
    return Array.isArray(pair) && typeof pair[0] === 'string' && typeof pair[1] === 'string' ? [{ provider: pair[0], model: pair[1] }] : [];
  } catch { return []; } });
}
export function modelPreferenceSource(host: { config: Obj; local: object }) {
  const settings = obj(host.config.settings);
  const favorites = Array.isArray(settings.favorites) ? arr(settings.favorites) : legacyFavorites(host.local);
  return { favoriteModels: favorites.map(favorite => JSON.stringify([str(favorite.provider), str(favorite.model)])),
    providerModelPreferences: decodeModelPrefs(settings.providerModelPreferences ?? (host.local as { providerModelPreferences?: unknown }).providerModelPreferences) };
}
export function modelPreferencesMigrationPatch(settings: Obj, local: Host['local']): Obj | null {
  const favorites = legacyFavorites(local), preferences = decodeModelPrefs((local as { providerModelPreferences?: unknown }).providerModelPreferences);
  const migration: Obj = { ...(settings.favorites === null && favorites.length ? { favorites } : {}),
    ...(settings.providerModelPreferences === null && Object.keys(preferences).length ? { providerModelPreferences: preferences as unknown as Obj } : {}) };
  return Object.keys(migration).length ? { migrateModelPreferences: migration } : null;
}
const environment = (host: Host) => str(obj(host.config.environment).environmentId);
export async function saveModelPreferences(host: Host, native: Native, patch: Obj): Promise<void> {
  const id = environment(host), settings = await host.rpc(native, 'server.getSettings', {});
  if (environment(host) !== id) throw new ClientError('The environment changed before model preferences were saved.');
  await host.rpc(native, 'server.updateSettings', { patch: { ...modelPreferencesMigrationPatch(settings, host.local), ...patch } }, true);
  if (environment(host) !== id) return;
  const config = await host.rpc(native, 'server.getConfig', {});
  if (environment(host) === id) host.config = config;
}
const migrations = new WeakSet<object>();
/** The migration is atomic on the server, so two clients cannot overwrite an already claimed list. */
export async function migrateModelPreferences(host: Host, native: Native): Promise<void> {
  const patch = modelPreferencesMigrationPatch(obj(host.config.settings), host.local);
  if (!patch || migrations.has(host)) return;
  migrations.add(host);
  try { await saveModelPreferences(host, native, patch); } finally { migrations.delete(host); }
}
const favoriteChoices = new WeakMap<object, Map<string, { choice: boolean; settings: unknown; pending: boolean }>>();
export async function toggleModelFavorite(host: Host, native: Native, instanceId: string, model: string): Promise<void> {
  let choices = favoriteChoices.get(host); if (!choices) favoriteChoices.set(host, choices = new Map());
  const key = JSON.stringify([instanceId, model]), settings = obj(host.config.settings).favorites, choiceKey = `${environment(host)}\n${key}`, previous = choices.get(choiceKey);
  const current = previous && (previous.pending || previous.settings === settings) ? previous.choice : modelPreferenceSource(host).favoriteModels.includes(key);
  const next = { choice: !current, settings, pending: true }; choices.set(choiceKey, next);
  try {
    await saveModelPreferences(host, native, { setModelFavorites: [{ provider: instanceId, model, favorite: next.choice }] });
    next.pending = false;
  } catch (error) { if (choices.get(choiceKey) === next) { if (previous && !previous.pending) choices.set(choiceKey, previous); else choices.delete(choiceKey); } throw error; }
}
export async function runEnvironmentModelPrefOp(host: Host, native: Native, op: string, instanceId: string, slug: string, value: string,
  models: { slug: string; isCustom: boolean }[]): Promise<void> {
  const source = modelPreferenceSource(host);
  runModelPrefOp(source, op, instanceId, slug, value, models);
  const prefs = source.providerModelPreferences[instanceId] ?? { hiddenModels: [], modelOrder: [] };
  const patch = op === 'provider-model-move' ? { setProviderModelOrder: { provider: instanceId, modelOrder: prefs.modelOrder } }
    : { setModelsHidden: (op === 'provider-model-bulk' ? models.filter(model => !model.isCustom).map(model => model.slug) : [slug])
      .map(model => ({ provider: instanceId, model, hidden: prefs.hiddenModels.includes(model) })) };
  await saveModelPreferences(host, native, patch);
}
