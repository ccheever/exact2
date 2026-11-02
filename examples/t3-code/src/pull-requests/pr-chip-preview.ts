// A sent pull request chip's hover, adapted from T3 Code 2359643cff (MIT, see LICENSE-T3): contextChipParts.tsx
// PullRequestChip. With a preview target (usePullRequestPreviewTarget: resolvePullRequestPreviewTarget on the
// thread's environment) the chip is a PullRequestLinkPreview, whose card falls back to PullRequestContextDetails when
// the read fails; without one, PullRequestContextDetails is the chip's tooltip.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { capitalizeTerm, terminologyForUrl } from '../details/git-logic';
import { resolvePullRequestPreviewTarget } from './pages-pr-links-logic';
import { prEnvironment, readsPullRequests } from './pages-pr-environments';

/** The card's target as pages-pr-links.ts readPreview reads it (linkChips' `target`), or '' when there is none. */
export function pullRequestChipTarget(client: T3Client | undefined, url: string): string {
  const environment = client?.environmentId ? prEnvironment(client, client.environmentId) : null;
  if (!environment) return '';
  const target = resolvePullRequestPreviewTarget({ environmentId: environment.id, projects: environment.projects.map(project => ({ ...project, environmentId: environment.id })),
    pullRequestsEnabled: readsPullRequests(environment), url });
  return target ? JSON.stringify({ ...target.input, ...(environment.focused ? {} : { environmentId: environment.id }) }) : '';
}

const STATE_LABELS: Record<string, string> = { open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' };

/**
 * PullRequestContextDetails' lines, in the order pages-pr-links.contract PrContextDetails reads them: the state's key
 * (resolvePullRequestState: an open draft is "draft") and label, "<Term> #N" in the URL's host's words, the title, the
 * head branch and the base branch.
 */
export function pullRequestContextLines(metadata: Obj): string[] {
  const state = str(metadata.state) === 'open' && metadata.isDraft === true ? 'draft' : STATE_LABELS[str(metadata.state)] ? str(metadata.state) : 'open';
  const url = str(metadata.url);
  return [state, STATE_LABELS[state]!, `${capitalizeTerm(terminologyForUrl(url).singular)} #${Number(metadata.number)}`, str(metadata.title),
    str(metadata.headBranch), str(metadata.baseBranch)];
}

/** The hover a chip's pull request metadata gives it: the card's target (`size`) and the details' lines (`menu`). */
export function pullRequestChipHover(client: T3Client | undefined, pullRequest: unknown): { size: string; menu: { id: string; label: string }[] } {
  const metadata = obj(pullRequest);
  return { size: pullRequestChipTarget(client, str(metadata.url)), menu: pullRequestContextLines(metadata).map((label, index) => ({ id: String(index), label })) };
}
