// Settings › Appearance › Thread details (ThreadDetailsItemsRow at 6d450a4284, MIT, see LICENSE-T3): "N of 10 on", a
// checkbox per item and Reset to default, writing the same choices the thread details card's own menu writes.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { appearanceSections } from '../../src/appearance/settings-appearance';
import { threadDetailsMenu, threadDetailsVisibility } from '../../src/details/thread-details-items';
import { applyCoreSetting, decodeClientPrefs, DEVICE_DEFAULTS, restoreLabels } from '../../src/settings/settings-core';
import { settingsCore } from '../../src/settings/settings-core-view';
import { searchSettings } from '../../src/settings/settings-search';

type Fake = { local: Obj; ready: boolean; environmentId: string; config: Obj; projectGroups(): never[]; settingsCoreRequest(): Promise<Obj> };
function fake(): Fake {
  const client: Fake = {
    local: { deviceSettings: { ...DEVICE_DEFAULTS }, clientSettings: decodeClientPrefs({}), groupingMode: 'repository' },
    ready: true, environmentId: 'env1',
    config: { environment: { environmentId: 'env1', label: 'Studio', serverVersion: '0.1.5', capabilities: {} }, providers: [], settings: {} },
    projectGroups: () => [],
    async settingsCoreRequest() { return client.config.settings as Obj; },
  };
  return client;
}
const native = { available: true, later: async () => ({ ok: true, generation: 0, value: { fonts: [] } }) } as unknown as Native;
const as = (client: Fake) => client as unknown as T3Client;
const set = (client: Fake, part: string, value: string) => applyCoreSetting(as(client), native, `thread-details-items:${part}|||`, value);
const rowOf = async (client: Fake) => {
  const page = await settingsCore(as(client), native, '', '', '', '', 'appearance', '', true) as unknown as { sections: { rows: Obj[] }[] };
  return page.sections.flatMap(section => section.rows).find(row => row.id === 'thread-details-items')!;
};

describe('Appearance › Thread details', () => {
  test('sits between Chat width and Effort display, reading "9 of 10 on" at the defaults (Usage is off)', async () => {
    const rows = appearanceSections(decodeClientPrefs({}))[0]!.rows;
    expect(rows.map(row => row.id).slice(2, 5)).toEqual(['chat-width', 'thread-details-items', 'effort-display']);
    const row = await rowOf(fake());
    expect(row).toMatchObject({ title: 'Thread details', kind: 'items-menu', label: '9 of 10 on', resettable: false, resetLabel: 'thread details',
      description: 'Choose what the thread details card shows. You can also right-click the card.' });
    expect((row.options as Obj[]).map(option => [option.label, option.selected])).toEqual([
      ['Environment and worktree', true], ['Open in editor', true], ['Project scripts', true], ['Branch and pull request', true], ['Git actions', true],
      ['Changes', true], ['Servers', true], ['Automations', true], ['Lineage', true], ['Usage', false]]);
  });

  test('its menu writes what the card reads; Reset to default and the row reset clear every choice', async () => {
    const client = fake();
    expect(await set(client, 'branch', 'false')).toBe('');
    expect(await set(client, 'usage', 'true')).toBe('');
    // Only choices that differ from the default are kept, as the card's own menu keeps them.
    expect(client.local.threadDetailsItems).toEqual({ branch: false, usage: true });
    expect(threadDetailsVisibility(as(client))).toMatchObject({ branch: false, usage: true, environment: true });
    expect(threadDetailsMenu(as(client)).resetDisabled).toBe(false);
    expect(await rowOf(client)).toMatchObject({ label: '9 of 10 on', resettable: true });
    await set(client, 'changes', 'false');
    expect((await rowOf(client)).label).toBe('8 of 10 on');
    await set(client, 'branch', 'true');
    expect(client.local.threadDetailsItems).toEqual({ usage: true, changes: false });
    // The row's reset button sends `reset` with its target; the menu's Reset to default sends `reset` alone.
    await set(client, 'reset', 'threadDetailsItems');
    expect(client.local.threadDetailsItems).toEqual({});
    expect(await rowOf(client)).toMatchObject({ label: '9 of 10 on', resettable: false });
    await set(client, 'usage', 'true');
    await set(client, 'reset', '');
    expect(threadDetailsMenu(as(client)).resetDisabled).toBe(true);
  });

  test('Restore defaults neither lists nor resets it, as useSettingsRestore leaves threadDetailsItems alone', async () => {
    const client = fake();
    await set(client, 'lineage', 'false');
    expect(restoreLabels(client.local as never, null)).toEqual([]);
    expect(await applyCoreSetting(as(client), native, 'restore-device-defaults:|||', '')).toBe('Device settings restored');
    expect(client.local.threadDetailsItems).toEqual({ lineage: false });
  });

  test('search finds it by its items and limits, and Environment identification is gone', () => {
    const context = { connected: true, autoSettle: true, scopeKind: 'all', keybindings: [] };
    expect(searchSettings('weekly quota', context)[0]).toMatchObject({ id: 'thread-details-items', title: 'Thread details', route: 'appearance' });
    expect(searchSettings('nightly artwork', context).map(item => item.id)).not.toContain('environment-identification');
  });
});
