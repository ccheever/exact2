// Managed Codex with a ChatGPT account, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/settings/CodexSetupSection.tsx (CodexSetupSection :58-156, the
// state and derivations of ManagedCodexSetup :194-316 and its descriptions, controls and
// alerts :568-1008, CodexSignInDescription :1082-1107), CodexSetupSection.logic.ts
// (readCodexSetupMode) and onboarding/providerReadiness.logic.ts (getOnboardingProviderState).
// Changes: React state becomes one CodexFlow per (connection, instance), kept here; the view is
// one model per presentation for Contract (codex-setup.contract). The commands, the effects
// (refresh after success, continue after install, auto start, open the page once per flow) and
// the native receiver are codex-setup-ops.ts. The hosted-web pieces (remoteWeb, handoffUrl,
// providerAuthReturnUrl) are out of scope: this app is a desktop client, so `remoteWeb` is
// always false and the pasted-URL form sits behind "Having trouble signing in?".
import { arr, num, obj, str, type Obj } from './domain';
import { setupOf } from './provider-setup';
import type { ProviderHost } from './providers';
import { redactedValue } from './redacted-text';
import { providerState, providerSummary } from './provider-readiness';

export type CodexHost = ProviderHost & { generation?: number };
export type CodexHandoff = {
  attemptId: string; primaryKey: string; input: Obj;
  subscription: string; state: Obj | null; error: string;
};
/** ManagedCodexSetup's React state and refs, one per instance on one connection. */
export type CodexFlow = {
  pending: boolean; error: string;
  awaiting: '' | 'sign-in' | 'handoff';
  continueWithSignIn: string; openRequested: boolean; openedFlow: string;
  requestedMethodId: string;
  autoStart: boolean; autoStartHandled: boolean; displayName: string;
  /** The authorization URL the loopback listener holds (receivingCallback), and its flow. */
  receiving: string; receivingFlow: string;
  /** The flow whose success already asked for a provider refresh. */
  refreshedFlow: string;
  /** Bumped when a pasted redirect URL connected, so the field starts empty. */
  cleared: number;
  handoff: CodexHandoff | null; transferFailed: boolean; importedAttempt: string; handoffSequence: number;
};

const flows = new WeakMap<object, Map<string, CodexFlow>>();
const freshFlow = (): CodexFlow => ({ pending: false, error: '', awaiting: '', continueWithSignIn: '', openRequested: false, openedFlow: '',
  requestedMethodId: 'chatgpt', autoStart: false, autoStartHandled: false, displayName: '', receiving: '', receivingFlow: '', refreshedFlow: '',
  cleared: 0, handoff: null, transferFailed: false, importedAttempt: '', handoffSequence: 0 });
export function codexFlows(host: object): Map<string, CodexFlow> {
  let map = flows.get(host);
  if (!map) flows.set(host, map = new Map());
  return map;
}
export function codexFlow(host: object, instanceId: string): CodexFlow {
  const map = codexFlows(host);
  let flow = map.get(instanceId);
  if (!flow) map.set(instanceId, flow = freshFlow());
  return flow;
}

/** readCodexSetupMode (CodexSetupSection.logic.ts). */
export function readCodexSetupMode(config: unknown): 'managed' | 'existing' {
  return config !== null && typeof config === 'object' && 'setupMode' in config && (config as Obj).setupMode === 'managed' ? 'managed' : 'existing';
}
/** The instance's config as the server stores it: the explicit instance, else the legacy `providers.codex` slot. */
export function codexInstance(settings: Obj, instanceId: string): Obj {
  const instance = obj(obj(settings.providerInstances)[instanceId]);
  if (Object.keys(instance).length) return instance;
  return instanceId === 'codex' ? { driver: 'codex', enabled: obj(obj(settings.providers).codex).enabled !== false, config: obj(obj(settings.providers).codex) } : {};
}

/** Everything ManagedCodexSetup derives from its streams, the provider snapshot and its own state. */
export function codexStatus(host: CodexHost, instanceId: string, provider: Obj | undefined, options: { enabled: boolean; readOnly: boolean }) {
  const entry = setupOf(host, instanceId), flow = codexFlow(host, instanceId);
  const handoffAuth = flow.handoff && obj(flow.handoff.state).phase === 'auth' ? obj(obj(flow.handoff.state).state) : null;
  const auth = handoffAuth ?? entry.auth.state, installation = entry.install.state;
  const methods = arr(auth?.methods);
  const phase = str(auth?.phase), installPhase = str(installation?.phase);
  const interaction = obj(auth?.interaction);
  const url = interaction.type === 'browser' ? str(interaction.url) : str(auth?.authorizationUrl);
  const installed = !!installation && installation.installedVersion != null;
  const authenticated = obj(provider?.auth).status === 'authenticated';
  const updateAvailable = installed && installation!.source !== 'local' && installation!.version != null && installation!.version !== installation!.installedVersion;
  const finishingSignIn = flow.awaiting !== '' && !authenticated && (phase === 'succeeded' || flow.awaiting === 'handoff');
  const startingAutomatically = flow.autoStart && !flow.autoStartHandled;
  const waitingForAuthState = flow.awaiting === 'sign-in' && !authenticated && (auth === null || phase === 'idle');
  const authInProgress = finishingSignIn || waitingForAuthState || flow.handoff !== null || phase === 'starting' || phase === 'waiting' || phase === 'verifying';
  const installActive = installPhase === 'downloading' || installPhase === 'extracting' || installPhase === 'verifying';
  const authActive = startingAutomatically || authInProgress || (flow.pending && !installActive);
  const queryError = entry.auth.error !== '' || entry.install.error !== '';
  const unavailable = options.readOnly || !options.enabled || flow.pending || queryError;
  const busy = flow.pending || authInProgress || installActive;
  const setup = provider?.setup && typeof provider.setup === 'object' ? obj(provider.setup) : undefined;
  const handoffFinished = obj(flow.handoff?.state).phase === 'finished';
  return {
    entry, flow, auth, installation, methods, phase, installPhase, url, flowId: str(auth?.flowId), installed, authenticated, updateAvailable,
    finishingSignIn, startingAutomatically, waitingForAuthState, authInProgress, installActive, authActive, queryError, unavailable, busy, setup, handoffFinished,
    hasSavedAccount: methods.some(method => str(method.id).startsWith('chatgpt-profile:')),
    reconnectEmail: str(methods.find(method => method.id === 'chatgpt')?.accountEmail),
    requestedAccountEmail: str(methods.find(method => method.id === flow.requestedMethodId)?.accountEmail),
    logoutWarning: phase === 'idle' && str(auth?.message).startsWith('Signed out locally.') ? str(auth?.message) : '',
    // callbackCompletion: a waiting browser flow outside a handoff accepts a pasted redirect URL.
    callback: flow.handoff === null && phase === 'waiting' && url !== '',
    token: `${phase}:${str(auth?.flowId)}|${installPhase}:${str(installation?.operationId)}|${authenticated}|${handoffFinished}`,
  };
}
export type CodexStatus = ReturnType<typeof codexStatus>;

const megabytes = (bytes: number) => (bytes / 1_000_000).toFixed(1);
/** runtimeDescription. */
export function codexRuntimeDescription(status: CodexStatus): string {
  const installation = status.installation;
  if (status.installPhase === 'downloading') return `Downloading ${megabytes(num(installation?.downloadedBytes))}${typeof installation?.totalBytes === 'number' ? ` of ${megabytes(installation.totalBytes)}` : ''} MB.`;
  if (status.installPhase === 'extracting') return 'Installing Codex.';
  if (status.installPhase === 'verifying') return 'Checking Codex.';
  if (status.installed) return `${installation?.source === 'local' ? 'Using your installed Codex' : 'Managed by T3 Code'}${str(installation?.installedVersion) ? ` · v${str(installation?.installedVersion)}` : ''}.`;
  return typeof installation?.message === 'string' ? installation.message : 'T3 Code downloads and manages Codex for you.';
}

/** The view CodexSetupSection draws (codex-setup.contract `CodexSetup`). */
export type CodexSetupView = {
  /** What a command names: the instance, or `<fleet key>\t<instance>` on a background computer (codex-setup-host.ts). */
  target: string;
  key: string; instanceId: string; token: string; presentation: string; title: string;
  /** 'managed', 'existing' (the onboarding card before a mode is chosen) or 'preparing' (no setup yet). */
  kind: string;
  description: string; email: string; emailPlaceholder: string;
  /** Onboarding: 'toggle' (Having trouble signing in?), 'plain' (Complete sign-in in your browser.) or ''. */
  help: string;
  /** 'ready', 'waiting', 'connect', 'checking', 'headline', 'install' (settings: Cancel), 'authenticated' (settings: Change account, Disconnect). */
  control: string; controlLabel: string; controlDisabled: boolean;
  /** 'cancel', 'different', 'existing' or ''. */
  secondary: string; secondaryDisabled: boolean;
  showCancel: boolean; cancelDisabled: boolean; savedAccount: boolean; differentDisabled: boolean; accountDisabled: boolean;
  usage: boolean; logoutWarning: string; callback: boolean; formKey: string; flowId: string; pasteDisabled: boolean; error: string;
};

const emailView = (provider: Obj | undefined) => redactedValue(obj(provider?.auth).email);
const base = (instanceId: string, presentation: string, title: string, token: string, target: string): CodexSetupView => ({
  target, key: `${target}:${presentation}`, instanceId, token, presentation, title, kind: 'managed', description: '', email: '', emailPlaceholder: '', help: '',
  control: '', controlLabel: '', controlDisabled: false, secondary: '', secondaryDisabled: false, showCancel: false, cancelDisabled: false,
  savedAccount: false, differentDisabled: false, accountDisabled: false, usage: false, logoutWarning: '', callback: false, formKey: '', flowId: '', pasteDisabled: false, error: '',
});

/** getOnboardingProviderState (providerReadiness.logic.ts), as pages-welcome.ts ports it. */
export const onboardingProviderState = (provider: Obj | undefined) => providerState(provider);

/**
 * CodexSetupSection for one instance. `mode` is readCodexSetupMode of its config; `busy` is the
 * setup queue's pending command (the reference's `pending` while a command runs).
 */
export function codexSetupView(host: CodexHost, instanceId: string, provider: Obj | undefined, options: {
  presentation: 'settings' | 'onboarding'; mode: 'managed' | 'existing'; enabled: boolean; readOnly: boolean; allowExistingCli: boolean; displayName?: string; busy?: boolean; target?: string;
}): CodexSetupView {
  const status = codexStatus(host, instanceId, provider, options), flow = status.flow;
  const title = options.displayName || flow.displayName || str(provider?.displayName) || 'Codex';
  const view = base(instanceId, options.presentation, title, status.token, options.target ?? instanceId);
  const pending = flow.pending || options.busy === true;
  const email = emailView(provider);
  if (options.mode === 'existing' && options.presentation === 'onboarding') {
    // The existing-CLI card: Ready, Checking, the summary, or Continue with ChatGPT / Use existing CLI.
    const state = onboardingProviderState(provider), ready = options.enabled && state === 'ready', checking = state === 'checking';
    const summary = providerSummary(provider), authenticated = status.authenticated;
    view.kind = 'existing';
    if (ready && email.value) Object.assign(view, { email: email.value, emailPlaceholder: email.placeholder });
    view.description = ready ? (email.value ? '' : 'Connected with your Codex CLI.') : checking ? 'Checking your Codex CLI...' : provider?.installed === true ? summary.headline : 'Code with your ChatGPT subscription.';
    view.control = ready ? 'ready' : checking ? 'checking' : authenticated ? 'headline' : 'connect';
    view.controlLabel = view.control === 'headline' ? summary.headline : 'Continue with ChatGPT';
    view.controlDisabled = options.readOnly;
    view.secondary = !authenticated && !ready && !checking ? 'existing' : '';
    return view;
  }
  if (options.mode === 'existing') { view.kind = 'none'; return view; }
  if (!status.setup) {
    view.kind = 'preparing';
    view.description = options.presentation === 'onboarding' ? 'Complete sign-in in your browser.' : 'Preparing sign-in.';
    return view;
  }
  const cannotStart = status.unavailable || status.busy || (!status.installed ? status.setup.canInstall !== true : status.setup.canAuthenticate !== true);
  const accountDescription = status.finishingSignIn ? 'Finishing sign-in...'
    : status.installActive ? codexRuntimeDescription(status)
      : status.authActive || status.phase === 'failed' || status.phase === 'cancelled'
        ? (status.phase === 'waiting' && status.requestedAccountEmail ? `Continue as ${status.requestedAccountEmail} on OpenAI.` : (typeof status.auth?.message === 'string' ? status.auth.message : 'Finish signing in in your browser.'))
        : status.authenticated ? (email.value ? '' : 'Signed in with ChatGPT.') : (status.reconnectEmail || 'Use your ChatGPT subscription.');
  const waitingLabel = status.finishingSignIn ? 'Finishing sign-in...'
    : status.handoffFinished ? (flow.transferFailed ? 'Retry connection' : 'Finishing sign-in...')
      : status.phase === 'waiting' ? 'Open sign-in page' : options.presentation === 'onboarding' ? 'Open sign-in page' : 'Signing in...';
  const waitingDisabled = status.handoffFinished ? flow.pending || !flow.transferFailed : status.url === '' || status.phase !== 'waiting';
  Object.assign(view, {
    callback: status.callback, formKey: `${status.flowId}:${flow.cleared}`, flowId: status.flowId, pasteDisabled: pending || options.readOnly,
    logoutWarning: status.logoutWarning, savedAccount: status.hasSavedAccount,
  });
  if (options.presentation === 'onboarding') {
    view.description = status.installActive ? codexRuntimeDescription(status)
      : status.authenticated ? (email.value ? '' : 'Connected to ChatGPT.')
        : status.callback ? ''
          : status.startingAutomatically || status.waitingForAuthState || status.phase === 'starting' || status.phase === 'waiting' || (pending && !status.installActive) ? 'Complete sign-in in your browser.'
            : status.authActive || status.phase === 'failed' || status.phase === 'cancelled' ? accountDescription : 'Code with your ChatGPT subscription.';
    view.help = !status.installActive && !status.authenticated ? (status.callback ? 'toggle' : view.description === 'Complete sign-in in your browser.' ? 'plain' : '') : '';
    if (status.authenticated && !status.installActive && email.value) Object.assign(view, { email: email.value, emailPlaceholder: email.placeholder });
    view.control = status.authenticated && !status.busy ? 'ready' : status.authActive ? 'waiting' : 'connect';
    view.controlLabel = view.control === 'waiting' ? waitingLabel : status.installActive || pending ? 'Setting up...' : status.hasSavedAccount ? 'Reconnect account' : 'Continue with ChatGPT';
    view.controlDisabled = view.control === 'waiting' ? waitingDisabled : cannotStart;
    view.secondary = (status.authActive && !status.finishingSignIn) || status.installActive ? 'cancel'
      : !status.authActive && !status.authenticated && status.hasSavedAccount ? 'different'
        : !status.authActive && !status.authenticated && options.allowExistingCli ? 'existing' : '';
    view.secondaryDisabled = view.secondary === 'cancel' ? pending || options.readOnly : view.secondary === 'different' ? status.unavailable || status.busy : options.readOnly || status.busy;
    view.error = status.logoutWarning || flow.error || (status.queryError ? 'Could not read setup status. Reconnect and try again.' : status.installPhase === 'failed' ? str(status.installation?.message) : '');
    return view;
  }
  view.description = accountDescription;
  if (status.authenticated && !accountDescription && email.value) Object.assign(view, { email: email.value, emailPlaceholder: email.placeholder });
  view.control = status.authActive ? 'waiting' : status.installActive ? 'install' : status.authenticated ? 'authenticated' : 'connect';
  view.controlLabel = view.control === 'waiting' ? waitingLabel : pending ? 'Setting up...' : status.hasSavedAccount ? 'Reconnect account' : 'Continue with ChatGPT';
  view.controlDisabled = view.control === 'waiting' ? waitingDisabled : cannotStart;
  view.showCancel = view.control === 'waiting' && !status.finishingSignIn;
  view.cancelDisabled = pending || options.readOnly;
  view.accountDisabled = view.differentDisabled = status.unavailable || status.busy;
  view.usage = status.authenticated;
  view.error = flow.error || (status.installPhase === 'failed' ? str(status.installation?.message) : status.queryError ? 'Could not read Codex setup status. Reconnect and try again.' : '');
  return view;
}

/** CodexManagedRuntimeFields: the read-only Binary path, CODEX_HOME path and Shadow home path rows. */
export function codexRuntimeFields(host: CodexHost, instanceId: string, provider: Obj | undefined) {
  const entry = setupOf(host, instanceId), executablePath = str(entry.install.state?.executablePath), paths = provider?.runtimePaths && typeof provider.runtimePaths === 'object' ? obj(provider.runtimePaths) : null;
  return {
    key: instanceId, binaryPath: executablePath, binaryPlaceholder: entry.install.error ? 'Could not read runtime path' : 'Not installed',
    homePath: str(paths?.homePath), shadowPath: str(paths?.shadowHomePath), shadowPlaceholder: paths ? 'Not used' : 'Unavailable',
    shadowDescription: str(paths?.shadowHomePath) ? 'Account-specific home sharing the Codex state above.' : 'This instance uses the shared Codex home directly.',
  };
}

/** ChatGptAccountPicker: the saved profiles, then "Use a different account". */
export function codexPicker(host: CodexHost, instanceId: string, target = instanceId) {
  const methods = arr(setupOf(host, instanceId).auth.state?.methods).filter(method => str(method.id).startsWith('chatgpt-profile:'));
  const provider = arr(host.config.providers).find(candidate => candidate.instanceId === instanceId);
  const token = codexStatus(host, instanceId, provider, { enabled: provider?.enabled !== false, readOnly: !host.writable }).token;
  return { key: target, instanceId, target, token, profiles: methods.map(method => ({ id: str(method.id), name: str(method.name) })), first: str(methods[0]?.id) || 'chatgpt-change-account' };
}
