// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/client-ops-auto-balance.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Auto balance's multi-machine update banner ops (client-ops.ts READ_OPS; task auto-balance,
// auto-balance-banner.ts). Like the su: ops they run offline and need no write scope.
//   ab:update-all  Update all / Update K machines / Retry (desktop apps ask once first)
//   ab:dismiss     "Dismiss update notice"
//   ab:copy        a manual machine's Copy update command / Copy relaunch command (id: its environment)
import type { Files, Native } from './protocol';
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { nativeUpdateDeps } from './server-update';
import { autoBalanceBannerOp } from './auto-balance-banner';

export async function autoBalanceOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  void value; void n; void storage; void out;
  if (!op.startsWith('ab:')) return false;
  return autoBalanceBannerOp(this, op, id, nativeUpdateDeps(native, this));
}
