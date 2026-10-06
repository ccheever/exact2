// Auto balance: a new-thread draft on a project several machines hold runs on the
// machine with the most free capacity (T3 Code MIT, see LICENSE-T3; reference
// 1e2ecbd975: apps/web/src/components/ChatView.tsx automaticEnvironment,
// needsLoadBalancing, loadBalancingCandidates, onAutoEnvironment, autoEnvironmentLabel
// and onSend's load-balancing guard; hooks/useLoadBalancedEnvironment.ts;
// packages/client-runtime/src/state/server.ts hostResources (5 s timeout, 5 s stale
// time); composerDraftStore.ts environmentSelection / loadBalancedEnvironmentId).
// Port changes:
// - The draft keeps `environmentSelection` and `loadBalancedEnvironmentId` per draft key
//   (`composerControls.balance`), beside its workspace context, so no other writer of
//   that context drops them.
// - Host resources load in a command (`cclocal:balance-load`) the root's `balanceLoad`
//   task sends when `branches.balanceFetch` changes (X19: no timers in a data module;
//   X21: each machine's request goes over its own transport, the focused connection or
//   its T3Fleet transport, with a 5 s deadline). The task hands the window's wall time
//   in; it stands for the client receipt time of that load's samples.
// - The reference retargets the draft's project reference when the choice resolves.
//   Here a draft belongs to the focused connection, and moving it refocuses the client,
//   so a choice that resolves while the user types records only the machine; the draft
//   moves there when it is sent (`autoBalanceSend`), then sends.
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import { bridgeReply, providerAvailable, type Native } from './protocol';
import { fleet, EnvironmentFleet, type FleetEntry } from './settings-b-fleet';
import { decodePrefs, type ConnectionPrefs } from './connections';
import { environmentOptions, runOnEnvironment, type EnvironmentOption } from './r4-git-env';
import { draftContext, patchDraftContext } from './composer-controls-branch';
import { composerNow } from './composer-controls';
import { referencedFiles } from './composer-editor-files';
import { isScratch, scratchRootOf } from './r12-threads-scratch';
import { pushToast } from './toast';
import { chooseLoadBalancedEnvironment, decodeHostResources, type HostResourcesSnapshot } from './load-balancing';

export const AUTO_ENVIRONMENT = 'auto';
export const HOST_RESOURCES_TIMEOUT_MS = 5_000;
export const HOST_RESOURCES_STALE_MS = 5_000;
const DEFAULT_WEIGHT = 50;

// ── The draft's selection (composerDraftStore) ─────────────────────────────
export type BalanceSelection = { selection: '' | 'auto' | 'manual'; choice: string };
type BalanceStore = Record<string, BalanceSelection>;
const store = (client: T3Client): BalanceStore => (client.local.composerControls as { balance?: BalanceStore }).balance ??= {};
export function draftSelection(client: T3Client, key = client.draftKey): BalanceSelection {
  const saved = store(client)[key];
  return { selection: saved?.selection === 'auto' || saved?.selection === 'manual' ? saved.selection : '', choice: str(saved?.choice) };
}
export function setDraftSelection(client: T3Client, value: BalanceSelection, key = client.draftKey): void {
  store(client)[key] = { selection: value.selection, choice: value.choice };
}
/** A draft that moved machines carries its selection; the old key's entry goes. */
export function moveDraftSelection(client: T3Client, from: string, to: string, value: BalanceSelection): void {
  delete store(client)[from];
  setDraftSelection(client, value, to);
}

// ── Load balancing preferences (Settings › Connections) ───────────────────
const prefsByClient = new WeakMap<T3Client, ConnectionPrefs>();
export const balancePrefs = (client: T3Client): ConnectionPrefs =>
  prefsByClient.get(client) ?? { loadBalancingEnabled: false, loadBalancingWeights: {}, githubRouting: {} };
export function noteBalancePrefs(client: T3Client, prefs: ConnectionPrefs): void { prefsByClient.set(client, prefs); }
/** The snapshot's read of the device preferences (app.ts, before the snapshot is built). */
export async function autoBalancePrepare(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available) return;
  try {
    const reply = await bridgeReply(native, { op: 'connectionPreferences' });
    if (reply.ok) noteBalancePrefs(client, decodePrefs(str(obj(reply.value).text, '{}')));
  } catch { /* the last read stands */ }
}

// ── hostResources: one entry per environment (createEnvironmentQueryAtomFamily) ──
export type HostResourcesEntry = { resources: HostResourcesSnapshot | null; receivedAt: number; fetchedAt: number; pending: boolean; failed: boolean; stale: boolean };
const resources = new Map<string, HostResourcesEntry>();
const inflight = new Map<string, Promise<void>>();
/** Tests reset the shared registry. */
export function resetHostResources(): void { resources.clear(); inflight.clear(); serial = 0; }
export const hostResourcesEntry = (environmentId: string): HostResourcesEntry | undefined => resources.get(environmentId);
let serial = 0;
/** registry.refresh: the next load asks these machines again; until it answers they read as pending. */
export function refreshHostResources(environmentIds: readonly string[]): void {
  serial++;
  for (const id of environmentIds) {
    const entry = resources.get(id);
    resources.set(id, entry ? { ...entry, pending: true, stale: true } : { resources: null, receivedAt: 0, fetchedAt: 0, pending: true, failed: false, stale: true });
  }
}

/** A logical-project machine with what the candidate filter reads. */
export type BalanceMachine = EnvironmentOption & { key: string; connected: boolean; config: Obj };
function machines(client: T3Client, source: EnvironmentFleet): BalanceMachine[] {
  const entries = [...source.entries.values()];
  return environmentOptions(client, source).map(option => {
    if (option.selected) return { ...option, key: '', connected: client.connection === 'connected', config: client.config };
    const entry = entries.find(candidate => candidate.environmentId === option.id) as FleetEntry | undefined;
    return { ...option, key: str(entry?.key), connected: entry?.phase === 'connected', config: entry?.config ?? {} };
  });
}

export const hasComposerAttachments = (client: T3Client): boolean =>
  client.snapshotDrafts.length > 0 || referencedFiles(client.local, client.draftKey, client.draft).length > 0;

export type AutoBalanceState = {
  /** onAutoEnvironment is offered: the Run on menu lists "Auto balance". */
  offered: boolean;
  /** automaticEnvironment: the draft runs wherever auto balance chooses. */
  automatic: boolean;
  /** needsLoadBalancing: automatic and no machine chosen yet. */
  needs: boolean;
  pending: boolean;
  failed: boolean;
  /** The resolved machine (loadBalancedEnvironmentId), else ''. */
  chosen: string;
  /** chooseLoadBalancedEnvironment over the current samples. */
  choice: string | null;
  candidates: string[];
  logical: BalanceMachine[];
  /** autoEnvironmentLabel ('' when the draft is not automatic). */
  label: string;
  /** The load the root task sends ('' when nothing needs asking). */
  fetch: string;
};

/** ChatView's automatic-environment state for the focused draft, at `now`. */
export function autoBalanceState(client: T3Client, now = composerNow(client), source: EnvironmentFleet = fleet): AutoBalanceState {
  const none: AutoBalanceState = { offered: false, automatic: false, needs: false, pending: false, failed: false, chosen: '', choice: null, candidates: [], logical: [], label: '', fetch: '' };
  if (client.threadId || !client.projectId || !client.ready) return none;
  const logical = machines(client, source);
  const project = client.shell.projects.find(entry => entry.id === client.projectId);
  // Auto balance retargets to an existing project; a machine's "No project" folder may not exist until it is picked.
  const scratch = isScratch(project, scratchRootOf(client.connection === 'connected', client.config));
  const prefs = balancePrefs(client);
  const offered = logical.length > 1 && !scratch && prefs.loadBalancingEnabled;
  const draft = draftSelection(client), context = draftContext(client);
  const automatic = offered && draft.selection !== 'manual' && (!hasComposerAttachments(client) || !!draft.choice)
    && (!context.branch || draft.selection === 'auto') && !context.worktreePath;
  const weight = (id: string) => prefs.loadBalancingWeights[id] ?? DEFAULT_WEIGHT;
  const needs = automatic && !draft.choice;
  const provider = arr(client.config.providers).find(entry => entry.instanceId === client.providerId);
  // selectedProvider: the chosen instance's driver, else the requested kind (resolveComposerProviderSelection falls back to Codex).
  const driver = str(provider?.driver) || 'codex';
  const candidates = !needs ? [] : logical.filter(machine => machine.connected && weight(machine.id) > 0
    && arr(machine.config.providers).some(entry => (!client.providerId || entry.instanceId === client.providerId) && str(entry.driver) === driver && providerAvailable(entry)))
    .map(machine => machine.id);
  const samples = candidates.map(id => {
    const entry = resources.get(id);
    return { environmentId: id, resources: entry && !entry.failed ? entry.resources : null, receivedAt: entry?.receivedAt ?? 0,
      pending: !entry || entry.pending, failed: !!entry && entry.failed && !entry.pending };
  });
  const pending = samples.some(sample => sample.pending);
  const choice = chooseLoadBalancedEnvironment(samples.map(sample => ({ ...sample, weight: weight(sample.environmentId) })), now);
  const failed = !pending && choice === null && samples.some(sample => sample.failed);
  const label = !automatic ? '' : draft.choice ? 'Auto balance' : pending ? 'Checking machines…' : failed ? 'Auto balance unavailable' : 'Auto balance';
  // Only mounted for unresolved automatic drafts, so idle clients do not poll hosts.
  const fetch = needs && candidates.length ? `${client.draftKey}|${candidates.join(',')}|${serial}` : '';
  return { offered, automatic, needs, pending, failed, chosen: draft.choice, choice, candidates, logical, label, fetch };
}

// ── The load (useLoadBalancedEnvironment's queries) ───────────────────────
async function fetchOne(client: T3Client, native: Native, machine: BalanceMachine, now: number, source: EnvironmentFleet): Promise<void> {
  const request = { op: 'request', method: 'server.getHostResources', payload: {}, timeout: HOST_RESOURCES_TIMEOUT_MS / 1000 };
  let value: HostResourcesSnapshot | null = null;
  try {
    if (!machine.key) value = decodeHostResources(await client.call(native, request));
    else {
      const entry = source.entries.get(machine.key);
      if (!entry || entry.phase !== 'connected') throw new Error('That machine is not connected.');
      const reply = await bridgeReply(EnvironmentFleet.native(native, machine.key), { ...request, generation: entry.generation });
      if (!reply.ok) throw new Error(reply.error!.message);
      value = decodeHostResources(reply.value);
    }
  } catch { value = null; }
  // A failed request keeps no sample (AsyncResult.Failure); receipt time is the client's.
  resources.set(machine.id, { resources: value, receivedAt: value ? now : 0, fetchedAt: now, pending: false, failed: value === null, stale: false });
}

const sending = new WeakSet<T3Client>();
/**
 * `cclocal:balance-load`: ask each candidate whose sample is missing, refreshed or older
 * than the 5 s stale time, then resolve the draft once nothing is pending (the reference's
 * effect): a send in flight blocks it, and so does a draft that changed meanwhile.
 */
export async function loadHostResources(client: T3Client, native: Native, fetch: string, now: number, source: EnvironmentFleet = fleet): Promise<string> {
  const before = autoBalanceState(client, now, source);
  if (!before.needs || !fetch || before.fetch !== fetch) return '';
  const key = client.draftKey;
  await Promise.all(before.logical.filter(machine => before.candidates.includes(machine.id)).map(machine => {
    const entry = resources.get(machine.id);
    const due = !entry || entry.stale || now - entry.fetchedAt >= HOST_RESOURCES_STALE_MS;
    if (!due) return Promise.resolve();
    let running = inflight.get(machine.id);
    if (!running) {
      if (entry) resources.set(machine.id, { ...entry, pending: true });
      running = fetchOne(client, native, machine, now, source).finally(() => inflight.delete(machine.id));
      inflight.set(machine.id, running);
    }
    return running;
  }));
  const after = autoBalanceState(client, now, source);
  if (!after.needs || after.pending || !after.choice || sending.has(client) || client.draftKey !== key) return '';
  const target = after.logical.find(machine => machine.id === after.choice);
  if (!target?.projectId) return '';
  setDraftSelection(client, { selection: 'auto', choice: after.choice });
  return '';
}

// ── The Run on menu ───────────────────────────────────────────────────────
/** onAutoEnvironment: attachments stay where they are; otherwise ask every machine again. */
export function chooseAutoEnvironment(client: T3Client, source: EnvironmentFleet = fleet): void {
  const state = autoBalanceState(client, composerNow(client), source);
  if (client.threadId || !state.offered) return;
  if (hasComposerAttachments(client)) {
    pushToast(client, { kind: 'warning', key: 'load-balancing-attachments', title: 'Keep attachments on this machine',
      description: 'Remove attachments before choosing automatic routing, then attach them on the selected machine.' });
    return;
  }
  refreshHostResources(state.logical.map(machine => machine.id));
  setDraftSelection(client, { selection: 'auto', choice: '' });
  patchDraftContext(client, { branch: '', worktreePath: '' });
}

/** The "Run on" options with "Auto balance" first when it is offered; it is the selected value while automatic. */
export function withAutoOption(client: T3Client, options: EnvironmentOption[], state = autoBalanceState(client)): EnvironmentOption[] {
  if (!state.offered || options.length < 2) return options;
  const auto: EnvironmentOption = { id: AUTO_ENVIRONMENT, label: state.label || 'Auto balance', machine: 'scale', primary: false, projectId: '', selected: state.automatic };
  return [auto, ...options.map(option => state.automatic ? { ...option, selected: false } : option)];
}
/** The trigger's icon and label: the scale and the auto label while automatic. */
export function autoIndicator<T extends { machine: string; label: string }>(client: T3Client, focused: T, state = autoBalanceState(client)): T {
  return state.automatic ? { ...focused, machine: 'scale', label: state.label } : focused;
}

// ── Send (onSend's guard, then the retarget the reference made at resolution) ──
/**
 * Sends the draft where auto balance chose: an unresolved draft waits with the reference's
 * toast; a draft resolved to another machine moves there first (its text, workspace
 * context, selection and model), and the send runs on that connection. A draft with
 * attachments stays on the machine that holds them.
 */
export async function autoBalanceSend(client: T3Client, native: Native, send: () => Promise<void>, source: EnvironmentFleet = fleet): Promise<void> {
  const state = autoBalanceState(client, composerNow(client), source);
  if (state.needs) {
    pushToast(client, { kind: 'warning', title: state.pending ? 'Checking machine resources' : 'Choose a machine to continue',
      description: state.pending ? 'Resource checks are still running. You can choose a machine in the composer.'
        : 'No eligible machine has available resources. Choose a machine in the composer to override.' });
    return;
  }
  sending.add(client);
  try {
    if (state.automatic && state.chosen && state.chosen !== client.environmentId && !hasComposerAttachments(client)) {
      const keep = { providerId: client.providerId, modelId: client.modelId, modelOptions: client.modelOptions, runtimeMode: client.runtimeMode, interactionMode: client.interactionMode };
      const selection = draftSelection(client);
      const moved = await runOnEnvironment(client, native, state.chosen, source);
      if (moved.status) client.adoptStatus(moved.status, moved.generation);
      if (client.environmentId !== state.chosen) return;
      setDraftSelection(client, selection);
      await client.synchronize(native);
      // The candidate filter asked for this provider instance there; keep the draft's model.
      const provider = arr(client.config.providers).find(entry => entry.instanceId === keep.providerId);
      if (provider && arr(provider.models).some(model => model.slug === keep.modelId)) Object.assign(client, keep);
    }
    await send();
  } finally { sending.delete(client); }
}
