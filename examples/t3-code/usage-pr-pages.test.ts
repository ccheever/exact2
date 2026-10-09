// usage-and-pr-pages (2026-10-09 desktop audit PG-3..PG-7): the Usage page's unpriced popover on hover, the
// toggles' titles, the environment menu's sizing and glass, the Pull Requests page's "Search authors" and its
// Escape to go back. Reference: T3 Code 1e2ecbd975 (MIT, see LICENSE-T3) UsagePage.tsx,
// PullRequestListFilters.tsx PullRequestAuthorFilter, _chat.pull-requests.tsx and hooks/useNavigateBack.ts.
// PG-2's rounding is in pages-usage.test.ts.
import { describe, expect, test } from 'bun:test';
import { defaultPrPrefs, presentList, emptyList } from './pages-prs';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** The lines of `component name` in `file`, up to the next top-level declaration. */
const component = async (file: string, name: string) => {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end).join('\n');
};
/** A one-line Contract `fn`'s body as JavaScript, over the standard calls it uses. */
const contractFn = async (file: string, name: string, params: string[], extra: Record<string, unknown> = {}) => {
  const body = new RegExp(`^fn ${name}\\([^)]*\\): [^=]+ = (.+)$`, 'm').exec(await source(file))?.[1];
  if (!body) throw new Error(`${file}: no fn ${name}`);
  const js = body.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');
  const std = {
    includes: (within: string | unknown[], item: unknown) => (within as unknown[]).includes(item as never),
    toLowerCase: (text: string) => text.toLowerCase(), trim: (text: string) => text.trim(),
    slice: (items: unknown[], start: number, end?: number) => items.slice(start, end),
    concat: (a: unknown[], b: unknown[]) => [...a, ...b], filter: (items: unknown[], keep: (item: never) => boolean) => items.filter(keep as never),
    ...extra,
  };
  return new Function(...Object.keys(std), ...params, `return ${js};`).bind(null, ...Object.values(std)) as (...args: unknown[]) => unknown;
};

describe('the Usage page (PG-3, PG-4, PG-5)', () => {
  test('the unpriced info opens its popover on hover and pins it on a press, as a pooled segment does (PG-3)', async () => {
    const info = await component('pages-usage.contract', 'UnpricedInfo');
    // PopoverTrigger openOnHover: the page's enterSeg (300 ms open delay on the window's hover clock) and pin; the
    // frame the page places the popup by and gives the focus back to on Escape is `usage-seg-unpriced`.
    expect(info).toContain('button id="usage-seg-unpriced" press=press hover=enter aria-label="Unpriced usage details" aria-expanded=open');
    expect(info).not.toContain('popovertarget');
    const headline = await component('pages-usage.contract', 'UsageHeadline');
    expect(headline).toContain('UnpricedInfo(open=(shown == "unpriced"), enter=enterSeg("unpriced"), press=pin("unpriced"))');
    const page = await component('pages-usage.contract', 'UsagePage');
    expect(page).toContain('shown=shown, setBreakdown=setBreakdown, enterSeg=enterSeg, pin=pin, op=op)');
    expect(page).toContain('when shown == "unpriced"\n                    UnpricedPopup(text=page.unpriced, x=popX, y=popY, w=popW, enterPop=enterPop)');
    // Light dismiss and Escape read the popup's own frame by the id every segment popover has.
    expect(page).toContain('let pop = frame(`usage-seg-pop-${held}`)');
    expect(page).toContain('focus(`usage-seg-${shown}`)');
    const popup = await component('pages-usage.contract', 'UnpricedPopup');
    // Side top, centred, sideOffset 4; the popup's hover keeps it open (Base UI's safePolygon and popup hover).
    expect(popup).toContain('left=(x + w / 2 - 10 * rem) width="20rem" top=0 height=y justify-content="flex-end" align-items="center"');
    expect(popup).toContain('column hover=enterPop("unpriced") padding-bottom="0.25rem" pointer-events="auto"');
    expect(popup).toContain('column id="usage-seg-pop-unpriced" ');
    expect(popup).toContain('background-color="light-dark(#ffffff, #111111)"');
    expect(popup).toContain('role="dialog" aria-label="Unpriced usage details"');
  });

  test('every metric and period toggle and select option carries its shortcut title (PG-4)', async () => {
    const page = await component('pages-usage.contract', 'UsagePage');
    const commands: [string, string][] = [['Cost', 'usage.cost'], ['Tokens', 'usage.tokens'], ['Limits', 'usage.limits'], ['Past 24h', 'usage.period.day'],
      ['7 days', 'usage.period.week'], ['30 days', 'usage.period.month'], ['90 days', 'usage.period.quarter']];
    for (const [label, command] of commands) {
      expect(page).toContain(`Segment(label="${label}", title=usageTitle(keys, "${command}", "${label}"), `);
      expect(page).toContain(`label="${label}", title=usageTitle(keys, "${command}", "${label}"), selected=`);
    }
    const summary = await component('pages-usage.contract', 'UsageSummary');
    expect(summary).toContain('Segment(label="Model", title="", ');
    const segment = await component('pages-usage.contract', 'Segment');
    expect(segment).toContain('when title != ""\n      button press=press hover=hover disabled=disabled title=title role="radio"');
    expect(await component('pages-usage.contract', 'UsageOption')).toContain('hover=hover title=title role="option"');
    const fn = (await source('pages-usage.contract')).match(/^fn usageTitle\(.+$/m)?.[0];
    expect(fn).toBe('fn usageTitle(keys: list<UsageKey>, id: string, label: string): string = match first(filter(keys, (k) => k.id == id)) { case some(k) => k.title, case none => label }');
  });

  test('the environment menu grows to its rows on an opaque popup (PG-5)', async () => {
    const breadcrumb = await component('pages-usage.contract', 'UsageBreadcrumb');
    expect(breadcrumb).toContain('column min-width=`${page.menuWidth / 16}rem` padding="0.25rem" box-sizing="border-box"');
    expect(breadcrumb).not.toContain('column width=`${page.menuWidth');
    expect(breadcrumb).toContain('background-color="light-dark(#ffffff, #111111)"');
    const row = await component('pages-usage.contract', 'CheckRow');
    // flex-1 truncate under a popup that sizes to its content: the label keeps its width and never truncates.
    expect(row).toContain('text label font-size="0.875rem" line-height="1.25rem" color="light-dark(#27272a, #f5f5f5)" flex-grow=1 flex-shrink=0 white-space="nowrap"');
    expect(row).not.toContain('line-clamp');
    const prices = await source('pages-usage-prices.contract');
    expect(prices).toContain('column min-width=`${prices.menuWidth / 16}rem` padding="0.25rem"');
    for (const file of ['pages-usage.contract', 'pages-usage-prices.contract', 'pages-prs.contract']) expect(await source(file)).not.toContain('#ffffffd5');
  });
});

describe('the Pull Requests page (PG-6, PG-7)', () => {
  const author = (login: string, name = '', selected = false) => ({ key: login.toLowerCase(), login, name, avatar: '', initial: login[0]!, detail: '0 merges loaded', selected });

  test('the Author submenu lists the chosen author, then up to ten that match "Search authors" (PG-6)', async () => {
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

  test('"Search authors" is focused as the submenu opens and keeps its keys from the menu (PG-6)', async () => {
    const menu = await component('pages-prs.contract', 'PrFiltersMenu');
    expect(menu).toContain('derive authors = prVisibleAuthors(page.authors, authorQuery)');
    expect(menu).toContain('    if name == "author" and sub != "author"\n      focus("pr-author-search")');
    expect(menu).toContain('focus(name == "author" ? "pr-author-search" : "pr-filters-sub-keys")');
    // Opening Filters again starts with no search, as the reference's popup remounts its state.
    expect(menu).toContain('action openMenu\n    sub = ""\n    authorQuery = ""');
    expect(menu).toContain('button popovertarget="pr-filters-menu" press=openMenu ');
    expect(menu).toContain('map(authors, (person) => KmItem(id=`pr-author-${person.key}`, label=person.login))');
    const sub = await component('pages-prs.contract', 'PrFilterSub');
    expect(sub).toContain('input id="pr-author-search" value=authorQuery input=searchAuthors key=searchKey autofocus=true placeholder="Search authors" aria-label="Search authors"');
    expect(sub).toContain('action searchKey(k: string)\n    if k != "ArrowDown" and k != "Escape"\n      stopPropagation()');
    expect(sub).toContain('each person in authors key=person.key');
    expect(sub).toContain('when length(authors) == 0\n          text "No authors found"');
    // The search field precedes Anyone.
    expect(sub.indexOf('pr-author-search')).toBeLessThan(sub.indexOf('testId="pr-author-anyone"'));
  });

  test('the list sends every author with a row in the state, with the name the search also matches (PG-6)', () => {
    const prefs = defaultPrPrefs();
    const entry = (number: number, login: string, name: string | null) => ({ projectId: 'p1', environmentId: 'e1', host: 'github.com', repository: 'acme/app', number,
      title: `PR ${number}`, url: '', state: 'open', isDraft: false, author: { login, name, avatarUrl: null }, labels: [], updatedAt: '2026-10-01T00:00:00Z', createdAt: '2026-10-01T00:00:00Z' });
    const entries = Array.from({ length: 14 }, (_, i) => entry(i + 1, `user${i}`, i === 3 ? 'Dana Smith' : null));
    const result = { viewers: {}, providers: [], entries, errors: [], truncated: false };
    const view = presentList(emptyList(prefs, ''), result, '', new Map(), prefs, '', Date.parse('2026-10-03T00:00:00.000Z'), '');
    expect(view.authors).toHaveLength(14);
    expect(view.authors.find(person => person.login === 'user3')).toMatchObject({ name: 'Dana Smith', detail: '0 merges loaded' });
  });

  test('an Escape nothing else holds blurs the focus and goes back to the page before (PG-7)', async () => {
    const list = await component('pages-prs.contract', 'PrList');
    expect(list).toContain('button press=escape aria-keyshortcuts="Escape" aria-label="Back" tabindex=-1 testId="pull-requests-key-back"');
    expect(list).toContain('  action escape\n    blur()\n    back()');
    const cover = await component('app-main.contract', 'PagesCover');
    expect(cover.split('\n').filter(line => line.includes('PrList(')).map(line => line.endsWith('act=prAct, back=openPage(""))'))).toEqual([true, true]);
  });

  test('the page\'s other owners of Escape hold it as modals while they own it (PG-7)', async () => {
    // Popovers (Base UI Popover stops the Escape it dismisses with) while they show; menus already are (KeyMenu modal).
    expect(await source('pages-pr-quick.contract')).toContain('row id=popId popover="auto" aria-modal=true position="absolute" padding-top="0.25rem" align-items="flex-start" testId=`${popId}-layer`\n        column width="20rem"');
    expect(await source('pages-pr-compose.contract')).toContain('row id=popId popover="auto" aria-modal=true ');
    expect(await source('pages-pr-actions.contract')).toContain('row id="pr-freshness" popover="auto" aria-modal=true ');
    const meta = await source('pages-pr-meta.contract');
    expect(meta).toContain('row id=pickerId popover="auto" aria-modal=true ');
    expect(meta).toContain('column id=menuId popover="auto" aria-modal=true ');
    // Editors whose own key handler cancels on Escape (PullRequestMarkdownEditor, the title, a reply, a code comment).
    const edit = await source('pages-pr-edit.contract');
    expect(edit).toContain('key=keys tabindex=-1 aria-modal=open testId=testId');
    expect(edit).toContain('gap="0.5rem" aria-modal=true testId="pull-request-title-editor"');
    const threads = await source('pages-pr-threads.contract');
    expect(threads).toContain('when replyOpen\n            // aria-modal: the reply');
    expect(threads).toContain('aria-modal=true testId="pull-request-comment-draft"');
    // The Usage page's narrow selects are Base UI Select popups, modal too.
    expect(await component('pages-usage.contract', 'UsageSelect')).toContain('column id=menuId popover="auto" aria-modal=true ');
  });
});
