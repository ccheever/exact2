// Pull request author filtering and Escape handling, executed through the Contract fixtures.
import { describe, expect, test } from 'bun:test';
import { defaultPrPrefs, presentList, emptyList } from './pages-prs';
import { contractFn } from './contract-fn-fixture';

describe('the Pull Requests page', () => {
  const author = (login: string, name = '', selected = false) => ({ key: login.toLowerCase(), login, name, avatar: '', initial: login[0]!, detail: '0 merges loaded', selected });

  test('the Author submenu lists the chosen author, then up to ten that match "Search authors"', async () => {
    const matches = await contractFn('pages-prs.contract', 'prAuthorMatches', ['person', 'needle']);
    const visible = await contractFn('pages-prs.contract', 'prVisibleAuthors', ['authors', 'query'], { prAuthorMatches: matches });
    const people = [author('alice', 'Alice Doe'), author('bob', 'Robert'), author('carol'), ...Array.from({ length: 12 }, (_, i) => author(`dev${i}`))];
    const logins = (list: unknown) => (list as { login: string }[]).map(person => person.login);
    expect(logins(visible(people, ''))).toEqual(['alice', 'bob', 'carol', 'dev0', 'dev1', 'dev2', 'dev3', 'dev4', 'dev5', 'dev6']);
    // The login or the name, case-insensitive and trimmed; the chosen author stays first whatever the search.
    expect(logins(visible(people, '  ROB '))).toEqual(['bob']);
    expect(logins(visible(people, 'doe'))).toEqual(['alice']);
    expect(logins(visible([...people, author('zed', '', true)], 'ali'))).toEqual(['zed', 'alice']);
    expect(logins(visible(people, 'nobody'))).toEqual([]);
    expect(logins(visible(people, 'dev1'))).toEqual(['dev1', 'dev10', 'dev11']);
  });

  test('the list sends every author with a row in the state, with the name the search also matches', () => {
    const prefs = defaultPrPrefs();
    const entry = (number: number, login: string, name: string | null) => ({ projectId: 'p1', environmentId: 'e1', host: 'github.com', repository: 'acme/app', number,
      title: `PR ${number}`, url: '', state: 'open', isDraft: false, author: { login, name, avatarUrl: null }, labels: [], updatedAt: '2026-10-01T00:00:00Z', createdAt: '2026-10-01T00:00:00Z' });
    const entries = Array.from({ length: 14 }, (_, i) => entry(i + 1, `user${i}`, i === 3 ? 'Dana Smith' : null));
    const result = { viewers: {}, providers: [], entries, errors: [], truncated: false };
    const view = presentList(emptyList(prefs, ''), result, '', new Map(), prefs, '', Date.parse('2026-10-03T00:00:00.000Z'), '');
    expect(view.authors).toHaveLength(14);
    expect(view.authors.find(person => person.login === 'user3')).toMatchObject({ name: 'Dana Smith', detail: '0 merges loaded' });
  });

  test('history: a trail of pages, as the reference\'s router history goes back through them', async () => {
    // pageTrailNext (pages-hero.contract) as openPage applies it, and pageBack's step (app.contract), page by page.
    const next = await contractFn('pages-hero.contract', 'pageTrailNext', ['trail', 'current', 'name']);
    let page = '', trail: string[] = [];
    const open = (name: string) => { trail = next(trail, page, name) as string[]; page = name; };
    const back = () => { page = trail.at(-1) ?? ''; trail = trail.slice(0, -1); };
    open('usage'); open('pull-requests'); back(); expect(page).toBe('usage'); back(); expect(page).toBe('');
    open('usage'); open('pull-requests'); open('usage'); back(); expect(page).toBe('pull-requests'); back(); expect(page).toBe('usage'); back(); expect(page).toBe('');
    // A thread between them starts a new trail: Escape from the page then goes to the thread.
    open('usage'); page = ''; open('pull-requests'); back(); expect(page).toBe('');
  });

  test('a hold stands where the data says its editor is still open: the draft, a reply not yet sent', async () => {
    const live = await contractFn('pages-pr-detail.contract', 'prdEscapeLive', ['field', 'code']);
    const code = (draftOpen: boolean, threads: { id: string; repliedSerial: number }[] = []) => ({ draftOpen, threads });
    expect(live('pr-code-draft', code(true))).toBe(true);
    expect(live('pr-code-draft', code(false))).toBe(false);
    expect(live('reply:t1:0', code(false, [{ id: 't1', repliedSerial: 0 }]))).toBe(true);
    // The reply went out: its thread's serial moved on, and its field is gone with no blur.
    expect(live('reply:t1:0', code(false, [{ id: 't1', repliedSerial: 1 }]))).toBe(false);
    expect(live('reply:t2:0', code(false, [{ id: 't1', repliedSerial: 0 }]))).toBe(false);
    expect(live('pull-request-title-input', code(false))).toBe(true);
  });
});
