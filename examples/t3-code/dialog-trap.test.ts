// dialog-trap-and-progress-value (DT-1): every modal dialog keeps Tab and Shift+Tab inside, as Base UI's focus trap does
// in each Dialog and AlertDialog of T3 Code 1e2ecbd975. Exact keeps no Tab inside an `aria-modal` box on macOS
// (EXACT2-GAPS.md X53, #282), so each dialog carries a trap: the FocusGuard pair of dialog-trap.contract, or the `key`
// handlers the older dialogs wrap with (20261008-dialog-shortcut-focus). These checks read the Contract sources, as
// dialog-focus.test.ts does: every `role="dialog"`/`"alertdialog"` is classified (a new one fails until it is), each
// guarded popup opens and closes with its pair, and every literal a guard names is an element id. The behavior is proven
// by the macOS drives in tasks/20261011-dialog-trap-and-progress-value.md.
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
  'app-settings.contract "settings-dialog"': 'the Settings page (a route in the reference)',
  'auto-balance.contract `notice-machines-${notice.id}`': 'Popover',
  'composer-chip-popover.contract "chip-popover"': 'Popover',
  'diff.contract "diff-base-picker"': 'Popover',
  'markdown.contract `Terminal excerpt, ${chip.label}`': 'Popover',
  'markdown.contract "context-chip-details"': 'Popover',
  'model-picker.contract "model-dialog"': 'ProviderModelPicker PopoverPopup',
  'pages-pr-actions.contract testId': 'Popover (the freshness card)',
  'pages-pr-compose.contract "pull-request-composer"': 'PullRequestComposer PopoverPopup',
  'pages-pr-links.contract "pull-request-link-preview"': 'PreviewCard',
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
 *  and the testIds handed to a component whose element takes its testId (or buttonId) as its id. */
const ids = new Set<string>(), idHeads: string[] = [];
const takesTestId = new Set<string>(), takesButtonId = new Set<string>();
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
    const prop = /\bid=`\$\{(\w+)\}([^`$]*)`/.exec(line);
    if (prop) propIds.push({ component, prop: prop[1]!, suffix: prop[2]! });
  }
}
for (const text of sources.values()) for (const line of text.split('\n')) {
  const call = /\b([A-Z]\w*)\(/.exec(line.trim())?.[1];
  if (call && takesTestId.has(call)) for (const m of line.matchAll(/\btestId="([^"]+)"/g)) ids.add(m[1]!);
  if (call && takesButtonId.has(call)) for (const m of line.matchAll(/\bbuttonId="([^"]+)"/g)) ids.add(m[1]!);
  for (const { component, prop, suffix } of propIds.filter(entry => entry.component === call))
    for (const m of line.matchAll(new RegExp(`\\b${prop}="([^"]+)"`, 'g'))) ids.add(m[1]! + suffix);
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
    expect(guard).toContain('box id=guardId tabindex=0 aria-hidden=true focus=enter position="absolute" left=0 top=0 width=1 height=1 opacity=0 pointer-events="none" testId=guardId');
  });

  for (const dialog of dialogs.filter(dialog => dialog.key in GUARDED)) {
    test(`${dialog.key}: the start guard is the popup's first child, the end guard its last`, () => {
      const base = GUARDED[dialog.key] === 'palette' && dialog.line.includes('position="absolute" left=floor((viewport.width - width) / 2) top=max(16') ? 'link-pr' : GUARDED[dialog.key]!;
      const children = block(source(dialog.file).split('\n'), dialog.index).filter(line => line.trim() !== '' && !line.trimStart().startsWith('//'));
      const depth = indent(children[0]!);
      const direct = children.filter(line => indent(line) === depth);
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
    ['providers-wizard.contract', '"usage-hub-close"', '"usage-hub-url"'],
    ['r6-pr.contract', '(merge.pending ? "pr-merge-cancel" : "pr-merge-confirm")', '"pr-merge-cancel"'],
    ['r9-connect.contract', '"pr-checkout-close"', '"pr-checkout-input"'],
    ['settings-a-hosts.contract', '"device-host-close"', '"device-host-name"'],
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
      expect(guards[1]).toEqual([stops[0]!]); // the end guard: Tab past the last stop goes to the first
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

  test('what the reference never stops at is no Tab stop: a preview\'s backdrop, Autocomplete items, the palette\'s accessory and toggles', () => {
    for (const [file, testId] of [['timeline-attachments.contract', 'testId="image-preview-backdrop"'], ['r4-composer.contract', 'testId="video-preview-backdrop"'], ['timeline-mermaid.contract', 'testId="diagram-preview-backdrop"']] as const)
      expect(source(file).split('\n').find(line => line.includes(testId))).toContain('aria-keyshortcuts="Escape" tabindex=-1');
    const palette = source('palette.contract').split('\n');
    const options = palette.filter(line => /^\s*button\b/.test(line) && line.includes('role="option"'));
    expect(options.length).toBe(2);
    for (const option of options) expect(option).toContain('tabindex=-1');
    expect(palette.find(line => line.includes('testId="palette-accessory"'))).toContain('tabindex=-1');
    expect(palette.find(line => line.includes('role="switch" tabindex=-1 testId=testId'))).toBeDefined();
    expect(source('settings-b-icons.contract').split('\n').find(line => line.includes('role="option"') && line.includes('sb:project-favicon-set'))).toContain('tabindex=-1');
  });
});
