// Per-question attachment drafts, adapted from T3 Code 74541bab17 (MIT); see LICENSE-T3. Sources:
// apps/web/src/questionAttachments.ts (questionAttachmentDraftId, clearQuestionAttachmentDraft),
// chat/usePendingThreadRequests.ts (the release of resolved requests' drafts) and
// desktop/SnapShotCoordinator.tsx (resolveSnapShotAttachmentTarget). Each question of a pending request
// keeps its own image shelf in `snapshotDrafts`, beside the thread's draft; its answer uploads that
// shelf as the question's attachments. The keys are neither `env:<thread>` nor `env:new:<project>`, so
// the sidebar's draft rows never list them.
import { str, type Obj } from '../client/domain';

export const questionDraftPrefix = (environmentId: string, threadId: string): string => `${environmentId}:question:${threadId}:`;

export function questionDraftKey(environmentId: string, threadId: string, requestId: string, questionId: string): string {
  return `${questionDraftPrefix(environmentId, threadId)}${JSON.stringify([requestId, questionId])}`;
}

/**
 * clearQuestionAttachmentDraft for each of the thread's question drafts (`prefix`) that no pending question keeps:
 * the draft goes and its images' local copies are released. True when one went.
 */
export function releaseQuestionDrafts(drafts: Record<string, Obj[]>, releases: string[], prefix: string, retained: ReadonlySet<string>): boolean {
  let released = false;
  for (const key of Object.keys(drafts)) {
    if (!key.startsWith(prefix) || retained.has(key)) continue;
    for (const image of drafts[key] ?? []) releases.push(str(image.id));
    delete drafts[key];
    released = true;
  }
  return released;
}

// useQuestionAttachmentPreparation: files being staged per draft key (a large image is fitted natively first). A
// question whose shelf is still taking files is unanswered, so its answer cannot leave without them.
const staging = new WeakMap<object, Map<string, number>>();
export const attachmentsStaging = (client: object, key: string): number => staging.get(client)?.get(key) ?? 0;
/** Counts `key` as staging until the returned release runs; each change redraws (`revision`). */
export function holdStaging(client: { revision: number }, key: string): () => void {
  let counts = staging.get(client);
  if (!counts) staging.set(client, counts = new Map());
  const held = counts;
  held.set(key, (held.get(key) ?? 0) + 1); client.revision++;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    const left = (held.get(key) ?? 0) - 1;
    if (left > 0) held.set(key, left); else held.delete(key);
    client.revision++;
  };
}

// The requests whose answer is being submitted, from its first upload: their shelves take nothing more and give
// nothing up (activePendingIsResponding).
const submitting = new WeakMap<object, Set<string>>();
export const answerSubmitting = (client: object, requestId: string): boolean => submitting.get(client)?.has(requestId) === true;
/** Marks `requestId` submitting until the returned release runs. */
export function holdSubmitting(client: { revision: number }, requestId: string): () => void {
  let ids = submitting.get(client);
  if (!ids) submitting.set(client, ids = new Set());
  const held = ids;
  held.add(requestId); client.revision++;
  return () => { held.delete(requestId); client.revision++; };
}

/** A capture's first sighting pins the question open then (null: none); later sightings keep that pin. */
export function pinSnapshot(pins: Map<string, string | null>, id: string, open: string | null): void {
  if (!pins.has(id)) pins.set(id, open);
}

/**
 * resolveSnapShotAttachmentTarget: a capture is pinned to the question open when it was first seen (`open`, or null
 * when none was), and it joins that question only while the same question is still open; otherwise it joins
 * `threadKey`. Absence is pinned too, so a later question never claims an earlier capture.
 */
export function snapshotAttachmentKey(pins: Map<string, string | null>, id: string, threadKey: string, open: string | null): string {
  pinSnapshot(pins, id, open);
  const pinned = pins.get(id);
  return pinned && pinned === open ? pinned : threadKey;
}
