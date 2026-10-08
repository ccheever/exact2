// The provider update launch notification, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/ProviderUpdatePrimaryNotification.tsx:94-312 (one prompt per update
// set, remembered when dismissed; "Update" runs the one-click updates on the primary environment
// one after another; then one toast follows the server's `updateState`) and
// ProviderUpdateLaunchNotification.tsx (no local secondary: the primary flow).
// Changes: the effects run on each shell answer (shellView → providerUpdates) instead of on
// React renders; "Update" is the `upkeep:update-launch` op (providers-upkeep.ts), sent through
// its own queue mutation so the app stays usable while it runs. The primary is the embedded
// server ("This machine"): the focus when it is the primary, else its background connection;
// with no primary known, the focused environment stands in (a lane or remote-only launch).
import type { T3Client } from './client';
import { pushToast, dismissToast } from './toast';
import { arr, obj, str, type Obj } from './domain';
import { bridgeReply, ClientError, type Native } from './protocol';
import { shellPrefs } from './shell-prefs';
import { fleet, EnvironmentFleet, type FleetEntry } from './settings-b-fleet';
import { isPrimaryEnvironment, primary } from './local-primary';
import { letGo } from './let-go';
import {
  canOneClickUpdateProviderCandidate, collectProviderUpdateCandidates, collectUpdatedProviderSnapshots, failure, firstFailedProviderUpdateMessage,
  getProviderUpdateInitialToastView, getProviderUpdateProgressToastView, getProviderUpdateRejectedToastView, providerUpdateNotificationKey,
  providersOf, shouldShowPrimaryProviderUpdateToast, success, type CommandResult, type ProviderUpdateToastView,
} from './provider-updates';

/** A server.updateProvider can run the provider's installer: wait up to 15 minutes, not the transport's 30 s. */
export const UPDATE_TIMEOUT_SECONDS = 900;
/** An environment that can be asked for an update: its providers, its label, and a request on its transport. */
export interface UpdateTarget { key: string; label: string; providers: Obj[]; request(method: string, payload: Obj): Promise<Obj> }
type Client = Pick<T3Client, 'ready' | 'preferencesLoaded' | 'environmentId' | 'config' | 'local' | 'generation' | 'call'>;

/** The focused environment as an update target. */
export function focusedTarget(client: Client, native: Native | null): UpdateTarget {
  return { key: `focus:${client.environmentId}`, label: str(obj(client.config.environment).label, 'Environment'), providers: arr(client.config.providers),
    request: (method, payload) => {
      if (!native) throw new ClientError('Open this app on macOS to update providers.');
      return client.call(native, { op: 'request', method, payload, timeout: UPDATE_TIMEOUT_SECONDS }, client.generation, true);
    } };
}
/** A connected background environment (settings-b-fleet.ts) as an update target. */
export function fleetTarget(entry: FleetEntry, native: Native | null, label = ''): UpdateTarget {
  return { key: entry.key, label: label || str(obj(entry.config.environment).label, 'Environment'), providers: arr(entry.config.providers),
    request: async (method, payload) => {
      if (!native) throw new ClientError('Open this app on macOS to update providers.');
      const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { op: 'request', method, payload, generation: entry.generation, timeout: UPDATE_TIMEOUT_SECONDS });
      if (!reply.ok) throw new ClientError(reply.error?.message || 'Provider update failed.', reply.error?.kind ?? 'rpc', reply.error?.uncertain === true);
      if (reply.generation !== entry.generation) throw new ClientError('The connection changed. Refresh before continuing.', 'stale', true);
      return obj(reply.value);
    } };
}
/** usePrimaryEnvironment + primaryServerProvidersAtom: the embedded server, wherever it is connected. */
export function primaryTarget(client: Client, native: Native | null): UpdateTarget | null {
  if (client.ready && (isPrimaryEnvironment(client.environmentId) || !primary.target?.environmentId)) return focusedTarget(client, native);
  const entry = [...fleet.entries.values()].find(candidate => candidate.primary === true && candidate.phase === 'connected' && Object.keys(candidate.config).length > 0);
  return entry ? fleetTarget(entry, native, primary.target?.label || '') : null;
}

type Active = { kind: 'prompt'; key: string; toastId: number; oneClick: Obj[] } | { kind: 'update'; key: string; providerInstanceIds: ReadonlySet<string>; providerCount: number };
type NotifyState = { active: Active | null; dismissed: Set<string> | null };
const states = new WeakMap<object, NotifyState>();
const seenProviderUpdateNotificationKeys = new Set<string>();
const stateOf = (client: object): NotifyState => { let state = states.get(client); if (!state) states.set(client, state = { active: null, dismissed: null }); return state; };
/** For tests: forget what this process has prompted. */
export function resetProviderUpdateNotifications(): void { seenProviderUpdateNotificationKeys.clear(); }

const settingsAction = { label: 'Settings', op: 'ui:settings', id: 'providers' };
/** addProviderUpdateToast: loading and success plain (success leaves after 3 s); warning and error stacked with Settings. */
function addProviderUpdateToast(client: object, view: Pick<ProviderUpdateToastView, 'type' | 'title' | 'description' | 'dismissAfterVisibleMs'>): number {
  const owner = client as T3Client;
  if (view.type === 'loading' || view.type === 'success') {
    return pushToast(owner, { kind: view.type, title: view.title, description: view.description, timeoutMs: view.dismissAfterVisibleMs ?? 0, hideCopy: true });
  }
  return pushToast(owner, { kind: view.type, title: view.title, description: view.description, timeoutMs: 0, stacked: true, action: settingsAction, actionVariant: 'outline', hideCopy: true });
}

/** The prompt and the running update, once per shell answer (the reference's two effects). */
export function providerUpdates(client: Client, source: UpdateTarget | null = primaryTarget(client, null)): void {
  const state = stateOf(client);
  if (!client.ready || !client.preferencesLoaded) return;
  if (!state.dismissed) state.dismissed = new Set(shellPrefs(client as T3Client).providerUpdateDismissals);
  const providers = source?.providers ?? [];
  const active = state.active;
  if (active?.kind === 'update') {
    const view = getProviderUpdateProgressToastView({ providers: providers.filter(provider => active.providerInstanceIds.has(str(provider.instanceId))), providerCount: active.providerCount });
    if (shouldShowPrimaryProviderUpdateToast(view)) { addProviderUpdateToast(client, view); state.active = null; }
  }
  const updateProviders = collectProviderUpdateCandidates(providers);
  const notificationKey = providerUpdateNotificationKey(updateProviders);
  const oneClickProviders = updateProviders.filter(provider => canOneClickUpdateProviderCandidate(provider, providers));
  const current = state.active;
  if (current?.kind === 'prompt' && current.key !== notificationKey) { dismissToast(client as T3Client, current.toastId); state.active = null; }
  if (!notificationKey || state.dismissed.has(notificationKey) || seenProviderUpdateNotificationKeys.has(notificationKey) || state.active) return;
  seenProviderUpdateNotificationKeys.add(notificationKey);
  const initialView = getProviderUpdateInitialToastView({ updateProviders, oneClickProviders });
  const dismissed = state.dismissed, key = notificationKey;
  const toastId = pushToast(client as T3Client, { kind: 'warning', title: initialView.title, description: initialView.description, timeoutMs: 0, stacked: true, actionVariant: 'outline', hideCopy: true,
    ...(updateProviders.length === 1 ? { leading: `provider:${str(updateProviders[0]!.driver)}` } : {}),
    ...(oneClickProviders.length > 0 ? { action: { label: 'Update', op: 'upkeep:update-launch', id: key }, secondary: settingsAction, secondaryVariant: 'outline' as const } : { action: settingsAction }),
    onClose: () => { dismissed.add(key); shellPrefs(client as T3Client).providerUpdateDismissals = [...dismissed].slice(-100); } });
  state.active = { kind: 'prompt', key, toastId, oneClick: oneClickProviders };
}

/** The prompt's "Update" (runUpdates): each one-click provider on the primary, in order, then one outcome toast. */
export async function runLaunchUpdates(client: Client, key: string, source: UpdateTarget | null): Promise<void> {
  const state = stateOf(client), prompt = state.active;
  if (prompt?.kind !== 'prompt' || prompt.key !== key || prompt.oneClick.length === 0 || !source) return;
  const providerCount = prompt.oneClick.length;
  const providerInstanceIds = new Set(prompt.oneClick.map(provider => str(provider.instanceId)));
  const activeUpdate: Active = { kind: 'update', key, providerInstanceIds, providerCount };
  state.active = activeUpdate;
  dismissToast(client as T3Client, prompt.toastId);
  const results: CommandResult<{ providers: Obj[] }>[] = [];
  for (const provider of prompt.oneClick) {
    try { results.push(success({ providers: providersOf(await source.request('server.updateProvider', { provider: str(provider.driver), instanceId: str(provider.instanceId) })) })); }
    catch (error) { results.push(failure(error, letGo(error))); }
  }
  if (state.active !== activeUpdate) return;
  const failedMessage = firstFailedProviderUpdateMessage(results);
  if (failedMessage) { addProviderUpdateToast(client, getProviderUpdateRejectedToastView(providerCount, failedMessage)); state.active = null; return; }
  const view = getProviderUpdateProgressToastView({ providers: collectUpdatedProviderSnapshots({ results, providerInstanceIds }), providerCount });
  if (shouldShowPrimaryProviderUpdateToast(view)) { addProviderUpdateToast(client, view); state.active = null; }
}
