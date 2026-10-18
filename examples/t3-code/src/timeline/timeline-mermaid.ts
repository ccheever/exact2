// Mermaid fences in the transcript, adapted from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/components/ChatMarkdown.tsx (MarkdownMermaidCodeBlock: a settled
// ```mermaid fence draws as a diagram) and chat/MermaidDiagram.tsx (render cache,
// errors and retry). Each fence renders in the Mermaid page (assets/mermaid/, an
// `iframe` per fence, markdown.contract MermaidBlock) with the reference's Mermaid
// build and configuration; the page reports its natural size or Mermaid's error
// (`diagram-frame`), which this keeps per theme and source, as the reference's
// render cache keys its renders. The transcript and a rendered Markdown file in
// Files draw them.
import { arr, obj, str } from '../client/domain';
import type { T3Client } from '../client/client';

/** What one theme's page reported: "loading" until it answers, then "rendered" or "error". */
export interface MermaidFrame { diagram: string; message: string; retryable: boolean; width: number; height: number }
/** markdown.contract MermaidDiagram: one settled fence, matched to its parsed block by its text, in both themes. */
export interface MermaidDiagramView { id: string; code: string; srcLight: string; srcDark: string; light: MermaidFrame; dark: MermaidFrame }
export const MERMAID_FONT = '-apple-system, "system-ui", "Segoe UI", system-ui, sans-serif';
/** The Mermaid page under the app's assets (LLP 1020.001: served at exact-app://app/ on Apple). */
export const MERMAID_PAGE = 'assets/mermaid/index.html';
const LOADING: MermaidFrame = Object.freeze({ diagram: 'loading', message: '', retryable: false, width: 0, height: 0 }) as MermaidFrame;
/** MermaidDiagram's MAX_CACHED_RENDERS. */
const MAX_CACHED_RENDERS = 64;

type Theme = 'light' | 'dark';
const key = (theme: Theme, code: string) => `${theme}\n${code.trim()}`;
/** Reported renders by `theme\nsource`, oldest first (MermaidDiagram's renderCache). */
const reports = new Map<string, MermaidFrame>();
/** Retries asked per `theme\nsource`: each one is a new page load. */
const attempts = new Map<string, number>();

const fence = /^( {0,3})(`{3,}|~{3,})([^\n`]*)\n([\s\S]*?)\n? {0,3}\2[`~]*[ \t]*(?:\n|$)/gm;
/** The code of every ```mermaid fence, as the parsed block's text (messageCodeBlocks' matching). */
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

/** The page for one fence in one theme; a retry's attempt number makes it a new load. */
export function mermaidSrc(code: string, theme: Theme): string {
  const params = new URLSearchParams({ theme, font: MERMAID_FONT, source: code.trim() });
  const attempt = attempts.get(key(theme, code)) ?? 0;
  if (attempt) params.set('attempt', String(attempt));
  return `${MERMAID_PAGE}#${params.toString()}`;
}

/** A fence's frame in one theme as its page reported it, or loading. */
export function frameFor(code: string, theme: Theme): MermaidFrame {
  return reports.get(key(theme, code)) ?? LOADING;
}

/** A fence as the view draws it: both themes' pages and what each reported. */
export function diagramFor(id: string, code: string): MermaidDiagramView {
  return { id, code, srcLight: mermaidSrc(code, 'light'), srcDark: mermaidSrc(code, 'dark'), light: frameFor(code, 'light'), dark: frameFor(code, 'dark') };
}

/** A message's settled Mermaid fences as diagrams; a streaming message keeps plain code blocks. */
export function messageDiagrams(markdown: string, streaming: boolean): MermaidDiagramView[] {
  return streaming ? [] : mermaidFences(markdown).map((code, index) => diagramFor(String(index), code));
}

/**
 * `diagram-frame`: a Mermaid page's report, the JSON it posts to its parent (assets/mermaid/render.js). A report for a
 * source and theme no fence asked for still lands: the page renders only what its fragment names.
 */
export function noteMermaidFrame(payload: string): string {
  let report;
  try { report = obj(JSON.parse(payload)); } catch { return ''; }
  const theme: Theme = report.theme === 'dark' ? 'dark' : 'light', source = str(report.source);
  if (!source.trim()) return '';
  const frame: MermaidFrame = report.status === 'rendered'
    ? { diagram: 'rendered', message: '', retryable: false, width: Number(report.width) || 0, height: Number(report.height) || 0 }
    : { diagram: 'error', message: str(report.message, 'The diagram could not be rendered.'), retryable: report.retryable === true, width: 0, height: 0 };
  if (frame.diagram === 'rendered' && (!(frame.width > 0) || !(frame.height > 0))) return '';
  const at = key(theme, source);
  reports.delete(at);
  reports.set(at, frame);
  while (reports.size > MAX_CACHED_RENDERS) reports.delete(reports.keys().next().value!);
  return '';
}

/** MermaidDiagram's Retry: forget the failed render and load its page again. */
export function retryMermaid(code: string, theme: string): string {
  const at = key(theme === 'dark' ? 'dark' : 'light', code);
  reports.delete(at);
  attempts.set(at, (attempts.get(at) ?? 0) + 1);
  return '';
}

/** Settled messages and plans of the open thread: a streaming fence stays a code block. */
export function transcriptMermaid(client: T3Client): string[] {
  const sources: string[] = [];
  for (const row of arr(client.projection.visibleTurnItems)) {
    const item = obj(row.item), type = str(item.type);
    if (item.streaming === true || !['user_message', 'assistant_message', 'proposed_plan'].includes(type)) continue;
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
  return sources.map((code, index) => diagramFor(String(index), code));
}

/** The expanded diagram (ExpandedImageDialog "Mermaid diagram"), per client and thread. */
const previews = new WeakMap<object, { threadId: string; code: string }>();
export function diagramPreviewAction(client: T3Client, op: string, code: string): string {
  if (op === 'diagram-close') { previews.delete(client); return ''; }
  if (!fileFences.get(client)?.includes(code) && !transcriptMermaid(client).includes(code)) return '';
  previews.set(client, { threadId: client.threadId, code });
  return '';
}
/** The open preview: offered only once a theme's page has rendered it (the dialog draws the window's theme). */
export function diagramPreviewView(client: T3Client): { diagramPreview: MermaidDiagramView[] } {
  const current = previews.get(client);
  if (!current || current.threadId !== client.threadId) return { diagramPreview: [] };
  const diagram = diagramFor('diagram-preview', current.code);
  return { diagramPreview: diagram.light.diagram === 'rendered' || diagram.dark.diagram === 'rendered' ? [diagram] : [] };
}
