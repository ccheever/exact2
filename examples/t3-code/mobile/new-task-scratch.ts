// Pinned mobile365aa87982 NewTaskRouteScreen / NewTaskFlowProvider.
// @ref llp/1107.005-composer-and-transcript.decision.md#new-task-ownership
import type { T3Client } from './shared/client';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';
import { obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { isScratch, scratchRootOf, openRemoteScratch } from './shared/r12-threads-scratch';
import { openScratchProject } from './shared/r11-upstream-scratch';
import { environmentOptions, runOnEnvironment } from './shared/r4-git-env';
import { letGo } from './shared/let-go';
import { mobileSessionGrants } from './environment-detail';

type Target = { environmentId: string; origin: string; generation: number; root: string; entry: FleetEntry | null };
function targets(client: T3Client, background: EnvironmentFleet): Target[] {
  const result: Target[] = [];
  const root = scratchRootOf(client.connection === 'connected' && client.configLive && client.shellLive, client.config);
  if (client.environmentId && root) result.push({ environmentId: client.environmentId, origin: client.origin, generation: client.generation, root, entry: null });
  for (const entry of background.entries.values()) {
    if (entry.environmentId === client.environmentId || entry.phase !== 'connected' || entry.synchronized !== entry.generation) continue;
    const root = scratchRootOf(true, entry.config);
    if (root) result.push({ environmentId: entry.environmentId, origin: entry.origin, generation: entry.generation, root, entry });
  }
  return result;
}
const key = (target: Target) => JSON.stringify([target.environmentId, target.origin, target.generation, target.root, target.entry?.key ?? '']);
/** Like the source: selected eligible environment first, otherwise the first connected eligible environment. */
export function mobileScratchTarget(client: T3Client, background: EnvironmentFleet, environmentId = ''): string {
  const available = targets(client, background), target = environmentId ? available.find(target => target.environmentId === environmentId) : available[0];
  return target ? key(target) : '';
}
const selection = (client: T3Client) => JSON.stringify([client.origin, client.environmentId, client.generation, client.projectId, client.threadId, client.threadEpoch]);

/** The shared RPC and shell adoption remain the sole owners. No synthetic project is made. */
export async function mobileOpenScratch(id: string, native: Native, client: T3Client, background: EnvironmentFleet,
  current: () => boolean): Promise<{ environmentId: string; projectId: string }> {
  const target = targets(client, background).find(target => key(target) === id), captured = selection(client);
  if (!target) throw new ClientError('That environment is no longer available for tasks without a project.');
  const valid = () => current() && selection(client) === captured && targets(client, background).some(candidate => key(candidate) === id && candidate.entry === target.entry);
  let lost: unknown;
  const assertCurrent = () => { if (lost) throw lost; if (!valid()) throw new ClientError('The selected workspace changed.', 'superseded'); };
  const scoped: Native = { available: native.available, watch: topic => native.watch(topic), later: async request => {
    assertCurrent();
    try { const answer = await native.later(request); assertCurrent(); return answer; }
    catch (error) { if (letGo(error)) lost = error; throw error; }
  } };
  const remote = target.entry ? EnvironmentFleet.native(scoped, target.entry.key) : scoped;
  const session = await bridgeReply(remote, { op: 'http', path: '/api/auth/session', generation: target.generation });
  assertCurrent();
  if (!session.ok) throw new ClientError(session.error?.message || 'Could not check this connection.');
  if (session.generation !== target.generation) throw new ClientError('The connection changed.', 'superseded');
  if (!mobileSessionGrants(obj(session.value), 'orchestration:operate')) throw new ClientError('This connection cannot create tasks without a project.');
  let projectId: string;
  try { projectId = target.entry ? await openRemoteScratch(client, scoped, target.entry) : await openScratchProject(client, scoped); }
  catch (error) { assertCurrent(); throw error; }
  assertCurrent();
  const project = (target.entry?.shell ?? client.shell).projects.find(project => project.id === projectId);
  if (!isScratch(project, target.root)) {
    throw new ClientError('The server did not return its folder for tasks without a project.');
  }
  return { environmentId: target.environmentId, projectId };
}

/** The shared move has an await after reducing local slots. Journal only those
 * cells; rollback never reads current draftKey or overwrites a newer cell value. */
function scratchMoveJournal(client: T3Client, from: string, to: string, environmentId: string) {
  const controls = client.local.composerControls as { contexts?: Record<string, unknown>; balance?: Record<string, unknown> };
  controls.contexts ??= {}; controls.balance ??= {};
  const groups: Array<[() => Record<string, unknown>, string[]]> = [
    [() => client.local.drafts, [from, to]],
    [() => (client.local.composerControls as typeof controls).contexts ?? {}, [from, to]],
    [() => (client.local.composerControls as typeof controls).balance ?? {}, [from, to]],
    [() => client.local.selections, [environmentId]],
  ];
  const cells = groups.flatMap(([read, keys]) => keys.map(key => {
    const map = read();
    return { read, map, key, had: Object.hasOwn(map, key), before: map[key], stagedHad: false, staged: undefined as unknown };
  }));
  let reduced = false;
  return {
    reduced() {
      if (reduced) return;
      reduced = true;
      for (const cell of cells) { cell.stagedHad = Object.hasOwn(cell.map, cell.key); cell.staged = cell.map[cell.key]; }
    },
    rollback() {
      if (!reduced) return;
      for (const cell of cells) {
        const map = cell.read();
        if (map !== cell.map || Object.hasOwn(map, cell.key) !== cell.stagedHad || !Object.is(map[cell.key], cell.staged)) continue;
        if (cell.had) map[cell.key] = cell.before; else delete map[cell.key];
      }
    },
  };
}

/** Bypass the command preamble: the shared move reduces immediately after the
 * destination check. Its current-selection rollback cannot own a later route;
 * failed Connect replies therefore use the captured-slot journal above. */
export async function mobileMoveScratch(environmentId: string, native: Native, client: T3Client,
  background: EnvironmentFleet, current: () => boolean) {
  const captured = selection(client), target = environmentOptions(client, background).find(option => option.id === environmentId);
  if (!current()) throw new ClientError('The selected workspace changed.', 'superseded');
  if (!target?.projectId || environmentId === client.environmentId) throw new ClientError('Choose another connected scratch environment.');
  const journal = scratchMoveJournal(client, client.draftKey, `${environmentId}:new:${target.projectId}`, environmentId);
  let lost: unknown;
  const scoped: Native = { available: native.available, watch: topic => native.watch(topic), later: async input => {
    if (obj(input).op === 'fleetStop') journal.reduced();
    try {
      if (lost) throw lost;
      if (!current() || selection(client) !== captured) throw new ClientError('The selected workspace changed.', 'superseded');
      // Decode the actual reply before shared runOnEnvironment can invoke its
      // current-draft rollback. Transport/server refusals also restore safely.
      const reply = obj(input).op === 'connect' ? await bridgeReply(native, input) : await native.later(input);
      if (!current() || selection(client) !== captured) throw new ClientError('The selected workspace changed.', 'superseded');
      if (obj(input).op === 'connect' && obj(reply).ok === false) {
        const error = obj(obj(reply).error);
        throw new ClientError(String(error.message || 'Could not change environment.'), String(error.kind || 'connection'), error.uncertain === true);
      }
      return reply;
    } catch (error) { if (letGo(error)) lost = error; throw error; }
  } };
  try { return await runOnEnvironment(client, scoped, environmentId, background); }
  catch (error) { journal.rollback(); if (lost) throw lost; throw error; }
}
