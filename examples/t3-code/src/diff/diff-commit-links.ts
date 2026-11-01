// Where a code-host commit link opens, adapted from T3 Code 2359643cff (MIT; see LICENSE-T3):
// lib/openPullRequestLink.ts useOpenChangeRequestLink's commit branch. A link to a commit of the thread's own repository
// (its project's host and repository) asks review.getDiffPreview for that commit in the thread's workspace; one the
// workspace has opens in the Diff pane, selected; one it lacks opens the pull request the link names (a GitHub
// `/pull/<n>/commits/<sha>`), else follows "Open links in". A panel choice or another Diff selection made while the
// lookup runs wins over it (rightPanelStore markUserAction). ⌘ and Ctrl open it outside; Shift and Alt keep their
// browser meaning, as "Open pull requests in T3 Code" turned off does.
import type { T3Client } from '../client/client';
import { arr, str } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { currentSelection, selectCommit, workspace, branchBaseRef } from './diff';
import { ownCommitLink } from './diff-commits';
import { openDiffOnSelection, panelState } from '../panel/panel-surfaces';
import { prLinksOpenInApp, prTarget } from '../panel/pr-surface';
import { openPullRequestIn } from '../panel/pr-attachment-surfaces';
import { openLink } from '../browser/browser-links';

export type LinkModifiers = { meta: boolean; ctrl: boolean; shift: boolean; alt: boolean };

/** Answers whether the link was taken as a commit link; when it was, it is already open (or a newer choice won). */
export async function openCommitLink(client: T3Client, native: Native, url: string, held: LinkModifiers): Promise<boolean> {
  if (!prLinksOpenInApp(client) || held.meta || held.ctrl || !client.threadId) return false;
  const project = client.shell.projects.find(entry => entry.id === client.projectId);
  const commit = ownCommitLink(project, url);
  if (!commit || held.shift || held.alt) return false;
  const cwd = workspace(client);
  if (!cwd) return false;
  const state = panelState(client), revision = ++state.userRevision, threadId = client.threadId;
  const selection = JSON.stringify(currentSelection(client)), baseRef = branchBaseRef(client);
  let local = false;
  try {
    const result = await client.rpc(native, 'review.getDiffPreview', { cwd, ...(baseRef ? { baseRef } : {}), commitSha: commit.sha, ignoreWhitespace: client.diffState.ignoreWhitespace });
    local = arr(result.sources).some(source => source.kind === 'commit' && str(source.headRef).toLowerCase() === commit.sha);
  } catch (error) { if (letGo(error)) return true; }
  if (client.threadId !== threadId || panelState(client) !== state || state.userRevision !== revision || JSON.stringify(currentSelection(client)) !== selection) return true;
  if (local) { selectCommit(client, commit.sha); await openDiffOnSelection(client, native); return true; }
  const target = prTarget(client, url);
  if (target) { client.diffOpen = false; openPullRequestIn(state, target); return true; }
  await openLink(client, native, url);
  return true;
}
