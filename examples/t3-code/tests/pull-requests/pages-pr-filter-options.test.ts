// Filters › Author and Labels search the host (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3:
// pullRequestList.logic.test.ts "filter picker choices" with its original names; PullRequestListFilters.tsx
// useHostedFilterOptions; state/pullRequests.ts usePullRequestFilterOptions).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { pullRequestsPage, prLocal, type PrInput } from '../../src/pull-requests/pages-prs';
import { filterOptionTargets, hostedFilterOptions, pullRequestAuthorChoices, pullRequestLabelChoices, selectedPullRequestAuthor } from '../../src/pull-requests/pages-pr-filter-options';
import { contractComponent, value } from '../support/contract-action-fixture';
import { appPath, appText } from '../support/app-files';

describe('filter picker choices', () => {
  const actor = (login: string, name: string | null = null) => ({ login, name, avatarUrl: null });

  test('offers authors the host found that no loaded row carries, after the loaded ones', () => {
    const loaded = [{ actor: actor('julius', 'Julius'), count: 4, mergedCount: 3 }, { actor: actor('theo'), count: 1, mergedCount: 0 }];
    const choices = pullRequestAuthorChoices(loaded, [actor('Julius'), actor('juliana', 'Juliana')], 'jul');
    expect(choices.map(choice => [choice.actor.login, choice.count])).toEqual([['julius', 4], ['juliana', 0]]);
  });

  test("keeps showing an author picked from the host's search once the search clears", () => {
    const loaded = [{ actor: actor('julius'), count: 4, mergedCount: 3 }];
    const hosted = [actor('Juliana', 'Juliana')];
    expect(selectedPullRequestAuthor(loaded, hosted, 'juliana')?.actor).toEqual(actor('Juliana', 'Juliana'));
    expect(selectedPullRequestAuthor(loaded, [], 'juliana')?.actor.login).toBe('juliana');
    expect(selectedPullRequestAuthor(loaded, hosted, 'JULIUS')?.count).toBe(4);
    expect(selectedPullRequestAuthor(loaded, hosted, undefined)).toBeUndefined();
  });

  test("puts the host's authors after the first loaded matches rather than after all of them", () => {
    const loaded = Array.from({ length: 40 }, (_, index) => ({ actor: actor(`dev-${index}`), count: 1, mergedCount: 0 }));
    const choices = pullRequestAuthorChoices(loaded, [actor('dev-from-host')], 'dev');
    expect(choices.slice(0, 30).map(choice => choice.actor.login)).toContain('dev-from-host');
  });

  test("keeps a loaded label's count when only the host's search found it", () => {
    const loaded = [{ name: 'size:XL', color: '#d93f0b', count: 7 }];
    const choices = pullRequestLabelChoices(loaded, [{ name: 'size:XL', color: '#d93f0b' }, { name: 'bug', color: '' }], 'xl-ish');
    expect(choices).toEqual([{ name: 'size:XL', color: '#d93f0b', count: 7 }, { name: 'bug', color: '', count: 0 }]);
  });
});

const NOW = Date.parse('2026-10-08T12:00:00Z');
const entry = (number: number, login: string, labels: Obj[] = [], state = 'open') => ({ projectId: 'p1', host: 'github.com', repository: 'acme/playground', number, title: `PR ${number}`,
  state, isDraft: false, author: { login, name: null, avatarUrl: null }, labels, updatedAt: '2026-10-08T11:00:00Z', additions: 1, deletions: 0 });

function fixture(answer: (payload: Obj) => unknown) {
  const asked: Obj[] = [];
  let wakes = 0;
  const client = {
    environmentId: 'env', ready: true, generation: 1, revision: 0, local: {} as Obj,
    config: { environment: { capabilities: { pullRequests: true } } }, shell: { projects: [{ id: 'p1', title: 'playground', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/playground' } }], threads: [] },
    rpc: async (_native: unknown, method: string, payload: Obj) => {
      if (method === 'pullRequests.list') return { entries: [entry(1, 'julius', [{ name: 'size:XL', color: 'd93f0b' }]), entry(2, 'julius', [], 'merged'), entry(3, 'theo')], viewers: {} };
      if (method === 'pullRequests.listStats') return { stats: [] };
      if (method === 'pullRequests.filterOptions') { asked.push(payload); return answer(payload); }
      throw new Error(`no reply for ${method}`);
    },
    restAccess: () => ({ call: async () => ({ id: '1-1' }) }),
  } as unknown as T3Client;
  client.projectGroups = () => client.shell.projects.map(project => ({ key: String(project.id), name: String(project.title ?? ''), members: [project] }));
  const native = { available: true, watch: () => {}, later: async (request: Obj) => { if (request.op === 'wake') wakes++; return { ok: true, generation: 1, value: {} }; } } as unknown as Native;
  const page = (input: Partial<PrInput> = {}) => pullRequestsPage(client, native, { open: true, refresh: 0, now: NOW, selected: '', query: '', typed: false, state: 'all', ...input } as PrInput);
  prLocal(client, 'state', 'all');
  return { client, asked, page, wakes: () => wakes };
}

describe("the pickers' host search (usePullRequestFilterOptions)", () => {
  test('a picker opening shows its loaded choices pending, then asks each server in the list\'s scope and folds the answer in', async () => {
    const f = fixture(() => ({ authors: [{ login: 'Julius', name: null, avatarUrl: null }, { login: 'juliana', name: 'Juliana', avatarUrl: 'https://avatars.example/j.png' }], labels: [], truncated: false }));
    const pending = (await f.page({ optionKind: 'authors', optionQuery: '' })).options;
    expect([pending.kind, pending.query, pending.busy, pending.failed, pending.authors.map(person => person.login)]).toEqual(['authors', '', true, false, ['julius', 'theo']]);
    expect([f.wakes(), f.asked]).toEqual([1, []]);
    const view = await f.page({ optionKind: 'authors', optionQuery: '' });
    expect(f.asked).toEqual([{ kind: 'authors' }]);
    expect(view.options.busy).toBe(false);
    // The loaded authors lead with their merges, then the host's that no row carries; "merges loaded" only above 0.
    expect(view.options.authors.map(person => [person.login, person.detail, person.avatar])).toEqual([['julius', '1 merges loaded', ''], ['theo', '', ''], ['juliana', '', 'https://avatars.example/j.png']]);
    // The same search within five minutes is answered from what the host said.
    await f.page({ optionKind: 'authors', optionQuery: '', now: NOW + 60_000 });
    expect(f.asked).toHaveLength(1);
    await f.page({ optionKind: 'authors', optionQuery: '', now: NOW + 6 * 60_000 });
    expect(f.asked).toHaveLength(2);
  });

  test('a typed search goes with the list\'s scope; an author picked from the host stays first once the search clears', async () => {
    const f = fixture(payload => ({ authors: payload.query === 'jul' ? [{ login: 'juliana', name: 'Juliana', avatarUrl: null }] : [], labels: [], truncated: false }));
    prLocal(f.client, 'host', 'github.com');
    await f.page({ optionKind: 'authors', optionQuery: ' jul ' });
    const view = await f.page({ optionKind: 'authors', optionQuery: ' jul ' });
    expect(f.asked).toEqual([{ kind: 'authors', host: 'github.com', query: 'jul' }]);
    expect([view.options.query, view.options.authors.map(person => person.login)]).toEqual(['jul', ['julius', 'juliana']]);
    prLocal(f.client, 'author', 'juliana');
    await f.page({ optionKind: 'authors', optionQuery: '' });
    const cleared = await f.page({ optionKind: 'authors', optionQuery: '' });
    expect(cleared.options.authors.map(person => [person.login, person.selected])).toEqual([['juliana', true], ['julius', false], ['theo', false]]);
    expect(cleared.authorChosen.map(person => person.login)).toEqual(['juliana']);
  });

  test('labels keep a loaded count, add the host\'s, and keep each chosen label; a failed read says so', async () => {
    let fail = false;
    const f = fixture(() => { if (fail) throw new Error('Unknown request tag: pullRequests.filterOptions'); return { authors: [], labels: [{ name: 'size:XL', color: 'd93f0b' }, { name: 'bug', color: null }], truncated: false }; });
    prLocal(f.client, 'label', 'wontfix');
    await f.page({ optionKind: 'labels', optionQuery: '' });
    const view = await f.page({ optionKind: 'labels', optionQuery: '' });
    expect(view.options.labels.map(label => [label.name, label.color, label.count, label.selected])).toEqual([['wontfix', '', 0, true], ['size:XL', '#d93f0b', 1, false], ['bug', '', 0, false]]);
    fail = true;
    await f.page({ optionKind: 'labels', optionQuery: 'zzz' });
    const failed = await f.page({ optionKind: 'labels', optionQuery: 'zzz' });
    expect([failed.options.busy, failed.options.failed, failed.options.labels.map(label => label.name)]).toEqual([false, true, ['wontfix']]);
    // No picker open: the host is not asked.
    const asked = f.asked.length;
    expect((await f.page({ optionKind: '' })).options.kind).toBe('');
    expect(f.asked).toHaveLength(asked);
  });
});

describe('the Filters menu pickers (PrFiltersMenu, PrFilterSub)', () => {
  const menu = contractComponent('pages-prs.contract', 'PrFiltersMenu');
  const person = (login: string, selected = false) => ({ key: login, login, name: '', avatar: '', initial: login[0]!.toUpperCase(), detail: '', selected });
  const options = (over: Obj = {}) => ({ kind: '', query: '', busy: false, failed: false, authors: [], labels: [], ...over });
  const page = (over: Obj = {}) => ({ authors: [person('julius'), person('theo')], authorChosen: [], labelFacets: [], labelChosen: [], options: options(), servers: [], projects: [], ...over });
  const fns = async () => {
    const { contractFn } = await import('../support/contract-fn-fixture');
    const prAuthorMatches = await contractFn('pages-prs.contract', 'prAuthorMatches', ['person', 'needle']);
    const prLabelMatches = await contractFn('pages-prs.contract', 'prLabelMatches', ['label', 'needle']);
    return {
      prOptionQuery: await contractFn('pages-prs.contract', 'prOptionQuery', ['query']),
      prVisibleAuthors: await contractFn('pages-prs.contract', 'prVisibleAuthors', ['chosen', 'authors', 'query'], { prAuthorMatches }),
      prVisibleLabels: await contractFn('pages-prs.contract', 'prVisibleLabels', ['chosen', 'labels', 'query'], { prLabelMatches }),
      KmItem: (item: Obj) => item,
    };
  };

  test('opening Author or Labels starts its search empty and tells the root which picker searches the host; typing sends the trimmed search', async () => {
    const state = menu.state(), sent: unknown[][] = [], scope = { ...(await fns()), page: page(), control: (...args: unknown[]) => sent.push(args), focus: () => undefined };
    state.authorQuery = 'old';
    menu.call('open', state, scope, ['author']);
    expect([state.sub, state.authorQuery, sent]).toEqual(['author', '', [['option-kind', 'authors']]]);
    menu.call('searchAuthors', state, scope, ['  jul ']);
    expect([state.authorQuery, sent.at(-1)]).toEqual(['  jul ', ['option-query', 'jul']]);
    menu.call('open', state, scope, ['labels']);
    expect([state.sub, sent.at(-1)]).toEqual(['labels', ['option-kind', 'labels']]);
    menu.call('open', state, scope, ['labels']);
    expect([state.sub, sent.at(-1)]).toEqual(['', ['option-kind', '']]);
  });

  test('the pickers show the loaded matches and a spinner until the host answers what is typed, then its answer; empty, they say why', async () => {
    const base = await fns(), state = { ...menu.state(), sub: 'author', authorQuery: 'ju' };
    const derived = (over: Obj) => menu.derives(state, { ...base, page: page(over) });
    let view = derived({ options: options({ kind: 'authors', query: 'j', authors: [person('julius'), person('juliana')] }) });
    expect([view.authorFresh, view.authorsBusy, (view.authors as { login: string }[]).map(entry => entry.login)]).toEqual([false, true, ['julius']]);
    view = derived({ options: options({ kind: 'authors', query: 'ju', authors: [person('julius'), person('juliana')] }) });
    expect([view.authorFresh, view.authorsBusy, (view.authors as { login: string }[]).map(entry => entry.login)]).toEqual([true, false, ['julius', 'juliana']]);
    const sub = contractComponent('pages-prs.contract', 'PrFilterSub');
    const scope = (over: Obj) => ({ sub: 'author', authors: [], authorsBusy: false, labels: [], labelsBusy: false, page: page({ options: options(over) }) });
    expect(sub.drawn('testId="pr-author-empty"', {}, scope({}))).toBe(true);
    expect(sub.drawn('testId="pr-author-empty"', {}, { ...scope({}), authorsBusy: true })).toBe(false);
    const source = (await Bun.file(appPath('pages-prs.contract')).text()).split('\n');
    const said = (marker: string, over: Obj) => value(/^\s*text (\(.+?\)) padding=/.exec(source.find(line => line.includes(marker))!)![1]!, scope(over));
    expect([said('testId="pr-author-empty"', {}), said('testId="pr-author-empty"', { failed: true })]).toEqual(['No authors found', "Couldn't search authors on the host"]);
    expect([said('testId="pr-label-empty"', {}), said('testId="pr-label-empty"', { failed: true })]).toEqual(['No labels found', "Couldn't search labels on the host"]);
  });
});

describe("the root's picker search (pull-requests-members.contract)", () => {
  const root = contractComponent('app.contract', 'T3Code');
  const lines = appText('app.contract').split('\n').map(line => line.trim());
  test('a picker opening clears its search; keystrokes settle 250 ms after the last one, never as a command', () => {
    const state: Obj = { prOptionKind: '', prOptionTyped: 'x', prOptionSent: 'x', prQuery: '', prApplied: '', prTyped: false }, sent: unknown[][] = [];
    const scope = { command: (...args: unknown[]) => { sent.push(args); return {}; } };
    // The gated task as the runner arms it: its timer runs while the gate holds, from the commit that last changed its key.
    const at = lines.findIndex(line => line.startsWith('task prOptionSettle when '));
    const [gate = '', key = ''] = lines[at]!.slice('task prOptionSettle when '.length).split(' key=');
    const [, interval = '0', fire = ''] = /^after\((\d+), (\w+)\)$/.exec(lines[at + 1]!) ?? [];
    let armedAt = -1, armedKey: unknown;
    const commit = (now: number) => {
      if (!value(gate, state)) { armedAt = -1; return; }
      const next = value(key, state);
      if (armedAt < 0 || next !== armedKey) { armedAt = now; armedKey = next; }
    };
    const advance = (now: number) => { if (armedAt >= 0 && now >= armedAt + Number(interval)) { armedAt = -1; root.call(fire, state, scope, []); commit(now); } };
    root.call('prControl', state, scope, ['option-kind', 'authors']);
    commit(0);
    expect([state.prOptionKind, state.prOptionTyped, state.prOptionSent, armedAt]).toEqual(['authors', '', '', -1]);
    for (const [now, text] of [[0, 'j'], [100, 'ju'], [200, 'jul']] as const) { advance(now); root.call('prControl', state, scope, ['option-query', text]); commit(now); }
    advance(449);
    expect([state.prOptionTyped, state.prOptionSent]).toEqual(['jul', '']);
    advance(450);
    expect([state.prOptionSent, armedAt]).toEqual(['jul', -1]);
    // The list resource takes the open picker and the search as sent, last; the other controls stay commands.
    expect(lines.find(line => line.startsWith('resource prList = '))).toEndWith('windowReturns, prOptionKind, prOptionSent) as shape PrListView');
    expect(sent).toEqual([]);
    root.call('prControl', state, scope, ['sort', 'newest']);
    expect(sent).toEqual([['pageslocal:pr-sort', '', 'newest', 0]]);
  });
});

describe('the host search over several servers (usePullRequestFilterOptions)', () => {
  test("each server is asked in its own scope; one that fails leaves the others' options and marks the answer failed", async () => {
    const asked: Obj[] = [];
    let wakes = 0;
    const client = { environmentId: 'focus', rpc: async (_native: unknown, method: string, payload: Obj) => {
      asked.push({ method, ...payload });
      if (payload.environmentId === 'down') throw new Error('Unknown request tag: pullRequests.filterOptions');
      return { authors: [{ login: payload.environmentId ? 'from-second' : 'from-focus', name: null, avatarUrl: null }], labels: [], truncated: false };
    } } as unknown as T3Client;
    const native = { available: true, later: async () => { wakes++; return { ok: true }; } } as unknown as Native;
    const targets = filterOptionTargets([{ environmentId: 'focus' }, { environmentId: 'second', projectIds: ['p2'] }, { environmentId: 'down' }], { projectId: '', host: 'github.com' }, 'authors', 'fro');
    expect(await hostedFilterOptions(client, native, 'authors', 'fro', targets, 1_000)).toEqual({ authors: [], labels: [], busy: true, failed: false });
    expect(wakes).toBe(1);
    const answer = await hostedFilterOptions(client, native, 'authors', 'fro', targets, 1_000);
    expect(asked).toEqual([
      { method: 'pullRequests.filterOptions', kind: 'authors', host: 'github.com', query: 'fro' },
      { method: 'pullRequests.filterOptions', kind: 'authors', projectIds: ['p2'], host: 'github.com', query: 'fro', environmentId: 'second' },
      { method: 'pullRequests.filterOptions', kind: 'authors', host: 'github.com', query: 'fro', environmentId: 'down' },
    ]);
    expect([answer.authors.map(author => author.login), answer.busy, answer.failed]).toEqual([['from-focus', 'from-second'], false, true]);
  });
});
