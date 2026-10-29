import { providerWritable } from './provider-permissions';
// ManagedCodexSetup's commands and effects, T3 Code 9c161462c9 (MIT, see LICENSE-T3):
// apps/web/src/components/settings/CodexSetupSection.tsx `run` :318-347, the handoff
// :349-387, cancelSignIn :388-395, signIn :397-454, setup :456-479, the auto start, continue
// and refresh effects :283-298 and :481-514, openPage and the listener cleanup :516-566, the
// buttons' handlers :620-1008; AddCodexAccountDialog.tsx (onboarding's createdAccount) and
// WelcomeWizard.tsx OnboardingCodexSetup.changeMode. Changes: each React effect is a step of
// `codexSetupPrepare`, which the snapshot answer runs after every refresh (app.ts) for the
// instances a row shows; a step that sends a request sets its guard first, because Exact may
// let an answer go after the request left (let-go.ts). The desktop's receiveProviderAuthCallback
// is openAuthSession's loopback redirect (LLP 1133.005 D9): a flow names the authorization URL
// whose callback it wants (`flow.receiving`), the window holds a resource on it (app.contract
// `codexLoopback`), and that answer (`receiveCallback`) has the host bind the redirect's port,
// open the browser and answer the one callback, then completes the flow; a new URL, or none, lets
// the answer go, which frees the port. `returnUrl` is omitted from provider.auth.start (the server
// does not use it in client mode; the clone registers no URL scheme) and is '' in the handoff.
import { arr, obj, str, type Obj } from '../client/domain';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { setupOf, watchedSetup } from './provider-setup';
import { codexFlow, codexFlows, codexStatus, readCodexSetupMode, codexInstance, type CodexFlow, type CodexHost } from './codex-setup';
import { codexCallbackUrl, isLoopbackHost, readCodexAuthorizationRequest } from './codex-auth-request';
import { CHATGPT_USAGE_URL, acknowledgeChatGptPlan } from './chatgpt-plan';
import { driverMeta, legacyProviders } from './providers-meta';
import { letGo } from '../client/let-go';

export const RECEIVE_FAILED = 'Could not finish sign-in on this computer. Try again or paste the redirect URL below.';
/** receiveCodexAuthCallback's bound: a sign-in whose browser has not called back in five minutes expires. */
export const RECEIVE_MS = 300_000;
declare function openAuthSession(url: string, options: { callback: string; state: string }): Promise<string>;
const SETUP_FAILED = 'Codex setup failed. Try again.';
import { HANDOFF_KEY, handoffHosts } from './codex-handoff-events';
import { openExternal as openSystem } from '../client/external-open';

/** One instance's setup on one connection: `native` reaches that connection, `module` the app's own module ops. */
export type CodexContext = {
  host: CodexHost; native: Native; module: Native; instanceId: string; origin: string; environmentId: string;
  /** The primary's background connection when this environment is remote (the handoff), else null. */
  primary: { key: string; generation: number; native: Native } | null;
};

const providerOf = (host: CodexHost, instanceId: string) => arr(host.config.providers).find(provider => provider.instanceId === instanceId);
const enabledOf = (host: CodexHost, instanceId: string) => providerOf(host, instanceId)?.enabled !== false;
const statusOf = (context: CodexContext) => codexStatus(context.host, context.instanceId, providerOf(context.host, context.instanceId), { enabled: enabledOf(context.host, context.instanceId), readOnly: !providerWritable(context.host) });
const call = (context: CodexContext, method: string, payload: Obj) => context.host.rpc(context.native, method, { instanceId: context.instanceId, ...payload }, true);

/** `run`: one command at a time; a failure (not an interruption) is the setup's error. */
export async function run(flow: CodexFlow, request: () => Promise<Obj>, onSuccess?: (value: Obj) => void): Promise<boolean> {
  if (flow.pending) return false;
  flow.pending = true; flow.error = '';
  try { const value = await request(); onSuccess?.(value); return true; }
  catch (error) { if (letGo(error)) throw error; flow.error = error instanceof Error && error.message ? error.message : SETUP_FAILED; return false; }
  finally { flow.pending = false; }
}

/** The browser open (ElectronShell.openExternal, external-open.ts; an agent run journals the URL). */
async function openExternal(_module: Native, url: string): Promise<boolean> { return openSystem(url); }

/** primaryAuthEnvironmentId: a remote environment signs in on the connected loopback primary. */
export function handoffAvailable(context: CodexContext): boolean {
  if (!context.primary) return false;
  try { return !isLoopbackHost(new URL(context.origin).hostname); } catch { return false; }
}

async function signIn(context: CodexContext, methodId = 'chatgpt'): Promise<void> {
  const flow = codexFlow(context.host, context.instanceId);
  if (flow.pending) return;
  flow.openRequested = true; flow.transferFailed = false; flow.requestedMethodId = methodId;
  if (handoffAvailable(context)) {
    const attemptId = `${context.instanceId}:${++flow.handoffSequence}`;
    const succeeded = await run(flow, () => call(context, 'provider.chatgpt.reconnect-profile', { methodId }), profile => {
      flow.handoff = { attemptId, primaryKey: context.primary!.key, subscription: '', state: null, error: '',
        input: { instanceId: context.instanceId, environmentId: context.environmentId, attemptId, returnUrl: '', profile: str(profile.clientId) ? profile : null } };
    });
    if (!succeeded) flow.openRequested = false;
    else { handoffHosts.add(context.host); await subscribeHandoff(context, flow); }
    return;
  }
  // Set before the request: a let-go answer still started the flow on the server.
  flow.awaiting = 'sign-in';
  if (!(await run(flow, () => call(context, 'provider.auth.start', { methodId, callbackMode: 'client' })))) { flow.openRequested = false; flow.awaiting = ''; }
}

/** `setup`: install or update first (then continue to sign in), else sign in. */
async function setup(context: CodexContext, methodId = 'chatgpt'): Promise<void> {
  const status = statusOf(context), flow = status.flow;
  if (status.unavailable || status.busy) return;
  if (status.installed && !status.updateAvailable) { await signIn(context, methodId); return; }
  flow.continueWithSignIn = methodId;
  if (!(await run(flow, () => call(context, 'provider.install.start', {})))) flow.continueWithSignIn = '';
}

async function cancelSignIn(context: CodexContext): Promise<void> {
  const flow = codexFlow(context.host, context.instanceId), status = statusOf(context);
  flow.awaiting = '';
  if (flow.handoff) { await dropHandoff(context, flow); return; }
  await run(flow, () => call(context, 'provider.auth.cancel', { flowId: status.flowId }));
}

/** openPage: hand the authorization URL to the loopback receiver (or open it again), else just open it. */
async function openPage(context: CodexContext, url: string): Promise<void> {
  const flow = codexFlow(context.host, context.instanceId), flowId = statusOf(context).flowId;
  try {
    if (!flow.handoff && flowId) {
      if (flow.receiving === url) { await openExternal(context.module, url); return; }
      if (!readCodexAuthorizationRequest(url)) throw new ClientError(RECEIVE_FAILED);
      flow.receiving = url; flow.receivingFlow = flowId; flow.receivingSince = 0;
    } else if (!(await openExternal(context.module, url))) throw new ClientError(RECEIVE_FAILED);
  } catch (error) { if (letGo(error)) throw error; flow.error = RECEIVE_FAILED; }
}

/**
 * receiveCodexAuthCallback's wait, as the window's resource on `flow.receiving` (app.contract `codexLoopback`):
 * openAuthSession binds the redirect's 127.0.0.1 port before the browser opens and answers the one GET on its path
 * with this sign-in's `state`; a callback with exactly one code or error (codexCallbackUrl) completes the flow. A refusal
 * (the port in use, the browser, a cancel) is RECEIVE_FAILED, which leaves the pasted-URL form.
 */
export async function receiveCallback(context: CodexContext, url: string): Promise<'received' | 'failed' | 'gone'> {
  const flow = codexFlow(context.host, context.instanceId), request = readCodexAuthorizationRequest(url);
  const fail = () => { if (flow.receiving === url) { flow.receiving = ''; flow.error = RECEIVE_FAILED; } return 'failed' as const; };
  if (!request) return fail();
  let callbackUrl: string;
  try {
    callbackUrl = await openAuthSession(url, { callback: request.redirectUri, state: request.state });
    codexCallbackUrl(callbackUrl, request.redirectUri, request.state);
  } catch (error) { if (letGo(error)) throw error; return fail(); }
  if (flow.receiving !== url) return 'gone';
  const flowId = flow.receivingFlow;
  flow.receiving = '';
  if (flowId) await run(flow, () => call(context, 'provider.auth.complete', { flowId, callbackUrl }));
  return 'received';
}

async function subscribeHandoff(context: CodexContext, flow: CodexFlow): Promise<void> {
  const handoff = flow.handoff, primary = context.primary;
  if (!handoff || !primary || handoff.subscription) return;
  handoff.subscription = 'requested';
  try {
    const reply = await bridgeReply(primary.native, { op: 'subscribe', key: `${HANDOFF_KEY}${context.instanceId}`, method: 'provider.chatgpt.handoff.subscribe', payload: handoff.input, generation: primary.generation });
    if (flow.handoff !== handoff) return;
    if (!reply.ok) throw new ClientError(reply.error?.message || 'ChatGPT sign-in on the primary environment was interrupted. Try again.');
    handoff.subscription = str(obj(reply.value).id);
  } catch (error) {
    if (letGo(error)) throw error;
    if (flow.handoff === handoff) { flow.error = 'ChatGPT sign-in on the primary environment was interrupted. Try again.'; flow.handoff = null; }
  }
}
async function dropHandoff(context: CodexContext, flow: CodexFlow): Promise<void> {
  const handoff = flow.handoff;
  flow.handoff = null;
  if (handoff?.subscription && context.primary) await bridgeReply(context.primary.native, { op: 'unsubscribe', key: `${HANDOFF_KEY}${context.instanceId}`, generation: context.primary.generation }).catch(() => null);
}

async function transferProfile(context: CodexContext, profile: Obj): Promise<void> {
  const flow = codexFlow(context.host, context.instanceId);
  const succeeded = await run(flow, () => call(context, 'provider.chatgpt.import-profile', { profile }));
  if (succeeded) { flow.awaiting = 'handoff'; await dropHandoff(context, flow); }
  flow.transferFailed = !succeeded;
}

/** OnboardingCodexSetup.changeMode / the editor's onModeChange: one atomic upsert of the instance. */
async function changeMode(context: CodexContext, mode: 'managed' | 'existing'): Promise<void> {
  const settings = await context.host.rpc(context.native, 'server.getSettings', {});
  const instance = codexInstance(settings, context.instanceId);
  const config = obj(instance.config);
  const next = { ...instance, driver: 'codex', enabled: true, config: { ...config, enabled: true, setupMode: mode } };
  const legacy = legacyProviders(settings), legacyDefault = context.instanceId === 'codex' && legacy ? driverMeta('codex')?.legacyDefault : undefined;
  await context.host.rpc(context.native, 'server.updateSettings', { patch: legacy && legacyDefault ? { providers: { ...legacy, codex: legacyDefault } } : {},
    providerInstanceMutation: { operation: 'upsert', instanceId: context.instanceId, instance: next } }, true);
  context.host.config = await context.host.rpc(context.native, 'server.getConfig', {});
}

/**
 * One command (`setup:codex-<what>`, `setup:chatgpt-<what>`). `token` is the state the pressed
 * control was drawn from (codexStatus().token); a press against a state that has moved on, or
 * one a command already succeeded from, sends nothing (the reference's pendingRef).
 */
export async function codexSetupOp(context: CodexContext, op: string, token: string, value: string, save: () => Promise<void>): Promise<void> {
  const flow = codexFlow(context.host, context.instanceId), status = statusOf(context), entry = setupOf(context.host, context.instanceId);
  if (op === 'setup:chatgpt-usage') { await openExternal(context.module, CHATGPT_USAGE_URL); return; }
  if (op === 'setup:chatgpt-plan-ack') { acknowledgeChatGptPlan(context.host, value); await save(); return; }
  if (!providerWritable(context.host)) return;
  const once = `${op}|${token}`;
  const guarded = !['setup:codex-callback', 'setup:codex-browser', 'setup:codex-mode', 'setup:codex-start'].includes(op);
  if (guarded && (token !== status.token || entry.done.has(once))) return;
  const done = () => { entry.done.add(once); };
  switch (op) {
    case 'setup:codex-setup': done(); await setup(context, value || 'chatgpt'); return;
    case 'setup:codex-pick': {
      // ChatGptAccountPicker.onSelect: a saved profile, else a new account.
      done(); await setup(context, status.methods.some(method => method.id === value) ? value : 'chatgpt-change-account'); return;
    }
    case 'setup:codex-different': done(); await setup(context, 'chatgpt-change-account'); return;
    case 'setup:codex-open': {
      // The waiting control: retry a transfer that failed, else open the sign-in page.
      done();
      const handoffState = obj(flow.handoff?.state);
      if (handoffState.phase === 'finished') await transferProfile(context, obj(handoffState.profile));
      else if (status.url) await openPage(context, status.url);
      return;
    }
    case 'setup:codex-cancel':
      if (!flow.handoff && !status.flowId) return;
      done(); flow.openRequested = false; await cancelSignIn(context); return;
    case 'setup:codex-cancel-any': {
      // The welcome card's Cancel: the sign-in, else the installation.
      done(); flow.openRequested = false; flow.continueWithSignIn = '';
      if (flow.handoff || (status.authActive && status.flowId)) await cancelSignIn(context);
      else if (str(status.installation?.operationId)) await run(flow, () => call(context, 'provider.install.cancel', { operationId: str(status.installation?.operationId) }));
      return;
    }
    case 'setup:codex-install-cancel':
      if (!str(status.installation?.operationId)) return;
      done(); flow.continueWithSignIn = '';
      await run(flow, () => call(context, 'provider.install.cancel', { operationId: str(status.installation?.operationId) }));
      return;
    case 'setup:codex-logout': done(); await run(flow, () => call(context, 'provider.auth.logout', {})); return;
    case 'setup:codex-callback': {
      // The pasted redirect URL, for the flow its field was drawn for.
      const callbackUrl = value.trim();
      if (!status.flowId || token !== status.flowId || !callbackUrl) return;
      if (await run(flow, () => call(context, 'provider.auth.complete', { flowId: status.flowId, callbackUrl }))) flow.cleared += 1;
      return;
    }
    case 'setup:codex-browser':
      if (status.url) await openExternal(context.module, status.url);
      return;
    case 'setup:codex-mode':
      await changeMode(context, value === 'managed' ? 'managed' : 'existing');
      // "Continue with ChatGPT" on the existing card asks the managed setup to start on its own.
      if (value === 'managed') { flow.autoStart = true; flow.autoStartHandled = false; }
      return;
    case 'setup:codex-start':
      // A created account starts its setup once its setup appears (AddCodexAccountDialog's renderSetup autoStart).
      flow.autoStart = true; flow.autoStartHandled = false; flow.displayName = value;
      return;
    default: throw new ClientError(`Unknown action: ${op}`);
  }
}

/**
 * The effects, after every refresh, for each instance whose setup a row shows. Instances no
 * row shows let their listener go (the reference's unmount).
 */
export async function codexSetupPrepare(contexts: CodexContext[], now = 0): Promise<void> {
  for (const context of contexts) {
    const flow = codexFlows(context.host).get(context.instanceId);
    if (!flow) continue;
    const watched = watchedSetup(context.host).has(context.instanceId);
    if (!watched) { flow.receiving = ''; continue; }
    await codexEffects(context, flow, now);
  }
}

async function codexEffects(context: CodexContext, flow: CodexFlow, now: number): Promise<void> {
  let status = statusOf(context);
  // Auth receipts and provider snapshots arrive apart: stay "Finishing sign-in..." until the snapshot is authenticated.
  if (status.authenticated || status.phase === 'failed' || status.phase === 'cancelled') flow.awaiting = '';
  if (flow.awaiting && status.phase === 'succeeded' && flow.refreshedFlow !== status.flowId) {
    flow.refreshedFlow = status.flowId;
    await context.host.rpc(context.native, 'server.refreshProviders', { instanceId: context.instanceId }).catch(error => { if (letGo(error)) throw error; });
  }
  // The handoff: import the finished profile once per attempt; a failure ends it.
  const handoff = flow.handoff, handoffState = obj(handoff?.state);
  if (handoff && handoffState.phase === 'finished' && flow.importedAttempt !== handoff.attemptId) {
    flow.importedAttempt = handoff.attemptId;
    await transferProfile(context, obj(handoffState.profile));
  } else if (handoff && (handoff.error || (handoffState.phase === 'auth' && ['failed', 'cancelled'].includes(str(obj(handoffState.state).phase))))) {
    flow.error = handoffState.phase === 'auth' ? (str(obj(handoffState.state).message) || 'ChatGPT sign-in could not finish. Try again.') : handoff.error;
    await dropHandoff(context, flow);
  }
  status = statusOf(context);
  if (flow.autoStart && !flow.autoStartHandled && !status.unavailable && !status.busy && status.installation !== null && status.setup?.canInstall === true) {
    flow.autoStartHandled = true; flow.autoStart = false;
    await setup(context);
    status = statusOf(context);
  }
  if (flow.continueWithSignIn && !flow.pending && !status.installActive) {
    if (status.installPhase === 'failed' || status.installPhase === 'cancelled') flow.continueWithSignIn = '';
    else if (status.installed) { const methodId = flow.continueWithSignIn; flow.continueWithSignIn = ''; await signIn(context, methodId); status = statusOf(context); }
  }
  // The receiver belongs to one authorization URL: a new one (or none) lets it go, which the reference's waiting
  // receive answers with its rejection (cancelProviderAuthCallback), and so does its five minutes' bound (`now`, the
  // snapshot's clock; 0 has none).
  if (flow.receiving && flow.receiving !== status.url) { flow.receiving = ''; flow.error = RECEIVE_FAILED; }
  if (flow.receiving && now > 0) {
    if (!flow.receivingSince) flow.receivingSince = now;
    else if (now - flow.receivingSince >= RECEIVE_MS) { flow.receiving = ''; flow.error = RECEIVE_FAILED; }
  }
  status = statusOf(context);
  if (flow.openRequested && status.phase === 'waiting' && status.flowId && status.url && flow.openedFlow !== status.flowId) {
    flow.openedFlow = status.flowId; flow.openRequested = false;
    await openPage(context, status.url);
  }
}

export { readCodexSetupMode };
