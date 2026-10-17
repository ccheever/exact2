// What keyboard focus can reach while a dialog is open, read from the Contract
// sources as context-menu-hookup.test.ts reads its handlers. Sequential focus follows tree order on every
// host and skips a node whose `tabindex` is negative (HTML's rule, LLP 1088 D7.3), so a component's
// controls in source order, less those, are its reference Tab order. Time and select inputs are stops on
// macOS too since main #327 (issues/closed/20261009-macos-controls-default-tab-order.md; driven on Custom snooze's Time and Unit),
// with Keyboard navigation on as every control is (LLP 1115.000 D7.2). These checks guard the wiring; the behavior is
// checked by driving the app on macOS: a source read cannot press Tab.
import { describe, expect, test } from 'bun:test';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { appFiles, appPath } from '../support/app-files';

const source = (file: string) => Bun.file(appPath(file)).text();
/** The lines of `component name` in `file`, up to the next top-level declaration. */
async function component(file: string, name: string): Promise<string[]> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end);
}
const testId = (line: string) => /testId=(?:"([^"]+)"|`([^`]+)`)/.exec(line)?.slice(1).find(Boolean) ?? '';

describe('dialog focus lifecycle uses the actual previous focus', () => {
  const window = contractComponent('app-window.contract', 'T3Window');
  function fixture() {
    const state: Env = { dialogFocusId: '', dialogFocusOwner: '', dialogFocusPopup: '', dialogFocusInvoker: '', dialogFrames: [], dialogSeated: [] };
    const live = new Set<string>(), menus = new Set<string>(), inert = new Set<string>(), focused: string[] = [];
    const scope = {
      none: null,
      querySelectorAll: (selector: string) => { expect(['[data-dialog-focus~="open"]', '[data-dialog-return~="menu"]']).toContain(selector); return [...(selector.includes('dialog-return') ? menus : live)].filter(id => !inert.has(id)); },
      frame: (id: string) => ({ unavailable: !live.has(id), width: live.has(id) ? 400 : 0, height: live.has(id) ? 300 : 0 }),
      dialogFrame: (owner: string, returns: { owner: string; target: string }[]) => ({ owner, returns }),
      dialogReturn: (owner: string, target: string, popup: string, invoker: string) => ({ owner, target, popup, invoker }),
      dialogLineage: (frames: { owner: string; returns: { owner: string; target: string }[] }[], owner: string) => frames.find(item => item.owner === owner)?.returns ?? [],
      focus: (id: string) => focused.push(id), stopPropagation() {},
    };
    const call = (action: string, ...args: unknown[]) => window.call(action, state, scope, args);
    const outside = (id: string) => call('windowFocused', { target: id });
    const inside = (owner: string, id: string) => call('dialogEntered', owner, { target: id });
    return { state, live, menus, inert, focused, call, outside, inside };
  }
  // Owner information comes from the popup's caller, including the environment half of task IDs.
  const menus = [
    ['pages-usage.contract', 'UsageBreadcrumb', '"usage-environment-menu"', '"usage-environment"', 'usage-environment-menu', 'usage-environment', 'usage-environment-keys', 'usage-model-prices'],
    ['timeline-plan.contract', 'PlanCard', '`plan-menu-${message.id}`', '`plan-actions-${message.id}`', 'plan-menu-one', 'plan-actions-one', 'plan-menu-one-keys', 'plan-save-one'],
    ['settings-scheduled.contract', 'TaskRow', '`task-menu-${item.id}`', '`task-actions-${item.environmentId}|${item.id}`', 'task-menu-one', 'task-actions-remote|one', 'task-menu-one-keys', 'task-edit-one'],
    ['settings-a-hosts.contract', 'HostRowView', 'host.menuId', '`device-host-options-${host.id}`', 'device-host-menu-one', 'device-host-options-one', 'device-host-menu-one-keys', 'device-host-edit-one'],
    ['connections.contract', 'EnvironmentRow', '`environment-menu-${environment.environmentId}-popup`', '`environment-menu-${environment.environmentId}`', 'environment-menu-remote-popup', 'environment-menu-remote', 'environment-menu-remote-popup-keys', 'environment-remove-remote'],
    ['connections.contract', 'EnvironmentRow', '`environment-menu-${environment.environmentId}-popup`', '`environment-menu-${environment.environmentId}`', 'environment-menu-local-popup', 'environment-menu-local', 'environment-menu-local-popup-keys', 'forget-server'],
    ['git.contract', 'R4GitMenu', '"details-git-options"', '"details-git-menu"', 'details-git-options', 'details-git-menu', 'details-git-options-keys', 'details-git-menu-commit'],
    ['pages-pr-stack.contract', 'PrdStackControls', '"pr-stack-menu"', '"pull-request-stack"', 'pr-stack-menu', 'pull-request-stack', 'pr-stack-keys', 'pr-stack-merge'],
    ['git.contract', 'R4BranchPicker', '"details-branch-menu"', '"details-branch"', 'details-branch-menu', 'details-branch', 'details-branch-search', 'details-branch-checkout-pr'],
    ['composer-controls.contract', 'ComposerMenus', '"composer-menu"', '"branch-selector"', 'composer-menu', 'branch-selector', 'branch-search', 'branch-checkout-pr'],
    ['panel.contract', 'R4AddSurface', '"r4-add-surface"', '"panel-add-surface"', 'r4-add-surface', 'panel-add-surface', 'r4-add-surface-keys', 'add-surface-device'],
    ['shell-panels.contract', 'SurfaceLauncher', '"surface-chooser"', '"surface-chooser"', 'surface-chooser', 'surface-chooser', 'surface-chooser', 'surface-device'],
  ] as const;
  for (const [file, name, popupExpr, invokerExpr, popup, invoker, first, row] of menus) {
    test(`${name} ${row}: closing returns hidden keyboard/pointer menu focus to its actual invoker`, async () => {
      const lines = (await component(file, name)).join('\n');
      if (['TaskRow', 'HostRowView', 'EnvironmentRow'].includes(name)) {
        expect(lines).toContain(`menuId=${popupExpr}, invoker=${invokerExpr}`);
        const shared = (await component(name === 'EnvironmentRow' ? 'settings-b-kit.contract' : 'settings-kit.contract', name === 'EnvironmentRow' ? 'CnMenu' : 'SkPopup')).join('\n');
        expect(shared).toContain('id=menuId data-dialog-return="menu" focusin=dialogMenuEntered(menuId, invoker)');
      } else {
        expect(lines).toContain(`dialogMenuEntered(${popupExpr}, ${invokerExpr}`);
        expect(lines).toContain('data-dialog-return=');
      }
      for (const target of [first, `${first}-first`, `${first}-last`, row, `${row}-km`]) for (const shown of [false, true]) {
        const f = fixture(); f.outside('settings-search'); f.menus.add(popup);
        f.call('dialogMenuEntered', popup, invoker, { target });
        f.live.add('opened-dialog'); f.inside('opened-dialog', 'dialog-cancel');
        expect(f.state.dialogFocusPopup).toBe(''); expect(f.state.dialogFocusInvoker).toBe('');
        f.live.clear(); if (!shown) f.menus.clear(); f.call('dialogClosed', 'opened-dialog');
        expect(f.focused).toEqual([shown ? target : invoker]);
      }
    });
  }
  test('an ordinary outside field clears menu metadata and keeps the actual pointer return', () => {
    const f = fixture(); f.call('dialogMenuEntered', 'usage-environment-menu', 'usage-environment', { target: 'usage-model-prices-km' });
    f.outside('settings-search'); f.live.add('opened-dialog'); f.inside('opened-dialog', 'dialog-cancel');
    f.live.clear(); f.call('dialogClosed', 'opened-dialog'); expect(f.focused).toEqual(['settings-search']);
  });
  test('a menu within a live parent dialog preserves ancestry through child and parent closure', () => {
    const f = fixture(); f.outside('settings-search'); f.live.add('provider-dialog'); f.inside('provider-dialog', 'provider-field');
    f.call('dialogMenuEntered', 'provider-menu', 'provider-options', { target: 'provider-signout-km' });
    f.live.add('sign-out-confirm'); f.inside('sign-out-confirm', 'sign-out-cancel');
    f.live.delete('sign-out-confirm'); f.call('dialogClosed', 'sign-out-confirm'); expect(f.focused).toEqual(['provider-options']);
    f.inside('provider-dialog', 'provider-options'); f.live.clear(); f.call('dialogClosed', 'provider-dialog');
    expect(f.focused).toEqual(['provider-options', 'settings-search']);
  });
  test('a page menu after an empty-return dialog clears its removed owner before the next snapshot', () => {
    const f = fixture(); f.live.add('first-dialog'); f.inside('first-dialog', 'first-field');
    f.live.clear(); f.call('dialogClosed', 'first-dialog'); expect(f.focused).toEqual([]);
    f.call('dialogMenuEntered', 'task-menu-one', 'task-actions-remote|one', { target: 'task-edit-one-km' });
    expect(f.state.dialogFocusOwner).toBe('');
    f.live.add('next-dialog'); f.inside('next-dialog', 'next-field'); f.live.clear(); f.call('dialogClosed', 'next-dialog');
    expect(f.focused).toEqual(['task-actions-remote|one']);
  });
  test('a menu entry records context without redirecting focus during direct-focus and bubbling-focusin delivery', () => {
    const f = fixture(); f.outside('settings-search');
    // KeyMenu can redirect from its first/last sentinel during direct focus, before the original focusin bubbles.
    for (const target of ['usage-model-prices-km', 'usage-environment-keys-first']) f.call('dialogMenuEntered', 'usage-environment-menu', 'usage-environment', { target });
    expect(f.focused).toEqual([]);
    f.live.add('usage-prices-dialog'); f.inside('usage-prices-dialog', 'usage-prices-apply');
    f.live.clear(); f.call('dialogClosed', 'usage-prices-dialog'); expect(f.focused).toEqual(['usage-environment']);
  });
  // Each case binds the real observer, popup, and dismiss path before reusing the shared lifecycle transition.
  const mapped = [
    ['Project picker', 'app-overlays.contract', 'project-dialog', 'projectsOpen', 'ProjectPicker', 'app-overlays.contract', '"project-dialog"', 'project-dialog-add', 'press=closeModal'],
    ['Image preview', 'app-overlays.contract', 'image-preview', '(data.imagePreviewId != "")', 'ImagePreviewDialog', 'timeline-attachments.contract', '"image-preview"', 'image-preview-close', 'local("image-close", "", "")'],
    ['Video preview', 'app-overlays.contract', 'video-preview', '(data.composerVideoPreview.length > 0)', 'VideoPreviewDialog', 'composer-video.contract', '"video-preview"', 'video-preview-close', 'editorlocal:r4c-video-close'],
    ['Chip image preview', 'app-overlays.contract', 'chip-image-preview', '(data.composerImagePreview.length > 0)', 'ComposerImageDialog', 'composer-chip-image.contract', '"chip-image-preview"', 'chip-image-close', 'editorlocal:chip-image-close'],
    ['Chip file preview', 'app-overlays.contract', 'chip-file-preview', 'shell.composerFile.open', 'ComposerFileDialog', 'composer-chip-file.contract', '"chip-file-preview"', 'chip-file-preview-copy', 'command("editorlocal:chip-file", "close"'],
    ['Diagram preview', 'app-overlays.contract', 'diagram-preview', '(data.diagramPreview.length > 0)', 'DiagramPreviewDialog', 'timeline-mermaid.contract', '"diagram-preview"', 'diagram-preview-close', 'diagram-close'],
    ['PR checkout', 'app-overlays.contract', 'pr-checkout-dialog', 'shell.prCheckout.open', 'R9CheckoutDialog', 'pr-checkout.contract', '"pr-checkout-dialog"', 'pr-checkout-input', 'git-pr-close'],
    ['Device setup', 'app-overlays.contract', 'device-setup', 'shell.panel.deviceSetup', 'R4DeviceDialog', 'panel.contract', '"device-setup"', 'device-setup-step-0', 'surface-setup-close'],
    ['Legacy rename', 'legacy-sidebar.contract', 'legacy-rename-project', '(data.legacy.dialog == "rename-project")', 'LegacyDialogFrame', 'legacy-sidebar.contract', 'label', 'legacy-project-title', 'legacy-dialog-close'],
    ['Legacy grouping', 'legacy-sidebar.contract', 'legacy-grouping', '(data.legacy.dialog == "grouping")', 'LegacyDialogFrame', 'legacy-sidebar.contract', 'label', 'legacy-grouping-rule', 'legacy-dialog-close'],
    ['Welcome', 'app-overlays.contract', 'welcome-dialog', 'welcome.show', 'WelcomeWizard', 'pages-welcome.contract', '"welcome-dialog"', 'welcome-step-1', 'welcomeFinish'],
    ['ChatGPT plan', 'app-settings.contract', 'chatgpt-plan-dialog', '(data.chatgptPlan.length > 0)', 'ChatGptPlanDialog', 'codex-setup.contract', '"chatgpt-plan-dialog"', 'chatgpt-plan-usage-button', 'setup:chatgpt-plan-ack'],
    ['Pairing reveal', 'connections-network-dialogs.contract', 'pairing-reveal-dialog', 'net.reveal.open', 'PairingRevealDialog', 'connections-network-dialogs.contract', '"pairing-reveal-dialog"', 'pairing-reveal-value', 'pairing-reveal-close'],
    ['Legacy remove', 'legacy-sidebar.contract', 'legacy-remove-project', '(data.legacy.dialog == "remove-project")', 'SettingsConfirm', 'settings-rest-dialogs.contract', 'dialogId', 'legacy-remove-project-cancel', 'legacy-dialog-close'],
    ['Legacy archive', 'legacy-sidebar.contract', 'legacy-archive-many', '(data.legacy.dialog == "archive-many")', 'SettingsConfirm', 'settings-rest-dialogs.contract', 'dialogId', 'legacy-archive-many-cancel', 'legacy-dialog-close'],
    ['Archive delete retained route', 'app-settings.contract', 'archive-delete', '(restEditor == "archive-delete")', 'SettingsConfirm', 'settings-rest-dialogs.contract', 'dialogId', 'archive-delete-cancel', 'cancel=close'],
    ['Signal kill', 'app-settings.contract', 'signal-kill', '(settingsOpen and settingsCore.killConfirm.pid != "")', 'SettingsConfirm', 'settings-rest-dialogs.contract', 'dialogId', 'signal-kill-cancel', 'rest:diag-kill-cancel'],
    ['Archived row delete', 'app-settings.contract', 'archive-context-delete', '(settingsOpen and settingsCore.archiveConfirm.id != "")', 'SettingsConfirm', 'settings-rest-dialogs.contract', 'dialogId', 'archive-context-delete-cancel', 'rest:archive-cancel'],
    ['Server update', 'app-overlays.contract', 'server-update-confirm', '(data.serverUpdate.confirmTitle != "")', 'AppConfirm', 'shell-panels.contract', 'confirmId', 'server-update-confirm-cancel', 'su:cancel'],
  ] as const;
  for (const [label, observerFile, owner, opening, popupComponent, popupFile, binding, initial, closing] of mapped) {
    test.each(['keyboard', 'pointer'])(`${label}: %s return is bound to its real open observer and popup`, async input => {
      const observer = await source(observerFile), popup = (await component(popupFile, popupComponent)).join('\n');
      expect(observer).toContain(`DialogFinalFocus(open=${opening}, owner="${owner}")`);
      expect(popup).toContain(`id=${binding}`); expect(popup).toContain(`focusin=dialogEntered(${binding})`);
      expect(popup).toContain('data-dialog-focus="open"');
      const callers = (await Promise.all(['app-overlays.contract', 'app-settings.contract', 'legacy-sidebar.contract', 'settings-a-dialogs.contract', 'settings-rest-dialogs.contract', popupFile].map(source))).join('\n');
      expect(callers).toContain(closing);
      if (binding === 'dialogId') expect(callers).toContain(`dialogId="${owner}"`);
      if (binding === 'confirmId') expect(callers).toContain(`confirmId="${owner}"`);
      if (binding === 'label') expect(callers).toContain(`label="${owner}"`);
      expect(popup).toMatch(/(?:DialogOpenFocus|FieldOpenSelect)\(target=/);
      const f = fixture(), before = input === 'keyboard' ? `keyboard-${owner}` : 'settings-search';
      const sentinel = contractComponent('dialog-trap.contract', 'DialogFinalFocus'), sentinelState = { shown: false, heldOwner: '' };
      const scope = { owner, dialogOpened: (id: string) => f.call('dialogOpened', id), dialogClosed: (id: string) => f.call('dialogClosed', id) };
      f.outside(before); f.live.add(owner); sentinel.call('laidOut', sentinelState, scope, [2, 1]); f.inside(owner, initial);
      f.inside(owner, `${owner}-edited`); sentinel.call('laidOut', sentinelState, scope, [2, 1]);
      f.live.clear(); sentinel.call('laidOut', sentinelState, scope, [1, 1]);
      expect(f.focused).toEqual([before]); expect(f.state.dialogFrames).toEqual([]);
    });
  }
  test.each(['layout-first', 'focus-first'])('%s snapshots once before initial focus, edits, refreshes and resize', order => {
    const f = fixture();
    f.outside('settings-search'); f.live.add('provider-dialog');
    if (order === 'layout-first') f.call('dialogOpened', 'provider-dialog');
    f.inside('provider-dialog', 'add-provider-step-0');
    f.call('dialogOpened', 'provider-dialog');
    f.inside('provider-dialog', 'provider-name');
    f.call('dialogOpened', 'provider-dialog');
    expect(f.state.dialogFrames).toEqual([{ owner: 'provider-dialog', returns: [{ owner: '', target: 'settings-search', popup: '', invoker: '' }] }]);
    f.live.clear(); f.call('dialogClosed', 'provider-dialog');
    expect(f.focused).toEqual(['settings-search']);
  });
  test('keyboard and pointer openings have different prior focus; reopening takes a new snapshot', () => {
    const f = fixture();
    for (const prior of ['add-provider', 'settings-search']) {
      f.outside(prior); f.live.add('provider-dialog'); f.inside('provider-dialog', 'provider-name');
      f.live.clear(); f.call('dialogClosed', 'provider-dialog');
    }
    expect(f.focused).toEqual(['add-provider', 'settings-search']);
  });
  test('empty focus is valid and an explicit outside destination wins', () => {
    const f = fixture(); f.live.add('project-dialog'); f.inside('project-dialog', 'project-dialog-add');
    f.live.clear(); f.call('dialogClosed', 'project-dialog');
    expect(f.focused).toEqual([]);
    f.outside('settings-search'); f.live.add('project-dialog'); f.inside('project-dialog', 'project-dialog-add');
    f.outside('new-page-search'); f.live.clear(); f.call('dialogClosed', 'project-dialog');
    expect(f.focused).toEqual([]); expect(f.state.dialogFrames).toEqual([]);
  });
  test('a nested confirm returns inside the wizard, which later returns outside', () => {
    const f = fixture(); f.outside('settings-search'); f.live.add('provider-dialog'); f.inside('provider-dialog', 'wizard-sign-out');
    f.live.add('sign-out-confirm'); f.inside('sign-out-confirm', 'sign-out-cancel');
    f.live.delete('sign-out-confirm'); f.call('dialogClosed', 'sign-out-confirm');
    expect(f.focused).toEqual(['wizard-sign-out']);
    f.inside('provider-dialog', 'wizard-sign-out'); f.live.clear(); f.call('dialogClosed', 'provider-dialog');
    expect(f.focused).toEqual(['wizard-sign-out', 'settings-search']);
  });
  test('an inert parent remains the nested child\'s owner even though queries exclude it', () => {
    const f = fixture(); f.outside('settings-search'); f.live.add('provider-dialog'); f.inside('provider-dialog', 'wizard-sign-out');
    f.inert.add('provider-dialog'); f.live.add('sign-out-confirm'); f.inside('sign-out-confirm', 'sign-out-cancel');
    expect(f.state.dialogFrames).toEqual([
      { owner: 'provider-dialog', returns: [{ owner: '', target: 'settings-search', popup: '', invoker: '' }] },
      { owner: 'sign-out-confirm', returns: [{ owner: '', target: 'settings-search', popup: '', invoker: '' }, { owner: 'provider-dialog', target: 'wizard-sign-out', popup: '', invoker: '' }] },
    ]);
    f.live.delete('sign-out-confirm'); f.inert.clear(); f.call('dialogClosed', 'sign-out-confirm');
    expect(f.focused).toEqual(['wizard-sign-out']);
  });
  test('blur toward an anonymous initial control cannot erase a newly opening dialog\'s prior focus', () => {
    const f = fixture(); f.outside('settings-search'); f.live.add('provider-dialog');
    f.call('windowBlurred', { target: 'settings-search', relatedTarget: null });
    f.inside('provider-dialog', ''); f.call('dialogOpened', 'provider-dialog');
    f.live.clear(); f.call('dialogClosed', 'provider-dialog'); expect(f.focused).toEqual(['settings-search']);
    f.outside('settings-search'); f.call('windowBlurred', { target: 'settings-search', relatedTarget: null });
    expect(f.state.dialogFocusId).toBe('');
  });
  test('removing a parent while a child remains transfers the original outside return', () => {
    const f = fixture(); f.outside('settings-search'); f.live.add('provider-dialog'); f.inside('provider-dialog', 'wizard-sign-out');
    f.live.add('sign-out-confirm'); f.inside('sign-out-confirm', 'sign-out-cancel');
    f.live.delete('provider-dialog'); f.call('dialogClosed', 'provider-dialog');
    expect(f.focused).toEqual([]);
    f.live.clear(); f.call('dialogClosed', 'sign-out-confirm'); expect(f.focused).toEqual(['settings-search']);
  });
  test.each([['sign-out-confirm', 'provider-dialog'], ['provider-dialog', 'sign-out-confirm']])('simultaneous removal in %s then %s order returns outside once', (first, last) => {
    const f = fixture(); f.outside('settings-search'); f.live.add('provider-dialog'); f.inside('provider-dialog', 'wizard-sign-out');
    f.live.add('sign-out-confirm'); f.inside('sign-out-confirm', 'sign-out-cancel'); f.live.clear();
    f.call('dialogClosed', first); f.call('dialogClosed', last);
    expect(f.focused).toEqual(['settings-search']); expect(f.state.dialogFrames).toEqual([]);
  });
  test('closing the parent restores focus when its nested observer unmounts without a closing resize', () => {
    const f = fixture(); f.outside('settings-search'); f.live.add('provider-dialog'); f.inside('provider-dialog', 'wizard-sign-out');
    f.live.add('sign-out-confirm'); f.inside('sign-out-confirm', 'sign-out-cancel'); f.live.clear();
    f.call('dialogClosed', 'provider-dialog');
    expect(f.focused).toEqual(['settings-search']); expect(f.state.dialogFrames).toEqual([]);
  });
  for (const closeOrder of [['sign-out-confirm', 'provider-dialog'], ['provider-dialog', 'sign-out-confirm']]) {
    test.each(['before-close', 'after-close', 'registered-before-close'])(`nested chain replacement %s, close order ${closeOrder.join(' then ')}`, timing => {
      const f = fixture(); f.outside('settings-search'); f.live.add('provider-dialog'); f.inside('provider-dialog', 'wizard-sign-out');
      f.live.add('sign-out-confirm'); f.inside('sign-out-confirm', 'sign-out-cancel'); f.live.clear(); f.live.add('next-dialog');
      if (timing === 'before-close') f.inside('next-dialog', 'next-field');
      if (timing === 'registered-before-close') f.call('dialogOpened', 'next-dialog');
      for (const owner of closeOrder) f.call('dialogClosed', owner);
      expect(f.focused).toEqual([]);
      f.inside('next-dialog', 'next-field'); f.live.clear(); f.call('dialogClosed', 'next-dialog');
      expect(f.focused).toEqual(['settings-search']); expect(f.state.dialogFrames).toEqual([]);
    });
  }
  test.each(['before-close', 'after-close', 'registered-before-close'])('replacement %s inherits return without a competing focus', timing => {
    const f = fixture(); f.outside('project-search'); f.live.add('project-dialog'); f.inside('project-dialog', 'project-dialog-add');
    f.live.delete('project-dialog'); f.live.add('provider-dialog');
    if (timing === 'before-close') f.inside('provider-dialog', 'provider-name');
    if (timing === 'registered-before-close') f.call('dialogOpened', 'provider-dialog');
    f.call('dialogClosed', 'project-dialog'); expect(f.focused).toEqual([]);
    f.inside('provider-dialog', 'provider-name');
    f.live.clear(); f.call('dialogClosed', 'provider-dialog'); expect(f.focused).toEqual(['project-search']);
  });
});

describe('invisible buttons are never Tab stops', () => {
  test('every pressable of 0 or 1pt (a key, a chord or a hatch press) says tabindex=-1', async () => {
    const files = appFiles().filter(name => name.endsWith('.contract'));
    const offenders: string[] = [];
    let found = 0;
    for (const file of files) {
      (await source(file)).split('\n').forEach((line, index) => {
        if (!/^\s*[a-z][\w-]*\b/.test(line) || !/\bpress=/.test(line) || !/\swidth=[01](\s|$)/.test(line) || !/\sheight=[01](\s|$)/.test(line)) return;
        found++;
        if (!/\btabindex=-1\b/.test(line)) offenders.push(`${file}:${index + 1} ${testId(line)}`);
      });
    }
    // The keyboard dispatch's two kinds, the composer's hatch keys, the palette's, Usage's, Settings' and the
    // sidebar's chords, and a surface tab's rename-cancel anchor.
    expect(found).toBeGreaterThanOrEqual(23);
    expect(offenders).toEqual([]);
  });
});

// These run the authored handlers. Source inventory above proves which real controls use them; native evidence is separate.
describe('the repaired control behaviors', () => {
  const plain = { shiftKey: false, metaKey: false, ctrlKey: false, altKey: false };
  const confirms = [
    ['shell-panels.contract', 'AppConfirm', 'fromCancel', 'fromConfirm', 'app-confirm-cancel', 'app-confirm-confirm', 'app-confirm', { confirmId: 'app-confirm' }],
    ['settings-rest-dialogs.contract', 'SettingsConfirm', 'fromCancel', 'fromConfirm', 'signal-kill-cancel', 'signal-kill-confirm', 'signal-kill', { dialogId: 'signal-kill' }],
    ['usage-bars.contract', 'ResetCreditDialog', 'fromCancel', 'fromConfirm', 'reset-credit-cancel', 'reset-credit-confirm', 'reset-credit-dialog', {}],
    ['codex-setup.contract', 'ChatGptPlanDialog', 'firstKeys', 'lastKeys', 'chatgpt-plan-usage-button', 'chatgpt-plan-continue-button', 'chatgpt-plan-usage-button', {}],
  ] as const;
  for (const [file, name, firstAction, lastAction, first, last, busyFallback, props] of confirms) {
    test(`${name} consumes both Tab directions from both controls and refuses modifier chords`, () => {
      const c = contractComponent(file, name);
      for (const busy of [false, true]) for (const shiftKey of [false, true]) for (const [action, next] of [[firstAction, last], [lastAction, first]]) {
        const focus: string[] = [], events: string[] = [];
        const scope = { ...props, busy, focus: (id: string) => focus.push(id), preventDefault: () => events.push('prevent'), stopPropagation: () => events.push('stop') };
        c.call(action!, {}, scope, ['Tab', { ...plain, shiftKey }]);
        expect(focus).toEqual([busy ? busyFallback : next]); expect(events.sort()).toEqual(['prevent', 'stop']);
        focus.length = 0; events.length = 0;
        for (const modifier of ['metaKey', 'ctrlKey', 'altKey']) c.call(action!, {}, scope, ['Tab', { ...plain, [modifier]: true }]);
        c.call(action!, {}, scope, ['Enter', plain]); expect(focus).toEqual([]); expect(events).toEqual([]);
    }
  });
  }

  test('the fallback popup holds both Tab directions and retries newly enabled controls', () => {
    const window = contractComponent('app-window.contract', 'T3Window');
    const focused: string[] = [], events: string[] = [], enabled: string[] = [];
    const scope = { querySelectorAll: () => enabled, focus: (id: string) => focused.push(id), preventDefault: () => events.push('prevent'), stopPropagation: () => events.push('stop') };
    for (const shiftKey of [false, true]) window.call('dialogFallback', { dialogFocusId: 'popup' }, scope, ['popup', ['first', 'last'], 'Tab', { ...plain, shiftKey }]);
    expect(focused).toEqual(['popup', 'popup']);
    enabled.push('first', 'last'); focused.length = 0;
    for (const shiftKey of [false, true]) window.call('dialogFallback', { dialogFocusId: 'popup' }, scope, ['popup', ['first', 'last'], 'Tab', { ...plain, shiftKey }]);
    expect(focused).toEqual(['first', 'last']);
    focused.length = 0;
    window.call('dialogFallback', { dialogFocusId: 'field' }, scope, ['popup', ['first', 'last'], 'Tab', plain]);
    expect(focused).toEqual([]);
  });
  test('the visible icon type group is one Tab stop; either arrow selects the adjacent mode and retains group focus', async () => {
    const c = contractComponent('settings-b-icons.contract', 'ProjectIconEditor');
    const modes = ['Icons', 'Emoji', 'Monogram'], values = ['lucide', 'emoji', 'monogram'];
    for (const [index, label] of modes.entries()) for (const direction of [-1, 1]) {
      const state = { mode: values[index] }, focus: string[] = [], events: string[] = [];
      c.call('modeKey', state, { focus: (id: string) => focus.push(id), preventDefault: () => events.push('prevent'), stopPropagation: () => events.push('stop') }, [label, direction === 1 ? 'ArrowRight' : 'ArrowLeft', plain]);
      const next = (index + direction + 3) % 3;
      expect(state.mode).toBe(values[next]); expect(focus).toEqual(['project-icon-mode']); expect(events).toEqual(['prevent', 'stop']);
    }
    const toggle = (await component('settings-b-icons.contract', 'SbToggle')).join('\n');
    expect(toggle).not.toContain('tabindex='); expect(toggle).not.toContain('key=navigate'); expect(toggle).not.toContain('data-focus-stop=');
    const editor = await component('settings-b-icons.contract', 'ProjectIconEditor');
    const group = editor.find(line => line.includes('role="radiogroup"'))!;
    expect(group).toContain('id="project-icon-mode" tabindex=0 key=modeKey(modeLabel)');
    expect(group).toContain('data-focus-stop="enabled"');
    expect(editor).toContain('        DialogOpenFocus(target="project-icon-mode")');
    const targets = editor.find(line => line.startsWith('  derive dialogTargets ='))!.split(' = ')[1];
    expect(value(targets, {})).toEqual(['project-icon-mode', 'project-icon-close']);
    const label = editor.find(line => line.startsWith('  derive modeLabel ='))!.split(' = ')[1];
    for (const [index, mode] of values.entries()) expect(value(label, { mode })).toBe(modes[index]);
    for (const key of ['Tab', 'Enter', 'ArrowRight']) for (const modifier of ['metaKey', 'ctrlKey', 'altKey']) {
      const state = { mode: 'lucide' }, focus: string[] = [], events: string[] = [];
      c.call('modeKey', state, { focus: (id: string) => focus.push(id), preventDefault: () => events.push('prevent'), stopPropagation: () => events.push('stop') }, ['Icons', key, { ...plain, [modifier]: true }]);
      expect(state.mode).toBe('lucide'); expect(focus).toEqual([]); expect(events).toEqual([]);
    }
  });
  test('Machines saves its trigger before native opening, focuses the first enabled copy and preserves deliberate outside focus', async () => {
    const c = contractComponent('auto-balance.contract', 'NoticeMachines');
    const state = { inside: false, over: false }, focus: string[] = [];
    const notice = { id: 'updates', machines: [{ id: 'running', status: 'running', copyLabel: 'Copy' }, { id: 'empty', status: 'idle', copyLabel: '' }, { id: 'one', status: 'idle', copyLabel: 'Copy' }, { id: 'two', status: 'idle', copyLabel: 'Copy' }] };
    const scope = { notice, busy: false, dialogFocusId: 'notice-machine-copy-one', none: null, some: (id: string) => id, focus: (id: string) => focus.push(id), ...c.derives(state, { notice }) };
    expect(scope.copyIds).toEqual(['notice-machine-copy-one', 'notice-machine-copy-two']);
    // PresenterMac.press runs the authored action before the native popover
    // command captures its previous responder. Escape restores that responder
    // before the closed toggle arrives, so focusout must not decide its owner.
    c.call('invoked', state, scope, []);
    expect(focus).toEqual(['notice-machines-trigger-updates']);
    const nativePrevious = focus.at(-1)!;
    c.call('toggled', state, scope, [{ newState: 'open' }]); c.call('entered', state, scope, []);
    c.call('left', state, scope, [{ relatedTarget: nativePrevious }]);
    c.call('toggled', state, { ...scope, dialogFocusId: nativePrevious }, [{ newState: 'closed' }]);
    expect(focus).toEqual([nativePrevious, 'notice-machine-copy-one']);
    focus.length = 0;
    for (let i = 0; i < 2; i++) {
      c.call('toggled', state, scope, [{ newState: 'open' }]); c.call('entered', state, scope, []);
      c.call('toggled', state, scope, [{ newState: 'closed' }]);
    }
    expect(focus).toEqual(['notice-machine-copy-one', 'notice-machines-trigger-updates', 'notice-machine-copy-one', 'notice-machines-trigger-updates']);
    focus.length = 0;
    c.call('left', state, scope, [{ relatedTarget: 'settings-search' }]);
    c.call('toggled', state, { ...scope, dialogFocusId: 'settings-search' }, [{ newState: 'closed' }]); expect(focus).toEqual([]);
    c.call('toggled', state, { ...scope, busy: true }, [{ newState: 'open' }]);
    c.call('toggled', state, { ...scope, copyIds: [] }, [{ newState: 'open' }]);
    expect(focus).toEqual(['notice-machines-updates', 'notice-machines-updates']);
    const trigger = (await component('auto-balance.contract', 'NoticeMachines')).find(line => line.includes('button id=`notice-machines-trigger-'))!;
    expect(trigger).toContain('press=invoked');
    expect(trigger).toContain('popovertarget=`notice-machines-${notice.id}`');
  });
});

describe('post-created provider traversal follows the Sign in content', () => {
  test('discovery, signed out, active auth, signed in, credentials and no-account docs choose the first rendered enabled control', async () => {
    const row = contractComponent('providers-setup.contract', 'ProviderAccountRow'), authStep = contractComponent('providers-setup.contract', 'ProviderWizardAuthStep');
    const rowLines = await component('providers-setup.contract', 'ProviderAccountRow');
    const wizardLines = await component('providers-wizard.contract', 'AddProviderDialog');
    const expr = (name: string) => wizardLines.find(line => line.startsWith(`  derive ${name} =`))!.split(' = ').slice(1).join(' = ');
    const base = { instanceId: 'example', methods: [], credentials: [], email: '', pickMethod: false, url: '', docsUrl: '', showCancel: false, showStart: false, startDisabled: false, showSignOut: false, signOutDisabled: false, details: false, disabled: false, deviceCode: '', terminal: false, callback: false, error: '' };
    const cases = [
      { name: 'signed out', patch: { showStart: true }, first: 'wizard-provider-sign-in-example' },
      { name: 'active browser auth', patch: { url: 'https://example.test/auth', showCancel: true }, first: 'wizard-provider-auth-open-example' },
      { name: 'signed in', patch: { email: 'example@example.test', showStart: true, showSignOut: true }, first: 'wizard-provider-email-example' },
      { name: 'method choice', patch: { pickMethod: true, methods: [{ id: 'one', name: 'One' }] }, first: 'wizard-provider-auth-method-example' },
      { name: 'disabled sign in', patch: { showStart: true, startDisabled: true }, first: 'wizard-finish' },
      { name: 'credentials', patch: { details: true, credentials: [{ name: 'token', index: 0, label: 'Token', secret: true }] }, first: 'wizard-provider-auth-credential-example-token' },
    ];
    for (const sample of cases) {
      const account = { ...base, ...sample.patch }, state = { methodId: '' }, scope = { account, busy: false, idPrefix: 'wizard-', wide: true, KmItem: (item: { id: string; label: string }) => item };
      const env = { ...scope, ...state, ...row.derives(state, scope) }, enabled: string[] = [];
      for (const line of rowLines) {
        const id = /testId=(`[^`]+`)/.exec(line)?.[1];
        if (!id || !/^\s*(?:button|link|SetupButton\(|RedactedText\()/u.test(line)) continue;
        if (!row.drawn(`testId=${id}`, state, scope)) continue;
        const disabled = /disabled=(\([^)]*\)|[A-Za-z][A-Za-z0-9]*)/.exec(line)?.[1];
        if (!disabled || !value(disabled, env)) enabled.push(value(id, env) as string);
      }
      const credentials = account.credentials.map(field => `wizard-provider-auth-credential-example-${field.name}`);
      if (account.details && !account.disabled) enabled.push(...credentials);
      enabled.push('wizard-finish', 'close-add-provider-x');
      const wizard = { created: 'example' }, authControls = value(expr('authControls'), { wizard });
      const targets = value(expr('dialogTargets'), { wizard, credentials, authControls });
      const focused: string[] = [];
      contractComponent('dialog-trap.contract', 'FocusGuard').call('enter', {}, { targets, popup: 'add-provider-dialog', start: false, stopPropagation() {}, querySelectorAll: (selector: string) => selector.includes('dialog-focus') ? ['add-provider-dialog'] : enabled, focus: (id: string) => focused.push(id) }, []);
      expect([sample.name, focused]).toEqual([sample.name, [sample.first]]);
    }
    for (const discovering of [true, false]) {
      const auth = { instanceId: 'example', account: [], discovering, docsUrl: 'https://example.test/docs' };
      expect(authStep.drawn('link href=auth.docsUrl', {}, { auth })).toBe(!discovering);
      const wizard = { created: 'example' }, authControls = value(expr('authControls'), { wizard }), targets = value(expr('dialogTargets'), { wizard, credentials: [], authControls }) as string[];
      const enabled = discovering ? ['wizard-finish', 'close-add-provider-x'] : ['wizard-provider-auth-docs-example', 'wizard-finish', 'close-add-provider-x'];
      expect(targets.filter(id => enabled.includes(id))[0]).toBe(discovering ? 'wizard-finish' : 'wizard-provider-auth-docs-example');
    }
    const setup = await source('providers-setup.contract');
    expect(setup).toContain('ProviderCredentialsForm(account=account, off=off, op=op, idPrefix=idPrefix)');
    expect(setup).toContain('id=`${idPrefix}provider-auth-credential-${account.instanceId}-${field.name}`');
    expect(setup).toContain('id=`${idPrefix}provider-callback-${account.instanceId}`');
    expect(setup).toContain('button id=testId data-focus-stop=(disabled ? "" : "enabled")');
  });
});
