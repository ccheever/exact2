// @ref llp/1106.006-review-and-files.decision.md#ownership
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { ClientError, type Native, type Files } from './shared/protocol';
import { mobileDraftChanged } from './draft';
import { mobileSessionGrants } from './environment-detail';
import { workspaceOf } from './shared/r4-surfaces-panel';

/** A screen answer belongs to a connection, workspace and draft, including reconnects. */
export function reviewOwner(client: T3Client): string {
  return JSON.stringify([client.origin, client.environmentId, client.projectId, client.threadId,
    client.generation, client.threadEpoch, workspaceOf(client).cwd]);
}
export function assertReviewOwner(client: T3Client, owner: string) {
  if (reviewOwner(client) !== owner) throw new ClientError('The workspace changed. Open this screen again.', 'superseded');
}
export function reviewNative(client: T3Client, native: Native, owner = reviewOwner(client)): Native {
  return { available: native.available, watch: topic => native.watch(topic), later: async input => {
    assertReviewOwner(client, owner);
    const result = await native.later(input);
    assertReviewOwner(client, owner);
    return result;
  } };
}
/** Explicit [] permissions deny host reads even if legacy scopes still contain a grant. */
export async function reviewFileAccess(client: T3Client, native: Native): Promise<boolean> {
  const session = await client.http(native, '/api/auth/session');
  return mobileSessionGrants(session, 'filesystem:read');
}
/** Shared records stay in composer-editor. The current plain mobile input appends a chip
 * through the synchronous shared draft reducer; it has no native rich-editor caret yet.
 */
export function reviewComposerNative(client: T3Client, native: Native, storage: Files, owner: string): Native {
  const guarded = reviewNative(client, native, owner);
  return { ...guarded, later: async input => {
    assertReviewOwner(client, owner);
    const request = obj(input);
    if (request.op !== 'editorInsert' && !(request.op === 'editorEdit' && request.all === true)) return guarded.later(input);
    const text = request.op === 'editorInsert' ? client.draft + str(request.text) : str(request.text);
    const answer = await mobileDraftChanged(client, text, guarded, storage);
    assertReviewOwner(client, owner);
    if (answer.message) throw new ClientError(answer.message);
    return { ok: true, generation: client.generation, value: { applied: true, text } };
  } };
}
