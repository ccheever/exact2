// Provider setup streams and commands, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// packages/client-runtime/src/state/server.ts:989-1060 (providerAuthState and
// providerInstallState: one `provider.auth.subscribe` / `provider.install.subscribe`
// per instance while a row shows it, idleTtlMs 0) and the commands of
// apps/web/src/components/settings/ProviderAuthenticationSection.tsx:103-160, 226-240,
// 267-313, 391-440 (run, send, openBrowser, copy link, credentials, callback, cancel,
// sign out) and ProviderSetupSection.tsx:152-186, 232-269, 316-320 (runCommand,
// install, cancel, remove, "Retry setup status").
// Changes: the state lives here per connection instead of in atoms; a row asks for its
// streams through `watchProviderSetup` (the Providers page and the wizard are owners).
// The reference coalesces presses with pendingRef while a command runs; Contract sends
// setup commands through a queue mutation, which runs a second press after the first
// instead, so every command carries the state token its button was drawn from and a
// press is dropped when that state moved on or a command already succeeded from it. Opening the browser is the native `remoteEditorsOpen`
// op: the clone's port of ElectronShell.openExternal (T3RemoteEditors.swift; an agent
// run records the URL instead of opening it). Terminal sign-in input goes through the
// terminal view (provider-auth-terminal.ts, T3TerminalSessions.swift).
import { arr, obj, str, type Obj } from './domain';
import { bridgeReply, ClientError, type Native } from './protocol';
import type { ProviderHost } from './providers';
import { letGo } from './let-go';
import { terminalEventError } from './provider-auth-terminal';

export type SetupStream = { state: Obj | null; error: string; subscription: string; requested: boolean };
export type ProviderSetupEntry = {
  auth: SetupStream; install: SetupStream;
  /** ProviderAuthenticationSection's error and pending; ProviderSetupActions' own. */
  authError: string; authPending: boolean; installError: string; installPending: string;
  /** Bumped when a credentials or callback form succeeds, so its drafts start empty (setDraft({ id: '', values: {} })). */
  cleared: number;
  /** `<op>|<token>` of the commands that succeeded since the streams' state last changed. */
  done: Set<string>;
};
type Store = { generation: number; entries: Map<string, ProviderSetupEntry>; owners: Map<string, { auth: string[]; install: string[] }> };
type Host = ProviderHost & { generation?: number };

const stores = new WeakMap<object, Store>();
const stream = (): SetupStream => ({ state: null, error: '', subscription: '', requested: false });
function store(host: Host): Store {
  const generation = host.generation ?? 0;
  let value = stores.get(host);
  if (!value || value.generation !== generation) { value = { generation, entries: new Map(), owners: value?.owners ?? new Map() }; stores.set(host, value); }
  return value;
}
/** The setup state of one instance on this connection (empty until a row watches it). */
export function setupOf(host: Host, id: string): ProviderSetupEntry {
  const entries = store(host).entries;
  let entry = entries.get(id);
  if (!entry) entries.set(id, entry = { auth: stream(), install: stream(), authError: '', authPending: false, installError: '', installPending: '', cleared: 0, done: new Set() });
  return entry;
}
const KEYS = { auth: 'provider-auth:', install: 'provider-install:' } as const;
const METHODS = { auth: 'provider.auth.subscribe', install: 'provider.install.subscribe' } as const;

/** One `provider-auth:<id>` / `provider-install:<id>` inbox entry (client.ts drain). */
export function providerSetupEvent(host: Host, entry: Obj): boolean {
  const key = str(entry.key), kind = key.startsWith(KEYS.auth) ? 'auth' : key.startsWith(KEYS.install) ? 'install' : null;
  if (!kind) return false;
  const value = store(host);
  if (Number(entry.generation) !== value.generation) return true;
  const setup = value.entries.get(key.slice(KEYS[kind].length)), target = setup?.[kind];
  if (!setup || !target || !target.subscription || str(entry.subscriptionId) !== target.subscription) return true;
  const item = obj(entry.value);
  // T3Transport retries a failed stream after a backoff and says so with `_retryDue`; the next watch resubscribes.
  if (item._retryDue || item._streamEnded) { target.subscription = ''; target.requested = false; return true; }
  if (item._transportError) { target.error = str(obj(item._transportError).message, 'Could not load the provider setup status.'); return true; }
  target.state = item; target.error = ''; setup.done.clear();
  return true;
}

/**
 * The streams one owner's rows show: `auth` and `install` list instance ids. The union of
 * every owner is subscribed; a stream no owner wants is closed and forgotten (idleTtlMs 0).
 */
export async function watchProviderSetup(host: Host, native: Native, owner: string, wanted: { auth: string[]; install: string[] }): Promise<void> {
  const value = store(host);
  value.owners.set(owner, wanted);
  const union = { auth: new Set<string>(), install: new Set<string>() };
  for (const entry of value.owners.values()) { entry.auth.forEach(id => union.auth.add(id)); entry.install.forEach(id => union.install.add(id)); }
  for (const [id, entry] of value.entries) {
    for (const kind of ['auth', 'install'] as const) {
      if (union[kind].has(id) || !(entry[kind].subscription || entry[kind].requested)) continue;
      entry[kind] = stream();
      await bridgeReply(native, { op: 'unsubscribe', key: `${KEYS[kind]}${id}`, generation: value.generation }).catch(() => null);
    }
  }
  if (!host.ready) return;
  for (const kind of ['auth', 'install'] as const) {
    for (const id of union[kind]) {
      const target = setupOf(host, id)[kind];
      if (target.subscription || target.requested) continue;
      target.requested = true;
      const generation = value.generation;
      try {
        const reply = await bridgeReply(native, { op: 'subscribe', key: `${KEYS[kind]}${id}`, method: METHODS[kind], payload: { instanceId: id }, generation });
        if (store(host).generation !== generation || reply.generation !== generation) continue;
        if (reply.ok) target.subscription = str(obj(reply.value).id);
        else { target.requested = false; target.error = reply.error?.message || 'Could not load the provider setup status.'; }
      } catch (error) { target.requested = false; if (letGo(error)) throw error; target.error = error instanceof Error ? error.message : 'Could not load the provider setup status.'; }
    }
  }
}

/** `${flowId}:${interactionId}`: the identity of the auth form a draft belongs to. */
export function authDraftId(state: Obj | null): string { return `${str(state?.flowId)}:${str(obj(state?.interaction).id)}`; }
/** The browser or device-code link, else the flow's authorization URL. */
export function authUrl(state: Obj | null): string {
  const interaction = obj(state?.interaction);
  return interaction.type === 'browser' || interaction.type === 'deviceCode' ? str(interaction.url) : str(state?.authorizationUrl);
}
const failureText = (error: unknown, fallback: string) => error instanceof Error && error.message ? error.message : fallback;

/** ProviderAuthenticationSection `run`: one command at a time; a failure (not an interruption) is the row's error. */
async function run(entry: ProviderSetupEntry, command: () => Promise<unknown>): Promise<boolean> {
  if (entry.authPending) return false;
  entry.authPending = true; entry.authError = '';
  try { await command(); return true; }
  catch (error) { if (letGo(error)) throw error; entry.authError = failureText(error, 'Provider sign-in failed. Try again.'); return false; }
  finally { entry.authPending = false; }
}
/** ProviderSetupActions `runCommand`. */
async function runInstall(entry: ProviderSetupEntry, label: string, command: () => Promise<unknown>): Promise<boolean> {
  if (entry.installPending) return false;
  entry.installPending = label; entry.installError = '';
  try { await command(); return true; }
  catch (error) { if (letGo(error)) throw error; entry.installError = failureText(error, 'Provider setup failed.'); return false; }
  finally { entry.installPending = ''; }
}

/**
 * One setup command (`setup:<what>`). `token` is the state the pressed button was drawn
 * from (the auth flow's draft id or `phase:flowId`, the install's `phase:operationId`);
 * a press made against a state that has since changed, or one a command already
 * succeeded from, sends nothing.
 */
export async function providerSetupOp(host: Host, native: Native, op: string, id: string, token: string, value: string): Promise<void> {
  if (!host.writable && op !== 'setup:retry') return;
  const entry = setupOf(host, id), auth = entry.auth.state, interaction = obj(auth?.interaction), install = entry.install.state;
  const once = `${op}|${token}`;
  const current = op.startsWith('setup:install-') ? `${str(install?.phase)}:${str(install?.operationId)}`
    : ['setup:auth-start', 'setup:auth-cancel'].includes(op) ? `${str(auth?.phase)}:${str(auth?.flowId)}` : authDraftId(auth);
  if (!['setup:auth-logout', 'setup:install-remove', 'setup:retry', 'setup:auth-event'].includes(op) && (token !== current || entry.done.has(once))) return;
  const call = (method: string, payload: Obj) => host.rpc(native, method, { instanceId: id, ...payload }, true);
  const succeeded = (ok: boolean) => { if (ok) entry.done.add(once); return ok; };
  const respond = (response: Obj) => run(entry, () => call('provider.auth.respond', { flowId: str(auth?.flowId), interactionId: str(interaction.id), response }));
  switch (op) {
    case 'setup:auth-start': {
      const methods = arr(auth?.methods);
      succeeded(await run(entry, () => call('provider.auth.start', value && methods.some(method => method.id === value) ? { methodId: value } : {})));
      return;
    }
    case 'setup:auth-cancel':
      if (str(auth?.flowId)) succeeded(await run(entry, () => call('provider.auth.cancel', { flowId: str(auth?.flowId) })));
      return;
    case 'setup:auth-logout':
      await run(entry, () => call('provider.auth.logout', {}));
      return;
    case 'setup:auth-open': {
      const url = authUrl(auth);
      if (!url) return;
      // Consent is recorded on the environment before the provider URL opens here.
      if (interaction.type === 'browser' && interaction.requiresConsent === true && !(await respond({ type: 'browser', action: 'accept' }))) return;
      try {
        const reply = await bridgeReply(native, { op: 'remoteEditorsOpen', url });
        if (!reply.ok || obj(reply.value).opened !== true) throw new ClientError('Unable to open link.');
        entry.authError = '';
      } catch (error) { if (letGo(error)) throw error; entry.authError = 'Could not open the sign-in page. Copy the link and open it in your browser.'; }
      return;
    }
    case 'setup:auth-copy': {
      const url = authUrl(auth);
      if (!url) return;
      try {
        const copied = obj(await native.later({ op: 'copyText', text: url }));
        if (copied.ok !== true) throw new ClientError('Could not copy the sign-in link.');
        // The link only works once consent is recorded.
        if (interaction.type === 'browser' && interaction.requiresConsent === true) await respond({ type: 'browser', action: 'accept' });
      } catch (error) { if (letGo(error)) throw error; entry.authError = 'Could not copy the sign-in link.'; }
      return;
    }
    case 'setup:auth-credentials': {
      if (interaction.type !== 'credentials' || !str(auth?.flowId)) return;
      // The form's drafts in field order, each URI-encoded and joined by "&" (ProviderCredentialsForm).
      const drafts = value.split('&').map(part => { try { return decodeURIComponent(part); } catch { return ''; } }), values: Obj = {};
      arr(interaction.fields).slice(0, 16).forEach((field, index) => { values[str(field.name)] = drafts[index] ?? ''; });
      if (succeeded(await respond({ type: 'credentials', values }))) entry.cleared += 1;
      return;
    }
    case 'setup:auth-callback': {
      const callbackUrl = value.trim();
      if (!str(auth?.flowId) || !callbackUrl) return;
      if (succeeded(await run(entry, () => call('provider.auth.complete', { flowId: str(auth?.flowId), callbackUrl })))) entry.cleared += 1;
      return;
    }
    case 'setup:auth-event': {
      // The sign-in terminal's own failures (T3TerminalView auth mode), scoped to the flow it was drawn for.
      const event = obj(JSON.parse(value || '{}'));
      if (str(event.authFlow) && str(event.authFlow) !== authDraftId(auth)) return;
      if (!str(auth?.flowId) || interaction.type !== 'terminal') return;
      entry.authError = terminalEventError(str(event.type));
      return;
    }
    case 'setup:install-start':
      succeeded(await runInstall(entry, 'Starting installation', () => call('provider.install.start', {})));
      return;
    case 'setup:install-cancel': {
      const operationId = str(install?.operationId);
      if (operationId) succeeded(await runInstall(entry, 'Cancelling installation', () => call('provider.install.cancel', { operationId })));
      return;
    }
    case 'setup:install-remove':
      await runInstall(entry, 'Removing runtime', () => call('provider.install.remove', {}));
      return;
    case 'setup:retry':
      // "Retry setup status": both queries ask again.
      for (const kind of ['auth', 'install'] as const) {
        const target = entry[kind];
        if (!target.error) continue;
        target.error = '';
        if (target.subscription) await bridgeReply(native, { op: 'unsubscribe', key: `${KEYS[kind]}${id}`, generation: store(host).generation }).catch(() => null);
        target.subscription = ''; target.requested = false;
      }
      return;
    default: throw new ClientError(`Unknown action: ${op}`);
  }
}
