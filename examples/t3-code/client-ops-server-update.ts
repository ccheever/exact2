// The server-update banner's, offline banner's and version card's client.command() ops
// (task server-update-banner; T3 Code MIT, see LICENSE-T3; reference 1e2ecbd975
// ChatView.tsx handleReconnectActiveEnvironment, handleDisconnectActiveEnvironment and
// the server-version item's onDismiss; ServerUpdateAction.tsx handleUpdate).
// A read group (client-ops.ts READ_OPS): they run offline and need no write scope.
//   su:update      the banner's Update / Retry / Copy … command (a desktop-managed app asks first)
//   su:confirm     the confirmation's Confirm; su:cancel its Cancel
//   su:dismiss     "Dismiss update notice" and the card's "Dismiss version mismatch warning"
//   su:reconnect   the offline banner's Reconnect
//   su:disconnect  "Disconnect server": switch the environment off and go Home
import type { Files, Native } from './protocol';
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { pushToast } from './toast';
import { connectionOps } from './client-ops-connection';
import { runConnectionOp } from './connections';
import { dismissServerUpdateFailure, dismissVersionMismatch } from './version-skew';
import { nativeUpdateDeps, updateEnvironment, updateTargetFromConfig } from './server-update';
import { canDisconnectEnvironment, confirms, dismissals, focusedKey, serverUpdateLabel, updateState, versionNotice } from './server-update-notices';

/** The banner's and card's ops (client-ops.ts READ_OPS: they run offline and need no write scope). */
export async function serverUpdateOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  void n;
  if (!op.startsWith('su:')) return false;
  if (op === 'su:update' || op === 'su:confirm') {
    const target = op === 'su:confirm' ? confirms.get(this) : updateTargetFromConfig(this.config, focusedKey(this), this.environmentId, serverUpdateLabel(this));
    confirms.delete(this);
    if (!target || !this.environmentId) return true;
    // ServerUpdateAction: the only confirmation in the flow; the remote machine installs without asking.
    if (op === 'su:update' && target.selfUpdate === 'desktop-managed' && target.desktopAppUpdate) { confirms.set(this, target); return true; }
    await updateEnvironment(target, nativeUpdateDeps(native, this));
  } else if (op === 'su:cancel') confirms.delete(this);
  else if (op === 'su:dismiss') {
    // Dismissal shares the version key (persisted); a failed attempt is dismissed for that attempt only.
    const state = updateState(this);
    if (state.status === 'failed' && (!id || state.attempt === id)) dismissServerUpdateFailure(state);
    dismissVersionMismatch(versionNotice(this).key, dismissals(this));
  } else if (op === 'su:reconnect') {
    try { await connectionOps.call(this, 'reconnect', '', '', 0, native, storage, out); }
    catch (error) { pushToast(this, { kind: 'error', title: 'Could not reconnect environment', description: error instanceof Error ? error.message : 'Failed to reconnect.', stacked: true }); }
  } else if (op === 'su:disconnect') {
    const environmentId = this.environmentId;
    if (!environmentId || !canDisconnectEnvironment(this.origin)) return true;
    try {
      const result = await runConnectionOp(native, 'environment-enabled', focusedKey(this), 'off', this.connection === 'connected', this, { failureTitle: 'Could not disconnect server' });
      if (result.status) this.adoptStatus(result.status, result.generation);
      // navigate({ to: "/" }): no thread of the switched-off server stays open.
      if (this.environmentId === environmentId) { this.environmentId = ''; this.projectId = ''; }
      this.threadId = ''; this.thread = null; this.threadEpoch++;
    } catch { /* runConnectionOp toasted "Could not disconnect server" */ }
  } else return false;
  return true;
}
