// ShellDetails.git (lane r4-git): the card's Version Control data, joined from
// r4-git-actions.ts (quick action, menu, progress, dialogs) and r4-git-branch.ts
// (the branch picker).
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { gitCardPrepare, gitCardView } from './r4-git-actions';
import { cardBranchView, type CardBranch } from './r4-git-branch';
import { environmentView } from './r4-git-env';

export async function gitDetails(client: T3Client, native: Native, input: { status: Obj | null; error: string; cwd: string; root: string; now: number;
  editor: string; envRow: boolean }) {
  const view = gitCardView(client, input.status, input.error, input.cwd, input.now, input.editor);
  // A failing ref read must not cost the card its other rows.
  const branch = await cardBranchView(client, native, input.cwd, input.root, input.status?.isRepo === true, input.now).catch((): CardBranch => ({ ...HIDDEN_BRANCH }));
  await gitCardPrepare(client, native).catch(() => undefined);
  return { ...view, ...environmentView(client), branch };
}

const HIDDEN_BRANCH: CardBranch = { show: false, open: false, label: '', value: '', disabled: true, query: '', refs: [], creatable: '', status: '', empty: '', enterOp: '', enterValue: '',
  originShown: false, originOn: false, count: 0, picked: false, checkout: [] };
export const EMPTY_GIT: Awaited<ReturnType<typeof gitDetails>> = {
  repo: true, initLabel: 'Initialize Git', initPending: false, quickLabel: 'Commit', quickIcon: 'git-commit', quickDisabled: true, quickHint: '',
  progress: false, progressStatus: '', progressOutput: '', progressElapsed: '', success: false, successTitle: '', successDescription: '',
  menuDisabled: false, menuOpen: false, menu: [], menuPublish: false, notes: [], dialog: '', sourceMark: 'github', commitBranch: '', commitDefault: false, editing: false,
  files: [], fileCount: 0, selectedCount: 0, allSelected: true, noneSelected: true, someSelected: false, selectedInsertions: 0, selectedDeletions: 0,
  confirmTitle: '', confirmDescription: '', confirmContinue: '', publishStep: 0, publishProviders: [], publishRows: [], publishProvider: 'github', publishProviderLabel: 'GitHub',
  publishHost: 'github.com', publishPlaceholder: 'owner/repo', publishRepository: '', publishVisibility: 'private', publishRemote: 'origin', publishProtocol: 'ssh',
  publishAdvanced: false, publishPending: false, publishError: '', publishAnyReady: false, publishCanNext: false, publishCanSubmit: false, publishPushed: false,
  publishName: '', publishUrl: '', publishSummary: '', envPick: false, envOptions: [], envCount: 1,
  branch: HIDDEN_BRANCH,
};
