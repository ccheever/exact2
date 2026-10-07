// The first-run gate and the "Set up T3 Code" wizard (lane "pages"): which
// computers to set up, each one's agents, and the projects to import from
// Claude Code and Codex history. With a primary environment (the embedded
// server, local-primary.ts) the gate decides as the desktop's FirstRunGate:
// resolveFirstRunDecision over the primary's live shell, its config and its
// welcome (first-run.ts, local-lifecycle.ts), and the wizard preselects "This
// machine" (listed first). With the Local environment switched off, or no
// primary at all (a refused development build), it decides as the hosted client
// does (resolveHostedFirstRunDecision): no saved environment, the wizard.
// Sources: T3 Code (MIT, see LICENSE-T3)
// apps/web/src/components/onboarding/{FirstRunGate,WelcomeWizard}.tsx,
// onboarding/{firstRun.logic,providerReadiness.logic,projectImport.logic}.ts,
// components/settings/{providerStatus,CodexSetupSection.logic}.ts.
import { arr, obj, str, num, applyShell, type Obj } from './domain';
import { ClientError, bridgeReply, type Native } from './protocol';
import { environmentSources } from './connections';
import { fleet, EnvironmentFleet } from './settings-b-fleet';
import { look } from './settings-appearance-look';
import { pagesPrefs } from './pages-prefs';
import { pushToast } from './toast';
import { wallEpoch, wallIso } from './r8-pointer-clock';
import type { T3Client } from './client';
import { OnboardingTerminal, resolveOnboardingProviderInstallCommand, resolveOnboardingProviderLoginCommand } from './onboarding-terminal';
import { letGo } from './let-go';
import { isFirstRunWorkspaceProvenanceAuthoritative, isFreshFirstRunWorkspace, resolveFirstRunDecision, resolveHostedFirstRunDecision } from './first-run';
import { focusedOnPrimary, isPrimaryEnvironment, primary, withoutPrimaryDuplicates } from './local-primary';
import { primaryWelcome } from './local-lifecycle';
import type { Shell } from './domain';
const terminals = new WeakMap<T3Client, Map<string, OnboardingTerminal>>();
function setupTerminal(client: T3Client, native: Native, environmentId: string): OnboardingTerminal {
  let byEnvironment = terminals.get(client);
  if (!byEnvironment) terminals.set(client, byEnvironment = new Map());
  let terminal = byEnvironment.get(environmentId);
  if (!terminal) {
    terminal = new OnboardingTerminal(async (target, method, input) => {
      if (target === client.environmentId) return client.rpc(native, method, input, true);
      const entry = [...fleet.entries.values()].find(item => item.environmentId === target);
      if (!entry || entry.phase !== 'connected') throw new ClientError('The environment is not connected.');
      const generation = entry.generation;
      const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { op: 'request', method, payload: input, generation });
      if (!reply.ok) throw new ClientError(reply.error?.message || 'The terminal request failed.');
      if (generation !== entry.generation || reply.generation !== generation) throw new ClientError('The connection changed.');
      return reply.value;
    });
    byEnvironment.set(environmentId, terminal);
  }
  return terminal;
}
async function closeSetupTerminals(client: T3Client): Promise<void> {
  for (const terminal of terminals.get(client)?.values() ?? []) if (terminal.state.kind !== 'closed') await terminal.close();
}

type Decision = 'pending' | 'wizard' | 'app';
type Candidate = { key: string; path: string; title: string; projectId: string; sources: string[]; threadCount: number; lastActiveAt: string; alreadyImported: boolean; git: boolean };
type Scan = { environmentId: string; loading: boolean; error: string; candidates: Candidate[]; truncated: boolean };
// Data sources have no wall clock: `now` is the last time the view was handed (LLP 1027.000).
type State = { decision: Decision; selection: Set<string> | null; auto: Set<string>; setup: string[]; scan: Scan | null; picked: Set<string> | null; importing: boolean; now: number; copied: number; seen: Set<string> };
const states = new WeakMap<object, State>();
const stateOf = (client: object): State => {
  let state = states.get(client);
  if (!state) states.set(client, state = { decision: 'pending', selection: null, auto: new Set(), setup: [], scan: null, picked: null, importing: false, now: 0, copied: 0, seen: new Set() });
  return state;
};

/** resolveHostedFirstRunDecision (first-run.ts): no saved environment and setup never finished → the wizard. */
export function hostedDecision(input: { hydrated: boolean; completed: boolean; catalogReady: boolean; environmentCount: number; localEnvironmentDisabled?: boolean }): { decision: Decision; persistCompletion: boolean } {
  return resolveHostedFirstRunDecision(input);
}

type Workspace = { environmentId: string; shell: Shell; live: boolean; config: Obj };
/** Every environment's workspace evidence: the focused connection and each background one. */
function workspaces(client: T3Client): Workspace[] {
  const out: Workspace[] = [];
  if (client.environmentId && client.connection === 'connected') out.push({ environmentId: client.environmentId, shell: client.shell, live: client.ready, config: client.config });
  for (const entry of fleet.entries.values()) {
    if (out.some(workspace => workspace.environmentId === entry.environmentId)) continue;
    out.push({ environmentId: entry.environmentId, shell: entry.shell, live: entry.phase === 'connected' && entry.synchronized === entry.generation, config: entry.config });
  }
  return out;
}
/**
 * FirstRunGate for the desktop's primary: resolveFirstRunDecision over the primary's shell, config
 * and welcome. `bootstrapped` waits for every environment's shell (useAllEnvironmentShellsBootstrapped).
 */
export function primaryDecision(client: T3Client, input: { hydrated: boolean; completed: boolean; catalogReady: boolean; saved: Obj[] }): { decision: Decision; persistCompletion: boolean } {
  const all = workspaces(client);
  const local = all.find(workspace => isPrimaryEnvironment(workspace.environmentId)) ?? (focusedOnPrimary(client) && client.connection === 'connected' ? all[0] : undefined);
  const welcome = primaryWelcome();
  const projects = all.flatMap(workspace => workspace.shell.projects.map(project => ({ id: str(project.id), environmentId: workspace.environmentId, workspaceRoot: str(project.workspaceRoot) })));
  const threads = all.flatMap(workspace => workspace.shell.threads.map(thread => ({ id: str(thread.id), projectId: str(thread.projectId), environmentId: workspace.environmentId,
    latestRun: thread.latestRun ?? null, latestUserMessageAt: typeof thread.latestUserMessageAt === 'string' ? thread.latestUserMessageAt : null, runtime: thread.runtime ?? null })));
  const evidence = new Set([...projects.map(project => project.environmentId), ...threads.map(thread => thread.environmentId)]);
  const workspaceFresh = isFreshFirstRunWorkspace({
    primaryEnvironmentId: local?.environmentId ?? null, serverCwd: local && str(local.config.cwd) ? str(local.config.cwd) : null,
    bootstrapProjectId: welcome ? str(welcome.bootstrapProjectId) || undefined : undefined, bootstrapThreadId: welcome ? str(welcome.bootstrapThreadId) || undefined : undefined,
    bootstrapProjectCreated: welcome?.bootstrapProjectCreated === true ? true : undefined, bootstrapThreadCreated: welcome?.bootstrapThreadCreated === true ? true : undefined,
    projects, threads,
  });
  return resolveFirstRunDecision({
    enabled: true, hydrated: input.hydrated, completed: input.completed,
    // allEnvironmentShellsBootstrappedAtom: every switched-on environment has its shell, or has stopped trying for now.
    bootstrapped: !!local?.live && input.saved.filter(entry => entry.enabled !== false).every(entry => {
      const id = str(entry.environmentId);
      if (id === client.environmentId && client.connection !== 'disconnected') return client.ready || ['error', 'reconnecting'].includes(client.connection);
      const live = [...fleet.entries.values()].find(candidate => candidate.environmentId === id);
      return !!live && ((live.phase === 'connected' && live.synchronized === live.generation) || ['error', 'unsupported', 'reconnecting'].includes(live.phase));
    }),
    authoritative: !!local?.live,
    workspaceAuthoritative: all.filter(workspace => evidence.has(workspace.environmentId)).every(workspace => workspace.live),
    workspaceProvenanceAuthoritative: isFirstRunWorkspaceProvenanceAuthoritative({ welcomeReceived: welcome !== null,
      bootstrapStatus: welcome?.bootstrapStatus === 'pending' || welcome?.bootstrapStatus === 'complete' ? welcome.bootstrapStatus : null }),
    catalogReady: input.catalogReady, serverConfigAvailable: !!local && Object.keys(local.config).length > 0,
    workspaceFresh, projectCount: projects.length, threadCount: threads.length,
  });
}
/** transitionFirstRunGateState: a wizard stays until it finishes; an app never falls back to pending. */
export function transition(current: Decision, next: Decision): Decision {
  if (current === 'wizard' || next === 'pending' || (current === 'app' && next !== 'wizard')) return current;
  return next;
}

// ── Agents (providerReadiness.logic.ts, providerStatus.ts) ─────────────────

export function providerState(provider: Obj | undefined): string {
  if (!provider) return 'checking';
  const auth = obj(provider.auth);
  if (provider.enabled === false || provider.status === 'disabled') return 'disabled';
  if (provider.installed !== true && provider.status === 'warning' && auth.status === 'unknown') return 'checking';
  if (provider.installed !== true) return 'install';
  if (auth.status === 'unauthenticated') return 'signIn';
  if (provider.status === 'ready') return 'ready';
  return 'attention';
}
const PRIORITY: Record<string, number> = { checking: 0, disabled: 1, install: 2, attention: 3, signIn: 4, ready: 5 };
export function providerSummary(provider: Obj | undefined): { headline: string; detail: string } {
  if (!provider) return { headline: 'Checking provider status', detail: 'Waiting for the server to report installation and authentication details.' };
  const auth = obj(provider.auth), message = str(provider.message), label = str(auth.label) || str(auth.type);
  if (provider.enabled === false || provider.status === 'disabled') return { headline: 'Disabled', detail: message || 'This provider is installed but disabled for new sessions in T3 Code.' };
  if (provider.installed !== true) return { headline: 'Not found', detail: message || 'CLI not detected on PATH.' };
  if (auth.status === 'unauthenticated') return { headline: label ? `Not authenticated · ${label}` : 'Not authenticated', detail: message };
  if (provider.status === 'warning') return { headline: 'Needs attention', detail: message || 'The provider is installed, but the server could not fully verify it.' };
  if (provider.status === 'error') return { headline: 'Unavailable', detail: message || 'The provider failed its startup checks.' };
  if (auth.status === 'authenticated') return { headline: label ? `Authenticated · ${label}` : 'Authenticated', detail: message };
  return { headline: 'Available', detail: message };
}
export type AgentRow = { key: string; driver: string; kind: string; instanceId: string; name: string; summary: string; state: string; badge: string; terminalOpen: boolean; terminalAvailable: boolean };
/** ConnectedAgentsStep: every Codex instance (setup rows until one is pointed at an existing CLI), then Claude. */
export function agentRows(config: Obj): AgentRow[] {
  const providers = arr(config.providers), settings = obj(config.settings);
  const best = new Map<string, Obj>();
  for (const provider of providers) {
    const current = best.get(str(provider.driver));
    if (!current || PRIORITY[providerState(provider)]! > PRIORITY[providerState(current)]!) best.set(str(provider.driver), provider);
  }
  const card = (driver: string, provider: Obj | undefined, key: string): AgentRow => {
    const state = providerState(provider), summary = providerSummary(provider);
    const name = str(provider?.displayName) || (driver === 'claudeAgent' ? 'Claude Code' : 'Codex');
    return { terminalOpen: false, terminalAvailable: false, key, driver, kind: 'card', instanceId: str(provider?.instanceId), name, state,
      summary: state === 'ready' ? 'Ready to code.' : `${summary.headline}${summary.detail ? ` · ${summary.detail}` : ''}`,
      badge: state === 'ready' ? 'Ready' : state === 'checking' ? 'Checking...' : state === 'disabled' ? 'Disabled' : state === 'attention' ? summary.headline : state === 'signIn' ? 'Sign in' : 'Install' };
  };
  const codex = providers.filter(provider => provider.driver === 'codex');
  const rows: AgentRow[] = (codex.length ? codex : [best.get('codex')]).map((provider, index) => {
    const instanceId = str(provider?.instanceId, 'codex');
    const instance = obj(obj(settings.providerInstances)[instanceId]);
    const instanceConfig = Object.keys(instance).length ? obj(instance.config) : obj(obj(settings.providers).codex);
    // OnboardingCodexSetup: only an explicit "existing" setup shows the plain readiness card.
    if (instanceConfig.setupMode === 'existing') return card('codex', provider, `codex:${instanceId}:${index}`);
    return { terminalOpen: false, terminalAvailable: false, key: `codex:${instanceId}:${index}`, driver: 'codex', kind: 'codex-setup', instanceId, name: 'Codex', state: providerState(provider), summary: 'Code with your ChatGPT subscription.', badge: '' };
  });
  rows.push(card('claudeAgent', best.get('claudeAgent'), 'claudeAgent'));
  return rows;
}

// ── Projects (projectImport.logic.ts) ──────────────────────────────────────

const RECENT_WINDOW = 30 * 24 * 60 * 60 * 1000;
/** partitionOnboardingProjects: every candidate is offered; recent busy repositories start selected. */
export function recentCandidates(candidates: Candidate[], now: number): Set<string> {
  return new Set(candidates.filter(candidate => candidate.git && candidate.threadCount >= 3 && candidate.lastActiveAt
    && Date.parse(candidate.lastActiveAt) >= now - RECENT_WINDOW && Date.parse(candidate.lastActiveAt) <= now).map(candidate => candidate.key));
}
export function decodeCandidates(environmentId: string, value: unknown): Candidate[] {
  return arr(obj(value).candidates).filter(candidate => str(candidate.path) && str(candidate.title)).map(candidate => ({
    key: JSON.stringify([environmentId, str(candidate.path)]), path: str(candidate.path), title: str(candidate.title), projectId: str(candidate.projectId),
    sources: (Array.isArray(candidate.sources) ? candidate.sources : []).filter((source): source is string => typeof source === 'string'),
    threadCount: num(candidate.threadCount), lastActiveAt: str(candidate.lastActiveAt), alreadyImported: candidate.alreadyImported === true,
    // Servers before the git scan omit `git`: treat their candidates as repositories.
    git: !('git' in candidate) || candidate.git !== null,
  }));
}

// ── The resource ───────────────────────────────────────────────────────────

const relative = (iso: string, now: number) => {
  const time = Date.parse(iso);
  if (!Number.isFinite(time)) return '';
  const minutes = Math.floor(Math.max(0, now - time) / 60_000);
  return minutes < 60 ? `${minutes}m ago` : minutes < 1440 ? `${Math.floor(minutes / 60)}h ago` : `${Math.floor(minutes / 1440)}d ago`;
};

export async function welcomeView(client: T3Client, native: Native | null | undefined, input: { step: string; now: number }) {
  const state = stateOf(client);
  // r13-store: `state.now` only ever holds an instant. Before exactTime resolves the window's time counts from
  // zero (1970) and stands for none, so it is not taken (the last known instant stays).
  const instant = input.now > 0 ? wallEpoch(input.now) : null;
  if (instant !== null) state.now = instant;
  const view = {
    show: false, step: input.step === 'agents' || input.step === 'import' ? input.step : 'connect',
    computers: [] as { key: string; environmentId: string; label: string; url: string; status: string; selected: boolean }[],
    ready: false, machines: [] as { key: string; label: string; agents: AgentRow[]; chatgpt: boolean; terminal: ReturnType<OnboardingTerminal['view']> & { font: string; fontSize: number; light: string; dark: string } }[],
    scanning: false, scanError: '', candidates: [] as { key: string; title: string; path: string; detail: string; selected: boolean }[],
    candidateCount: 0, selectedCount: 0, selectionLabel: '', truncated: false, importLabel: 'Import 0 projects', importing: false, emptyScan: false,
    commandCopied: state.copied,
  };
  if (!native?.available) return view;
  let saved: Obj[] = [], catalogReady = false;
  try {
    const reply = await bridgeReply(native, { op: 'environments' });
    catalogReady = reply.ok;
    saved = reply.ok ? withoutPrimaryDuplicates(arr(obj(reply.value).saved)).filter(entry => str(entry.origin) && str(entry.environmentId)) : [];
  } catch { /* the catalog stays unknown */ }
  const prefs = pagesPrefs(client);
  // r4-polish: the settings store is hydrated once the saved preferences are read (useClientSettingsHydrationStatus);
  // before that a relaunch would see no completion and lock into the wizard.
  const hydrated = (client as { preferencesLoaded?: boolean }).preferencesLoaded !== false;
  // The desktop has a primary unless its Local environment is off or no server can run here (hosted rules then).
  const hosted = primary.disabled || primary.unavailable;
  const next = hosted ? hostedDecision({ hydrated, completed: !!prefs.onboardingCompletedAt, catalogReady, environmentCount: saved.length, localEnvironmentDisabled: primary.disabled })
    : primaryDecision(client, { hydrated, completed: !!prefs.onboardingCompletedAt, catalogReady, saved });
  // Persisted only with a known instant (FirstRunGate stores `new Date().toISOString()`): with none yet it waits for
  // the next ask, which exactTime's arrival triggers, instead of writing 1970.
  if (next.persistCompletion && !prefs.onboardingCompletedAt) prefs.onboardingCompletedAt = wallIso(state.now);
  state.decision = transition(state.decision, next.decision);
  view.show = state.decision === 'wizard';
  if (!view.show || view.step !== 'agents') await closeSetupTerminals(client);
  if (!view.show) return view;
  // A failed pairing registers nothing (PairingForm): the link this client tried
  // and never reached stays out of the list rather than "Connecting…" forever.
  // The primary leads the list ("This machine", preselected); a pairing still connecting is listed once it connected.
  const sources = environmentSources(client, saved, fleet.entries).filter(source => {
    if (source.phase === 'connected') state.seen.add(source.key);
    return source.primary || !source.focused || state.seen.has(source.key) || saved.some(entry => str(entry.environmentId) === source.environmentId && str(entry.origin) === source.origin);
  });
  for (const source of sources) if (!state.auto.has(source.key)) {
    state.auto.add(source.key);
    if (state.selection) state.selection.add(source.key);
  }
  const selected = state.selection ?? new Set(sources.map(source => source.key));
  view.computers = sources.map(source => ({ key: source.key, environmentId: source.environmentId, label: source.label, url: `${source.origin.replace(/\/+$/, '')}/`,
    status: source.phase === 'connected' ? 'Connected' : 'Connecting…', selected: selected.has(source.key) }));
  view.ready = view.computers.some(computer => computer.selected) && view.computers.filter(computer => computer.selected).every(computer => computer.status === 'Connected');
  if (view.step === 'connect') return view;
  const setup = sources.filter(source => state.setup.includes(source.key));
  view.machines = setup.map(source => {
    const agents = Object.keys(source.config).length ? agentRows(source.config) : [];
    const terminal = setupTerminal(client, native, source.environmentId).view(), appearance = look(client);
    const active = terminal.environmentId === source.environmentId;
    for (const agent of agents) { agent.terminalOpen = active && terminal.open && agent.driver === terminal.driver; agent.terminalAvailable = !!str(source.config.cwd); }
    return { key: source.key, label: source.label || 'Computer', agents, terminal: { ...terminal, open: active && terminal.open, font: appearance.terminalFont, fontSize: appearance.terminalSize, light: appearance.terminalLight, dark: appearance.terminalDark }, chatgpt: arr(source.config.providers).some(provider => provider.driver === 'codex' && providerState(provider) === 'ready') };
  });
  if (view.step !== 'import') return view;
  // useProjectScans: this client reads the focused environment's history.
  const focused = setup.find(source => source.focused);
  // The scan is recorded only once it answers: an answer the runtime abandons
  // mid-request must not leave a scan that never finishes (the view shows the
  // search while this step's answer is still out).
  if (!state.scan || state.scan.environmentId !== (focused?.environmentId ?? '')) {
    const scan: Scan = { environmentId: focused?.environmentId ?? '', loading: false, error: focused ? '' : 'Connect to this computer to look for its projects.', candidates: [], truncated: false };
    if (focused) {
      try {
        const result = await client.rpc(native, 'agentSessions.scan', {});
        scan.candidates = decodeCandidates(focused.environmentId, result);
        scan.truncated = result.truncated === true;
      } catch (error) { if (letGo(error)) throw error; scan.error = error instanceof Error ? error.message : 'The scan failed.'; }
    }
    state.scan = scan;
    state.picked = null;
  }
  const scan = state.scan;
  const picked = state.picked ?? recentCandidates(scan.candidates, state.now);
  view.scanning = scan.loading; view.scanError = scan.error; view.truncated = scan.truncated;
  view.candidates = scan.candidates.map(candidate => ({ key: candidate.key, title: candidate.title, path: candidate.path, selected: picked.has(candidate.key),
    detail: [`${candidate.threadCount} ${candidate.threadCount === 1 ? 'thread' : 'threads'}`, relative(candidate.lastActiveAt, state.now), candidate.alreadyImported ? 'Imported' : ''].filter(Boolean).join(' · ') }));
  view.candidateCount = view.candidates.length;
  view.selectedCount = view.candidates.filter(candidate => candidate.selected).length;
  view.selectionLabel = `${view.selectedCount} of ${view.candidateCount} selected`;
  view.emptyScan = !scan.loading && !scan.error && scan.candidates.length === 0;
  view.importing = state.importing;
  view.importLabel = state.importing ? 'Importing…' : `Import ${view.selectedCount} ${view.selectedCount === 1 ? 'project' : 'projects'}`;
  return view;
}
export type WelcomeView = Awaited<ReturnType<typeof welcomeView>>;

// ── Commands (pageslocal:welcome-*) ────────────────────────────────────────

/** The wizard's own actions. Returns the project to open a new thread in, when finishing lands in one. */
export async function welcomeLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  const state = stateOf(client);
  if (op.startsWith('terminal-')) {
    const terminal = setupTerminal(client, native, id);
    if (op === 'terminal-exited') {
      let event: Obj = {}; try { event = obj(JSON.parse(value)); } catch { return ''; }
      if (terminal.state.kind !== 'closed' && str(event.terminalId) === terminal.state.session.terminalId && (event.type === 'exited' || event.type === 'closed')) await terminal.close();
      return '';
    }
    if (op === 'terminal-close') { await terminal.close(); return ''; }
    if (op === 'terminal-retry') { if (terminal.state.kind !== 'closed') await terminal.open(terminal.state.session); return ''; }
    const source = environmentSources(client, fleet.saved, fleet.entries).find(item => item.key === id);
    if (!source) throw new ClientError('The environment is not connected.');
    const agent = agentRows(source.config).find(item => item.key === value);
    if (!agent || (agent.driver !== 'claudeAgent' && agent.driver !== 'codex')) return '';
    const provider = arr(source.config.providers).find(item => item.instanceId === agent.instanceId) ?? { driver: agent.driver, instanceId: agent.instanceId };
    const platform = str(obj(obj(source.config.environment).platform).os);
    const command = agent.state === 'signIn' ? resolveOnboardingProviderLoginCommand(provider, obj(source.config.settings), platform) : resolveOnboardingProviderInstallCommand(agent.driver, platform);
    const ids = await client.ids(native, 1);
    await setupTerminal(client, native, source.environmentId).open({ environmentId: source.environmentId, terminalId: `onboarding-${agent.driver}-${ids[0]}`, driver: agent.driver, providerInstanceId: agent.instanceId, cwd: str(source.config.cwd), command });
    return '';
  }
  // CommandBlock: copy without a toast; the button shows Check for 1.5s.
  if (op === 'copy-command') {
    const reply = await bridgeReply(native, { op: 'copyText', text: value });
    if (!reply.ok) throw new ClientError(reply.error?.message || 'Could not copy.');
    state.copied++;
    return '';
  }
  if (op === 'select') {
    const current = state.selection ?? new Set([...state.auto]);
    if (current.has(id)) current.delete(id); else current.add(id);
    state.selection = current;
    return '';
  }
  // startSetup: the chosen computers, in the order the list shows them.
  if (op === 'setup') { state.setup = [...(state.selection ?? state.auto)]; state.scan = null; state.picked = null; return ''; }
  if (op === 'pick' || op === 'pick-all' || op === 'pick-none') {
    const candidates = state.scan?.candidates ?? [];
    const current = new Set(state.picked ?? recentCandidates(candidates, state.now));
    if (op === 'pick') { if (current.has(id)) current.delete(id); else current.add(id); }
    state.picked = op === 'pick-all' ? new Set(candidates.map(candidate => candidate.key)) : op === 'pick-none' ? new Set() : current;
    return '';
  }
  if (op === 'codex-mode') {
    // OnboardingCodexSetup.changeMode: point the instance at an existing CLI or at the managed ChatGPT runtime.
    const settings = obj(client.config.settings), instance = obj(obj(settings.providerInstances)[id]);
    const config = Object.keys(instance).length ? obj(instance.config) : obj(obj(settings.providers).codex);
    const patch = { providerInstances: { [id]: { ...instance, driver: 'codex', enabled: true, config: { ...config, enabled: true, setupMode: value === 'managed' ? 'managed' : 'existing' } } } };
    await client.rpc(native, 'server.updateSettings', { patch }, true);
    return '';
  }
  // The command carries the window's wall time (app.contract: wallTime.epochAtZero + now()). A launch-relative
  // count (the old `now()` value) is not an instant and never replaces the last one: it made 1970-01-01T00:01:37Z.
  if (op === 'finish' || op === 'import') { const instant = Number(value) > 0 ? wallEpoch(Number(value)) : null; if (instant !== null) state.now = instant; }
  if (op === 'finish') return finish(client, '');
  if (op === 'import') return importProjects(client, native);
  throw new ClientError(`Unknown setup action: ${op}`);
}

function finish(client: T3Client, landing: string): string {
  const state = stateOf(client);
  // useCompleteOnboarding: the completion time is the wall time of the finish. Unknown (the host has not told the
  // date): nothing is stored, rather than 1970; welcomeView records it on the first ask that knows the date.
  const stamp = wallIso(state.now);
  if (stamp) pagesPrefs(client).onboardingCompletedAt = stamp;
  state.decision = 'app';
  // WelcomeRouteView.onDone: a new thread in the imported project, else the index landing.
  if (landing && client.shell.projects.some(project => project.id === landing) && !client.threadId) {
    client.projectId = landing;
    (client as unknown as { chooseDefaults?: () => void }).chooseDefaults?.();
  }
  return '';
}

/** ImportStep.runImport: create each missing project, then import its agent history. */
export async function importProjects(client: T3Client, native: Native): Promise<string> {
  const state = stateOf(client);
  const candidates = (state.scan?.candidates ?? []).filter(candidate => (state.picked ?? recentCandidates(state.scan?.candidates ?? [], state.now)).has(candidate.key));
  if (!candidates.length) return finish(client, '');
  state.importing = true;
  let imported = 0, skipped = 0, completed = 0, landing = '', fallback = '';
  try {
    for (const candidate of candidates) {
      let projectId = candidate.projectId || str(client.shell.projects.find(project => str(project.workspaceRoot).replace(/\/+$/, '') === candidate.path.replace(/\/+$/, ''))?.id);
      if (!projectId) {
        const ids = await bridgeReply(native, { op: 'ids', count: 2 });
        const [commandId, created] = (Array.isArray(ids.value) ? ids.value : []).map(value => str(value));
        if (!ids.ok || !commandId || !created) continue;
        try {
          await client.rpc(native, 'projects.mutate', { type: 'project.create', commandId: `onboarding:project:create:${created}`, projectId: created, title: candidate.title, workspaceRoot: candidate.path, createWorkspaceRootIfMissing: false, defaultModelSelection: null }, true);
          projectId = created;
        } catch { continue; }
      }
      try {
        const result = await client.rpc(native, 'agentSessions.import', { projectId, expectedWorkspaceRoot: candidate.path }, true);
        imported += num(result.importedCount); skipped += num(result.skippedCount);
        if (num(result.importedCount) > 0 && !landing) landing = projectId;
        if (num(result.skippedCount) === 0) { completed += 1; if (!fallback) fallback = projectId; }
      } catch { /* counted below as not imported */ }
    }
    client.shell = applyShell(client.shell, await client.restAccess(native).http('/api/orchestration/shell'));
  } finally { state.importing = false; }
  if (completed < candidates.length) {
    const warning = imported > 0 && skipped > 0 ? `Imported ${imported} ${imported === 1 ? 'thread' : 'threads'}. ${skipped} ${skipped === 1 ? 'thread' : 'threads'} could not be imported.`
      : skipped > 0 ? `${skipped} ${skipped === 1 ? 'thread could' : 'threads could'} not be imported.`
        : imported > 0 ? `Imported ${imported} ${imported === 1 ? 'thread' : 'threads'}. Some thread history could not be imported.` : 'Could not import thread history.';
    pushToast(client, { kind: 'warning', title: 'Some history was not imported', description: warning, timeoutMs: 0 });
  } else if (imported > 0) pushToast(client, { kind: 'success', title: `Imported ${imported} ${imported === 1 ? 'thread' : 'threads'}` });
  return finish(client, landing || fallback);
}

/** Whether the wizard is over the app (the index shows NoProjectsHero beneath it). */
export function welcomeShowing(client: object): boolean {
  return stateOf(client).decision === 'wizard';
}
