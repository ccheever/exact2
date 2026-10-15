// A pull request chip's press, in the composer and in a sent message (contextChipParts.tsx PullRequestChip's `onOpen`,
// T3 Code 1e2ecbd975, MIT, see LICENSE-T3): useOpenPrLink opens it beside the thread when its URL names a project of this
// environment that reads pull requests (useOpenChangeRequestLink), and otherwise as an ordinary link that follows
// "Open links in".
import type { T3Client } from './client';
import type { Native } from './protocol';
import { surfaceLocal } from './r4-surfaces-panel';
import { prEnvironment, readsPullRequests } from './pages-pr-environments';
import { resolvePullRequestPreviewTarget } from './pages-pr-links-logic';
import { openLink } from './pages-pr-links';

/** The preview target (`r5-pr-open`'s reference) for `url`, or '' when no project of the focused environment names it. */
export function pullRequestChipTarget(client: T3Client, url: string): string {
  const environment = prEnvironment(client, client.environmentId);
  const target = environment ? resolvePullRequestPreviewTarget({ environmentId: environment.id, projects: environment.projects.map(project => ({ ...project, environmentId: environment.id })),
    pullRequestsEnabled: readsPullRequests(environment), url }) : null;
  return target ? JSON.stringify(target.input) : '';
}
export async function openPullRequest(client: T3Client, native: Native, url: string): Promise<void> {
  await openLink(client, native, false, `\n${pullRequestChipTarget(client, url)}\n${url}\n`, async (link, where) => { await surfaceLocal(client, native, 'r5-pr-open', where, link); });
}
