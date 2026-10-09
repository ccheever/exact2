// browser-surface part 3 (capture): Annotate's result in the composer (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/desktop/src/preview/PickedElementPayload.ts, apps/web/src/lib/elementContext.ts `normalizeElementContextSelection`,
// apps/web/src/lib/composerContextRecords.ts `previewAnnotationContext*`, apps/web/src/lib/previewAnnotation.ts, and
// composerDraftStore.ts `addPreviewAnnotation`).
//
// The module's overlay (assets/browser-annotate.js, T3BrowserAnnotate.swift) hands back a PreviewAnnotationPayload and,
// when the crop worked, a PNG it wrote among the composer's draft images. This file validates the payload as the
// desktop's validator does (a page can reach the overlay's DOM, so nothing it sends is trusted), keeps a compact copy
// beside the drafts (as terminal contexts are kept, terminal-integrations.ts), puts the annotation's chip in the
// composer (`[label](t3-context://v1/preview-annotation/<id>)`) and the crop on the shelf, and builds the
// `preview-annotation` record a send carries. The sent chip is drawn by r4-timeline-chips.ts.
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import { contextId, contextLabel, contextLink, contextReferences } from './composer-editor-menu';

// ── PickedElementPayload.ts: the strict structural validators ──────────────────────────────────
const isStringOrNull = (value: unknown): value is string | null => value === null || typeof value === 'string';
const isFiniteNumberOrNull = (value: unknown): value is number | null => value === null || (typeof value === 'number' && Number.isFinite(value));
function isPickedStackFrame(value: unknown): boolean {
  if (typeof value !== 'object' || value === null) return false;
  const frame = value as Record<string, unknown>;
  return isStringOrNull(frame.functionName) && isStringOrNull(frame.fileName) && isFiniteNumberOrNull(frame.lineNumber) && isFiniteNumberOrNull(frame.columnNumber);
}
export type PickedStackFrame = { functionName: string | null; fileName: string | null; lineNumber: number | null; columnNumber: number | null };
export type PickedElementPayload = {
  pageUrl: string; pageTitle: string | null; tagName: string; selector: string | null; htmlPreview: string; componentName: string | null;
  source: PickedStackFrame | null; stack: PickedStackFrame[]; styles: string; pickedAt: string;
};
export type AnnotationRect = { x: number; y: number; width: number; height: number };
export type PreviewAnnotationPayload = {
  id: string; pageUrl: string; pageTitle: string | null; comment: string;
  elements: Array<{ id: string; element: PickedElementPayload; rect: AnnotationRect }>;
  regions: Array<{ id: string; rect: AnnotationRect }>;
  strokes: Array<{ id: string; color: string; width: number; points: Array<{ x: number; y: number }>; bounds: AnnotationRect }>;
  styleChanges: Array<{ targetId: string; selector: string | null; property: string; previousValue: string; value: string }>;
  screenshot: null | { dataUrl: string; width: number; height: number; cropRect: AnnotationRect };
  createdAt: string;
};

export function isPickedElementPayload(value: unknown): value is PickedElementPayload {
  if (typeof value !== 'object' || value === null) return false;
  const c = value as Record<string, unknown>;
  if (typeof c.pageUrl !== 'string') return false;
  if (typeof c.tagName !== 'string') return false;
  if (typeof c.htmlPreview !== 'string') return false;
  if (typeof c.styles !== 'string') return false;
  if (typeof c.pickedAt !== 'string') return false;
  if (!isStringOrNull(c.pageTitle)) return false;
  if (!isStringOrNull(c.selector)) return false;
  if (!isStringOrNull(c.componentName)) return false;
  if (c.source !== null && !isPickedStackFrame(c.source)) return false;
  if (!Array.isArray(c.stack)) return false;
  return c.stack.every(isPickedStackFrame);
}
function isRect(value: unknown): boolean {
  if (typeof value !== 'object' || value === null) return false;
  const rect = value as Record<string, unknown>;
  return ['x', 'y', 'width', 'height'].every(key => typeof rect[key] === 'number' && Number.isFinite(rect[key]));
}
function isPoint(value: unknown): boolean {
  if (typeof value !== 'object' || value === null) return false;
  const point = value as Record<string, unknown>;
  return typeof point.x === 'number' && Number.isFinite(point.x) && typeof point.y === 'number' && Number.isFinite(point.y);
}
export function isPreviewAnnotationPayload(value: unknown): value is PreviewAnnotationPayload {
  if (typeof value !== 'object' || value === null) return false;
  const annotation = value as Record<string, unknown>;
  if (typeof annotation.id !== 'string') return false;
  if (typeof annotation.pageUrl !== 'string') return false;
  if (!isStringOrNull(annotation.pageTitle)) return false;
  if (typeof annotation.comment !== 'string') return false;
  if (typeof annotation.createdAt !== 'string') return false;
  if (annotation.screenshot !== null) return false;
  const { elements, regions, strokes, styleChanges } = annotation;
  if (!Array.isArray(elements) || !elements.every(entry => {
    if (typeof entry !== 'object' || entry === null) return false;
    const target = entry as Record<string, unknown>;
    return typeof target.id === 'string' && isPickedElementPayload(target.element) && isRect(target.rect);
  })) return false;
  if (!Array.isArray(regions) || !regions.every(entry => {
    if (typeof entry !== 'object' || entry === null) return false;
    const target = entry as Record<string, unknown>;
    return typeof target.id === 'string' && isRect(target.rect);
  })) return false;
  if (!Array.isArray(strokes) || !strokes.every(entry => {
    if (typeof entry !== 'object' || entry === null) return false;
    const target = entry as Record<string, unknown>;
    return typeof target.id === 'string' && typeof target.color === 'string' && typeof target.width === 'number' && Number.isFinite(target.width)
      && Array.isArray(target.points) && target.points.every(isPoint) && isRect(target.bounds);
  })) return false;
  if (!Array.isArray(styleChanges)) return false;
  return styleChanges.every(entry => {
    if (typeof entry !== 'object' || entry === null) return false;
    const change = entry as Record<string, unknown>;
    return typeof change.targetId === 'string' && isStringOrNull(change.selector) && typeof change.property === 'string'
      && typeof change.previousValue === 'string' && typeof change.value === 'string';
  });
}

// ── elementContext.ts and composerContextRecords.ts ──────────────────────────────────────────────
const ELEMENT_CONTEXT_HTML_PREVIEW_LIMIT = 4000;
const ELEMENT_CONTEXT_STYLES_LIMIT = 4000;
const PREVIEW_LABEL_MAX_CHARS = 48;
const truncate = (value: string, limit: number) => (value.length <= limit ? value : `${value.slice(0, Math.max(0, limit - 1))}…`);
const normalizeText = (value: string) => value.replace(/\r\n/g, '\n').replace(/^\n+|\n+$/g, '');
export type ElementContextDetails = {
  pageUrl: string; pageTitle: string | null; tagName: string; selector: string | null; htmlPreview: string; componentName: string | null;
  source: PickedStackFrame | null; styles: string;
};
/** normalizeElementContextSelection: trimmed and clamped, the first stack frame standing in for a missing source. */
export function normalizeElementContextSelection(raw: PickedElementPayload): ElementContextDetails | null {
  const pageUrl = raw.pageUrl.trim(), tagName = raw.tagName.trim().toLowerCase();
  if (!pageUrl || !tagName) return null;
  const frame = raw.source ?? raw.stack[0] ?? null;
  return {
    pageUrl, pageTitle: raw.pageTitle?.trim() ?? null, tagName, selector: raw.selector?.trim() || null,
    htmlPreview: truncate(normalizeText(raw.htmlPreview), ELEMENT_CONTEXT_HTML_PREVIEW_LIMIT), componentName: raw.componentName?.trim() || null,
    source: frame ? { functionName: frame.functionName?.trim() || null, fileName: frame.fileName?.trim() || null, lineNumber: frame.lineNumber ?? null, columnNumber: frame.columnNumber ?? null } : null,
    styles: truncate(normalizeText(raw.styles), ELEMENT_CONTEXT_STYLES_LIMIT),
  };
}
/** previewAnnotationContextLabel: the comment (48 characters), else the page title, else "Preview annotation". */
export function previewAnnotationContextLabel(annotation: Pick<PreviewAnnotationPayload, 'comment' | 'pageTitle'>): string {
  const comment = annotation.comment.trim().replace(/\s+/g, ' ');
  if (comment) return comment.length > PREVIEW_LABEL_MAX_CHARS ? `${comment.slice(0, PREVIEW_LABEL_MAX_CHARS - 1)}…` : comment;
  return annotation.pageTitle?.trim() || 'Preview annotation';
}
export const previewAnnotationContextId = (annotationId: string) => contextId('preview-annotation', annotationId);
export function previewAnnotationContextReference(annotation: PreviewAnnotationPayload): { kind: string; contextId: string; label: string } {
  return { kind: 'preview-annotation', contextId: previewAnnotationContextId(annotation.id), label: previewAnnotationContextLabel(annotation) };
}
function targetSummary(annotation: PreviewAnnotationPayload): string {
  const parts: string[] = [], plural = (count: number, noun: string) => `${count} ${noun}${count === 1 ? '' : 's'}`;
  if (annotation.elements.length > 0) parts.push(plural(annotation.elements.length, 'selected element'));
  if (annotation.regions.length > 0) parts.push(plural(annotation.regions.length, 'marked region'));
  if (annotation.strokes.length > 0) parts.push(plural(annotation.strokes.length, 'drawing'));
  return parts.join(', ');
}
/** previewAnnotationContextRecord: what a send carries for the chip (the crop travels as its own image record). */
export function previewAnnotationContextRecord(annotation: PreviewAnnotationPayload, options?: { screenshotContextId?: string }): Obj {
  const targets = annotation.elements.flatMap(target => {
    const element = normalizeElementContextSelection(target.element);
    return element ? [{ id: target.id, element }] : [];
  });
  return {
    version: 1, contextId: previewAnnotationContextId(annotation.id), kind: 'preview-annotation',
    label: contextLabel(previewAnnotationContextLabel(annotation), 'preview-annotation'),
    annotationId: annotation.id, pageUrl: annotation.pageUrl, pageTitle: annotation.pageTitle, comment: annotation.comment.trim(), targetSummary: targetSummary(annotation),
    styleChanges: annotation.styleChanges.map(change => `${change.property}: ${change.previousValue || '(unset)'} → ${change.value}`),
    ...(targets.length > 0 ? { elements: targets.map(target => target.element), elementIds: targets.map(target => target.id) } : {}),
    styleChangeDetails: annotation.styleChanges.map(change => ({ ...change })),
    ...(annotation.regions.length > 0 ? { regionCount: annotation.regions.length } : {}),
    ...(annotation.strokes.length > 0 ? { strokeCount: annotation.strokes.length } : {}),
    ...(options?.screenshotContextId !== undefined ? { screenshotContextId: options.screenshotContextId } : {}),
  };
}

// ── The draft's annotations (composerDraftStore addPreviewAnnotation) ─────────────────────────────
/** The compact copy kept beside the drafts: the payload without a data URL, and the crop's draft image id. */
export type SavedAnnotation = { annotation: PreviewAnnotationPayload; imageId: string };
type Local = { previewAnnotations?: Record<string, SavedAnnotation> };
const saved = (client: T3Client): Record<string, SavedAnnotation> => (client.local as unknown as Local).previewAnnotations ?? {};

/** Keeps the annotation (and its crop's image id) by context id; its chip names it from the prompt. */
export function savePreviewAnnotation(client: T3Client, annotation: PreviewAnnotationPayload, imageId: string): string {
  const id = previewAnnotationContextId(annotation.id), local = client.local as unknown as Local;
  local.previewAnnotations = { ...saved(client), [id]: { annotation: { ...annotation, screenshot: null }, imageId } };
  return id;
}
/** The chip `addPreviewAnnotation` puts in the prompt. */
export const previewAnnotationLink = (annotation: PreviewAnnotationPayload) => contextLink('preview-annotation', previewAnnotationContextId(annotation.id), previewAnnotationContextLabel(annotation));

/** Saved annotations the prompt's chips name. */
export function referencedAnnotations(client: T3Client, text: string): SavedAnnotation[] {
  const records = saved(client);
  return contextReferences(text).flatMap(reference => (reference.kind === 'preview-annotation' && records[reference.id] ? [records[reference.id]!] : []));
}
/** buildMessageContext's annotation records: the crop links its image record when the image went with the message. */
export function annotationMessageRecords(client: T3Client, text: string, sentImageIds: ReadonlySet<string> = new Set()): Obj[] {
  return referencedAnnotations(client, text).map(entry => previewAnnotationContextRecord(entry.annotation,
    entry.imageId && sentImageIds.has(entry.imageId) ? { screenshotContextId: contextId('image', entry.imageId) } : undefined));
}
/** The native editor's chip kinds: `preview-annotation/<id>` → its colour family (T3ComposerStyler palette). */
export function annotationChipContexts(client: T3Client, text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const entry of referencedAnnotations(client, text)) out[`preview-annotation/${previewAnnotationContextId(entry.annotation.id)}`] = 'preview-annotation';
  return out;
}
/** Keeps only the annotations a draft still names (adoptTerminalContexts' rule for a restored draft). */
export function adoptPreviewAnnotations(next: object, previous: Obj): void {
  const records: Record<string, SavedAnnotation> = {};
  for (const [id, value] of Object.entries(obj(previous.previewAnnotations))) {
    const entry = obj(value);
    if (isPreviewAnnotationPayload(entry.annotation) && previewAnnotationContextId(str(obj(entry.annotation).id)) === id) records[id] = { annotation: entry.annotation, imageId: str(entry.imageId) };
  }
  Object.assign(next, { previewAnnotations: records });
}
