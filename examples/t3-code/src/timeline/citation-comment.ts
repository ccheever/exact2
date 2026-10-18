// An assistant citation's comment, adapted from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// AssistantCitationChip.tsx (the chip's PopoverTrigger "Add comment to citation" / "Edit citation comment" and its
// popover "Edit citation comment"), AssistantCitationCommentEditor.tsx (Enter saves, Command/Ctrl+Enter saves and sends,
// Escape and Cancel cancel), ComposerPromptEditorTiptap.tsx ComposerCitationNodeView (onSave rewrites the chip with
// withAssistantCitationComment; after Cite, Cancel removes the chip: `removeOnCancel`; onRestoreFocus puts the caret
// after the chip) and ChatComposer.tsx (citeAssistantText opens it under the cited text; submitCitationAndSend).
// The chip is drawn by the native composer, which reports its comment button's press, or Cite's opening, with the
// anchor's window frame (T3ComposerChipPress.swift); the window draws the popover there (citation-comment.contract).
import { obj, str, type Obj } from '../client/domain';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { ASSISTANT_CITATION_MAX_COMMENT_LENGTH, parseAssistantCitationHref, serializeAssistantCitation, withAssistantCitationComment, type AssistantCitation } from './diff-citations';
import type { ChipPopover } from '../composer/composer-chip-popover';

const LINK = /^\[Assistant quote\]\((t3-citation:\/\/v1\/[^\s)]+)\)$/;

/** The citation a composer chip's source link holds, or null. */
export function citationOfLink(link: string) {
  const href = LINK.exec(link)?.[1];
  return href ? parseAssistantCitationHref(href) : null;
}

const numbers = (value: unknown) => Array.isArray(value) ? (value as unknown[]).map(Number) : [];

type Cited = Pick<AssistantCitation, 'environmentId' | 'threadId' | 'messageId' | 'start' | 'end'>;
const citedKey = (citation: Cited) => JSON.stringify([citation.environmentId, citation.threadId, citation.messageId, citation.start, citation.end]);
const cited = new WeakMap<object, Map<string, number>>();
/** Cite's measure of where the quote starts in its answer's text stream (the selection's raw start), comment aside. */
export function rememberCitedStart(owner: object, citation: Cited, rawStart: number): void {
  const starts = cited.get(owner) ?? new Map<string, number>();
  starts.delete(citedKey(citation)); starts.set(citedKey(citation), rawStart);
  while (starts.size > 64) starts.delete(starts.keys().next().value!);
  cited.set(owner, starts);
}
export function citedStart(owner: object, citation: Cited): number { return cited.get(owner)?.get(citedKey(citation)) ?? -1; }

/** A citation chip's open comment, as the window's `chip` resource shows it; `startOf` is Cite's measure of its quote. */
export function citationPopover(press: Obj, base: ChipPopover, startOf: (citation: Cited) => number = () => -1): ChipPopover {
  const citation = citationOfLink(str(press.link));
  const frame = numbers(press.frame), anchor = numbers(press.anchorFrame);
  return {
    ...base, seq: Number(press.seq) || 0, open: true, surface: str(press.surface), kind: 'citation', label: str(press.label),
    // The popup's own name (PopoverPopup aria-label); the chip's comment, the textarea's starting value.
    title: 'Edit citation comment', comment: citation?.comment ?? '',
    x: frame[0] || 0, y: frame[1] || 0, width: frame[2] || 0, height: frame[3] || 0,
    anchor: str(press.anchor) === 'source' ? 'source' : 'chip', ax: anchor[0] || 0, ay: anchor[1] || 0, aw: anchor[2] || 0, ah: anchor[3] || 0,
    // The cited text stays marked in its answer while the comment is open (AssistantCitationSource.tsx); the answer's row
    // carries the turn item and the citation the message, so both compare without their kind prefix (citation-source.ts).
    message: citation ? citation.messageId.replace(/^(?:turn-item|message):/, '') : '', quote: citation?.text ?? '', at: citation ? startOf(citation) : -1,
  };
}

/**
 * The popover's Save (`chip-cite-save`; `chip-cite-send` saves and sends; `chip-cite-commit` is an outside press
 * committing a changed draft, which leaves the focus where the press put it unless it put it nowhere: then the prompt takes
 * it after the chip, PopoverPopup's `finalFocus`), Cancel (`chip-cite-cancel`) and the window's Escape (`chip-cite-escape`:
 * the editor's Cancel in its textarea, else Base UI's escape-key dismissal, which keeps the chip; the native editor knows
 * where the focus is): `id` is the press's sequence, `value` the comment.
 */
export async function citationCommentLocal(native: Native, op: string, id: string, value: string): Promise<string> {
  const seq = Number(id) || 0;
  const reply = await bridgeReply(native, { op: 'editorChip' });
  const press = reply.ok ? obj(reply.value) : {};
  if (press.open !== true || str(press.kind) !== 'citation' || Number(press.seq) !== seq) return '';
  let text: string | undefined;
  if (op === 'chip-cite-cancel' || op === 'chip-cite-escape') text = press.removeOnCancel === true ? '' : undefined;
  else {
    if (value.length > ASSISTANT_CITATION_MAX_COMMENT_LENGTH) throw new ClientError(`Comments can contain up to ${ASSISTANT_CITATION_MAX_COMMENT_LENGTH.toLocaleString('en-US')} characters.`);
    const citation = citationOfLink(str(press.link));
    if (!citation) throw new ClientError('This citation can no longer be edited.');
    text = serializeAssistantCitation(withAssistantCitationComment(citation, value));
  }
  const finish = await bridgeReply(native, { op: 'editorChipFinish', seq, ...(text === undefined ? {} : { text }), focus: op !== 'chip-cite-commit', send: op === 'chip-cite-send', ...(op === 'chip-cite-escape' ? { escape: true } : {}) });
  if (!finish.ok || obj(finish.value).applied !== true) throw new ClientError('The prompt could not be updated.');
  return '';
}

/**
 * Cite's second half (ChatComposer citeAssistantText → requestCitationComment): the chip just inserted opens its
 * comment under the cited text, `source` the text's window frame [x, top, width, height] as the answer measured it.
 */
export async function openCitationComment(native: Native, href: string, source: number[]): Promise<boolean> {
  const reply = await bridgeReply(native, { op: 'editorCiteOpen', href, source: source.length === 4 && source.every(Number.isFinite) ? source : [] });
  return reply.ok && obj(reply.value).opened === true;
}
