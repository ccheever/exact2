// T3 Code 9c161462c9 (MIT, LICENSE-T3), ComposerPromptEditorTiptap saveCitationDraft and
// ChatComposer saveCitationDraftToPrompt: retain a comment when a request replaces its editor.
import type { T3Client } from '../client/client';
import { str, type Obj } from '../client/domain';
import { requestPresentation } from '../composer/requests';
import { citationOfLink } from './citation-comment';
import { resolveAssistantCitationCommentDismissal, serializeAssistantCitation, withAssistantCitationComment } from './diff-citations';

type Draft = { seq: number; key: string; value: string; link: string; at: number; comment: string | null };
const drafts = new WeakMap<T3Client, Draft>();

export function registerCitationDraft(client: T3Client, press: Obj): void {
  const seq = Number(press.seq);
  const link = str(press.link);
  if (press.open !== true || press.kind !== 'citation' || press.surface !== 'composer' || !Number.isSafeInteger(seq)
    || str(press.owner) !== client.snapshotOwner || !link || !client.draft.includes(link) || !citationOfLink(link)) return;
  const first = client.draft.indexOf(link), at = typeof press.start === 'number' ? press.start : first;
  if (!Number.isSafeInteger(at) || at < 0 || client.draft.slice(at, at + link.length) !== link
    || (press.start === undefined && client.draft.indexOf(link, first + 1) >= 0)) return;
  const previous = drafts.get(client);
  drafts.set(client, { seq, key: client.draftKey, value: client.draft, link, at,
    comment: previous?.seq === seq && previous.key === client.draftKey ? previous.comment : null });
}

export function noteCitationDraft(client: T3Client, seq: number, comment: string): void {
  const draft = drafts.get(client);
  if (!draft || draft.seq !== seq || draft.key !== client.draftKey) return;
  draft.comment = comment;
  preserveCitationDraft(client);
}

export function clearCitationDraft(client: T3Client, seq: number): void {
  if (drafts.get(client)?.seq === seq) drafts.delete(client);
}

/** Save only into the exact prompt that held the citation, without overwriting a newer edit or another thread. */
export function preserveCitationDraft(client: T3Client): boolean {
  const draft = drafts.get(client);
  if (!draft || draft.comment === null || draft.key !== client.draftKey || requestPresentation(client).requestMode === '') return false;
  if (client.draft !== draft.value) { drafts.delete(client); return false; }
  const citation = citationOfLink(draft.link);
  if (!citation) return false;
  const dismissal = resolveAssistantCitationCommentDismissal({ reason: 'none', draft: draft.comment, savedComment: citation.comment });
  if (dismissal.kind !== 'commit') return false;
  const link = serializeAssistantCitation(withAssistantCitationComment(citation, dismissal.comment));
  const value = draft.value.slice(0, draft.at) + link + draft.value.slice(draft.at + draft.link.length);
  client.local.drafts[draft.key] = value;
  drafts.set(client, { ...draft, value, link, comment: null });
  return true;
}
