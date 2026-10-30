// Every modal dialog keeps Tab and Shift+Tab inside, as Base UI's focus trap does
// in each Dialog and AlertDialog of T3 Code 1e2ecbd975. Exact keeps no Tab inside an `aria-modal` box on macOS, and
// the `dialog` that `showModal(id)` keeps it in is a sheet or an `NSAlert` there (dialog-trap.contract says why that
// is not the reference's), so each dialog carries a trap: the FocusGuard pair of dialog-trap.contract, or the `key`
// handlers the older dialogs wrap with. These checks read the Contract sources, as
// dialog-focus.test.ts does: every `role="dialog"`/`"alertdialog"` is classified (a new one fails until it is), each
// guarded popup opens and closes with its pair, and every literal a guard names is an element id. The behavior is proven
// by driving each dialog on macOS: a source read cannot press Tab.
import { describe, expect, test } from 'bun:test';
import { contractComponent, value } from '../support/contract-action-fixture';
import { appFiles, appText } from '../support/app-files';


const files = appFiles().filter(name => name.endsWith('.contract')).sort();
const sources = new Map<string, string>(await Promise.all(files.map(async file => [file, appText(file)] as const)));
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
// Native dialogs use the host's modal focus containment, including streamed page prompts.
const nativeDialogs = new Set(dialogs.filter(dialog => dialog.line.trimStart().startsWith('dialog ')).map(dialog => dialog.key));

// The FocusGuard pair of each guarded dialog: its key and the guard ids' base.
const GUARDED: Record<string, string> = {
  'app-overlays.contract "project-dialog"': 'project-dialog',
  'browser-profiles.contract "browser-profile-remove-dialog"': 'browser-profile-remove',
  'connections-routes.contract "route-remove-dialog"': 'route-remove',
  'connections.contract "environment-remove-dialog"': 'environment-remove',
  'legacy-sidebar.contract label': 'legacy-dialog',
  'pages-usage-dialogs.contract dialogId': 'usage-dialog',
  'pages-welcome.contract "welcome-dialog"': 'welcome',
  'palette.contract view.testId': 'palette', // CommandPaletteBody, then PaletteDialog (link-pr), below
  'palette.contract "pull-request-thread-picker"': 'pr-thread-picker', // PaletteThreadPicker
  'providers-wizard.contract "add-provider-dialog"': 'add-provider',
  'providers-wizard.contract "usage-hub-dialog"': 'usage-hub',
  'providers-wizard.contract "usage-hub-remove-dialog"': 'usage-hub-remove',
  'composer-video.contract "video-preview"': 'video-preview',
  'composer-chip-image.contract "chip-image-preview"': 'chip-image-preview',
  'composer-chip-file.contract "chip-file-preview"': 'chip-file-preview',
  'git.contract testId': 'dialog-frame',
  'panel.contract "device-setup"': 'device-setup',
  'pr-row.contract "pr-merge-dialog"': 'pr-merge',
  'pr-checkout.contract "pr-checkout-dialog"': 'pr-checkout',
  'add-project-machine.contract "add-machine-dialog"': 'add-machine',
  'app-route-modal.contract modalId': 'route-modal',
  'settings-a-collections.contract "theme-remove-collection"': 'theme-remove-collection',
  'settings-a-dialogs.contract dialogId': 'sa-dialog',
  'settings-a-hosts.contract "device-host-dialog"': 'device-host',
  'settings-eas-hosts.contract "eas-host-dialog"': 'eas-host',
  'settings-appearance-import.contract "add-theme-dialog"': 'add-theme',
  'settings-b-actions.contract "project-action-dialog"': 'project-action',
  'settings-b-actions.contract "action-delete-dialog"': 'action-delete',
  'settings-b-icons.contract "project-icon-dialog"': 'project-icon',
  'settings-b-icons.contract "project-favicon-picker"': 'project-favicon',
  'settings-core-body.contract "restore-defaults-dialog"': 'restore-defaults',
  'settings-scheduled.contract "scheduled-task-dialog"': 'scheduled-task',
  'settings-scheduled.contract "webhook-rotate-dialog"': 'webhook-rotate',
  'scheduled-deliveries.contract "webhook-deliveries-dialog"': 'webhook-deliveries',
  'snapshot.contract "snapshot-setup-dialog"': 'snapshot-setup',
  'ssh-prompt.contract "ssh-password-dialog"': 'ssh-password',
  'timeline-attachments.contract "image-preview"': 'image-preview',
  'timeline-mermaid.contract "diagram-preview"': 'diagram-preview',
  'timeline-plan.contract "plan-save-dialog"': 'plan-save',
  'timeline.contract "revert-dialog"': 'revert',
};
// The dialogs that wrap with their own `key` handlers (the older ones, from before the FocusGuard pair): the
// component and the focus each wrap sends.
const KEYED: Record<string, { component: string; wraps: string[] }> = {
  'pages-pr-actions.contract "pr-action-dialog"': { component: 'PrActionDialog', wraps: ['focus(actions.pending ? "pr-action-dialog-cancel" : "pr-action-dialog-confirm")', 'focus("pr-action-dialog-cancel")'] },
  'codex-setup.contract "codex-picker"': { component: 'ChatGptAccountPicker', wraps: ['focus("codex-picker-connect-button")', 'focus(`codex-picker-option-${picker.first}`)'] },
  'codex-setup.contract "chatgpt-plan-dialog"': { component: 'ChatGptPlanDialog', wraps: ['focus(busy ? "chatgpt-plan-usage-button" : "chatgpt-plan-continue-button")', 'focus("chatgpt-plan-usage-button")'] },
  'connections-network-dialogs.contract "network-access-dialog"': { component: 'NetworkAccessDialog', wraps: ['focus("network-access-confirm")', 'focus("network-access-cancel")'] },
  'connections-network-dialogs.contract "tailscale-disable-dialog"': { component: 'TailscaleDisableDialog', wraps: ['focus("tailscale-disable-confirm")', 'focus("tailscale-disable-cancel")'] },
  'connections-network-dialogs.contract "tailscale-setup-dialog"': { component: 'TailscaleSetupDialog', wraps: ['focus("tailscale-setup-close")', 'focus("tailscale-port")'] },
  'connections-network-dialogs.contract "create-pairing-dialog"': { component: 'CreatePairingDialog', wraps: ['focus("pairing-create-close")', 'focus("pairing-label")'] },
  'connections-network-dialogs.contract "pairing-reveal-dialog"': { component: 'PairingRevealDialog', wraps: ['focus("pairing-reveal-close")', 'focus("pairing-reveal-value")'] },
  'connections.contract "connection-dialog"': { component: 'AddEnvironmentDialog', wraps: ['focus("connection-mode-remote")'] }, // ModeCard wraps the other way
  'pages-pr-stack.contract "pr-stack-dialog"': { component: 'PsDialogPopup', wraps: ['focus(e.shiftKey ? "pr-stack-dialog-confirm" : "pr-stack-dialog-cancel")'] },
  'providers-wizard.contract "chatgpt-account-dialog"': { component: 'ChatGptAccountDialog', wraps: ['focus("chatgpt-account-continue-button")', 'focus("chatgpt-account-name-field")'] },
  'settings-rest-dialogs.contract dialogId': { component: 'SettingsConfirm', wraps: ['focus(busy ? dialogId : `${dialogId}-confirm`)', 'focus(busy ? dialogId : `${dialogId}-cancel`)'] },
  'shell-panels.contract "app-confirm"': { component: 'AppConfirm', wraps: ['focus(busy ? confirmId : `${confirmId}-confirm`)', 'focus(busy ? confirmId : `${confirmId}-cancel`)'] },
  'sidebar-overlays.contract "sidebar-snooze"': { component: 'SidebarSnoozeDialog', wraps: ['focus("snooze-close")', 'focus(toggleId)'] },
  'this-machine.contract "local-environment-dialog"': { component: 'LocalEnvironmentDialog', wraps: ['focus("local-environment-confirm")', 'focus("local-environment-cancel")'] },
  'usage-bars.contract "reset-credit-dialog"': { component: 'ResetCreditDialog', wraps: ['focus(busy ? "reset-credit-dialog" : "reset-credit-confirm")', 'focus(busy ? "reset-credit-dialog" : "reset-credit-cancel")'] },
};
// No trap, as the reference has none: a Popover, Menu, hover card, toast, page or non-modal panel there.
const NOT_MODAL: Record<string, string> = {
  'browser-frames.contract "browser-page-dialog"': 'Chromium\'s tab-modal JavaScript dialog: modal to its page only',
  'auto-balance.contract `notice-machines-${notice.id}`': 'Popover',
  'composer-controls.contract `notice-details-card-${id}`': 'Popover (NoticeDescription, openOnHover)',
  'thread-end.contract `thread-now-details-card-${message.id}`': 'Popover (ThreadNow NoticeText, openOnHover)',
  'requests.contract `approval-why-card-${id}`': 'Popover (ComposerPendingApprovalPanel, openOnHover)',
  'timeline-approval.contract `timeline-approval-why-card-${id}`': 'Popover (TimelinePendingApprovalRow, openOnHover)',
  'composer-chip-popover.contract "chip-popover"': 'Popover',
  'citation-comment.contract "citation-comment"': 'Popover (AssistantCitationChip\'s comment)',
  'diff.contract "diff-base-picker"': 'Popover',
  'markdown.contract `Terminal excerpt, ${chip.label}`': 'Popover',
  'markdown.contract `Quoted assistant text: ${chip.label}`': 'Popover (AssistantCitationChip)',
  'markdown.contract "context-chip-details"': 'Popover',
  'skill-chip.contract `Skill ${label}`': 'Popover (SkillChipPopover)',
  'timeline-worktree.contract "Worktree setup"': 'Popover (BackgroundWorktreeSetupChip)',
  'model-picker.contract "model-dialog"': 'ProviderModelPicker PopoverPopup',
  'pages-pr-actions.contract testId': 'Popover (the freshness card)',
  'pages-pr-compose.contract "pull-request-composer"': 'PullRequestComposer PopoverPopup',
  'pages-pr-meta.contract `pull-request-${which}-menu`': 'Popover',
  'pages-pr-quick.contract popId': 'Popover',
  'pages-usage.contract "usage-seg-pop-unpriced"': 'Popover',
  'providers-upkeep.contract `${advisory.popId}-popover`': 'Popover',
  'git.contract "details-branch-picker"': 'Popover',
  'pr-row.contract `details-pr-checks-popover-${row.key}`': 'Popover',
  'settings-a-fonts.contract `font-picker-${rowId}`': 'Popover',
  'settings-a-hosts.contract "Device tools"': 'Popover',
  'settings-appearance-editor.contract "theme-editor"': 'ThemeEditorPanel, a floating panel that is no modal',
  'settings-b-accent.contract "provider-accent-panel"': 'Popover',
  'settings-mobile-beta.contract mark.caption': 'MobileAppSettingsRow Popover',
  'settings-rows.contract `settings-scope-search-${scope}`': 'Combobox popup',

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
    const unclassified = keys.filter(key => !nativeDialogs.has(key) && !(key in GUARDED) && !(key in KEYED) && !(key in NOT_MODAL));
    expect(unclassified).toEqual([]);
    // Nothing listed has gone away (a renamed dialog would leave its entry behind).
    for (const key of [...Object.keys(GUARDED), ...Object.keys(KEYED), ...Object.keys(NOT_MODAL)]) expect(keys).toContain(key);
    expect(keys.length).toBe(nativeDialogs.size + Object.keys(GUARDED).length + 1 + Object.keys(KEYED).length + Object.keys(NOT_MODAL).length); // palette.contract has two
  });
});

describe('the FocusGuard pair (Base UI FloatingFocusManager\'s inside guards)', () => {
  test('the guard intercepts bubbling focus before redirect, so the stale guard event cannot replace the popup owner', () => {
    const guard = source('dialog-trap.contract');
    expect(guard).toContain('action enter\n    stopPropagation()');
    expect(guard).toContain('tabindex=0 aria-hidden=true focusin=enter key=hold');
    expect(guard).toContain('pointer-events="none" testId=guardId');
    expect(guard).toContain('focus(target == "" ? popup : target)');
  });
  const guard = contractComponent('dialog-trap.contract', 'FocusGuard');
  for (const [name, enabled] of ['FocusGuard', 'ControlGuard'].flatMap(name => [['first', 'middle', 'last'], ['middle', 'last'], ['first', 'middle'], ['middle'], []].map(enabled => [name, enabled] as const))) {
    test(`${name}, enabled candidates ${JSON.stringify(enabled)}: both guards and both Tab directions stay inside`, () => {
      const guard = contractComponent('dialog-trap.contract', name);
      for (const start of [false, true]) {
        const focused: string[] = [], events: string[] = [];
        const scope = { start, targets: ['first', 'middle', 'last'], popup: 'popup',
          querySelectorAll: (q: string) => q.includes('dialog-focus') ? ['popup'] : enabled,
          focus: (id: string) => focused.push(id), stopPropagation: () => events.push('stop'), preventDefault: () => events.push('prevent') };
        guard.call('enter', {}, scope, []);
        expect(focused.pop()).toBe((start ? enabled.at(-1) : enabled[0]) ?? 'popup');
        expect(events.shift()).toBe('stop');
        for (const shiftKey of [false, true]) {
          guard.call('hold', {}, scope, ['Tab', { shiftKey, metaKey: false, ctrlKey: false, altKey: false }]);
          expect(focused.pop()).toBe((shiftKey ? enabled.at(-1) : enabled[0]) ?? 'popup');
        }
        expect(events).toEqual(['prevent', 'stop', 'prevent', 'stop']);
        for (const [k, modifiers] of [['Enter', {}], ['Tab', { metaKey: true }], ['Tab', { ctrlKey: true }], ['Tab', { altKey: true }]] as const)
          guard.call('hold', {}, scope, [k, modifiers]);
        expect(focused).toEqual([]);
      }
    });
  }
  test('an already removed popup cannot redirect a queued guard focus event', () => {
    const focused: string[] = [];
    guard.call('enter', {}, { start: false, targets: ['first'], popup: 'removed', querySelectorAll: () => ['first'], stopPropagation() {}, focus: (id: string) => focused.push(id) }, []);
    expect(focused).toEqual([]);
  });

  test('SSH guards include the native password field while unlocked and fall back when it locks', () => {
    const prompt = contractComponent('ssh-prompt.contract', 'SshPasswordPrompt');
    const controls = source('ssh-prompt.contract').split('\n').filter(line => /\bid="ssh-password-(?:field|cancel|continue)"/.test(line));
    for (const [responding, expired, first, last] of [
      [false, false, 'ssh-password-field', 'ssh-password-continue'],
      [false, true, 'ssh-password-cancel', 'ssh-password-cancel'],
      [true, false, 'ssh-password-dialog', 'ssh-password-dialog'],
    ] as const) {
      const env = { responding, prompt: { expired } }, enabled = controls.flatMap(line => {
        const marker = /data-focus-stop=(.+)$/.exec(line)?.[1];
        return marker && value(marker, env) === 'enabled' ? [/\bid="([^"]+)"/.exec(line)![1]!] : [];
      });
      const targets = prompt.derives({}, env).dialogTargets;
      for (const [start, expected] of [[false, first], [true, last]] as const) {
        const focused: string[] = [];
        guard.call('enter', {}, { start, targets, popup: 'ssh-password-dialog', querySelectorAll: (q: string) => q.includes('dialog-focus') ? ['ssh-password-dialog'] : enabled, stopPropagation() {}, focus: (id: string) => focused.push(id) }, []);
        expect(focused).toEqual([expected]);
      }
    }
  });

  test.each([
    ['git-publish.contract', 'PublishStep', 'git.contract', 'DialogFrame', 'git-publish-step-0', 'git-publish-dialog', { index: 0, current: 0 }, { index: 0, current: 2 }],
    ['settings-rows.contract', 'CoreSelect', 'settings-a-dialogs.contract', 'SaDialog', 'setting-background-policy', 'background-activity-dialog', { rowId: 'background-policy', disabled: false, stop: 'enabled' }, { rowId: 'background-policy', disabled: true, stop: 'enabled' }],
    ['pages-usage-prices.contract', 'UsageGhostIcon', 'pages-usage-dialogs.contract', 'UsageDialogShell', 'usage-prices-add', 'usage-prices-dialog', { elementId: 'usage-prices-add', disabled: false }, { elementId: 'usage-prices-add', disabled: true }],
  ] as const)('%s %s exposes its actual eligibility to the shared dialog guards', (file, component, frameFile, frameComponent, first, popup, on, off) => {
    const lines = source(file).split('\n'), body = block(lines, lines.indexOf(`component ${component}`));
    const control = body.find(line => /^\s*button\b/.test(line) && /\bid=/.test(line))!;
    const marker = /data-focus-stop=(.+)$/.exec(control)?.[1];
    const id = /\bid=(`[^`]+`|[A-Za-z]+)/.exec(control)![1]!;
    for (const [env, eligible] of [[on, true], [off, false]] as const) {
      expect(value(id, env)).toBe(first);
      const enabled = marker && String(value(marker, env)).split(' ').includes('enabled') ? [first, `${popup}-close`] : [`${popup}-close`]; // `~=`: one of its words
      const targets = contractComponent(frameFile, frameComponent).derives({}, { first, testId: popup, dialogId: popup }).dialogTargets;
      const focused: string[] = [];
      guard.call('enter', {}, { start: false, targets, popup, querySelectorAll: (q: string) => q.includes('dialog-focus') ? [popup] : enabled, stopPropagation() {}, focus: (id: string) => focused.push(id) }, []);
      expect(focused).toEqual([eligible ? first : `${popup}-close`]);
    }
  });

  for (const dialog of dialogs.filter(dialog => dialog.key in GUARDED)) {
    test(`${dialog.key}: the start guard is the popup's first child, the end guard its last`, () => {
      const base = GUARDED[dialog.key] === 'palette' && dialog.line.includes('position="absolute" left=floor((viewport.width - width) / 2) top=max(16') ? 'link-pr' : GUARDED[dialog.key]!;
      const lines = source(dialog.file).split('\n');
      const children = block(lines, dialog.index).filter(line => line.trim() !== '' && !line.trimStart().startsWith('//'));
      const depth = indent(children[0]!);
      const direct = children.filter(line => indent(line) === depth);
      // ControlGuard is the pair of a dialog whose stops are all buttons; Settings' and Usage's modal starts with
      // RouteFocusGuard, which wraps back to the page's last control (dialog-trap.contract).
      const kind = direct[0]!.trim().startsWith('ControlGuard(') ? 'ControlGuard' : 'FocusGuard';
      const start = direct[0]!.trim().startsWith('RouteFocusGuard(') ? 'RouteFocusGuard' : kind;
      expect(direct[0]!.trim()).toStartWith(`${start}(guardId="${base}-guard-start", targets=`);
      expect(direct.at(-1)!.trim()).toStartWith(`${kind}(guardId="${base}-guard-end", targets=`);
      expect(children.filter(line => line.includes('FocusGuard(') || line.includes('ControlGuard(')).length).toBe(2);
      // Every name a target can take is an element of the app (or a prop the shell's callers fill: `first`).
      for (const guard of [direct[0]!, direct.at(-1)!]) {
        const props = /targets=(\w+), popup=(.*)\)$/.exec(guard.trim())!;
        expect(props).not.toBeNull();
        expect(dialog.line).toContain(`id=${props[2]}`);
        expect(dialog.line).toContain('data-dialog-focus="open"');
        // The popup's Tab fallback, bound directly or called from the popup's own key action.
        if (dialog.line.includes('key=dialogFallback(')) expect(dialog.line).toContain(`key=dialogFallback(${props[2]}, ${props[1]})`);
        else expect(source(dialog.file)).toContain(`dialogFallback(${props[2]}, ${props[1]}, k, e)`);
        expect(source(dialog.file)).toContain(`derive ${props[1]} =`);
      }
    });
  }
});

describe('the guards name ordered controls; mounted enabled filtering decides busy fallbacks', () => {
  const targets = (file: string, component: string, scope: Record<string, unknown>) => {
    const lines = source(file).split('\n'), from = lines.indexOf(`component ${component}`);
    const line = lines.slice(from).find(line => line.startsWith('  derive dialogTargets ='))!;
    return value(line.slice(line.indexOf(' = ') + 3), scope);
  };
  test('all-disabled confirms keep their popup as fallback, and editable fields stay available in busy dialogs', () => {
    expect(targets('connections-routes.contract', 'RouteRemoveDialog', {})).toEqual(['route-remove-cancel', 'route-remove-confirm']);
    expect(targets('connections.contract', 'EnvironmentRemoveDialog', {})).toEqual(['environment-remove-cancel', 'environment-remove-confirm']);
    expect(targets('snapshot.contract', 'SnapshotSetup', {})).toEqual(['snapshot-setup-record', 'snapshot-permission-screen-allow', 'snapshot-permission-accessibility-allow', 'snapshot-setup-advanced', 'snapshot-setup-cancel', 'snapshot-setup-close']);
    expect(targets('providers-wizard.contract', 'UsageHubDialog', {})).toEqual(['usage-hub-url', 'usage-hub-label', 'usage-hub-close']);
    expect(targets('settings-a-hosts.contract', 'DeviceHostDialog', {})).toEqual(['device-host-name', 'device-host-options', 'device-host-close']);
    expect(targets('pr-checkout.contract', 'CheckoutDialog', {})).toEqual(['pr-checkout-input', 'pr-checkout-close']);
    expect(targets('panel.contract', 'DeviceSurfaceSetup', {})).toEqual(['device-setup-step-0', 'device-setup-close']);
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
      ['timeline-plan.contract', 'testId="plan-save-dialog"', 'testId="plan-save-close"'], ['legacy-sidebar.contract', 'role="dialog" aria-modal=dialogSeated.includes(label) aria-label=title testId=label', 'testId=`${label}-close`']] as const) {
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

  test('the shared frames put the X after their content: DialogFrame, SaDialog, UsageDialogShell', () => {
    for (const [file, close] of [['git.contract', 'button id=`${testId}-close`'], ['settings-a-dialogs.contract', 'button id=`${dialogId}-close`'], ['pages-usage-dialogs.contract', 'button id=`${dialogId}-close`']] as const) {
      const text = source(file);
      expect(text.indexOf(close)).toBeGreaterThan(text.indexOf('        children', text.indexOf('FocusGuard(')));
    }
  });

  test('what the reference never stops at is no Tab stop: a preview\'s backdrop, Autocomplete items, the palette\'s accessory', () => {
    for (const [file, testId] of [['timeline-attachments.contract', 'testId="image-preview-backdrop"'], ['composer-video.contract', 'testId="video-preview-backdrop"'], ['composer-chip-image.contract', 'testId="chip-image-backdrop"'], ['timeline-mermaid.contract', 'testId="diagram-preview-backdrop"']] as const)
      expect(source(file).split('\n').find(line => line.includes(testId))).toContain('aria-keyshortcuts="Escape" tabindex=-1');
    const palette = source('palette.contract').split('\n');
    const options = palette.filter(line => /^\s*button\b/.test(line) && line.includes('role="option"'));
    expect(options.length).toBe(3); // Regular, repository, and publish-host rows.
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
      .toBe('FocusGuard(guardId="palette-guard-start", targets=dialogTargets, popup=view.testId)');
    expect(source('palette.contract')).toContain('view.footerAction != "" ? "palette-footer-action"');
  });

  test('while a dialog is busy its guards name a stop that stays enabled, where one is left', () => {
    const guard = (file: string, id: string) => source(file).split('\n').find(line => line.includes(`FocusGuard(guardId="${id}"`))!.trim();
    expect(guard('snapshot.contract', 'snapshot-setup-guard-start')).toContain('targets=dialogTargets, popup="snapshot-setup-dialog"');
    expect(guard('snapshot.contract', 'snapshot-setup-guard-end')).toContain('targets=dialogTargets, popup="snapshot-setup-dialog"');
    expect(guard('pages-welcome.contract', 'welcome-guard-start')).toContain('targets=dialogTargets, popup="welcome-dialog"');
    // The stops they fall back to take no `busy`: Advanced, the Connect step (held only while importing), the hub's label
    // field, the host editor's Advanced options.
    const line = (file: string, needle: string) => source(file).split('\n').find(text => text.includes(needle))!;
    expect(line('snapshot.contract', 'button id="snapshot-setup-advanced"')).not.toContain('disabled=');
    expect(line('pages-welcome.contract', 'WelcomeSteps(step=step')).toContain('busy=view.importing');
    expect(line('providers-wizard.contract', 'input id="usage-hub-label"')).not.toContain('disabled=');
    expect(line('settings-a-hosts.contract', 'button id="device-host-options"')).not.toContain('disabled=');
  });

  test('a shared control takes the id a guard names as its own prop, so no two instances share one', () => {
    const shared = ['MediaActions', 'UsagePrimaryButton', 'UsageGhostIcon', 'WelcomePrimary', 'WelcomeGhost', 'WelcomeDisclosure', 'DialogButton', 'DialogCheckbox', 'PreviewStep', 'PaletteToggle'];
    // What a guard can name: its target's literals, and those of the `first` a shared frame's guard takes.
    const firsts = (line: string) => [...line.matchAll(/\bfirst=(\((?:[^()]|\([^()]*\))*\)|"[^"]*")/g)].flatMap(m => named(m[1]!));
    const targets = new Set([...sources.values()].flatMap(text => text.split('\n')).flatMap(line =>
      [...[...line.matchAll(/"([^"\n]*)"/g)].map(m => m[1]!), ...firsts(line)]));
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

// Base UI's Dialog moves the focus into the popup as it opens, so the
// first Tab and Shift+Tab already start inside the trap. Its default is the popup's first Tab stop (what Tab past the
// last comes round to: the end guard's target, or a key wrap's); an element that autofocuses as it mounts, or the
// dialog's own `initialFocus`, takes it first. Each modal dialog carries one DialogOpenFocus naming that element: a
// dialog without one opened with nothing focused under real keys, so it fails here.
describe('every modal dialog takes the focus as it opens (Base UI\'s initialFocus)', () => {
  // Where the reference's popup puts the focus when it is not the first Tab stop (T3 Code 1e2ecbd975, apps/web/src).
  const NAMED: Record<string, [string, string]> = {
    'palette.contract view.testId': ['"palette-input"', 'ui/command.tsx: CommandInput autoFocus (the link-PR dialog\'s field is its first stop too)'],
    'settings-b-actions.contract "project-action-dialog"': ['"action-name"', 'projectScriptEditor.tsx: the Name input autoFocus'],
    'timeline-attachments.contract "image-preview"': ['"image-preview-close"', 'chat/ExpandedImageDialog.tsx: initialFocus={closeButtonRef}'],
    'pages-welcome.contract "welcome-dialog"': ['(step == "connect" and pairOpen ? "welcome-pairing-link" : (step == "connect" and view.ready and not busy ? "welcome-continue" : (step != "connect" and not view.importing ? "welcome-step-1" : (step == "import" ? "welcome-skip-import" : (view.computers.length > 0 ? "welcome-computer-0" : "welcome-add-computer")))))',
      'onboarding/WelcomeWizard.tsx: initialFocus onboarding-pairing-url, else Continue (autoFocus) while it is enabled, else the first stop'],
  };
  // Where the reference autofocuses the first Tab stop itself (the same element either way): checked so that a change to
  // the stops cannot move it away.
  const AUTOFOCUSED: Record<string, string> = {
    'providers-wizard.contract "usage-hub-dialog"': '"usage-hub-url"', // settings/AddUsageLimitSourceDialog.tsx: Hub URL autoFocus
    'settings-a-hosts.contract "device-host-dialog"': '(busy ? "device-host-options" : "device-host-name")', // settings/DeviceHostEditor.tsx
    'settings-eas-hosts.contract "eas-host-dialog"': '"eas-host-name"', // settings/EasDeviceHostsSettings.tsx: the Name input autoFocus
    'settings-appearance-import.contract "add-theme-dialog"': '"add-theme-query"', // settings/ThemeSearchSection.tsx
    'connections-network-dialogs.contract "create-pairing-dialog"': '"pairing-label"', // settings/ConnectionsSettings.tsx: Client label autoFocus
  };
  // The dialogs whose focus on opening app.contract moves itself (#409): the profile removal confirm (Cancel).
  const OWN: Record<string, string> = {
    'browser-profiles.contract "browser-profile-remove-dialog"': 'task removalFocus',
  };
  // A field that opens with its text selected takes the focus the same way (dialog-trap.contract FieldOpenSelect).
  const opens = (line: string) => { const m = /^(?:DialogOpenFocus\(target=(.*)\)|FieldOpenSelect\(target=(.*?), length=.*\))$/.exec(line.trim()); return m ? m[1] ?? m[2] : undefined; };

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
      else if (dialog.key in GUARDED) {
        const names = named(target);
        for (const name of names) expect(source(dialog.file)).toContain(name);
      }
      else expect([dialog.key, KEYED[dialog.key]!.wraps.some(wrap => wrap.includes(target))]).toEqual([dialog.key, true]);
      if (dialog.key in AUTOFOCUSED) expect(target).toBe(AUTOFOCUSED[dialog.key]!);
    });
  }

  test('Add provider opens on its Provider step, Model prices on Apply to, New task on Runs on (the reference over CDP)', () => {
    const line = (file: string, needle: string) => source(file).split('\n').find(text => text.includes(needle))!.trim();
    expect(line('providers-wizard.contract', 'DialogOpenFocus(target=(wizard.created')).toBe('DialogOpenFocus(target=(wizard.created == "" ? "add-provider-step-0" : "close-add-provider-x"))');
    expect(line('pages-usage-dialogs.contract', 'DialogOpenFocus(')).toBe('DialogOpenFocus(target=first)');
    expect(line('pages-usage-dialogs.contract', 'dialogId="usage-prices-dialog"')).toContain('first=(not page.prices.pending and not page.prices.hasChanges ? "usage-prices-apply-button" :');
    expect(line('settings-scheduled.contract', 'DialogOpenFocus(')).toBe('DialogOpenFocus(target=(draft.id == "" and not busy ? "task-environment" : "task-title"))');
  });

  test('Project grouping opens on its grouping rule, the reference popup\'s first Tab stop (LegacySidebar.tsx:2594)', () => {
    const line = (needle: string) => source('legacy-sidebar.contract').split('\n').find(text => text.includes(needle))!;
    expect(line('label="legacy-grouping"')).toContain('first="legacy-grouping-rule"');
    // A trap stop, so Tab past the X comes round to it as well.
    expect(line('select id="legacy-grouping-rule"')).toContain('data-focus-stop="enabled"');
  });
});

// Base UI's Dialog also gives the focus back as it closes
// (FloatingFocusManager's `returnFocus`, DialogPopup's `finalFocus` left at its default): to its trigger, else to what held
// the focus as it opened. Once a dialog takes the focus as it opens (above), closing it removes the focused node, so each
// modal dialog gives the focus back: a DialogFinalFocus beside the `when` that mounts it naming its opener, its own
// focus() on close, or (a known difference, issues/closed/20261009-gui-dialog-action-commands.md) no opener the view
// can name.
describe('every modal dialog gives focus back through a surviving observer or its existing close action', () => {
  const owners: Record<string, [string, string[]]> = {
    'pages-pr-actions.contract "pr-action-dialog"': ['pages-pr-actions.contract', ['pr-action-dialog']],
    'app-overlays.contract "project-dialog"': ['app-overlays.contract', ['project-dialog']],
    'connections-routes.contract "route-remove-dialog"': ['app-settings.contract', ['route-remove-dialog']],
    'connections.contract "environment-remove-dialog"': ['app-settings.contract', ['environment-remove-dialog']],
    'legacy-sidebar.contract label': ['legacy-sidebar.contract', ['legacy-rename-project', 'legacy-grouping']],
    'pages-usage-dialogs.contract dialogId': ['app-main.contract', ['usage-prices-dialog', 'usage-model-dialog']],
    'pages-welcome.contract "welcome-dialog"': ['app-overlays.contract', ['welcome-dialog']],
    'providers-wizard.contract "add-provider-dialog"': ['app-settings.contract', ['add-provider-dialog']],
    'providers-wizard.contract "usage-hub-dialog"': ['app-settings.contract', ['usage-hub-dialog']],
    'providers-wizard.contract "usage-hub-remove-dialog"': ['app-settings.contract', ['usage-hub-remove-dialog']],
    'providers-wizard.contract "chatgpt-account-dialog"': ['app-settings.contract', ['chatgpt-account-dialog']],
    'composer-video.contract "video-preview"': ['app-overlays.contract', ['video-preview']],
    'composer-chip-image.contract "chip-image-preview"': ['app-overlays.contract', ['chip-image-preview']],
    'composer-chip-file.contract "chip-file-preview"': ['app-overlays.contract', ['chip-file-preview']],
    'git.contract testId': ['app-overlays.contract', ['git-commit-dialog', 'git-confirm-dialog', 'git-publish-dialog']],
    'panel.contract "device-setup"': ['app-overlays.contract', ['device-setup']],
    'pr-row.contract "pr-merge-dialog"': ['pr-row.contract', ['pr-merge-dialog']],
    'pr-checkout.contract "pr-checkout-dialog"': ['app-overlays.contract', ['pr-checkout-dialog']],
    'add-project-machine.contract "add-machine-dialog"': ['app-overlays.contract', ['add-machine-dialog']],
    'settings-a-collections.contract "theme-remove-collection"': ['app-settings.contract', ['theme-remove-collection']],
    'settings-a-dialogs.contract dialogId': ['app-settings.contract', ['background-activity-dialog']],
    'settings-a-hosts.contract "device-host-dialog"': ['app-settings.contract', ['device-host-dialog']],
    'settings-eas-hosts.contract "eas-host-dialog"': ['app-settings.contract', ['eas-host-dialog']],
    'settings-appearance-import.contract "add-theme-dialog"': ['app-settings.contract', ['add-theme-dialog']],
    'settings-b-actions.contract "project-action-dialog"': ['app-settings.contract', ['project-action-dialog']],
    'settings-b-actions.contract "action-delete-dialog"': ['settings-b-actions.contract', ['action-delete-dialog']],
    'settings-b-icons.contract "project-icon-dialog"': ['app-settings.contract', ['project-icon-dialog']],
    'settings-b-icons.contract "project-favicon-picker"': ['app-settings.contract', ['project-favicon-picker']],
    'settings-core-body.contract "restore-defaults-dialog"': ['app-settings.contract', ['restore-defaults-dialog']],
    'settings-scheduled.contract "scheduled-task-dialog"': ['app-settings.contract', ['scheduled-task-dialog']],
    'settings-scheduled.contract "webhook-rotate-dialog"': ['settings-scheduled.contract', ['webhook-rotate-dialog']],
    'scheduled-deliveries.contract "webhook-deliveries-dialog"': ['app-settings.contract', ['webhook-deliveries-dialog']],
    'snapshot.contract "snapshot-setup-dialog"': ['app-settings.contract', ['snapshot-setup-dialog']],
    'timeline-attachments.contract "image-preview"': ['app-overlays.contract', ['image-preview']],
    'timeline-mermaid.contract "diagram-preview"': ['app-overlays.contract', ['diagram-preview']],
    'timeline-plan.contract "plan-save-dialog"': ['app-overlays.contract', ['plan-save-dialog']],
    'timeline.contract "revert-dialog"': ['app-overlays.contract', ['revert-dialog']],
    'codex-setup.contract "chatgpt-plan-dialog"': ['app-settings.contract', ['chatgpt-plan-dialog']],
    'connections-network-dialogs.contract "pairing-reveal-dialog"': ['connections-network-dialogs.contract', ['pairing-reveal-dialog']],
    'pages-pr-stack.contract "pr-stack-dialog"': ['pages-pr-actions.contract', ['pr-stack-dialog']],
    'usage-bars.contract "reset-credit-dialog"': ['app-overlays.contract', ['reset-credit-dialog']],
  };
  const own: Record<string, [string, string]> = {
    // AppRouteModal's finalFocus={false}: closing leaves the focus where it is, blurred first (the close button's leave).
    'app-route-modal.contract modalId': ['app-route-modal.contract', 'action leave\n    blur()\n    dismiss()'],
    'browser-profiles.contract "browser-profile-remove-dialog"': ['app.contract', 'focus(`browser-profile-options-${removalAt}`)'],
    'palette.contract view.testId': ['app.contract', 'focus("pull-request-more")'],
    'palette.contract "pull-request-thread-picker"': ['app.contract', 'focus("pull-request-more")'],
    'ssh-prompt.contract "ssh-password-dialog"': ['app.contract', 'focus("ssh-connect")'],
    'codex-setup.contract "codex-picker"': ['codex-setup.contract', 'focus(`codex-account-${picker.instanceId}`)'],
    'connections-network-dialogs.contract "network-access-dialog"': ['connections-network-dialogs.contract', 'focus("network-access-switch")'],
    'connections-network-dialogs.contract "tailscale-disable-dialog"': ['connections-network-dialogs.contract', 'focus("tailscale-https-switch")'],
    'connections-network-dialogs.contract "tailscale-setup-dialog"': ['connections-network-dialogs.contract', 'focus("tailscale-https-switch")'],
    'connections-network-dialogs.contract "create-pairing-dialog"': ['connections-network-dialogs.contract', 'focus("create-pairing-link")'],
    'connections.contract "connection-dialog"': ['connections.contract', 'focus(routeTarget == "" ? "manage-connection" : `environment-route-add-${routeTarget}`)'],
    'sidebar-overlays.contract "sidebar-snooze"': ['sidebar-overlays.contract', 'focus(data.sidebar.dialogReturn)'],
    'this-machine.contract "local-environment-dialog"': ['this-machine.contract', 'focus("local-environment-switch")'],
  };
  const shared: Record<string, string[]> = {
    'app-settings.contract': ['project-remove', 'project-remove-member', 'archive-delete', 'signal-kill', 'archive-context-delete', 'theme-remove', 'update-install'],
    'legacy-sidebar.contract': ['legacy-remove-project', 'legacy-archive-many'],
    'app-overlays.contract': ['server-update-confirm', 'update-everything-confirm', 'app-confirm'],
    'providers-wizard.contract': ['wizard-sign-out-confirm'],
    'settings-appearance-import.contract': ['theme-update'],
  };
  const finals = [...sources].flatMap(([file, text]) => text.split('\n').flatMap((line, index) =>
    line.trim().startsWith('DialogFinalFocus(') ? [{ file, index, line: line.trim(), owner: /owner="([^"]+)"/.exec(line)![1]! }] : []));
  const assertObserver = (file: string, owner: string) => expect(finals.filter(use => use.file === file && use.owner === owner)).toHaveLength(1);
  test('every observer survives the popup it watches and snapshots its owner only once', () => {
    const lifecycle = contractComponent('dialog-trap.contract', 'DialogFinalFocus');
    const state = { shown: false, heldOwner: '' }, opened: string[] = [], closed: string[] = [];
    const scope = { owner: 'first', dialogOpened: (id: string) => opened.push(id), dialogClosed: (id: string) => closed.push(id) };
    for (const width of [0, 2, 2, 0, 2]) lifecycle.call('laidOut', state, scope, [width, 1]);
    lifecycle.call('laidOut', state, { ...scope, owner: 'changed' }, [1, 1]);
    lifecycle.call('laidOut', state, scope, [1, 1]);
    expect(opened).toEqual(['first']); expect(closed).toEqual(['first']);
    for (const use of finals) {
      const inside = dialogs.filter(dialog => dialog.file === use.file && dialog.index < use.index && use.index <= dialog.index + block(source(dialog.file).split('\n'), dialog.index).length);
      expect([use.file, use.owner, inside.map(dialog => dialog.key)]).toEqual([use.file, use.owner, []]);
    }
  });
  for (const dialog of dialogs.filter(dialog => dialog.key in GUARDED || dialog.key in KEYED)) {
    test(`${dialog.key}: closing has a qualified owner`, () => {
      if (dialog.key in owners) {
        const [file, list] = owners[dialog.key]!; list.forEach(owner => assertObserver(file, owner));
        expect(dialog.line).toContain('data-dialog-focus="open"'); expect(dialog.line).toContain('focusin=dialogEntered(');
      } else if (dialog.key in own) {
        const [file, action] = own[dialog.key]!; expect(source(file)).toContain(action);
      } else {
        expect(['settings-rest-dialogs.contract dialogId', 'shell-panels.contract "app-confirm"']).toContain(dialog.key);
        expect(dialog.line).toContain('data-dialog-focus="open"');
        for (const [file, list] of Object.entries(shared)) list.forEach(owner => assertObserver(file, owner));
        for (const [file, action] of [['sidebar-overlays.contract', 'focus(data.sidebar.dialogReturn)'], ['providers-upkeep.contract', 'focus(what == "acp-delete"'], ['providers-setup.contract', 'focus(what == "setup-logout"']] as const) expect(source(file)).toContain(action);
      }
    });
  }
  test('every observer is an inventoried owner, and row deletion cannot remove its only observer', () => {
    const expected = Object.values(owners).flatMap(([file, list]) => list.map(owner => `${file}:${owner}`)).concat(Object.entries(shared).flatMap(([file, list]) => list.map(owner => `${file}:${owner}`)));
    expect(finals.map(use => `${use.file}:${use.owner}`).sort()).toEqual([...new Set(expected)].sort());
    const lines = source('app-settings.contract').split('\n');
    for (const id of ['environment-remove-dialog', 'route-remove-dialog', 'signal-kill', 'archive-context-delete']) {
      const observer = lines.find(line => line.includes('DialogFinalFocus(') && line.includes(`owner="${id}"`))!;
      expect(indent(observer)).toBe(4);
    }
  });
});
