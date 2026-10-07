// Pinned365aa87982 ThreadRouteScreen stages a one-use script launch before navigation.
// @ref llp/1106.005-composer-and-transcript.decision.md
import type { T3Client } from './shared/client';
import { mobileClient, mobileNative } from './client';
import { ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { reviewOwner, reviewNative, assertReviewOwner } from './review-owner';
import { mobileSessionGrants } from './mobile-grants';
interface Launch { owner: string; token: string; terminalId: string; threadId: string; cwd: string;
  worktreePath: string | null; env: Record<string, string>; initialInput: string; active?: boolean }
const launches = new WeakMap<T3Client, Launch>();
let sequence = 0;
const routes = new WeakMap<T3Client, { identity: string }>();
export function mobileThreadHeaderObserveRoute(route: string, client: T3Client) { if (routes.get(client)?.identity !== route) routes.set(client, { identity: route }); }
export function mobileStageThreadScript(client: T3Client, terminalId: string, launch: Omit<Launch, 'owner'|'token'|'terminalId'|'threadId'>): string {
  const token = `${reviewOwner(client)}:${++sequence}`;
  launches.set(client, { ...launch, owner: reviewOwner(client), token, terminalId, threadId: client.threadId });
  return token;
}
export function mobileThreadHeaderPending(client: T3Client = mobileClient): { token: string; terminalId: string } {
  const launch = launches.get(client); return launch?.owner === reviewOwner(client) ? { token: launch.token, terminalId: launch.terminalId } : { token: '', terminalId: '' };
}
export function mobileThreadHeaderPendingLaunch(terminalId: string, client: T3Client = mobileClient): string {
  const launch = launches.get(client);
  return launch?.owner === reviewOwner(client) && launch.terminalId === terminalId ? launch.token : '';
}
/** Explicit root action, only after the owned terminal route is ready. No resource writes.
 * Remove before issuing: failures/uncertainty can never replay a shell command automatically. */
export async function mobileThreadHeaderLaunch(terminalId: string, token: string, nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  const launch = launches.get(client), ref = { environmentId: client.environmentId, threadId: client.threadId };
  const result = (message = '') => ({ revision: client.revision, message, requestRoute: '', ...ref });
  if (!launch || launch.active || launch.token !== token || launch.terminalId !== terminalId || launch.owner !== reviewOwner(client)) return result();
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to run scripts.');
  launch.active = true; const route = routes.get(client);
  const assertRoute = () => { assertReviewOwner(client, launch.owner); if (routes.get(client) !== route) throw new ClientError('The terminal screen changed.', 'superseded'); };
  const guard = () => { assertRoute(); if (launches.get(client) !== launch) throw new ClientError('The script launch changed.', 'superseded'); };
  const owned = reviewNative(client, letGoAware(mobileNative(nativeInput)), launch.owner);
  const base: Native = { ...owned, later: async input => { assertRoute(); const answer = await owned.later(input); assertRoute(); return answer; } };
  try {
    const session = await client.http(base, '/api/auth/session'); guard();
    if (!mobileSessionGrants(session, 'terminal:operate')) throw new ClientError('This connection cannot operate terminals.');
    launches.delete(client);
    await client.rpc(base, 'terminal.open', { threadId: launch.threadId, terminalId, cwd: launch.cwd,
      worktreePath: launch.worktreePath, env: launch.env, cols: 80, rows: 24 });
    assertReviewOwner(client, launch.owner);
    const data = launch.initialInput; launch.initialInput = '';
    await client.rpc(base, 'terminal.write', { threadId: launch.threadId, terminalId, data });
  } catch (error) { if (launches.get(client) === launch) launches.delete(client); if (letGo(error)) throw error;
    return result(error instanceof Error ? error.message : 'Could not run project script.');
  } finally { launch.initialInput = ''; client.revision++; }
  return result();
}
