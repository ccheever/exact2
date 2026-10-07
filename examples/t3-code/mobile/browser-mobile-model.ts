// @ref llp/1107.010-mobile-browser-devices.decision.md#connection-and-command-ownership
// Pinned365aa87982 mobile state/preview.ts and browserTabs.ts, projected through the existing RPC client.
import { arr, obj, str, num, type Obj } from './shared/domain';

export interface BrowserTabs { serverEpoch: string | null; revision: number; sessions: Obj[]; listed: boolean }
export const emptyBrowserTabs = (): BrowserTabs => ({ serverEpoch: null, revision: 0, sessions: [], listed: false });
export function applyBrowserEvent(current: BrowserTabs, event: Obj): BrowserTabs {
  if (num(event.revision) <= current.revision) return current;
  const index = current.sessions.findIndex(session => session.tabId === event.tabId), sessions = [...current.sessions];
  if (event.type === 'closed') { if (index !== -1) sessions.splice(index, 1); }
  else if (event.type === 'failed') {
    const existing = sessions[index];
    if (existing) sessions[index] = { ...existing, navStatus: { _tag: 'LoadFailed', url: event.url, title: event.title,
      code: event.code, description: event.description }, updatedAt: event.createdAt };
  } else sessions[index === -1 ? sessions.length : index] = obj(event.snapshot);
  return { ...current, serverEpoch: str(event.serverEpoch), revision: num(event.revision), sessions };
}
export function applyBrowserList(result: Obj, events: Obj[]): BrowserTabs {
  return events.filter(event => event.serverEpoch === result.serverEpoch && num(event.revision) > num(result.revision))
    .reduce(applyBrowserEvent, { serverEpoch: str(result.serverEpoch), revision: num(result.revision), sessions: arr(result.sessions), listed: true });
}
export function latestBrowserTab(tabs: Obj[]): Obj | null {
  return tabs.reduce<Obj | null>((latest, tab) => !latest || str(tab.updatedAt) > str(latest.updatedAt) ? tab : latest, null);
}
export const browserTabUrl = (tab: Obj) => obj(tab.navStatus)._tag === 'Idle' ? '' : str(obj(tab.navStatus).url);
export function browserTabTitle(tab: Obj) {
  const status = obj(tab.navStatus); if (status._tag === 'Idle') return 'New tab';
  if (str(status.title).trim()) return str(status.title).trim();
  try { return new URL(str(status.url)).host || str(status.url); } catch { return str(status.url); }
}
/** Same normalization branches as pinned shared/preview.ts; user-facing source alert handles errors. */
export function normalizeMobilePreviewUrl(raw: string): string {
  const value = raw.trim(); if (!value) throw new Error('Enter an http or https URL.');
  const loopback = /^(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1?\])(?::|\/|$)/i.test(value);
  const parsed = new URL(value.includes('://') ? value : `${loopback ? 'http' : 'https'}://${value}`);
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') throw new Error('Enter an http or https URL.');
  return parsed.href;
}
export const browserZoomLevels = [0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4, 5];
