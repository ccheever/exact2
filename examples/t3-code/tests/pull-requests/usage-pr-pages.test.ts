// Pull request author filtering and Escape handling, executed through the Contract fixtures.
import { describe, expect, test } from 'bun:test';
import { defaultPrPrefs, presentList, emptyList } from '../../src/pull-requests/pages-prs';
import { contractFn } from '../support/contract-fn-fixture';

describe('the Pull Requests page', () => {
  const author = (login: string, name = '', selected = false) => ({ key: login.toLowerCase(), login, name, avatar: '', initial: login[0]!, detail: '', selected });

  test('while its search is ahead of the host\'s answer, Author lists the chosen author, then up to thirty loaded ones that match', async () => {
    const matches = await contractFn('pages-prs.contract', 'prAuthorMatches', ['person', 'needle']);
    const visible = await contractFn('pages-prs.contract', 'prVisibleAuthors', ['chosen', 'authors', 'query'], { prAuthorMatches: matches });
    const people = [author('alice', 'Alice Doe'), author('bob', 'Robert'), author('carol'), ...Array.from({ length: 40 }, (_, i) => author(`dev${i}`))];
    const logins = (list: unknown) => (list as { login: string }[]).map(person => person.login);
    expect(logins(visible([], people, ''))).toEqual(['alice', 'bob', 'carol', ...Array.from({ length: 27 }, (_, i) => `dev${i}`)]);
    // The login or the name, case-insensitive and trimmed; the chosen author stays first whatever the search.
    expect(logins(visible([], people, '  ROB '))).toEqual(['bob']);
    expect(logins(visible([], people, 'doe'))).toEqual(['alice']);
    const zed = author('zed', '', true);
    expect(logins(visible([zed], [...people, zed], 'ali'))).toEqual(['zed', 'alice']);
    expect(logins(visible([], people, 'nobody'))).toEqual([]);
    expect(logins(visible([], people, 'dev1'))).toEqual(['dev1', ...Array.from({ length: 10 }, (_, i) => `dev1${i}`)]);
  });

  test('while its search is ahead of the host\'s answer, Labels lists the chosen labels no loaded match holds, then the loaded matches', async () => {
    const matches = await contractFn('pages-prs.contract', 'prLabelMatches', ['label', 'needle']);
    const visible = await contractFn('pages-prs.contract', 'prVisibleLabels', ['chosen', 'labels', 'query'], { prLabelMatches: matches });
    const label = (name: string, count: number, selected = false) => ({ key: name.toLowerCase(), name, color: count ? '#d73a4a' : '', count, selected });
    const loaded = [label('bug', 3, true), label('docs', 1)], chosen = [label('bug', 0, true), label('size:XL', 0, true)];
    const names = (list: unknown) => (list as { name: string; count: number }[]).map(entry => `${entry.name}:${entry.count}`);
    expect(names(visible(chosen, loaded, ''))).toEqual(['size:XL:0', 'bug:3', 'docs:1']);
    expect(names(visible(chosen, loaded, ' DO '))).toEqual(['bug:0', 'size:XL:0', 'docs:1']);
  });

  test('the list sends every author with a row in the state, with the name the search also matches, and the merges loaded only above 0', () => {
    const prefs = { ...defaultPrPrefs(), author: 'nobody-here', labels: ['bug'] };
    const entry = (number: number, login: string, name: string | null, state = 'open') => ({ projectId: 'p1', environmentId: 'e1', host: 'github.com', repository: 'acme/app', number,
      title: `PR ${number}`, url: '', state, isDraft: false, author: { login, name, avatarUrl: null }, labels: [], updatedAt: '2026-10-01T00:00:00Z', createdAt: '2026-10-01T00:00:00Z' });
    const entries = [...Array.from({ length: 14 }, (_, i) => entry(i + 1, `user${i}`, i === 3 ? 'Dana Smith' : null)), entry(20, 'user5', null, 'merged')];
    const result = { viewers: {}, providers: [], entries, errors: [], truncated: false };
    const view = presentList(emptyList(prefs, ''), result, '', new Map(), { ...prefs, state: 'all' }, '', Date.parse('2026-10-03T00:00:00.000Z'), '');
    expect(view.authors).toHaveLength(14);
    expect(view.authors.find(person => person.login === 'user3')).toMatchObject({ name: 'Dana Smith', detail: '' });
    expect(view.authors[0]).toMatchObject({ login: 'user5', detail: '1 merges loaded' });
    // The chosen author is a choice though no loaded row carries it; each chosen label is one.
    expect(view.authorChosen.map(person => [person.login, person.selected])).toEqual([['nobody-here', true]]);
    expect(view.labelChosen.map(entry => [entry.name, entry.count, entry.selected])).toEqual([['bug', 0, true]]);
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
