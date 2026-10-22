// T3 Code (MIT, see ../LICENSE-T3), pinned365aa87982a4d81cc8e0c085e8d1a40ca7daecdc.
// Adapted from packages/shared/src/htmlRender.ts:16–146 and toolOutput.ts:11–97,167–174.
// Pure mobile foundation only: no feed, asset, native-view or theme ownership.
import { t3ToolDefinition } from './shared/timeline-t3tools';

export interface HtmlRenderReference {
  readonly attachmentId: string;
  readonly title: string;
  readonly height: number;
  readonly heights?: ReadonlyArray<readonly [width: number, height: number]>;
}

const MIN_HEIGHT = 80, MAX_HEIGHT = 2000, COLUMN_WIDTH = 728, MAX_MEASURED_HEIGHTS = 24;
const MAX_PARSED_BYTES = 16_384, MAX_CONTENT_BLOCKS = 32, MAX_ENVELOPE_DEPTH = 4, MAX_ENVELOPE_NODES = 128;
const encoder = new TextEncoder();
const clampHeight = (height: number) => Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, Math.round(height)));

function readMeasuredHeights(value: unknown): HtmlRenderReference['heights'] {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_MEASURED_HEIGHTS) return;
  const heights = value.flatMap(entry => Array.isArray(entry) && entry.length === 2
    && Number.isInteger(entry[0]) && entry[0] >= 1 && entry[0] <= 10_000
    && typeof entry[1] === 'number' && Number.isFinite(entry[1])
    ? [[entry[0] as number, clampHeight(entry[1])] as const] : []);
  return heights.length === value.length ? [...heights].sort((left, right) => left[0] - right[0]) : undefined;
}

export function readHtmlRenderReference(value: unknown): HtmlRenderReference | undefined {
  if (typeof value !== 'object' || value === null) return;
  const { attachmentId, title, height, heights } = value as Record<string, unknown>;
  if (typeof attachmentId !== 'string' || attachmentId.length === 0 || attachmentId.length > 256
    || typeof title !== 'string' || typeof height !== 'number' || !Number.isFinite(height)) return;
  const measured = readMeasuredHeights(heights);
  return { attachmentId, title: title.trim().slice(0, 200) || 'HTML', height: clampHeight(height),
    ...(measured === undefined ? {} : { heights: measured }) };
}

export function htmlRenderReferencesEqual(left: HtmlRenderReference, right: HtmlRenderReference): boolean {
  return left.attachmentId === right.attachmentId && left.title === right.title && left.height === right.height
    && (left.heights ?? []).length === (right.heights ?? []).length
    && (left.heights ?? []).every(([width, height], index) => right.heights?.[index]?.[0] === width && right.heights[index][1] === height);
}

function measuredHeight(heights: NonNullable<HtmlRenderReference['heights']>, width: number): number {
  const above = heights.findIndex(([measuredWidth]) => measuredWidth >= width);
  const high = above === -1 ? heights.length - 1 : above;
  const low = heights[high]![0] === width ? high : Math.max(0, high - 1);
  return Math.max(heights[low]![1], heights[high]![1]);
}

/** The page keeps its measured height unless the author requested a scrolling frame. */
export function htmlRenderFrameHeight(reference: HtmlRenderReference, width: number, contentHeight?: number): number {
  const heights = reference.heights;
  if (heights === undefined || heights.length === 0) return clampHeight(Math.min(reference.height, contentHeight ?? reference.height));
  const cap = measuredHeight(heights, COLUMN_WIDTH) > reference.height ? reference.height : MAX_HEIGHT;
  return clampHeight(Math.min(cap, contentHeight ?? measuredHeight(heights, width)));
}

export function htmlRenderFileName(title: string): string {
  const name = title.replace(/[\\/:*?"<>|\p{Cc}]+/gu, ' ').replace(/\s+/g, ' ').trim().slice(0, 120).trim();
  return `${name || 'Page'}.html`;
}

interface ResultBudget { remainingBytes: number; remainingNodes: number; exceeded: boolean }
interface ResultEnvelope { data?: Record<PropertyKey, unknown>; failed: boolean }
// effect/Predicate.isObject includes arrays and functions; arrays are handled before the object branch.
const isObject = (value: unknown): value is Record<PropertyKey, unknown> =>
  value !== null && (typeof value === 'object' || typeof value === 'function');

function readResult(value: unknown, budget: ResultBudget, depth = 0): ResultEnvelope {
  if (depth > MAX_ENVELOPE_DEPTH || budget.remainingNodes-- <= 0) {
    budget.exceeded = true;
    return { failed: false };
  }
  if (typeof value === 'string') {
    if (value.length > budget.remainingBytes) { budget.exceeded = true; return { failed: false }; }
    const bytes = encoder.encode(value).byteLength;
    if (bytes > budget.remainingBytes) { budget.exceeded = true; return { failed: false }; }
    budget.remainingBytes -= bytes;
    try { return readResult(JSON.parse(value), budget, depth + 1); } catch { return { failed: false }; }
  }
  if (Array.isArray(value)) {
    if (value.length > MAX_CONTENT_BLOCKS) { budget.exceeded = true; return { failed: false }; }
    let data: ResultEnvelope['data'], failed = false;
    for (const block of value) {
      const text = isObject(block) ? block.text : undefined;
      const result = readResult(isObject(text) ? (text.text ?? text) : text, budget, depth + 1);
      data ??= result.data; failed ||= result.failed;
      if (budget.exceeded) break;
    }
    return { ...(data === undefined ? {} : { data }), failed };
  }
  if (!isObject(value)) return { failed: false };
  const failed = value.isError === true || value.is_error === true || value._tag === 'OrchestratorMcpFailure' || value.error != null;
  const content = value.structuredContent ?? value.content;
  if (content !== undefined) {
    const nested = readResult(content, budget, depth + 1);
    return { ...nested, failed: failed || nested.failed };
  }
  return { data: value, failed };
}

/** Mobile's completed dynamic-tool gate, then the pinned bounded result reader.
 * Compact projections may set outputOmitted while retaining this render metadata.
 * The source's 8192-byte compact cleanup removes thread-summary fields, never htmlRender.
 */
export function htmlRenderFromToolItem(item: {
  readonly type?: unknown; readonly status?: unknown; readonly toolName?: unknown; readonly output?: unknown;
}): HtmlRenderReference | undefined {
  if (item.type !== 'dynamic_tool' || item.status !== 'completed'
    || t3ToolDefinition(typeof item.toolName === 'string' ? item.toolName : null)?.summaryAction !== 'html-render') return;
  const budget: ResultBudget = { remainingBytes: MAX_PARSED_BYTES, remainingNodes: MAX_ENVELOPE_NODES, exceeded: false };
  const result = readResult(item.output, budget);
  return result.failed || budget.exceeded ? undefined : readHtmlRenderReference(result.data?.htmlRender);
}
