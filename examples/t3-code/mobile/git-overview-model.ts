// Mobile source365aa87982 (MIT, LICENSE-T3). Pure menu/confirmation function bodies
// from packages/client-runtime/src/state/gitActions.ts; local structural types only.
// @ref llp/1107.011-responsive-workspace.decision.md#navigation-and-data-ownership
import { arr, num, obj, str, type Obj } from './shared/domain';
export interface VcsStatusResult { isRepo: boolean; refName: string | null; hasWorkingTreeChanges: boolean;
  hasUpstream: boolean; aheadCount: number; behindCount: number; pr: {state: string; number: number} | null }
type GitStackedAction = 'commit' | 'push' | 'create_pr' | 'commit_push' | 'commit_push_pr';
export type GitActionIconName = "commit" | "push" | "pr";

export type GitDialogAction = "commit" | "push" | "create_pr";

export interface GitActionMenuItem {
  id: "commit" | "push" | "pr";
  label: string;
  disabled: boolean;
  icon: GitActionIconName;
  kind: "open_dialog" | "open_pr";
  dialogAction?: GitDialogAction;
}

export interface GitQuickAction {
  label: string;
  disabled: boolean;
  kind: "run_action" | "run_pull" | "open_pr" | "show_hint";
  action?: GitStackedAction;
  hint?: string;
}

export interface DefaultBranchActionDialogCopy {
  title: string;
  description: string;
  continueLabel: string;
}

export type DefaultBranchConfirmableAction =
  | "push"
  | "create_pr"
  | "commit_push"
  | "commit_push_pr";


export function buildMenuItems(
  gitStatus: VcsStatusResult | null,
  isBusy: boolean,
  hasOriginRemote = true,
): GitActionMenuItem[] {
  if (!gitStatus) return [];

  const hasBranch = gitStatus.refName !== null;
  const hasChanges = gitStatus.hasWorkingTreeChanges;
  const hasOpenPr = gitStatus.pr?.state === "open";
  const isBehind = gitStatus.behindCount > 0;
  const canPushWithoutUpstream = hasOriginRemote && !gitStatus.hasUpstream;
  const canCommit = !isBusy && hasChanges;
  const canPush =
    !isBusy &&
    hasBranch &&
    !hasChanges &&
    !isBehind &&
    gitStatus.aheadCount > 0 &&
    (gitStatus.hasUpstream || canPushWithoutUpstream);
  const canCreatePr =
    !isBusy &&
    hasBranch &&
    !hasChanges &&
    !hasOpenPr &&
    gitStatus.aheadCount > 0 &&
    !isBehind &&
    (gitStatus.hasUpstream || canPushWithoutUpstream);
  const canOpenPr = !isBusy && hasOpenPr;

  return [
    {
      id: "commit",
      label: "Commit",
      disabled: !canCommit,
      icon: "commit",
      kind: "open_dialog",
      dialogAction: "commit",
    },
    {
      id: "push",
      label: "Push",
      disabled: !canPush,
      icon: "push",
      kind: "open_dialog",
      dialogAction: "push",
    },
    hasOpenPr
      ? {
          id: "pr",
          label: "View PR",
          disabled: !canOpenPr,
          icon: "pr",
          kind: "open_pr",
        }
      : {
          id: "pr",
          label: "Create PR",
          disabled: !canCreatePr,
          icon: "pr",
          kind: "open_dialog",
          dialogAction: "create_pr",
        },
  ];
}


export function getGitActionDisabledReason(input: {
  item: GitActionMenuItem;
  gitStatus: VcsStatusResult | null;
  isBusy: boolean;
  hasOriginRemote: boolean;
}): string | null {
  const { item, gitStatus, isBusy, hasOriginRemote } = input;
  if (!item.disabled) return null;
  if (isBusy) return "Git action in progress.";
  if (!gitStatus) return "Git status is unavailable.";

  const hasBranch = gitStatus.refName !== null;
  const hasChanges = gitStatus.hasWorkingTreeChanges;
  const hasOpenPr = gitStatus.pr?.state === "open";
  const isAhead = gitStatus.aheadCount > 0;
  const isBehind = gitStatus.behindCount > 0;

  if (item.id === "commit") {
    if (!hasChanges) {
      return "Worktree is clean. Make changes before committing.";
    }
    return "Commit is currently unavailable.";
  }

  if (item.id === "push") {
    if (!hasBranch) {
      return "Detached HEAD: checkout a branch before pushing.";
    }
    if (hasChanges) {
      return "Commit or stash local changes before pushing.";
    }
    if (isBehind) {
      return "Branch is behind upstream. Pull/rebase before pushing.";
    }
    if (!gitStatus.hasUpstream && !hasOriginRemote) {
      return 'Add an "origin" remote before pushing.';
    }
    if (!isAhead) {
      return "No local commits to push.";
    }
    return "Push is currently unavailable.";
  }

  if (hasOpenPr) {
    return "View PR is currently unavailable.";
  }
  if (!hasBranch) {
    return "Detached HEAD: checkout a branch before creating a PR.";
  }
  if (hasChanges) {
    return "Commit local changes before creating a PR.";
  }
  if (!gitStatus.hasUpstream && !hasOriginRemote) {
    return 'Add an "origin" remote before creating a PR.';
  }
  if (!isAhead) {
    return "No local commits to include in a PR.";
  }
  if (isBehind) {
    return "Branch is behind upstream. Pull/rebase before creating a PR.";
  }
  return "Create PR is currently unavailable.";
}

export function requiresDefaultBranchConfirmation(
  action: GitStackedAction,
  isDefaultBranch: boolean,
): boolean {
  if (!isDefaultBranch) return false;
  return (
    action === "push" ||
    action === "create_pr" ||
    action === "commit_push" ||
    action === "commit_push_pr"
  );
}

export function resolveDefaultBranchActionDialogCopy(input: {
  action: DefaultBranchConfirmableAction;
  branchName: string;
  includesCommit: boolean;
}): DefaultBranchActionDialogCopy {
  const branchLabel = input.branchName;
  const suffix = ` on "${branchLabel}". You can continue on this branch or create a feature branch and run the same action there.`;

  if (input.action === "push" || input.action === "commit_push") {
    if (input.includesCommit) {
      return {
        title: "Commit & push to default branch?",
        description: `This action will commit and push changes${suffix}`,
        continueLabel: `Commit & push to ${branchLabel}`,
      };
    }
    return {
      title: "Push to default branch?",
      description: `This action will push local commits${suffix}`,
      continueLabel: `Push to ${branchLabel}`,
    };
  }

  if (input.includesCommit) {
    return {
      title: "Commit, push & create PR from default branch?",
      description: `This action will commit, push, and create a PR${suffix}`,
      continueLabel: "Commit, push & create PR",
    };
  }
  return {
    title: "Push & create PR from default branch?",
    description: `This action will push local commits and create a PR${suffix}`,
    continueLabel: "Push & create PR",
  };
}

export function mobileGitStatus(input: Obj | null): VcsStatusResult | null {
  return input ? { isRepo: input.isRepo === true, refName: typeof input.refName === 'string' ? input.refName : null,
    hasWorkingTreeChanges: input.hasWorkingTreeChanges === true, hasUpstream: input.hasUpstream === true,
    aheadCount: num(input.aheadCount), behindCount: num(input.behindCount),
    pr: input.pr ? { state: str(obj(input.pr).state), number: num(obj(input.pr).number) } : null } : null;
}
export function mobileGitSummary(status: Obj | null): string {
  if (!status) return 'Loading branch status…';
  if (!status.isRepo) return 'Not a git repository';
  const count = arr(obj(status.workingTree).files).length;
  const parts = [status.hasWorkingTreeChanges ? `${count} file${count === 1 ? '' : 's'} changed` : 'Clean'];
  if (num(status.aheadCount) > 0) parts.push(`${status.aheadCount} ahead`);
  if (num(status.behindCount) > 0) parts.push(`${status.behindCount} behind`);
  if (obj(status.pr).state === 'open') parts.push(`PR #${obj(status.pr).number} open`);
  return parts.join(' · ');
}
// packages/shared/src/git.ts sanitizeBranchFragment/sanitizeFeatureBranchName.
export function mobileFeatureBranch(raw: string): string {
  const normalized = raw.trim().toLowerCase().replace(/['"`]/g, '').replace(/^[./\s_-]+|[./\s_-]+$/g, '');
  const value = normalized.replace(/[^a-z0-9/_-]+/g, '-').replace(/\/+/g, '/').replace(/-+/g, '-')
    .replace(/^[./_-]+|[./_-]+$/g, '').slice(0, 64).replace(/[./_-]+$/g, '') || 'update';
  return value.startsWith('feature/') ? value : `feature/${value}`;
}
export function mobileAutoBranch(names: string[]): string {
  const used = new Set(names.map(name => name.toLowerCase())); let name = 'feature/update', n = 2;
  while (used.has(name)) name = `feature/update-${n++}`;
  return name;
}
