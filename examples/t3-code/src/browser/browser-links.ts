// Where a link opens, the "Open links in" setting (MIT reference, see LICENSE-T3, T3 Code
// 9c161462c9: apps/web/src/browser/browserLinkTarget.ts, useOpenLink.ts, openFileInPreview.ts `openUrlInPreview`;
// apps/web/src/components/preview/openTerminalLinkInPreview.ts; components/ChatMarkdown.tsx's anchor;
// components/settings/IntegrationsSettings.tsx `BrowserLinkTargetSetting`).
//
// Chat Markdown links (`chatlocal:link-open`, kind "link": ⌘ or Ctrl held is the way out of the in-app default),
// check details and the published repository (kind "button", no modifier), a pull request link with no project to
// open it beside, and terminal links all ask `resolveLinkTarget`. "app" opens a Browser tab beside the thread
// (`preview.open`); an in-app open that fails falls back to the system browser (`remoteEditorsOpen`, the clone's
// shell.openExternal); a project script's `previewUrl` always opens in the app.
import type { T3Client } from '../client/client';
import { obj, str } from '../client/domain';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { activeRef } from '../terminal/terminal-drawer-view';
import { scopedThreadKey, type ScopedThreadRef } from '../terminal/terminal-ui-state';
import { surfaceStore } from '../panel/panel-surfaces';
import { pushToast } from '../shell/toast';
import { browserHost, installBrowserCleanup, listPreviewSessions, openBrowserIn, openPreviewSession, syncNativeSessions } from './browser-surface';
import type { PreviewSessionSnapshot, PreviewViewportSetting } from './browser-state';
import { resolveBrowserOpenDefaults } from './browser-defaults';
import { BrowserSettingsReadError } from './browser-profiles';
import { mediaFileReference } from '../media/media-reference';
import { assetUrl } from '../settings/settings-b-icons';
import { openHome } from '../pages/home-landing';

export type BrowserLinkTarget = 'system' | 'app';
export type LinkEvent = { readonly metaKey: boolean; readonly ctrlKey: boolean; readonly button?: number };
const NO_MODIFIER: LinkEvent = { metaKey: false, ctrlKey: false };

/** Only http(s) can load in the in-app browser; mailto:, vscode:// and the rest belong to the shell. */
export function isWebUrl(url: string): boolean {
  try { const { protocol } = new URL(url); return protocol === 'http:' || protocol === 'https:'; } catch { return false; }
}
/** "app" only when the preference asks for it, the runtime can honour it, the URL can load in-app and the click
 *  carried no modifier: the modifier is the one-gesture way out when the default is in-app. */
export function resolveLinkTarget(input: { url: string; event: LinkEvent; preference: BrowserLinkTarget; canOpenInApp: boolean }): BrowserLinkTarget {
  if (input.event.metaKey || input.event.ctrlKey || input.event.button === 1) return 'system';
  if (input.preference !== 'app') return 'system';
  if (!input.canOpenInApp) return 'system';
  if (!isWebUrl(input.url)) return 'system';
  return 'app';
}
/** The stored "Open links in" value, as the settings row shows it (useClientSettings). */
export const linkTargetPreference = (client: T3Client): BrowserLinkTarget => str(obj(client.local.clientSettings).browserLinkTarget) === 'app' ? 'app' : 'system';
/** resolveBrowserLinkTargetPreference: the configured default once the client's settings were read (a command reads them
 *  before its op runs, the clone's ensureClientSettingsHydrated); unread, it is refused (BrowserSettingsReadError) and no
 *  browser opens, as a failed hydration rejects in the reference. */
export function resolveLinkTargetPreference(client: T3Client): BrowserLinkTarget {
  if ((client as { preferencesLoaded?: boolean }).preferencesLoaded === false) throw new BrowserSettingsReadError();
  return linkTargetPreference(client);
}
/** Whether the in-app target exists: a thread to open beside, on this macOS client (isPreviewSupportedInRuntime). */
export const canOpenLinksInApp = (hasThread: boolean, native: Native | null | undefined): boolean => hasThread && !!native?.available;

/** browserDefaultOpenViewport / browserDefaultOpenProfileId: what a new tab opens with (`resolveBrowserOpenDefaults`). */
export type BrowserDefaults = { viewport: PreviewViewportSetting; profileId: string };

/** rightPanelStore.openBrowser for a thread that may not be the shown one. */
function openBesideThread(client: T3Client, ref: ScopedThreadRef, tabId: string): void {
  const store = surfaceStore(client), key = `${ref.environmentId}:${ref.threadId}`;
  let state = store.panels.get(key);
  if (!state) { state = { surfaces: [], active: '', visible: false, userRevision: 0 }; store.panels.set(key, state); }
  const active = activeRef(client);
  if (active && scopedThreadKey(active) === scopedThreadKey(ref)) client.diffOpen = false;
  openBrowserIn(state, tabId, scopedThreadKey(ref));
}

/** openUrlInPreview: a new tab at the URL under the configured defaults (or the ones a caller already resolved), beside the thread. */
export async function openUrlInPreview(client: T3Client, native: Native, ref: ScopedThreadRef, url: string, resolved?: BrowserDefaults): Promise<PreviewSessionSnapshot> {
  installBrowserCleanup(client, native);
  // Read once, and refused while the settings are unread (BrowserSettingsReadError): never a tab born at the schema defaults.
  const host = browserHost(client), defaults = resolved ?? resolveBrowserOpenDefaults(client);
  await listPreviewSessions(client, native, ref); // the server's epoch first: the tab's native identity names it
  const snapshot = await openPreviewSession((method, payload) => client.rpc(native, method, payload, true), host.store, ref, { url, viewport: defaults.viewport, profileId: defaults.profileId });
  openBesideThread(client, ref, snapshot.tabId);
  await syncNativeSessions(client, native);
  return snapshot;
}

/** openFileInPreview: a browser document (a page or a PDF) in a new Browser tab beside the thread. Inside the workspace
 *  the page may load its siblings (`workspace-file`); a file outside it is served on its own (`media-file`). */
export async function openFileInPreview(client: T3Client, native: Native, ref: ScopedThreadRef, filePath: string, workspaceRoot: string): Promise<void> {
  if (!native.available) throw new ClientError('The integrated browser is unavailable in this runtime.');
  const inside = mediaFileReference(filePath, workspaceRoot).relativePath !== undefined;
  const asset = obj(await client.rpc(native, 'assets.createUrl', { resource: { _tag: inside ? 'workspace-file' : 'media-file', threadId: ref.threadId, path: filePath } }));
  const url = assetUrl(client.origin, str(asset.relativeUrl));
  if (!url) throw new ClientError('The environment returned an invalid asset URL.');
  await openUrlInPreview(client, native, ref, url);
}

/** ElectronShell.openExternal (T3RemoteEditors.swift; an agent run records the URL). */
export async function openInSystemBrowser(native: Native, url: string): Promise<void> {
  const reply = await bridgeReply(native, { op: 'remoteEditorsOpen', url });
  if (!reply.ok || obj(reply.value).opened === false) throw new ClientError('Link opening is unavailable.');
}

/** useOpenLink: the setting decides; a failed in-app open falls back to the system browser rather than dropping the click.
 *  Unread settings open neither browser (BrowserSettingsReadError is thrown, as the reference rethrows it). */
export async function openLink(client: T3Client, native: Native, url: string, options: { event?: LinkEvent; threadRef?: ScopedThreadRef | null } = {}): Promise<BrowserLinkTarget> {
  const ref = options.threadRef === undefined ? activeRef(client) : options.threadRef;
  const target = resolveLinkTarget({ url, event: options.event ?? NO_MODIFIER, preference: resolveLinkTargetPreference(client), canOpenInApp: canOpenLinksInApp(!!ref, native) });
  if (target === 'app' && ref) {
    try { await openUrlInPreview(client, native, ref, url); return 'app'; }
    catch (error) { if (letGo(error) || error instanceof BrowserSettingsReadError) throw error; }
  }
  await openInSystemBrowser(native, url);
  return 'system';
}

/** `chatlocal:link-open`: a chat Markdown link ("link": its click's ⌘ or Ctrl, read from the module's last gesture),
 *  a button that opens a URL ("button": check details, the published repository; no modifier), or a web search
 *  result in the work-log inspector ("external": the system browser). */
export async function openLinkFromUi(client: T3Client, native: Native, pressed: string, url: string): Promise<string> {
  // `kind|meta+shift`: the press's own modifiers (app-window.contract linkOpen).
  const bar = pressed.indexOf('|'), kind = bar < 0 ? pressed : pressed.slice(0, bar), held = bar < 0 ? [] : pressed.slice(bar + 1).split('+').filter(Boolean);
  if (!url) return '';
  if (kind === 'link' && url.startsWith('#')) return openFragmentLink(client, native, held);
  // ChatMarkdown leaves a mailto, irc or xmpp link's click to the desktop shell, whose window-open
  // handler opens only http(s) and remote-editor links (DesktopWindow.ts, parseSafeExternalUrl): nothing opens.
  if (kind === 'link' && !isWebUrl(url)) return '';
  const event = kind === 'link' ? { metaKey: held.includes('meta'), ctrlKey: held.includes('control') } : NO_MODIFIER;
  // A button on the pull request page ("button-page") has no thread to open beside: the system browser.
  // A work-log web result ("external") is the reference's target=_blank link: always the system browser (setWindowOpenHandler).
  try {
    if (kind === 'external') await openInSystemBrowser(native, url);
    else await openLink(client, native, url, { event, ...(kind === 'button-page' ? { threadRef: null } : {}) });
  }
  catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: kind === 'link' || kind === 'external' ? 'Unable to open link' : 'Unable to open check details', description: error instanceof Error ? error.message : 'An error occurred.' });
  }
  return '';
}

/** A Markdown `#fragment` link's click (ChatMarkdown `handleMarkdownFragmentClick`):
 *  it scrolls to an element with that id where the page has one (a GFM footnote or a raw-HTML id, neither of which the
 *  clone draws); with none the click goes on and sets the page's hash, which Electron's hash router (main.tsx
 *  `createHashHistory`) reads as the index route, whose IndexDraftLanding opens the draft of the most recently active
 *  project and the composer takes the focus (`sidebar:new-thread`, app.contract commandCompleted). A click with ⌘, ⌥, ⇧
 *  or ⌃ held the handler leaves to the shell, which opens nothing for it (a new window it denies). */
export async function openFragmentLink(client: T3Client, native: Native, held: string[] = []): Promise<string> {
  if (held.length) return '';
  try { await openHome(client, native); }
  catch (error) { if (letGo(error)) throw error; }
  return 'sidebar:new-thread';
}

/** openTerminalLinkInPreview: a terminal hyperlink where the setting says, unless ⌘ or Ctrl asked for the system
 *  browser. A preview that fails to open is reported and falls back; one let go (interrupted) does neither. */
export async function openTerminalLinkInPreview(input: {
  url: string; threadRef: ScopedThreadRef; forceBrowser: boolean; supported: boolean;
  preference: () => Promise<BrowserLinkTarget> | BrowserLinkTarget; defaults: () => Promise<BrowserDefaults> | BrowserDefaults;
  openPreview: (input: { environmentId: string; input: { threadId: string; url: string; viewport: PreviewViewportSetting; profileId: string } }) => Promise<PreviewSessionSnapshot>;
  fallbackToBrowser: () => void | Promise<void>; opened?: (snapshot: PreviewSessionSnapshot) => void; report?: (error: TerminalLinkPreviewOpenError) => void;
}): Promise<void> {
  const supportsPreview = !input.forceBrowser && isWebUrl(input.url) && input.supported && input.threadRef.threadId.length > 0 && (await input.preference()) === 'app';
  if (!supportsPreview) return void (await input.fallbackToBrowser());
  // resolveBrowserDefaults: read before the open and outside its fallback, so unread settings open neither browser.
  const defaults = await input.defaults();
  let snapshot: PreviewSessionSnapshot;
  try {
    snapshot = await input.openPreview({ environmentId: input.threadRef.environmentId, input: { threadId: input.threadRef.threadId, url: input.url, viewport: defaults.viewport, profileId: defaults.profileId } });
  } catch (cause) {
    if (letGo(cause)) throw cause;
    (input.report ?? (() => undefined))(new TerminalLinkPreviewOpenError(input.threadRef, new URL(input.url).origin, cause));
    return void (await input.fallbackToBrowser());
  }
  input.opened?.(snapshot);
}
export class TerminalLinkPreviewOpenError extends Error {
  constructor(readonly threadRef: ScopedThreadRef, readonly targetOrigin: string, readonly cause: unknown) {
    super(`Failed to open terminal link ${targetOrigin} in preview for thread ${threadRef.threadId}.`);
    this.name = 'TerminalLinkPreviewOpenError';
  }
}

/** ChatView's script run: a script's `previewUrl` opens in the app once it runs (`autoOpenPreview`), whatever the setting. */
export async function openScriptPreview(client: T3Client, native: Native, previewUrl: string): Promise<void> {
  const ref = activeRef(client);
  if (!ref) return;
  try { await openUrlInPreview(client, native, ref, previewUrl); }
  catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not open preview', description: error instanceof Error && error.message ? error.message : 'An unexpected error occurred.' });
  }
}
