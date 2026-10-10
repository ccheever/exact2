import { mobileComposerTargetRequire, mobileComposerTargetCurrent, mobileComposerTargetText } from './composer-target';
// @ref llp/1109.006-review-and-files.decision.md#ownership
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
/** A comment selection binds its content target separately from read-only diff ownership. */
export function assertReviewComposerOwner(client: T3Client, owner: string) {
  mobileComposerTargetRequire(client, owner);
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
  const guarded = reviewNative(client, native, owner), target = mobileComposerTargetRequire(client);
  return { ...guarded, later: async input => {
    assertReviewOwner(client, owner);
    const request = obj(input);
    if (request.op !== 'editorInsert' && !(request.op === 'editorEdit' && request.all === true)) return guarded.later(input);
    if (!mobileComposerTargetCurrent(client, target)) throw new ClientError('The composer changed.', 'superseded');
    const text = request.op === 'editorInsert' ? (mobileComposerTargetText(client, target) ?? '') + str(request.text) : str(request.text);
    const answer = await mobileDraftChanged(client, text, guarded, storage, target.owner);
    assertReviewOwner(client, owner);
    assertReviewComposerOwner(client, target.owner);
    if (answer.message) throw new ClientError(answer.message);
    return { ok: true, generation: client.generation, value: { applied: true, text } };
  } };
}
