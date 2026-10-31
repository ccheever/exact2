// useUpdateEverything.ts, updateEverythingRun.tsx and UpdateEverythingDialog.tsx, T3 Code 6d450a4284 (MIT; see
// LICENSE-T3). Per-window state replaces React atoms; existing transport jobs keep
// server updates alive across focus changes. This app has no desktop update feed, so its target is the bundled
// server's version and its release archives never count as published.
import { obj, str } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { environmentSources } from '../connections/connections';
import { fleet } from '../connections/settings-b-fleet';
import { sessionScopes } from '../connections/local-primary';
import { CLIENT_VERSION } from '../connections/version-skew';
import { awaitServerUpdate, nativeUpdateDeps, serverUpdateStateFor, type ServerUpdateOutcome, type ServerUpdateTarget } from '../connections/server-update';
import { providerUpdateMachines, updateAllProviders, updateTargets, type ProviderUpdateMachine } from '../providers/providers-upkeep';
import { getProviderUpdateRunOutcome, type ProviderUpdateRun } from '../providers/provider-updates';
import { providerDisplayName } from '../providers/provider-updates';
import { pushToast } from '../shell/toast';
import { cachedReleasePublished, npmReleaseKey } from './npm-release';
import { closeFinishedUpdateRun, collectRestartingMachines, collectServerUpdateTargets, collectUpdateEverythingResult, describeUpdateCapsule, describeUpdateEverything,
  formatUpdateElapsed, getUpdateEverythingToastView, resolveUpdateRunMachineStatus, RUN_STATUS_LABELS,
  type UpdateEverythingMachine, type UpdateEverythingRun, type UpdateRunMachine } from './update-everything-logic';

type Plan = ReturnType<typeof updateEverythingPlan>;
type State = { now: number; dialog: Plan | null; run: UpdateEverythingRun | null; details: boolean; skippedOpen: boolean };
const APP_LABELS = { 'this-app': 'This app', server: 'Server', 'desktop-app': 'Desktop app' } as const;
const states = new WeakMap<object, State>();
export function updateEverythingState(client: object): State {
  let state = states.get(client);
  if (!state) states.set(client, state = { now: 0, dialog: null, run: null, details: false, skippedOpen: false });
  return state;
}
export function updateEverythingMachines(client: T3Client): UpdateEverythingMachine[] {
  return environmentSources(client, fleet.saved, fleet.entries).map(source => ({
    environmentId: source.environmentId, key: source.key, label: source.label,
    serverConfig: Object.keys(source.config).length ? source.config : null, connected: source.phase === 'connected',
    hostedByThisApp: source.primary, canMaintain: sessionScopes(source.environmentId, source.scopes).includes('environment:maintain'),
    updateState: serverUpdateStateFor(source.environmentId, str(obj(source.config.environment).serverVersion) || null),
  }));
}
export function updateEverythingPlan(client: T3Client, native: Native | null = null, published = false) {
  const machines = updateEverythingMachines(client), collected = collectServerUpdateTargets(machines, CLIENT_VERSION, false);
  const servers = published ? collected.targets : collected.targets.filter(server => !collected.waitForNpm.includes(server));
  const skipped = [...collected.skipped, ...collected.targets.filter(server => !servers.includes(server))
    .map(server => ({ environmentId: server.environmentId, label: server.serverLabel, reason: `t3@${CLIENT_VERSION} is not on npm yet` }))];
  const providers = providerUpdateMachines(updateTargets(client, native));
  return { servers, providers, skipped, restarts: collectRestartingMachines(machines, servers, false),
    ...describeUpdateEverything({ localVersion: null, localMachines: machines.filter(machine => machine.hostedByThisApp), servers, skipped,
      providerMachines: providers.map(machine => ({
        environmentId: machine.environmentId, label: machine.label, providers: machine.candidates.map(provider => providerDisplayName(str(provider.driver))),
      })) }), hasUpdates: servers.length > 0 || providers.length > 0 };
}
const notify = (native: Native) => { void native.later({ op: 'sidebarNotify' }).catch(() => {}); };
export function updateEverythingPass(client: T3Client, now: number): void {
  updateEverythingState(client).now = now;
}
/** The npmRelease resource's key (useNpmReleasePublished): npm is asked only while a remote npm-managed server is behind. */
export function npmReleaseProbe(client: T3Client, now: number): string {
  return npmReleaseKey(collectServerUpdateTargets(updateEverythingMachines(client), CLIENT_VERSION, false).waitForNpm.length > 0 ? CLIENT_VERSION : null, now);
}
function versionOf(client: T3Client, environmentId: string): string | null {
  const machine = updateEverythingMachines(client).find(machine => machine.environmentId === environmentId);
  return machine?.connected ? str(obj(machine.serverConfig?.environment).serverVersion) || null : null;
}
function machineState(client: T3Client, machine: UpdateRunMachine) {
  return serverUpdateStateFor(machine.environmentId, versionOf(client, machine.environmentId));
}
function settle(state: State, id: string, failure: string | null, serverUpdated = false, providerFailure?: string | null): void {
  if (!state.run) return;
  state.run = closeFinishedUpdateRun({ ...state.run, machines: state.run.machines.map(machine => machine.environmentId !== id ? machine : {
    ...machine, result: failure === null ? 'done' : 'failed', failure, ...(serverUpdated ? { server: null } : {}),
    ...(providerFailure === undefined ? {} : { providerFailure }),
  }) });
}
export type RunDeps = { providers: (machines: readonly ProviderUpdateMachine[]) => Promise<ProviderUpdateRun[]>;
  server: (target: ServerUpdateTarget) => Promise<ServerUpdateOutcome>;
  now: () => number; changed: () => void; toast: (result: ReturnType<typeof collectUpdateEverythingResult>) => void };
/** Providers finish once before servers start. Each server's completion stays independent of the others. */
export async function runEverything(state: State, plan: Pick<Plan, 'providers' | 'servers'>, deps: RunDeps): Promise<void> {
  if (state.run) return;
  const machines = new Map<string, UpdateRunMachine>(), startedAt = deps.now();
  for (const provider of plan.providers) machines.set(provider.environmentId, { environmentId: provider.environmentId, label: provider.label,
    updatesProviders: true, server: null, handedOffVersion: null, startedAt, result: null, failure: null, providerFailure: null });
  for (const server of plan.servers) machines.set(server.environmentId, { environmentId: server.environmentId, label: server.serverLabel,
    updatesProviders: machines.has(server.environmentId), server, handedOffVersion: null, startedAt, result: null, failure: null, providerFailure: null });
  state.run = { localVersion: null, phase: 'providers', restartAt: null, machines: [...machines.values()] };
  deps.changed();
  const runs = await deps.providers(plan.providers), outcomes = runs.map(getProviderUpdateRunOutcome), failures = new Map<string, string>();
  runs.forEach((run, index) => { const outcome = outcomes[index]!; if (outcome.status !== 'updated') failures.set(run.environmentId,
    outcome.status === 'failed' ? outcome.line : 'Provider update interrupted'); });
  for (const machine of machines.values()) if (machine.server === null) settle(state, machine.environmentId, failures.get(machine.environmentId) ?? null);
  if (state.run) state.run = closeFinishedUpdateRun({ ...state.run, phase: 'servers', machines: state.run.machines.map(machine => machine.server ? { ...machine, startedAt: deps.now() } : machine) });
  deps.changed();
  const servers = await Promise.all(plan.servers.map(async server => {
    const outcome = await deps.server(server), providerFailure = failures.get(server.environmentId) ?? null;
    settle(state, server.environmentId, outcome.status === 'failed' ? outcome.message : providerFailure, outcome.status !== 'failed', providerFailure);
    deps.changed();
    return { label: server.serverLabel, outcome };
  }));
  deps.toast(collectUpdateEverythingResult({ localVersion: null, localFailures: [], servers, providers: outcomes }));
}
function nativeRunDeps(client: T3Client, native: Native): RunDeps {
  return { providers: machines => updateAllProviders(client, machines, { quiet: true }),
    server: target => awaitServerUpdate(target, nativeUpdateDeps(native, client), { quiet: true }),
    now: () => updateEverythingState(client).now, changed: () => notify(native),
    toast: result => { const view = getUpdateEverythingToastView(result, CLIENT_VERSION); if (view) pushToast(client, {
      kind: view.type, title: view.title, description: view.description, stacked: true,
    }); } };
}
/** A failed server retries on its own; successful provider installs are never repeated. */
export function retryEverythingServers(state: State, deps: Pick<RunDeps, 'server' | 'now' | 'changed'>): void {
  const failed = state.run?.machines.filter(machine => machine.result === 'failed' && machine.server !== null) ?? [];
  if (!state.run) return;
  const ids = new Set(failed.map(machine => machine.environmentId));
  state.run = { ...state.run, machines: state.run.machines.map(machine => ids.has(machine.environmentId)
    ? { ...machine, result: null, failure: null, startedAt: deps.now() } : machine) };
  deps.changed();
  for (const machine of failed) void deps.server(machine.server!).then(outcome => {
    settle(state, machine.environmentId, outcome.status === 'updated' ? machine.providerFailure
      : outcome.status === 'failed' ? outcome.message : 'Could not start the update', outcome.status === 'updated');
    deps.changed();
  });
}
export function updateEverythingView(client: T3Client, now: number) {
  const state = updateEverythingState(client); state.now = now;
  const published = cachedReleasePublished(CLIENT_VERSION), plan = state.dialog ?? updateEverythingPlan(client, null, published);
  const views = state.run?.machines.map(machine => ({ machine, status: resolveUpdateRunMachineStatus(machine, state.run!.phase, machineState(client, machine)) })) ?? [];
  const capsule = state.run ? describeUpdateCapsule(state.run, views, null, now) : null;
  return { available: plan.hasUpdates, running: state.run !== null, details: state.details,
    label: capsule?.label ?? 'Update', tone: capsule?.tone ?? 'default', elapsed: capsule?.slow?.elapsed ?? '',
    action: capsule?.action ?? '', confirm: state.dialog !== null, title: `${plan.summary}?`,
    body: '', summary: plan.summary, ticking: views.some(view => !['done', 'failed', 'waiting'].includes(view.status)),
    machines: views.map(({ machine, status }) => ({ id: machine.environmentId, label: machine.label, status: RUN_STATUS_LABELS[status], failure: machine.failure ?? '',
      elapsed: ['done', 'failed', 'waiting'].includes(status) ? '' : formatUpdateElapsed(now - machine.startedAt), failed: status === 'failed' })),
  };
}
export async function updateEverythingOp(client: T3Client, op: string, native: Native, now = 0): Promise<boolean> {
  if (op.startsWith('sidebar:')) op = op.slice(8);
  if (!op.startsWith('ue:')) return false;
  const state = updateEverythingState(client), deps = nativeRunDeps(client, native);
  if (now > 0) state.now = now;
  if (op === 'ue:open' && !state.run) {
    const plan = updateEverythingPlan(client, native, cachedReleasePublished(CLIENT_VERSION)); if (plan.hasUpdates) { state.dialog = plan; state.skippedOpen = false; }
  } else if (op === 'ue:cancel') state.dialog = null;
  else if (op === 'ue:skipped') state.skippedOpen = !state.skippedOpen;
  else if (op === 'ue:confirm' && state.dialog && !state.run) {
    const plan = state.dialog; state.dialog = null;
    void runEverything(state, plan, deps).catch(error => { state.run = null; pushToast(client, { kind: 'error', title: 'Update failed', description: String(error), stacked: true }); notify(native); });
  } else if (op === 'ue:details') state.details = !state.details;
  else if (op === 'ue:retry') retryEverythingServers(state, deps);
  else if (op === 'ue:dismiss' && state.run) state.run = closeFinishedUpdateRun({ ...state.run, machines: state.run.machines.filter(machine => machine.result === null) });
  return true;
}

/** UpdateEverythingDialog: one row per machine that updates, the skipped ones behind a disclosure, and the running threads a restart interrupts. */
export function updateEverythingDialog(client: T3Client) {
  const state = updateEverythingState(client), plan = state.dialog;
  if (!plan) return { open: false, title: '', version: '', providers: '', rows: [], skippedRows: [], skippedOpen: false, warning: '' };
  const sources = environmentSources(client, fleet.saved, fleet.entries);
  const runningOn = (environmentId: string) => {
    const source = sources.find(source => source.environmentId === environmentId);
    const threads = source?.focused ? client.shell.threads : fleet.entries.get(source?.key ?? '')?.shell.threads ?? [];
    return threads.filter(thread => ['preparing', 'starting', 'running'].includes(str(thread.status))).length;
  };
  let stopped = 0, continuing = 0;
  for (const restart of plan.restarts) {
    const running = runningOn(restart.environmentId);
    if (restart.continuesThreads) continuing += running; else stopped += running;
  }
  const running = stopped + continuing, threads = `${running} running ${running === 1 ? 'thread' : 'threads'}`;
  const warning = !running ? '' : stopped === 0 ? `${threads} will be interrupted and can resume after the restart.`
    : continuing === 0 ? `${threads} will be interrupted.` : `${threads} will be interrupted. ${continuing} can resume after the restart.`;
  const updating = plan.rows.filter(row => row.app !== null || row.providers.length > 0), skipped = plan.rows.filter(row => row.skipped !== null);
  const restarting = new Set(plan.restarts.map(machine => machine.environmentId));
  return { open: true, title: `Update ${updating.length} ${updating.length === 1 ? 'machine' : 'machines'}?`,
    // "T3 Code <version> · N provider updates"; the version is drawn in the monospace face.
    version: plan.version ?? '', providers: plan.providerCount > 0 ? `${plan.providerCount} provider ${plan.providerCount === 1 ? 'update' : 'updates'}` : '',
    rows: updating.map(row => ({ id: row.environmentId ?? 'this-app', label: row.label, app: row.app === null ? '' : APP_LABELS[row.app],
      skipped: row.app === null && row.skipped !== null, restartsLast: row.app === 'this-app', providers: row.providers.join(', '),
      running: row.environmentId !== null && restarting.has(row.environmentId) && runningOn(row.environmentId) > 0 ? `${runningOn(row.environmentId)} running` : '' })),
    skippedRows: skipped.map(row => ({ id: row.environmentId ?? 'this-app', label: row.label, reason: row.skipped ?? '' })),
    skippedOpen: state.skippedOpen, warning };
}
