// Settings › Restore defaults and the pull request glyph of a timeline lifecycle row, against the reference.
// Settings › Restore defaults resets exactly what the reference's useSettingsRestore resets (SettingsPanels.tsx: its
// one updateSettings patch, and useTheme's theme, mix and follow-system keys), so Font smoothing, Sidebar (legacy), the
// legacy thread sort, Plan mode (legacy) and typography's Advanced switch are neither listed nor reset.
// A pull request Timeline's lifecycle rows draw the pull request glyph on the rail (LifecycleEvent's IconMarker),
// never the actor's avatar.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { applyCoreSetting, applyDeviceSetting, CLIENT_DEFAULTS, decodeClientPrefs, DEVICE_DEFAULTS, KEPT_ON_RESTORE, restoreLabels } from './settings-core';
import { settingsCore } from './settings-core-view';
import { presentTimeline } from './pages-pr-timeline';

type Fake = { local: Obj; ready: boolean; environmentId: string; config: Obj; writes: Obj[]; projectGroups(): { key: string; name: string; members: Obj[] }[]; settingsCoreRequest(native: Native, method: string, payload: Obj): Promise<Obj> };
function fake(): Fake {
  const client: Fake = {
    local: { deviceSettings: { ...DEVICE_DEFAULTS, snapShotEnabled: false }, clientSettings: decodeClientPrefs({}), groupingMode: 'repository' },
    ready: true, environmentId: 'env1', writes: [],
    config: { environment: { environmentId: 'env1', label: 'Studio', serverVersion: '0.0.46', capabilities: { projectSettingsOverrides: true } }, providers: [], settings: { projectSettingsOverrides: {} } },
    projectGroups: () => [],
    async settingsCoreRequest(_native, method, payload) {
      if (method !== 'server.getSettings') client.writes.push(payload);
      return client.config.settings as Obj;
    },
  };
  return client;
}
const native = { available: true, later: async () => ({ ok: true, generation: 0, value: { fonts: [] } }) } as unknown as Native;
const as = (client: Fake) => client as unknown as T3Client;
const prefsOf = (client: Fake) => client.local.clientSettings as Record<string, unknown>;
const restore = (client: Fake) => applyCoreSetting(as(client), native, 'restore-device-defaults:|||', '');

/** The reference's restoreDefaults patch (SettingsPanels.tsx), its client-side keys in its order; the environment keys
 *  (autoResumeLimitedThreads, …, enableAgentBrowserAccess) are settings-core's RESTORED_SERVER_KEYS. */
const REFERENCE_PATCH_CLIENT_KEYS = [
  'appearanceContrast', 'diffColorScheme', 'chatWidth', 'timestampFormat', 'notificationMode', 'inAppNotificationsEnabled', 'wordWrap',
  'persistComposerContextStrip', 'diffFilesCollapsed', 'diffIgnoreWhitespace', 'diffLayout', 'proactivePanelsEnabled', 'showSkillsInSlashMenu',
  'composerCollapseOnScroll', 'composerRichTextEnabled', 'sendShortcut', 'followUpBehavior', 'contextWindowMeterEnabled', 'environmentIdentificationMode',
  'glassOpacity', 'panelAnimationDurationMs', 'sidebarThreadPreviewCount', 'sidebarProjectGroupingMode', 'sidebarProjectSortOrder', 'sidebarWorkingShelfEnabled',
  'confirmThreadArchive', 'confirmThreadDelete', 'confirmThreadUnpin', 'confirmQuit', 'fontFamilySans', 'fontFamilyComposer', 'fontFamilyCode', 'fontFamilyTerminal',
  'fontSizeInterface', 'fontSizePrompt', 'fontSizeCode', 'fontSizeTerminal', 'browserDefaultViewport', 'browserDefaultZoomFactor', 'browserDefaultAppearance',
  'browserRecordingFrameRate', 'browserRecordingShowKeyPresses', 'browserRecordingShowMousePresses', 'browserLinkTarget', 'browserAutoShowFloatingPreview',
];
/** useTheme's keys the restore also resets: setTheme("system"), clearThemeHalves(), setFollowSystem(true). */
const REFERENCE_THEME_KEYS = ['theme', 'themeLight', 'themeDark', 'appearanceMode'];
/** Where the clone keeps a reference key outside CLIENT_DEFAULTS / DEVICE_DEFAULTS. */
const ELSEWHERE: Record<string, string> = { sidebarProjectGroupingMode: 'groupingMode', browserDefaultViewport: 'local root', browserDefaultZoomFactor: 'local root', browserDefaultAppearance: 'local root' };

/** One valid value apart from the default for every key this app keeps in clientSettings. */
const CHANGED: Record<keyof typeof CLIENT_DEFAULTS, string | number | boolean> = {
  notificationMode: 'sound', inAppNotificationsEnabled: true, diffColorScheme: 'blue-orange', chatWidth: 'wide', appearanceContrast: 150, panelAnimationDurationMs: 200,
  confirmThreadArchive: true, confirmThreadDelete: false, confirmThreadUnpin: true, diffFilesCollapsed: false, diffIgnoreWhitespace: false, diffLayout: 'split',
  environmentIdentificationMode: 'pill', glassOpacity: 60, fontSizeInterface: 14, fontSizePrompt: 16, fontSizeCode: 14, fontSizeTerminal: 14, fontFamilyCode: 'Menlo',
  fontFamilyComposer: 'Menlo', fontFamilySans: 'Avenir', fontFamilyTerminal: 'Menlo', fontSmoothing: false, persistComposerContextStrip: true, contextWindowMeterEnabled: true,
  composerRichTextEnabled: false, followUpBehavior: 'steer', proactivePanelsEnabled: true, showSkillsInSlashMenu: false, legacySidebarEnabled: true,
  sidebarWorkingShelfEnabled: true, wordWrap: false, browserLinkTarget: 'app', theme: 'grove', themeLight: 'ocean', themeDark: 'ember', typographyAdvanced: true,
  confirmQuit: 'direct', sidebarProjectSortOrder: 'manual', sidebarThreadSortOrder: 'created_at', sidebarThreadPreviewCount: 8, browserRecordingFrameRate: 60,
  browserRecordingShowKeyPresses: true, browserRecordingShowMousePresses: true, browserAutoShowFloatingPreview: false,
};

describe('Restore defaults resets what the reference resets', () => {
  test('every device key is either one the reference resets or one it keeps, and nothing else', () => {
    const kept = new Set<string>([...KEPT_ON_RESTORE, 'planModeEnabled']);
    const reset = new Set([...REFERENCE_PATCH_CLIENT_KEYS, ...REFERENCE_THEME_KEYS]);
    const own = [...Object.keys(CLIENT_DEFAULTS), ...Object.keys(DEVICE_DEFAULTS)];
    // Every key this app keeps is classified: reset by the reference, or kept by it; never both.
    for (const key of own) expect([key, reset.has(key) !== kept.has(key)]).toEqual([key, true]);
    // Every client-side key of the reference's patch is one this app keeps (or keeps elsewhere).
    for (const key of REFERENCE_PATCH_CLIENT_KEYS) expect([key, own.includes(key) || key in ELSEWHERE]).toEqual([key, true]);
    expect([...kept].sort()).toEqual(['fontSmoothing', 'legacySidebarEnabled', 'planModeEnabled', 'sidebarThreadSortOrder', 'typographyAdvanced']);
  });

  test('with every value changed, a restore resets the reference\'s keys and keeps the five', async () => {
    expect(Object.keys(CHANGED).sort()).toEqual(Object.keys(CLIENT_DEFAULTS).sort());
    const client = fake();
    client.local.clientSettings = decodeClientPrefs(CHANGED);
    for (const key of Object.keys(CLIENT_DEFAULTS)) expect([key, prefsOf(client)[key]]).toEqual([key, CHANGED[key as keyof typeof CHANGED]]);
    client.local.deviceSettings = { composerCollapseOnScroll: false, planModeEnabled: true, timestampFormat: '24-hour', appearanceMode: 'dark', sendShortcut: 'mod-enter', snapShotEnabled: true };
    client.local.groupingMode = 'separate';
    const listed = restoreLabels(client.local as never, null);
    expect(listed).not.toContain('Font smoothing');
    expect(listed.slice(0, 3)).toEqual(['Theme', 'Follow system', 'Theme mix']);
    expect(await restore(client)).toBe('Device settings restored');
    const prefs = prefsOf(client);
    for (const key of Object.keys(CLIENT_DEFAULTS)) {
      const keptKey = (KEPT_ON_RESTORE as readonly string[]).includes(key);
      expect([key, prefs[key]]).toEqual([key, keptKey ? CHANGED[key as keyof typeof CHANGED] : CLIENT_DEFAULTS[key as keyof typeof CLIENT_DEFAULTS]]);
    }
    // Plan mode (legacy) stays; the reference's device keys reset; SnapShot's own keys are not Restore defaults'.
    expect(client.local.deviceSettings).toEqual({ ...DEVICE_DEFAULTS, planModeEnabled: true, snapShotEnabled: true });
    expect(client.local.groupingMode).toBe('repository');
    expect(restoreLabels(client.local as never, null)).toEqual([]);
  });

  test('Font smoothing alone leaves Restore defaults disabled; with Chat width the list names Chat width only and a restore keeps it', async () => {
    const client = fake();
    applyDeviceSetting(client.local as never, 'fontSmoothing', 'false');
    for (const [key, value] of [['legacySidebarEnabled', 'true'], ['sidebarThreadSortOrder', 'created_at'], ['typographyAdvanced', 'true'], ['planModeEnabled', 'true']] as const)
      applyDeviceSetting(client.local as never, key, value);
    expect(await settingsCore(as(client), native, '', '', '', '', 'appearance', '', true)).toMatchObject({ restoreCount: 0, restoreText: '' });
    applyDeviceSetting(client.local as never, 'chatWidth', 'wide');
    expect(await settingsCore(as(client), native, '', '', '', '', 'appearance', '', true)).toMatchObject({ restoreCount: 1, restoreText: 'This will reset: Chat width.' });
    await restore(client);
    const prefs = prefsOf(client);
    expect([prefs.chatWidth, prefs.fontSmoothing, prefs.legacySidebarEnabled, prefs.sidebarThreadSortOrder, prefs.typographyAdvanced, (client.local.deviceSettings as Obj).planModeEnabled])
      .toEqual(['comfortable', false, true, 'created_at', true, true]);
    expect(client.writes).toEqual([]);
  });
});

describe('A lifecycle row draws the pull request glyph on the rail', () => {
  test('the opened row keeps its actor for the name', () => {
    const view = presentTimeline({ detail: { number: 168, projectId: 'p1', createdAt: '2026-10-01T00:00:00Z', author: { login: 'octocat', avatarUrl: 'https://example.invalid/a.png' } },
      activity: { comments: [], commits: [], commentCount: 0 }, activityPending: false, activityError: '', now: Date.parse('2026-10-02T00:00:00Z') });
    expect(view.oldest.map(row => [row.kind, row.label, row.author, row.hasActor])).toEqual([['opened', 'Pull request opened', 'octocat', true]]);
  });
});
