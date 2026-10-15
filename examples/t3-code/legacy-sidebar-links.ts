// The legacy sidebar row's links (T3 Code 1e2ecbd975; MIT, see LICENSE-T3): the pull request glyph
// (LegacySidebar.tsx handlePrClick over lib/openPullRequestLink.ts useOpenPrLink and
// useOpenChangeRequestLink) and "New thread on <branch>" (handleThreadContextMenu over
// hooks/useHandleNewThread.ts with the thread's branch, worktree and mode); the discovered-port
// button is legacy-sidebar-ports.ts. Each targets its row's thread, wherever it lives.
import { obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { letGo } from './let-go';
import { pushToast } from './toast';
import { scopedThreadKey, selectThreadTerminalUiState, terminalUiStore, type ScopedThreadRef } from './terminal-ui-state';
import { surfaceStore, type PanelState } from './r4-surfaces-panel';
import { openPullRequestIn } from './r5-panels-surfaces';
import { prTarget } from './r5-panels-pr';
import { currentPullRequestLink } from './shell-pr';
import { branchPullRequest } from './sidebar-branch-pr';
import { findProjectForChangeRequest, parseChangeRequestUrl } from './palette-linkpr';
import { openInSystemBrowser, openLink } from './browser-links';
import { patchDraftContext } from './composer-controls-branch';
import { fleet, focusFleetThread, parseFleetThreadId, type EnvironmentFleet } from './settings-b-fleet';
import { sidebarSession } from './sidebar-state';

interface Navigator { openSelected?(native: Native, id: string): Promise<void>; openDraft?(native: Native, projectId: string): Promise<void> }
const failure = (error: unknown, fallback: string) => error instanceof Error && error.message ? error.message : fallback;

/** A row's thread (`fleet:<environment>:<id>` for a background environment's) with its environment's reads. */
export interface RowThread { thread: Obj; ref: ScopedThreadRef; focused: boolean; reads: { config: Obj; shell: { projects: Obj[] } } }
export function rowThread(client: T3Client, id: string, source: EnvironmentFleet = fleet): RowThread | null {
  const fleetRef = parseFleetThreadId(id);
  if (!fleetRef || fleetRef.environmentId === client.environmentId) {
    const threadId = fleetRef?.threadId ?? id, thread = client.shell.threads.find(entry => entry.id === threadId);
    return thread ? { thread, ref: { environmentId: client.environmentId, threadId }, focused: true, reads: { config: client.config, shell: client.shell } } : null;
  }
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === fleetRef.environmentId && candidate.phase === 'connected');
  const thread = entry?.shell.threads.find(candidate => str(candidate.id) === fleetRef.threadId);
  return entry && thread ? { thread, ref: fleetRef, focused: false, reads: { config: entry.config, shell: entry.shell } } : null;
}

/** rightPanelStore for a thread that may not be the shown one (its panel, by the thread's key). */
function panelOf(client: T3Client, ref: ScopedThreadRef): PanelState {
  const store = surfaceStore(client), key = scopedThreadKey(ref);
  let state = store.panels.get(key);
  if (!state) { state = { surfaces: [], active: '', visible: false, userRevision: 0 }; store.panels.set(key, state); }
  return state;
}
/** ChatView's effect on every thread change: the composer takes the focus (a frame later), unless that thread's terminal drawer is open. */
export function composerTakesFocus(client: T3Client, ref: ScopedThreadRef): boolean {
  return !selectThreadTerminalUiState(terminalUiStore(client).getState().terminalUiStateByThreadKey, ref).terminalOpen;
}
/**
 * navigateToThread: the selection clears and anchors on the row; a thread that is not open opens ('sidebar:opened',
 * with ':focus' when the composer takes the focus), a background environment's by focusing that environment
 * (select-thread, client-ops-threads.ts).
 */
export async function goTo(client: T3Client, native: Native, id: string, ref: ScopedThreadRef): Promise<string> {
  const session = sidebarSession(client);
  session.selection = []; session.anchor = id;
  if (ref.environmentId === client.environmentId && client.threadId === ref.threadId) return '';
  if (ref.environmentId !== client.environmentId) {
    const focused = await focusFleetThread(client, native, `fleet:${ref.environmentId}:${ref.threadId}`);
    (client as unknown as { adoptStatus?(value: Obj, generation: number): void }).adoptStatus?.(focused.value, focused.generation);
  } else await (client as unknown as Navigator).openSelected?.(native, ref.threadId);
  return composerTakesFocus(client, ref) ? 'sidebar:opened:focus' : 'sidebar:opened';
}

/**
 * handlePrClick (openPullRequestsInRightPanel = a thread's route): ⌘ or Ctrl leaves the link to the system
 * browser; beside a thread, a pull request a project of the thread's environment reads opens in that thread's right
 * panel and the thread opens; on another page, one an environment reads opens on the Pull Requests page; otherwise it
 * is an ordinary link, which follows "Open links in" ("Unable to open pull request link" if that fails).
 */
export async function openRowPullRequest(client: T3Client, native: Native, id: string, pressed: string): Promise<string> {
  // `mod:<page>`: the click held ⌘ or Ctrl (legacy-sidebar.contract `prPress`, the press's own MouseEvent).
  const held = pressed.startsWith('mod:'), page = held ? pressed.slice('mod:'.length) : pressed;
  const row = rowThread(client, id);
  // `prStatus?.url ?? currentLinkedPr?.url`: the link's, else the branch's pull request.
  const url = row ? str(currentPullRequestLink(row.thread.pullRequests)?.url) || str(branchPullRequest(client, { ...row.thread, id })?.url) : '';
  if (!row || !url) return '';
  const linkFailed = (error: unknown) => {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Unable to open pull request link', description: failure(error, 'An error occurred.'), stacked: true });
  };
  if (held) { try { await openInSystemBrowser(native, url); } catch (error) { linkFailed(error); } return ''; }
  // routeThreadRef (resolveActiveThreadRouteRef): a server thread's route, or a draft once it was sent; an unsent draft is none.
  const besideThread = page === '' && client.threadId !== '';
  if (besideThread) {
    const target = prTarget(row.reads as unknown as T3Client, url);
    if (target) {
      if (row.focused && client.threadId === row.ref.threadId) client.diffOpen = false;
      openPullRequestIn(panelOf(client, row.ref), target);
      return goTo(client, native, id, row.ref);
    }
  } else {
    const selection = pageSelection(client, url);
    if (selection) return `sidebar:pr-page:${selection}`;
  }
  try { await openLink(client, native, url, { threadRef: besideThread ? row.ref : null }); } catch (error) { linkFailed(error); }
  return '';
}
/** useOpenChangeRequestLink with no thread: the exact project across the environments that read pull requests, the focused first. */
function pageSelection(client: T3Client, url: string, source: EnvironmentFleet = fleet): string {
  const parsed = parseChangeRequestUrl(url);
  if (!parsed) return '';
  const capabilities = (config: Obj) => obj(obj(config.environment).capabilities);
  const environments = [{ environmentId: client.environmentId, focused: true, config: client.config, projects: client.shell.projects },
    ...[...source.entries.values()].filter(entry => entry.phase === 'connected' && entry.environmentId !== client.environmentId)
      .map(entry => ({ environmentId: entry.environmentId, focused: false, config: entry.config, projects: entry.shell.projects }))];
  for (const environment of environments) {
    if (capabilities(environment.config).pullRequests !== true) continue;
    const project = findProjectForChangeRequest(environment.projects, parsed);
    if (!project) continue;
    const many = capabilities(environment.config).threadPullRequests === true;
    const target = prTarget({ config: environment.config, shell: { projects: [project] } } as unknown as T3Client, url);
    if (!target) continue;
    return JSON.stringify({ projectId: target.projectId, host: many ? target.host : '', repository: target.repository, number: target.number,
      ...(environment.focused ? {} : { environmentId: environment.environmentId }) });
  }
  return '';
}

// ── "New thread on <branch>" (explicit branch carry-over) ─────────────────────────────────────
/** handleNewThread(projectRef, { branch, worktreePath, envMode: worktreePath ? "worktree" : "local", startFromOrigin: false }). */
export async function openBranchDraft(client: T3Client, native: Native, thread: Obj, projectId: string): Promise<string> {
  try { await (client as unknown as Navigator).openDraft?.(native, projectId); }
  catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not create thread', description: failure(error, 'An error occurred.'), stacked: true });
    return '';
  }
  const worktreePath = str(thread.worktreePath);
  // A worktree it already has, or its branch on the local checkout: neither starts a new worktree from origin.
  patchDraftContext(client, { branch: str(thread.branch), worktreePath, envMode: worktreePath ? 'worktree' : 'local' });
  return 'sidebar:new-thread';
}
