// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r11-device-diff.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r11-device: the Diff's working-tree and branch preview for a project outside the server's
// workspace root (MIT reference, see LICENSE-T3: apps/web/src/components/DiffPanel.tsx
// shouldRetryBranchDiffAtEnvironmentCwd). The server answers `review.getDiffPreview` for a cwd outside
// its configured root with "Review diff preview cwd must stay within the configured workspace root.";
// the reference then asks again at the environment's own cwd (`server.getConfig` cwd) and shows that.
import { obj, str, type Obj } from './domain';

type Request = { method: string; payload: Obj };

/** One diff request, retried at the server's cwd as DiffPanel does when the project's cwd is refused. */
export async function requestDiff(config: unknown, request: Request, send: (method: string, payload: Obj) => Promise<Obj>): Promise<Obj> {
  try { return await send(request.method, request.payload); }
  catch (error) {
    const serverCwd = str(obj(config).cwd), cwd = str(request.payload.cwd);
    const refused = error instanceof Error && error.message.includes('configured workspace root');
    if (request.method !== 'review.getDiffPreview' || !refused || !serverCwd || serverCwd === cwd) throw error;
    return send(request.method, { ...request.payload, cwd: serverCwd });
  }
}
