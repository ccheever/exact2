// providerSkills.ts and ChatComposer workspace refresh (T3 Code 1e2ecbd975).
// Root Contract actions own the 10-second completion-based retry clock; this
// module owns snapshot completeness and stale-response/single-flight guards.
import { arr, str, type Obj } from './domain';

export function hasCompleteProviderWorkspaceSnapshot(provider: Obj | undefined, cwd: string): boolean {
  const snapshot = cwd && arr(provider?.workspaceSnapshots).find(entry => str(entry.cwd) === cwd);
  return !!snapshot && !snapshot.slashCommandsPending;
}
export function workspaceValues(provider: Obj, cwd: string, key: 'skills' | 'slashCommands'): Obj[] {
  const snapshot = cwd && arr(provider.workspaceSnapshots).find(entry => str(entry.cwd) === cwd);
  return arr(snapshot ? snapshot[key] : provider[key]);
}

export type WorkspaceReadiness = { key: string; needed: boolean };
export type WorkspaceRefresh = { key: string; retry: boolean };
type Attempt = { key: string; instanceId: string; cwd: string; complete: boolean; observedComplete: boolean; flight: Promise<WorkspaceRefresh> | null };
export class WorkspaceDiscovery {
  private attempts = new Map<string, Attempt>();
  private serial = 0;
  private selected: Attempt | undefined;

  state(environment: string, generation: number, provider: Obj, cwd: string): WorkspaceReadiness {
    const instanceId = str(provider.instanceId);
    if (!environment || !instanceId || !cwd) { this.selected = undefined; return { key: '', needed: false }; }
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
    return { key: attempt.key, needed: !attempt.complete };
  }

  async refresh(key: string, request: (method: string, payload: Obj) => Promise<Obj>): Promise<WorkspaceRefresh> {
    const attempt = this.selected;
    // Root mutations carry the readiness key, so switching environments before
    // dispatch never sends an old workspace request through the new transport.
    if (!attempt || attempt.key !== key || attempt.complete) return { key, retry: false };
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
      return { key, retry: !attempt.complete };
    })();
    attempt.flight = flight;
    try { return await flight; } finally { attempt.flight = null; }
  }
}
