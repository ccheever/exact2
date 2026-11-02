// T3 Code 9c161462c9 (MIT): packages/source-control-*/src/client/definition.ts; each host's change request terms at
// 7c3b3b3605ef (src/shared/definition.ts `changeRequest`).
// Host presentation and supported operations are shared by clone, publish and PR consumers.
import { num, str, type Obj } from '../client/domain';

/** What a host calls a change request; the article follows how the short label is said ("an MR"). */
export type ChangeRequestTerms = { shortLabel: string; shortLabelWithArticle: string; singular: string; plural: string };
const PULL_REQUEST: ChangeRequestTerms = { shortLabel: 'PR', shortLabelWithArticle: 'a PR', singular: 'pull request', plural: 'pull requests' };
const MERGE_REQUEST: ChangeRequestTerms = { shortLabel: 'MR', shortLabelWithArticle: 'an MR', singular: 'merge request', plural: 'merge requests' };
/** UNKNOWN_SOURCE_CONTROL_CLIENT's terms, for a host no definition claims. */
export const CHANGE_REQUEST_TERMS: ChangeRequestTerms = { shortLabel: 'change request', shortLabelWithArticle: 'a change request', singular: 'change request', plural: 'change requests' };

export const sourceControlHosts = [
  { kind: 'github', label: 'GitHub', pickerLabel: 'GitHub', host: 'github.com', pathHint: 'owner/repo', cloneTransport: 'https', inferOwner: true, changeRequest: PULL_REQUEST, actions: ['merge', 'ready', 'draft', 'close', 'reopen', 'delete-branch', 'update-branch', 'enable-auto-merge', 'disable-auto-merge', 'revert', 'approve-workflows'] },
  { kind: 'gitlab', label: 'GitLab', pickerLabel: 'GitLab', host: 'gitlab.com', pathHint: 'group/project', cloneTransport: 'ssh', inferOwner: true, changeRequest: MERGE_REQUEST, actions: ['merge', 'ready', 'draft', 'close', 'reopen', 'update-branch', 'enable-auto-merge', 'disable-auto-merge'] },
  { kind: 'forgejo', label: 'Forgejo', pickerLabel: 'Forgejo / Gitea', host: 'your server', pathHint: 'owner/repo', cloneTransport: 'https', inferOwner: true, changeRequest: PULL_REQUEST, actions: ['merge', 'close', 'reopen', 'update-branch'] },
  { kind: 'bitbucket', label: 'Bitbucket', pickerLabel: 'Bitbucket', host: 'bitbucket.org', pathHint: 'workspace/repository', cloneTransport: 'ssh', inferOwner: false, changeRequest: PULL_REQUEST, actions: ['merge', 'close'] },
  { kind: 'azure-devops', label: 'Azure DevOps', pickerLabel: 'Azure DevOps', host: 'dev.azure.com', pathHint: 'project/repository', cloneTransport: 'ssh', inferOwner: false, changeRequest: PULL_REQUEST, actions: ['merge', 'ready', 'draft', 'close', 'reopen', 'enable-auto-merge', 'disable-auto-merge'] },
  { kind: 'gitcafe', label: 'GitCafe', pickerLabel: 'GitCafe', host: 'git.cafe', pathHint: 'owner/repo', cloneTransport: 'ssh', inferOwner: true, changeRequest: PULL_REQUEST, actions: ['merge', 'ready', 'draft', 'close', 'reopen', 'update-branch'] },
];
export const sourceControlHost = (kind: string) => sourceControlHosts.find(host => host.kind === kind);
export const sourceControlHostName = (kind: string) => sourceControlHost(kind)?.label ?? kind;
export const sourceControlHostActions = (kind: string) => sourceControlHost(kind)?.actions ?? [];
/** definition.ts `hardLineBreaks` (2359643cff): the hosts whose own rendering keeps a single newline as a line break. */
export const sourceControlHardLineBreaks = (kind: string) => ['github', 'forgejo', 'azure-devops'].includes(kind);
/** A host's change request terms, "change request" for a host no definition claims. */
export const changeRequestTerms = (kind: string): ChangeRequestTerms => sourceControlHost(kind)?.changeRequest ?? CHANGE_REQUEST_TERMS;

export function checkoutCommand(detail: Obj): string {
  const number = num(detail.number);
  if (!Number.isSafeInteger(number) || number < 1) return '';
  switch (str(detail.provider, 'github')) {
    case 'github': return `gh pr checkout ${number}`;
    case 'gitlab': return `glab mr checkout ${number}`;
    case 'gitcafe': return `cafe pr checkout ${number}`;
    case 'azure-devops': return `az repos pr checkout --id ${number}`;
    case 'forgejo': {
      const url = str(detail.repositoryUrl);
      return url ? `git fetch '${url.replaceAll("'", "'\\''")}' refs/pull/${number}/head && git checkout -B pulls/${number} FETCH_HEAD` : '';
    }
    case 'bitbucket': {
      const repository = str(detail.headRepositoryNameWithOwner), branch = str(detail.headBranch);
      return /^[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+$/.test(repository) && /^[A-Za-z0-9._/@+=,-]+$/.test(branch)
        ? `git clone --single-branch --branch ${branch} https://bitbucket.org/${repository}.git t3code-pr-${number}` : '';
    }
    default: return '';
  }
}

export const sourceControlHostFields = [
  { kind: 'gitcafe', key: 'token', label: 'Token', secret: true, description: 'A git.cafe token, used before CAFE_TOKEN and the cafe login, so GitCafe works without the CLI signed in.' },
  { kind: 'bitbucket', key: 'accessToken', label: 'Access token', secret: true, description: "Scoped to one repository, project, or workspace. Create it in that item's Bitbucket settings." },
  { kind: 'bitbucket', key: 'email', label: 'Atlassian account email', secret: false, description: 'With an API token instead of an access token.' },
  { kind: 'bitbucket', key: 'apiToken', label: 'API token', secret: true, description: 'Give it read and write access to repositories and pull requests, and read:user:bitbucket.' },
];
