// T3 Code 7c3b3b3605ef packages/shared/src/paletteSearch.test.ts (MIT, see LICENSE-T3): the client's cases. The
// server-only messageMatcher and recordThreadOpen cases are the embedded server's.
import { describe, expect, test } from 'bun:test';
import {
  claimThreadOpen, findSearchTokenRanges, foldSearchQuery, nextShownOrder, pickedQuery, rankMessageMatches, rankSearchGroups, rankSearchItems,
  scoreSearchTerms, scoreThreadSearch, threadRecentUse, threadSearchPullRequests, threadSearchSubject, type SearchTermsMatch, type ThreadSearchSubject,
} from '../../src/palette/palette-search';

const NOW = Date.UTC(2026, 9, 23);
const DAY = 24 * 60 * 60 * 1000;
const thread = (title: string, overrides: Partial<ThreadSearchSubject> = {}): ThreadSearchSubject => ({ title, activityAt: NOW - 30 * DAY, ...overrides });
function rankTitles(query: string, threads: ReadonlyArray<ThreadSearchSubject>): string[] {
  const normalized = foldSearchQuery(query);
  return rankSearchItems(threads, (subject, options) => scoreThreadSearch(normalized, subject, NOW, options)).map(({ item }) => item.title);
}

describe('scoreThreadSearch', () => {
  test('ranks whole words over prefixes over newer mid-word matches', () => {
    expect(rankTitles('auth', [thread('Fix oauth redirect', { activityAt: NOW }), thread('Authentication cleanup'), thread('Share auth across worktrees')]))
      .toEqual(['Share auth across worktrees', 'Authentication cleanup', 'Fix oauth redirect']);
  });
  test('treats camelCase, kebab, path, and digit boundaries as word starts', () => {
    const starts = (title: string, query: string) => scoreThreadSearch(query, thread(title), NOW)?.ranges;
    expect(starts('useThreadSearch hook', 'search')).toEqual([{ start: 9, end: 15 }]);
    expect(starts('fix/sidebar-perf', 'perf')).toEqual([{ start: 12, end: 16 }]);
    expect(starts('Upgrade to pnpm12', '12')).toEqual([{ start: 15, end: 17 }]);
    expect(starts('Fix JSONSchema parsing', 'sch')).toEqual([{ start: 8, end: 11 }]);
    expect(starts('Fix HTTPServer crash', 'ser')).toEqual([{ start: 8, end: 11 }]);
    expect(starts('Rename index.ts', '.ts')).toEqual([{ start: 12, end: 15 }]);
    // A match is whole only where a word ends: "githuba" does not fill "GitHubActions".
    expect(scoreThreadSearch('githuba', thread('GitHubActions setup'), NOW)?.quality).toBeLessThan(1);
    expect(rankTitles('.ts', [thread('Fix parser.tsx'), thread('Rename index.ts')])).toEqual(['Rename index.ts', 'Fix parser.tsx']);
    // Short tokens never match mid-word.
    expect(scoreThreadSearch('ci', thread('Decision log'), NOW)).toBeNull();
  });
  test('lets recent activity and frequent opens decide between equal matches', () => {
    expect(rankTitles('goldbug', [thread('Goldbug epilogue', { activityAt: NOW - 20 * DAY }), thread('Goldbug processes', { activityAt: NOW - DAY })]))
      .toEqual(['Goldbug processes', 'Goldbug epilogue']);
    expect(rankTitles('goldbug', [thread('Goldbug processes', { activityAt: NOW - DAY }),
      thread('Goldbug notes', { activityAt: NOW - 40 * DAY, opens: { openedAt: NOW - DAY, score: 12, picks: [] } })])).toEqual(['Goldbug notes', 'Goldbug processes']);
  });
  test('puts the thread picked for a query first when that query is typed again', () => {
    const threads = [thread('Release checklist', { opens: { openedAt: NOW - 5 * DAY, score: 1, picks: [{ query: 'rel', at: NOW - 5 * DAY }] } }), thread('Relay pairing', { activityAt: NOW })];
    expect(rankTitles('rel', threads)).toEqual(['Release checklist', 'Relay pairing']);
    // A pick never lifts a match inside a word above a word match.
    expect(rankTitles('auth', [thread('Fix oauth redirect', { opens: { openedAt: NOW - DAY, score: 1, picks: [{ query: 'auth', at: NOW - DAY }] } }), thread('Share auth across worktrees')]))
      .toEqual(['Share auth across worktrees', 'Fix oauth redirect']);
    // Two letters are too little to go on; once the query leaves the pick behind, the better match wins again.
    expect(rankTitles('re', threads)).toEqual(['Relay pairing', 'Release checklist']);
    expect(rankTitles('rela', threads)).toEqual(['Relay pairing']);
    // An old pick fades with the thread's last open.
    expect(rankTitles('rel', [thread('Release checklist', { opens: { openedAt: NOW - 90 * DAY, score: 1, picks: [{ query: 'rel', at: NOW - 90 * DAY }] } }), thread('Relay pairing', { activityAt: NOW })]))
      .toEqual(['Relay pairing', 'Release checklist']);
    // Opening an old pick some other way does not revive it over a newer pick.
    expect(rankTitles('rel', [thread('Release checklist', { opens: { openedAt: NOW - DAY, score: 2, picks: [{ query: 'rel', at: NOW - 30 * DAY }] } }),
      thread('Relay pairing', { opens: { openedAt: NOW - DAY, score: 1, picks: [{ query: 'rel', at: NOW - DAY }] } })])).toEqual(['Relay pairing', 'Release checklist']);
    // The latest pick for a query wins over an earlier one with a better title match.
    expect(rankTitles('deploy', [thread('Deploy', { opens: { openedAt: NOW - DAY, score: 1, picks: [{ query: 'deploy', at: NOW - DAY }] } }),
      thread('Deploy timeout on Railway', { opens: { openedAt: NOW, score: 1, picks: [{ query: 'deploy', at: NOW }] } })])).toEqual(['Deploy timeout on Railway', 'Deploy']);
    // A thread's exact pick counts even after a newer pick for a longer query.
    expect(rankTitles('fix', [thread('Fix browser', { opens: { openedAt: NOW, score: 2, picks: [{ query: 'fix browser', at: NOW }, { query: 'fix', at: NOW - 2 * DAY }] } }),
      thread('Fix sidebar', { opens: { openedAt: NOW - DAY, score: 1, picks: [{ query: 'fix', at: NOW - DAY }] } })])).toEqual(['Fix sidebar', 'Fix browser']);
    // A pick for a longer query does not pass the thread named by the query.
    expect(rankTitles('lakebed', [thread('Lakebed perf audit', { opens: { openedAt: NOW, score: 1, picks: [{ query: 'lakebed perf', at: NOW }] } }), thread('Lakebed')]))
      .toEqual(['Lakebed', 'Lakebed perf audit']);
  });
  test("finds a branch pasted with the app's worktree prefix", () => {
    const subject = thread('Cmd+K search', { branch: 't3/cmdk-search-v2' });
    expect(scoreThreadSearch(foldSearchQuery('t3/cmdk-search-v2'), subject, NOW)).not.toBeNull();
    expect(scoreThreadSearch(foldSearchQuery('t3/cmdk'), subject, NOW)).not.toBeNull();
  });
  test("ranks a title holding the words above the app's own branch prefix and project name", () => {
    const busy = { activityAt: NOW, opens: { openedAt: NOW, score: 6, picks: [] } };
    expect(rankTitles('t3 connect', [thread('Diagnose Connect provisioning failures', { ...busy, branch: 't3/diagnose-connect', projectTitle: 't3code' }),
      thread('T3 Connect Tunnel Lifecycle Review', { projectTitle: 't3code' })])[0]).toBe('T3 Connect Tunnel Lifecycle Review');
    expect(rankTitles('t3code', [thread('Fix sidebar lag', { ...busy, projectTitle: 't3code', branch: 't3code/sidebar' }),
      thread('T3 Code vs Gemini Token Volume', { projectTitle: 'slophouse' })])[0]).toBe('T3 Code vs Gemini Token Volume');
  });
  test('matches pull request numbers, branches, and projects with every token somewhere', () => {
    const subject = thread('Trim model lists', { branch: 't3/trim-mcp-models', projectTitle: 't3code', pullRequests: [{ number: 22889, title: 'Smaller MCP payloads' }] });
    for (const query of ['#22889', '22889', 'trim mcp', 'payloads t3code', 'models trim']) expect(scoreThreadSearch(foldSearchQuery(query), subject, NOW)).not.toBeNull();
    expect(scoreThreadSearch('trim lakebed', subject, NOW)).toBeNull();
  });
  test('forgives one typo only when nothing matches exactly', () => {
    const threads = [thread('Hosting alternatives to Railway'), thread('Trim MCP model lists')];
    expect(rankTitles('railwya', threads)).toEqual(['Hosting alternatives to Railway']);
    expect(rankTitles('trim mpc', threads)).toEqual(['Trim MCP model lists']);
    expect(rankTitles('drga', [thread('Restore drag handles'), thread('Organize indicators')])).toEqual(['Restore drag handles']);
    expect(rankTitles('22889', [thread('Stats', { pullRequests: [{ number: 22189 }] })])).toEqual([]);
    // Words with digits still forgive one.
    expect(rankTitles('t3coed', [thread('t3code perf audit'), thread('Other')])).toEqual(['t3code perf audit']);
    expect(rankTitles('oath2', [thread('Fix oauth2 redirect')])).toEqual(['Fix oauth2 redirect']);
    // A typo beats a mid-word accident: "ilde" is "idle", not the middle of "Builder".
    expect(rankTitles('ilde', [thread('Typesafe App Builder'), thread('De-risk idle tunnel cleanup')])).toEqual(['De-risk idle tunnel cleanup', 'Typesafe App Builder']);
    // "ing" is not "Nightly": a three-letter typo keeps the first letter.
    expect(rankTitles('ing', [thread('Nightly build fix')])).toEqual([]);
    // A short word may miss one letter while it is typed, but not change one.
    expect(rankTitles('srv', [thread('Server restart'), thread('Other')])).toEqual(['Server restart']);
    expect(rankTitles('sidb', [thread('Fix sidebar lag')])).toEqual(['Fix sidebar lag']);
    expect(rankTitles('cron', [thread('Config cleanup')])).toEqual([]);
    // Filler words in a phrase-style query are not required, but alone they are.
    expect(rankTitles('fix the sidebar', [thread('Fix Sidebar Lag')])).toEqual(['Fix Sidebar Lag']);
    expect(rankTitles('the', [thread('Fix sidebar lag')])).toEqual([]);
    // A typed straight apostrophe finds a curly one.
    expect(rankTitles("don't", [thread('Don’t merge yet')])).toEqual(['Don’t merge yet']);
    // Typos count across camelCase joins.
    expect(rankTitles('githbu', [thread('GitHub API usage'), thread('Other')])).toEqual(['GitHub API usage']);
    expect(rankTitles('macso', [thread('Fix macOS window'), thread('Other')])).toEqual(['Fix macOS window']);
    // "enter" is not "external" when the right thread is already there.
    expect(rankTitles('cmd enter', [thread('Fix Cmd+Enter'), thread('Open links external')])).toEqual(['Fix Cmd+Enter']);
  });
});

describe('queries that fold to empty words', () => {
  test('returns for a hash and a space, or marks that fold to nothing', () => {
    expect(rankTitles('# fix', [thread('Fix sidebar lag')])).toEqual(['Fix sidebar lag']);
    expect(rankTitles('#', [thread('Fix sidebar lag')])).toEqual([]);
    expect(rankTitles('´´´', [thread('Fix sidebar lag')])).toEqual([]);
    expect(scoreSearchTerms('# fix', ['Fix sidebar'])).toBeNull();
    expect(findSearchTokenRanges('Fix sidebar', '´´´')).toEqual([]);
  });
});

describe('scoreThreadSearch beyond Latin text and titles', () => {
  test('leaves out a row whose words only meet across unrelated fields', () => {
    const scattered = thread('eMudhra certificate update request', { pullRequests: [{ number: 22645, title: 'fix model picker' }, { number: 22649, title: 'CI speedups' }] });
    expect(scoreThreadSearch('fix ci', scattered, NOW)).toBeNull();
    // The project explains its own words; one field explains the rest.
    const lakebed = thread('Weekly notes', { projectTitle: 'lakebed', pullRequests: [{ number: 243, title: 'perf: faster cold starts' }] });
    expect(scoreThreadSearch('lakebed perf', lakebed, NOW)?.explain?.()).toEqual({ label: 'PR', text: '#243 perf: faster cold starts', ranges: [{ start: 5, end: 9 }] });
  });
  test('says which field matched when the title does not', () => {
    const subject = thread('Trim model lists', { branch: 't3/cleanup-payloads', pullRequests: [{ number: 22889, title: 'Smaller MCP payloads' }] });
    for (const query of ['#22889', 'pr 22889', 'pr22889', 'pull request #22889']) {
      expect(scoreThreadSearch(foldSearchQuery(query), subject, NOW)?.explain?.()).toEqual({ label: 'PR', text: '#22889 Smaller MCP payloads', ranges: [{ start: 1, end: 6 }] });
    }
    // The app's own worktree prefix is not part of what the branch says.
    expect(scoreThreadSearch('cleanup', subject, NOW)?.explain?.()).toEqual({ label: 'Branch', text: 'cleanup-payloads', ranges: [{ start: 0, end: 7 }] });
    expect(scoreThreadSearch('trim', subject, NOW)?.explain?.()).toBeUndefined();
  });
  test('reads letters and numbers typed together, without forgiving typos in numbers', () => {
    const threads = [thread('Upgrade to pnpm 12'), thread('GPT-5 eval'), thread('iOS 26 support'), thread('Other')];
    expect(rankTitles('pnpm12', threads)).toEqual(['Upgrade to pnpm 12']);
    expect(rankTitles('gpt5', threads)).toEqual(['GPT-5 eval']);
    expect(rankTitles('ios26', threads)).toEqual(['iOS 26 support']);
    expect(rankTitles('pnpm13', threads)).toEqual([]);
  });
  test('reads whole words typed together, but not scattered letters', () => {
    const threads = [thread('VS Code extension settings'), thread('Fix UI bug in sidebar'), thread('I plan to open source all of lakebed in the next few weeks')];
    expect(rankTitles('vscode', threads)).toEqual(['VS Code extension settings']);
    expect(rankTitles('fixuibug', threads)).toEqual(['Fix UI bug in sidebar']);
    expect(rankTitles('lint', threads)).toEqual([]);
    // Short words before a mere prefix are a coincidence.
    expect(rankTitles('inth', [thread('Show skill names in thread overview')])).toEqual([]);
    expect(rankTitles('tos', [thread('Move Client State to Server')])).toEqual([]);
    expect(rankTitles('t3co', [thread('T3 Code vs Gemini')])).toEqual(['T3 Code vs Gemini']);
    expect(rankTitles('"sidebar lag"', [thread('Fix sidebar lag')])).toEqual(['Fix sidebar lag']);
    // A pasted query with a separate accent mark matches the composed title.
    expect(rankTitles('café', [thread('Café menu')])).toEqual(['Café menu']);
    // Three letters of the last word are enough, as it is typed.
    expect(rankTitles('gomod', [thread('Migrate to Go modules')])).toEqual(['Migrate to Go modules']);
    // Joined words beat an earlier match inside a word; a strong match elsewhere does not hide them.
    expect(scoreThreadSearch('vscode', thread('Fixvscode and VS Code'), NOW)?.quality).toBe(0.6);
    expect(rankTitles('vscode', [thread('VS Code remote support'), thread('Old work', { branch: 'vscode', activityAt: NOW - 400 * DAY })])).toContain('VS Code remote support');
  });
  test('stores picked queries without splitting an emoji', () => {
    expect(pickedQuery(`${'a'.repeat(31)}🚀`)).toBe('a'.repeat(31));
    expect(pickedQuery('  Upl ')).toBe('upl');
    // Folded before it is cut, as stored and as compared.
    const spaced = 'investigate    workspace thread reset bug';
    expect(pickedQuery(spaced)).toBe(pickedQuery(foldSearchQuery(spaced)));
  });
  test('matches emoji in titles', () => {
    expect(rankTitles('🚀', [thread('🚀 Launch checklist'), thread('Launch notes')])).toEqual(['🚀 Launch checklist']);
  });
  test('treats each Chinese or Japanese character as a word start', () => {
    expect(scoreThreadSearch('搜索', thread('实现全文搜索'), NOW)).not.toBeNull();
    expect(scoreThreadSearch('検索', thread('新しい検索機能'), NOW)).not.toBeNull();
  });
});

describe('scoreThreadSearch with heavy use and loose input', () => {
  test('keeps a whole word on a quiet thread above a mid-word hit on a thread opened daily', () => {
    // Sixty daily opens as the server records them: the score halves every two weeks and adds one per open.
    let opens = { openedAt: NOW - 60 * DAY, score: 1, picks: [] };
    for (let day = 59; day >= 1; day--) opens = { openedAt: NOW - day * DAY, score: opens.score * 2 ** (-DAY / (14 * DAY)) + 1, picks: [] };
    expect(rankTitles('auth', [thread('Fix oauth redirect', { activityAt: NOW, opens }), thread('Share auth across worktrees', { activityAt: NOW - 90 * DAY })]))
      .toEqual(['Share auth across worktrees', 'Fix oauth redirect']);
  });
  test('reads a word and the start of the next typed together, highlighting each part', () => {
    const threads = [thread('Fix sidebar lag'), thread('Unrelated work')];
    expect(rankTitles('fixsid', threads)).toEqual(['Fix sidebar lag']);
    // A later "F" can start the run.
    expect(rankTitles('fixsid', [thread('Feature: Fix sidebar lag')])).toEqual(['Feature: Fix sidebar lag']);
    expect(scoreThreadSearch('fixsid', threads[0]!, NOW)?.ranges).toEqual([{ start: 0, end: 3 }, { start: 4, end: 7 }]);
    // Initials are not words typed together.
    expect(rankTitles('fsl', threads)).toEqual([]);
  });
  test('finds nothing rather than an unrelated thread when no word matches', () => {
    const threads = ['Projectless threads without Git', 'Concern about connection setup', 'Simplify worktree thread creation', 'Start a new conversation',
      'Setting up the sentinel', 'I plan to open source all of lakebed in the next few weeks', 'Design notes', 'Link pull request to thread'].map(title => thread(title));
    for (const query of ['posthog', 'cron', 'stripe', 'gpu', 'fxsb', 'lint', 'deno']) expect(rankTitles(query, threads)).toEqual([]);
  });
  test('matches pieces of references that hold a number, and environment names', () => {
    const subject = thread('Trim model lists', { id: 'thread-5fe6f59c-4976', environmentLabel: 'BeeBox',
      pullRequests: [{ number: 22889, references: ['pingdotgg/t3code#22889', 'https://github.com/pingdotgg/t3code/pull/22889'] }] });
    for (const query of ['t3code#22889', 'github.com/pingdotgg/t3code/pull/22889', 'https://github.com/pingdotgg/t3code/pull/22889/files', '5fe6f59c', 'beebox']) {
      expect(scoreThreadSearch(foldSearchQuery(query), subject, NOW)).not.toBeNull();
    }
    expect(scoreThreadSearch('github.com', subject, NOW)).toBeNull();
    // A number is never cut short.
    for (const query of ['github.com/pingdotgg/t3code/pull/2288', 't3code#2288']) expect(scoreThreadSearch(foldSearchQuery(query), subject, NOW)).toBeNull();
  });
});

describe('rankMessageMatches', () => {
  test('merges environments by word match, then picks, then quality and use', () => {
    const subjects = new Map([['a-word', thread('A word')], ['a-word-2', thread('A word too')], ['b-prefix', thread('B prefix')], ['b-recent', thread('B recent', { activityAt: NOW })],
      ['b-picked', thread('B picked', { opens: { openedAt: NOW - DAY, score: 1, picks: [{ query: 'needle', at: NOW - DAY }] } })]]);
    // Environment A sent two whole words, environment B a prefix and two more.
    const matches = [{ key: 'a-word', quality: 1 }, { key: 'a-word-2', quality: 1 }, { key: 'b-prefix', quality: 0.8 }, { key: 'b-recent', quality: 1 }, { key: 'b-picked', quality: 0.8 }];
    expect(rankMessageMatches(matches, match => subjects.get(match.key), 'needle', NOW).map(match => match.key)).toEqual(['b-picked', 'b-recent', 'a-word', 'a-word-2', 'b-prefix']);
    // A palette lists title matches above, so they use up no message rows.
    const titled = new Map([...subjects, ['c-titled', thread('Needle cleanup')]]);
    expect(rankMessageMatches([{ key: 'c-titled', quality: 1 }, ...matches], match => titled.get(match.key), 'needle', NOW, { withoutTitleMatches: true }).map(match => match.key))
      .not.toContain('c-titled');
  });
  test('ranks a thread that keeps using the word above a passing mention of equal quality', () => {
    // The real case: 21 hits seven weeks back against one this week, opened today.
    const subjects = new Map([['mention', thread('Mention', { activityAt: NOW - 3 * DAY, opens: { openedAt: NOW, score: 1, picks: [] } })], ['about', thread('About it', { activityAt: NOW - 48 * DAY })]]);
    expect(rankMessageMatches([{ key: 'mention', quality: 1, hits: 1 }, { key: 'about', quality: 1, hits: 21 }], match => subjects.get(match.key), 'tailscale', NOW).map(match => match.key))
      .toEqual(['about', 'mention']);
  });
  test('keeps matches inside words below word matches, however used', () => {
    const subjects = new Map([['busy', thread('Busy', { activityAt: NOW, opens: { openedAt: NOW, score: 20, picks: [] } })], ['quiet', thread('Quiet')]]);
    expect(rankMessageMatches([{ key: 'busy', quality: 0.3 }, { key: 'quiet', quality: 1 }], match => subjects.get(match.key), 'rust', NOW).map(match => match.key)).toEqual(['quiet', 'busy']);
  });
  test('an older server sends no quality: its matches count as words', () => {
    const subjects = new Map([['old', thread('Old', { activityAt: NOW })], ['inside', thread('Inside')]]);
    expect(rankMessageMatches([{ key: 'inside', quality: 0.3 }, { key: 'old' }], match => subjects.get(match.key), 'needle', NOW).map(match => match.key)).toEqual(['old', 'inside']);
  });
});

describe('rankSearchGroups', () => {
  test('orders groups by their best match and keeps the given order on ties', () => {
    const groups = rankSearchGroups([{ group: 'actions', items: ['Open settings', 'New thread'] }, { group: 'threads', items: ['Settings sync rewrite', 'Thread settings'] },
      { group: 'projects', items: ['Settings app'] }], (item, options) => scoreSearchTerms('settings', [item], options));
    expect(groups.map(({ group, ranked }) => [group, ranked.map(({ item }) => item)]))
      .toEqual([['actions', ['Open settings']], ['threads', ['Settings sync rewrite', 'Thread settings']], ['projects', ['Settings app']]]);
  });
  test('matches inside a word only in thread titles', () => {
    expect(scoreSearchTerms('arch', ['Search project contents'])).toBeNull();
    expect(scoreThreadSearch('arch', thread('Notes', { branch: 'research-extension-system' }), NOW)).toBeNull();
    expect(scoreThreadSearch('arch', thread('Research notes'), NOW)?.quality).toBe(0.3);
  });
  test('forgives typos in names but not in hidden keywords', () => {
    const groups = rankSearchGroups([{ group: 'actions', items: [['Open settings', 'preferences', 'keybindings']] }, { group: 'settings', items: [['Keybindings', 'General']] }],
      (terms, options) => scoreSearchTerms('keybindngs', terms, options));
    expect(groups.map(({ group }) => group)).toEqual(['settings']);
    expect(scoreSearchTerms('duckdb', ['Search engine', 'duckduckgo'], { typos: true })).toBeNull();
    expect(scoreSearchTerms('rust', ['GitHub sharing', 'trusted'])).toBeNull();
    // Filler words do not make a short query long enough for keyword prefixes.
    expect(scoreSearchTerms('the ci', ['Context window indicator', 'circle'])).toBeNull();
  });
  test('lists a thread named for the query above a command with it as a keyword', () => {
    const rank = (command: string[], subject: ThreadSearchSubject) => rankSearchGroups<string, string[] | ThreadSearchSubject, SearchTermsMatch>(
      [{ group: 'actions', items: [command] }, { group: 'threads', items: [subject] }],
      (item, options) => Array.isArray(item) ? scoreSearchTerms('mcp', item, options) : scoreThreadSearch('mcp', item, NOW, options)).map(({ group }) => group);
    const named = thread('Trim MCP model lists');
    const picked = thread('Trim MCP model lists', { opens: { openedAt: NOW, score: 1, picks: [{ query: 'mcp', at: NOW }] } });
    expect(rank(['Restart agent session', 'reload', 'mcp'], named)).toEqual(['threads', 'actions']);
    // A command named for the query stays first; a pick reorders threads but never moves their group.
    expect(rank(['Reload MCP servers'], named)).toEqual(['actions', 'threads']);
    expect(rank(['Reload MCP servers'], picked)).toEqual(['actions', 'threads']);
  });
  test('does not renew a query\'s pick when the thread is picked for another one', () => {
    expect(rankTitles('deploy', [thread('Deploy notes', { opens: { openedAt: NOW, score: 2, picks: [{ query: 'scripts', at: NOW }, { query: 'deploy', at: NOW - 60 * DAY }] } }),
      thread('Deploy timeout', { opens: { openedAt: NOW - DAY, score: 1, picks: [{ query: 'deploy', at: NOW - DAY }] } })])).toEqual(['Deploy timeout', 'Deploy notes']);
  });
});

describe('findSearchTokenRanges', () => {
  test('maps folded matches back to the source text and skips short tokens', () => {
    expect(findSearchTokenRanges('Le Café résumé ci', 'CAFE resume ci')).toEqual([{ start: 3, end: 7 }, { start: 8, end: 14 }]);
  });
  test('highlights a three-letter token only where it starts a word', () => {
    expect(findSearchTokenRanges('drafts, enableFTS', 'fts')).toEqual([{ start: 14, end: 17 }]);
    expect(findSearchTokenRanges('Open handlers.ts', '.ts')).toEqual([{ start: 13, end: 16 }]);
  });
});

describe('thread shells as search subjects', () => {
  test('activity is what the user wrote; this client\'s opens count beside the server\'s', () => {
    const shell = { id: 'subject-a', title: 'Fix sidebar', branch: 't3/fix', createdAt: '2026-10-01T00:00:00.000Z', updatedAt: '2026-10-20T00:00:00.000Z', latestUserMessageAt: '2026-10-10T00:00:00.000Z' };
    expect(threadSearchSubject(shell, 'repo', null, 'env').activityAt).toBe(Date.parse('2026-10-10T00:00:00.000Z'));
    // A server that reports authored messages: a thread the user never wrote in counts from its creation.
    expect(threadSearchSubject({ ...shell, latestUserAuthoredMessageAt: null }, 'repo', null, 'env').activityAt).toBe(Date.parse('2026-10-01T00:00:00.000Z'));
    // Opens a build cannot read rank as none.
    expect(threadSearchSubject({ ...shell, opens: { openedAt: 'soon' } }, 'repo', null, 'env').opens).toBeNull();
    expect(claimThreadOpen('env', 'subject-a', NOW)).toBe(true);
    expect(claimThreadOpen('env', 'subject-a', NOW + 60_000)).toBe(false);
    const opened = threadSearchSubject({ ...shell, opens: { openedAt: NOW - DAY, score: 3, picks: [{ query: 'fix', at: NOW - DAY }] } }, 'repo', null, 'env');
    expect(opened.opens).toEqual({ openedAt: NOW + 60_000, score: 3, picks: [{ query: 'fix', at: NOW - DAY }] });
    expect(threadRecentUse(opened)).toBe(NOW + 60_000);
  });
  test('pull requests: the branch\'s, then the visible links newest first, titles of the newest three', () => {
    const link = (number: number, at: string, extra = {}) => ({ number, repository: 'acme/web', url: `https://github.com/acme/web/pull/${number}`, linkedAt: at, snapshot: { title: `PR ${number}` }, ...extra });
    const pullRequests = threadSearchPullRequests({ branchPullRequest: { number: 1, repository: 'acme/web', url: 'u1' },
      pullRequests: [link(2, '2026-10-01'), link(3, '2026-10-05'), link(4, '2026-10-03', { source: 'stack-dismissed' }), link(5, '2026-10-02'), link(6, '2026-09-01')] });
    expect(pullRequests.map(pr => [pr.number, pr.title])).toEqual([[1, null], [3, 'PR 3'], [5, 'PR 5'], [2, 'PR 2'], [6, null]]);
    expect(pullRequests[1]!.references).toEqual(['acme/web#3', 'https://github.com/acme/web/pull/3']);
  });
  test('shown message rows keep their places when a slower environment answers', () => {
    const first = nextShownOrder(null, 'needle', ['a', 'b', 'c'], 2);
    expect(first).toEqual({ query: 'needle', keys: ['a', 'b', 'c'], shown: 2 });
    expect(nextShownOrder(first, 'needle', ['z', 'a', 'b', 'c'], 2)).toEqual({ query: 'needle', keys: ['a', 'b', 'z', 'c'], shown: 3 });
    expect(nextShownOrder(first, 'needle', ['a', 'b', 'c'], 2)).toBe(first);
  });
});
