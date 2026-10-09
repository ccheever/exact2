// settings-rows-and-labels (2026-10-09 desktop audit S1-8, S2-6, S2-9, S2-10): Settings texts and accessible names that
// live in the Contract views, read from the sources as hover-layer.test.ts reads them. The data each one reads is tested
// beside its view model (settings-rest.test.ts, settings-a.test.ts); the rendered result is the task's macOS drive.
import { describe, expect, test } from 'bun:test';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** The lines of `component name` in `file`, up to the next top-level declaration. */
async function component(file: string, name: string): Promise<string> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end).join('\n');
}

describe('S1-8: Restore defaults confirms with ConfirmDialogHost buttons', () => {
  test('Cancel and Confirm', async () => {
    const dialog = await component('settings-core-body.contract', 'SettingsRestoreDialog');
    expect(dialog).toContain('text "Cancel"');
    expect(dialog).toContain('text "Confirm" font-size="0.875rem"');
    expect(dialog).not.toContain('"Restore defaults"');
  });
});

describe('S2-6: the base branch trigger says origin/ only for a listed local branch', () => {
  test('resolveBranchTriggerLabel over the chosen project refs', async () => {
    const page = await source('settings-scheduled.contract');
    expect(page).toContain('fn taskBaseLabel(baseRef: string, startFromOrigin: bool, local: bool): string = baseRef == "" ? "Select ref" : (startFromOrigin and local ? `From origin/${baseRef}` : `From ${baseRef}`)');
    const editor = await component('settings-scheduled.contract', 'ScheduledEditor');
    expect(editor).toContain('derive baseLocal = length(filter(data.branches, (group) => group.projectId == projectId and length(filter(group.refs, (ref) => ref.value == baseRef and not ref.remote)) > 0)) > 0');
    expect(editor).toContain('text taskBaseLabel(baseRef, startFromOrigin, baseLocal) font-size');
  });
});

describe('S2-10: Source Control names its controls and resets as the reference does', () => {
  test('ScopedSettingRow reads the row control and reset labels, the title when they are empty', async () => {
    const row = await component('settings-source-control.contract', 'ScopedSettingRow');
    expect(row).toContain('aria-label=`Reset ${row.resetLabel != "" ? row.resetLabel : row.title} to default`');
    expect(row).toContain('SettingsSwitch(checked=row.checked, label=(row.control != "" ? row.control : row.title)');
    expect(row).toContain('SettingsSelect(selectId=`scoped-${row.key}`, label=(row.control != "" ? row.control : row.title)');
  });
});

describe('S2-9, S2-10: the Icon submenu', () => {
  test('its kinds are radio items, the current one checked; every menu row label starts at the left', async () => {
    const item = await component('settings-b-kit.contract', 'CnMenuItem');
    expect(item).toContain('role=(radio ? "menuitemradio" : "menuitem") aria-checked=(radio and highlighted)');
    expect(item).toContain('text label font-size="0.875rem" line-height="1.25rem" flex=1 min-width=0 white-space="nowrap" line-clamp=1 text-align="left"');
    const menu = await component('connections.contract', 'EnvironmentIconMenu');
    expect(menu).toContain('itemId=`environment-icon-${environment.environmentId}-${choice.kind}`, destructive=false, disabled=(environment.iconLock != ""), radio=true, highlighted=choice.selected');
    expect(menu).toContain('text "Icon" font-size="0.875rem" line-height="1.25rem" flex=1 min-width=0 text-align="left"');
    // Every other CnMenuItem stays a plain menu item.
    const uses = (await Promise.all(['connections.contract', 'settings-b-kit.contract', 'providers-setup.contract'].map(source))).join('\n').match(/CnMenuItem\([^\n]*/g)!;
    expect(uses.map(use => /radio=(true|false)/.exec(use)![1])).toEqual(['false', 'false', 'true', 'false', 'false', 'false']);
  });
});
