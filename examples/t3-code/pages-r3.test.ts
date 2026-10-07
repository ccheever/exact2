// Lane "pages", round 3: the wizard after a failed pairing, CommandBlock's
// Copy→Check, and the pull request viewer's not-found state (7bc161f869).
import { afterEach, beforeEach, expect, test } from 'bun:test';
import { noPrimary, resetPrimary } from './local-primary-fixture';
// These cases are the hosted rules (resolveHostedFirstRunDecision): no embedded server runs on this Mac.
beforeEach(noPrimary);
afterEach(resetPrimary);
import { welcomeView, welcomeLocal } from './pages-welcome';
import { pullRequestDetail, isPullRequestNotFound, unavailable, gitHubPullRequestBrowserUrl } from './pages-pr-detail';
import { ClientError, type Native } from './protocol';
import type { T3Client } from './client';
import type { Obj } from './domain';

function native(saved: Obj[], copied: string[] = []): Native {
  return { available: true, watch: () => {}, later: async (request: Obj) => {
    if (request.op === 'environments') return { ok: true, value: { saved }, generation: 1 };
    if (request.op === 'copyText') { copied.push(String(request.text)); return { ok: true, value: {}, generation: 1 }; }
    return { ok: true, value: {}, generation: 1 };
  } } as unknown as Native;
}
const wizardClient = (connection: string) => ({ local: {}, origin: 'http://127.0.0.1:9', environmentId: 'env-x', connection, statusMessage: 'Connecting to 127.0.0.1…', scopes: [],
  config: { environment: { label: 'Mac' } }, shell: { projects: [], threads: [], sequence: 0 }, threadId: '', projectId: '' }) as unknown as T3Client;

test('a pairing that never connected is not listed as a computer that is still connecting', async () => {
  const failed = wizardClient('reconnecting');
  const view = await welcomeView(failed, native([]), { step: 'connect', now: 1 });
  expect([view.show, view.computers.length, view.ready]).toEqual([true, 0, false]);
  // Once it has connected it stays listed, connecting or not.
  const live = wizardClient('connected');
  expect((await welcomeView(live, native([]), { step: 'connect', now: 1 })).computers.map(computer => computer.status)).toEqual(['Connected']);
  (live as unknown as { connection: string }).connection = 'reconnecting';
  expect((await welcomeView(live, native([]), { step: 'connect', now: 1 })).computers.map(computer => computer.status)).toEqual(['Connecting…']);
});

test('copying the pairing command flips the button to Check without a toast', async () => {
  const client = wizardClient('connected'), copied: string[] = [];
  expect((await welcomeView(client, native([], copied), { step: 'connect', now: 1 })).commandCopied).toBe(0);
  await welcomeLocal(client, native([], copied), 'copy-command', '', 'npx t3 pair');
  await welcomeLocal(client, native([], copied), 'copy-command', '', 'npx t3 pair');
  expect(copied).toEqual(['npx t3 pair', 'npx t3 pair']);
  expect((await welcomeView(client, native([], copied), { step: 'connect', now: 1 })).commandCopied).toBe(2);
  expect((client as unknown as { toasts?: unknown[] }).toasts ?? []).toEqual([]);
});

const selection = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'ccheever/exact2', number: 7 });
const identity = { provider: 'github', canonicalKey: 'github.com/ccheever/exact2', locator: { remoteUrl: 'git@github.com:ccheever/exact2.git' } };
function prClient(error: Error): T3Client {
  return { ready: true, environmentId: 'env', shell: { projects: [{ id: 'p1', repositoryIdentity: identity }] },
    rpc: async () => { throw error; } } as unknown as T3Client;
}

test('a pull request link that names an issue reads as not found; other failures keep the default title', async () => {
  const notFound = new ClientError('Pull request operation detail failed: Could not resolve to a PullRequest with the number of 7.', 'PullRequestOperationError', false, { reason: 'not-found', detail: 'Could not resolve' });
  expect(isPullRequestNotFound(notFound)).toBe(true);
  expect(isPullRequestNotFound(new ClientError('x', 'PullRequestsUnavailableError', false, { reason: 'not-found' }))).toBe(false);
  const view = await pullRequestDetail(prClient(notFound), { available: true } as Native, { selected: selection, refresh: 1, now: 0 });
  expect([view.errorTitle, view.error, view.githubUrl, view.numberLabel]).toEqual(['Pull request #7 not found', "It may be an issue rather than a pull request, or this account can't see it.", 'https://github.com/ccheever/exact2/pull/7', '#7']);
  const other = await pullRequestDetail(prClient(new ClientError('Pull request operation detail failed: gh is not signed in.', 'PullRequestOperationError')), { available: true } as Native, { selected: selection, refresh: 2, now: 0 });
  expect([other.errorTitle, other.error]).toEqual(['Could not load pull requests', 'Pull request operation detail failed: gh is not signed in.']);
  expect(unavailable({ projectId: 'p', host: '', repository: 'a/b', number: 3 }, { error: 'x', notFound: false }, undefined).githubUrl).toBe('');
  expect(gitHubPullRequestBrowserUrl({ provider: 'github', canonicalKey: 'ghe.example.com/a/b', locator: { remoteUrl: 'https://ghe.example.com/a/b.git' } }, 'a/b', 3)).toBe('https://ghe.example.com/a/b/pull/3');
  expect(gitHubPullRequestBrowserUrl({ provider: 'gitlab' }, 'a/b', 3)).toBe('');
});
