// Inline HTML pages and MCP app documents (T3 Code 9c161462c9, MIT; LICENSE-T3).
import { arr, num, obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { assetUrl } from '../settings/settings-b-icons';
import { paintOf } from '../appearance/settings-appearance-look';
import { fontStack, themeRoles } from '../appearance/settings-appearance';
import { STANDARD } from '../appearance/settings-appearance-editor';
import { builtInThemeColors } from '../appearance/settings-theme-library';
import { htmlRenderFileName, htmlRenderTheme, htmlRenderThemeFragment, htmlRenderFrameHeight, htmlRenderOutlineBoxes, readHtmlRenderContentHeight, type HtmlRenderReference } from './html-render';
import { clampMcpAppHeight, mcpAppFileName, type McpAppReference } from './mcp-app';
import { htmlRenderFromToolItem, mcpAppFromToolItem } from './inline-tool-output';

export type InlineReference = { kind: 'html'; reference: HtmlRenderReference } | { kind: 'mcp'; reference: McpAppReference };
export type InlineDocumentView = { key: string; kind: string; title: string; src: string; ready: boolean; error: string; height: number; outline: { key: string; kind: string; x: number; y: number; width: number; height: number; lines: number }[] };
type Frame = { url: string; expiresAt: number; src: string; ready: boolean; error: string; height?: number; reading: boolean };
type State = { generation: number; environment: string; frames: Map<string, Frame> };
const states = new WeakMap<T3Client, State>();
const MIN_URL_LIFE = 5 * 60_000;
const LIMIT = 64;

function state(client: T3Client): State {
  let value = states.get(client);
  if (!value || value.generation !== client.generation || value.environment !== client.environmentId) {
    value = { generation: client.generation, environment: client.environmentId, frames: new Map() };
    states.set(client, value);
  }
  return value;
}
export function inlineReference(item: Obj): InlineReference | null {
  if (item.type !== 'dynamic_tool' || item.status !== 'completed') return null;
  const tool = { toolName: str(item.toolName), output: item.output };
  const html = htmlRenderFromToolItem(tool);
  if (html) return { kind: 'html', reference: html };
  const app = mcpAppFromToolItem(tool);
  return app ? { kind: 'mcp', reference: app } : null;
}
export const inlineDocumentKey = (client: T3Client, sourceThreadId: string, itemId: string, ref: InlineReference) => JSON.stringify([client.environmentId, sourceThreadId || client.threadId, itemId, ref.kind, ref.reference.attachmentId]);

export function inlineTheme(client: T3Client) {
  const { prefs, custom, mode } = paintOf(client), appearance = mode === 'light' ? 'light' : 'dark';
  const themeId = appearance === 'light' ? prefs.themeLight : prefs.themeDark;
  const own = custom.find(theme => theme.id === themeId);
  const colors = { ...STANDARD[appearance], ...(own?.[appearance] ?? builtInThemeColors(themeId, appearance) ?? themeRoles(themeId, appearance, custom)) };
  return htmlRenderTheme(colors, appearance, { sans: fontStack(prefs.fontFamilySans, false), mono: fontStack(prefs.fontFamilyCode, true) });
}

/** Resource refresh signs once per mounted document; a changing cache URL never reloads an interactive frame. */
export async function syncInlineDocuments(client: T3Client, native: Native, now: number): Promise<void> {
  const held = state(client), threadId = client.threadId;
  for (const row of arr(client.projection.visibleTurnItems).slice(-LIMIT)) {
    const item = obj(row.item), ref = inlineReference(item);
    if (!ref) continue;
    const key = inlineDocumentKey(client, str(row.sourceThreadId), str(item.id), ref);
    const cached = held.frames.get(key);
    if (cached?.src || cached?.reading || cached?.error) continue;
    const frame: Frame = cached ?? { url: '', expiresAt: 0, src: '', ready: false, error: '', reading: false };
    held.frames.set(key, frame); frame.reading = true;
    while (held.frames.size > LIMIT) held.frames.delete(held.frames.keys().next().value!);
    try {
      if (!frame.url || frame.expiresAt - now <= MIN_URL_LIFE) {
        const answer = obj(await client.rpc(native, 'assets.createUrl', { resource: { _tag: 'attachment', attachmentId: ref.reference.attachmentId,
          fileName: ref.kind === 'html' ? htmlRenderFileName(ref.reference.title) : mcpAppFileName(ref.reference), mimeType: 'text/html', disposition: 'inline',
          ...(ref.kind === 'mcp' ? { renderIntent: 'mcp-app' } : {}) } }));
        frame.url = assetUrl(client.origin, str(answer.relativeUrl)); frame.expiresAt = num(answer.expiresAt);
        if (!frame.url) throw new Error('Document URL unavailable.');
      }
      if (client.generation !== held.generation || client.environmentId !== held.environment || client.threadId !== threadId || states.get(client) !== held) return;
      frame.src = frame.url + (ref.kind === 'html' ? htmlRenderThemeFragment(inlineTheme(client)) : '');
    } catch (error) {
      if (letGo(error)) { held.frames.delete(key); throw error; }
      frame.error = error instanceof Error ? error.message : 'Unable to load document.';
    } finally { frame.reading = false; }
  }
}

export function inlineDocumentView(client: T3Client, key: string, ref: InlineReference, width = 728): InlineDocumentView {
  const frame = state(client).frames.get(key);
  return { key, kind: ref.kind, title: ref.kind === 'html' ? ref.reference.title : ref.reference.tool, src: frame?.src ?? '', ready: frame?.ready ?? false, error: frame?.error ?? '',
    height: ref.kind === 'html' ? htmlRenderFrameHeight(ref.reference, width, frame?.height) : clampMcpAppHeight(frame?.height ?? 320),
    outline: ref.kind === 'html' ? htmlRenderOutlineBoxes(ref.reference, width).map((box, index) => ({ ...box, key: String(index) })) : [] };
}
export function inlineDocumentEvent(client: T3Client, key: string, op: string, payload: string): void {
  const frame = state(client).frames.get(key);
  if (!frame) return;
  if (op === 'loaded') frame.ready = true;
  if (op === 'retry') { frame.error = ''; frame.src = ''; frame.ready = false; }
  if (op === 'message') {
    let message: unknown;
    try { message = JSON.parse(payload); } catch { return; }
    const height = readHtmlRenderContentHeight(message);
    if (height !== undefined) { frame.height = height; frame.ready = true; }
  }
}
