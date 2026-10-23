// Menus the reference draws as Base UI Menus are `popover="auto"` popovers here, opened by `popovertarget`: the Mac
// host light-dismisses them (exact2 PR #676), so a press anywhere outside closes them, as Base UI's outside press does.
// They were app-drawn layers whose click-away backdrop covered only their card or panel, so a press on the sidebar,
// the timeline or the prompt left them open. Read from the Contract sources and run through their actions; the
// presses themselves are driven on macOS.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';

const source = (file: string) => Bun.file(appPath(file)).text();
const line = (text: string, needle: string) => text.split('\n').find(candidate => candidate.includes(needle)) ?? '';
const event = { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, repeat: false };
function effects(calls: unknown[][]) {
  return Object.fromEntries(['preventDefault', 'stopPropagation', 'focus', 'blur', 'showPopover', 'hidePopover', 'local', 'command']
    .map(name => [name, (...args: unknown[]) => calls.push([name, ...args])]));
}
const kmKeyEnd = (k: string) => k === 'ArrowUp' ? 'last' : (k === 'ArrowDown' || k === 'Enter' || k === ' ' || k === 'Space' ? 'first' : '');
const kmTriggerKey = (k: string) => ['ArrowDown', 'ArrowUp', 'Enter', ' ', 'Space'].includes(k);

describe('the file subheader\'s menus (FilePreviewPanel)', () => {
  test('Open in: its chevron opens the popover, end-aligned under it, modal while it shows', async () => {
    const files = await source('files-surface.contract');
    const menu = line(files, 'column id="file-editors-menu"');
    for (const part of ['popover="auto"', 'position-area="bottom span-left"', 'margin-top="0.25rem"', 'min-width="10rem"', 'toggle=toggled',
      'data-dialog-return="menu"', 'focusin=dialogMenuEntered("file-editors-menu", "file-choose-editor")']) expect(menu).toContain(part);
    const chevron = line(files, 'button id="file-choose-editor"');
    for (const part of ['popovertarget="file-editors-menu"', 'key=editorsKey', 'aria-haspopup="menu"', 'aria-expanded=editorsOpen']) expect(chevron).toContain(part);
    expect(chevron).not.toContain('press=');
    expect(line(files, 'KeyMenu(menuId="file-editors-keys"')).toContain('keyed=0, gap="0px", modal=true)');
    expect(line(files, 'button id=`file-editor-${editor.id}`')).toContain('popovertarget="file-editors-menu" popovertargetaction="hide"');
    const header = contractComponent('files-surface.contract', 'FileHeader'), state = header.state();
    header.call('editorsToggled', state, {}, [{ newState: 'open' }]);
    expect(state.editorsOpen).toBe(true);
    header.call('editorsToggled', state, {}, [{ newState: 'closed' }]);
    expect(state.editorsOpen).toBe(false);
  });

  test('a folder crumb: its popover lists the folder while it shows, and a file picked closes it', async () => {
    const files = await source('files-surface.contract');
    const menu = line(files, 'column id=`crumb-menu-${crumb.id}`');
    for (const part of ['popover="auto"', 'position-area="bottom span-right"', 'margin-top="0.25rem"', 'toggle=toggled', 'data-dialog-return="menu"']) expect(menu).toContain(part);
    expect(line(files, 'button id=`crumb-${crumb.id}`')).toContain('popovertarget=`crumb-menu-${crumb.id}` key=crumbKey');
    expect(line(files, 'KeyMenu(menuId="crumb-menu-keys"')).toContain('keyed=kmEndKeyed(menuEnd), gap="0px", modal=true)');
    const view = contractComponent('files-surface.contract', 'FilesCrumbView');
    const crumb = { id: 'src', path: 'src', label: 'src' };
    for (const key of ['ArrowDown', 'ArrowUp', 'Enter', ' ']) {
      const state = view.state(), calls: unknown[][] = [];
      view.call('crumbKey', state, { ...effects(calls), crumb, kmKeyEnd, kmTriggerKey }, [key, event]);
      expect(state.menuEnd).toBe(key === 'ArrowUp' ? 'last' : 'first');
      expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ['showPopover', 'crumb-menu-src', { source: 'crumb-src' }]]);
    }
    const state = view.state(), calls: unknown[][] = [];
    view.call('crumbKey', state, { ...effects(calls), crumb, kmKeyEnd, kmTriggerKey }, ['Tab', event]);
    expect(calls).toEqual([]);
    view.call('toggled', state, { ...effects(calls), crumb }, [{ newState: 'open' }]);
    expect(calls).toEqual([['local', 'surface-files-crumb', 'src', 'open']]);
    calls.length = 0;
    state.menuEnd = 'last';
    view.call('toggled', state, { ...effects(calls), crumb }, [{ newState: 'closed' }]);
    expect(state.menuEnd).toBe('');
    expect(calls).toEqual([['local', 'surface-files-crumb-close', 'src', '']]);
    calls.length = 0;
    view.call('pick', state, { ...effects(calls), crumb }, ['src/util', true]);
    expect(calls).toEqual([['local', 'surface-files-crumb-dir', 'src/util', '']]);
    calls.length = 0;
    view.call('pick', state, { ...effects(calls), crumb }, ['src/index.ts', false]);
    expect(calls).toEqual([['hidePopover', 'crumb-menu-src'], ['local', 'surface-file', 'src/index.ts', '']]);
  });
});

describe('a linked pull request\'s actions (ThreadPullRequestsPanel)', () => {
  test('"…" opens the row\'s popover, end-aligned under it; its rows mount while it shows and close it as they act', async () => {
    const surfaces = await source('panel.contract');
    expect(surfaces).not.toContain('linked-pr-menu-backdrop');
    const menu = line(surfaces, 'column id=`linked-pr-menu-${row.key}`');
    for (const part of ['popover="auto"', 'position-area="bottom span-left"', 'margin-top="0.25rem"', 'toggle=toggled', 'data-dialog-return="menu"',
      'focusin=dialogMenuEntered(`linked-pr-menu-${row.key}`, `linked-pr-actions-${row.key}`)']) expect(menu).toContain(part);
    const trigger = line(surfaces, 'button id=`linked-pr-actions-${row.key}`');
    for (const part of ['popovertarget=`linked-pr-menu-${row.key}`', 'key=menuKey', 'aria-haspopup="menu"', 'aria-expanded=menuOpen']) expect(trigger).toContain(part);
    expect(trigger).not.toContain('press=');
    expect(line(surfaces, 'KeyMenu(menuId="linked-pr-menu-keys"')).toContain('keyed=keyed, gap="0px", modal=true)');
    expect(line(surfaces, 'button id=`linked-pr-menu-${test}`')).toContain('popovertarget=menuId popovertargetaction="hide"');
    const view = contractComponent('panel.contract', 'LinkedPrRowView');
    const row = { key: 'github.com/o/r#7', number: 7 };
    for (const key of ['ArrowDown', 'ArrowUp', 'Enter', ' ']) {
      const state = view.state(), calls: unknown[][] = [];
      view.call('menuKey', state, { ...effects(calls), row, kmKeyEnd, kmTriggerKey }, [key, event]);
      expect(state.menuEnd).toBe(key === 'ArrowUp' ? 'last' : 'first');
      expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ['showPopover', 'linked-pr-menu-github.com/o/r#7', { source: 'linked-pr-actions-github.com/o/r#7' }]]);
    }
    const state = view.state();
    view.call('toggled', state, {}, [{ newState: 'open' }]);
    expect(state.menuOpen).toBe(true);
    state.menuEnd = 'first';
    view.call('toggled', state, {}, [{ newState: 'closed' }]);
    expect([state.menuOpen, state.menuEnd]).toEqual([false, '']);
  });
});

describe('the approval\'s "More approval options" menu (ComposerPendingApprovalActions)', () => {
  test('"…" opens a popover over it, end-aligned, modal while it shows; a choice closes it', async () => {
    const requests = await source('requests.contract');
    expect(requests).not.toContain('approval-menu-dismiss');
    const menu = line(requests, 'column id=`approval-menu-${approval.id}`');
    for (const part of ['popover="auto"', 'position-area="top span-left"', 'margin-bottom="0.25rem"', 'toggle=toggled', 'data-dialog-return="menu"',
      'focusin=dialogMenuEntered(`approval-menu-${approval.id}`, `approval-more-${approval.id}`)']) expect(menu).toContain(part);
    // MenuPopup width "content": the popup is as wide as its rows (d356059c84).
    expect(menu).not.toContain(' width=');
    const trigger = line(requests, 'button id=`approval-more-${approval.id}`');
    for (const part of ['popovertarget=`approval-menu-${approval.id}`', 'key=menuKey', 'aria-haspopup="menu"', 'aria-expanded=menuOpen']) expect(trigger).toContain(part);
    expect(trigger).not.toContain('press=');
    expect(line(requests, 'KeyMenu(menuId=`approval-menu-${approval.id}-keys`')).toContain('items=menuItems, keyed=0, gap="0px", modal=true)');
    expect(line(requests, 'button id=testId press=choose')).toContain('popovertarget=menuId popovertargetaction="hide"');
    const actions = contractComponent('requests.contract', 'ApprovalActions'), state = actions.state();
    actions.call('toggled', state, {}, [{ newState: 'open' }]);
    expect(state.menuOpen).toBe(true);
    actions.call('toggled', state, {}, [{ newState: 'closed' }]);
    expect(state.menuOpen).toBe(false);
  });
});

describe('the device rail\'s menus (DeviceControlsRail)', () => {
  test('each trigger opens its popover beside it; its expanded state follows the toggle; a choice closes it', async () => {
    const device = await source('device-workspace.contract');
    const menu = line(device, 'column id=menuId data-dialog-return="menu"');
    for (const part of ['focusin=dialogMenuEntered(menuId, testId)', 'toggle=toggled', 'popover="auto"', 'position-area=area', 'margin-right="0.25rem"', 'min-width="10rem"']) expect(menu).toContain(part);
    const trigger = line(device, 'button id=testId popovertarget=menuId');
    for (const part of ['key=triggerKey', 'aria-haspopup="menu"', 'aria-expanded=open']) expect(trigger).toContain(part);
    expect(trigger).not.toContain('press=');
    expect(line(device, 'button id=testId press=press popovertarget=menuId popovertargetaction="hide"')).toContain('role=(icon == ""');
    expect(device).not.toMatch(/ws\.rail\.menu|surface-session-menu|railEnd/);
    const button = contractComponent('device-workspace.contract', 'RailMenuButton'), state = button.state();
    button.call('toggled', state, {}, [{ newState: 'open' }]);
    expect(state.open).toBe(true);
    button.call('toggled', state, {}, [{ newState: 'closed' }]);
    expect(state.open).toBe(false);
  });
});
