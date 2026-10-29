// Mermaid fences in the transcript, adapted from T3 Code 74541bab17 (MIT, see LICENSE-T3):
// apps/web/src/components/chat/MarkdownMermaidCodeBlock.tsx (a closed ```mermaid
// fence draws as a diagram, also while its reply streams) and chat/MermaidDiagram.tsx
// (pele, errors and retry). Each fence renders in the Mermaid page (assets/mermaid/,
// an `iframe` per fence, markdown.contract MermaidBlock) with the reference's pele
// build and options; the page reports the drawn diagram's size, which follows the
// lane's width, or pele's error (`diagram-frame`). This keeps the reports per theme,
// surface and source. The transcript and a rendered Markdown file in Files draw them.
import { arr, obj, str } from '../client/domain';
import type { T3Client } from '../client/client';

/** What one theme's page reported: "loading" until it answers, then "rendered" (the drawn size) or "error". */
export interface MermaidSize { drawnFor: number; width: number; height: number }
export interface MermaidFrame { diagram: string; message: string; retryable: boolean; width: number; height: number; sizes: MermaidSize[] }
/** mermaid-frame.contract MermaidDiagram: one settled fence, matched to its parsed block by its text, in both themes. */
export interface MermaidDiagramView { id: string; code: string; srcLight: string; srcDark: string; light: MermaidFrame; dark: MermaidFrame }
export const MERMAID_FONT = '-apple-system, "system-ui", "Segoe UI", system-ui, sans-serif';
/** The Mermaid page under the app's assets (LLP 1020.001: served at exact-app://app/ on Apple). */
export const MERMAID_PAGE = 'assets/mermaid/index.html';
const LOADING: MermaidFrame = Object.freeze({ diagram: 'loading', message: '', retryable: false, width: 0, height: 0, sizes: [] }) as MermaidFrame;
/** Widths kept per report: one fence drawn at several widths (a resized window) keeps each one's height. */
const MAX_SIZES = 8;
/** Reports kept, oldest dropped first. */
const MAX_CACHED_RENDERS = 64;

type Theme = 'light' | 'dark';
/**
 * Where a page draws: the transcript's lane (`chat`), a rendered file in Files (`file`) or the expanded preview.
 * Pele lays a diagram out for its page's width, so each surface keeps its own report; the preview takes the size
 * its opener's page drew and ignores its own page's reports.
 */
export type MermaidView = 'chat' | 'file' | 'preview';
const key = (theme: Theme, view: string, code: string) => `${theme}\n${view}\n${code.trim()}`;
/** Reported renders by `theme\nview\nsource`, oldest first. */
const reports = new Map<string, MermaidFrame>();
/** Retries asked per `theme\nsource`: each one is a new page load on every surface. */
const attempts = new Map<string, number>();
const retryKey = (theme: Theme, code: string) => `${theme}\n${code.trim()}`;

const fence = /^( {0,3})(`{3,}|~{3,})([^\n`]*)\n([\s\S]*?)\n? {0,3}\2[`~]*[ \t]*(?:\n|$)/gm;
/** The code of every closed ```mermaid fence, as the parsed block's text: a diagram draws once its fence closes
 *  (MarkdownMermaidCodeBlock), where messageCodeBlocks takes a fence still open too, as code. */
export function mermaidFences(markdown: string): string[] {
  if (!markdown.includes('mermaid')) return [];
  const found: string[] = [];
  for (const match of markdown.matchAll(fence)) {
    if ((match[3]!.trim().split(/\s+/)[0] ?? '').toLowerCase() !== 'mermaid') continue;
    const indent = match[1]!.length;
    const code = (indent ? match[4]!.split('\n').map(line => line.replace(new RegExp(`^ {0,${indent}}`), '')).join('\n') : match[4]!).replace(/\n+$/, '');
    if (code.trim() && !found.includes(code)) found.push(code);
  }
  return found;
}

/** The page for one fence in one theme on one surface; a retry's attempt number makes it a new load. */
export function mermaidSrc(code: string, theme: Theme, view: MermaidView = 'chat', width = 0): string {
  const params = new URLSearchParams({ theme, font: MERMAID_FONT, source: code.trim(), view });
  // The preview's page lays the diagram out for the width its opener drew it at, and the SVG scales to its box.
  if (width > 0) params.set('width', String(width));
  const attempt = attempts.get(retryKey(theme, code)) ?? 0;
  if (attempt) params.set('attempt', String(attempt));
  return `${MERMAID_PAGE}#${params.toString()}`;
}

/** A fence's frame in one theme on one surface as its page reported it, or loading. */
export function frameFor(code: string, theme: Theme, view: MermaidView = 'chat'): MermaidFrame {
  return reports.get(key(theme, view, code)) ?? LOADING;
}

/** A fence as the view draws it: both themes' pages and what each reported. */
export function diagramFor(id: string, code: string, view: MermaidView = 'chat'): MermaidDiagramView {
  return { id, code, srcLight: mermaidSrc(code, 'light', view), srcDark: mermaidSrc(code, 'dark', view), light: frameFor(code, 'light', view), dark: frameFor(code, 'dark', view) };
}

/**
 * A message's closed Mermaid fences as diagrams, also while it streams (isClosedCodeFence): `mermaidFences` matches
 * only a fence with its closer, so the fence still streaming stays a code block.
 */
export function messageDiagrams(markdown: string): MermaidDiagramView[] {
  return mermaidFences(markdown).map((code, index) => diagramFor(String(index), code));
}

/**
 * `diagram-frame`: a Mermaid page's report, the JSON it posts to its parent (assets/mermaid/render.js). A report for a
 * source and theme no fence asked for still lands: the page renders only what its fragment names.
 */
export function noteMermaidFrame(payload: string): string {
  let report;
  try { report = obj(JSON.parse(payload)); } catch { return ''; }
  const theme: Theme = report.theme === 'dark' ? 'dark' : 'light', source = str(report.source), view = str(report.view, 'chat');
  if (!source.trim() || view === 'preview') return '';
  const at = key(theme, view, source);
  const width = Number(report.width) || 0, height = Number(report.height) || 0, drawnFor = Math.round(Number(report.drawnFor) || 0);
  const kept = (reports.get(at)?.sizes ?? []).filter(size => size.drawnFor !== drawnFor);
  const frame: MermaidFrame = report.status === 'rendered'
    ? { diagram: 'rendered', message: '', retryable: false, width, height, sizes: [...kept, { drawnFor, width, height }].slice(-MAX_SIZES) }
    : { diagram: 'error', message: str(report.message, 'The diagram could not be rendered.'), retryable: report.retryable === true, width: 0, height: 0, sizes: [] };
  if (frame.diagram === 'rendered' && (!(frame.width > 0) || !(frame.height > 0))) return '';
  reports.delete(at);
  reports.set(at, frame);
  while (reports.size > MAX_CACHED_RENDERS) reports.delete(reports.keys().next().value!);
  return '';
}

/** MermaidDiagram's Retry: forget the failed render and load its page again. */
export function retryMermaid(code: string, theme: string): string {
  const which: Theme = theme === 'dark' ? 'dark' : 'light';
  for (const view of ['chat', 'file'] as const) reports.delete(key(which, view, code));
  attempts.set(retryKey(which, code), (attempts.get(retryKey(which, code)) ?? 0) + 1);
  return '';
}

/** The closed fences of the open thread's messages and plans. */
export function transcriptMermaid(client: T3Client): string[] {
  const sources: string[] = [];
  for (const row of arr(client.projection.visibleTurnItems)) {
    const item = obj(row.item), type = str(item.type);
    if (!['user_message', 'assistant_message', 'proposed_plan'].includes(type)) continue;
    for (const code of mermaidFences(type === 'proposed_plan' ? str(item.markdown) : str(item.text))) if (!sources.includes(code)) sources.push(code);
    if (sources.length >= 24) break;
  }
  return sources;
}

/**
 * FileMarkdownPreview is ChatMarkdown, so a rendered Markdown file's settled
 * ```mermaid fences draw as diagrams too (MarkdownMermaidCodeBlock, isStreaming false). Files remembers them per
 * client while it builds its view, so the expand button can open them. A sent attachment's rendered Markdown is
 * ChatMarkdown too (attachment-surface.ts) and draws the same way: the right panel shows one of the two at a time.
 */
const fileFences = new WeakMap<object, string[]>();
export function filesMermaid(client: T3Client, markdown: string): MermaidDiagramView[] {
  const sources = mermaidFences(markdown).slice(0, 24);
  fileFences.set(client, sources);
  return sources.map((code, index) => diagramFor(String(index), code, 'file'));
}

/** The expanded diagram (ExpandedImageDialog "Mermaid diagram"), per client and thread. */
const previews = new WeakMap<object, { threadId: string; code: string; view: MermaidView }>();
export function diagramPreviewAction(client: T3Client, op: string, code: string): string {
  if (op === 'diagram-close') { previews.delete(client); return ''; }
  const view: MermaidView | null = fileFences.get(client)?.includes(code) ? 'file' : transcriptMermaid(client).includes(code) ? 'chat' : null;
  if (!view) return '';
  previews.set(client, { threadId: client.threadId, code, view });
  return '';
}
/**
 * The open preview: offered only once a theme's page has rendered it (the dialog draws the window's theme). It
 * shows the diagram as its opener drew it (the reference expands an image of the drawn SVG): that surface's size,
 * in a page of its own.
 */
export function diagramPreviewView(client: T3Client): { diagramPreview: MermaidDiagramView[] } {
  const current = previews.get(client);
  if (!current || current.threadId !== client.threadId) return { diagramPreview: [] };
  const view: MermaidView = current.view;
  const drawn = diagramFor('diagram-preview', current.code, view);
  const diagram = { ...drawn, srcLight: mermaidSrc(current.code, 'light', 'preview', drawn.light.width), srcDark: mermaidSrc(current.code, 'dark', 'preview', drawn.dark.width) };
  return { diagramPreview: diagram.light.diagram === 'rendered' || diagram.dark.diagram === 'rendered' ? [diagram] : [] };
}
