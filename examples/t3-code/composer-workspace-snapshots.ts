// providerSkills.ts and ChatComposer workspace refresh (T3 Code 1e2ecbd975).
// Root Contract actions own the 10-second completion-based retry clock; this
// module owns snapshot completeness and stale-response/single-flight guards.
// ChatComposer.tsx:2195-2268: only a pending snapshot (`slashCommandsPending`)
// arms a timer for the retry. A failed or snapshot-less answer is retried when
// the refresh effect next runs after the cooldown: when the prompt, the provider
// list or the settings change. `wake` counts those changes for the root.
import { arr, str, type Obj } from './domain';

export function hasCompleteProviderWorkspaceSnapshot(provider: Obj | undefined, cwd: string): boolean {
  const snapshot = cwd && arr(provider?.workspaceSnapshots).find(entry => str(entry.cwd) === cwd);
  return !!snapshot && !snapshot.slashCommandsPending;
}
export function workspaceValues(provider: Obj, cwd: string, key: 'skills' | 'slashCommands'): Obj[] {
  const snapshot = cwd && arr(provider.workspaceSnapshots).find(entry => str(entry.cwd) === cwd);
  return arr(snapshot ? snapshot[key] : provider[key]);
}

export type WorkspaceReadiness = { key: string; needed: boolean; timer: boolean; wake: number };
export type WorkspaceRefresh = { key: string; retry: boolean; wake: number };
/** What re-runs the reference's refresh effect: the prompt, and the provider list or settings (one object, replaced on change). */
export type WorkspaceWake = { prompt: string; config: unknown };
const pendingSnapshot = (provider: Obj, cwd: string) => arr(provider.workspaceSnapshots).some(entry => str(entry.cwd) === cwd && entry.slashCommandsPending === true);
type Attempt = { key: string; instanceId: string; cwd: string; complete: boolean; observedComplete: boolean; flight: Promise<WorkspaceRefresh> | null };
export class WorkspaceDiscovery {
  private attempts = new Map<string, Attempt>();
  private serial = 0;
  private selected: Attempt | undefined;
  private seen: WorkspaceWake | null = null;
  private wake = 0;
  private shown = true;

  /**
   * `shown`: whether ChatComposer is mounted. The reference mounts it on a thread's chat view only, so
   * Settings, the Usage and Pull Requests pages and the welcome stop its refreshes, and mounting it again
   * starts its refs over (a fresh attempt, refreshed at once when the snapshot is incomplete).
   */
  state(environment: string, generation: number, provider: Obj, cwd: string, wake: WorkspaceWake = { prompt: '', config: provider }, shown = this.shown): WorkspaceReadiness {
    if (this.seen && (this.seen.prompt !== wake.prompt || this.seen.config !== wake.config)) this.wake++;
    this.seen = wake;
    if (shown && !this.shown) this.attempts.clear();
    this.shown = shown;
    const instanceId = str(provider.instanceId);
    if (!shown || !environment || !instanceId || !cwd) { this.selected = undefined; return { key: '', needed: false, timer: false, wake: this.wake }; }
    const base = JSON.stringify([environment, generation, instanceId, cwd]);
    let attempt = this.attempts.get(base);
    const complete = hasCompleteProviderWorkspaceSnapshot(provider, cwd);
    // A reset replaces the attempt, including its key: an older RPC's answer
    // cannot mark the replacement complete or postpone its retry.
    if (!attempt || (attempt.observedComplete && !complete)) {
      attempt = { key: JSON.stringify([base, ++this.serial]), instanceId, cwd, complete: false, observedComplete: false, flight: null };
      this.attempts.set(base, attempt);
    }
    attempt.observedComplete = complete;
    if (complete) attempt.complete = true;
    this.selected = attempt;
    return { key: attempt.key, needed: !attempt.complete, timer: pendingSnapshot(provider, cwd), wake: this.wake };
  }

  async refresh(key: string, request: (method: string, payload: Obj) => Promise<Obj>): Promise<WorkspaceRefresh> {
    const attempt = this.selected;
    // Root mutations carry the readiness key, so switching environments before
    // dispatch never sends an old workspace request through the new transport.
    if (!attempt || attempt.key !== key || attempt.complete) return { key, retry: false, wake: this.wake };
    if (attempt.flight) return attempt.flight;
    const flight = (async () => {
      let complete = false;
      try {
        const result = await request('server.refreshProviders', { instanceId: attempt.instanceId, cwd: attempt.cwd });
        complete = hasCompleteProviderWorkspaceSnapshot(arr(result.providers).find(entry => entry.instanceId === attempt.instanceId), attempt.cwd);
      } catch { /* Discovery failures stay silent and retry after the root cooldown. */ }
      // A config subscription may have supplied a complete snapshot while this
      // call was pending. Never replace that knowledge with an older partial reply.
      attempt.complete = attempt.complete || complete;
      // Changes seen while the request was out do not wake a retry (the
      // reference's effect returns early while its key is claimed).
      return { key, retry: !attempt.complete, wake: this.wake };
    })();
    attempt.flight = flight;
    try { return await flight; } finally { attempt.flight = null; }
  }
}
