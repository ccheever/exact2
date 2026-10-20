// T3 Code 9c161462c9 (MIT, see LICENSE-T3): closedViewStore.ts and reopenClosedView.ts.
// The newest user-closed view wins across threads; terminals are deliberately excluded.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import type { Files, Native } from '../client/protocol';
import { threadOps } from '../client/client-ops-threads';
import { ensureDraftThreadId } from '../composer/draft-thread-id';
import { activeRef } from '../terminal/terminal-drawer-view';
import { fleet } from '../connections/settings-b-fleet';
import { parseScopedThreadKey } from '../terminal/terminal-ui-state';
import { browserHost, openBrowserIn, openPreviewSession, syncNativeSessions } from '../browser/browser-surface';
import { readSnapshot, type PreviewSessionSnapshot } from '../browser/browser-state';
import { INCOGNITO_BROWSER_PROFILE_ID } from '../browser/browser-profiles';
import { parseSelection } from '../pull-requests/pages-pr-detail';
import { prSurfaceId } from './pr-surface';
import { savedPanel } from './saved-panel';
import { panelState, surfaceStore, surfaceLocal, openFileIn, type PanelState, type Surface } from './panel-surfaces';
import { ensureFile } from './files-surface';

export type ClosedView = { id: string; key: string; surface: Surface; snapshot?: PreviewSessionSnapshot; draft?: true; projectId?: string; page?: string };
export type ReopenOwner = { environmentKnown: boolean; catalogReady: boolean; ownerExists: boolean; shellLive: boolean; panel: Pick<PanelState, 'visible' | 'surfaces'> };
const live = new WeakMap<object, ClosedView[]>();
type Holder = { local: object };
const persistent = (entry: ClosedView) => entry.snapshot?.profileId !== INCOGNITO_BROWSER_PROFILE_ID;

/** Decode persisted history through the same validation as panel restoration; incognito stays in memory. */
export function adoptClosedViews(next: object, saved: Obj): void {
  const entries: ClosedView[] = [];
  for (const raw of arr(saved.closedViews).slice(0, 20)) {
    const key = str(raw.key), id = str(raw.id), surface = obj(raw.surface);
    if (!id || !parseScopedThreadKey(key)) continue;
    if (surface.kind === 'browser') {
      const snapshot = readSnapshot(raw.snapshot);
      if (snapshot && snapshot.profileId !== INCOGNITO_BROWSER_PROFILE_ID)
        entries.push({ id, key, snapshot, surface: { id: `browser:${snapshot.tabId}`, kind: 'browser', path: '', line: 0, reveal: 0, browser: { tabId: snapshot.tabId, threadKey: key } } });
    } else {
      const kept = savedPanel({ surfaces: [{ ...surface, id: str(surface.id), kind: str(surface.kind), path: str(surface.path), line: Number(surface.line) || 0 }], active: str(surface.id), visible: true });
      const restored = kept?.surfaces[0];
      if (restored && restored.kind !== 'terminal') entries.push({ id, key, surface: { ...restored, reveal: 0 } });
    }
  }
  for (const entry of entries) {
    const raw = arr(saved.closedViews).find(candidate => candidate.id === entry.id);
    if (raw?.draft === true && str(raw.projectId)) { entry.draft = true; entry.projectId = str(raw.projectId); }
    if (entry.surface.kind === 'pull-request' && parseSelection(str(raw?.page))) entry.page = str(raw?.page);
  }
  (next as { closedViews?: ClosedView[] }).closedViews = entries;
}
export function closedViews(owner: Holder): ClosedView[] {
  let entries = live.get(owner);
  if (!entries) { entries = [...((owner.local as { closedViews?: ClosedView[] }).closedViews ?? [])]; live.set(owner, entries); }
  return entries;
}
function replace(owner: Holder, entries: ClosedView[]): void {
  live.set(owner, entries);
  (owner.local as { closedViews?: ClosedView[] }).closedViews = entries.filter(persistent);
}
/** Capture before native cleanup removes the Browser snapshot. Remember only after the tab closes. */
export function captureClosedView(client: T3Client, key: string, surface: Surface): ClosedView | null {
  if (surface.kind === 'terminal') return null;
  const ref = surface.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
  const snapshot = ref && surface.browser ? browserHost(client).store.read(ref).sessions[surface.browser.tabId] : undefined;
  if (surface.kind === 'browser' && !snapshot) return null;
  return { id: `closed:${key}:${surface.id}`, key, ...(!client.threadId ? { draft: true as const, projectId: client.projectId } : {}), surface: { ...surface }, ...(snapshot ? { snapshot } : {}) };
}
export function rememberClosedView(owner: Holder, entry: ClosedView | null): void {
  if (!entry) return;
  replace(owner, [entry, ...closedViews(owner).filter(other => other.key !== entry.key || other.surface.id !== entry.surface.id)].slice(0, 20));
}
/** Unknown ownership waits; deleted threads are skipped and kept; removed environments and open tabs drop. */
export function planNextReopen(entries: readonly ClosedView[], ownerOf: (entry: ClosedView) => ReopenOwner): { drop: ClosedView[]; restore: ClosedView | null } {
  const drop: ClosedView[] = [];
  for (const entry of entries) {
    const owner = ownerOf(entry);
    if (!owner.environmentKnown) { if (!owner.catalogReady) break; drop.push(entry); continue; }
    if (!owner.ownerExists) { if (owner.shellLive) continue; break; }
    const open = (entry.surface.kind === 'browser' || owner.panel.visible) && owner.panel.surfaces.some(surface => surface.id === entry.surface.id);
    if (open) { drop.push(entry); continue; }
    return { drop, restore: entry };
  }
  return { drop, restore: null };
}
export async function reopenClosedView(client: T3Client, native: Native, storage: Files): Promise<string> {
  const entries = closedViews(client);
  const plan = planNextReopen(entries, entry => {
    const ref = parseScopedThreadKey(entry.key)!;
    const focused = ref.environmentId === client.environmentId;
    const background = [...fleet.entries.values()].find(candidate => candidate.environmentId === ref.environmentId);
    const shell = focused ? client.shell : background?.shell;
    const draft = entry.draft === true || ref.threadId.startsWith('new:');
    const ownerExists = !!entry.page || (draft ? shell?.projects.some(project => project.id === (entry.projectId || ref.threadId.slice(4))) === true
      : shell?.threads.some(thread => thread.id === ref.threadId && thread.deletedAt == null) === true);
    return { environmentKnown: focused || !!background, catalogReady: client.ready, ownerExists,
      shellLive: focused ? client.ready : background?.phase === 'connected' && background.synchronized === background.generation,
      panel: entry.page ? { visible: openPages.get(client) === entry.page, surfaces: openPages.get(client) === entry.page ? [entry.surface] : [] } : surfaceStore(client).panels.get(entry.key) ?? { surfaces: [], visible: false } };
  });
  replace(client, entries.filter(entry => !plan.drop.includes(entry)));
  const entry = plan.restore;
  if (!entry) return '';
  if (entry.page) { replace(client, closedViews(client).filter(other => other.id !== entry.id)); return `sidebar:pr-page:${entry.page}`; }
  const ref = parseScopedThreadKey(entry.key)!;
  const draft = entry.draft === true || ref.threadId.startsWith('new:');
  const id = ref.environmentId === client.environmentId ? ref.threadId : `fleet:${ref.environmentId}:${ref.threadId}`;
  const surface = entry.surface;
  // ReopenClosedViewShortcut's `workspaceAvailable: project !== null`: the owner's project, read before any navigation. A
  // Files or file tab whose owner has none is not reopened, and its entry is deferred to the end.
  const shell = ref.environmentId === client.environmentId ? client.shell : [...fleet.entries.values()].find(candidate => candidate.environmentId === ref.environmentId)?.shell;
  const ownerProject = draft ? entry.projectId || ref.threadId.slice(4) : str(shell?.threads.find(thread => thread.id === ref.threadId)?.projectId);
  if ((surface.kind === 'files' || (surface.kind === 'file' && !surface.attachment)) && !shell?.projects.some(project => project.id === ownerProject)) {
    replace(client, [...closedViews(client).filter(other => other.id !== entry.id), entry]);
    return '';
  }
  try {
    const target = draft ? (ref.environmentId === client.environmentId ? (entry.projectId || ref.threadId.slice(4)) : `fleet:${ref.environmentId}:${entry.projectId || ref.threadId.slice(4)}`) : id;
    // The owning thread already shown is not selected again: that would hide its Diff and restart its stream.
    const shown = !draft && ref.environmentId === client.environmentId && ref.threadId === client.threadId;
    if (!shown) await threadOps.call(client, draft ? 'new-thread' : 'select-thread', target, '', 0, native, storage, { message: '', id: target, value: '' });
    const state = panelState(client);
    if (entry.snapshot) {
      const snapshot = entry.snapshot;
      if (!client.threadId && !activeRef(client)) await ensureDraftThreadId(client, native);
      const restoredRef = activeRef(client);
      if (!restoredRef) throw new Error('The Browser tab has no owning thread.');
      const reopened = await openPreviewSession((method, payload) => client.rpc(native, method, payload, true), browserHost(client).store, restoredRef,
        { ...(snapshot.navStatus._tag === 'Idle' ? {} : { url: snapshot.navStatus.url }), viewport: snapshot.viewport, profileId: snapshot.profileId });
      client.diffOpen = false; openBrowserIn(state, reopened.tabId, `${restoredRef.environmentId}:${restoredRef.threadId}`);
      await syncNativeSessions(client, native);
    } else if (surface.kind === 'file' && !surface.attachment) {
      // panels.openFile on the owning thread: the tab goes back at the end, active; its read follows.
      client.diffOpen = false;
      const opened = openFileIn(state, surface.path, surface.line);
      if (opened.kind === 'file') await ensureFile(client, native, opened.path, true);
    }
    else {
      if (!state.surfaces.some(other => other.id === surface.id)) state.surfaces.push({ ...surface });
      await surfaceLocal(client, native, 'activate', surface.id, '');
    }
    replace(client, closedViews(client).filter(other => other.id !== entry.id));
    return '';
  } catch (error) {
    replace(client, [...closedViews(client).filter(other => other.id !== entry.id), entry]);
    throw error;
  }
}

const openPages = new WeakMap<object, string>();
export function notePagePullRequest(client: object, selected: string, visible: boolean): void { openPages.set(client, visible ? selected : ''); }
export function rememberPagePullRequest(client: T3Client, selected: string): void {
  const selection = parseSelection(selected);
  if (!selection) return;
  const key = `${selection.environmentId || client.environmentId}:pull-requests`;
  const pr = { projectId: selection.projectId, host: selection.host, repository: selection.repository, number: selection.number, url: '' };
  rememberClosedView(client, { id: `closed-page:${selected}`, key, page: selected,
    surface: { id: prSurfaceId(pr), kind: 'pull-request', path: '', line: 0, reveal: 0, pr } });
  openPages.set(client, '');
}
