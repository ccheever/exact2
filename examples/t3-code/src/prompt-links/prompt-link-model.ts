// T3 Code 6d450a4284 (MIT, LICENSE-T3): packages/client-runtime/src/promptLinks.ts.
// The clone keeps received entities as plain objects; repository and PR keys retain the reference's matching rules.
import { arr, obj, str, type Obj } from '../client/domain';
import { resolveSelectableModel } from '../composer/composer-controls-model';
import { visiblePullRequests } from '../panel/shell-pr';
import type { PromptLinkPullRequest, PromptLinkRepository } from './prompt-links';

// A link's `owner/name` is GitHub's. GitCafe syncs a repository two-way with GitHub under the
// same path, so a checkout cloned from git.cafe counts as the linked repository.
const PROMPT_LINK_REPOSITORY_HOSTS = ['github.com', 'git.cafe', 'staging.git.cafe'];

export function findPromptLinkProject<T extends Obj>(projects: readonly T[], repository: PromptLinkRepository): T | undefined {
  const path = `${repository.owner}/${repository.name}`.toLowerCase();
  const keys = new Set(PROMPT_LINK_REPOSITORY_HOSTS.map(host => `${host}/${path}`));
  return projects.find(project => {
    const identity = obj(project.repositoryIdentity);
    return keys.has(str(obj(identity.origin).canonicalKey ?? identity.canonicalKey).toLowerCase());
  });
}
export function findPromptLinkPullRequestThread<T extends Obj>(threads: readonly T[], pullRequest: PromptLinkPullRequest): T | undefined {
  const repository = `${pullRequest.owner}/${pullRequest.name}`.toLowerCase();
  let latest: T | undefined;
  for (const thread of threads) {
    if (latest && str(thread.updatedAt) <= str(latest.updatedAt)) continue;
    const legacy = obj(thread.linkedPullRequest);
    let legacyHost = '';
    try { legacyHost = new URL(str(legacy.url)).hostname; } catch {}
    const links = Array.isArray(thread.pullRequests) ? visiblePullRequests(thread.pullRequests) : Object.keys(legacy).length ? [{ ...legacy, host: legacyHost }] : [];
    if (links.some(link => str(link.host).toLowerCase() === 'github.com' && str(link.repository).toLowerCase() === repository && link.number === pullRequest.number)) latest = thread;
  }
  return latest;
}
export function resolvePromptLinkModelSelection(providers: readonly Obj[], request: { provider: string; model: string | null }): { selection: { instanceId: string; model: string }; exact: boolean } | null {
  const driver = request.provider === 'claude' ? 'claudeagent' : request.provider;
  const usable = providers.filter(provider => str(provider.driver).toLowerCase() === driver && provider.enabled === true && provider.availability !== 'unavailable');
  const instance = usable.find(provider => provider.status === 'ready') ?? usable.find(provider => provider.status === 'warning');
  if (!instance) return null;
  const models = arr(instance.models);
  const requested = request.model === null ? '' : resolveSelectableModel(str(instance.driver), request.model, models);
  const model = requested || str((models.find(candidate => candidate.isDefault === true) ?? models[0])?.slug);
  return model ? { selection: { instanceId: str(instance.instanceId), model }, exact: request.model === null || requested !== '' } : null;
}
