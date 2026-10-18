// Inline HTML pages and MCP app documents (T3 Code 9c161462c9, MIT; LICENSE-T3).
import { arr, num, obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Files, Native } from '../client/protocol';
import { openLink } from '../browser/browser-links';
import { letGo } from '../client/let-go';
import { assetUrl } from '../settings/settings-b-icons';
import { paintOf } from '../appearance/settings-appearance-look';
import { fontStack, themeRoles } from '../appearance/settings-appearance';
import { STANDARD } from '../appearance/settings-appearance-editor';
import { builtInThemeColors } from '../appearance/settings-theme-library';
import { htmlRenderFileName, htmlRenderTheme, htmlRenderThemeFragment, htmlRenderThemeMessage, readHtmlRenderKeyDown, readHtmlRenderLinkRequest, readHtmlRenderImageRequest, htmlRenderResult, htmlRenderFrameHeight, htmlRenderOutlineBoxes, readHtmlRenderContentHeight, type HtmlRenderReference } from './html-render';
import { clampMcpAppHeight, mcpAppFileName, type McpAppReference } from './mcp-app';
import { documentShortcutCommand, documentShortcutsMessage } from './inline-document-shortcuts';
import { makeDocumentRuntime, drainDocumentRuntime, disposeDocumentRuntime, type DocumentRuntime } from './inline-document-host';
import { htmlRenderFromToolItem, mcpAppFromToolItem } from './inline-tool-output';

export type InlineReference = { kind: 'html'; reference: HtmlRenderReference } | { kind: 'mcp'; reference: McpAppReference };
export type InlineDocumentView = { key: string; kind: string; title: string; src: string; ready: boolean; error: string; height: number; posts: { key: string; text: string }[]; permission: string; permissionSerial: number; fullscreen: boolean; closed: boolean; shortcut: string; shortcutSerial: number; outline: { key: string; kind: string; x: number; y: number; width: number; height: number; lines: number }[] };
type Frame = { token: string; url: string; expiresAt: number; src: string; ready: boolean; error: string; height?: number; reading: boolean; lastTheme?: string; lastShortcuts?: string; shortcut?: string; shortcutSerial?: number; posts: { key: string; text: string }[]; serial: number; runtime?: DocumentRuntime; fullscreen?: boolean; width?: number; inlineWidth?: number; viewportHeight?: number };
type State = { generation: number; environment: string; frames: Map<string, Frame> };
const states = new WeakMap<T3Client, State>();
const MIN_URL_LIFE = 5 * 60_000;
const LIMIT = 64;
let nextDocument = 0;
const ADAPTER_PAGE = 'assets/inline-document/index.html';
export function inlineDocumentSource(src: string, token: string): string {
  return ADAPTER_PAGE + '#' + encodeURIComponent(JSON.stringify({ src, token }));
}

function postToDocument(client: T3Client, frame: Frame, message: unknown): void {
  const text = JSON.stringify({ t3Document: frame.token, message });
  if (frame.posts.length >= 64 || new TextEncoder().encode(text).byteLength > 256 * 1024) {
    frame.error = 'This document exceeded its message limit.';
    if (frame.runtime) disposeDocumentRuntime(frame.runtime);
    frame.posts = [];
    client.revision++;
    return;
  }
  frame.posts.push({ key: `${frame.token}:${++frame.serial}`, text });
  client.revision++;
}
function updateShortcuts(client: T3Client, frame: Frame): void {
  const message = documentShortcutsMessage(client.config), signature = JSON.stringify(message);
  if (frame.ready && frame.lastShortcuts !== signature) { frame.lastShortcuts = signature; postToDocument(client, frame, message); }
}
function updateHtmlTheme(client: T3Client, frame: Frame): void {
  const theme = inlineTheme(client), signature = JSON.stringify(theme);
  if (frame.ready && frame.lastTheme !== signature) { frame.lastTheme = signature; postToDocument(client, frame, htmlRenderThemeMessage(theme)); }
}

function state(client: T3Client): State {
  let value = states.get(client);
  if (!value || value.generation !== client.generation || value.environment !== client.environmentId) {
    if (value) for (const frame of value.frames.values()) if (frame.runtime) disposeDocumentRuntime(frame.runtime);
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
  const active = new Set(arr(client.projection.visibleTurnItems).slice(-LIMIT).flatMap(row => { const item = obj(row.item), ref = inlineReference(item); return ref ? [inlineDocumentKey(client, str(row.sourceThreadId), str(item.id), ref)] : []; }));
  for (const [key, frame] of held.frames) if (!active.has(key)) { if (frame.runtime) disposeDocumentRuntime(frame.runtime); held.frames.delete(key); }
  for (const row of arr(client.projection.visibleTurnItems).slice(-LIMIT)) {
    const item = obj(row.item), ref = inlineReference(item);
    if (!ref) continue;
    const key = inlineDocumentKey(client, str(row.sourceThreadId), str(item.id), ref);
    const cached = held.frames.get(key);
    if (cached) updateShortcuts(client, cached);
    if (cached && ref.kind === 'html') updateHtmlTheme(client, cached);
    if (cached?.runtime) { await drainDocumentRuntime(cached.runtime, native, now); cached.runtime.host.updateHostContext(); }
    if (cached?.src || cached?.reading || cached?.error) continue;
    const frame: Frame = cached ?? { token: `${held.generation}:${++nextDocument}`, url: '', expiresAt: 0, src: '', ready: false, error: '', reading: false, posts: [], serial: 0 };
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
      if (ref.kind === 'mcp' && !frame.runtime) {
        frame.runtime = makeDocumentRuntime(client, { app: ref.reference, sourceThreadId: str(row.sourceThreadId) || threadId, itemId: str(item.id), conversationThreadId: threadId,
          post: message => { if (states.get(client) !== held || held.frames.get(key) !== frame) return; postToDocument(client, frame, message); },
          theme: () => inlineTheme(client), owned: () => states.get(client) === held && held.frames.get(key) === frame && client.threadId === threadId,
          width: () => frame.width ?? 728, viewportHeight: () => frame.viewportHeight ?? 700, canExpand: () => ![...held.frames.values()].some(other => other.runtime?.permission || other !== frame && (other.fullscreen || other.runtime?.fullscreen)), height: height => { frame.height = height; client.revision++; } });
        const result = obj(item.output);
        frame.runtime.host.setToolCall({ arguments: item.input, result: { ...result, content: arr(result.content) } });
      }
      frame.src = inlineDocumentSource(frame.url + (ref.kind === 'html' ? htmlRenderThemeFragment(inlineTheme(client)) : ''), frame.token);
    } catch (error) {
      if (letGo(error)) { held.frames.delete(key); throw error; }
      frame.error = error instanceof Error ? error.message : 'Unable to load document.';
    } finally { frame.reading = false; }
  }
}

export function inlineDocumentView(client: T3Client, key: string, ref: InlineReference, width = 728): InlineDocumentView {
  const frame = state(client).frames.get(key);
  const inlineWidth = frame?.inlineWidth ?? width;
  return { key, kind: ref.kind, title: ref.kind === 'html' ? ref.reference.title : ref.reference.tool, src: frame?.src ?? '', ready: frame?.ready ?? false, error: frame?.error ?? '',
    height: ref.kind === 'html' ? htmlRenderFrameHeight(ref.reference, inlineWidth, frame?.height) : clampMcpAppHeight(frame?.height ?? 320),
    posts: frame?.posts ?? [], permission: frame?.runtime?.permission?.text ?? '', permissionSerial: frame?.runtime?.permission?.serial ?? 0, fullscreen: frame?.runtime?.fullscreen ?? frame?.fullscreen ?? false, closed: frame?.runtime?.closed ?? false, shortcut: frame?.shortcut ?? '', shortcutSerial: frame?.shortcutSerial ?? 0,
    outline: ref.kind === 'html' ? htmlRenderOutlineBoxes(ref.reference, inlineWidth).map((box, index) => ({ ...box, key: String(index) })) : [] };
}
export async function inlineDocumentEvent(client: T3Client, key: string, op: string, payload: string, native?: Native, storage?: Files, openImage?: (image: { src: string; name: string }) => void): Promise<void> {
  const frame = state(client).frames.get(key);
  if (!frame) return;
  if (op === 'shortcut-ack' && String(frame.shortcutSerial) === payload) frame.shortcut = '';
  if (op === 'ack') frame.posts = frame.posts.filter(post => post.key !== payload);
  if (op === 'permission' && frame.runtime?.permission && payload.split(':')[0] === String(frame.runtime.permission.serial)) { const permission = frame.runtime.permission; frame.runtime.permission = null; permission.settle(payload.endsWith(':allow')); }
  if (op === 'fullscreen') { const full = payload === 'open'; if (!full || ![...state(client).frames.values()].some(other => other.runtime?.permission || other !== frame && (other.fullscreen || other.runtime?.fullscreen))) { if (frame.runtime) frame.runtime.fullscreen = full; else frame.fullscreen = full; } frame.runtime?.host.updateHostContext(); }
  if (op === 'measure') { const [width, height] = payload.split('|').map(Number); if (width && height) { frame.width = width; if (frame.runtime?.fullscreen || frame.fullscreen) frame.viewportHeight = height; else frame.inlineWidth = width; frame.runtime?.host.updateHostContext(); } }
  // The adapter's own load does not mean its nested server document is ready.
  if (op === 'retry') { if (frame.runtime) disposeDocumentRuntime(frame.runtime); frame.runtime = undefined; frame.posts = []; frame.error = ''; frame.src = ''; frame.ready = false; frame.token = `${client.generation}:${++nextDocument}`; }
  if (op === 'message') {
    let message: unknown;
    try { const envelope = obj(JSON.parse(payload)); if (envelope.t3Document !== frame.token) return; message = envelope.message; } catch { return; }
    if (obj(message).method === 't3/adapter-ready') { frame.ready = true; updateShortcuts(client, frame); if (!frame.runtime) updateHtmlTheme(client, frame); }
    if (!frame.runtime) {
      const link = readHtmlRenderLinkRequest(message);
      if (link && native) {
        await openLink(client, native, link.url, { event: { metaKey: link.metaKey, ctrlKey: link.ctrlKey } });
        postToDocument(client, frame, htmlRenderResult(link.id));
      }
      const image = readHtmlRenderImageRequest(message);
      if (image) openImage?.(image);
    }
    const keyEvent = readHtmlRenderKeyDown(message);
    if (keyEvent && !frame.runtime?.permission) { const command = documentShortcutCommand(client.config, keyEvent); if (command) { frame.shortcut = command; frame.shortcutSerial = (frame.shortcutSerial ?? 0) + 1; } }
    frame.runtime?.host.receive(message);
    const height = readHtmlRenderContentHeight(message);
    if (height !== undefined) { if (!frame.runtime?.fullscreen && !frame.fullscreen) frame.height = height; frame.ready = true; }
  }
  if (frame.runtime && native) await drainDocumentRuntime(frame.runtime, native, frame.runtime.now, storage);
}
