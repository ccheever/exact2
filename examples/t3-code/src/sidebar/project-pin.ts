// Pinning and archiving a project group, adapted from T3 Code 2359643cff (MIT; see LICENSE-T3):
// apps/web/src/hooks/useProjectGroupUpdate.ts (useProjectGroupUpdate, useProjectPin, checkProjectAccess) and
// useOrderWhileOpen.ts (keepOrder, withArrivedKeys). One update goes to each checkout of the group, one machine at
// a time, through the member's own environment (`fleet:` ids are a background environment's), and the first failure
// is a toast. A member's environment must be connected, take project organization (older servers drop `pinned` and
// `archived`, so their projects do not offer them) and let this session operate on it.
import { obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { ClientError, type Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { pushToast } from '../shell/toast';
import { sessionScopes } from '../connections/local-primary';
import { fleet, parseFleetThreadId, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { backgroundEntry, envAccess } from './background-threads';

export type ProjectGroupUpdate = Partial<{ title: string; pinned: boolean; archived: boolean }>;
type Member = Obj;

/** The member's environment: its id, raw project id, label, config, grants and whether it is connected now. */
function memberHome(client: T3Client, member: Member, source: EnvironmentFleet = fleet) {
  const ref = parseFleetThreadId(str(member.id)), environmentId = str(member.environmentId) || ref?.environmentId || client.environmentId;
  if (environmentId === client.environmentId) {
    return { environmentId, projectId: ref ? ref.threadId : str(member.id), label: str(obj(client.config.environment).label), config: client.config,
      scopes: client.scopes, connected: client.connection === 'connected' && Object.keys(client.config).length > 0 };
  }
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === environmentId);
  return { environmentId, projectId: ref ? ref.threadId : str(member.id), label: str(obj(entry?.config.environment).label), config: entry?.config ?? {},
    scopes: entry?.scopes ?? [], connected: backgroundEntry(client, environmentId, source) !== null && Object.keys(entry?.config ?? {}).length > 0 };
}

/** canOrganizeProjectGroup: every member's server keeps `pinned` and `archived` from project.update. */
export function canOrganizeProjectGroup(client: T3Client, members: readonly Member[], source: EnvironmentFleet = fleet): boolean {
  return members.length > 0 && members.every(member => obj(obj(memberHome(client, member, source).config.environment).capabilities).projectOrganization === true);
}
const canOperate = (client: T3Client, member: Member, source: EnvironmentFleet) => {
  const home = memberHome(client, member, source);
  return sessionScopes(home.environmentId, home.scopes).includes('orchestration:operate');
};
/** useProjectPin canPinProjectGroup: the group can be organized and this session may operate on every member's environment. */
export function canPinProjectGroup(client: T3Client, members: readonly Member[], source: EnvironmentFleet = fleet): boolean {
  return canOrganizeProjectGroup(client, members, source) && members.every(member => canOperate(client, member, source));
}

/**
 * updateProjectGroup: the update on each member in turn; the first failure is a toast (titled per machine when the
 * group spans several) and stops the rest. Returns whether every member took it.
 */
export async function updateProjectGroup(client: T3Client, native: Native, members: readonly Member[], input: ProjectGroupUpdate, failureTitle: string,
  source: EnvironmentFleet = fleet): Promise<boolean> {
  const fail = (title: string, message: string) => { pushToast(client, { kind: 'error', title, description: message, stacked: true }); return false; };
  const denied = (member: Member) => !canOperate(client, member, source);
  const first = members.find(denied);
  if (first) return fail(failureTitle, `This connection cannot change projects in ${memberHome(client, first, source).label || 'this environment'}.`);
  const unavailable = members.find(member => !memberHome(client, member, source).connected);
  if (unavailable) return fail(failureTitle, `Connect ${memberHome(client, unavailable, source).label || 'the selected environment'} and try again.`);
  for (const member of members) {
    const home = memberHome(client, member, source);
    // Access can be revoked between writes.
    if (denied(member)) return fail(failureTitle, `This connection cannot change projects in ${home.label || 'this environment'}.`);
    try {
      if (home.environmentId === client.environmentId) {
        const access = client.restAccess(native), [commandId] = await access.ids(1);
        await access.request('projects.mutate', { type: 'project.update', commandId, projectId: home.projectId, ...input }, true);
      } else {
        const access = envAccess(client, native, home.environmentId, source);
        if (!access) throw new ClientError('That environment is no longer connected.');
        const [commandId] = await client.restAccess(native).ids(1);
        await access.request('projects.mutate', { type: 'project.update', commandId, projectId: home.projectId, ...input });
      }
    } catch (error) {
      if (letGo(error)) throw error;
      return fail(members.length > 1 ? `${failureTitle} on ${home.label || 'the current environment'}` : failureTitle, error instanceof Error ? error.message : 'An error occurred.');
    }
  }
  return true;
}

/** useProjectPin setProjectGroupPinned. */
export const setProjectGroupPinned = (client: T3Client, native: Native, members: readonly Member[], pinned: boolean, source: EnvironmentFleet = fleet) =>
  updateProjectGroup(client, native, members, { pinned }, pinned ? 'Failed to pin project' : 'Failed to unpin project', source);

/** keepOrder: `items` in `order` by key; items missing from `order` follow, in their own order. */
export function keepOrder<T>(items: readonly T[], order: readonly string[] | null, key: (item: T) => string): T[] {
  if (order === null) return [...items];
  const rank = new Map(order.map((itemKey, index) => [itemKey, index]));
  return items.map((item, index) => ({ item, rank: rank.get(key(item)) ?? order.length + index })).sort((a, b) => a.rank - b.rank).map(({ item }) => item);
}
/** withArrivedKeys: `order` with keys it has not seen added at the end, or `order` itself when nothing is new. */
export function withArrivedKeys(order: readonly string[] | null, keys: readonly string[]): readonly string[] | null {
  const known = new Set(order);
  const arrived = keys.filter(itemKey => !known.has(itemKey));
  return arrived.length === 0 ? order : [...(order ?? []), ...arrived];
}
/**
 * useOrderWhileOpen over a holder: while `open`, the order the list had when it opened, so a row a pin moves does not
 * slide out from under the pointer; rows that arrive later join at the end. Closed, it forgets that order.
 */
export function orderWhileOpen<T>(holder: { order: readonly string[] | null }, items: readonly T[], key: (item: T) => string, open: boolean): T[] {
  if (!open) { holder.order = null; return [...items]; }
  const ordered = keepOrder(items, holder.order, key);
  holder.order = withArrivedKeys(holder.order, ordered.map(key));
  return ordered;
}
