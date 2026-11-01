// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r4-git-route.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Where the card's Git ops land (lane r4-git): `shell:git-*` writes from
// shell-commands.ts and `chatlocal:git-*` view state from timeline-presentation.ts.
// Any of them closes the card's open popup first, as choosing from a menu does.
import type { T3Client } from './client';
import type { Files, Native } from './protocol';
import { gitCommand, gitLocal } from './r4-git-actions';
import { branchCommand, branchLocal, branchState } from './r4-git-branch';
import { checkoutConfirm, checkoutLocal } from './r9-connect-checkout'; // lane r9-connect: the pull request checkout dialog

/** `shell:git-<op>`: branch checkout or creation, else a Git action. */
export async function gitShellCommand(client: T3Client, native: Native, storage: Files, op: string, id: string, value: string): Promise<string> {
  branchState(client).open = '';
  if (op === 'branch' || op === 'branch-create') return branchCommand(client, native, storage, op, value, id === 'enter');
  if (op === 'pr-checkout') return checkoutConfirm(client, native, id, value);
  // The card's older quick-action op.
  return gitCommand(client, native, op === 'action' ? 'quick' : op, id, value);
}

/** `chatlocal:git-<op>`: the picker and menu state, the dialogs' fields. */
export async function gitChatLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  if (op === 'open' || op === 'query' || op === 'origin') return branchLocal(client, op, id, value);
  branchState(client).open = '';
  if (op.startsWith('pr-')) return checkoutLocal(client, native, op.slice(3), value);
  return gitLocal(client, native, op === 'open-file' ? 'open-file' : op, id, value);
}

/** The failure toasts' titles (GitActionsControl, BranchToolbarBranchSelector). */
export const GIT_FAILURE_TITLES: Record<string, string> = {
  'shell:git-quick': 'Action failed', 'shell:git-menu': 'Action failed', 'shell:git-commit': 'Action failed', 'shell:git-commit-branch': 'Action failed',
  'shell:git-confirm': 'Action failed', 'shell:git-action': 'Action failed', 'shell:git-init': 'Git initialization failed', 'shell:git-publish': 'Publish failed',
  'shell:git-branch': 'Failed to switch ref.', 'shell:git-branch-create': 'Failed to create and switch ref.',
};
