// providerSkills.ts and ChatComposer workspace refresh (T3 Code 2359643cff), with
// useProviderWorkspaceSnapshotRefresh's attempt limit (7c3b3b3605ef).
// Root Contract actions own the 10-second completion-based retry clock; this
// module owns snapshot completeness and stale-response/single-flight guards.
// ChatComposer.tsx:2195-2268: only a pending snapshot (`slashCommandsPending`)
// arms a timer for the retry. A failed or snapshot-less answer is retried when
// the refresh effect next runs after the cooldown: when the prompt, the provider
// list or the settings change. `wake` counts those changes for the root.
import { arr, str, type Obj } from '../client/domain';

export const PROVIDER_WORKSPACE_SNAPSHOT_TTL_MS = 5 * 60_000;
// A cwd that never scans, such as a deleted worktree, would otherwise retry forever.
const WORKSPACE_SNAPSHOT_MAX_ATTEMPTS = 3;
export function hasCurrentProviderWorkspaceSnapshot(provider: Obj | undefined, cwd: string, now: number): boolean {
  const snapshot = cwd && arr(provider?.workspaceSnapshots).find(entry => str(entry.cwd) === cwd);
  return !!snapshot && !snapshot.slashCommandsPending && now - Date.parse(str(snapshot.checkedAt)) < PROVIDER_WORKSPACE_SNAPSHOT_TTL_MS;
}
export function hasCompleteProviderWorkspaceSnapshot(provider: Obj | undefined, cwd: string): boolean {
  const snapshot = cwd && arr(provider?.workspaceSnapshots).find(entry => str(entry.cwd) === cwd);
  return !!snapshot && !snapshot.slashCommandsPending;
}
const snapshotCheckedAt = (provider: Obj | undefined, cwd: string): string | undefined => {
  const snapshot = cwd ? arr(provider?.workspaceSnapshots).find(entry => str(entry.cwd) === cwd) : undefined;
  return snapshot ? str(snapshot.checkedAt) : undefined;
};
export function workspaceValues(provider: Obj, cwd: string, key: 'skills' | 'slashCommands'): Obj[] {
  const snapshot = cwd && arr(provider.workspaceSnapshots).find(entry => str(entry.cwd) === cwd);
  return arr(snapshot ? snapshot[key] : provider[key]);
}

export type WorkspaceReadiness = { key: string; needed: boolean; timer: boolean; wake: number };
export type WorkspaceRefresh = { key: string; retry: boolean; wake: number };
/** What re-runs the reference's refresh effect: the prompt, and the provider list or settings (one object, replaced on change). */
export type WorkspaceWake = { prompt: string; config: unknown };
const pendingSnapshot = (provider: Obj, cwd: string) => arr(provider.workspaceSnapshots).some(entry => str(entry.cwd) === cwd && entry.slashCommandsPending === true);
type Attempt = { key: string; base: string; instanceId: string; cwd: string; complete: boolean; observedComplete: boolean; requestedAt: number | undefined; checkedAt: string | undefined; flight: Promise<WorkspaceRefresh> | null };
export class WorkspaceDiscovery {
  private attempts = new Map<string, Attempt>();
  // Failed refreshes within the current request's TTL, by environment, instance and cwd.
  private failures = new Map<string, number>();
  // Whether the last provider seen had a complete snapshot for the cwd, a stale one too.
  private hadSnapshot = new Map<string, boolean>();
  private serial = 0;
  private selected: Attempt | undefined;
  private seen: WorkspaceWake | null = null;
  private wake = 0;
  private shown = true;
  private now: number | undefined;

  /**
   * `shown`: whether ChatComposer is mounted. The reference mounts it on a thread's chat view only, so
   * Settings, the Usage and Pull Requests pages and the welcome stop its refreshes, and mounting it again
   * starts its refs over (a fresh attempt, refreshed at once when the snapshot is incomplete).
   */
  state(environment: string, generation: number, provider: Obj, cwd: string, wake: WorkspaceWake = { prompt: '', config: provider }, shown = this.shown, now = this.now): WorkspaceReadiness {
    this.now = now;
    if (this.seen && (this.seen.prompt !== wake.prompt || this.seen.config !== wake.config)) this.wake++;
    this.seen = wake;
    // Mounting again starts over, failures too: the reference drops a cwd's state with its last subscriber.
    if (shown && !this.shown) { this.attempts.clear(); this.failures.clear(); this.hadSnapshot.clear(); }
    this.shown = shown;
    const instanceId = str(provider.instanceId);
    const base = shown && environment && instanceId && cwd ? JSON.stringify([environment, generation, instanceId, cwd]) : '';
    // Nothing watches the folder, provider or environment the composer moved from: its state goes with it
    // (useProviderWorkspaceSnapshotRefresh's atom family drops a node with its last subscriber).
    if (this.selected && this.selected.base !== base) { this.attempts.delete(this.selected.base); this.failures.delete(this.selected.base); this.hadSnapshot.delete(this.selected.base); }
    if (!base) { this.selected = undefined; return { key: '', needed: false, timer: false, wake: this.wake }; }
    let attempt = this.attempts.get(base);
    const complete = now === undefined ? hasCompleteProviderWorkspaceSnapshot(provider, cwd) : hasCurrentProviderWorkspaceSnapshot(provider, cwd, now);
    const recent = attempt?.requestedAt !== undefined && now !== undefined && now - attempt.requestedAt < PROVIDER_WORKSPACE_SNAPSHOT_TTL_MS;
    // The failures start over when the server drops the snapshot or the TTL from the last request passes, not on a
    // complete snapshot: one that is stale may be all a failing scan returns (planProviderWorkspaceSnapshotRefresh).
    const hasSnapshot = hasCompleteProviderWorkspaceSnapshot(provider, cwd);
    if ((this.hadSnapshot.get(base) && !hasSnapshot) || (attempt?.requestedAt !== undefined && now !== undefined && !recent)) this.failures.delete(base);
    this.hadSnapshot.set(base, hasSnapshot);
    // A reset replaces the attempt, including its key: an older RPC's answer
    // cannot mark the replacement complete or postpone its retry.
    if (!attempt || (!recent && ((attempt.observedComplete && !complete) || (attempt.complete && !complete && now !== undefined)))) {
      attempt = { key: JSON.stringify([base, ++this.serial]), base, instanceId, cwd, complete: false, observedComplete: false, requestedAt: undefined, checkedAt: undefined, flight: null };
      this.attempts.set(base, attempt);
    }
    attempt.observedComplete = complete;
    attempt.checkedAt = snapshotCheckedAt(provider, cwd);
    if (complete) attempt.complete = true;
    this.selected = attempt;
    // After its last allowed failure a scan waits out the TTL from its request.
    const exhausted = (this.failures.get(base) ?? 0) >= WORKSPACE_SNAPSHOT_MAX_ATTEMPTS && recent;
    return { key: attempt.key, needed: !attempt.complete && !exhausted, timer: pendingSnapshot(provider, cwd), wake: this.wake };
  }

  async refresh(key: string, request: (method: string, payload: Obj) => Promise<Obj>): Promise<WorkspaceRefresh> {
    const attempt = this.selected;
    // Root mutations carry the readiness key, so switching environments before
    // dispatch never sends an old workspace request through the new transport.
    if (!attempt || attempt.key !== key || attempt.complete) return { key, retry: false, wake: this.wake };
    if (attempt.flight) return attempt.flight;
    attempt.requestedAt = this.now;
    const base = attempt.base, before = attempt.checkedAt;
    const flight = (async () => {
      let complete = false;
      try {
        const result = await request('server.refreshProviders', { instanceId: attempt.instanceId, cwd: attempt.cwd });
        const refreshed = arr(result.providers).find(entry => entry.instanceId === attempt.instanceId), at = this.now;
        // The same stale snapshot again is a failed scan; a new server timestamp proves progress even when this
        // clock is ahead of the server's.
        complete = hasCompleteProviderWorkspaceSnapshot(refreshed, attempt.cwd) && (at === undefined
          || hasCurrentProviderWorkspaceSnapshot(refreshed, attempt.cwd, at) || snapshotCheckedAt(refreshed, attempt.cwd) !== before);
      } catch { /* Discovery failures stay silent and retry after the root cooldown. */ }
      // A config subscription may have supplied a complete snapshot while this
      // call was pending. Never replace that knowledge with an older partial reply.
      attempt.complete = attempt.complete || complete;
      if (attempt.complete) { this.failures.delete(base); return { key, retry: false, wake: this.wake }; }
      // A dropped folder's late failure is not counted: its state went with its last watcher.
      if (!this.attempts.has(base)) return { key, retry: false, wake: this.wake };
      const failed = (this.failures.get(base) ?? 0) + 1;
      this.failures.set(base, failed);
      // The last attempt keeps its request time, so the snapshot TTL holds off the next scan.
      if (failed >= WORKSPACE_SNAPSHOT_MAX_ATTEMPTS) return { key, retry: false, wake: this.wake };
      attempt.requestedAt = undefined;
      // Changes seen while the request was out do not wake a retry (the
      // reference's effect returns early while its key is claimed).
      return { key, retry: true, wake: this.wake };
    })();
    attempt.flight = flight;
    try { return await flight; } finally { attempt.flight = null; }
  }
}
