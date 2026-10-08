import { expect, test } from 'bun:test';
import { parsePrQuery, groupEntries, rankByMergeReadiness, sortGroups, presentRow, presentList, emptyList, listPayload, prPrefs, prLocal, labelChip, defaultPrPrefs, rowRef } from './pages-prs';
import { presentDetail, emptyDetail, summarizeChecks, checksTone, parseSelection } from './pages-pr-detail';
import { adoptPagesPrefs } from './pages-prefs';
import type { Obj } from './domain';

const entry = (number: number, extra: Obj = {}): Obj => ({
  provider: 'github', host: 'github.com', projectId: 'p1', projectTitle: 'Parity', repository: 'acme/app', number, title: `Change ${number}`,
  url: `https://github.com/acme/app/pull/${number}`, author: { login: 'eiiot', name: null, avatarUrl: 'https://avatars.example/e.png' },
  headBranch: `fix/${number}`, baseBranch: 'main', state: 'open', isDraft: false, mergeability: 'mergeable', additions: 5, deletions: 2,
  createdAt: '2026-10-01T00:00:00.000Z', updatedAt: '2026-10-02T00:00:00.000Z', viewerReviewRequested: false, labels: [], ...extra,
});

test('the search qualifies labels, author, draft, review and checks and leaves the rest as text', () => {
  expect(parsePrQuery('fix label:bug,"needs review" author:me draft:true review:approved checks:failing crash')).toEqual({
    text: 'fix crash', filters: { labels: [['bug', 'needs review']], author: 'me', draft: 'only', review: 'approved', checks: 'failing' } });
  expect(parsePrQuery('-label:wip area:ui').filters).toEqual({ labels: [['area:ui']], excludedLabels: ['wip'] });
});

test('rows group as the reference groups them and rank by merge readiness', () => {
  const viewers = { 'github.com': 'eiiot' };
  const groups = groupEntries([entry(1), entry(2, { author: { login: 'other', name: null, avatarUrl: null }, viewerReviewRequested: true }), entry(3, { author: { login: 'x', name: null, avatarUrl: null } })], viewers);
  expect(groups.map(group => [group.key, group.entries.map(e => e.number)])).toEqual([['authored', [1]], ['reviewRequested', [2]], ['others', [3]]]);
  const ranked = rankByMergeReadiness([entry(1, { mergeability: 'conflicting' }), entry(2, { checksState: 'passing', reviewDecision: 'approved' }), entry(3, { isDraft: true }), entry(4, { checksState: 'passing' })]);
  expect(ranked.map(e => e.number)).toEqual([2, 4, 3, 1]);
  const sorted = sortGroups([{ key: 'others', label: 'Others', entries: [entry(1, { additions: 50 }), entry(2, { additions: 1 })] }], 'largest', '', 'all');
  expect(sorted[0]!.entries.map(e => e.number)).toEqual([1, 2]);
});

test('a row carries its address, state, conflict, checks and label chips', () => {
  const row = presentRow(entry(60, { mergeability: 'conflicting', checksState: 'passing', labels: [{ name: 'bug', color: 'd73a4a' }, { name: 'ui', color: null }] }), Date.parse('2026-10-06T00:00:00.000Z'), '');
  expect(row).toMatchObject({ number: 60, state: 'open', conflict: 'Conflicts with main', checks: 'passing', checksLabel: 'Checks: All checks have passed', additions: '+5', deletions: '-2', age: '4d ago', author: 'eiiot', moreLabels: 1 });
  expect(parseSelection(row.ref)).toEqual({ projectId: 'p1', host: 'github.com', repository: 'acme/app', number: 60 });
  expect(row.labels[0]).toEqual({ key: 'bug', name: 'bug', background: 'light-dark(#d73a4a14, #d73a4a1f)', ink: 'light-dark(#5b2c31, #e8a1a8)' });
  expect(row.labels[1]!.background).toBe('light-dark(#f4f4f5, #ffffff0f)');
  expect(presentRow(entry(60), 0, row.ref).selected).toBe(true);
  expect(labelChip('x', '').ink).toBe('light-dark(#27272a, #f5f5f5)');
});

test('the list view answers groups, facets, hosts and its empty states', () => {
  const prefs = defaultPrPrefs();
  const result = { viewers: { 'github.com': 'nobody' }, providers: [{ host: 'github.com', kind: 'github', configured: true, detail: null }], entries: [entry(1, { labels: [{ name: 'bug', color: 'd73a4a' }] }), entry(2, { state: 'merged' })], errors: [], truncated: false };
  const view = presentList(emptyList(prefs, ''), result, '', new Map(), prefs, '', Date.parse('2026-10-03T00:00:00.000Z'), '');
  expect(view.groups.map(group => [group.label, group.count])).toEqual([['Others', 1]]);
  expect(view.hosts.map(host => host.label)).toEqual(['All', 'GitHub']);
  expect(view.labelFacets).toEqual([{ key: 'bug', name: 'bug', color: '#d73a4a', count: 1, selected: false }]);
  expect(view.authors[0]).toMatchObject({ login: 'eiiot', detail: '1 merges loaded' });
  const none = presentList(emptyList(prefs, 'zzz'), { ...result, entries: [] }, '', new Map(), prefs, 'zzz', 0, '');
  expect([none.empty, none.emptyAction]).toEqual(['Nothing matches “zzz”', 'clear']);
  const failed = presentList(emptyList(prefs, ''), null, 'gh is not signed in', new Map(), prefs, '', 0, '');
  expect([failed.empty, failed.emptyDetail, failed.emptyAction]).toEqual(['Could not load pull requests', 'gh is not signed in', 'refresh']);
});

test('the list controls persist in the preference record and narrow the request', () => {
  const owner = { local: {} as Record<string, unknown> };
  prLocal(owner, 'sort', 'largest'); prLocal(owner, 'state', 'merged'); prLocal(owner, 'label', 'bug'); prLocal(owner, 'author', 'eiiot'); prLocal(owner, 'draft', 'hide');
  const reloaded = { local: {} as Record<string, unknown> };
  adoptPagesPrefs(reloaded.local, JSON.parse(JSON.stringify(owner.local)));
  const prefs = prPrefs(reloaded);
  expect(prefs).toMatchObject({ sort: 'largest', state: 'merged', labels: ['bug'], author: 'eiiot', draft: 'hide' });
  expect(listPayload(prefs, 'crash review:approved label:ui')).toEqual({ state: 'merged', involvement: 'all', limit: 99, query: 'crash',
    filters: { review: 'approved', draft: 'hide', author: 'eiiot', labels: [['ui'], ['bug']] } });
  const view = emptyList(prefs, '');
  expect([view.filterCount, view.filterBadge, view.filtersWidth, view.authorLabel, view.labelsLabel, view.stateIcon, view.draftIcon]).toEqual([4, '4', 110.8, 'eiiot', '1 selected', 'git-merge', 'eye-off']);
  prLocal(owner, 'author', 'EIIOT');
  expect(prPrefs(owner).author).toBe('');
  expect(() => prLocal(owner, 'colour', 'x')).toThrow('Unknown pull request control');
});

test('the detail offers only what the host can do and this viewer may ask', () => {
  const detail = { ...entry(30, { isDraft: true, mergeability: 'conflicting' }), body: '# Why', changedFiles: 4, additions: 331, deletions: 7, checks: [{ name: 'ci', status: 'success', description: null, url: null }],
    capabilities: { actions: ['ready', 'close', 'merge'], reviewers: { request: true, listCandidates: true }, labels: true, edit: { changeRequest: true, comment: true } },
    viewerPermissions: { actions: ['close'], comment: true, resolve: true, verdicts: [], requestReviewers: false }, reviewers: [], labels: [] };
  const view = presentDetail(emptyDetail(), detail, { comments: [{ id: 'c1', kind: 'comment', author: null, body: 'Looks good', createdAt: '2026-10-02T00:00:00.000Z' }], commentCount: 1, commits: [] }, Date.parse('2026-10-13T00:00:00.000Z'));
  expect(view).toMatchObject({ numberLabel: '#30', state: 'draft', stateLabel: 'Draft', conflict: '', checkoutCommand: 'gh pr checkout 30', files: '4 files', additions: '+331', deletions: '-7',
    checksSummary: 'All checks passed', checksTone: 'passing', updated: 'updated 11d ago', canReady: false, canClose: true, canReview: false, canLabel: true, commentsLabel: 'Comments (1)', hostName: 'GitHub' });
  expect(view.bodies.map(body => body.id)).toEqual(['pr-body:acme/app#30', 'pr-comment:c1']);
  expect(summarizeChecks([{ status: 'failure' }, { status: 'success' }])).toBe('1 of 2 failing');
  expect(checksTone([{ status: 'pending' }])).toBe('pending');
  expect(rowRef(entry(7))).toBe('{"projectId":"p1","host":"github.com","repository":"acme/app","number":7}');
});

test('one Refresh press invalidates once, though the announcement it causes asks the list again while it reads', async () => {
  const { pullRequestsPage } = await import('./pages-prs');
  const calls: string[] = [];
  let release: () => void = () => {};
  const gate = new Promise<void>(resolve => { release = resolve; });
  const client = {
    environmentId: 'env', ready: true, generation: 1, revision: 0, local: {}, config: { environment: { capabilities: { pullRequests: true } } },
    shell: { projects: [{ id: 'p1', title: 'Parity' }], threads: [] },
    restAccess: () => ({ call: async () => ({ id: 'sub-1' }) }),
    rpc: async (_native: unknown, method: string) => { calls.push(method); if (method === 'pullRequests.list' && calls.filter(c => c === 'pullRequests.list').length > 1) await gate; return method === 'pullRequests.list' ? { entries: [] } : {}; },
  } as unknown as Parameters<typeof pullRequestsPage>[0];
  const native = { available: true, watch: () => {}, later: async () => ({ ok: true }) } as unknown as Parameters<typeof pullRequestsPage>[1];
  const input = { open: true, now: 0, selected: '', query: '', typed: false };
  await pullRequestsPage(client, native, { ...input, refresh: 0 });
  // The press, then two more asks while its read is out (each announcement bumps the revision).
  const reads = [pullRequestsPage(client, native, { ...input, refresh: 1 }), pullRequestsPage(client, native, { ...input, refresh: 1 }), pullRequestsPage(client, native, { ...input, refresh: 1 })];
  await Bun.sleep(5);
  release();
  await Promise.all(reads);
  expect(calls.filter(c => c === 'pullRequests.invalidate').length).toBe(1);
});

// pr-list-title-clip: a `button` centres its text (the UA sheet's `text-align: center`, LLP 1001), and the
// macOS host lays an overflowing centred line out centred, cutting its start (X57; CSS and Chrome start-align
// it). The reference's pull request buttons say `text-left` (PULL_REQUEST_ROW_CLASS, the timeline's
// CollapsibleTrigger, PullRequestCopyableCode), so on every pull request surface a single-line text that can
// overflow (an ellipsis, `line-clamp=1`, or `nowrap` with `overflow="hidden"`) inside a button must resolve
// `text-align` to left: the nearest `text-align` on the way up to the button decides. Read from the Contract
// sources as dialog-focus.test.ts reads its stops; the live drive in tasks/20261008-pr-list-title-clip.md is
// the proof.
test('every single-line text that can overflow inside a pull request button is start-aligned, as PULL_REQUEST_ROW_CLASS says text-left', async () => {
  const { readdirSync } = await import('node:fs');
  const files = readdirSync(new URL('./', import.meta.url)).filter(name => /^pages-prs?(-.+)?\.contract$/.test(name)).sort();
  const offenders: string[] = [];
  let checked = 0;
  const align = (line: string) => /\btext-align="([a-z-]+)"/.exec(line)?.[1] ?? '';
  const overflows = (line: string) => /\btext-overflow="ellipsis"/.test(line) || /\bline-clamp=1\b/.test(line)
    || (/\bwhite-space="nowrap"/.test(line) && /\boverflow="hidden"/.test(line));
  for (const file of files) {
    const lines = (await Bun.file(new URL(`./${file}`, import.meta.url)).text()).split('\n');
    const indent = (line: string) => line.length - line.trimStart().length;
    lines.forEach((line, index) => {
      if (!/^\s*text\b/.test(line) || !overflows(line)) return;
      // The ancestors, innermost first, up to the component's `view`: the first `text-align` met (the text's
      // own, an ancestor's, or the button's own) wins; a button with none centres.
      let depth = indent(line), decided = align(line), inButton = false;
      for (let up = index - 1; up >= 0 && !inButton && depth > 0; up--) {
        const above = lines[up]!;
        if (!above.trim() || above.trimStart().startsWith('//') || indent(above) >= depth) continue;
        depth = indent(above);
        if (/^\s*(view|component)\b/.test(above)) break;
        decided ||= align(above);
        inButton = /^\s*button\b/.test(above);
      }
      if (!inButton) return;
      checked++;
      if (!['left', 'start'].includes(decided)) offenders.push(`${file}:${index + 1}`);
    });
  }
  // The list row's title, author, repository and labels; the timeline group's authors; the base freshness mark;
  // the copyable branch and checkout command; the menus' and pickers' rows.
  expect(checked).toBeGreaterThanOrEqual(15);
  expect(offenders).toEqual([]);
});
