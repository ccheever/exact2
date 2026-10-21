import { requestComposerField } from '../composer/composer-field-request';
// An assistant citation's comment, adapted from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// AssistantCitationChip.tsx (the chip's PopoverTrigger "Add comment to citation" / "Edit citation comment" and its
// popover "Edit citation comment"), AssistantCitationCommentEditor.tsx (Enter saves, Command/Ctrl+Enter saves and sends,
// Escape and Cancel cancel), ComposerPromptEditorTiptap.tsx ComposerCitationNodeView (onSave rewrites the chip with
// withAssistantCitationComment; after Cite, Cancel removes the chip: `removeOnCancel`; onRestoreFocus puts the caret
// after the chip) and ChatComposer.tsx (citeAssistantText opens it under the cited text; submitCitationAndSend).
// Shared field facts identify the chip and measured anchor; the root draws its comment popover.
import { obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { composerFieldOwner } from '../composer/auto-balance-owner';
import { composerFieldCurrent } from '../composer/composer-field-bridge';
import { composerChipCurrent, composerCitationNoEdit, composerCitationPrepare } from '../composer/composer-chip-bridge';
import { composerChipStore } from '../composer/composer-chip-store';
import { composerCitationHref, insertCitation } from '../composer/composer-source-chips';
import type { ComposerEdit } from '../composer/composer-markdown-edit';
import type { ComposerCitationEffect } from '../composer/composer-citation-receipts';
import type { ComposerFieldSnapshot } from '../composer/composer-field-commands';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { ASSISTANT_CITATION_MAX_COMMENT_LENGTH, parseAssistantCitationHref, serializeAssistantCitation, withAssistantCitationComment, type AssistantCitation } from './diff-citations';
import type { ChipPopover } from '../composer/composer-chip-popover';

/** The citation a composer chip's source link holds, or null. */
export function citationOfLink(link: string) {
  const href = composerCitationHref(link);
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

/** Queue one exact source edit. Opening, closing and sending happen only in
 *  the independent field ACK, even if this caller is no longer waiting. */
async function editCitation(client: T3Client, native: Native, current: ComposerFieldSnapshot, edit: ComposerEdit, effect: ComposerCitationEffect, focus: boolean): Promise<boolean> {
  const expected = current.value.slice(0, edit.start) + edit.text + current.value.slice(edit.end);
  const receipt = composerCitationPrepare(client, current.value, expected, effect);
  if (!receipt) return false;
  try {
    const reply = await requestComposerField(native, { op: 'editorEdit', snapshotOwner: current.snapshotOwner, fieldOwner: current.owner, value: current.value,
      start: edit.start, end: edit.end, expect: current.value.slice(edit.start, edit.end), text: edit.text,
      caret: edit.selection.end, focus, citation: receipt.citation });
    if (!reply.accepted) throw new ClientError('The prompt could not be updated.');
    return true;
  } catch (error) {
    composerChipStore(client).citations.refuse(receipt);
    throw error;
  }
}

/** Save/send replace the current chip; fresh Cancel removes its owned spaces.
 *  Root focus facts distinguish comment Cancel from an Escape that only closes. */
export async function citationCommentLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  if (!['chip-cite-save', 'chip-cite-send', 'chip-cite-commit', 'chip-cite-cancel'].includes(op)) return '';
  const seq = Number(id), press = composerChipCurrent(client), current = composerFieldCurrent(client, composerFieldOwner(client));
  if (!Number.isSafeInteger(seq) || seq <= 0 || !press?.open || press.kind !== 'citation' || press.surface !== 'composer'
    || press.seq !== seq || !current || current.composing || press.owner !== current.snapshotOwner || press.fieldOwner !== current.owner
    || current.value.slice(press.start, press.end) !== press.link) return '';
  const send = op === 'chip-cite-send', focus = op !== 'chip-cite-commit';
  let edit: ComposerEdit;
  if (op === 'chip-cite-cancel') {
    if (!press.removeOnCancel) {
      composerCitationNoEdit(client, seq, { send: false, focus: 'after' });
      return '';
    }
    const cancellation = composerChipStore(client).presses.cancel(current, seq);
    if (!cancellation) return '';
    edit = cancellation;
  } else {
    if (value.length > ASSISTANT_CITATION_MAX_COMMENT_LENGTH) throw new ClientError(`Comments can contain up to ${ASSISTANT_CITATION_MAX_COMMENT_LENGTH.toLocaleString('en-US')} characters.`);
    const citation = citationOfLink(press.link);
    if (!citation) throw new ClientError('This citation can no longer be edited.');
    const text = serializeAssistantCitation(withAssistantCitationComment(citation, value));
    if (text === press.link) {
      composerCitationNoEdit(client, seq, { send, focus: focus ? 'after' : 'outside' });
      return '';
    }
    edit = { start: press.start, end: press.end, text, selection: { start: press.start + text.length, end: press.start + text.length } };
  }
  await editCitation(client, native, current, edit, { kind: 'finish', seq, send }, focus);
  return '';
}

/** Cite inserts once at the selection start and lets its independent input ACK
 *  open the fresh comment under the measured source, without a caller callback. */
export async function openCitationComment(client: T3Client, native: Native, href: string, source: number[]): Promise<boolean> {
  const citation = parseAssistantCitationHref(href), current = composerFieldCurrent(client, composerFieldOwner(client));
  if (!citation || !current || current.composing || source.length !== 4 || !source.every(Number.isFinite)
    || !(source[2]! > 0) || !(source[3]! > 0)) return false;
  const inserted = insertCitation(current.value, current.start, serializeAssistantCitation(citation));
  return editCitation(client, native, current, inserted.edit, { kind: 'open', fresh: inserted.citation,
    sourceFrame: [source[0]!, source[1]!, source[2]!, source[3]!] }, true);
}
