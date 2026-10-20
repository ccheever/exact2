import { describe, expect, test } from 'bun:test';
import { groupGitHubAccounts, hostCredentials, hostCredentialsCommand, nextGitHubHosts } from '../../src/settings/source-control-credentials';
import { obj, type Obj } from '../../src/client/domain';
import { connected } from '../support/client-fixture';
import { sourceControlRows, scopedPatch, DEFAULTS } from '../../src/settings/source-control-view';
import { checkoutCommand } from '../../src/shared/source-control-hosts';
import { newProjectPublishTargets } from '../../src/palette/palette-add';
import { canEditPullRequestChangeRequest, canEditPullRequestComment } from '../../src/pull-requests/pages-pr-writes-logic';
import { emptyThreadState, presentThreads } from '../../src/pull-requests/pages-pr-threads';
import { canDeletePullRequestBranch } from '../../src/details/pr-row-logic';
import { contractComponent } from '../support/contract-action-fixture';

describe('source control host settings', () => {
  const github = { hosts: { 'github.com': { enabled: true, account: 'alice' } }, tokens: { 'github.com': '<redacted>', 'code.example': '<other-redacted>' } };
  const settings = { sourceControlHosts: { github, gitcafe: { token: '<cafe-redacted>' }, bitbucket: { accessToken: '', email: 'before@example.com', apiToken: '<api-redacted>' } } };
  test('groups stored accounts per host and separates broken and environment logins', () => {
    expect(groupGitHubAccounts([{ host: 'github.com', account: 'alice', authenticated: true }, { host: 'github.com', account: 'bob', authenticated: true, active: true },
      { host: 'github.com', account: 'broken', authenticated: false }, { host: 'github.com', account: 'env', authenticated: true, environmentVariable: 'GH_TOKEN' }])[0])
      .toMatchObject({ active: 'bob', selectable: [{ account: 'alice' }, { account: 'bob' }], broken: [{ account: 'broken' }], environmentVariable: 'GH_TOKEN' });
    expect(nextGitHubHosts(github.hosts, 'github.com', { account: null })).toEqual({});
    expect(nextGitHubHosts(github.hosts, 'github.com', { enabled: false })).toEqual({ 'github.com': { enabled: false, account: 'alice' } });
  });
  test('secret fields carry only configured status and an empty draft', () => {
    const views = hostCredentials(settings, {}, 'env1', 2, true);
    expect(views.find(view => view.kind === 'gitcafe')?.fields).toEqual([{ key: 'token', label: 'Token', description: expect.any(String), secret: true, value: '', saved: true }]);
    expect(views.find(view => view.kind === 'bitbucket')?.fields.find(field => field.key === 'apiToken')).toMatchObject({ value: '', saved: true });
    expect(JSON.stringify(views)).not.toContain('redacted>');
  });
  test('token/account and generic field writes retain other redacted secrets and rescan', async () => {
    const { client, native } = await connected(), later = native.later.bind(native), patches: Obj[] = [];
    client.scopes = [...client.scopes, 'settings:write'];
    let rescans = 0;
    native.later = async input => {
      const request = obj(input);
      if (request.method === 'server.getSettings') return native.good(settings);
      if (request.method === 'server.updateSettings') { patches.push(obj(obj(request.payload).patch)); return native.good(settings); }
      return later(input);
    };
    const write = (input: Record<string, string>) => hostCredentialsCommand(client, native, { environment: 'env1', ...input }, () => rescans++);
    await write({ kind: 'github', action: 'token', host: ' CODE.Example ', value: ' synthetic-token ' });
    expect(obj(obj(patches[0].sourceControlHosts).github)).toEqual({ ...github, tokens: { ...github.tokens, 'code.example': 'synthetic-token' } });
    await write({ kind: 'github', action: 'account', host: 'github.com', value: 'bob' });
    expect(obj(obj(patches[1].sourceControlHosts).github)).toEqual({ ...github, hosts: { 'github.com': { enabled: true, account: 'bob' } } });
    await write({ kind: 'bitbucket', action: 'field', key: 'email', value: 'after@example.com' });
    expect(obj(patches[2].sourceControlHosts)).toEqual({ ...settings.sourceControlHosts, bitbucket: { ...settings.sourceControlHosts.bitbucket, email: 'after@example.com' } });
    await write({ kind: 'gitcafe', action: 'remove' });
    expect(obj(obj(patches[3].sourceControlHosts).gitcafe)).toEqual({ token: '' });
    expect(rescans).toBe(4);
    await expect(write({ kind: 'github', action: 'token', host: 'https://github.com', value: 'x' })).rejects.toThrow('host name');
    await expect(write({ kind: 'gitcafe', action: 'field', key: 'other', value: 'x' })).rejects.toThrow('Unsupported');
    await expect(hostCredentialsCommand(client, native, { environment: 'removed', kind: 'gitcafe', action: 'remove' }, () => {})).rejects.toThrow('no longer selected');
    expect(patches).toHaveLength(4);
  });
  test('an unchanged secret field blur sends no write', () => {
    const owner = contractComponent('source-control-credentials.contract', 'HostCredentialInput'), calls: unknown[][] = [];
    const scope = { field: { key: 'token', value: '', secret: true, saved: true }, kind: 'gitcafe', environmentId: 'env1', restInput: (...args: unknown[]) => calls.push(args) };
    owner.call('commit', owner.state(scope), scope, ['']);
    expect(calls).toEqual([]);
    owner.call('commit', owner.state(scope), scope, ['new-token']);
    expect(calls).toEqual([['host-credentials', 'environment=env1&kind=gitcafe&action=field&key=token&value=', 'new-token']]);
  });
  test('the t3 prefix and merge credit preference share project override semantics', () => {
    expect(DEFAULTS.branchNamePrefix).toBe('t3');
    expect(scopedPatch({}, '', 'branchNamePrefix', '__default__')).toEqual({ branchNamePrefix: 't3' });
    expect(scopedPatch({ projectSettingsOverrides: { p: { branchNamePrefix: 'custom' } } }, 'p', 'removeAgentCreditsOnMerge', 'true'))
      .toEqual({ projectSettingsOverrides: { p: { branchNamePrefix: 'custom', removeAgentCreditsOnMerge: true } } });
    expect(sourceControlRows({ removeAgentCreditsOnMerge: true }, '', 'Local', [], true).repositories.find(row => row.key === 'removeAgentCreditsOnMerge')?.checked).toBe(true);
  });
});

test('new project targets include ready hosts with a known owner and GitCafe uses its CLI', () => {
  const discovery = { sourceControlProviders: ['github', 'gitlab', 'gitcafe', 'bitbucket', 'azure-devops', 'forgejo'].map(kind => ({ kind, status: 'available', auth: { status: 'authenticated', account: { _tag: 'Some', value: 'fixture' } } })) };
  expect(newProjectPublishTargets(discovery).map(target => target.definition.kind)).toEqual(['github', 'gitlab', 'forgejo', 'gitcafe']);
  expect(checkoutCommand({ provider: 'gitcafe', number: 7 })).toBe('cafe pr checkout 7');
  expect(checkoutCommand({ provider: 'gitlab', number: 9 })).toBe('glab mr checkout 9');
  expect(checkoutCommand({ provider: 'unknown', number: 9 })).toBe('');
});

test('item-specific edit and resolve permissions override login inference', () => {
  const detail = { capabilities: { edit: { changeRequest: true, comment: true }, review: { resolve: true } }, author: { login: 'me' }, viewer: 'me', viewerPermissions: { editChangeRequest: false, actions: ['merge'], resolve: true } };
  expect(canEditPullRequestChangeRequest(detail)).toBe(false);
  expect(canEditPullRequestChangeRequest({ ...detail, viewer: 'other', viewerPermissions: { editChangeRequest: true } })).toBe(true);
  expect(canEditPullRequestComment(detail, { kind: 'issue-comment', author: { login: 'me' }, canEdit: false })).toBe(false);
  expect(canEditPullRequestComment(detail, { kind: 'review-comment', author: { login: 'other' }, canEdit: true })).toBe(true);
  const cards = presentThreads({}, emptyThreadState(), { projectId: 'p', repository: 'a/b', number: 1 }, detail, [{ id: 'a', canResolve: false }, { id: 'b' }], 0);
  expect(cards.map(card => card.canResolve)).toEqual([false, true]);
});

test('deleting the source branch requires a completed PR, capability and permission', () => {
  const detail = { state: 'merged', capabilities: { deleteBranch: true, actions: ['delete-branch'] }, viewerPermissions: { actions: ['delete-branch'] } };
  expect(canDeletePullRequestBranch(detail)).toBe(true);
  expect(canDeletePullRequestBranch({ ...detail, state: 'closed' })).toBe(true);
  expect(canDeletePullRequestBranch({ ...detail, state: 'open' })).toBe(false);
  expect(canDeletePullRequestBranch({ ...detail, capabilities: { ...detail.capabilities, deleteBranch: false } })).toBe(false);
  expect(canDeletePullRequestBranch({ ...detail, viewerPermissions: { actions: [] } })).toBe(false);
});


test('source-branch confirmation keeps Tab on Cancel while its destructive action is pending', () => {
  const dialog = contractComponent('pages-pr-actions.contract', 'PrActionDialog');
  for (const pending of [false, true]) for (const shiftKey of [false, true]) {
    const focused: string[] = [];
    dialog.call('fromCancel', {}, { actions: { pending }, preventDefault() {}, focus: (id: string) => focused.push(id) }, ['Tab', { shiftKey, metaKey: false, ctrlKey: false, altKey: false }]);
    expect(focused).toEqual([pending ? 'pr-action-dialog-cancel' : 'pr-action-dialog-confirm']);
  }
});
