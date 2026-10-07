// @ref llp/1107.011-responsive-workspace.decision.md#navigation-and-data-ownership
// Root owns serialized UI state. These projections reuse the existing client caches.
import { mobileClient } from './client';
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { workspaceOf } from './shared/r4-surfaces-panel';
import { mobileWorkspaceEntries } from './mobile-workspace';
import { workspaceInspectorFocusOwner, workspaceInspectorSnapshot, workspaceInspectorTransition, type WorkspaceInspectorInput, type WorkspaceInspectorEvent } from './workspace-inspector';
import { mobileFilesSnapshot, type FileTreeSnapshot } from './file-data';
import { mobileReviewSnapshot, type ReviewSnapshot } from './review-data';
import { EMPTY_MOBILE_GIT, mobileGitSnapshot, type MobileGitSnapshot } from './git-overview';
import { mobileReviewColors, mobileReviewNavigatorAppearance } from './review-colors';

export function mobileInspectorContext(entries: unknown, candidateSupported: boolean, columnSupported: boolean, columnWidth: number, resizing: boolean, reducedMotion: boolean,
  dark: boolean, client: T3Client = mobileClient) {
  const routes = mobileWorkspaceEntries(entries), top = routes.at(-1), current = workspaceOf(client);
  const selected = client.ready && !!client.threadId && top?.params.threadEnvironment === client.environmentId && top.params.threadId === client.threadId
    && client.shell.threads.some(thread => thread.id === client.threadId);
  const kind = top?.name === 'thread' ? 'thread' : top?.name === 'threadFiles' ? 'files' : top?.name === 'threadFile' ? 'file'
    : top?.name === 'threadReview' ? 'review' : 'other';
  const review = kind === 'review' ? mobileReviewSnapshot(dark, client) : null;
  const renderable = !!selected && (kind === 'review' ? !!review?.files.length && !review.error && !review.raw
    : kind === 'files' ? !!current.cwd && !mobileFilesSnapshot('', client).error : true);
  const input: WorkspaceInspectorInput = { focus: top ? { routeId: top.id, kind, environmentId: top.params.threadEnvironment ?? '',
    threadId: top.params.threadId ?? '', generation: client.generation, cwd: selected ? current.cwd : '',
    selectedPath: kind === 'file' ? top?.params.filePath ?? '' : '', renderable, candidateSupported } : null,
    liveRouteIds: routes.map(route => route.id), now: 0, reducedMotion, columnSupported, columnWidth, resizing };
  const serialized = JSON.stringify(input);
  return { serialized, key: serialized, focusOwner: workspaceInspectorFocusOwner(input.focus), hasWorkspace: !!selected && !!current.cwd };
}
function parse(input: string): Record<string, unknown> { try { return obj(JSON.parse(input)); } catch { return {}; } }
const emptyFiles: FileTreeSnapshot = { owner: '', revision: 0, title: '', query: '', selectedPath: '', rows: [], loading: false,
  error: '', truncated: false, emptyTitle: '', emptyDetail: '' };
interface Content { owner: string; environmentId: string; threadId: string; generation: number; cwd: string;
  files: FileTreeSnapshot; reviewFiles: ReviewSnapshot['files']; reviewSelected: string; git: MobileGitSnapshot }
/** Freeze outgoing render data; a new selected thread cannot paint into its pane. */
function captureContent(serialized: string, view: ReturnType<typeof workspaceInspectorSnapshot>, now: number, dark: boolean, client: T3Client) {
  const old = parse(serialized), scope = workspaceOf(client);
  if (!view.contentOwner) return '';
  const matches = view.contentEnvironmentId === client.environmentId && view.contentThreadId === client.threadId
    && view.contentGeneration === client.generation && view.cwd === scope.cwd;
  if (!matches) return old.owner === view.contentOwner ? serialized : '';
  const review = mobileReviewSnapshot(dark, client);
  const next: Content = { owner: view.contentOwner, environmentId: client.environmentId, threadId: client.threadId,
    generation: client.generation, cwd: scope.cwd, files: mobileFilesSnapshot(view.selectedPath, client),
    reviewFiles: review.files, reviewSelected: review.selectedPath, git: mobileGitSnapshot(now, client) };
  return JSON.stringify(next);
}
export function mobileInspectorTransition(serialized: string, context: string, kind: string, value: string, now: number,
  content: string, dark: boolean, client: T3Client = mobileClient) {
  const parsed = parse(context), focus = obj(parsed.focus);
  const input: WorkspaceInspectorInput = { focus: typeof focus.routeId === 'string' ? focus as unknown as WorkspaceInspectorInput['focus'] : null,
    liveRouteIds: Array.isArray(parsed.liveRouteIds) ? parsed.liveRouteIds.filter((id): id is string => typeof id === 'string') : [],
    columnSupported: parsed.columnSupported === true, columnWidth: Number(parsed.columnWidth) || 0, resizing: parsed.resizing === true, reducedMotion: parsed.reducedMotion === true, now };
  const event: WorkspaceInspectorEvent | undefined = kind === 'files' || kind === 'git' ? { kind: 'mode', owner: value, mode: kind }
    : kind === 'deadline' || kind === 'end-exit' || kind === 'deactivate' || kind === 'release-role' ? { kind, token: value } : undefined;
  const result = workspaceInspectorTransition(serialized, input, event);
  return { before: serialized, contextKey: context, serialized: result.serialized,
    content: captureContent(content, result, now, dark, client), revealInspector: result.revealInspector };
}
export function mobileInspectorPresentation(content: string, serialized: string, visible: boolean, query: string, scheme: string, themeId: string,
  client: T3Client = mobileClient, baseFontSize = 16) {
  const view = workspaceInspectorSnapshot(serialized), stored = parse(content), valid = stored.owner === view.contentOwner && !!view.contentOwner;
  const colors = mobileReviewColors(scheme, themeId), files = valid && Array.isArray(obj(stored.files).rows) ? stored.files as unknown as FileTreeSnapshot : emptyFiles;
  const reviewFiles = valid && Array.isArray(stored.reviewFiles) ? stored.reviewFiles as ReviewSnapshot['files'] : [];
  const git = valid && obj(stored.git).owner ? stored.git as MobileGitSnapshot : EMPTY_MOBILE_GIT;
  const kind = view.kind === 'changed-files' ? 'changes' : view.contentMode === 'git' ? 'git' : 'files';
  const title = kind === 'changes' ? 'Changed files' : kind === 'git' ? git.branchLabel : 'Files';
  const subtitle = kind === 'changes' ? `${reviewFiles.length} file${reviewFiles.length === 1 ? '' : 's'}` : kind === 'files' ? files.title : '';
  const registration = parse(view.registrationJSON);
  return { title, files, reviewFiles, navigator: mobileReviewNavigatorAppearance(scheme, themeId, baseFontSize), reviewSelected: valid ? str(stored.reviewSelected) : '', git,
    showFiles: valid && kind === 'files', showChanges: valid && kind === 'changes', showGit: valid && kind === 'git',
    configuration: JSON.stringify({ routeKey: 't3-workspace-inspector', owner: str(registration.token), kind, title, subtitle, query,
      visible: visible && view.active && valid, foreground: colors.foreground, muted: colors.muted, sheet: colors.sheet, border: colors.border }) };
}
