// Mermaid fences in the transcript, adapted from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/components/ChatMarkdown.tsx (MarkdownMermaidCodeBlock: a settled
// ```mermaid fence draws as a diagram) and chat/MermaidDiagram.tsx (render queue,
// errors and retry). The app module lays diagrams out with the reference's own
// Mermaid build (T3TimelineMermaid.swift) and answers flattened paths and text
// runs in both themes; the transcript and a rendered Markdown file in Files draw
// them (markdown.contract CodeBlock).
import { arr, obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';

export interface MermaidItem {
  id: string; kind: string; d: string; transform: string; fill: string; fillDark: string; stroke: string; strokeDark: string;
  width: number; dash: string; cap: string; join: string; text: string; x: number; y: number; size: number; weight: number; slant: string; baseline: string;
}
/** A fence's diagram: "loading" until both themes are answered, then "rendered" or "error". */
export interface DiagramFields { diagram: string; message: string; retryable: boolean; width: number; height: number; viewBox: string; items: MermaidItem[] }
/** markdown.contract MermaidDiagram: one settled fence, matched to its parsed block by its text. */
export interface MermaidDiagramView extends DiagramFields { id: string; code: string }
export const NO_DIAGRAM: DiagramFields = Object.freeze({ diagram: '', message: '', retryable: false, width: 0, height: 0, viewBox: '', items: [] }) as DiagramFields;
export const MERMAID_FONT = '-apple-system, "system-ui", "Segoe UI", system-ui, sans-serif';
const LOADING: DiagramFields = { ...NO_DIAGRAM, diagram: 'loading' };

const answers = new Map<string, { light: string; dark: string }>();
const parsed = new Map<string, DiagramFields>();
const retries = new Set<string>();

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
 * Asks the module for every diagram not yet answered in both themes; the module
 * answers at once ('' while rendering) and announces each finished render.
 * `sources`: the transcript's fences, or another surface's (filesMermaid).
 */
export async function prepareMermaid(client: T3Client, native: Native | null | undefined, sources = transcriptMermaid(client)): Promise<void> {
  if (!native?.available || !client.origin) return;
  const missing = sources.filter(code => { const known = answers.get(code); return !known || !known.light || !known.dark; });
  if (!missing.length) return;
  const asked = missing.filter(code => retries.has(code));
  const retry = asked.flatMap(code => [`light\n${code.trim()}`, `dark\n${code.trim()}`]);
  for (const code of asked) retries.delete(code);
  try {
    const value = await client.restAccess(native).call({ op: 'mermaidRender', origin: client.origin, font: MERMAID_FONT, retry,
      diagrams: missing.flatMap(code => [{ source: code.trim(), theme: 'light' }, { source: code.trim(), theme: 'dark' }]) });
    for (const entry of arr(value.items)) {
      const [theme, ...rest] = str(entry.key).split('\n'), source = rest.join('\n'), json = str(entry.json);
      for (const code of missing.filter(candidate => candidate.trim() === source)) {
        const known = answers.get(code) ?? { light: '', dark: '' };
        const next = theme === 'dark' ? { ...known, dark: json } : { ...known, light: json };
        if (next.light !== known.light || next.dark !== known.dark) parsed.delete(code);
        answers.set(code, next);
      }
    }
  } catch { /* Not connected to the app module: the fences stay code blocks until it answers. */ }
  if (answers.size > 64) for (const code of [...answers.keys()].slice(0, answers.size - 64)) { answers.delete(code); parsed.delete(code); }
}

/** Paints are drawn through light-dark(), so a missing one is transparent rather than `none`. */
const color = (value: unknown) => typeof value === 'string' && /^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(value) ? value : '#00000000';
/** Joins the two themes' flattened renders: one geometry, both paints. */
export function joinThemes(lightJson: string, darkJson: string): DiagramFields {
  let light: Obj, dark: Obj;
  try { light = obj(JSON.parse(lightJson)); dark = obj(JSON.parse(darkJson)); } catch { return { ...NO_DIAGRAM, diagram: 'error', message: 'The diagram could not be rendered.' }; }
  if (light.status !== 'rendered') return { ...NO_DIAGRAM, diagram: 'error', message: str(light.message, 'The diagram could not be rendered.'), retryable: light.retryable === true };
  const darkItems = dark.status === 'rendered' ? arr(dark.items) : [];
  const items = arr(light.items).map((item, index): MermaidItem => {
    const other = darkItems[index]?.kind === item.kind ? darkItems[index]! : item;
    return { id: String(index), kind: str(item.kind), d: str(item.d), transform: str(item.transform, 'matrix(1 0 0 1 0 0)'), fill: color(item.fill), fillDark: color(other.fill),
      stroke: color(item.stroke), strokeDark: color(other.stroke), width: Number(item.width) || 0, dash: str(item.dash) || 'none', cap: str(item.cap, 'butt'),
      join: str(item.join, 'miter'), text: str(item.text), x: Number(item.x) || 0, y: Number(item.y) || 0, size: Number(item.size) || 16,
      weight: Number(item.weight) || 400, slant: str(item.slant, 'normal'), baseline: str(item.baseline, 'alphabetic') };
  });
  return { diagram: 'rendered', message: '', retryable: false, width: Number(light.width) || 0, height: Number(light.height) || 0, viewBox: str(light.viewBox), items };
}

/** A fence's diagram as its code highlight carries it: loading until both themes are answered. */
export function diagramFor(code: string): DiagramFields {
  const known = answers.get(code);
  if (!known || !known.light || !known.dark) return LOADING;
  let diagram = parsed.get(code);
  if (!diagram) { diagram = joinThemes(known.light, known.dark); parsed.set(code, diagram); }
  return diagram;
}

/** A message's settled Mermaid fences as diagrams; a streaming message keeps plain code blocks. */
export function messageDiagrams(markdown: string, streaming: boolean): MermaidDiagramView[] {
  return streaming ? [] : mermaidFences(markdown).map((code, index) => ({ id: String(index), code, ...diagramFor(code) }));
}

/** MermaidDiagram's Retry: forget the failed render so the next snapshot asks again. */
export function retryMermaid(code: string): string {
  answers.delete(code); parsed.delete(code); retries.add(code);
  return '';
}

/**
 * markdown-links-and-files-preview: FileMarkdownPreview is ChatMarkdown, so a rendered Markdown file's settled
 * ```mermaid fences draw as diagrams too (MarkdownMermaidCodeBlock, isStreaming false). Files asks for them while it
 * builds its view and remembers them, per client, so the expand button can open them. A sent attachment's rendered
 * Markdown is ChatMarkdown too (audit-wave-followups-3 FW-2, r5-panels-attach.ts) and asks the same way: the right
 * panel shows one of the two at a time.
 */
const fileFences = new WeakMap<object, string[]>();
export async function filesMermaid(client: T3Client, native: Native | null | undefined, markdown: string): Promise<MermaidDiagramView[]> {
  const sources = mermaidFences(markdown).slice(0, 24);
  fileFences.set(client, sources);
  await prepareMermaid(client, native, sources);
  return sources.map((code, index) => ({ id: String(index), code, ...diagramFor(code) }));
}

/** The expanded diagram (ExpandedImageDialog "Mermaid diagram"), per client and thread. */
const previews = new WeakMap<object, { threadId: string; code: string }>();
export function diagramPreviewAction(client: T3Client, op: string, code: string): string {
  if (op === 'diagram-close') { previews.delete(client); return ''; }
  if (!fileFences.get(client)?.includes(code) && !transcriptMermaid(client).includes(code)) return '';
  previews.set(client, { threadId: client.threadId, code });
  return '';
}
export function diagramPreviewView(client: T3Client): { diagramPreview: MermaidDiagramView[] } {
  const current = previews.get(client);
  if (!current || current.threadId !== client.threadId) return { diagramPreview: [] };
  const diagram = diagramFor(current.code);
  return { diagramPreview: diagram.diagram === 'rendered' ? [{ id: 'diagram-preview', code: current.code, ...diagram }] : [] };
}
