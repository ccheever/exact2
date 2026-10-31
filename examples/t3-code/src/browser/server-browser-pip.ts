// T3 Code v0.1.5, apps/web/src/browser/serverPictureInPicture.ts (MIT; see LICENSE-T3). A server-rendered tab floats in
// the platform's picture-in-picture window: a stream of its own (the reference's `interactive: false` client), kept
// outside the floating player so the window outlives the player, paints a hidden image that Exact's
// `requestPictureInPicture` floats (browser-frames.contract `BrowserFrameLayer`, LLP 1042 §9). The window's own close is
// the image's `leavepictureinpicture`; closing it here unmounts the image, which closes the window.
import type { T3Client } from '../client/client';
import { serverStreamFrame, serverStreamUrl } from './server-browser-runtime';

type ServerPip = { key: string; runtimeId: string; threadId: string; tabId: string; seed: string; request: number };
const pips = new WeakMap<T3Client, ServerPip>();
let requests = 0;

/** serverPictureInPictureKey: one window per tab of a thread. */
export const serverPictureInPictureKey = (threadId: string, tabId: string): string => JSON.stringify([threadId, tabId]);
/** The window's stream is its own, beside the tab's and the floating player's (server-browser-runtime.ts `runtime`). */
export const serverPipRuntimeId = (runtimeId: string): string => `${runtimeId}:pip`;

/** useServerPictureInPictureKey */
export const serverPictureInPictureKeyOf = (client: T3Client): string | null => pips.get(client)?.key ?? null;
/** The floating window's stream, which the panel keeps alive while it floats (browser-surface.ts `browserPrepare`). */
export const serverPipOf = (client: T3Client): Readonly<ServerPip> | null => pips.get(client) ?? null;

export function closeServerPictureInPicture(client: T3Client): void { pips.delete(client); }

/** openServerPictureInPicture, seeded with the picture the floating player shows now. */
export function openServerPictureInPicture(client: T3Client, input: { threadId: string; tabId: string; runtimeId: string }): void {
  closeServerPictureInPicture(client);
  pips.set(client, { key: serverPictureInPictureKey(input.threadId, input.tabId), runtimeId: serverPipRuntimeId(input.runtimeId),
    threadId: input.threadId, tabId: input.tabId, seed: serverStreamFrame(client, input.runtimeId).image, request: ++requests });
}

/** The reference's `onLeave` causes beside the window's close: a refused stream after its retries, a session the server
 *  ended, and a PDF, which renders in the client and never reaches the server's stream. */
export function settleServerPictureInPicture(client: T3Client, tab: { threadId: string; tabId: string; pdf: boolean } | null): void {
  const pip = pips.get(client);
  if (!pip) return;
  const frame = serverStreamFrame(client, pip.runtimeId);
  if (frame.refused || frame.gone || frame.setupNeed) closeServerPictureInPicture(client);
  else if (tab?.pdf && tab.threadId === pip.threadId && tab.tabId === pip.tabId) closeServerPictureInPicture(client);
}

/** The hidden image the window floats (browser-shapes.contract `BrowserPip`): the seed until its stream's first picture. */
export type BrowserPipView = { show: boolean; runtimeId: string; streamUrl: string; seed: string; request: number };
export const emptyBrowserPip = (): BrowserPipView => ({ show: false, runtimeId: '', streamUrl: '', seed: '', request: 0 });
export function browserPipView(client: T3Client): BrowserPipView {
  const pip = pips.get(client);
  return pip ? { show: true, runtimeId: pip.runtimeId, streamUrl: serverStreamUrl(client, pip.runtimeId), seed: pip.seed, request: pip.request } : emptyBrowserPip();
}
