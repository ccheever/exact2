// dialog-trap-and-progress-value (DT-1): every modal dialog keeps Tab and Shift+Tab inside, as Base UI's focus trap does
// in each Dialog and AlertDialog of T3 Code 1e2ecbd975. Exact keeps no Tab inside an `aria-modal` box on macOS
// (exact2 #282), so each dialog carries a trap: the FocusGuard pair of dialog-trap.contract, or the `key`
// handlers the older dialogs wrap with (20261008-dialog-shortcut-focus). These checks read the Contract sources, as
// dialog-focus.test.ts does: every `role="dialog"`/`"alertdialog"` is classified (a new one fails until it is), each
// guarded popup opens and closes with its pair, and every literal a guard names is an element id. The behavior is proven
// by the macOS drives in the 20261011-dialog-trap-and-progress-value record on the feat(example)/t3-code branch.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';

const dir = new URL('./', import.meta.url);
const files = readdirSync(dir).filter(name => name.endsWith('.contract')).sort();
const sources = new Map<string, string>(await Promise.all(files.map(async file => [file, await Bun.file(new URL(`./${file}`, import.meta.url)).text()] as const)));
const source = (file: string) => sources.get(file)!;
const indent = (line: string) => line.length - line.trimStart().length;
/** The lines of the block that `line` (an index into `lines`) opens: its children, comments and blanks included. */
function block(lines: string[], at: number): string[] {
  let end = at + 1;
  while (end < lines.length && (lines[end]!.trim() === '' || indent(lines[end]!) > indent(lines[at]!))) end++;
  return lines.slice(at + 1, end);
}
/** A dialog's key: its file and its testId expression (or its label when it has none). */
function dialogKey(file: string, line: string): string {
  const testId = /testId=("[^"]*"|`[^`]*`|[A-Za-z.]+)/.exec(line)?.[1];
  const label = /aria-label=("[^"]*"|`[^`]*`|[A-Za-z.]+)/.exec(line)?.[1];
  return `${file} ${testId ?? label ?? '?'}`;
}
const dialogs = files.flatMap(file => source(file).split('\n').flatMap((line, index) =>
  !line.trimStart().startsWith('//') && /role="(dialog|alertdialog)"/.test(line) ? [{ file, index, line, key: dialogKey(file, line) }] : []));

// The FocusGuard pair of each guarded dialog: its key and the guard ids' base.
const GUARDED: Record<string, string> = {
  'app-overlays.contract "project-dialog"': 'project-dialog',
  'browser-profiles.contract "browser-profile-remove-dialog"': 'browser-profile-remove',
  'browser-profiles.contract `browser-import-step-${wizard.step}`': 'browser-import',
  'connections-routes.contract "route-remove-dialog"': 'route-remove',
  'connections.contract "environment-remove-dialog"': 'environment-remove',
  'legacy-sidebar.contract label': 'legacy-dialog',
  'pages-usage-dialogs.contract dialogId': 'usage-dialog',
  'pages-welcome.contract "welcome-dialog"': 'welcome',
  'palette.contract view.testId': 'palette', // CommandPaletteBody, then PaletteDialog (link-pr), below
  'providers-wizard.contract "add-provider-dialog"': 'add-provider',
  'providers-wizard.contract "usage-hub-dialog"': 'usage-hub',
  'providers-wizard.contract "usage-hub-remove-dialog"': 'usage-hub-remove',
  'r4-composer.contract "video-preview"': 'video-preview',
  'r4-git.contract testId': 'r4-dialog',
  'r4-surfaces.contract "device-setup"': 'device-setup',
  'r6-pr.contract "pr-merge-dialog"': 'pr-merge',
  'r9-connect.contract "pr-checkout-dialog"': 'pr-checkout',
  'settings-a-collections.contract "theme-remove-collection"': 'theme-remove-collection',
  'settings-a-dialogs.contract dialogId': 'sa-dialog',
  'settings-a-hosts.contract "device-host-dialog"': 'device-host',
  'settings-appearance-import.contract "add-theme-dialog"': 'add-theme',
  'settings-b-actions.contract "project-action-dialog"': 'project-action',
  'settings-b-actions.contract "action-delete-dialog"': 'action-delete',
  'settings-b-icons.contract "project-icon-dialog"': 'project-icon',
  'settings-b-icons.contract "project-favicon-picker"': 'project-favicon',
  'settings-core-body.contract "restore-defaults-dialog"': 'restore-defaults',
  'settings-scheduled.contract "scheduled-task-dialog"': 'scheduled-task',
  'snapshot.contract "snapshot-setup-dialog"': 'snapshot-setup',
  'ssh-prompt.contract "ssh-password-dialog"': 'ssh-password',
  'timeline-attachments.contract "image-preview"': 'image-preview',
  'timeline-mermaid.contract "diagram-preview"': 'diagram-preview',
  'timeline-plan.contract "plan-save-dialog"': 'plan-save',
  'timeline.contract "revert-dialog"': 'revert',
};
// The dialogs that wrap with their own `key` handlers (20261008-dialog-shortcut-focus and the tasks after it): the
// component and the focus each wrap sends.
const KEYED: Record<string, { component: string; wraps: string[] }> = {
  'codex-setup.contract "codex-picker"': { component: 'ChatGptAccountPicker', wraps: ['focus("codex-picker-connect-button")', 'focus(`codex-picker-option-${picker.first}`)'] },
  'codex-setup.contract "chatgpt-plan-dialog"': { component: 'ChatGptPlanDialog', wraps: ['focus("chatgpt-plan-continue-button")', 'focus("chatgpt-plan-usage-button")'] },
  'connections-network-dialogs.contract "network-access-dialog"': { component: 'NetworkAccessDialog', wraps: ['focus("network-access-confirm")', 'focus("network-access-cancel")'] },
  'connections-network-dialogs.contract "tailscale-disable-dialog"': { component: 'TailscaleDisableDialog', wraps: ['focus("tailscale-disable-confirm")', 'focus("tailscale-disable-cancel")'] },
  'connections-network-dialogs.contract "tailscale-setup-dialog"': { component: 'TailscaleSetupDialog', wraps: ['focus("tailscale-setup-close")', 'focus("tailscale-port")'] },
  'connections-network-dialogs.contract "create-pairing-dialog"': { component: 'CreatePairingDialog', wraps: ['focus("pairing-create-close")', 'focus("pairing-label")'] },
  'connections-network-dialogs.contract "pairing-reveal-dialog"': { component: 'PairingRevealDialog', wraps: ['focus("pairing-reveal-close")', 'focus("pairing-reveal-value")'] },
  'connections.contract "connection-dialog"': { component: 'AddEnvironmentDialog', wraps: ['focus("connection-mode-remote")'] }, // ModeCard wraps the other way
  'pages-pr-actions.contract "pr-action-dialog"': { component: 'PrActionDialog', wraps: ['focus("pr-action-dialog-confirm")', 'focus("pr-action-dialog-cancel")'] },
  'pages-pr-stack.contract "pr-stack-dialog"': { component: 'PsDialogPopup', wraps: ['focus(e.shiftKey ? "pr-stack-dialog-confirm" : "pr-stack-dialog-cancel")'] },
  'providers-wizard.contract "chatgpt-account-dialog"': { component: 'ChatGptAccountDialog', wraps: ['focus("chatgpt-account-continue-button")', 'focus("chatgpt-account-name-field")'] },
  'settings-rest-dialogs.contract dialogId': { component: 'SettingsConfirm', wraps: ['focus(`${dialogId}-confirm`)', 'focus(`${dialogId}-cancel`)'] },
  'shell-panels.contract "app-confirm"': { component: 'AppConfirm', wraps: ['focus(`${confirmId}-confirm`)', 'focus(`${confirmId}-cancel`)'] },
  'sidebar-overlays.contract "sidebar-snooze"': { component: 'SidebarSnoozeDialog', wraps: ['focus("snooze-close")', 'focus(toggleId)'] },
  'this-machine.contract "local-environment-dialog"': { component: 'LocalEnvironmentDialog', wraps: ['focus("local-environment-confirm")', 'focus("local-environment-cancel")'] },
  'usage-bars.contract "reset-credit-dialog"': { component: 'ResetCreditDialog', wraps: ['focus("reset-credit-confirm")', 'focus("reset-credit-cancel")'] },
};
// No trap, as the reference has none: a Popover, Menu, hover card, toast, page or non-modal panel there.
const NOT_MODAL: Record<string, string> = {
  'auto-balance.contract `notice-machines-${notice.id}`': 'Popover',
  'composer-chip-popover.contract "chip-popover"': 'Popover',
  'citation-comment.contract "citation-comment"': 'Popover (AssistantCitationChip\'s comment, diff-cite-popover)',
  'diff.contract "diff-base-picker"': 'Popover',
  'markdown.contract `Terminal excerpt, ${chip.label}`': 'Popover',
  'markdown.contract "context-chip-details"': 'Popover',
  'model-picker.contract "model-dialog"': 'ProviderModelPicker PopoverPopup',
  'pages-pr-actions.contract testId': 'Popover (the freshness card)',
  'pages-pr-compose.contract "pull-request-composer"': 'PullRequestComposer PopoverPopup',
  'pages-pr-meta.contract `pull-request-${which}-menu`': 'Popover',
  'pages-pr-quick.contract popId': 'Popover',
  'pages-usage.contract "usage-seg-pop-unpriced"': 'Popover',
  'providers-upkeep.contract `${advisory.popId}-popover`': 'Popover',
  'r4-git.contract "details-branch-picker"': 'Popover',
  'r6-pr.contract `details-pr-checks-popover-${row.key}`': 'Popover',
  'settings-a-fonts.contract `font-picker-${rowId}`': 'Popover',
  'settings-a-hosts.contract "Device tools"': 'Popover',
  'settings-appearance-editor.contract "theme-editor"': 'ThemeEditorPanel, a floating panel that is no modal',
  'settings-b-accent.contract "provider-accent-panel"': 'Popover',
  'settings-mobile-beta.contract "TestFlight beta"': 'NightlyMobileBeta Popover',
  'settings-mobile-beta.contract "Google Play beta"': 'NightlyMobileBeta Popover',
  'settings-rows.contract `setting-inheritance-${row.id}-popover`': 'Popover',
  'settings-source-control.contract `device-versions-${tool.kind}-popover`': 'Popover',
  'shell-details.contract "thread-details"': 'Popover',
  'shell-toast.contract toast.title': 'a toast',
  'sidebar-overlays.contract "scope-menu"': 'Menu',
  'snapshot.contract `snapshot-contents-popup-${image.id}`': 'Popover',
  'snooze-calendar.contract "snooze-calendar"': 'Popover',
  'theme-color-picker.contract `theme-color-${row.id}-popover`': 'Popover',
  'usage-pooled.contract `usage-seg-pop-${seg.id}`': 'Popover',
};

/** Every element id the sources declare: literal ids, templated ones (whole, and their literal heads: `x-${…}` → `x-`),
 *  and the testIds handed to a component whose element takes its testId (or buttonId, elementId, target) as its id. */
const ids = new Set<string>(), idHeads: string[] = [];
const takesTestId = new Set<string>(), takesButtonId = new Set<string>(), takesElementId = new Set<string>(), takesTarget = new Set<string>();
// A component whose element's id is a prop with a suffix (`id=\`${rowId}-allow\``): each call's value names one.
const propIds: { component: string; prop: string; suffix: string }[] = [];
for (const text of sources.values()) {
  for (const m of text.matchAll(/\bid="([^"]+)"/g)) ids.add(m[1]!);
  for (const m of text.matchAll(/\bid=`([^`]*)`/g)) ids.add(m[1]!);
  for (const m of text.matchAll(/\bid=`([^`$]*)\$\{/g)) idHeads.push(m[1]!);
  let component = '';
  for (const line of text.split('\n')) {
    component = /^component (\w+)/.exec(line)?.[1] ?? component;
    if (/\bid=testId\b/.test(line)) takesTestId.add(component);
    if (/\bid=buttonId\b/.test(line)) takesButtonId.add(component);
    if (/\bid=elementId\b/.test(line)) takesElementId.add(component);
    if (/\bid=target\b/.test(line)) takesTarget.add(component);
    const prop = /\bid=`\$\{(\w+)\}([^`$]*)`/.exec(line);
    if (prop) propIds.push({ component, prop: prop[1]!, suffix: prop[2]! });
    // Any other prop a component's element takes whole as its id (ThemeCardAction's actionId, SnapSwitch's switchId).
    const whole = /\bid=(\w+)(?=\s)/.exec(line)?.[1];
    if (whole && !['testId', 'buttonId', 'elementId', 'target'].includes(whole)) propIds.push({ component, prop: whole, suffix: '' });
  }
}
for (const text of sources.values()) for (const line of text.split('\n')) {
  const call = /\b([A-Z]\w*)\(/.exec(line.trim())?.[1];
  if (call && takesTestId.has(call)) for (const m of line.matchAll(/\btestId="([^"]+)"/g)) ids.add(m[1]!);
  if (call && takesButtonId.has(call)) for (const m of line.matchAll(/\bbuttonId="([^"]+)"/g)) ids.add(m[1]!);
  if (call && takesElementId.has(call)) for (const m of line.matchAll(/\belementId="([^"]+)"/g)) ids.add(m[1]!);
  if (call && takesTarget.has(call)) for (const m of line.matchAll(/\btarget=(?:"([^"]+)"|`([^`$]*)\$\{)/g)) m[1] ? ids.add(m[1]) : idHeads.push(m[2]!);
  for (const { component, prop, suffix } of propIds.filter(entry => entry.component === call)) {
    for (const m of line.matchAll(new RegExp(`\\b${prop}="([^"]+)"`, 'g'))) ids.add(m[1]! + suffix);
    if (suffix === '') for (const m of line.matchAll(new RegExp(`\\b${prop}=\`([^\`$]*)\\$\\{`, 'g'))) idHeads.push(m[1]!);
  }
}
const declared = (id: string) => ids.has(id) || idHeads.some(head => head !== '' && id.startsWith(head));
/** The string literals and templates a guard's target expression can evaluate to: the whole expression, or a ternary's
 *  branches (a literal right after `?` or `:`), never the operands of its conditions. */
const named = (expression: string) => [...expression.matchAll(/(?:^|[?:])\s*\(?\s*(?:"([^"]*)"|`([^`]*)`)/g)].map(m => m[1] ?? m[2]!).filter(name => name !== '');

describe('every dialog is classified: a FocusGuard pair, its own key wraps, or no modal in the reference', () => {
  test('each role="dialog" and role="alertdialog" is in exactly one list', () => {
    const keys = dialogs.map(dialog => dialog.key);
    const unclassified = keys.filter(key => !(key in GUARDED) && !(key in KEYED) && !(key in NOT_MODAL));
    expect(unclassified).toEqual([]);
    // Nothing listed has gone away (a renamed dialog would leave its entry behind).
    for (const key of [...Object.keys(GUARDED), ...Object.keys(KEYED), ...Object.keys(NOT_MODAL)]) expect(keys).toContain(key);
    expect(keys.length).toBe(Object.keys(GUARDED).length + 1 + Object.keys(KEYED).length + Object.keys(NOT_MODAL).length); // palette.contract has two
  });
});

describe('the FocusGuard pair (Base UI FloatingFocusManager\'s inside guards)', () => {
  test('a guard is an invisible, aria-hidden Tab stop that sends the focus to its target', () => {
    const guard = source('dialog-trap.contract');
    expect(guard).toContain('component FocusGuard');
    expect(guard).toContain('action enter\n    if target != ""\n      focus(target)');
    expect(guard).toContain('box id=guardId tabindex=0 aria-hidden=true focus=enter key=hold position="absolute" left=0 top=0 width=1 height=1 opacity=0 pointer-events="none" testId=guardId');
  });

  test('a guard whose target cannot take the focus keeps it: the key that would leave the dialog stays on the guard', () => {
    const guard = source('dialog-trap.contract');
    // Shift+Tab leaves from the start guard, Tab from the end guard; the other way goes on into the dialog.
    expect(guard).toContain('derive start = includes(guardId, "-guard-start")');
    expect(guard).toContain('action hold(k: string, e: KeyboardEvent)\n    if k == "Tab" and e.shiftKey == start\n      preventDefault()\n      stopPropagation()\n      if target != ""\n        focus(target)');
  });

  // A popup that takes the focus itself (the import wizard's, Base UI's `restoreFocus: "popup"`) has its guards beside it,
  // before and after, as Base UI renders them around the popup, so Tab from the popup reaches its first stop and Shift+Tab
  // the guard before it (adopt-main-fixes-r11 RT-5: main #490 carries Tab on from a focused box that is no stop, X79).
  const BESIDE = new Set(['browser-profiles.contract `browser-import-step-${wizard.step}`']);
  for (const dialog of dialogs.filter(dialog => dialog.key in GUARDED)) {
    test(`${dialog.key}: the start guard is the popup's first child, the end guard its last${BESIDE.has(dialog.key) ? ' (beside it: before and after)' : ''}`, () => {
      const base = GUARDED[dialog.key] === 'palette' && dialog.line.includes('position="absolute" left=floor((viewport.width - width) / 2) top=max(16') ? 'link-pr' : GUARDED[dialog.key]!;
      const lines = source(dialog.file).split('\n');
      const beside = BESIDE.has(dialog.key);
      // Beside the popup: its parent's children (the popup among them), less comments and blanks.
      const parent = beside ? lines.slice(0, dialog.index).findLastIndex(line => line.trim() !== '' && !line.trimStart().startsWith('//') && indent(line) < indent(dialog.line)) : dialog.index;
      const children = block(lines, parent).filter(line => line.trim() !== '' && !line.trimStart().startsWith('//'));
      const depth = indent(children[0]!);
      const direct = children.filter(line => indent(line) === depth);
      if (beside) expect(direct.map(line => line.trim().split(/[ (]/)[0])).toEqual(['FocusGuard', 'column', 'FocusGuard']);
      expect(direct[0]!.trim()).toStartWith(`FocusGuard(guardId="${base}-guard-start", target=`);
      expect(direct.at(-1)!.trim()).toStartWith(`FocusGuard(guardId="${base}-guard-end", target=`);
      expect(children.filter(line => line.includes('FocusGuard(')).length).toBe(2);
      // Every name a target can take is an element of the app (or a prop the shell's callers fill: `first`).
      for (const guard of [direct[0]!, direct.at(-1)!]) {
        const target = /target=(.*)\)$/.exec(guard.trim())![1]!;
        for (const name of named(target)) expect([name, declared(name)]).toEqual([name, true]);
      }
    });
  }
});

describe('the guards name the reference\'s first and last Tab stops', () => {
  // [file, start guard target, end guard target] for the dialogs whose stops are plain buttons and fields.
  const pairs: [string, string, string][] = [
    ['connections-routes.contract', '"route-remove-confirm"', '"route-remove-cancel"'],
    ['connections.contract', '"environment-remove-confirm"', '"environment-remove-cancel"'],
    ['settings-core-body.contract', '"restore-defaults-confirm"', '"restore-defaults-cancel"'],
    ['timeline.contract', '"revert-keep"', '"revert-cancel"'],
    ['timeline-plan.contract', '"plan-save-close"', '"plan-save-path"'],
    ['providers-wizard.contract', '(busy ? "usage-hub-label" : "usage-hub-close")', '"usage-hub-url"'],
    ['r6-pr.contract', '(merge.pending ? "pr-merge-cancel" : "pr-merge-confirm")', '"pr-merge-cancel"'],
    ['r9-connect.contract', '"pr-checkout-close"', '"pr-checkout-input"'],
    ['settings-a-hosts.contract', '(busy ? "device-host-options" : "device-host-close")', '(busy ? "device-host-options" : "device-host-name")'],
    ['settings-appearance-import.contract', '"add-theme-close"', '"add-theme-query"'],
    ['browser-profiles.contract', 'wizard.tabLast', 'wizard.tabFirst'],
  ];
  for (const [file, start, end] of pairs) {
    test(`${file}: Shift+Tab before the first stop goes to ${start}, Tab past the last to ${end}`, () => {
      const lines = source(file).split('\n').map(line => line.trim());
      expect(lines.some(line => line.startsWith('FocusGuard(') && line.includes('-guard-start') && line.endsWith(`target=${start})`))).toBe(true);
      expect(lines.some(line => line.startsWith('FocusGuard(') && line.includes('-guard-end') && line.endsWith(`target=${end})`))).toBe(true);
    });
  }

  test('in the plain confirms the stops in tree order are the end guard\'s target first and the start guard\'s last', () => {
    for (const [file, anchor] of [['connections-routes.contract', 'testId="route-remove-dialog"'], ['connections.contract', 'testId="environment-remove-dialog"'],
      ['settings-core-body.contract', 'testId="restore-defaults-dialog"'], ['timeline.contract', 'testId="revert-dialog"'], ['timeline-plan.contract', 'testId="plan-save-dialog"'], ['providers-wizard.contract', 'testId="usage-hub-dialog"'],
      ['settings-a-hosts.contract', 'testId="device-host-dialog"'], ['settings-appearance-import.contract', 'testId="add-theme-dialog"'], ['r9-connect.contract', 'testId="pr-checkout-dialog"']] as const) {
      const lines = source(file).split('\n');
      const body = block(lines, lines.findIndex(line => line.includes(anchor)));
      const stops = body.filter(line => /^\s*(button|input|textarea)\b/.test(line) && !/tabindex=-1\b/.test(line)).map(line => /\bid="([^"]+)"/.exec(line)?.[1] ?? '?');
      const guards = body.filter(line => line.includes('FocusGuard(')).map(line => named(/target=(.*)\)$/.exec(line.trim())![1]!));
      // A busy fallback comes first (`busy ? <a stop still enabled> : <the stop>`): the last name is the idle one.
      expect(guards[1]!.at(-1)).toBe(stops[0]!); // the end guard: Tab past the last stop goes to the first
      expect(guards[0]!.at(-1)).toBe(stops.at(-1)!); // the start guard: Shift+Tab before the first goes to the last
    }
  });
});

describe('the reference\'s order of stops', () => {
  test('the X is DialogPopup\'s last child: after the footer in New task and in Set up snapshots', () => {
    for (const [file, anchor, footer] of [['settings-scheduled.contract', 'testId="scheduled-task-dialog"', 'testId="cancel-scheduled-task"'],
      ['snapshot.contract', 'testId="snapshot-setup-dialog"', 'testId="snapshot-setup-enable"']] as const) {
      const lines = source(file).split('\n');
      const body = block(lines, lines.findIndex(line => line.includes(anchor)));
      const close = body.findIndex(line => line.includes('aria-label="Close"'));
      expect(close).toBeGreaterThan(body.findIndex(line => line.includes(footer)));
      expect(body[close]).toContain('position="absolute" top="0.5rem" right="0.5rem"');
    }
    // The X keeps its corner: the popup it now sits in directly is positioned.
    expect(source('settings-scheduled.contract').split('\n').find(line => line.includes('testId="scheduled-task-dialog"'))).toContain('column position="relative" width="100%" max-width="36rem"');
  });

  test('the dialogs that drew no X have DialogPopup\'s (showCloseButton is left on in the reference): last, at the top right', () => {
    for (const [file, anchor, close] of [['providers-wizard.contract', 'testId="usage-hub-dialog"', 'testId="usage-hub-close"'],
      ['timeline-plan.contract', 'testId="plan-save-dialog"', 'testId="plan-save-close"'], ['legacy-sidebar.contract', 'role="dialog" aria-modal=true aria-label=title testId=label', 'testId=`${label}-close`']] as const) {
      const lines = source(file).split('\n');
      const at = lines.findIndex(line => line.includes(anchor));
      expect(lines[at]).toContain('position="relative"');
      const body = block(lines, at).filter(line => line.trim() !== '' && !line.trimStart().startsWith('//'));
      const x = body.findIndex(line => line.includes(close));
      expect(body[x]).toContain('aria-label="Close"');
      expect(body[x]).toContain('position="absolute" top="0.5rem" right="0.5rem"');
      expect(body[x + 2]!.trim()).toStartWith('FocusGuard('); // the X, its icon, then the end guard
    }
  });

  test('the shared frames put the X after their content: R4Dialog, SaDialog, UsageDialogShell', () => {
    for (const [file, close] of [['r4-git.contract', 'button id=`${testId}-close`'], ['settings-a-dialogs.contract', 'button id=`${dialogId}-close`'], ['pages-usage-dialogs.contract', 'button id=`${dialogId}-close`']] as const) {
      const text = source(file);
      expect(text.indexOf(close)).toBeGreaterThan(text.indexOf('        children', text.indexOf('FocusGuard(')));
    }
  });

  test('what the reference never stops at is no Tab stop: a preview\'s backdrop, Autocomplete items, the palette\'s accessory', () => {
    for (const [file, testId] of [['timeline-attachments.contract', 'testId="image-preview-backdrop"'], ['r4-composer.contract', 'testId="video-preview-backdrop"'], ['timeline-mermaid.contract', 'testId="diagram-preview-backdrop"']] as const)
      expect(source(file).split('\n').find(line => line.includes(testId))).toContain('aria-keyshortcuts="Escape" tabindex=-1');
    const palette = source('palette.contract').split('\n');
    const options = palette.filter(line => /^\s*button\b/.test(line) && line.includes('role="option"'));
    expect(options.length).toBe(2);
    for (const option of options) expect(option).toContain('tabindex=-1');
    expect(palette.find(line => line.includes('testId="palette-accessory"'))).toContain('tabindex=-1');
    expect(source('settings-b-icons.contract').split('\n').find(line => line.includes('role="option"') && line.includes('sb:project-favicon-set'))).toContain('tabindex=-1');
  });
});

describe('review fixes (PR #431)', () => {
  test('content search\'s toggles are Tab stops after the field, as the reference\'s Base UI Toggles; Shift+Tab from the field goes to the last', () => {
    const palette = source('palette.contract').split('\n');
    const toggle = block(palette, palette.indexOf('component PaletteToggle')).find(line => /^\s*button\b/.test(line))!;
    expect(toggle).not.toContain('tabindex');
    const calls = palette.filter(line => line.trim().startsWith('PaletteToggle('));
    expect(calls.map(line => /testId="([^"]+)"/.exec(line)![1])).toEqual(['palette-match-case', 'palette-whole-word', 'palette-regex']);
    expect(palette.findIndex(line => line.includes('testId="palette-input"'))).toBeLessThan(palette.indexOf(calls[0]!));
    expect(palette.find(line => line.includes('FocusGuard(guardId="palette-guard-start"'))!.trim())
      .toBe('FocusGuard(guardId="palette-guard-start", target=(view.footerAction != "" ? "palette-footer-action" : (view.toggles ? "palette-regex" : "palette-input")))');
  });

  test('while a dialog is busy its guards name a stop that stays enabled, where one is left', () => {
    const guard = (file: string, id: string) => source(file).split('\n').find(line => line.includes(`FocusGuard(guardId="${id}"`))!.trim();
    expect(guard('snapshot.contract', 'snapshot-setup-guard-start')).toContain('target=(busy ? (error != "" ? "snapshot-setup-advanced" : "") : "snapshot-setup-close")');
    expect(guard('snapshot.contract', 'snapshot-setup-guard-end')).toContain('target=(busy ? (error != "" ? "snapshot-setup-advanced" : "") : ');
    expect(guard('pages-welcome.contract', 'welcome-guard-start')).toContain('target=(step == "agents" ? (busy ? "welcome-step-1" : "welcome-agents-continue") :');
    // The stops they fall back to take no `busy`: Advanced, the Connect step (held only while importing), the hub's label
    // field, the host editor's Advanced options.
    const line = (file: string, needle: string) => source(file).split('\n').find(text => text.includes(needle))!;
    expect(line('snapshot.contract', 'button id="snapshot-setup-advanced"')).not.toContain('disabled=');
    expect(line('pages-welcome.contract', 'WelcomeSteps(step=step')).toContain('busy=view.importing');
    expect(line('providers-wizard.contract', 'input id="usage-hub-label"')).not.toContain('disabled=');
    expect(line('settings-a-hosts.contract', 'button id="device-host-options"')).not.toContain('disabled=');
  });

  test('a shared control takes the id a guard names as its own prop, so no two instances share one', () => {
    const shared = ['MediaActions', 'UsagePrimaryButton', 'UsageGhostIcon', 'WelcomePrimary', 'WelcomeGhost', 'WelcomeDisclosure', 'R4Button', 'R4Checkbox', 'PreviewStep', 'PaletteToggle'];
    // What a guard can name: its target's literals, and those of the `first` a shared frame's guard takes.
    const firsts = (line: string) => [...line.matchAll(/\bfirst=(\((?:[^()]|\([^()]*\))*\)|"[^"]*")/g)].flatMap(m => named(m[1]!));
    const targets = new Set([...sources.values()].flatMap(text => text.split('\n')).flatMap(line =>
      [...named(/FocusGuard\(.*target=(.*)\)$/.exec(line.trim())?.[1] ?? '""'), ...firsts(line)]));
    for (const component of shared) {
      const [file, text] = [...sources].find(([, text]) => text.includes(`\ncomponent ${component}\n`))!;
      const body = block(text.split('\n'), text.split('\n').indexOf(`component ${component}`));
      expect([file, component, body.some(line => /\bid=elementId\b/.test(line))]).toEqual([file, component, true]);
      expect([file, component, body.some(line => /\bid=testId\b/.test(line))]).toEqual([file, component, false]);
      for (const [callFile, callText] of sources) for (const call of callText.split('\n').filter(line => line.trim().startsWith(`${component}(`))) {
        const id = /\belementId=("[^"]*")/.exec(call)?.[1];
        expect([callFile, component, id === undefined]).toEqual([callFile, component, false]);
        const value = JSON.parse(id!) as string;
        if (value !== '') {
          expect(call).toContain(`testId="${value}"`); // the instance's own name, so it is unique where its testId is
          expect([value, targets.has(value)]).toEqual([value, true]); // and only where a guard names it
        }
      }
    }
    // A thread's markdown images and videos carry none.
    for (const call of source('media-markdown.contract').split('\n').filter(line => line.trim().startsWith('MediaActions('))) expect(call).toContain('elementId="", ');
  });
});

// dialog-initial-focus-and-model-prices (DF-1, DF-2): Base UI's Dialog moves the focus into the popup as it opens, so the
// first Tab and Shift+Tab already start inside the trap. Its default is the popup's first Tab stop (what Tab past the
// last comes round to: the end guard's target, or a key wrap's); an element that autofocuses as it mounts, or the
// dialog's own `initialFocus`, takes it first. Each modal dialog carries one DialogOpenFocus naming that element: a
// dialog without one opened with nothing focused under real keys (realinput-1011a DT-5, DT-6, DT-8), so it fails here.
describe('every modal dialog takes the focus as it opens (Base UI\'s initialFocus)', () => {
  // Where the reference's popup puts the focus when it is not the first Tab stop (T3 Code 1e2ecbd975, apps/web/src).
  const NAMED: Record<string, [string, string]> = {
    'palette.contract view.testId': ['"palette-input"', 'ui/command.tsx: CommandInput autoFocus (the link-PR dialog\'s field is its first stop too)'],
    'settings-b-actions.contract "project-action-dialog"': ['"action-name"', 'projectScriptEditor.tsx: the Name input autoFocus'],
    'timeline-attachments.contract "image-preview"': ['"image-preview-close"', 'chat/ExpandedImageDialog.tsx: initialFocus={closeButtonRef}'],
    'pages-welcome.contract "welcome-dialog"': ['(step == "connect" and pairOpen ? "welcome-pairing-link" : (step == "connect" and view.ready and not busy ? "welcome-continue" : (step != "connect" and not view.importing ? "welcome-step-1" : (step == "import" ? "welcome-skip-import" : (length(view.computers) > 0 ? "welcome-computer-0" : "welcome-add-computer")))))',
      'onboarding/WelcomeWizard.tsx: initialFocus onboarding-pairing-url, else Continue (autoFocus) while it is enabled, else the first stop'],
  };
  // Where the reference autofocuses the first Tab stop itself (the same element either way): checked so that a change to
  // the stops cannot move it away.
  const AUTOFOCUSED: Record<string, string> = {
    'providers-wizard.contract "usage-hub-dialog"': '"usage-hub-url"', // settings/AddUsageLimitSourceDialog.tsx: Hub URL autoFocus
    'settings-a-hosts.contract "device-host-dialog"': '(busy ? "device-host-options" : "device-host-name")', // settings/DeviceHostEditor.tsx
    'settings-appearance-import.contract "add-theme-dialog"': '"add-theme-query"', // settings/ThemeSearchSection.tsx
    'connections-network-dialogs.contract "create-pairing-dialog"': '"pairing-label"', // settings/ConnectionsSettings.tsx: Client label autoFocus
  };
  // The dialogs whose focus on opening app.contract moves itself (import-wizard-initial-focus, #409): the browser import
  // wizard (its first stop, or the popup) and the profile removal confirm (Cancel).
  const OWN: Record<string, string> = {
    'browser-profiles.contract "browser-profile-remove-dialog"': 'task removalFocus',
    'browser-profiles.contract `browser-import-step-${wizard.step}`': 'task importWizardStepFocus',
  };
  const opens = (line: string) => /^DialogOpenFocus\(target=(.*)\)$/.exec(line.trim())?.[1];

  test('DialogOpenFocus moves the focus to its target once, at the popup\'s first layout, and again only after it was hidden', () => {
    const trap = source('dialog-trap.contract');
    expect(trap).toContain('component DialogOpenFocus\n  props\n    target: string\n  state opened = false\n  action laidOut(width: number, height: number)\n    if width == 0 or height == 0\n      opened = false\n    else if not opened\n      opened = true\n      if target != ""\n        focus(target)');
    expect(trap).toContain('box aria-hidden=true resize=laidOut position="absolute" left=0 top=0 width=1 height=1 opacity=0 pointer-events="none"');
  });

  for (const dialog of dialogs.filter(dialog => dialog.key in GUARDED || dialog.key in KEYED)) {
    test(`${dialog.key}: the focus moves into the popup as it opens`, () => {
      const children = block(source(dialog.file).split('\n'), dialog.index).filter(line => line.trim() !== '' && !line.trimStart().startsWith('//'));
      const direct = children.filter(line => indent(line) === indent(children[0]!));
      const targets = children.map(opens).filter(target => target !== undefined);
      if (dialog.key in OWN) {
        expect(targets).toEqual([]);
        expect(source('app.contract')).toContain(OWN[dialog.key]!);
        return;
      }
      expect(targets.length).toBe(1);
      // Right inside the popup: after the start guard where it has one, else its first child.
      expect(opens(direct[dialog.key in GUARDED ? 1 : 0]!)).toBe(targets[0]!);
      const target = targets[0]!;
      for (const name of named(target)) expect([name, declared(name)]).toEqual([name, true]);
      if (dialog.key in NAMED) expect(target).toBe(NAMED[dialog.key]![0]);
      else if (dialog.key in GUARDED) expect(target).toBe(/target=(.*)\)$/.exec(direct.at(-1)!.trim())![1]!); // the first stop
      else expect([dialog.key, KEYED[dialog.key]!.wraps.some(wrap => wrap.includes(target))]).toEqual([dialog.key, true]);
      if (dialog.key in AUTOFOCUSED) expect(target).toBe(AUTOFOCUSED[dialog.key]!);
    });
  }

  test('DF-1: Add provider opens on its Provider step, Model prices on Apply to, New task on Runs on (the reference over CDP)', () => {
    const line = (file: string, needle: string) => source(file).split('\n').find(text => text.includes(needle))!.trim();
    expect(line('providers-wizard.contract', 'DialogOpenFocus(target=(wizard.created')).toBe('DialogOpenFocus(target=(wizard.created == "" ? "add-provider-step-0" : "close-add-provider-x"))');
    expect(line('pages-usage-dialogs.contract', 'DialogOpenFocus(')).toBe('DialogOpenFocus(target=first)');
    expect(line('pages-usage-dialogs.contract', 'dialogId="usage-prices-dialog"')).toContain('first=(not page.prices.pending and not page.prices.hasChanges ? "usage-prices-apply-button" :');
    expect(line('settings-scheduled.contract', 'DialogOpenFocus(')).toBe('DialogOpenFocus(target=(draft.id == "" and not busy ? "task-environment" : "task-title"))');
  });
});

// dialog-initial-focus-and-model-prices, review fix: Base UI's Dialog also gives the focus back as it closes
// (FloatingFocusManager's `returnFocus`, DialogPopup's `finalFocus` left at its default): to its trigger, else to what held
// the focus as it opened. Once a dialog takes the focus as it opens (above), closing it removes the focused node, so each
// modal dialog gives the focus back: a DialogFinalFocus beside the `when` that mounts it naming its opener, its own
// focus() on close, or (listed in the task record's Known differences, X53) no opener the view can name.
describe('every modal dialog gives the focus back as it closes (Base UI\'s finalFocus)', () => {
  type Final = { final: [string, string] } | { own: [string, string] } | { none: string };
  // A shared frame's callers each name their own: SettingsConfirm by dialogId, AppConfirm by confirmId.
  const SETTINGS_CONFIRM: Record<string, Final> = {
    'legacy-remove-project': { none: 'the legacy sidebar asks from a native context menu on a row, which holds no focus' },
    'legacy-archive-many': { none: 'the legacy sidebar asks from a native context menu on a row, which holds no focus' },
    'project-remove': { final: ['app-settings.contract', '"remove-project"'] },
    'project-remove-member': { final: ['app-settings.contract', '`remove-checkout-${restSubject}`'] },
    'archive-delete': { none: 'no control opens it: an archived row asks through archive-context-delete' },
    'update-install': { final: ['app-settings.contract', '"setting-version-update"'] },
    'signal-kill': { none: 'the confirm keeps only the process id, not the diagnostics row whose SIGKILL asked' },
    'archive-context-delete': { none: 'asked from a native context menu on an archived row, which holds no focus' },
    'theme-remove': { final: ['app-settings.contract', '`theme-remove-${settingsThemeSubject}`'] },
    'theme-update': { final: ['settings-appearance-import.contract', '`theme-install-${data.pendingUpdate}`'] },
    sidebar: { own: ['sidebar-overlays.contract', 'focus(data.sidebar.dialogReturn)'] }, // the sidebar's three (dialogId by its dialog)
  };
  const APP_CONFIRM: Record<string, Final> = {
    'server-update-confirm': { none: 'asked from a server update notice\'s action, which the confirm does not keep' },
    'app-confirm': { own: ['app.contract', 'focus("thread-title")'] }, // the title menu's; a terminal close's gives it to the terminal
    'wizard-sign-out-confirm': { own: ['providers-wizard.contract', 'focus(`wizard-provider-sign-out-${wizard.created}`)'] },
    'acp-confirm': { own: ['providers-upkeep.contract', 'focus(what == "acp-delete" ? `acp-delete-${target}` : `acp-provider-disable-${target}`)'] },
    'provider-setup-confirm': { own: ['providers-setup.contract', 'focus(what == "setup-logout" ? `provider-sign-out-${instanceId}` : `provider-runtime-remove-${instanceId}`)'] },
  };
  const FINAL: Record<string, Final | 'callers'> = {
    'app-overlays.contract "project-dialog"': { none: 'opened from six controls (the sidebar\'s two, the legacy sidebar, the landing page, the empty pull request list, the palette) and the view keeps no record of which' },
    'browser-profiles.contract "browser-profile-remove-dialog"': { own: ['app.contract', 'focus(`browser-profile-options-${removalAt}`)'] },
    'browser-profiles.contract `browser-import-step-${wizard.step}`': { own: ['app.contract', 'focus("browser-profiles-add")'] },
    'connections-routes.contract "route-remove-dialog"': { final: ['app-settings.contract', '`environment-route-remove-${environment.environmentId}-${route.id}`'] },
    'connections.contract "environment-remove-dialog"': { final: ['app-settings.contract', '`environment-menu-${environment.environmentId}`'] },
    'legacy-sidebar.contract label': { none: 'the legacy sidebar opens Rename and Group into… from a native context menu on a row, which holds no focus' },
    'pages-usage-dialogs.contract dialogId': { final: ['app-main.contract', '(usage.detail.open ? `usage-model-${usage.detail.key}` : "usage-environment")'] },
    'pages-welcome.contract "welcome-dialog"': { none: 'it opens at launch, before anything holds the focus' },
    'palette.contract view.testId': { own: ['app.contract', 'focus(palettePage == "pr-link-thread" ? "pull-request-more"'] },
    'providers-wizard.contract "add-provider-dialog"': { final: ['app-settings.contract', '(providerDialog == "hub" ? "add-usage-hub" : (providerDialog == "hub-remove" ? `remove-usage-hub-${providerTarget}` : "add-provider"))'] },
    'providers-wizard.contract "usage-hub-dialog"': { final: ['app-settings.contract', '(providerDialog == "hub" ? "add-usage-hub" : (providerDialog == "hub-remove" ? `remove-usage-hub-${providerTarget}` : "add-provider"))'] },
    'providers-wizard.contract "usage-hub-remove-dialog"': { final: ['app-settings.contract', '(providerDialog == "hub" ? "add-usage-hub" : (providerDialog == "hub-remove" ? `remove-usage-hub-${providerTarget}` : "add-provider"))'] },
    'r4-composer.contract "video-preview"': { none: 'the view keeps the video, not which of the composer\'s tiles (or a message\'s) opened it' },
    'r4-git.contract testId': { final: ['app-overlays.contract', 'details.git.dialogFrom'] },
    'r4-surfaces.contract "device-setup"': { none: 'it opens itself when a Device surface is chosen before a device is ready, from either surface menu' },
    'r6-pr.contract "pr-merge-dialog"': { final: ['r6-pr.contract', '`details-pr-merge-${details.prRows.merge.target}`'] },
    'r9-connect.contract "pr-checkout-dialog"': { none: 'opened from either branch picker (the thread details\' or the composer\'s), and the view keeps no record of which' },
    'settings-a-collections.contract "theme-remove-collection"': { final: ['app-settings.contract', '(settingsThemeDialog == "background-activity" ? "setting-background-activity" : (settingsThemeDialog == "import" ? "add-theme" : `theme-remove-${settingsThemeSubject}`))'] },
    'settings-a-dialogs.contract dialogId': { final: ['app-settings.contract', '(settingsThemeDialog == "background-activity" ? "setting-background-activity" : (settingsThemeDialog == "import" ? "add-theme" : `theme-remove-${settingsThemeSubject}`))'] },
    'settings-a-hosts.contract "device-host-dialog"': { final: ['app-settings.contract', '(settingsCore.hostEditor.title == "Add device host" ? "device-hosts-add" : `device-host-options-${settingsCore.hostEditor.id}`)'] },
    'settings-appearance-import.contract "add-theme-dialog"': { final: ['app-settings.contract', '(settingsThemeDialog == "background-activity" ? "setting-background-activity" : (settingsThemeDialog == "import" ? "add-theme" : `theme-remove-${settingsThemeSubject}`))'] },
    'settings-b-actions.contract "project-action-dialog"': { final: ['app-settings.contract', '`edit-project-action-${restSubject}`'] },
    'settings-b-actions.contract "action-delete-dialog"': { final: ['settings-b-actions.contract', '"action-delete"'] },
    'settings-b-icons.contract "project-icon-dialog"': { final: ['app-settings.contract', '"project-choose-icon"'] },
    'settings-b-icons.contract "project-favicon-picker"': { final: ['app-settings.contract', '"project-choose-file"'] },
    'settings-core-body.contract "restore-defaults-dialog"': { final: ['app-settings.contract', '"settings-restore-defaults"'] },
    'settings-scheduled.contract "scheduled-task-dialog"': { final: ['app-settings.contract', '(restSubject == "" or startsWith(restSubject, "new|") ? "new-scheduled-task" : `task-actions-${restSubject}`)'] },
    'snapshot.contract "snapshot-setup-dialog"': { final: ['app-settings.contract', 'snapshotSetupFrom'] },
    'ssh-prompt.contract "ssh-password-dialog"': { own: ['app.contract', 'focus("ssh-connect")'] },
    'timeline-attachments.contract "image-preview"': { none: 'opened from a message\'s image, a video\'s play button or a context chip, and the view keeps only the image' },
    'timeline-mermaid.contract "diagram-preview"': { none: 'opened from a transcript diagram or a file\'s fence, and the view keeps only the diagram\'s code' },
    'timeline-plan.contract "plan-save-dialog"': { final: ['app-overlays.contract', '`plan-actions-${data.planSaveRow}`'] },
    'timeline.contract "revert-dialog"': { final: ['app-overlays.contract', '`edit-from-${data.revertPending}`'] },
    'codex-setup.contract "codex-picker"': { own: ['codex-setup.contract', 'focus(`codex-account-${picker.instanceId}`)'] },
    'codex-setup.contract "chatgpt-plan-dialog"': { none: 'it shows itself once after a ChatGPT sign-in (ChatGptWelcomeCoordinator): nothing opened it' },
    'connections-network-dialogs.contract "network-access-dialog"': { own: ['connections-network-dialogs.contract', 'focus("network-access-switch")'] },
    'connections-network-dialogs.contract "tailscale-disable-dialog"': { own: ['connections-network-dialogs.contract', 'focus("tailscale-https-switch")'] },
    'connections-network-dialogs.contract "tailscale-setup-dialog"': { own: ['connections-network-dialogs.contract', 'focus("tailscale-https-switch")'] },
    'connections-network-dialogs.contract "create-pairing-dialog"': { own: ['connections-network-dialogs.contract', 'focus("create-pairing-link")'] },
    'connections-network-dialogs.contract "pairing-reveal-dialog"': { none: 'it shows itself when copying a pairing link fails' },
    'connections.contract "connection-dialog"': { own: ['connections.contract', 'focus(routeTarget == "" ? "manage-connection" : `environment-route-add-${routeTarget}`)'] },
    'pages-pr-actions.contract "pr-action-dialog"': { own: ['pages-pr-actions.contract', 'focus(opener)'] },
    'pages-pr-stack.contract "pr-stack-dialog"': { final: ['pages-pr-actions.contract', '"pull-request-stack"'] },
    'providers-wizard.contract "chatgpt-account-dialog"': { final: ['app-settings.contract', '(providerDialog == "hub" ? "add-usage-hub" : (providerDialog == "hub-remove" ? `remove-usage-hub-${providerTarget}` : "add-provider"))'] },
    'settings-rest-dialogs.contract dialogId': 'callers',
    'shell-panels.contract "app-confirm"': 'callers',
    'sidebar-overlays.contract "sidebar-snooze"': { own: ['sidebar-overlays.contract', 'focus(data.sidebar.dialogReturn)'] },
    'this-machine.contract "local-environment-dialog"': { own: ['this-machine.contract', 'focus("local-environment-switch")'] },
    'usage-bars.contract "reset-credit-dialog"': { own: ['app.contract', 'focus(slice(result.message, 6))'] }, // the command's `focus:usage-reset-<n>` (composer-controls-usage.ts)
  };
  const finals = [...sources].flatMap(([file, text]) => text.split('\n').flatMap((line, index) =>
    line.trim().startsWith('DialogFinalFocus(') ? [{ file, index, line: line.trim() }] : []));
  const targetOf = (line: string) => /target=(.*)\)$/.exec(line)![1]!;
  const check = (what: Final, label: string) => {
    if ('final' in what) {
      const [file, target] = what.final;
      const uses = finals.filter(use => use.file === file && targetOf(use.line).includes(target));
      expect([label, uses.length]).toEqual([label, 1]);
      for (const name of named(targetOf(uses[0]!.line))) expect([label, name, declared(name)]).toEqual([label, name, true]);
    } else if ('own' in what) expect([label, source(what.own[0]).includes(what.own[1])]).toEqual([label, true]);
    else expect(what.none.length).toBeGreaterThan(0);
  };

  test('DialogFinalFocus stands outside the popup: 2 wide while its dialog is open, and gives the focus back once, as it shuts', () => {
    const trap = source('dialog-trap.contract');
    expect(trap).toContain('component DialogFinalFocus\n  props\n    open: bool\n    target: string\n  state shown = false\n  state back = ""\n  action laidOut(width: number, height: number)\n    if width >= 2 and not shown\n      shown = true\n      back = target\n    else if width == 1 and shown\n      shown = false\n      if back != ""\n        focus(back)');
    expect(trap).toContain('box aria-hidden=true resize=laidOut position="absolute" left=0 top=0 width=(open ? 2 : 1) height=1 opacity=0 pointer-events="none"');
    // Never inside a modal popup: it would go with the popup it watches (the Add Action editor's stands in its overlay,
    // beside the popup, as its Delete confirm does).
    for (const use of finals) {
      const inside = dialogs.filter(dialog => dialog.file === use.file && dialog.index < use.index && use.index <= dialog.index + block(source(dialog.file).split('\n'), dialog.index).length);
      expect([use.file, use.line, inside.map(dialog => dialog.key)]).toEqual([use.file, use.line, []]);
    }
  });

  for (const dialog of dialogs.filter(dialog => dialog.key in GUARDED || dialog.key in KEYED)) {
    test(`${dialog.key}: closing gives the focus back`, () => {
      expect(dialog.key in FINAL).toBe(true);
      const what = FINAL[dialog.key]!;
      if (what !== 'callers') return check(what, dialog.key);
      const [component, idProp, table] = dialog.key.startsWith('settings-rest') ? ['SettingsConfirm', 'dialogId', SETTINGS_CONFIRM] as const : ['AppConfirm', 'confirmId', APP_CONFIRM] as const;
      const calls = [...sources].flatMap(([file, text]) => text.split('\n').filter(line => line.trim().startsWith(`${component}(`)).map(line => ({ file, id: new RegExp(`\\b${idProp}=("[^"]*"|[^,)]*)`).exec(line)![1]! })));
      for (const call of calls) {
        const id = call.file === 'sidebar-overlays.contract' ? 'sidebar' : JSON.parse(call.id) as string;
        expect([call.file, id, id in table]).toEqual([call.file, id, true]);
        check(table[id]!, `${call.file} ${id}`);
      }
    });
  }

  test('every DialogFinalFocus is one a dialog above names', () => {
    const listed = [...Object.values(FINAL), ...Object.values(SETTINGS_CONFIRM), ...Object.values(APP_CONFIRM)].flatMap(what => typeof what === 'object' && 'final' in what ? [what.final] : []);
    for (const use of finals) expect([use.file, use.line, listed.some(([file, target]) => file === use.file && targetOf(use.line).includes(target))]).toEqual([use.file, use.line, true]);
    expect(finals.length).toBe(17);
  });

  test('the openers it names: the snapshot setup\'s, and the git card\'s by the control that opened it', () => {
    const window = source('app-window.contract');
    expect(window).toContain('snapshotSetupFrom = snapshotSetupOpen ? snapshotSetupFrom : "snapshot-enabled"');
    expect(window).toContain('snapshotSetupFrom = open and not snapshotSetupOpen ? "snapshot-setup" : snapshotSetupFrom');
    for (const id of ['snapshot-enabled', 'snapshot-setup', 'details-git-menu', 'details-git-action']) expect([id, declared(id)]).toEqual([id, true]);
  });
});
