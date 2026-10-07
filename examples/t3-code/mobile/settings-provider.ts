// @ref llp/1106.009-mobile-settings.decision.md#root-and-native-lifetime
// @ref llp/1106.003-pairing-and-transport.decision.md#decision
// T3 Code 365aa87982 SettingsProviderAccountsRouteScreen. No credentials persisted.
import { mobileClient } from './client';
import { arr, obj, str, type Obj } from './shared/domain';
import { letGo } from './shared/let-go';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { settingsCall, settingsEndpoint, settingsEndpointCurrent, settingsGrants, settingsNative, settingsSources,
  type MobileSettingsEndpoint, type MobileSettingsSource } from './settings-server-source';

type Account = { streamKey: string; sourceKey: string; instanceId: string; generation: number; subscription: string; lastSequence: number;
  state: Obj | null; error: string; queryError: string; pending: boolean; choosing: boolean; revealed: boolean; draftIdentity: string; values: Record<string, string>; externalLinksAvailable: boolean };
const accounts = new Map<string, Account>();
const streamPrefix = 'mobile-provider-auth:';
const identity = (state: Obj | null) => `${str(state?.flowId)}:${str(obj(state?.interaction).id)}`;
function accountFor(endpoint: MobileSettingsEndpoint, instanceId: string): Account {
  const streamKey = streamPrefix + encodeURIComponent(JSON.stringify([endpoint.source.key, instanceId]));
  let account = accounts.get(streamKey);
  if (!account || account.generation !== endpoint.generation) {
    account = { streamKey, sourceKey: endpoint.source.key, instanceId, generation: endpoint.generation, subscription: '', lastSequence: 0,
      state: null, error: '', queryError: '', pending: false, choosing: false, revealed: false, draftIdentity: '', values: {}, externalLinksAvailable: false };
    accounts.set(streamKey, account);
  }
  return account;
}
function adopt(account: Account, state: Obj) {
  const nextIdentity = identity(state);
  if (nextIdentity !== account.draftIdentity) { account.values = {}; account.draftIdentity = nextIdentity; }
  account.state = state;
}
/** Observe the shared owner's event read before it acknowledges. Never drain or ack independently. */
export function settingsProviderNative(native: Native): Native {
  return { available: native.available, watch: topic => native.watch(topic), later: async request => {
    if (obj(request).op !== 'events') return native.later(request);
    // bridgeReply resolves a large transferred batch before inspection; return the same decoded envelope.
    const reply = await bridgeReply(native, request);
    if (reply.ok) settingsProviderEvents(arr(obj(reply.value).events));
    return reply;
  } };
}
export function settingsProviderEvents(events: Obj[]): void {
  for (const event of events) {
    const account = accounts.get(str(event.key));
    if (!account || event.generation !== account.generation || typeof event.seq !== 'number' || event.seq <= account.lastSequence
      || account.subscription && event.subscriptionId !== account.subscription) continue;
    account.lastSequence = event.seq;
    const state = obj(event.value);
    if (state._retryDue || state._streamEnded) { account.subscription = ''; continue; }
    if (state._transportError) { account.queryError = str(obj(state._transportError).message, 'Could not load provider sign-in.'); continue; }
    account.queryError = ''; adopt(account, state);
  }
}
async function watch(endpoint: MobileSettingsEndpoint, account: Account) {
  if (account.subscription) return;
  const reply = await settingsCall(endpoint, { op: 'subscribe', key: account.streamKey, method: 'provider.auth.subscribe', payload: { instanceId: account.instanceId } });
  account.subscription = str(reply.id);
  if (!account.subscription) throw new ClientError('Provider sign-in did not return a subscription.');
}
export function mobileProviderProjection(source: Pick<MobileSettingsSource, 'key' | 'label'>, provider: Obj, state: Obj | null,
  view: { key: string; error?: string; queryError?: string; pending?: boolean; choosing?: boolean; revealed?: boolean; values?: Record<string, string>; writable?: boolean; externalLinksAvailable?: boolean }) {
  const interaction = obj(state?.interaction), setup = obj(provider.setup), auth = obj(provider.auth), active = ['starting', 'waiting', 'verifying'].includes(str(state?.phase));
  const signedIn = auth.status === 'authenticated' || auth.status === 'unknown' && state?.phase === 'succeeded';
  const discovering = provider.driver === 'acpRegistry' && !active && !signedIn && !view.queryError && state?.methods === undefined;
  const external = !active && !signedIn && (setup.canAuthenticate === false || provider.driver === 'acpRegistry' && Array.isArray(state?.methods) && state.methods.length === 0);
  const url = ['browser', 'deviceCode'].includes(str(interaction.type)) ? str(interaction.url) : str(state?.authorizationUrl);
  const disabled = view.pending === true || !!view.queryError || discovering || view.writable === false;
  const values = view.values ?? {}, fields: { name: string; label: string; secret: boolean; value: string; limit: number }[] = [];
  if (interaction.type === 'terminal') fields.push({ name: 'input', label: 'Terminal response', secret: true, value: values.input ?? '', limit: 4095 });
  if (interaction.type === 'credentials') for (const field of arr(interaction.fields)) fields.push({ name: str(field.name), label: str(field.label), secret: field.secret === true, value: values[str(field.name)] ?? '', limit: 16384 });
  const callback = !!url && (interaction.type === 'browser' ? interaction.acceptsCallback === true : !state?.interaction);
  if (callback) fields.push({ name: 'callback', label: 'Final localhost URL', secret: false, value: values.callback ?? '', limit: 16384 });
  const actions: { op: string; value: string; label: string; icon: string; disabled: boolean; danger: boolean; inline: boolean; loading: boolean }[] = [];
  const action = (op: string, label: string, icon: string, extraDisabled = false, value = '', danger = false) => actions.push({ op, value, label, icon, disabled: disabled || extraDisabled, danger, inline: ['respond-terminal', 'respond-credentials', 'complete'].includes(op), loading: view.pending === true && op === 'choose' });
  if (interaction.type === 'terminal') action('respond-terminal', 'Send response', 'arrow.up');
  if (interaction.type === 'credentials') action('respond-credentials', 'Connect', 'person.crop.circle');
  if (callback) action('complete', 'Continue', 'arrow.right', !values.callback?.trim());
  if (view.choosing && !active) {
    for (const method of arr(state?.methods)) action('start', str(method.name), 'person.crop.circle', false, str(method.id));
    action('choose-cancel', 'Cancel', 'xmark');
  }
  if (url) action('open', 'Open sign-in page', 'globe', view.externalLinksAvailable !== true);
  if (external && setup.documentationUrl) actions.push({ op: 'docs', value: '', label: 'Open docs', icon: 'globe', disabled: view.externalLinksAvailable !== true, danger: false, inline: false, loading: false });
  else if (active && state?.flowId) action('cancel', 'Cancel sign-in', 'xmark');
  else if (!active && !external && setup.canAuthenticate !== false) action('choose', signedIn ? 'Change account' : 'Sign in', 'person.crop.circle', provider.enabled !== true || provider.installed !== true || state === null);
  if (!active && signedIn && (auth.canLogout ?? setup.canAuthenticate) === true) action('logout', 'Sign out', 'person.crop.circle', state === null, '', true);
  const email = signedIn && !active ? str(auth.email).trim() : '';
  return { key: view.key, label: str(provider.displayName) || str(provider.driver), disabled, pending: view.pending === true,
    message: active || ['failed', 'cancelled'].includes(str(state?.phase)) ? str(state?.message) : signedIn ? 'Signed in.' : discovering ? 'Discovering sign-in methods…'
      : external ? "No in-app sign-in advertised. Follow the provider's docs to finish setup." : 'Connect this provider.',
    email: email ? view.revealed ? email : '••••••@••••••' : '', emailAction: view.revealed ? 'Hide account email' : 'Reveal account email',
    deviceCode: interaction.type === 'deviceCode' ? `Enter code ${str(interaction.userCode)} on the sign-in page.` : '',
    output: interaction.type === 'terminal' ? str(interaction.output).replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '') : '',
    error: view.error || view.queryError || '', fields, actions,
    logoutTitle: 'Sign out?', logoutMessage: `Running threads sharing this sign-in on ${source.label} will stop. Thread history is kept.` };
}
type PreparedSection = { source: Pick<MobileSettingsSource, 'key' | 'label'>; providers: { provider: Obj; key: string }[]; writable: boolean; permissionError: string };
export interface ProviderSettingsData { ready: boolean; error: string; emptyMessage: string; sections: { key: string; title: string; emptyMessage: string; accounts: ReturnType<typeof mobileProviderProjection>[] }[] }
let prepared: { signature: string; sections: PreparedSection[] } | null = null;
let preparationEpoch = 0;
const emptyProviderData = (): ProviderSettingsData => ({ ready: false, error: '', emptyMessage: '', sections: [] });
function providerSelection(environmentIdsJSON: string): { ids: string[]; signature: string } {
  const value: unknown = JSON.parse(environmentIdsJSON);
  if (!Array.isArray(value) || value.some(id => typeof id !== 'string')) throw new ClientError('The provider settings scope is invalid.');
  const ids = [...new Set(value as string[])];
  return { ids, signature: JSON.stringify([...ids].sort()) };
}
/** Local field/reveal/chooser projection: no native reads, subscriptions, watches or permission assumptions. */
export function mobileProviderAccountsSnapshot(environmentIdsJSON: string, active: boolean): ProviderSettingsData {
  const empty = emptyProviderData();
  if (!active) return empty;
  try {
    if (prepared?.signature !== providerSelection(environmentIdsJSON).signature) return empty;
    const sections = prepared.sections.map(section => ({ key: section.source.key, title: section.source.label,
      emptyMessage: section.providers.length ? '' : 'Configure a provider with in-app sign-in in web or desktop Settings.',
      accounts: section.providers.flatMap(({ provider, key }) => {
        const account = accounts.get(key);
        return account ? [mobileProviderProjection(section.source, provider, account.state,
          { ...account, key, writable: section.writable, queryError: account.queryError || section.permissionError })] : [];
      }) }));
    return { ...empty, ready: true, emptyMessage: sections.length ? '' : 'Select a connected environment.', sections };
  } catch (error) { return { ...empty, error: error instanceof Error ? error.message : 'The provider settings scope is invalid.' }; }
}
export async function mobileProviderAccounts(environmentIdsJSON: string, active: boolean, nativeInput: Native | null | undefined, externalLinksAvailable = false): Promise<ProviderSettingsData> {
  const empty = emptyProviderData(), epoch = ++preparationEpoch;
  // Unrelated app revisions must not wake native I/O for a never-opened/fully released route.
  if (!active && accounts.size === 0) { prepared = null; return empty; }
  if (!nativeInput?.available) return { ...empty, error: active ? 'Open T3 Code on your iPhone or iPad to manage provider accounts.' : '' };
  const native = settingsNative(nativeInput);
  native.watch('t3.status'); native.watch('t3.fleet');
  try {
    const { ids, signature } = providerSelection(environmentIdsJSON);
    const allSources = await settingsSources(native);
    if (epoch !== preparationEpoch) return empty;
    const sources = allSources.filter(source => active && ids.includes(source.environmentId) && source.enabled && source.phase === 'connected');
    const wanted = new Set<string>();
    for (const source of sources) for (const provider of arr(source.config.providers).filter(provider => obj(provider.setup).canAuthenticate === true || provider.driver === 'acpRegistry' && provider.installed === true)) wanted.add(accountFor(settingsEndpoint(source, native), str(provider.instanceId)).streamKey);
    // Release subscriptions when leaving the route or reducing its environment selection.
    for (const [key, account] of accounts) if (!wanted.has(key)) {
      const source = allSources.find(item => item.key === account.sourceKey);
      if (source && account.subscription) {
        const endpoint = settingsEndpoint(source, native);
        if (endpoint.generation === account.generation && settingsEndpointCurrent(endpoint)) await settingsCall(endpoint, { op: 'unsubscribe', key: account.streamKey });
      }
      if (epoch !== preparationEpoch) return empty;
      accounts.delete(key);
    }
    const sections = await Promise.all(sources.map(async source => {
      const endpoint = settingsEndpoint(source, native);
      let writable = false, permissionError = '';
      try { writable = settingsGrants(await settingsCall(endpoint, { op: 'http', path: '/api/auth/session' }), 'providers:manage'); }
      catch (error) { if (letGo(error)) throw error; permissionError = error instanceof Error ? error.message : 'Could not verify provider permissions.'; }
      const providers = arr(source.config.providers).filter(provider => obj(provider.setup).canAuthenticate === true || provider.driver === 'acpRegistry' && provider.installed === true);
      return { source: { key: source.key, label: source.label }, writable, permissionError,
        providers: await Promise.all(providers.map(async provider => {
          const account = accountFor(endpoint, str(provider.instanceId));
          account.externalLinksAvailable = externalLinksAvailable;
          try { await watch(endpoint, account); } catch (error) { if (letGo(error)) throw error; account.queryError = error instanceof Error ? error.message : 'Could not load provider sign-in.'; }
          return { provider, key: account.streamKey };
        })) };
    }));
    if (epoch !== preparationEpoch) return empty;
    prepared = active ? { signature, sections } : null;
    return active ? mobileProviderAccountsSnapshot(environmentIdsJSON, true) : empty;
  } catch (error) {
    if (letGo(error)) throw error;
    if (epoch === preparationEpoch) prepared = null;
    return { ...empty, error: error instanceof Error ? error.message : 'Could not load provider accounts.' };
  }
}
/** Fields are ephemeral and flow-scoped. Root passes edits; never persist or echo in logs. */
export function mobileProviderField(key: string, name: string, value: string) {
  const account = accounts.get(key);
  if (!account || account.pending) return { revision: mobileClient.revision, message: '' };
  const interaction = obj(account.state?.interaction);
  const limit = interaction.type === 'terminal' ? 4095 : 16384;
  const allowed = name === 'callback' || interaction.type === 'terminal' && name === 'input' || interaction.type === 'credentials' && arr(interaction.fields).some(field => field.name === name);
  if (!allowed || value.length > limit) return { revision: mobileClient.revision, message: 'That sign-in field is invalid.' };
  account.values[name] = value;
  return { revision: ++mobileClient.revision, message: '' };
}
export async function mobileProviderCommand(op: string, key: string, value: string, nativeInput: Native | null | undefined) {
  const account = accounts.get(key);
  if (!account || account.pending) return { revision: mobileClient.revision, message: 'That sign-in action is no longer available.' };
  const finish = (message = '') => ({ revision: ++mobileClient.revision, message });
  if (op === 'reveal') { account.revealed = !account.revealed; return finish(); }
  if (op === 'choose-cancel') { account.choosing = false; return finish(); }
  account.pending = true; account.error = '';
  try {
    if (!nativeInput?.available) throw new ClientError('Provider sign-in requires the native app.');
    const native = settingsNative(nativeInput), source = (await settingsSources(native)).find(source => source.key === account.sourceKey);
    if (!source) throw new ClientError('This environment is no longer saved on this device.');
    const endpoint = settingsEndpoint(source, native), provider = arr(source.config.providers).find(provider => provider.instanceId === account.instanceId);
    if (endpoint.generation !== account.generation || !provider) throw new ClientError('The provider connection changed. Reopen this page.');
    const state = account.state, interaction = obj(state?.interaction), flowId = str(state?.flowId);
    const currentIdentity = identity(state);
    if (op !== 'docs' && !settingsGrants(await settingsCall(endpoint, { op: 'http', path: '/api/auth/session' }), 'providers:manage')) throw new ClientError('This connection does not allow provider account changes.');
    const projection = mobileProviderProjection(source, provider, state, { key, values: account.values, choosing: account.choosing, externalLinksAvailable: account.externalLinksAvailable, queryError: account.queryError });
    const allowed = projection.actions.find(action => action.op === op || op === 'logout-confirmed' && action.op === 'logout');
    if (!allowed || allowed.disabled) throw new ClientError('That sign-in action is no longer available.');
    const rpc = async (method: string, payload: Obj) => {
      if (identity(account.state) !== currentIdentity) throw new ClientError('The sign-in prompt changed. Check the current prompt before continuing.', 'stale');
      const result = await settingsCall(endpoint, { op: 'request', method, payload: { instanceId: account.instanceId, ...payload } });
      if (typeof result.phase === 'string' && identity(account.state) === currentIdentity) adopt(account, result);
      return result;
    };
    const respond = async (response: Obj) => {
      if (!flowId || !str(interaction.id)) throw new ClientError('The sign-in prompt is no longer available.');
      await rpc('provider.auth.respond', { flowId, interactionId: str(interaction.id), response });
    };
    if (op === 'choose' && arr(state?.methods).length > 1) account.choosing = true;
    else if (op === 'choose' || op === 'start') {
      if (op === 'start' && !arr(state?.methods).some(method => method.id === value)) throw new ClientError('That sign-in method is no longer available.');
      account.choosing = false; await rpc('provider.auth.start', op === 'start' ? { methodId: value } : {});
    } else if (op === 'cancel') await rpc('provider.auth.cancel', { flowId });
    else if (op === 'logout-confirmed') await rpc('provider.auth.logout', {});
    else if (op === 'logout') throw new ClientError('Confirm sign out before continuing.');
    else if (op === 'respond-terminal') { await respond({ type: 'terminal', data: `${account.values.input ?? ''}\r` }); account.values = {}; }
    else if (op === 'respond-credentials') { await respond({ type: 'credentials', values: account.values }); account.values = {}; }
    else if (op === 'complete') { await rpc('provider.auth.complete', { flowId, callbackUrl: account.values.callback ?? '' }); account.values = {}; }
    else if (op === 'open' || op === 'docs') {
      const url = op === 'docs' ? str(obj(provider.setup).documentationUrl) : ['browser', 'deviceCode'].includes(str(interaction.type)) ? str(interaction.url) : str(state?.authorizationUrl);
      if (interaction.type === 'browser' && interaction.requiresConsent === true && op === 'open') await respond({ type: 'browser', action: 'accept' });
      const reply = await bridgeReply(native, { op: 'mobileOpenURL', url });
      if (!reply.ok) throw new ClientError(op === 'docs' ? 'Could not open the provider docs.' : 'Could not open the sign-in page.');
    } else throw new ClientError('Unknown provider sign-in action.');
    return finish();
  } catch (error) { if (letGo(error)) throw error; account.error = error instanceof Error ? error.message : 'Could not update provider sign-in.'; return finish(account.error); }
  finally { account.pending = false; }
}
