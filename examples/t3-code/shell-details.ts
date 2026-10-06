// The thread details panel's data (MIT reference: chat/ThreadDetailsPanel.tsx,
// BranchToolbar panel sections, chat/OpenInPicker.tsx, editorPreferences.ts,
// GitActionsControl.logic.ts resolveQuickAction, BranchToolbarEnvironmentSelector):
// where the thread runs, the workspace folder, the preferred editor, the
// version-control rows from vcs.refreshStatus and the lineage (shell-lineage.ts).
import type { T3Client } from './client';
import { arr, obj, str, num, type Obj } from './domain';
import type { Native } from './protocol';
import { lineageView } from './shell-lineage';
import { inlineOpen } from './shell-prefs';
import { watchVcsStatus } from './shell-vcs';
import { draftContext, previousWorktree } from './composer-controls-branch';
import { isLoopback } from './settings-b-fleet';
import { machineKind } from './connections';
import { commandShortcut } from './shell';
import { NO_SCRIPTS, cardScripts } from './r6-polish-scripts';
import { gitDetails, EMPTY_GIT } from './r4-git-card'; // lane r4-git: branch picker, Git actions and dialogs
import { environmentOptions } from './r4-git-env'; // lane r4-git: the environments that hold this project
import { prRowsView, emptyPrRows } from './r5-panels-pr'; // lane r5-panels: the pull request rows under the branch
import { EDITOR_DEFINITIONS } from './editors';
import { openInView } from './remote-open'; // OpenInPicker's remote Open: deep links for environments on other machines

/** EDITORS in contracts/editor.ts order; `file-manager` is Finder on macOS (editorLabelForPlatform). */
export const EDITORS: [string, string][] = EDITOR_DEFINITIONS.map(editor => [editor.id, editor.label]);

type QuickAction = { label: string; kind: string; disabled: boolean; hint: string; action: string };
/** GitActionsControl.logic.ts resolveQuickAction (GitHub terminology: PR). */
export function quickAction(status: Obj | null): QuickAction {
  const hint = (label: string, text: string): QuickAction => ({ label, kind: 'show_hint', disabled: true, hint: text, action: '' });
  const run = (label: string, action: string): QuickAction => ({ label, kind: 'run_action', disabled: false, hint: '', action });
  if (!status) return hint('Commit', 'Git status is unavailable.');
  const pr = obj(status.pr), hasOpenPr = pr.state === 'open', ahead = num(status.aheadCount) > 0, behind = num(status.behindCount) > 0;
  const isDefault = status.isDefaultRef === true, hasRemote = status.hasPrimaryRemote !== false, upstream = status.hasUpstream === true;
  if (status.refName == null) return hint('Commit', 'Create and checkout a ref before pushing or opening a pull request.');
  if (status.hasWorkingTreeChanges === true) {
    if (!upstream && !hasRemote) return run('Commit', 'commit');
    if (hasOpenPr || isDefault) return run('Commit & push', 'commit_push');
    return run('Commit, push & PR', 'commit_push_pr');
  }
  if (!upstream) {
    if (!hasRemote) return { label: 'Publish repository', kind: 'open_publish', disabled: false, hint: '', action: '' };
    if (!ahead) return hasOpenPr ? hint('Commit', 'Branch is up to date. No action needed.') : hint('Push', 'No local commits to push.');
    if (hasOpenPr || isDefault) return run('Push', isDefault ? 'commit_push' : 'push');
    return run('Push & create PR', 'create_pr');
  }
  if (ahead && behind) return hint('Sync ref', 'Branch has diverged from upstream. Rebase/merge first.');
  if (behind) return { label: 'Pull', kind: 'run_pull', disabled: false, hint: '', action: '' };
  if (ahead) return hasOpenPr || isDefault ? run('Push', isDefault ? 'commit_push' : 'push') : run('Push & create PR', 'create_pr');
  if (hasOpenPr) return hint('Commit', 'Branch is up to date. No action needed.');
  if (num(status.aheadOfDefaultCount ?? status.aheadCount) > 0 && !isDefault) return run('Create PR', 'create_pr');
  return hint('Commit', 'Branch is up to date. No action needed.');
}

/** GitQuickActionIcon: commit, cloud up/down, the source control host's mark for a pull request, else info. */
export function quickActionIcon(action: QuickAction, status: Obj | null): string {
  const provider = str(obj(status?.sourceControlProvider).kind) || 'github';
  const host = provider === 'github' ? 'github' : 'pull-request';
  if (action.kind === 'open_publish') return 'cloud-upload';
  if (action.kind === 'run_pull') return 'cloud-download';
  if (action.kind === 'run_action') return action.action === 'commit' ? 'git-commit' : action.action === 'push' || action.action === 'commit_push' ? 'cloud-upload' : host;
  return action.label === 'Commit' ? 'git-commit' : action.label === 'Push' ? 'cloud-upload' : 'info';
}

export function preferredEditor(available: string[], last: string): string {
  if (last && available.includes(last)) return last;
  return EDITORS.find(([id]) => available.includes(id))?.[0] ?? '';
}

/** shouldShowEnvironmentIndicator and the static label (BranchToolbar.logic.ts, BranchToolbarEnvironmentSelector, upstream bb7997709d):
 * a remote (non-primary) machine always gets a row, even when it is the only one, because the user still needs to see where it runs. */
export function environmentIndicator(input: { isPrimary: boolean; available: number; environmentId: string; runtimeLabel: string; savedLabel: string; machine: string }) {
  const runtime = input.runtimeLabel.trim(), saved = input.savedLabel.trim();
  const generic = (label: string) => !label || ['local', 'local environment'].includes(label.toLowerCase());
  const label = input.isPrimary ? (!generic(runtime) ? runtime : !generic(saved) ? saved : 'This device') : runtime || saved || input.environmentId;
  return { envShow: input.available > 1 || !input.isPrimary, envLabel: label || 'Run on', envKind: input.machine || 'server' };
}

type LineageView = ReturnType<typeof lineageView>;
const noLineage: LineageView = { lineageTitle: 'Lineage', lineage: [], showLineage: false, previousCount: 0, previousFailed: 0, mergeRunId: '', mergeTargetId: '', mergeSourceId: '', mergeLabel: '', mergeHint: '' };
const empty = { ready: false, inline: false, error: '', folderName: '', folderLabel: '', cwd: '', editorId: '', editorLabel: '', editors: [] as { id: string; label: string; selected: boolean }[], editorShortcut: '', editorShow: false, editorHint: '', editorUnavailable: '', editorEmpty: false,
  isGit: false, branch: '', actionLabel: 'Commit', actionKind: 'show_hint', actionDisabled: true, actionHint: '', insertions: 0, deletions: 0,
  envModeSelect: false, envMode: 'local', envIcon: 'folder', previousLabel: '', actionIcon: 'git-commit', changesEnabled: false, diffScheme: 'red-green',
  envShow: false, envLabel: '', envKind: 'server', git: EMPTY_GIT, prRows: emptyPrRows(), ...NO_SCRIPTS, ...noLineage };
const lastEditors = new WeakMap<T3Client, string>();
export function rememberEditor(client: T3Client, editor: string): void { lastEditors.set(client, editor); }

/** The inline card's per-thread key (scopedThreadKey: a draft is keyed by its draft). */
export const detailsKey = (client: T3Client) => client.draftKey;

/**
 * Data sources cannot read the clock (Date.now() fails the whole answer): `now` is the window's wall time.
 * The `shellDetails` source. Where chat has room (`wide`), the card docks inline
 * and is open unless this thread closed it (ThreadDetailsCard, upstream
 * 429c625a85); otherwise it is the header's popover, fetched only while open.
 */
export async function shellDetails(client: T3Client, native: Native | null | undefined, open: boolean, threadId: string, wide = false, now = 0, rightGap = 0) {
  const inline = wide && inlineOpen(client, detailsKey(client));
  if (!(inline || (open && !wide)) || !native?.available || !client.ready || !client.projectId) {
    if (native?.available && client.ready) await watchVcsStatus(client, native, '', now);
    return empty;
  }
  const thread = client.shell.threads.find(entry => entry.id === threadId);
  const project = client.shell.projects.find(entry => entry.id === (thread ? thread.projectId : client.projectId));
  const draft = thread ? null : draftContext(client);
  const worktree = thread ? str(thread.worktreePath) : str(draft?.worktreePath);
  const cwd = worktree || str(project?.workspaceRoot);
  const raw: unknown[] = Array.isArray(client.config.availableEditors) ? client.config.availableEditors : [];
  const available = raw.filter((value): value is string => typeof value === 'string' && EDITORS.some(([id]) => id === value));
  // Remote mode ignores the server's PATH probe: what matters is what runs on this Mac (OpenInPicker effectiveEditors).
  const openIn = await openInView(client, native, available, str(project?.title));
  const editor = preferredEditor(openIn.editors, lastEditors.get(client) ?? '');
  const label = (id: string) => EDITORS.find(([candidate]) => candidate === id)?.[1] ?? id;
  // subscribeVcsStatus while the card is shown (shell-vcs.ts); a closed card ends the stream below.
  const { status, error } = await watchVcsStatus(client, native, cwd, now);
  const action = quickAction(status);
  // The Changes row reads the branch's totals when the server reports them (upstream d1034d62b2).
  const totals = obj(status?.branchChanges ?? status?.workingTree);
  // BranchToolbarEnvModeSelector: a draft picks Current checkout / New worktree here; a thread's workspace is locked.
  const envMode = worktree ? (thread ? 'worktree' : 'local') : draft?.envMode === 'worktree' ? 'worktree' : 'local';
  const creating = !thread && envMode === 'worktree' && !worktree;
  const previous = thread ? null : previousWorktree(client);
  const environment = obj(client.config.environment);
  const env = environmentIndicator({ isPrimary: isLoopback(client.origin), available: environmentOptions(client).length, environmentId: client.environmentId,
    runtimeLabel: str(environment.label), savedLabel: '', machine: machineKind(client.config) });
  return {
    ready: true, inline, error, folderName: creating ? 'New worktree' : cwd.replace(/\/+$/, '').split('/').pop() ?? '',
    // The panel names the workspace kind only when it is not the project folder.
    folderLabel: creating ? 'Create' : worktree ? 'Worktree' : '', cwd,
    envModeSelect: !thread, envMode, envIcon: envMode === 'worktree' && !worktree ? 'folder-git-2' : worktree ? 'folder-git' : 'folder',
    previousLabel: previous?.branch ?? '', actionIcon: quickActionIcon(action, status),
    // ChatView passes onOpenChanges only for a server thread in a Git repository.
    changesEnabled: !!thread && status?.isRepo !== false, // GitActionsControl's isRepo is true until status says otherwise
    editorId: editor, editorLabel: `Open in ${editor ? label(editor) : 'editor'}`, editors: openIn.unavailable ? [] : openIn.editors.map(id => ({ id, label: label(id), selected: id === editor })),
    editorShow: openIn.show, editorHint: openIn.hint, editorUnavailable: openIn.unavailable, editorEmpty: openIn.empty,
    editorShortcut: commandShortcut(client.config, 'editor.openFavorite'),
    ...cardScripts(obj(client.config.settings), project), // r6-polish: the project script row (r6-polish-scripts.ts)
    isGit: status?.isRepo === true, branch: str(status?.refName), actionLabel: action.label, actionKind: action.kind === 'run_action' ? action.action : action.kind,
    actionDisabled: action.disabled, actionHint: action.hint, insertions: num(totals.insertions), deletions: num(totals.deletions), ...env,
    diffScheme: client.local.clientSettings?.diffColorScheme === 'blue-orange' ? 'blue-orange' : 'red-green',
    git: await gitDetails(client, native, { status, error, cwd, root: str(project?.workspaceRoot), now, editor, envRow: env.envShow }),
    prRows: await prRowsView(client, native, { status, threadId, now, projectId: str(project?.id), rightGap, inline }), // lane r6-pr: the row's place in the window
    ...(thread ? lineageView(client.shell.threads, client.threadId === threadId ? client.projection : null, threadId, arr(client.config.providers), now) : noLineage),
  };
}
