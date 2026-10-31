import { expect, test } from 'bun:test';
import { findPromptLinkProject, findPromptLinkPullRequestThread, resolvePromptLinkModelSelection } from '../../src/prompt-links/prompt-link-model';

test('repository matching uses the first exact origin and never groups a fork with its upstream', () => {
  const projects = [
    { id: 'fork', repositoryIdentity: { canonicalKey: 'github.com/pingdotgg/t3code', origin: { canonicalKey: 'github.com/me/t3code' } } },
    { id: 'none', repositoryIdentity: null },
    { id: 'recent', repositoryIdentity: { canonicalKey: 'github.com/PingDotGG/T3Code' } },
    { id: 'older', repositoryIdentity: { canonicalKey: 'github.com/pingdotgg/t3code' } },
  ];
  expect(findPromptLinkProject(projects, { owner: 'pingdotgg', name: 't3code' })?.id).toBe('recent');
  expect(findPromptLinkProject(projects, { owner: 'me', name: 't3code' })?.id).toBe('fork');
});

test('repository matching accepts checkouts cloned from git.cafe and its staging host', () => {
  const projects = [
    { id: 'gitlab', repositoryIdentity: { canonicalKey: 'gitlab.com/pingdotgg/t3code' } },
    { id: 'cafe', repositoryIdentity: { canonicalKey: 'git.cafe/PingDotGG/t3code' } },
    { id: 'staging', repositoryIdentity: { canonicalKey: 'staging.git.cafe/pingdotgg/other' } },
  ];
  expect(findPromptLinkProject(projects, { owner: 'pingdotgg', name: 't3code' })?.id).toBe('cafe');
  expect(findPromptLinkProject(projects, { owner: 'pingdotgg', name: 'other' })?.id).toBe('staging');
  expect(findPromptLinkProject(projects, { owner: 'pingdotgg', name: 'missing' })).toBeUndefined();
});

test('PR links open the most recently updated visible match, including legacy links', () => {
  const link = { host: 'github.com', repository: 'pingdotgg/t3code', number: 7 };
  const threads = [
    { id: 'older', updatedAt: '2026-01-01', pullRequests: [link] },
    { id: 'legacy', updatedAt: '2026-02-01', linkedPullRequest: { repository: link.repository, number: 7, url: 'https://github.com/pingdotgg/t3code/pull/7' } },
    { id: 'dismissed', updatedAt: '2026-03-01', pullRequests: [{ ...link, source: 'stack-dismissed' }] },
    { id: 'empty', updatedAt: '2026-04-01', pullRequests: [], linkedPullRequest: { ...link, url: 'https://github.com/pingdotgg/t3code/pull/7' } },
  ];
  expect(findPromptLinkPullRequestThread(threads, { owner: 'PingDotGG', name: 't3code', number: 7 })?.id).toBe('legacy');
  expect(findPromptLinkPullRequestThread(threads, { owner: 'PingDotGG', name: 't3code', number: 8 })).toBeUndefined();
});

test('provider/model hints resolve aliases and available instances without choosing an unavailable provider', () => {
  const models = [{ slug: 'small', name: 'Small' }, { slug: 'big', name: 'Big', aliases: ['opus'], isDefault: true }];
  const providers = [
    { instanceId: 'broken', driver: 'claudeAgent', enabled: true, status: 'error', models },
    { instanceId: 'work', driver: 'claudeAgent', enabled: true, status: 'ready', models },
    { instanceId: 'codex', driver: 'codex', enabled: true, status: 'ready', availability: 'unavailable', models },
  ];
  expect(resolvePromptLinkModelSelection(providers, { provider: 'claude', model: 'opus' })).toEqual({ selection: { instanceId: 'work', model: 'big' }, exact: true });
  expect(resolvePromptLinkModelSelection(providers, { provider: 'claude', model: 'unknown' })).toEqual({ selection: { instanceId: 'work', model: 'big' }, exact: false });
  expect(resolvePromptLinkModelSelection(providers, { provider: 'codex', model: null })).toBeNull();
});
