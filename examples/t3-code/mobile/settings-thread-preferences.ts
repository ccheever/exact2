// @ref llp/1109.009-mobile-settings.decision.md#scoped-server-settings
// Pinned365aa87982 SettingsThreadsRouteScreen, autoSettleSettingsSync and AutoSettleDaysField.
import { mobileClient } from './client';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { settingsTokens } from './settings-appearance';
import { normalizeMobilePreferences } from './settings-preferences';
import { decodeMobileServerScope, mobileServerPlan, mobileServerTargets, type MobileServerScope } from './settings-server';
import { settingsAdoptConfig, settingsCall, settingsEndpoint, settingsGrants, settingsNative, settingsSources, type MobileSettingsSource } from './settings-server-source';

const AUTO_KEYS = ['sidebarAutoSettleOnMerge', 'sidebarAutoSettleAfterDays'] as const;
let writing = false;
type ThreadProjection = {scope: string; data: ReturnType<typeof mobileThreadPreferencesProjection>};
let lastProjection: ThreadProjection | null = null;
let heldProjection: ThreadProjection | null = null;
/** Source field validates the whole input; invalid/same values reset without sending. */
export function mobileAutoSettleDays(raw: string, current: number): number | null {
  const text = raw.trim(), value = /^\d+$/.test(text) ? Number(text) : NaN;
  return Number.isInteger(value) && value >= 1 && value <= 90 && value !== current ? value : null;
}
export function mobileThreadTargets(scope: MobileServerScope, sources: MobileSettingsSource[]) {
  const targets = mobileServerTargets(scope, sources.filter(source => obj(obj(source.config.environment).capabilities).threadAutoSettlement === true));
  return scope.members === null ? targets : scope.members.flatMap(member => targets.filter(target => target.source.environmentId === member.environmentId && target.projectId === member.id));
}
function pickerAppearance(preferences: unknown) {
  const prefs = normalizeMobilePreferences(preferences);
  const colors = (scheme: string, theme: string) => {const tokens = settingsTokens(scheme, theme); return {primary: tokens['--color-primary-text'], foreground: tokens['--color-foreground'], sheet: tokens['--color-sheet-solid']};};
  return JSON.stringify({themeMode: prefs.themeMode, baseFontSize: prefs.baseFontSize, light: colors('light', prefs.lightThemeId), dark: colors('dark', prefs.darkThemeId)});
}
export function mobileThreadPreferencesProjection(scope: MobileServerScope, sources: MobileSettingsSource[], writable: Set<string>, preferences: unknown, preferencesReady: boolean, pending = false, error = '', nativePicker = false) {
  const targets = mobileThreadTargets(scope, sources), first = targets[0], project = scope.members !== null;
  const supportsOverrides = targets.every(target => obj(obj(target.source.config.environment).capabilities).projectSettingsOverrides === true);
  const disabled = pending || !targets.length || targets.some(target => !writable.has(target.source.environmentId)) || project && !supportsOverrides;
  const uniform = (key: string) => first && targets.every(target => target.settings[key] === first.settings[key]) ? first.settings[key] : null;
  const mismatches = first ? targets.filter(target => (target.source.environmentId !== first.source.environmentId || target.projectId !== first.projectId)
    && AUTO_KEYS.some(key => target.settings[key] !== first.settings[key])).map(target => target.source.label) : [];
  const prefs = normalizeMobilePreferences(preferences);
  return { scopeJSON: JSON.stringify(scope), nativePicker, pickerAppearance: pickerAppearance(preferences), title: 'Thread behavior', error, pending, serverVisible: !!first, project, projectLabel: scope.projectLabel,
    supportsOverrides, hasOverrides: project && targets.some(target => AUTO_KEYS.some(key => target.sources[key] === 'project')), disabled,
    autoResume: uniform('autoResumeLimitedThreads') === true, autoResumeMixed: uniform('autoResumeLimitedThreads') === null,
    snooze: uniform('snoozeLimitedThreads') === true, snoozeMixed: uniform('snoozeLimitedThreads') === null,
    settleMerged: first?.settings.sidebarAutoSettleOnMerge === true,
    settleInactive: first !== undefined && first.settings.sidebarAutoSettleAfterDays !== null,
    afterDays: typeof first?.settings.sidebarAutoSettleAfterDays === 'number' ? first.settings.sidebarAutoSettleAfterDays : 3,
    mismatchLabels: pending ? '' : mismatches.join(', '),
    autoSettlePatch: first ? JSON.stringify(Object.fromEntries(AUTO_KEYS.map(key => [key, first.settings[key]]))) : '',
    preferencesReady, workingEnabled: prefs.workingEnabled, planModeEnabled: prefs.planModeEnabled };
}
async function load(scope: MobileServerScope, native: Native, fresh: boolean) {
  const sources = (await settingsSources(native)).filter(source => scope.environmentIds.includes(source.environmentId) && source.enabled && source.phase === 'connected');
  const endpoints = sources.map(source => settingsEndpoint(source, native)), writable = new Set<string>();
  await Promise.all(endpoints.map(async endpoint => {
    if (fresh) settingsAdoptConfig(endpoint, await settingsCall(endpoint, { op: 'request', method: 'server.getConfig', payload: {} }));
    if (obj(obj(endpoint.source.config.environment).capabilities).threadAutoSettlement !== true) return;
    if (settingsGrants(await settingsCall(endpoint, { op: 'http', path: '/api/auth/session' }), 'settings:write')) writable.add(endpoint.source.environmentId);
  }));
  return { sources, endpoints, writable };
}
export async function mobileThreadPreferences(scopeJSON: string, preferences: unknown, ready: boolean, nativeInput?: Native | null) {
  const scope = decodeMobileServerScope(scopeJSON);
  if (!nativeInput?.available) return mobileThreadPreferencesProjection(scope, [], new Set(), preferences, ready, false, 'Open T3 Code on your iPhone or iPad to manage server settings.');
  if (writing && heldProjection?.scope === scopeJSON) {
    const prefs = normalizeMobilePreferences(preferences);
    return {...heldProjection.data, pickerAppearance: pickerAppearance(preferences), pending: true, disabled: true, mismatchLabels: '', preferencesReady: ready, workingEnabled: prefs.workingEnabled, planModeEnabled: prefs.planModeEnabled};
  }
  try {
    const {sources, writable} = await load(scope, settingsNative(nativeInput), false);
    const data = mobileThreadPreferencesProjection(scope, sources, writable, preferences, ready, writing, '', true);
    if (!writing) lastProjection = {scope: scopeJSON, data};
    return data;
  } catch (error) { if (letGo(error)) throw error; return mobileThreadPreferencesProjection(scope, [], new Set(), preferences, ready, writing, error instanceof Error ? error.message : 'Could not load thread settings.', true); }
}
export function mobileThreadSettingsPatch(key: string, raw: string, scope: MobileServerScope, targets: ReturnType<typeof mobileThreadTargets>): Obj {
  const first = targets[0]; if (!first) throw new ClientError('There are no capable settings targets.');
  if (key === 'apply') {
    const value = obj(JSON.parse(raw));
    if (Object.keys(value).length !== 2 || typeof value.sidebarAutoSettleOnMerge !== 'boolean'
      || !(value.sidebarAutoSettleAfterDays === null || typeof value.sidebarAutoSettleAfterDays === 'number' && Number.isInteger(value.sidebarAutoSettleAfterDays) && value.sidebarAutoSettleAfterDays >= 1 && value.sidebarAutoSettleAfterDays <= 90)) throw new ClientError('The displayed auto-settle defaults are invalid.');
    return {sidebarAutoSettleOnMerge: value.sidebarAutoSettleOnMerge, sidebarAutoSettleAfterDays: value.sidebarAutoSettleAfterDays};
  }
  if (key === 'inactive') { if (raw !== 'true' && raw !== 'false') throw new ClientError('Choose On or Off.'); return { sidebarAutoSettleAfterDays: raw === 'true' ? 3 : null }; }
  if (key === 'days') {
    const parsed = mobileAutoSettleDays(raw, Number(first.settings.sidebarAutoSettleAfterDays));
    if (first.settings.sidebarAutoSettleAfterDays === null || parsed === null) throw new ClientError('Choose a different whole number of days from 1 to 90.');
    return { sidebarAutoSettleAfterDays: parsed };
  }
  if (!['sidebarAutoSettleOnMerge', 'autoResumeLimitedThreads', 'snoozeLimitedThreads'].includes(key)
    || scope.members !== null && key !== 'sidebarAutoSettleOnMerge') throw new ClientError('This setting does not belong to the selected scope.');
  if (raw !== 'true' && raw !== 'false') throw new ClientError('Choose On or Off.');
  return { [key]: raw === 'true' };
}
export async function mobileThreadPreferencesCommand(scopeJSON: string, key: string, raw: string, nativeInput?: Native | null) {
  let initialDays: number | null = null;
  if (key === 'days-native') {
    try {
      const event = obj(JSON.parse(raw));
      if (event.scope !== scopeJSON || typeof event.initial !== 'number' || !Number.isInteger(event.initial) || event.initial < 1 || event.initial > 90
        || typeof event.raw !== 'string' || mobileAutoSettleDays(event.raw, event.initial) === null) throw new Error('The day picker changed. Reopen it before saving.');
      initialDays = event.initial; raw = event.raw; key = 'days';
    } catch { return {revision: mobileClient.revision, message: 'The day picker changed. Reopen it before saving.'}; }
  }
  if (key === 'days' && (!/^\d+$/.test(raw.trim()) || Number(raw.trim()) < 1 || Number(raw.trim()) > 90)) return {revision: mobileClient.revision, message: ''};
  if (writing) return { revision: mobileClient.revision, message: 'A thread settings update is already in progress.' };
  writing = true; heldProjection = lastProjection?.scope === scopeJSON ? lastProjection : null;
  try {
    if (!nativeInput?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to manage server settings.');
    const scope = decodeMobileServerScope(scopeJSON), {sources, endpoints, writable} = await load(scope, settingsNative(nativeInput), true);
    const targets = mobileThreadTargets(scope, sources);
    if (!targets.length || targets.some(target => !writable.has(target.source.environmentId))) throw new ClientError('The selected environments do not allow settings changes.');
    // Missing connections must not silently narrow a captured scope. Incapable connected servers are source-excluded.
    if (scope.environmentIds.some(id => !sources.some(source => source.environmentId === id))) throw new ClientError('The selected settings scope changed. Reopen this page.');
    if (initialDays !== null && targets[0]!.settings.sidebarAutoSettleAfterDays !== initialDays) throw new ClientError('The auto-settle value changed. Reopen the day picker before saving.');
    if (key === 'clear' && scope.members === null) throw new ClientError('Choose a project to clear its overrides.');
    if (key === 'days' && Number(raw.trim()) === targets[0]!.settings.sidebarAutoSettleAfterDays) return {revision: mobileClient.revision, message: ''};
    const writes = mobileServerPlan(targets, key === 'clear' ? {} : mobileThreadSettingsPatch(key, raw, scope, targets), key === 'clear' ? AUTO_KEYS : undefined);
    const results = await Promise.allSettled(writes.map(async write => {
      const endpoint = endpoints.find(endpoint => endpoint.source.environmentId === write.environmentId)!;
      const settings = await settingsCall(endpoint, { op: 'request', method: 'server.updateSettings', payload: {patch: write.patch} });
      settingsAdoptConfig(endpoint, {...endpoint.source.config, settings});
    }));
    for (const result of results) if (result.status === 'rejected' && letGo(result.reason)) throw result.reason;
    const failures = results.flatMap((result, index) => result.status === 'rejected' ? [`${sources.find(source => source.environmentId === writes[index]!.environmentId)?.label}: ${result.reason instanceof Error ? result.reason.message : 'Not saved.'}`] : []);
    if (failures.length) throw new ClientError(`${failures.length < writes.length ? 'Saved on some environments. ' : ''}${failures.join('\n')}`);
    return {revision: ++mobileClient.revision, message: ''};
  } catch (error) { if (letGo(error)) throw error; return {revision: ++mobileClient.revision, message: error instanceof Error ? error.message : 'Could not save thread settings.'}; }
  finally { writing = false; heldProjection = null; }
}
