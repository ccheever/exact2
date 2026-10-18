// The composer chips' other presses, adapted from T3 Code 9c161462c9 (MIT, see LICENSE-T3):
// composerContextPresentation.tsx (the registry's renderers and ComposerContextActions), contextChipParts.tsx
// (ContextChipPopover, PullRequestChip, FileChip, ImageChipButton), ComposerPromptEditorTiptap.tsx ComposerMentionNodeView,
// ThreadContextChip.tsx and chat/AssistantCitationChip.tsx, and ChatComposer.tsx's `composerContextActions`.
//
// A review comment and a preview annotation open their details in a ContextChipPopover above the chip, as a skill does
// (composer-chip-popover.ts); the window draws them (composer-chip-popover.contract). Every other chip's press is an
// action: a mention opens its file in the right panel (openMention: rightPanelStore.openFile), a thread opens it
// (ThreadContextChip's Link), a citation's source link opens its thread at the cited answer (assistantCitationNavigation),
// a pull request opens beside the thread or in the browser (useOpenPrLink), an image opens the expanded preview over the
// draft's images (buildExpandedImagePreview), a video its expanded player (expandVideo), and a file AttachmentFilePreview
// over the draft's bytes (openFile, composer-chip-file.ts). The native editor reports
// the action (T3ComposerChipPress.swift `act`); the window's `chipAct` task sends `editorlocal:chip-act` once per press.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { openFileSurface } from '../panel/panel-surfaces';
import { chatLocal } from '../timeline/timeline-presentation';
import { parseAssistantCitationHref } from '../timeline/diff-citations';
import { reviewCommentRecords } from './composer-editor';
import { contextId } from './composer-editor-menu';
import { draftFiles } from './composer-editor-files';
import { videoOp } from './composer-attachments';
import { openComposerFile } from './composer-chip-file';
import { openPullRequest } from '../pull-requests/pr-chip-open';
import { previewAnnotationContextLabel, type PreviewAnnotationPayload, type SavedAnnotation } from '../browser/browser-annotation';

/** What a context chip's details popover shows (the rest of composer-chip-popover.ts's ChipPopover). */
export type ChipDetails = { kind: string; label: string; title: string; description: string; lines: string; rows: number; diff: string; image: string; aspect: number };

/**
 * isPullRequestSummaryContext for a record without pull request metadata (a legacy `PR #N` summary): its chip is the
 * review comment's popover with the pull request glyph and "Pull request" as its kind.
 */
function legacyPullRequest(record: Obj): boolean {
  return str(record.sectionId).startsWith('pull-request:') && !str(record.diff).trim() && /^PR #\d+$/.test(str(record.filePath));
}

/**
 * ComposerReviewCommentDetails: the file path, "<section> · <range>", the comment and its diff. The trigger's name and
 * the popover's sr-only title are "<kind>, <label>" (ContextChip's `kindLabel + ", " + label`).
 */
export function reviewCommentDetails(record: Obj, label: string): ChipDetails | null {
  if (record.kind !== 'review-comment' || record.pullRequest !== undefined) return null;
  const kind = legacyPullRequest(record) ? 'Pull request' : 'Review comment', diff = str(record.diff).trim() ? str(record.diff) : '';
  return { kind: 'review-comment', label: str(record.filePath), title: `${kind}, ${label}`, description: str(record.text).trim(),
    lines: `${str(record.sectionTitle)} · ${str(record.rangeLabel)}`, rows: diff ? diff.split('\n').length : 0, diff, image: '', aspect: 0 };
}

/** previewAnnotationTooltip: the page, the comment and what it marks, each after a blank line. */
export function previewAnnotationSummary(annotation: PreviewAnnotationPayload): string {
  const lines = [annotation.pageTitle?.trim() || annotation.pageUrl];
  if (annotation.comment.trim()) lines.push('', annotation.comment.trim());
  const plural = (count: number, noun: string) => `${count} ${noun}${count === 1 ? '' : 's'}`;
  const targets: string[] = [];
  if (annotation.elements.length > 0) targets.push(plural(annotation.elements.length, 'element'));
  if (annotation.regions.length > 0) targets.push(plural(annotation.regions.length, 'region'));
  if (annotation.strokes.length > 0) targets.push(plural(annotation.strokes.length, 'drawing'));
  if (annotation.styleChanges.length > 0) targets.push(plural(annotation.styleChanges.length, 'style change'));
  if (targets.length > 0) lines.push('', targets.join(', '));
  return lines.join('\n');
}

/**
 * ComposerPreviewAnnotationDetails: the annotated crop (or "Screenshot unavailable") above the summary. The crop is the
 * draft image Annotate saved (`image`, its height over its width `aspect`; 0 when unknown).
 */
export function previewAnnotationDetails(entry: SavedAnnotation): ChipDetails {
  const label = previewAnnotationContextLabel(entry.annotation);
  return { kind: 'preview-annotation', label, title: `Preview annotation, ${label}`, description: previewAnnotationSummary(entry.annotation), lines: '', rows: 0, diff: '',
    image: /^[a-f0-9-]{36}$/i.test(entry.imageId) ? entry.imageId : '', aspect: entry.width && entry.height ? entry.height / entry.width : 0 };
}

/** The details for a context chip's press (`kind` as the editor names it, `record` the draft's record), or null. */
export function contextDetails(kind: string, record: Obj, label: string): ChipDetails | null {
  if (kind === 'review-comment') return reviewCommentDetails(record, label);
  if (kind === 'preview-annotation' && record.annotation && typeof record.annotation === 'object') return previewAnnotationDetails(record as unknown as SavedAnnotation);
  return null;
}

/** The draft's records a press can name, by context id (the ids are kind-scoped, so one map holds them all). */
export function draftContextRecords(client: T3Client): Obj {
  const local = obj(client.local);
  return { ...obj(local.terminalContexts), ...reviewCommentRecords(client), ...obj(local.previewAnnotations) };
}

/** The draft's images in prompt-independent order (buildExpandedImagePreview over `composerImages`). */
const images = new WeakMap<T3Client, { key: string; id: string } | null>();
/** The draft images' natural sizes in points, by id (a saved draft image never changes; ZoomableImage's `onLoad`). */
const imageSizes = new Map<string, { width: number; height: number }>();
export type ComposerImagePreview = { open: boolean; id: string; name: string; position: string; many: boolean; width: number; height: number };
export function composerImagePreview(client: T3Client): ComposerImagePreview[] {
  const open = images.get(client);
  if (!open || open.key !== client.draftKey) return [];
  const list = client.snapshotDrafts, index = list.findIndex(image => str(image.id) === open.id);
  if (index < 0) return [];
  const size = imageSizes.get(open.id) ?? { width: 0, height: 0 };
  return [{ open: true, id: open.id, name: str(list[index]!.name, 'image'), position: list.length > 1 ? `(${index + 1}/${list.length})` : '', many: list.length > 1, ...size }];
}
/** Reads the sizes of the draft's images the module has not measured yet; an image it cannot read stays unknown (0). */
export async function measureDraftImages(client: T3Client, native: Native): Promise<void> {
  for (const image of client.snapshotDrafts) {
    const id = str(image.id);
    if (!id || imageSizes.has(id)) continue;
    try {
      const size = await client.call(native, { op: 'snapshotDraftSize', id });
      const width = Number(size.width), height = Number(size.height);
      if (width > 0 && height > 0) imageSizes.set(id, { width, height });
    } catch { /* unknown: the preview keeps the whole media box */ }
  }
}
/** `editorlocal:chip-image-*`: ExpandedImageDialog's close and its wrap-around steps. */
export function composerImageStep(client: T3Client, op: string): string {
  const open = images.get(client);
  if (op === 'chip-image-close' || !open) { images.set(client, null); return ''; }
  const list = client.snapshotDrafts, index = list.findIndex(image => str(image.id) === open.id);
  if (index < 0 || list.length === 0) { images.set(client, null); return ''; }
  const next = list[(((index + (op === 'chip-image-previous' ? -1 : 1)) % list.length) + list.length) % list.length]!;
  images.set(client, { key: open.key, id: str(next.id) });
  return '';
}

/** One action press (the editor's `editorChip` reply while it holds it). */
export async function performChipAct(client: T3Client, native: Native, press: Obj): Promise<string> {
  const act = str(press.act), context = str(press.context);
  if (act === 'mention') { if (str(press.path)) await openFileSurface(client, native, str(press.path), 0); return ''; }
  if (act === 'thread') {
    const threadId = str(client.shell.threads.find(thread => contextId('thread', str(thread.id)) === context)?.id);
    return chatLocal(client, native, 'chip-thread', threadId, '');
  }
  if (act === 'citation') {
    const href = /\]\((t3-citation:\/\/[^\s)]+)\)$/.exec(str(press.link))?.[1] ?? '';
    const citation = href ? parseAssistantCitationHref(href) : null;
    if (!citation) throw new ClientError('That citation is no longer available.');
    return chatLocal(client, native, 'chip-citation', citation.threadId, citation.messageId);
  }
  if (act === 'pull-request' || act.startsWith('pr-')) {
    const url = str(obj(obj(reviewCommentRecords(client)[context]).pullRequest).url);
    if (!url) throw new ClientError('That pull request is no longer available.');
    await openPullRequest(client, native, url);
    return '';
  }
  if (act === 'image') {
    const id = context.startsWith('image_') ? context.slice('image_'.length) : '';
    if (!client.snapshotDrafts.some(image => str(image.id) === id)) throw new ClientError('That image is no longer attached.');
    await measureDraftImages(client, native);
    images.set(client, { key: client.draftKey, id });
    return '';
  }
  if (act === 'file') { openComposerFile(client, context); return ''; }
  if (act === 'video') {
    const file = draftFiles(client.local).find(entry => entry.contextId === context && entry.draftKey === client.draftKey);
    if (!file) throw new ClientError('That video is no longer attached.');
    return videoOp(client, native, 'shelf-video-play', file.id);
  }
  return '';
}

/** `editorlocal:chip-act`: the press the window saw (`seq`), taken from the editor once, then performed. */
export async function chipAct(client: T3Client, native: Native, seq: number): Promise<string> {
  let press: Obj = {};
  try { const reply = await bridgeReply(native, { op: 'editorChip' }); press = reply.ok ? obj(reply.value) : {}; } catch { return ''; }
  if (Number(press.seq) !== seq || press.open === true || !str(press.act)) return '';
  await bridgeReply(native, { op: 'editorChipClose', seq }).catch(() => undefined);
  // The Settings prompt sample has no actions (SettingsFontPreviews renders no ComposerContextActionsContext); a press
  // from another draft's composer is stale.
  if (str(press.surface) !== 'composer' || (str(press.owner) && str(press.owner) !== client.snapshotOwner)) return '';
  return performChipAct(client, native, press);
}
