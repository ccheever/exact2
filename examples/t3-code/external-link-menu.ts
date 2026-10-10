// A reply's web link menu (shell-context-menu; MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/web/src/components/chat/externalLinkContextMenu.ts and ChatMarkdown.tsx's anchor `onContextMenu`). A right-click on
// an http(s) link in Markdown is the app's own menu, so the desktop shell's never opens there: "Link to thread" or
// "Unlink from thread" first for a pull request the open thread can link or links, then "Open in integrated browser"
// where a thread can show the Browser beside it, "Open in system browser" and "Copy Link". The module's `contextMenu` op
// (T3ContextMenu.swift) shows the items at the pointer and names the pick.
import type { T3Client } from './client';
import { bridgeReply, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';
import { showContextMenu } from './context-menu-actions';
import { canOpenLinksInApp, openInSystemBrowser, openUrlInPreview } from './browser-links';
import { BrowserSettingsReadError } from './browser-profiles';
import { activeRef } from './terminal-drawer-view';
import { ensureDraftThreadId } from './r7-handoff-thread';
import { changeChatLink, chatLinkThreadAction } from './pages-pr-links';
import type { MenuItem } from './sidebar-menu';

export type ExternalLinkContextMenuAction = 'open-in-preview' | 'open-external' | 'copy-link' | 'link-to-thread' | 'unlink-from-thread';
export type ExternalLinkContextMenuFailureOperation = 'show-link-context-menu' | 'open-link-in-preview' | 'open-link-external' | 'copy-link'
  | 'link-pull-request-to-thread' | 'unlink-pull-request-from-thread';

const FAILURE_OPERATION_BY_ACTION: Record<ExternalLinkContextMenuAction, ExternalLinkContextMenuFailureOperation> = {
  'open-in-preview': 'open-link-in-preview',
  'open-external': 'open-link-external',
  'copy-link': 'copy-link',
  'link-to-thread': 'link-pull-request-to-thread',
  'unlink-from-thread': 'unlink-pull-request-from-thread',
};

const EXTERNAL_LINK_CONTEXT_MENU_ITEMS: MenuItem[] = [
  { id: 'open-in-preview', label: 'Open in integrated browser' },
  { id: 'open-external', label: 'Open in system browser' },
  { id: 'copy-link', label: 'Copy Link' },
];

/** externalLinkContextMenuItems: the integrated browser only where it can open; the thread action first. */
export function externalLinkContextMenuItems(options: { canOpenInPreview: boolean; threadLinkAction?: 'link-to-thread' | 'unlink-from-thread' }): MenuItem[] {
  const items = options.canOpenInPreview ? EXTERNAL_LINK_CONTEXT_MENU_ITEMS : EXTERNAL_LINK_CONTEXT_MENU_ITEMS.filter(item => item.id !== 'open-in-preview');
  if (options.threadLinkAction === undefined) return items.map(item => ({ ...item }));
  return [{ id: options.threadLinkAction, label: options.threadLinkAction === 'link-to-thread' ? 'Link to thread' : 'Unlink from thread' }, ...items.map(item => ({ ...item }))];
}

function resolveExternalWebLink(href: string | null | undefined): URL | null {
  if (!href) return null;
  try {
    const url = new URL(href.startsWith('//') ? `https:${href}` : href);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url : null;
  } catch { return null; }
}
export function resolveExternalWebLinkHref(href: string | null | undefined): string | null { return resolveExternalWebLink(href)?.href ?? null; }
export function resolveExternalWebLinkHost(href: string | undefined): string | null { return resolveExternalWebLink(href)?.hostname || null; }

export interface ShowExternalLinkContextMenuOptions {
  href: string;
  /** Absent means yes, as in the reference. */
  canOpenInPreview?: boolean;
  threadLinkAction?: 'link-to-thread' | 'unlink-from-thread';
  showContextMenu: (items: MenuItem[]) => Promise<ExternalLinkContextMenuAction | null>;
  openInPreview: (href: string) => Promise<void>;
  openExternal: (href: string) => Promise<void>;
  copyLink: (href: string) => Promise<unknown>;
  updateThreadLink?: (href: string, linked: boolean) => Promise<void>;
  reportFailure: (operation: ExternalLinkContextMenuFailureOperation, cause: unknown) => void;
}

/** showExternalLinkContextMenu: the menu, then the picked action; a failure is reported by its operation. */
export async function showExternalLinkContextMenu(options: ShowExternalLinkContextMenuOptions): Promise<void> {
  const { href, canOpenInPreview = true, threadLinkAction } = options;
  let action: ExternalLinkContextMenuAction | null;
  try { action = await options.showContextMenu(externalLinkContextMenuItems({ canOpenInPreview, ...(threadLinkAction ? { threadLinkAction } : {}) })); }
  catch (cause) { options.reportFailure('show-link-context-menu', cause); return; }
  try {
    if (action === 'open-in-preview') await options.openInPreview(href);
    else if (action === 'open-external') await options.openExternal(href);
    else if (action === 'copy-link') await options.copyLink(href);
    else if (action === 'link-to-thread' || action === 'unlink-from-thread') await options.updateThreadLink?.(href, action === 'link-to-thread');
  } catch (cause) {
    if (action) options.reportFailure(FAILURE_OPERATION_BY_ACTION[action], cause);
  }
}

/** `chatlocal:link-menu` (value = the link's href): ChatMarkdown's anchor `onContextMenu` over the open thread. */
export async function chatExternalLinkMenu(client: T3Client, native: Native, storage: Files | undefined, href: string): Promise<string> {
  if (!href || !resolveExternalWebLinkHost(href)) return '';
  // A new thread's draft has its thread ref from the start in the reference (a file preview's links offer the Browser
  // there); here its id is allocated when the Browser opens, as the Browser surface does (addBrowserSurface).
  const draft = !client.threadId && !activeRef(client) && !!client.environmentId && !!client.projectId;
  const threadLinkAction = storage ? chatLinkThreadAction(client, href) : undefined;
  await showExternalLinkContextMenu({
    href,
    canOpenInPreview: canOpenLinksInApp(!!activeRef(client) || draft, native),
    ...(threadLinkAction ? { threadLinkAction } : {}),
    showContextMenu: async items => (await showContextMenu(client, native, items) || null) as ExternalLinkContextMenuAction | null,
    openInPreview: async target => {
      if (draft && !activeRef(client)) await ensureDraftThreadId(client, native);
      const ref = activeRef(client);
      if (!ref) throw new Error('Thread context is unavailable.');
      try { await openUrlInPreview(client, native, ref, target); }
      catch (error) {
        if (letGo(error)) throw error;
        if (error instanceof BrowserSettingsReadError) pushToast(client, { kind: 'error', title: 'Unable to open link in browser', description: error.message });
        throw error;
      }
    },
    openExternal: target => openInSystemBrowser(native, target),
    copyLink: async target => {
      const reply = await bridgeReply(native, { op: 'copyText', text: target });
      if (!reply.ok) throw new Error(reply.error?.message || 'The clipboard is unavailable.');
    },
    updateThreadLink: async (target, linked) => { if (storage) await changeChatLink(client, native, storage, target, linked); },
    reportFailure: (operation, cause) => {
      if (letGo(cause)) throw cause;
      // reportMarkdownActionFailure logs; only the thread link's failures are a toast.
      if (operation === 'link-pull-request-to-thread' || operation === 'unlink-pull-request-from-thread') {
        pushToast(client, { kind: 'error', title: operation === 'link-pull-request-to-thread' ? 'Unable to link pull request' : 'Unable to unlink pull request',
          description: cause instanceof Error ? cause.message : 'The request failed.' });
      }
    },
  });
  return '';
}
