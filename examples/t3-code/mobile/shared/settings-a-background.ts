// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/settings-a-background.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// General → Background activity, Advanced (lane settings-a).
// Reference: packages/shared/src/backgroundActivitySettings.ts (presets, resolve,
// normalize), SettingsPanels.logic.ts (backgroundActivitySharedPolicySettings,
// backgroundActivityOverrideSettings, normalizeIntervalSeconds) and
// SettingsPanels.tsx BackgroundActivityAdvancedDialog. Durations travel as
// milliseconds (Schema.DurationFromMillis); the dialog shows whole seconds.
import { obj, str, type Json, type Obj } from './domain';
import { ClientError } from './protocol';

export const BACKGROUND_PROFILES = ['balanced', 'performance', 'battery-saver'] as const;
type Profile = (typeof BACKGROUND_PROFILES)[number];
export const PROFILE_LABELS: Record<Profile, string> = { balanced: 'Balanced', performance: 'Performance', 'battery-saver': 'Battery saver' };
const INTERVALS = ['automaticGitFetchInterval', 'providerHealthRefreshInterval', 'hostPowerMonitorActiveInterval', 'hostPowerMonitorIdleInterval', 'idleClientTtl'] as const;
const SWITCHES = ['pauseWhenHostLocked', 'pauseWhenHostLowPower', 'pauseWhenClientLowPower', 'pauseWhenOnBattery'] as const;
type Interval = (typeof INTERVALS)[number];
type Switch = (typeof SWITCHES)[number];
export type Resolved = { profile: Profile } & Record<Interval, number> & Record<Switch, boolean>;

// Seconds; the reference presets in Duration units.
const PRESETS: Record<Profile, Resolved> = {
  performance: { profile: 'performance', automaticGitFetchInterval: 15, providerHealthRefreshInterval: 60, hostPowerMonitorActiveInterval: 30, hostPowerMonitorIdleInterval: 120,
    idleClientTtl: 45, pauseWhenHostLocked: true, pauseWhenHostLowPower: false, pauseWhenClientLowPower: false, pauseWhenOnBattery: false },
  balanced: { profile: 'balanced', automaticGitFetchInterval: 30, providerHealthRefreshInterval: 300, hostPowerMonitorActiveInterval: 30, hostPowerMonitorIdleInterval: 300,
    idleClientTtl: 45, pauseWhenHostLocked: true, pauseWhenHostLowPower: true, pauseWhenClientLowPower: true, pauseWhenOnBattery: false },
  'battery-saver': { profile: 'battery-saver', automaticGitFetchInterval: 0, providerHealthRefreshInterval: 900, hostPowerMonitorActiveInterval: 60, hostPowerMonitorIdleInterval: 600,
    idleClientTtl: 45, pauseWhenHostLocked: true, pauseWhenHostLowPower: true, pauseWhenClientLowPower: true, pauseWhenOnBattery: true },
};
export const DEFAULT_BACKGROUND_ACTIVITY = { schemaVersion: 1, profile: 'balanced', overrides: {} };
const isProfile = (value: unknown): value is Profile => typeof value === 'string' && (BACKGROUND_PROFILES as readonly string[]).includes(value);
const seconds = (millis: unknown) => typeof millis === 'number' && Number.isFinite(millis) ? Math.round(millis / 1000) : undefined;

/** getBackgroundActivityBaseProfile. */
export function baseProfile(activity: Obj): Profile {
  if (activity.profile === 'custom') return isProfile(activity.baseProfile) ? activity.baseProfile : 'balanced';
  return isProfile(activity.profile) ? activity.profile : 'balanced';
}
/** resolveBackgroundActivitySettings over the environment's stored value (the legacy top-level intervals included). */
export function resolveBackground(settings: Obj): Resolved {
  const activity = obj(settings.backgroundActivity);
  const isDefault = (activity.profile ?? 'balanced') === 'balanced' && activity.baseProfile === undefined && Object.keys(obj(activity.overrides)).length === 0;
  const legacy = isProfile(settings.backgroundActivityProfile) ? settings.backgroundActivityProfile : 'balanced';
  const legacyGit = seconds(settings.automaticGitFetchInterval), legacyHealth = seconds(settings.providerHealthRefreshInterval);
  if (isDefault && (legacy !== 'balanced' || (legacyGit !== undefined && legacyGit !== 30) || (legacyHealth !== undefined && legacyHealth !== 300))) {
    const preset = PRESETS[legacy];
    return { ...preset, ...(legacyGit !== undefined ? { automaticGitFetchInterval: legacyGit } : {}), ...(legacyHealth !== undefined ? { providerHealthRefreshInterval: legacyHealth } : {}) };
  }
  const profile = baseProfile(activity), preset = PRESETS[profile];
  const overrides = activity.profile === 'custom' ? obj(activity.overrides) : {};
  const next = { ...preset };
  for (const key of INTERVALS) { const value = seconds(overrides[key]); if (value !== undefined) next[key] = value; }
  for (const key of SWITCHES) if (typeof overrides[key] === 'boolean') next[key] = overrides[key] as boolean;
  return next;
}
const sameAs = (a: Resolved, b: Resolved) => INTERVALS.every(key => a[key] === b[key]) && SWITCHES.every(key => a[key] === b[key]);
/** normalizeBackgroundActivitySettings: collapse to a preset when the values match one, else custom over the base. */
export function normalizeBackground(resolved: Resolved): Obj {
  for (const profile of [resolved.profile, ...BACKGROUND_PROFILES]) if (sameAs(resolved, PRESETS[profile])) return { schemaVersion: 1, profile, overrides: {} };
  const preset = PRESETS[resolved.profile], overrides: Obj = {};
  for (const key of INTERVALS) if (resolved[key] !== preset[key]) overrides[key] = resolved[key] * 1000;
  for (const key of SWITCHES) if (resolved[key] !== preset[key]) overrides[key] = resolved[key];
  return { schemaVersion: 1, profile: 'custom', baseProfile: resolved.profile, overrides };
}
/** The profile the General select shows: 'advanced' when the normalized value is custom. */
export function profileOption(settings: Obj): string {
  const normalized = normalizeBackground(resolveBackground(settings));
  return normalized.profile === 'custom' ? 'advanced' : str(normalized.profile, 'balanced');
}
const fullOverrides = (resolved: Resolved): Obj => Object.fromEntries([...INTERVALS.map(key => [key, resolved[key] * 1000]), ...SWITCHES.map(key => [key, resolved[key]])]);

const FIELDS: Record<string, [Interval, number]> = { git: ['automaticGitFetchInterval', 0], health: ['providerHealthRefreshInterval', 0], active: ['hostPowerMonitorActiveInterval', 5], idle: ['hostPowerMonitorIdleInterval', 5] };
const TOGGLES: Record<string, Switch> = { locked: 'pauseWhenHostLocked', 'host-low-power': 'pauseWhenHostLowPower', 'client-low-power': 'pauseWhenClientLowPower', battery: 'pauseWhenOnBattery' };

/** The backgroundActivity value one Advanced dialog control writes. part: policy, reset, git|health|active|idle, or a switch id. */
export function advancedBackgroundValue(settings: Obj, part: string, raw: string): Json {
  const resolved = resolveBackground(settings);
  if (part === 'reset') return { ...DEFAULT_BACKGROUND_ACTIVITY };
  if (part === 'policy') {
    // backgroundActivitySharedPolicySettings: keep the custom overrides, change the shared policy.
    if (!isProfile(raw)) throw new ClientError('Choose a supported background policy.');
    const normalized = normalizeBackground(resolved);
    return { schemaVersion: 1, profile: 'custom', baseProfile: raw, overrides: normalized.profile === 'custom' ? obj(normalized.overrides) : {} } as Json;
  }
  const field = FIELDS[part];
  if (field) {
    // normalizeIntervalSeconds: a blank or non-finite entry falls to the minimum; fractions round.
    const value = raw.trim() === '' ? NaN : Number(raw);
    const next = Number.isFinite(value) ? Math.max(field[1], Math.round(value)) : field[1];
    if (next > 86_400) throw new ClientError('Use an interval of one day or less.');
    return { schemaVersion: 1, profile: 'custom', baseProfile: resolved.profile, overrides: { ...fullOverrides(resolved), [field[0]]: next * 1000 } } as Json;
  }
  const toggle = TOGGLES[part];
  if (toggle) {
    if (raw !== 'true' && raw !== 'false') throw new ClientError('Choose On or Off.');
    return { schemaVersion: 1, profile: 'custom', baseProfile: resolved.profile, overrides: { ...fullOverrides(resolved), [toggle]: raw === 'true' } } as Json;
  }
  throw new ClientError('That background activity control is no longer available.');
}

export type BackgroundSwitch = { id: string; label: string; checked: boolean };
export type BackgroundDialog = { available: boolean; policy: string; policyLabel: string; policies: { id: string; value: string; label: string; detail: string; icon: string; selected: boolean; disabled: boolean }[];
  git: number; health: number; active: number; idle: number; switches: BackgroundSwitch[] };
export function backgroundDialog(settings: Obj, available: boolean): BackgroundDialog {
  const resolved = resolveBackground(settings);
  return { available, policy: resolved.profile, policyLabel: PROFILE_LABELS[resolved.profile],
    policies: BACKGROUND_PROFILES.map(id => ({ id, value: id, label: PROFILE_LABELS[id], detail: '', icon: '', selected: id === resolved.profile, disabled: false })),
    git: resolved.automaticGitFetchInterval, health: resolved.providerHealthRefreshInterval, active: resolved.hostPowerMonitorActiveInterval, idle: resolved.hostPowerMonitorIdleInterval,
    switches: [['locked', 'Pause when host is locked'], ['host-low-power', 'Pause on host low power'], ['client-low-power', 'Pause on client low power'], ['battery', 'Pause on battery']]
      .map(([id, label]) => ({ id: id!, label: label!, checked: resolved[TOGGLES[id!]!] })) };
}
