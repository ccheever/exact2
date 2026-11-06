// ShellDetails.git: the card's Version Control data, joined from
// git-actions.ts (quick action, menu, progress, dialogs) and git-branch.ts
// (the branch picker).
import type { T3Client } from '../client/client';
import type { Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { gitCardPrepare, gitCardView } from './git-actions';
import { cardBranchView, type CardBranch } from './git-branch';
import { environmentView } from './git-env';

/** `unstarted`: a draft or a thread with no run yet, whose card stays quiet until there is work (GitActionsControl quietEmptyDraft). */
export async function gitDetails(client: T3Client, native: Native, input: { status: Obj | null; error: string; cwd: string; root: string; now: number;
  editor: string; envRow: boolean; unstarted?: boolean }) {
  const view = gitCardView(client, input.status, input.error, input.cwd, input.now, input.editor, input.unstarted === true);
  // A failing ref read must not cost the card its other rows.
  const branch = await cardBranchView(client, native, input.cwd, input.root, input.status?.isRepo === true).catch((): CardBranch => ({ ...HIDDEN_BRANCH }));
  await gitCardPrepare(client, native).catch(() => undefined);
  // The run row's glyph tooltips are the details card's to name (shell-details.ts).
  return { ...view, ...environmentView(client), branch, runTip: '', workspaceTip: '' };
}

const HIDDEN_BRANCH: CardBranch = { show: false, open: false, label: '', labelHead: '', labelTail: '', value: '', disabled: true, query: '', refs: [], creatable: '', status: '', empty: '', enterOp: '', enterValue: '',
  originShown: false, originOn: false, count: 0, checkout: [] };
export const EMPTY_GIT: Awaited<ReturnType<typeof gitDetails>> = {
  writable: false, repo: true, quiet: false, changesAdd: '+0', changesDel: '-0', runTip: '', workspaceTip: '', initLabel: 'Initialize Git', initPending: false, quickLabel: 'Commit', quickIcon: 'git-commit', quickDisabled: true, quickHint: '',
  progress: false, progressStatus: '', progressOutput: '', progressElapsed: '', success: false, successTitle: '', successDescription: '',
  menuDisabled: false, menuOpen: false, menu: [], menuPublish: false, notes: [], dialog: '', dialogFrom: '', sourceMark: 'github', commitBranch: '', commitDefault: false, editing: false,
  files: [], fileCount: 0, selectedCount: 0, filesOpenable: false, allSelected: true, noneSelected: true, someSelected: false, selectedInsertions: 0, selectedDeletions: 0,
  confirmTitle: '', confirmDescription: '', confirmContinue: '', confirmFeatureReason: '', canChangeBranch: false, publishStep: 0, publishProviders: [], publishRows: [], publishProvider: 'github', publishProviderLabel: 'GitHub',
  publishHost: 'github.com', publishPlaceholder: 'owner/repo', publishRepository: '', publishVisibility: 'private', publishRemote: 'origin', publishProtocol: 'ssh',
  publishAdvanced: false, publishPending: false, publishError: '', publishAnyReady: false, publishCanNext: false, publishCanSubmit: false, publishPushed: false,
  publishName: '', publishUrl: '', publishSummary: '', envPick: false, envOptions: [], envAdd: [], envAddMenuWidth: 160, envCount: 1,
  branch: HIDDEN_BRANCH,
};
