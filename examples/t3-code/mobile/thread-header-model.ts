// Pinned365aa87982 packages/client-runtime/src/state/gitActions.ts and mobile terminalMenu.ts.
// Quick-action body preserved; types use the existing mobile structural model.
// @ref llp/1109.005-composer-and-transcript.decision.md
import type { GitQuickAction, VcsStatusResult } from './git-overview-model';
import { arr, num, obj, str, type Obj } from './shared/domain';

export function resolveQuickAction(
  gitStatus: VcsStatusResult | null,
  isBusy: boolean,
  isDefaultBranch = false,
  hasOriginRemote = true,
): GitQuickAction {
  if (isBusy) {
    return { label: "Commit", disabled: true, kind: "show_hint", hint: "Git action in progress." };
  }

  if (!gitStatus) {
    return {
      label: "Commit",
      disabled: true,
      kind: "show_hint",
      hint: "Git status is unavailable.",
    };
  }

  const hasBranch = gitStatus.refName !== null;
  const hasChanges = gitStatus.hasWorkingTreeChanges;
  const hasOpenPr = gitStatus.pr?.state === "open";
  const isAhead = gitStatus.aheadCount > 0;
  const isBehind = gitStatus.behindCount > 0;
  const isDiverged = isAhead && isBehind;

  if (!hasBranch) {
    return {
      label: "Commit",
      disabled: true,
      kind: "show_hint",
      hint: "Create and checkout a branch before pushing or opening a PR.",
    };
  }

  if (hasChanges) {
    if (!gitStatus.hasUpstream && !hasOriginRemote) {
      return { label: "Commit", disabled: false, kind: "run_action", action: "commit" };
    }
    if (hasOpenPr || isDefaultBranch) {
      return { label: "Commit & push", disabled: false, kind: "run_action", action: "commit_push" };
    }
    return {
      label: "Commit, push & PR",
      disabled: false,
      kind: "run_action",
      action: "commit_push_pr",
    };
  }

  if (!gitStatus.hasUpstream) {
    if (!hasOriginRemote) {
      if (hasOpenPr && !isAhead) {
        return { label: "View PR", disabled: false, kind: "open_pr" };
      }
      return {
        label: "Push",
        disabled: true,
        kind: "show_hint",
        hint: 'Add an "origin" remote before pushing or creating a PR.',
      };
    }
    if (!isAhead) {
      if (hasOpenPr) {
        return { label: "View PR", disabled: false, kind: "open_pr" };
      }
      return {
        label: "Push",
        disabled: true,
        kind: "show_hint",
        hint: "No local commits to push.",
      };
    }
    if (hasOpenPr || isDefaultBranch) {
      return {
        label: "Push",
        disabled: false,
        kind: "run_action",
        action: isDefaultBranch ? "commit_push" : "push",
      };
    }
    return {
      label: "Push & create PR",
      disabled: false,
      kind: "run_action",
      action: "create_pr",
    };
  }

  if (isDiverged) {
    return {
      label: "Sync branch",
      disabled: true,
      kind: "show_hint",
      hint: "Branch has diverged from upstream. Rebase/merge first.",
    };
  }

  if (isBehind) {
    return {
      label: "Pull",
      disabled: false,
      kind: "run_pull",
    };
  }

  if (isAhead) {
    if (hasOpenPr || isDefaultBranch) {
      return {
        label: "Push",
        disabled: false,
        kind: "run_action",
        action: isDefaultBranch ? "commit_push" : "push",
      };
    }
    return {
      label: "Push & create PR",
      disabled: false,
      kind: "run_action",
      action: "create_pr",
    };
  }

  if (hasOpenPr && gitStatus.hasUpstream) {
    return { label: "View PR", disabled: false, kind: "open_pr" };
  }

  return {
    label: "Commit",
    disabled: true,
    kind: "show_hint",
    hint: "Branch is up to date. No action needed.",
  };
}


export function compactThreadGitStatus(status: Obj | null): string {
  if (!status) return 'Checking status';
  if (!status.isRepo) return 'Not a repo';
  const parts: string[] = [];
  if (status.hasWorkingTreeChanges) parts.push(`${arr(obj(status.workingTree).files).length} changed`);
  else if (num(status.aheadCount) === 0 && num(status.behindCount) === 0) parts.push('Clean');
  if (num(status.aheadCount) > 0) parts.push(`${status.aheadCount} ahead`);
  if (num(status.behindCount) > 0) parts.push(`${status.behindCount} behind`);
  if (obj(status.pr).state === 'open') parts.push(`PR #${obj(status.pr).number}`);
  return parts.join(' · ');
}
export function compactThreadBranch(branch: string): string {
  return branch.length <= 24 ? branch : `${branch.slice(0, 12)}…${branch.slice(-11)}`;
}
export function threadTerminalStatus(status: string, running: boolean): string {
  return status === 'running' ? running ? 'Task running' : 'Ready' : status === 'starting' ? 'Starting'
    : status === 'exited' ? 'Exited' : status === 'error' ? 'Error' : 'Not started';
}
export function threadScriptSymbol(icon: string): string {
  const symbols: Record<string, string> = { test: 'flask', lint: 'checklist', configure: 'wrench.and.screwdriver', build: 'hammer', debug: 'ladybug' }; return symbols[icon] ?? 'play';
}
const terminalNumber = (id: string) => /^term(?:inal)?-(\d+)(?:-[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12})?$/i.exec(id)?.[1];
export function threadTerminalLabel(id: string, summary: Obj | null | undefined): string {
  return str(summary?.label).trim() || (terminalNumber(id) ? `Terminal ${terminalNumber(id)}` : id);
}
export function threadNextTerminalId(ids: string[], suffix?: string): string {
  const used = new Set(ids.map(terminalNumber)); let index = 1;
  while (used.has(String(index))) index++;
  return `term-${index}${suffix === undefined ? '' : `-${suffix}`}`;
}
