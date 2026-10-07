// @ref llp/1107.002-design-system-parity.spec.md#user-preference-and-accessibility-scaling
// T3 Code mobile 365aa87982 appearancePreferences.ts and mobileTheme.ts.
// Device preferences are separate from the shared client's server settings.
import { bridgeReply, type Native } from './shared/protocol';
import { obj } from './shared/domain';

export const MOBILE_THEME_OPTIONS = [
  { id: 't3-code', label: 'T3 Code' }, { id: 't3-chat', label: 'T3 Chat' },
  { id: 'grove', label: 'Grove' }, { id: 'ocean', label: 'Ocean' },
  { id: 'ember', label: 'Ember' }, { id: 'iris', label: 'Iris' },
] as const;
export type MobileThemeId = typeof MOBILE_THEME_OPTIONS[number]['id'];
export interface MobilePreferences {
  themeMode: 'system' | 'light' | 'dark'; lightThemeId: MobileThemeId; darkThemeId: MobileThemeId;
  baseFontSize: number; terminalFontSize: number | null; codeFontSize: number | null; codeWordBreak: boolean;
  composerEnterBehavior: 'send' | 'newline'; followUpBehavior: 'queue' | 'steer';
  projectGroupingMode: 'repository' | 'repository_path' | 'separate';
  planModeEnabled: boolean; workingEnabled: boolean; workingExpanded: boolean; snoozedExpanded: boolean; settledExpanded: boolean;
}
export const MOBILE_PREFERENCE_DEFAULTS: MobilePreferences = {
  themeMode: 'system', lightThemeId: 't3-code', darkThemeId: 't3-code', baseFontSize: 16,
  terminalFontSize: null, codeFontSize: null, codeWordBreak: false, composerEnterBehavior: 'send',
  followUpBehavior: 'queue', projectGroupingMode: 'repository', planModeEnabled: false, workingEnabled: false,
  workingExpanded: false, snoozedExpanded: false, settledExpanded: false,
};
const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));
const validNumber = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);
const theme = (value: unknown): MobileThemeId => MOBILE_THEME_OPTIONS.some(option => option.id === value) ? value as MobileThemeId : 't3-code';
export function normalizeMobilePreferences(input: unknown): MobilePreferences {
  let decoded = input;
  if (typeof input === 'string') { try { decoded = JSON.parse(input); } catch { decoded = {}; } }
  const value = obj(decoded);
  return {
    themeMode: value.themeMode === 'dark' || value.themeMode === 'light' ? value.themeMode : 'system',
    lightThemeId: theme(value.lightThemeId), darkThemeId: theme(value.darkThemeId),
    baseFontSize: validNumber(value.baseFontSize) ? clamp(Math.round(value.baseFontSize), 11, 22) : 16,
    terminalFontSize: validNumber(value.terminalFontSize) ? clamp(value.terminalFontSize, 6, 14) : null,
    codeFontSize: validNumber(value.codeFontSize) ? clamp(Math.round(value.codeFontSize), 8, 18) : null,
    codeWordBreak: value.codeWordBreak === true,
    composerEnterBehavior: value.composerEnterBehavior === 'newline' ? 'newline' : 'send',
    followUpBehavior: value.followUpBehavior === 'steer' ? 'steer' : 'queue',
    projectGroupingMode: value.projectGroupingMode === 'repository_path' || value.projectGroupingMode === 'separate' ? value.projectGroupingMode : 'repository',
    planModeEnabled: value.planModeEnabled === true, workingEnabled: value.workingEnabled === true, workingExpanded: value.workingExpanded === true,
    snoozedExpanded: value.snoozedExpanded === true, settledExpanded: value.settledExpanded === true,
  };
}
export function resolveMobileAppearance(preferences: MobilePreferences, systemScheme: string) {
  const scheme = preferences.themeMode === 'system' ? systemScheme === 'dark' ? 'dark' : 'light' : preferences.themeMode;
  const scale = preferences.baseFontSize / 16;
  return {
    scheme, themeId: scheme === 'dark' ? preferences.darkThemeId : preferences.lightThemeId,
    baseFontSize: preferences.baseFontSize,
    terminalFontSize: preferences.terminalFontSize ?? clamp(Math.round(10.5 * scale * 2) / 2, 6, 14),
    codeFontSize: preferences.codeFontSize ?? clamp(Math.round(12 * scale), 8, 18),
    codeWordBreak: preferences.codeWordBreak,
    terminalCustom: preferences.terminalFontSize !== null, codeCustom: preferences.codeFontSize !== null,
  };
}
export function mobileTextRoles(baseFontSize: number) {
  const scale = normalizeMobilePreferences({ baseFontSize }).baseFontSize / 16;
  const role = (font: number, line: number) => ({ size: Math.max(8, Math.round(font * scale)), line: Math.max(10, Math.round(line * scale)) });
  return { micro: role(11, 14), caption: role(12, 16), label: role(13, 17), footnote: role(14, 19),
    body: role(16, 23), headline: role(18, 23), title: role(21, 28), largeTitle: role(26, 32), display: role(30, 36) };
}
export async function mobileSettingsPreferences(native?: Native | null) {
  if (!native?.available) return { ready: false, error: 'Open T3 Code on your iPhone or iPad.', revision: 0, ...MOBILE_PREFERENCE_DEFAULTS };
  native.watch('t3.mobile-preferences');
  const result = await bridgeReply(native, { op: 'mobilePreferences' });
  if (!result.ok) return { ready: false, error: result.error!.message, revision: 0, ...MOBILE_PREFERENCE_DEFAULTS };
  return { ready: true, error: '', revision: Number(obj(result.value).revision) || 0, ...normalizeMobilePreferences(result.value) };
}
/** One atomic patch, including selecting one theme for both appearances. Root refreshes its resources. */
export async function mobileSavePreference(key: string, value: string, native?: Native | null) {
  if (!native?.available) return { revision: 0, message: 'Open T3 Code on your iPhone or iPad.' };
  let patch: Record<string, unknown>;
  if (key === 'bothThemeIds') patch = { lightThemeId: value, darkThemeId: value };
  else if (['baseFontSize', 'terminalFontSize', 'codeFontSize'].includes(key)) {
    if (value === 'auto' && key !== 'baseFontSize') patch = { [key]: null };
    else if (value.trim() !== '' && Number.isFinite(Number(value))) patch = { [key]: Number(value) };
    else return { revision: 0, message: 'Choose a valid font size.' };
  } else if (['codeWordBreak', 'planModeEnabled', 'workingEnabled', 'workingExpanded', 'snoozedExpanded', 'settledExpanded'].includes(key)) {
    if (value !== 'true' && value !== 'false') return { revision: 0, message: 'Choose on or off.' };
    patch = { [key]: value === 'true' };
  } else patch = { [key]: value };
  const result = await bridgeReply(native, { op: 'mobilePreferencesPatch', patch });
  return { revision: result.ok ? Number(obj(result.value).revision) || 0 : 0, message: result.ok ? '' : result.error!.message };
}

/** Contract transports nullable overrides in JSON; booleans/numbers stay strongly typed in JS. */
export async function mobilePreferencesResource(native?: Native | null) {
  const snapshot = await mobileSettingsPreferences(native);
  return { ready: snapshot.ready, error: snapshot.error, revision: snapshot.revision, serialized: JSON.stringify(snapshot) };
}
/** Root runs on initial preference load and after themeMode changes using its existing presentation op. */
export async function mobileApplyAppearance(input: unknown, native?: Native | null) {
  if (!native?.available) return { revision: 0, message: 'Open T3 Code on your iPhone or iPad.' };
  const preferences = normalizeMobilePreferences(input);
  const result = await bridgeReply(native, { op: 'devicePresentation', appearanceMode: preferences.themeMode });
  return { revision: 0, message: result.ok ? '' : result.error!.message };
}
