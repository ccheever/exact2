// The pull request page's file links (MarkdownFileLink with no primary action, T3 Code 7c3b3b3605ef; MIT, see
// LICENSE-T3): a press opens the link's menu where it was clicked (showFileContextMenu at the event's clientX, clientY),
// from the page's text and from a review comment in the Code tab's or a guide's diff alike. The menu is a
// LinkMenuPopoverAt the page shows once its insets are the point (LLP 1021 D2).
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const body = contractComponent('src/pull-requests/pages-pr-detail.contract', 'PrdBody');
const button = contractComponent('src/timeline/link-menus.contract', 'FileLinkButton');
const popover = contractComponent('src/timeline/link-menus.contract', 'LinkMenuPopoverAt');

/** The page's props as its derives read them, and its outward calls, recorded. */
function page() {
  const calls: unknown[][] = [], state = body.state();
  const scope = { scrollId: 'pr-page-scroll', tab: 'code', detail: { guide: { enabled: true }, codeTab: { available: true } },
    local: (...args: unknown[]) => calls.push(['local', ...args]), act: (...args: unknown[]) => calls.push(['act', ...args]),
    showPopover: (...args: unknown[]) => calls.push(['showPopover', ...args]) };
  return { calls, state, scope };
}

describe('the pull request page\'s file link menu', () => {
  test('a press hands its point to the page, which has no popover of its own to open', () => {
    const sent: unknown[][] = [], scope = { tip: '/srv/app/README.md', target: 'README.md', menu: [], fileLinkPressMenu: true,
      fileLinkPopover: 'pr-file-link-menu-page', fileLinkMenu: () => {}, local: (...args: unknown[]) => sent.push(args) };
    button.call('press', {}, scope, [{ clientX: 70, clientY: 38 }]);
    button.call('press', {}, { ...scope, fileLinkPressMenu: false }, [{ clientX: 70, clientY: 38 }]);
    expect(sent).toEqual([['file-link-at', '70', '38'], ['surface-file-link', '/srv/app/README.md', 'README.md']]);
    // The page shows the menu once the insets are set; a popovertarget would open it at the last press's point.
    expect(() => button.attribute('press=press', 'popovertarget', {}, scope)).toThrow(/has no popovertarget/);
  });

  test('the page sets the menu\'s insets to the point, then shows it; a key\'s press is the window\'s corner', () => {
    const { calls, state, scope } = page();
    body.call('textLocal', state, scope, ['file-link-at', '70', '38']);
    expect([state.fileLeft, state.fileTop]).toEqual(['70px', '38px']);
    body.call('textLocal', state, scope, ['file-link-at', '0', '0']);
    expect([state.fileLeft, state.fileTop]).toEqual(['0px', '0px']);
    expect(calls).toEqual([['showPopover', 'pr-file-link-menu-page-press'], ['showPopover', 'pr-file-link-menu-page-press']]);
    const at = (left: string, top: string) => ['left', 'top', 'margin'].map(attr => popover.attribute('popover="auto"', attr, {}, { menuId: 'm', rows: [], pick: () => {}, left, top }));
    expect(at('70px', '38px')).toEqual(['70px', '38px', 0]);
  });

  test('a review comment\'s file link in the Code tab or a guide\'s diff presses through the page too', () => {
    const { calls, state, scope } = page();
    // Before, these got the panel's own `local`, where `file-link-at` is no timeline action (timeline-presentation.ts).
    const lines = appText('src/pull-requests/pages-pr-detail.contract').split('\n');
    const localOf = (callee: string) => /\blocal=(\w+)/.exec(lines.find(line => line.trim().startsWith(`${callee}(`)) ?? '')?.[1];
    expect(['PrdCode', 'GuideDiffs'].map(localOf)).toEqual(['diffLocal', 'diffLocal']);
    body.call('diffLocal', state, scope, ['file-link-at', '120', '400']);
    body.call('diffLocal', state, scope, ['fold', 'thread:1', '']);
    expect([state.fileLeft, state.fileTop]).toEqual(['120px', '400px']);
    expect(calls).toEqual([['showPopover', 'pr-file-link-menu-page-press'], ['local', 'fold', 'thread:1', '']]);
  });
});
