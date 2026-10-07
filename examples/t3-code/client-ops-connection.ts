// The connection's client.command() ops (client-ops.ts): connect and
// disconnect, the Connections settings' environment ops (connections.ts), and
// Refresh, which drops the loaded shell and synchronizes again.
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { runConnectionOp, CONNECTION_OPS } from './connections';
import { obj, str } from './domain';
import { ClientError, parsePairing, type Files, type Native } from './protocol';
import { withStandardScope } from './remote-scopes';

/** Connect, disconnect, the Connections settings' environment ops and Refresh. */
export async function connectionOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op === 'connect' || op === 'reconnect') {
      const target = parsePairing(id || this.origin, value);
      // Retry without a new credential probes a live socket instead of replacing it (T3Transport retryNow, 9333509).
      const response = await this.raw(native, { op: op === 'reconnect' && !target.credential ? 'retry' : 'connect', ...withStandardScope(target) });
      if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind);
      this.adoptStatus(obj(response.value), response.generation);
      this.error = '';
    } else if (op === 'disconnect' || op === 'forget') {
      const focused = this.environmentId;
      const response = await this.raw(native, { op: 'disconnect', forget: op === 'forget' });
      if (response.ok) this.adoptStatus(obj(response.value), response.generation);
      if (response.ok && op === 'forget' && !str(obj(response.value).origin)) this.dropFocus(focused);
    } else if (CONNECTION_OPS.includes(op)) {
      const focused = op === 'environment-forget' && value === this.environmentId ? value : '';
      const result = await runConnectionOp(native, op, id, value, this.connection === 'connected', this);
      if (result.status) this.adoptStatus(result.status, result.generation);
      if (focused && result.status && !str(result.status.origin)) this.dropFocus(focused);
      this.error = '';
    } else if (op === 'refresh') {
      this.shellLoaded = false; this.thread = null; this.error = '';
      await this.synchronize(native);
    } else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
