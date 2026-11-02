// T3 Code v0.1.6, apps/desktop/src/preview/Manager.ts `openPictureInPicture`, `closePictureInPicture` and the frame
// capture it starts (MIT; see LICENSE-T3). A desktop tab's separate preview window: Electron's floating panel titled
// "Preview · <the page's title>", here Exact's panel (`requestPictureInPicture(id, presentation="panel")`, LLP 1042
// §9.1) over a hidden image of the tab's frame, captured about 12 times a second (server-browser-members.contract
// `desktopPipCapture`). Each tab has its own window, as the reference keeps one per tab (LLP 1042 §9.2).
import type { T3Client } from '../client/client';
import { appFiles } from '../media/save-file';
import { letGo } from '../client/let-go';
import { automationCalls } from './browser-automation-run';
import { frameEntry, type BrowserDesktopPipView } from './browser-frames';

type DesktopPip = { runtimeId: string; frameId: string; title: string; request: number };
/** The open windows by tab, in the order they opened (`pictureInPictureSessionsRef`). */
const pips = new WeakMap<T3Client, Map<string, DesktopPip>>();
let requests = 0;

const pipsOf = (client: T3Client): Map<string, DesktopPip> => {
  let open = pips.get(client);
  if (!open) pips.set(client, open = new Map());
  return open;
};

/** Whether the tab's separate window is open. */
export const desktopPipOpen = (client: T3Client, runtimeId: string): boolean => pips.get(client)?.has(runtimeId) ?? false;

/** openPictureInPicture: the title is read once, as the window is made ("Browser preview" for a page with none); a tab
 *  whose window is open keeps it. */
export function openDesktopPictureInPicture(client: T3Client, runtimeId: string): void {
  const entry = frameEntry(client, runtimeId);
  if (!entry || desktopPipOpen(client, runtimeId)) return;
  const title = (entry.record?.title ?? '').trim();
  pipsOf(client).set(runtimeId, { runtimeId, frameId: entry.frameId, title: title ? `Preview · ${title}` : 'Browser preview', request: ++requests });
}

/** closePictureInPicture, the window's own close among them; with no tab, closeAllPictureInPicture. */
export function closeDesktopPictureInPicture(client: T3Client, runtimeId?: string): void {
  if (runtimeId === undefined) pips.delete(client);
  else pips.get(client)?.delete(runtimeId);
}

/** The windows as the frame layer shows them; a tab whose frame is gone shows none. */
export function desktopPipViews(client: T3Client): BrowserDesktopPipView[] {
  return [...pips.get(client)?.values() ?? []].filter(pip => frameEntry(client, pip.runtimeId)).map(pip => ({ show: true, ...pip }));
}

/** Each window's next picture: its tab's frame into one of three files in turn (the window reads the last while the
 *  next is written). `seq` moves on with every round, so a failed capture keeps the loop going with the picture before. */
let shots = 0;
const latest = new WeakMap<T3Client, Map<number, string>>();
export async function captureDesktopPips(client: T3Client): Promise<{ shots: { path: string; request: number }[]; seq: number }> {
  const seq = ++shots, before = latest.get(client) ?? new Map<number, string>(), after = new Map<number, string>();
  const taken: { path: string; request: number }[] = [];
  for (const pip of desktopPipViews(client)) {
    const path = `app:/tmp/browser-frames/pip-${pip.request}-${seq % 3}.png`;
    try {
      await appFiles().fs.mkdir('app:/tmp/browser-frames', { recursive: true });
      await automationCalls().captureScreenshot({ id: pip.frameId, path });
      after.set(pip.request, path);
    } catch (error) {
      if (letGo(error)) throw error;
      if (before.has(pip.request)) after.set(pip.request, before.get(pip.request)!);
    }
    if (after.has(pip.request)) taken.push({ path: after.get(pip.request)!, request: pip.request });
  }
  latest.set(client, after);
  return { shots: taken, seq };
}
