// T3 Code v0.1.6, apps/desktop/src/preview/Manager.ts `openPictureInPicture`, `closePictureInPicture` and the frame
// capture it starts (MIT; see LICENSE-T3). A desktop tab's separate preview window: Electron's floating panel titled
// "Preview · <the page's title>", here Exact's panel (`requestPictureInPicture(id, presentation="panel")`, LLP 1042
// §9.1) over a hidden image of the tab's frame, captured about 12 times a second (server-browser-members.contract
// `desktopPipCapture`). One image floats at a time in Exact, so opening one tab's window closes another's.
import type { T3Client } from '../client/client';
import { appFiles } from '../media/save-file';
import { letGo } from '../client/let-go';
import { automationCalls } from './browser-automation-run';
import { EMPTY_DESKTOP_PIP, frameEntry, type BrowserDesktopPipView } from './browser-frames';

type DesktopPip = { runtimeId: string; frameId: string; title: string; request: number };
const pips = new WeakMap<T3Client, DesktopPip>();
let requests = 0;

/** The tab whose separate window is open, or null. */
export const desktopPipRuntimeOf = (client: T3Client): string | null => pips.get(client)?.runtimeId ?? null;

/** openPictureInPicture: the title is read once, as the window is made ("Browser preview" for a page with none). */
export function openDesktopPictureInPicture(client: T3Client, runtimeId: string): void {
  const entry = frameEntry(client, runtimeId);
  if (!entry) return;
  const title = (entry.record?.title ?? '').trim();
  pips.set(client, { runtimeId, frameId: entry.frameId, title: title ? `Preview · ${title}` : 'Browser preview', request: ++requests });
}

/** closePictureInPicture, the window's own close among them. */
export function closeDesktopPictureInPicture(client: T3Client, runtimeId?: string): void {
  if (runtimeId === undefined || pips.get(client)?.runtimeId === runtimeId) pips.delete(client);
}

/** The window as the frame layer shows it; a tab whose frame is gone shows none. */
export function desktopPipView(client: T3Client): BrowserDesktopPipView {
  const pip = pips.get(client);
  return pip && frameEntry(client, pip.runtimeId) ? { show: true, ...pip } : { ...EMPTY_DESKTOP_PIP };
}

/** The window's next picture: the tab's frame into one of three files in turn (the window reads the last while the next
 *  is written). `seq` moves on with every attempt, so a failed capture keeps the loop going with the picture before. */
let shots = 0;
const latest = new WeakMap<T3Client, { request: number; path: string }>();
export async function captureDesktopPip(client: T3Client, runtimeId: string, request: number): Promise<{ path: string; request: number; seq: number }> {
  const pip = pips.get(client), seq = ++shots;
  const before = latest.get(client)?.request === request ? latest.get(client)!.path : '';
  if (!pip || pip.runtimeId !== runtimeId || pip.request !== request) return { path: '', request, seq };
  const path = `app:/tmp/browser-frames/pip-${request}-${seq % 3}.png`;
  try {
    await appFiles().fs.mkdir('app:/tmp/browser-frames', { recursive: true });
    await automationCalls().captureScreenshot({ id: pip.frameId, path });
  } catch (error) {
    if (letGo(error)) throw error;
    return { path: before, request, seq };
  }
  latest.set(client, { request, path });
  return { path, request, seq };
}
