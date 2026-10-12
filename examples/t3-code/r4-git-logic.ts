// Git action rules for the workspace card (lane r4-git), adapted from T3 Code (MIT;
// see LICENSE-T3): apps/web/src/components/GitActionsControl.logic.ts (quick action,
// menu items, default-branch copy, progress), GitActionsControl.tsx
// (getMenuActionDisabledReason, the publish provider readiness) and
// packages/shared/src/sourceControl.ts (change-request terminology and marks).
// Pure functions over a VcsStatusResult; the runner lives in r4-git-actions.ts.
import { arr, obj, str, num, type Obj } from './domain';

export type Terminology = { shortLabel: string; singular: string };
const PRESENTATION: Record<string, { short: string; long: string; icon: string; name: string }> = {
  github: { short: 'PR', long: 'pull request', icon: 'github', name: 'GitHub' },
  gitlab: { short: 'MR', long: 'merge request', icon: 'gitlab', name: 'GitLab' },
  forgejo: { short: 'PR', long: 'pull request', icon: 'forgejo', name: 'Forgejo' },
  'azure-devops': { short: 'PR', long: 'pull request', icon: 'azure-devops', name: 'Azure DevOps' },
  bitbucket: { short: 'PR', long: 'pull request', icon: 'bitbucket', name: 'Bitbucket' },
  unknown: { short: 'change request', long: 'change request', icon: 'pull-request', name: 'source control' },
};
const providerOf = (status: Obj | null): Obj | null => {
  const provider = status?.sourceControlProvider;
  return provider && typeof provider === 'object' && !Array.isArray(provider) ? provider as Obj : null;
};
const presentationOf = (provider: Obj | null) => PRESENTATION[str(provider?.kind) || 'github'] ?? PRESENTATION.github!;
/** getChangeRequestTerminology: a status without a provider reads as GitHub's "PR". */
export function terminology(status: Obj | null): Terminology {
  const provider = providerOf(status);
  if (!provider) return { shortLabel: 'PR', singular: 'pull request' };
  const presentation = presentationOf(provider);
  return { shortLabel: presentation.short, singular: presentation.long };
}
/** getSourceControlPresentation(...).Icon as a mark name (an unknown host gets the pull-request glyph). */
export const providerMark = (status: Obj | null): string => presentationOf(providerOf(status)).icon;

export type QuickAction = { label: string; disabled: boolean; kind: 'run_action' | 'run_pull' | 'open_publish' | 'show_hint'; action?: string; hint?: string };
const hint = (label: string, text: string): QuickAction => ({ label, disabled: true, kind: 'show_hint', hint: text });
const run = (label: string, action: string): QuickAction => ({ label, disabled: false, kind: 'run_action', action });

/** resolveQuickAction (web panel variant). */
export function quickAction(status: Obj | null, busy: boolean, isDefaultRef = false, hasPrimaryRemote = true): QuickAction {
  if (busy) return hint('Commit', 'Git action in progress.');
  if (!status) return hint('Commit', 'Git status is unavailable.');
  const term = terminology(status);
  const hasOpenPr = obj(status.pr).state === 'open', ahead = num(status.aheadCount) > 0, behind = num(status.behindCount) > 0;
  const upstream = status.hasUpstream === true;
  const defaultDelta = num(status.aheadOfDefaultCount ?? status.aheadCount) > 0;
  if (status.refName == null) return hint('Commit', `Create and checkout a ref before pushing or opening a ${term.singular}.`);
  if (status.hasWorkingTreeChanges === true) {
    if (!upstream && !hasPrimaryRemote) return run('Commit', 'commit');
    if (hasOpenPr || isDefaultRef) return run('Commit & push', 'commit_push');
    return run(`Commit, push & ${term.shortLabel}`, 'commit_push_pr');
  }
  if (!upstream) {
    if (!hasPrimaryRemote) return { label: 'Publish repository', disabled: false, kind: 'open_publish' };
    if (!ahead) return hasOpenPr ? hint('Commit', 'Branch is up to date. No action needed.') : hint('Push', 'No local commits to push.');
    if (hasOpenPr || isDefaultRef) return run('Push', isDefaultRef ? 'commit_push' : 'push');
    return run(`Push & create ${term.shortLabel}`, 'create_pr');
  }
  if (ahead && behind) return hint('Sync ref', 'Branch has diverged from upstream. Rebase/merge first.');
  if (behind) return { label: 'Pull', disabled: false, kind: 'run_pull' };
  if (ahead) return hasOpenPr || isDefaultRef ? run('Push', isDefaultRef ? 'commit_push' : 'push') : run(`Push & create ${term.shortLabel}`, 'create_pr');
  if (hasOpenPr) return hint('Commit', 'Branch is up to date. No action needed.');
  if (defaultDelta && !isDefaultRef) return run(`Create ${term.shortLabel}`, 'create_pr');
  return hint('Commit', 'Branch is up to date. No action needed.');
}

/** GitQuickActionIcon as a mark name. */
export function quickActionIcon(action: QuickAction, status: Obj | null): string {
  if (action.kind === 'open_publish') return 'cloud-upload';
  if (action.kind === 'run_pull') return 'cloud-download';
  if (action.kind === 'run_action') return action.action === 'commit' ? 'git-commit' : action.action === 'push' || action.action === 'commit_push' ? 'cloud-upload' : providerMark(status);
  return action.label === 'Commit' ? 'git-commit' : action.label === 'Push' ? 'cloud-upload' : 'info';
}

export type MenuItem = { id: 'commit' | 'push' | 'pr'; label: string; disabled: boolean; icon: string; dialogAction: string };
/** buildMenuItems: Commit, then Push and Create PR where a primary remote exists (no PR entry while one is open). */
export function menuItems(status: Obj | null, busy: boolean, hasPrimaryRemote: boolean): MenuItem[] {
  if (!status) return [];
  const term = terminology(status);
  const hasBranch = status.refName != null, changes = status.hasWorkingTreeChanges === true, hasOpenPr = obj(status.pr).state === 'open';
  const behind = num(status.behindCount) > 0, upstream = status.hasUpstream === true;
  const defaultDelta = num(status.aheadOfDefaultCount ?? status.aheadCount) > 0;
  const pushable = upstream || (hasPrimaryRemote && !upstream);
  const commit: MenuItem = { id: 'commit', label: 'Commit', disabled: busy || !changes, icon: 'git-commit', dialogAction: 'commit' };
  if (!hasPrimaryRemote) return [commit];
  const push: MenuItem = { id: 'push', label: 'Push', disabled: !(!busy && hasBranch && !behind && num(status.aheadCount) > 0 && pushable), icon: 'cloud-upload', dialogAction: 'push' };
  if (hasOpenPr) return [commit, push];
  return [commit, push, { id: 'pr', label: `Create ${term.shortLabel}`, icon: providerMark(status), dialogAction: 'create_pr',
    disabled: !(!busy && hasBranch && !changes && !hasOpenPr && defaultDelta && !behind && pushable) }];
}

/** getMenuActionDisabledReason: the hover note on a disabled menu item. */
export function menuReason(item: MenuItem, status: Obj | null, busy: boolean, hasPrimaryRemote: boolean): string {
  if (!item.disabled) return '';
  if (busy) return 'Git action in progress.';
  if (!status) return 'Git status is unavailable.';
  const hasBranch = status.refName != null, changes = status.hasWorkingTreeChanges === true;
  const ahead = num(status.aheadCount) > 0, behind = num(status.behindCount) > 0, upstream = status.hasUpstream === true;
  const term = terminology(status);
  if (item.id === 'commit') return changes ? 'Commit is currently unavailable.' : 'Worktree is clean. Make changes before committing.';
  if (item.id === 'push') {
    if (!hasBranch) return 'Detached HEAD: check out a branch before pushing.';
    if (changes) return 'Commit or stash local changes before pushing.';
    if (behind) return 'Branch is behind upstream. Pull/rebase before pushing.';
    if (!upstream && !hasPrimaryRemote) return 'Add an "origin" remote before pushing.';
    if (!ahead) return 'No local commits to push.';
    return 'Push is currently unavailable.';
  }
  if (!hasBranch) return `Detached HEAD: check out a branch before creating a ${term.singular}.`;
  if (changes) return `Commit local changes before creating a ${term.singular}.`;
  if (!upstream && !hasPrimaryRemote) return `Add an "origin" remote before creating a ${term.singular}.`;
  if (!ahead) return `No local commits to include in a ${term.singular}.`;
  if (behind) return `Branch is behind upstream. Pull/rebase before creating a ${term.singular}.`;
  return `Create ${term.singular} is currently unavailable.`;
}

/** The menu's trailing notes (detached HEAD, behind upstream, a status error). */
export function menuNotes(status: Obj | null, error: string): { id: string; text: string; tone: string }[] {
  const notes: { id: string; text: string; tone: string }[] = [];
  if (status && status.refName === null) notes.push({ id: 'detached', text: 'Detached HEAD: create and check out a branch to enable push and pull request actions.', tone: 'warning' });
  if (status && status.refName != null && status.hasWorkingTreeChanges !== true && num(status.behindCount) > 0 && num(status.aheadCount) === 0)
    notes.push({ id: 'behind', text: 'Behind upstream. Pull/rebase first.', tone: 'warning' });
  if (error) notes.push({ id: 'error', text: error, tone: 'error' });
  return notes;
}

export const requiresDefaultBranchConfirmation = (action: string, isDefaultRef: boolean): boolean =>
  isDefaultRef && ['push', 'create_pr', 'commit_push', 'commit_push_pr'].includes(action);

/** resolveDefaultBranchActionDialogCopy. */
export function defaultBranchCopy(action: string, branchName: string, includesCommit: boolean, term: Terminology): { title: string; description: string; continueLabel: string } {
  const suffix = ` on "${branchName}". You can continue on this ref or create a feature ref and run the same action there.`;
  if (action === 'push' || action === 'commit_push') {
    return includesCommit
      ? { title: 'Commit & push to default ref?', description: `This action will commit and push changes${suffix}`, continueLabel: `Commit & push to ${branchName}` }
      : { title: 'Push to default ref?', description: `This action will push local commits${suffix}`, continueLabel: `Push to ${branchName}` };
  }
  return includesCommit
    ? { title: `Commit, push & create ${term.shortLabel} from default ref?`, description: `This action will commit, push, and create a ${term.singular}${suffix}`, continueLabel: `Commit, push & create ${term.shortLabel}` }
    : { title: `Push & create ${term.shortLabel} from default ref?`, description: `This action will push local commits and create a ${term.singular}${suffix}`, continueLabel: `Push & create ${term.shortLabel}` };
}

/** formatGitActionElapsed: "12s", "1m 5s". */
export function formatElapsed(startedAt: number, now: number): string {
  if (startedAt <= 0) return '';
  const seconds = Math.max(0, Math.floor((now - startedAt) / 1000));
  return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}

/** resolveGitActionProgressPresentation over the runner's state. */
export function progressPresentation(state: { running: boolean; operation: string; label: string; output: string; phaseAt: number; hookAt: number }): { status: string; output: string; startedAt: number } | null {
  if (!state.running || (state.operation !== 'run_change_request' && state.operation !== 'pull')) return null;
  const label = state.label.trim(), output = state.output.trim(), pull = state.operation === 'pull';
  return { status: label && label !== 'Running source control action' ? label : pull ? 'Pulling latest changes...' : 'Starting source control action...',
    output: !pull && output ? output : '', startedAt: pull ? state.phaseAt : (state.hookAt || state.phaseAt) };
}

/** The working tree's files for the commit dialog (StartTruncatedPath shows the whole path). */
export function workingFiles(status: Obj | null): { path: string; insertions: number; deletions: number }[] {
  return arr(obj(status?.workingTree).files).map(file => ({ path: str(file.path), insertions: num(file.insertions), deletions: num(file.deletions) })).filter(file => file.path);
}

// ── Publish repository (PublishRepositoryDialog) ─────────────────────────────
export const PUBLISH_PROVIDERS = [
  { value: 'forgejo', label: 'Forgejo / Gitea', host: 'your server', placeholder: 'owner/repo' },
  { value: 'github', label: 'GitHub', host: 'github.com', placeholder: 'owner/repo' },
  { value: 'gitlab', label: 'GitLab', host: 'gitlab.com', placeholder: 'group/project' },
  { value: 'bitbucket', label: 'Bitbucket', host: 'bitbucket.org', placeholder: 'workspace/repository' },
  { value: 'azure-devops', label: 'Azure DevOps', host: 'dev.azure.com', placeholder: 'project/repository' },
] as const;
const optionNone = (value: unknown): string => { const option = obj(value); return option._tag === 'Some' ? str(option.value) : typeof value === 'string' ? value : ''; };
/** getPublishProviderReadiness over the discovery's providers. */
export function publishReadiness(provider: string, discovered: Obj[]): { ready: boolean; hint: string; account: string; host: string } {
  const entry = discovered.find(candidate => str(candidate.kind) === provider);
  if (!entry) return { ready: false, hint: 'Provider status unavailable. Open Settings -> Source Control and rescan.', account: '', host: '' };
  const auth = obj(entry.auth), account = optionNone(auth.account), host = optionNone(auth.host);
  if (str(entry.status) !== 'available') return { ready: false, hint: str(entry.installHint), account, host };
  if (str(auth.status) === 'unauthenticated') return { ready: false, hint: optionNone(auth.detail) || `${str(entry.label, provider)} is not authenticated. Open Settings -> Source Control for setup guidance.`, account, host };
  return { ready: true, hint: '', account, host };
}
/** The provider cards, ready ones first then by label. */
export function sortedPublishProviders(discovered: Obj[]) {
  return PUBLISH_PROVIDERS.map(option => ({ ...option, ...publishReadiness(option.value, discovered) }))
    .sort((left, right) => left.ready !== right.ready ? (left.ready ? -1 : 1) : left.label.localeCompare(right.label));
}
/** canSubmitPublishRepository: "owner/name" with both parts present. */
export function publishPathValid(repository: string): boolean {
  const parts = repository.trim().split('/');
  return (parts[0]?.trim() ?? '').length > 0 && parts.slice(1).join('/').trim().length > 0;
}
