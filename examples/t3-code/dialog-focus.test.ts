// dialog-shortcut-focus: what keyboard focus can reach while a dialog is open, read from the Contract
// sources as context-menu-hookup.test.ts reads its handlers. Sequential focus follows tree order on every
// host and skips a node whose `tabindex` is negative (HTML's rule, LLP 1088 D7.3), so a component's
// controls in source order, less those, are its reference Tab order. On macOS the host also skips date,
// time and select inputs (X52); they are kept here, as the reference has them. These checks guard the
// wiring; the behavior is proven by the macOS drives in tasks/20261008-dialog-shortcut-focus.md.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';

const dir = new URL('./', import.meta.url);
const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** The lines of `component name` in `file`, up to the next top-level declaration. */
async function component(file: string, name: string): Promise<string[]> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end);
}
const testId = (line: string) => /testId=(?:"([^"]+)"|`([^`]+)`)/.exec(line)?.slice(1).find(Boolean) ?? '';
/** The testIds of the Tab stops among `lines`, in tree order: buttons, inputs and selects without a negative tabindex. */
const stops = (lines: string[]) => lines.filter(line => /^\s*(button|input|select|textarea)\b/.test(line) && !/tabindex=-1\b/.test(line)).map(testId);

describe('invisible buttons are never Tab stops', () => {
  test('every pressable of 0 or 1pt (a key, a chord or a hatch press) says tabindex=-1', async () => {
    const files = readdirSync(dir).filter(name => name.endsWith('.contract'));
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

  test('the keyboard dispatch still binds every chord on the buttons it takes out of the Tab order', async () => {
    const lines = await component('settings-shortcuts.contract', 'ShortcutButton');
    const button = lines.find(line => /^\s*button\b/.test(line))!;
    expect(button).toContain('aria-keyshortcuts=item.chord');
    expect(button).toContain('press=press');
    expect(button).toContain('tabindex=-1');
  });
});

describe('a dialog keeps Tab and Shift+Tab inside, in the reference order', () => {
  test('AppConfirm (ConfirmDialogHost): Cancel takes the focus; Tab from Confirm and Shift+Tab from Cancel wrap', async () => {
    const lines = await component('shell-panels.contract', 'AppConfirm');
    expect(stops(lines)).toEqual(['app-confirm-cancel', 'app-confirm-confirm']);
    const cancel = lines.find(line => line.includes('testId="app-confirm-cancel"'))!;
    const confirm = lines.find(line => line.includes('testId="app-confirm-confirm"'))!;
    expect(cancel).toContain('id=`${confirmId}-cancel` press=cancel key=fromCancel autofocus=true');
    expect(confirm).toContain('id=`${confirmId}-confirm` press=confirm key=fromConfirm');
    const body = lines.join('\n');
    expect(body).toContain('action fromCancel(k: string, e: KeyboardEvent)\n    if k == "Tab" and e.shiftKey');
    expect(body).toContain('preventDefault()\n      focus(`${confirmId}-confirm`)');
    expect(body).toContain('action fromConfirm(k: string, e: KeyboardEvent)\n    if k == "Tab" and not e.shiftKey');
    expect(body).toContain('preventDefault()\n      focus(`${confirmId}-cancel`)');
    // Each call site names its own ids; the main window's is the one the terminal's close focuses.
    const ids = (await Promise.all(['app-overlays.contract', 'providers-setup.contract', 'providers-wizard.contract'].map(source))).join('\n')
      .split('\n').filter(line => line.includes('AppConfirm(')).map(line => /confirmId="([^"]+)"/.exec(line)?.[1]);
    expect(ids.sort()).toEqual(['app-confirm', 'provider-setup-confirm', 'server-update-confirm', 'wizard-sign-out-confirm']);
  });

  test('SettingsConfirm wraps the same way, by its dialog id', async () => {
    const lines = await component('settings-rest-dialogs.contract', 'SettingsConfirm');
    expect(stops(lines)).toEqual(['${dialogId}-cancel', '${dialogId}-confirm']);
    const body = lines.join('\n');
    expect(body).toContain('button id=`${dialogId}-cancel` press=cancel key=fromCancel autofocus=true');
    expect(body).toContain('button id=`${dialogId}-confirm` press=confirm key=fromConfirm');
    expect(body).toContain('focus(`${dialogId}-confirm`)');
    expect(body).toContain('focus(`${dialogId}-cancel`)');
  });

  test('Custom snooze: one stop for the schedule type, the fields, Cancel, Snooze, then Close (DialogPopup renders it last)', async () => {
    const lines = await component('sidebar-overlays.contract', 'SidebarSnoozeDialog');
    // The two arms of `when data.sidebar.dialogMode == "date"` … `else`, each with what surrounds them.
    const indent = (line: string) => line.length - line.trimStart().length;
    const when = lines.findIndex(line => line.trim() === 'when data.sidebar.dialogMode == "date"');
    const otherwise = lines.findIndex((line, index) => index > when && line.trim() === 'else' && indent(line) === indent(lines[when]!));
    const after = lines.findIndex((line, index) => index > otherwise && line.trim() !== '' && indent(line) <= indent(lines[otherwise]!));
    const date = [...lines.slice(0, otherwise), ...lines.slice(after)];
    const duration = [...lines.slice(0, when), ...lines.slice(otherwise)];
    // The roving toggle (tabindex 0 or -1 by `toggleStop`) is one stop; at open it is "Date and time".
    const atOpen = (list: string[]) => stops(list).filter(id => id !== 'snooze-mode-duration');
    expect(atOpen(date)).toEqual(['snooze-mode-date', 'snooze-date', 'snooze-time', 'snooze-cancel', 'snooze-confirm', 'snooze-close']);
    expect(atOpen(duration)).toEqual(['snooze-mode-date', 'snooze-decrease', 'snooze-amount', 'snooze-increase', 'snooze-unit', 'snooze-cancel', 'snooze-confirm', 'snooze-close']);
    const body = lines.join('\n');
    expect(body).toContain('tabindex=(toggleStop == "date" ? 0 : -1)');
    expect(body).toContain('tabindex=(toggleStop == "duration" ? 0 : -1)');
    expect(body).toContain('focus("snooze-close")'); // Shift+Tab from the toggle
    expect(body).toContain('focus(toggleId)'); // Tab from Close
    expect(body).toContain('button id="snooze-close" press=dismiss key=closeKey');
    // fix-keyboard-focus: Escape is the dialog's unless the command palette covers it (the palette's first).
    expect(body).toContain('button press=dismiss disabled=busy aria-keyshortcuts=(covered ? "" : "Escape") testId="snooze-cancel"');
    expect(body).toContain('focus(data.sidebar.dialogReturn)');
  });

  test('Add Environment (Settings › Connections): Remote link takes the focus, Tab wraps between it and Close, and closing returns to the trigger', async () => {
    const lines = await component('connections.contract', 'AddEnvironmentDialog');
    const view = lines.slice(lines.findIndex(line => line.trim() === 'view'));
    // ModeCard draws each card's button; the remote arm's fields; the dialog's Close after both arms and the autocomplete.
    const order = view.filter(line => /^\s*(ModeCard\(|button|input)\b/.test(line)).map(line => /ModeCard\(mode="(\w+)"/.exec(line)?.[1] ?? (testId(line) || (/aria-label="([^"]+)"/.exec(line)?.[1] ?? '')));
    expect(order.slice(0, 2)).toEqual(['remote', 'ssh']);
    expect(order.at(-1)).toBe('close-connection');
    const body = lines.join('\n');
    expect(body).toContain('action dismiss\n    closeDialog()\n    focus(routeTarget == "" ? "manage-connection" : `environment-route-add-${routeTarget}`)');
    expect(view.find(line => line.includes('testId="close-connection"'))).toContain('button id="close-connection" press=dismiss key=closeKey');
    expect(body).toContain('action closeKey(k: string, e: KeyboardEvent)\n    if k == "Tab" and not e.shiftKey');
    expect(body).toContain('focus("connection-mode-remote")');
    const modeCard = await component('connections.contract', 'ModeCard');
    const card = modeCard.find(line => /^\s*button\b/.test(line))!;
    expect(card).toContain('button id=`connection-mode-${mode}` press=choose hover=hover key=wrapKey autofocus=(mode == "remote")');
    expect(modeCard.join('\n')).toContain('if mode == "remote" and k == "Tab" and e.shiftKey');
    expect(modeCard.join('\n')).toContain('focus("close-connection")');
    expect((await source('connections.contract')).split('\n').find(line => line.includes('testId="manage-connection"'))).toContain('id="manage-connection"');
    expect((await source('connections-routes.contract'))).toContain('id=`environment-route-add-${environmentId}` testId=`environment-route-add-${environmentId}`');
  });

  test('the root moves the focus into a sidebar dialog however it opened, and title-menu confirms give it back', async () => {
    const app = await source('app.contract');
    expect(app).toContain('task sidebarDialogFocus when data.sidebar.dialog != "" and not paletteOpen key=data.sidebar.dialog');
    expect(app).toContain('focus(data.sidebar.dialog == "snooze" ? "snooze-mode-date" : `sidebar-${data.sidebar.dialog}-cancel`)');
    expect(app).toMatch(/if startsWith\(confirmOp, "shell:"\)[^\n]*\n\s+focus\("thread-title"\)/);
    expect(app).toMatch(/confirmValue = ""\n\s+focus\("app-confirm-cancel"\)/); // the terminal's close asks with the focus in the confirm
    // The terminal's own close buttons keep the focus when pressed; they move it into the confirm themselves.
    const terminal = await source('terminal.contract');
    expect(terminal.match(/action askClose[^\n]*\n\s+confirm\("ui:confirm"[^\n]*\n\s+focus\("app-confirm-cancel"\)/g)?.length).toBe(2);
    expect(terminal).toContain('button press=askClose(tab.target, tab.closeTitle, tab.closeBody)');
    expect(terminal).toContain('id="terminal-close", strip=strip, edge=true, press=askClose)');
    const overlays = await source('sidebar-overlays.contract');
    expect(overlays).toContain('cancel=dismissDialog, confirm=run("dialog-confirm", "", ""))');
    const rows = (await source('sidebar-row.contract')).split('\n').filter(line => line.includes('testId=`thread-${t.id}`') && line.includes('press=open(t.id, "click")'));
    expect(rows.length).toBe(2);
    for (const row of rows) expect(row).toContain('id=`thread-${t.id}`');
  });
});
