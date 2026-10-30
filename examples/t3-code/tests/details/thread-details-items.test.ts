// What the thread details card shows (threadDetailsItems.ts and ThreadDetailsItemsMenu.tsx at 6d450a4284): the ten
// items and their defaults, the choices stored as differences, the menu's rows and Reset, the saved record read back,
// and the card's sections following the choices (shell-details.contract DetailsBody, git.contract GitRows).
import { describe, expect, test } from 'bun:test';
import {
  THREAD_DETAILS_ITEMS, adoptThreadDetailsItems, isDefaultThreadDetailsVisibility, resolveThreadDetailsVisibility, setThreadDetailsItem, shownItemCount,
  threadDetailsChoices, threadDetailsItemsLocal, threadDetailsMenu, threadDetailsVisibility,
} from '../../src/details/thread-details-items';
import { adoptFilesPrefs } from '../../src/panel/files-prefs';
import { contractComponent } from '../support/contract-action-fixture';

const holder = () => ({ local: {} as object });

describe('the items and their defaults (THREAD_DETAILS_ITEMS)', () => {
  test('ten items in the reference order, each on but Usage', () => {
    expect(THREAD_DETAILS_ITEMS.map(item => item.label)).toEqual(['Environment and worktree', 'Open in editor', 'Project scripts', 'Branch and pull request',
      'Git actions', 'Changes', 'Servers', 'Automations', 'Lineage', 'Usage']);
    const visibility = resolveThreadDetailsVisibility({});
    expect(Object.entries(visibility).filter(([, on]) => !on).map(([id]) => id)).toEqual(['usage']);
    expect([isDefaultThreadDetailsVisibility(visibility), shownItemCount(visibility)]).toEqual([true, 9]);
  });
});

describe('the choices are stored as differences from the default (setThreadDetailsItem)', () => {
  test('hiding Changes and showing Usage store two keys; going back to a default drops its key', () => {
    let choices = setThreadDetailsItem({}, 'changes', false);
    choices = setThreadDetailsItem(choices, 'usage', true);
    expect(choices).toEqual({ changes: false, usage: true });
    expect(resolveThreadDetailsVisibility(choices)).toMatchObject({ changes: false, usage: true, servers: true });
    expect(setThreadDetailsItem(choices, 'changes', true)).toEqual({ usage: true });
    expect(setThreadDetailsItem({}, 'servers', true)).toEqual({});
  });
  test('the menu (a checkbox per item, Reset disabled at the defaults) writes the client preference record', () => {
    const client = holder();
    expect(threadDetailsMenu(client).resetDisabled).toBe(true);
    threadDetailsItemsLocal(client, 'item', 'changes', 'false');
    threadDetailsItemsLocal(client, 'item', 'usage', 'true');
    threadDetailsItemsLocal(client, 'item', 'nonsense', 'false');
    expect((client.local as { threadDetailsItems?: object }).threadDetailsItems).toEqual({ changes: false, usage: true });
    const menu = threadDetailsMenu(client);
    expect(menu.items.filter(item => item.id === 'changes' || item.id === 'usage')).toEqual([{ id: 'changes', label: 'Changes', checked: false }, { id: 'usage', label: 'Usage', checked: true }]);
    expect(menu.resetDisabled).toBe(false);
    threadDetailsItemsLocal(client, 'reset', '', '');
    expect([threadDetailsChoices(client), isDefaultThreadDetailsVisibility(threadDetailsVisibility(client))]).toEqual([{}, true]);
  });
  test('a relaunch reads the saved record back (with the Files preferences), dropping unknown items, non-booleans and defaults', () => {
    const next = {};
    adoptFilesPrefs(next, { threadDetailsItems: { changes: false, usage: true, servers: true, gone: false, lineage: 'no' } });
    expect((next as { threadDetailsItems?: object }).threadDetailsItems).toEqual({ changes: false, usage: true });
    const fresh = {};
    adoptThreadDetailsItems(fresh, {});
    expect('threadDetailsItems' in fresh).toBe(false);
  });
});

describe('the card follows the choices (DetailsBody, GitRows)', () => {
  const all = resolveThreadDetailsVisibility({});
  const details = (items: object, extra: object = {}) => ({
    items: { ...all, ...items }, editorShow: true, scripts: [], cwd: '/repo', error: '', envModeSelect: false, showLineage: false, lineage: [],
    automations: { show: false }, servers: { show: false, rows: [] }, usage: { show: false, rows: [] }, itemsMenu: { items: [], resetDisabled: true },
    git: { repo: true, quiet: false, progress: false, success: false, menu: [], envPick: false, branch: { show: true } }, ...extra,
  });
  const body = contractComponent('shell-details.contract', 'DetailsBody');
  const scope = (items: object, density = 'full') => ({ density, inline: true, maxHeight: 600, rem: 16, KmItem: (item: unknown) => item, details: details(items) });
  test('a hidden item leaves its rows out; with nothing left the card offers "Choose what to show"', () => {
    expect(body.drawn('testId="details-choose-items"', body.state(), scope({}))).toBe(false);
    expect(body.drawn('OpenInRow(details=details', body.state(), scope({ openIn: false }))).toBe(false);
    expect(body.drawn('OpenInRow(details=details', body.state(), scope({}))).toBe(true);
    const none = Object.fromEntries(THREAD_DETAILS_ITEMS.map(item => [item.id, false]));
    expect(body.drawn('testId="details-choose-items"', body.state(), scope(none))).toBe(true);
    // The full layout still names its top part when the Workspace section is gone (thread-details-card-layout.ts).
    expect(body.drawn('box width=0 height=0 aria-hidden=true id="frame-details-full-top"', body.state(), scope(none))).toBe(true);
  });
  test('Git actions and Changes hide on their own (GitActionsControl showActions, showChanges)', () => {
    const rows = contractComponent('git.contract', 'GitRows');
    const git = (items: object) => ({ compact: false, KmItem: (item: unknown) => item, details: details(items) });
    expect([rows.drawn('GitChanges(details=details', rows.state(), git({ changes: false })), rows.drawn('testId="details-git-group"', rows.state(), git({ changes: false }))]).toEqual([false, true]);
    expect([rows.drawn('GitChanges(details=details', rows.state(), git({ gitActions: false })), rows.drawn('testId="details-git-group"', rows.state(), git({ gitActions: false }))]).toEqual([true, false]);
    expect(rows.drawn('testId="details-branch"', rows.state(), git({ branch: false }))).toBe(false);
  });
});
