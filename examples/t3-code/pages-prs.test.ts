import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from './contract-action-fixture';
import { contractFn } from './contract-fn-fixture';
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
  expect([view.filterCount, view.filterBadge, view.authorLabel, view.labelsLabel, view.stateIcon, view.draftIcon]).toEqual([4, '4', 'eiiot', '1 selected', 'git-merge', 'eye-off']);
  // The Filters menu is placed by `position-area="bottom span-left"`; no estimated trigger width feeds a tail.
  expect('filtersWidth' in view).toBe(false);
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

// A `button` centres its text (the UA sheet's `text-align: center`, LLP 1001), and the
// macOS host lays an overflowing centred line out centred, cutting its start (issues/closed/20261009-apple-overflow-line-alignment.md; CSS and Chrome start-align
// it). The reference's pull request buttons say `text-left` (PULL_REQUEST_ROW_CLASS, the timeline's
// CollapsibleTrigger, PullRequestCopyableCode), so on every pull request surface a single-line text that can
// overflow (an ellipsis, `line-clamp=1`, or `nowrap` with `overflow="hidden"`) inside a button must resolve
// `text-align` to left: the nearest `text-align` on the way up to the button decides. Read from the Contract
// sources as dialog-focus.test.ts reads its stops; driving the app on macOS shows the
// titles themselves. Since main 24083b9b9 an overflowing line is start-aligned on the Apple hosts, so a text sized by
// its own words (no `flex=1`, `flex-grow` or `width`) has no spare width to centre in and needs no override: the
// base freshness mark and the filter submenu's value lost theirs.
test('every single-line text that can overflow inside a pull request button is start-aligned, as PULL_REQUEST_ROW_CLASS says text-left', async () => {
  const { readdirSync } = await import('node:fs');
  const files = readdirSync(new URL('./', import.meta.url)).filter(name => /^pages-prs?(-.+)?\.contract$/.test(name)).sort();
  const prs = (await Bun.file(new URL('./pages-prs.contract', import.meta.url)).text()).split('\n');
  const actions = (await Bun.file(new URL('./pages-pr-actions.contract', import.meta.url)).text()).split('\n');
  const value = prs.findIndex(line => /^\s*text value font-size="0.75rem"/.test(line));
  const base = actions.findIndex(line => /^\s*text base font-family="ui-monospace"[^\n]*color=paWarn\(\)/.test(line));
  const exempt = new Set([`pages-prs.contract:${value + 1}`, `pages-pr-actions.contract:${base + 1}`]);
  const offenders: string[] = [], sized: string[] = [];
  let checked = 0;
  const align = (line: string) => /\btext-align="([a-z-]+)"/.exec(line)?.[1] ?? '';
  const overflows = (line: string) => /\btext-overflow="ellipsis"/.test(line) || /\bline-clamp=1\b/.test(line)
    || (/\bwhite-space="nowrap"/.test(line) && /\boverflow="hidden"/.test(line));
  for (const file of files) {
    const lines = (await Bun.file(new URL(`./${file}`, import.meta.url)).text()).split('\n');
    const indent = (line: string) => line.length - line.trimStart().length;
    lines.forEach((line, index) => {
      if (!/^\s*text\b/.test(line) || !overflows(line)) return;
      // Only the two `exempt` lines, each sized by its words in a row, pass; any other overflowing text still resolves left.
      if (exempt.has(`${file}:${index + 1}`)) { sized.push(`${file}:${index + 1}`); return; }
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
  expect(checked).toBeGreaterThanOrEqual(13);
  expect(sized).toHaveLength(2);
  expect(offenders).toEqual([]);
  // The two exempt sites are among the text-sized lines and carry no alignment of their own.
  expect(sized.sort()).toEqual([...exempt].sort());
  for (const line of [prs[value]!, actions[base]!]) {
    expect(line).not.toContain('text-align=');
    expect(line).toContain('min-width=0');
    expect(line).toContain('white-space="nowrap" overflow="hidden" text-overflow="ellipsis"');
  }
  expect(prs[value]).toContain('max-width="8rem"');
  // Its label is the reference's `flex-1` span, which keeps its words' width (min-width auto), so a long value such as
  // "Changes requested" is the one cut, never drawn over the label.
  const label = prs[value - 2]!;
  expect(label).toMatch(/^\s*text label [^\n]*flex=1 /);
  expect(label).not.toContain('min-width=0');
});


describe('PR menu public opening and nested Filters focus', () => {
  const plainKey = { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, repeat: false };
  const candidates = [{ id: 'first-row', label: 'First' }, { id: 'last-row', label: 'Last' }];
  const openers: [string, string, string, string, string, string][] = [
    ['pages-prs.contract', 'PrSortMenu', 'triggerKey', 'pr-sort-menu', 'pull-requests-sort', 'menuItems'],
    ['pages-prs.contract', 'PrHostMenu', 'triggerKey', 'pr-host-menu', 'pull-requests-provider', 'menuItems'],
    ['pages-prs.contract', 'PrFiltersMenu', 'triggerKey', 'pr-filters-menu', 'pull-requests-filters', 'filterItems'],
    ['pages-pr-actions.contract', 'PrdActionsMenu', 'moreKey', 'pr-more-menu', 'pull-request-more', 'rows'],
    ['pages-pr-code.contract', 'PrdCodeToolbar', 'triggerKey', 'scope-menu', 'pull-request-code-scope', 'menuItems'],
    ['pages-pr-handoffs.contract', 'PrdCheckoutMenu', 'triggerKey', 'pr-checkout-menu', 'pull-request-checkout', 'menuItems'],
    ['pages-pr-quick.contract', 'PrRowStack', 'triggerKey', 'row-stack', 'row-stack-trigger', 'layerItems'],
    ['pages-pr-stack.contract', 'PrdStackControls', 'triggerKey', 'pr-stack-menu', 'pull-request-stack', 'stackItems'],
  ];

  test.each(openers)('%s %s opens at the real endpoint once, with its own source', async (file, name, action, popup, trigger, sequence) => {
    const kmOpenKey = await contractFn('menu-keys.contract', 'kmOpenKey', ['k']);
    const kmArrow = await contractFn('menu-keys.contract', 'kmArrow', ['k']);
    const kmTriggerKey = await contractFn('menu-keys.contract', 'kmTriggerKey', ['k'], { kmOpenKey, kmArrow });
    const kmTriggerTarget = await contractFn('menu-keys.contract', 'kmTriggerTarget', ['items', 'k']);
    const component = contractComponent(file, name), calls: unknown[][] = [];
    const state = component.state();
    const scope: Env = {
      kmTriggerKey, kmTriggerTarget, [sequence]: candidates, menuId: 'scope-menu', popId: 'row-stack', rowRef: 'p#7',
      preventDefault: () => calls.push(['prevent']), stopPropagation: () => calls.push(['stop']),
      showPopover: (...args: unknown[]) => calls.push(['show', ...args]), focus: (id: string) => calls.push(['focus', id]),
      control: (...args: unknown[]) => calls.push(['prepare', ...args]),
      openMenu: () => { calls.push(['prepare']); component.call('openMenu', state, {}, []); },
    };
    for (const key of ['ArrowDown', 'ArrowUp', 'Enter', ' ', 'Space']) {
      calls.length = 0;
      if (name === 'PrFiltersMenu') { state.sub = 'author'; state.authorQuery = 'old'; }
      component.call(action, state, scope, [key, plainKey]);
      const prep = name === 'PrFiltersMenu' ? [['prepare']] : name === 'PrRowStack' ? [['prepare', 'quick-stack', 'p#7']] : [];
      expect(calls).toEqual([['prevent'], ['stop'], ...prep, ['show', popup, { source: trigger }], ['focus', key === 'ArrowUp' ? 'last-row' : 'first-row']]);
      if (name === 'PrFiltersMenu') expect([state.sub, state.authorQuery]).toEqual(['', '']);
      calls.length = 0;
      component.call(action, state, scope, [key, { ...plainKey, repeat: true }]);
      expect(calls).toEqual([['prevent'], ['stop']]);
    }
    for (const modifier of ['metaKey', 'ctrlKey', 'altKey', 'shiftKey']) {
      calls.length = 0;
      component.call(action, state, scope, ['ArrowDown', { ...plainKey, [modifier]: true }]);
      expect(calls).toEqual([]);
    }
    calls.length = 0;
    component.call(action, state, { ...scope, [sequence]: [] }, ['ArrowDown', plainKey]);
    expect(calls.filter(([kind]) => kind === 'show')).toEqual([['show', popup, { source: trigger }]]);
    expect(calls.some(([kind]) => kind === 'focus')).toBe(false);
    const text = await Bun.file(new URL(`./${file}`, import.meta.url)).text();
    expect(text).toContain(`items=${sequence}, keyed=0,`);
    expect(text).not.toContain('KeyMenuOpen');
    expect(text).not.toMatch(/focus\([^\n]*-km["`]/);
  });

  test('menu sequences use rendered eligible rows, including unavailable providers and pending actions', () => {
    const KmItem = (fields: { id: string; label: string }) => fields;
    const derive = (file: string, name: string, scope: Env, sequence: string) => {
      const component = contractComponent(file, name);
      return component.derives(component.state(), { KmItem, ...scope })[sequence];
    };
    expect(derive('pages-prs.contract', 'PrSortMenu', {}, 'menuItems')).toEqual([
      { id: 'pr-sort-ready', label: 'Merge readiness' }, { id: 'pr-sort-blocked', label: 'Blocked on me' },
      { id: 'pr-sort-updated', label: 'Recently updated' }, { id: 'pr-sort-newest', label: 'Newest shown' },
      { id: 'pr-sort-oldest', label: 'Oldest shown' }, { id: 'pr-sort-largest', label: 'Largest shown' }, { id: 'pr-sort-smallest', label: 'Smallest shown' },
    ]);
    expect(derive('pages-prs.contract', 'PrHostMenu', { page: { hosts: [
      { label: 'All', unavailable: '' }, { label: 'GitHub', unavailable: '' }, { label: 'Unavailable', unavailable: 'offline' },
    ] } }, 'menuItems')).toEqual([{ id: 'pr-host-All', label: 'All' }, { id: 'pr-host-GitHub', label: 'GitHub' }]);
    const checkout = { handoffs: { pending: '', canCheckout: true }, actOn: [{ key: 'secondary', label: 'Secondary' }] };
    expect(derive('pages-pr-handoffs.contract', 'PrdCheckoutMenu', checkout, 'menuItems')).toEqual([
      { id: 'pr-checkout-worktree', label: 'In a separate worktree' }, { id: 'pr-checkout-local', label: 'In this repository' },
      { id: 'pr-act-on-secondary', label: 'Secondary' },
    ]);
    expect(derive('pages-pr-handoffs.contract', 'PrdCheckoutMenu', { ...checkout, handoffs: { pending: 'checkout', canCheckout: true } }, 'menuItems')).toEqual([]);
    expect(derive('pages-pr-code.contract', 'PrdCodeToolbar', { code: { commits: [{ short: 'a123', headline: 'Fix cache' }], moreCommits: 1 } }, 'menuItems')).toEqual([
      { id: 'pull-request-code-scope-all', label: 'All commits' }, { id: 'pull-request-code-scope-a123', label: 'Fix cache' },
      { id: 'pull-request-code-scope-more', label: 'Show more' },
    ]);
    const quick = { rowRef: 'p#7', popId: 'row-stack', popover: { ref: 'p#7', retry: true, layers: [{ number: 8, title: 'Next PR' }] } };
    expect(derive('pages-pr-quick.contract', 'PrRowStack', quick, 'layerItems')).toEqual([
      { id: 'row-stack-retry', label: 'Retry stack refresh' }, { id: 'row-stack-layer-8', label: 'Next PR' },
    ]);
    expect(derive('pages-pr-quick.contract', 'PrRowStack', { ...quick, rowRef: 'p#9' }, 'layerItems')).toEqual([]);
    expect(derive('pages-pr-stack.contract', 'PrdStackControls', { stack: {
      retry: false, selectable: true, layers: [{ number: 8, title: 'Next PR' }], canMerge: true, mergeDisabled: true,
      mergeItemLabel: 'Merge stack', canRebase: true, rebaseDisabled: false,
    } }, 'stackItems')).toEqual([{ id: 'pr-stack-layer-8', label: 'Next PR' }, { id: 'pr-stack-rebase', label: 'Rebase stack' }]);
  });

  test('first Escape consumes only the child; pointer exit blurs, and the parent close returns Filters focus', () => {
    const menu = contractComponent('pages-prs.contract', 'PrFiltersMenu');
    const search = contractComponent('pages-prs.contract', 'PrFilterSub');
    const state = menu.state(), calls: unknown[][] = [];
    const scope: Env = {
      preventDefault: () => calls.push(['prevent']), stopPropagation: () => calls.push(['stop']),
      focus: (id: string) => calls.push(['focus', id]), blur: () => calls.push(['blur']),
    };
    scope.closeSub = () => menu.call('closeSub', state, scope, []);
    scope.searchEscape = scope.closeSub;
    for (const throughSearch of [false, true]) {
      for (const pointerLeft of [false, true]) {
        state.sub = 'author'; state.subLeft = false; calls.length = 0;
        if (pointerLeft) menu.call('rowLeft', state, scope, ['author']);
        if (throughSearch) search.call('searchKey', {}, scope, ['Escape']);
        else menu.call('subEscape', state, scope, ['Escape']);
        expect(state.sub).toBe('');
        expect(calls).toEqual([['prevent'], ['stop'], pointerLeft ? ['blur'] : ['focus', 'pr-filter-author']]);
        calls.length = 0;
        menu.call('subEscape', state, scope, ['Escape']);
        expect(calls).toEqual([]); // The next Escape is left to the host's parent popover dismissal.
        menu.call('menuChanged', state, scope, [{ oldState: 'open', newState: 'closed' }]);
        expect(calls).toEqual([['focus', 'pull-requests-filters']]);
      }
    }
    calls.length = 0;
    menu.call('menuChanged', state, scope, [{ oldState: 'closed', newState: 'open' }]);
    expect(calls).toEqual([]);
    state.sub = 'author'; state.subLeft = true;
    menu.call('subBack', state, scope, ['ArrowLeft']);
    expect(state.sub).toBe('');
    expect(calls).toEqual([['prevent'], ['focus', 'pr-filter-author']]);
  });

  test('Filters hover and keyboard return focus actual rows and preserve the next row', async () => {
    const kmIndex = await contractFn('menu-keys.contract', 'kmIndex', ['items', 'current']);
    const kmAfter = await contractFn('menu-keys.contract', 'kmAfter', ['items', 'current'], { kmIndex });
    const kmLetter = await contractFn('menu-keys.contract', 'kmLetter', ['k']);
    const first = (items: unknown[]) => items[0];
    const kmTarget = await contractFn('menu-keys.contract', 'kmTarget', ['items', 'current', 'k'], { kmIndex, kmAfter, kmLetter, first, none: undefined });
    const menu = contractComponent('pages-prs.contract', 'PrFiltersMenu');
    const row = contractComponent('pages-prs.contract', 'PrSubTrigger');
    const state = menu.state(), rowState = row.state(), calls: unknown[][] = [];
    const items = [{ id: 'pr-filter-author', label: 'Author' }, { id: 'pr-filter-labels', label: 'Labels' }];
    const scope: Env = {
      kmTarget, kmOpenKey: () => false, filterItems: items, testId: 'pr-filter-author', keysId: 'pr-filters-keys',
      preventDefault: () => calls.push(['prevent']), stopPropagation: () => calls.push(['stop']),
      focus: (id: string) => calls.push(['focus', id]), leave: () => calls.push(['leave']),
    };
    row.call('hover', rowState, scope, [true]);
    expect(calls).toEqual([['focus', 'pr-filter-author']]);
    calls.length = 0;
    menu.call('subKey', state, scope, ['author', 'ArrowDown', plainKey]);
    expect(calls).toEqual([['prevent'], ['stop'], ['focus', 'pr-filter-labels']]);
    calls.length = 0;
    row.call('focused', rowState, scope, [true]);
    row.call('hover', rowState, scope, [false]);
    expect(calls).toEqual([['focus', 'pr-filters-keys'], ['leave']]);
    const text = await Bun.file(new URL('./pages-prs.contract', import.meta.url)).text();
    expect(text).not.toContain('pr-filters-rest');
    expect(text).toContain('popover="auto" toggle=menuChanged');
  });
});
