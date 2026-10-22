// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/auto-balance-owner.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The composer's draft identity (task auto-balance; T3 Code MIT, see LICENSE-T3; reference
// 1e2ecbd975 apps/web/src/composerDraftStore.ts keys a draft by its draft id, so ChatView's
// setDraftThreadContext moving the draft's projectRef to the machine Auto balance chose leaves
// the composer's text, caret and focus alone).
// Here a draft is keyed by its machine and project, and app.contract's `composerOwner` tells the
// text typed in the window from a snapshot's (`composerText`). A draft that Auto balance moved
// keeps the owner it had, for as long as it stays the open draft: the window's typed text stays
// the composer's value, so the field is not rewritten and keeps its caret and focus.
import type { T3Client } from './client';

const aliases = new WeakMap<T3Client, { key: string; owner: string }>();

/** `${serverUrl}:${projectId}:${threadId}`, the owner of a draft that has not moved. */
const plainOwner = (client: T3Client): string => `${client.origin}:${client.projectId}:${client.threadId}`;

/** The snapshot's `composerOwner`: the moved draft's earlier owner while it is open, else the plain one. */
export function composerOwner(client: T3Client): string {
  const alias = aliases.get(client);
  if (alias && alias.key === client.draftKey) return alias.owner;
  if (alias) aliases.delete(client);
  return plainOwner(client);
}

/** After a move: the open draft (now at its new key) keeps `owner`, the one it had before. */
export function keepComposerOwner(client: T3Client, owner: string): void {
  aliases.set(client, { key: client.draftKey, owner });
}
