// An administrator's managed policy as clients see it, after T3 Code 5d3d78ab93 (MIT, see LICENSE-T3):
// packages/contracts/src/managedPolicy.ts (`ManagedPolicySummary` and its messages and checks), providerPolicy.ts
// (`isRuntimeModeAbove`) and settings.ts (`disallowedProviderMessage`). A server whose policy file manages the
// environment sends the summary as `serverConfig.managedPolicy`: which settings are locked and what it enforces,
// never the values. Change from the reference: the summary is read leniently; one that does not match the schema
// reads as absent instead of failing the whole config.
import { obj, type Obj } from './domain';

/** RuntimeMode, narrowest first. */
export const RUNTIME_MODES = ['approval-required', 'auto-accept-edits', 'auto', 'full-access'] as const;
export type RuntimeMode = (typeof RUNTIME_MODES)[number];
const RUNTIME_MODE_LABELS: Record<RuntimeMode, string> = { 'approval-required': 'Supervised', 'auto-accept-edits': 'Auto-accept edits', auto: 'Auto', 'full-access': 'Full access' };
const isRuntimeMode = (value: unknown): value is RuntimeMode => RUNTIME_MODES.includes(value as RuntimeMode);

export type ManagedPolicyClient = 'desktop' | 'local-browser' | 'remote';
export interface ManagedPolicySummaryLimits {
  maxRuntimeMode?: RuntimeMode;
  clients?: ManagedPolicyClient[];
  /** `false` also when `clients` leaves out `remote`, since Connect is remote access. */
  connect?: boolean;
  webhookTasks?: boolean;
  telemetry?: boolean;
  browserCookieImport?: boolean;
  providers?: { allowedDrivers?: string[]; lockedInstanceFields?: Record<string, string[]> };
  updates?: { mode: 'auto' | 'off'; allowNightly?: boolean };
}
export interface ManagedPolicySummary {
  organization: string;
  supportUrl?: string;
  /** Dot paths into ServerSettings that users cannot change. */
  lockedSettings: string[];
  limits?: ManagedPolicySummaryLimits;
}

const trimmed = (value: unknown): string | undefined => typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined;
const strings = (value: unknown): string[] | undefined => Array.isArray(value) && value.every(entry => typeof entry === 'string') ? value : undefined;
const isRecord = (value: unknown): value is Obj => value !== null && typeof value === 'object' && !Array.isArray(value);

function limitsOf(value: Obj): ManagedPolicySummaryLimits | null {
  const limits: ManagedPolicySummaryLimits = {};
  if (value.maxRuntimeMode !== undefined) { if (!isRuntimeMode(value.maxRuntimeMode)) return null; limits.maxRuntimeMode = value.maxRuntimeMode; }
  if (value.clients !== undefined) {
    const clients = strings(value.clients);
    if (!clients?.length || !clients.every(client => ['desktop', 'local-browser', 'remote'].includes(client))) return null;
    limits.clients = clients as ManagedPolicyClient[];
  }
  for (const key of ['connect', 'webhookTasks', 'telemetry', 'browserCookieImport'] as const) {
    if (value[key] === undefined) continue;
    if (typeof value[key] !== 'boolean') return null;
    limits[key] = value[key];
  }
  if (value.providers !== undefined) {
    if (!isRecord(value.providers)) return null;
    const providers: NonNullable<ManagedPolicySummaryLimits['providers']> = {};
    if (value.providers.allowedDrivers !== undefined) {
      const drivers = strings(value.providers.allowedDrivers);
      if (!drivers?.length) return null;
      providers.allowedDrivers = drivers;
    }
    if (value.providers.lockedInstanceFields !== undefined) {
      const fields = value.providers.lockedInstanceFields;
      if (!isRecord(fields) || !Object.values(fields).every(paths => strings(paths) !== undefined)) return null;
      providers.lockedInstanceFields = fields as Record<string, string[]>;
    }
    limits.providers = providers;
  }
  if (value.updates !== undefined) {
    const updates = obj(value.updates);
    if (updates.mode !== 'auto' && updates.mode !== 'off') return null;
    if (updates.allowNightly !== undefined && typeof updates.allowNightly !== 'boolean') return null;
    limits.updates = { mode: updates.mode, ...(typeof updates.allowNightly === 'boolean' ? { allowNightly: updates.allowNightly } : {}) };
  }
  return limits;
}

/** `serverConfig.managedPolicy`: the summary, or undefined when the environment is not managed. */
export function managedPolicySummary(config: Obj): ManagedPolicySummary | undefined {
  const value = config.managedPolicy;
  if (!isRecord(value)) return undefined;
  const organization = trimmed(value.organization), lockedSettings = strings(value.lockedSettings);
  const supportUrl = value.supportUrl === undefined ? undefined : trimmed(value.supportUrl);
  if (organization === undefined || lockedSettings === undefined || (value.supportUrl !== undefined && supportUrl === undefined)) return undefined;
  const limits = value.limits === undefined ? undefined : isRecord(value.limits) ? limitsOf(value.limits) : null;
  if (limits === null) return undefined;
  return { organization, ...(supportUrl ? { supportUrl } : {}), lockedSettings, ...(limits ? { limits } : {}) };
}

/** Whether `mode` grants the agent more than `ceiling`; no ceiling allows every mode. */
export const isRuntimeModeAbove = (mode: RuntimeMode, ceiling: RuntimeMode | undefined): boolean =>
  ceiling !== undefined && RUNTIME_MODES.indexOf(mode) > RUNTIME_MODES.indexOf(ceiling);

/** The message every surface shows when the policy stops an update. */
export const managedUpdatesMessage = (organization: string): string => `${organization} manages T3 Code updates.`;
/** The message when the policy allows only stable releases. */
export const managedNightlyDeniedMessage = (organization: string): string => `${managedUpdatesMessage(organization)} Only stable releases can be installed.`;
/** "Example Corp's policy does not allow webhook tasks." */
export const managedPolicyDeniedMessage = (organization: string, capability: string): string => `${organization}'s policy does not allow ${capability}.`;
/** "Example Corp limits threads to Auto-accept edits." */
export const managedRuntimeModeCeilingMessage = (organization: string, ceiling: RuntimeMode): string => `${organization} limits threads to ${RUNTIME_MODE_LABELS[ceiling]}.`;
/** Why a provider the policy leaves out cannot be used (ServerSettingsError `use-disallowed-provider`). */
export const disallowedProviderMessage = (organization: string | undefined, label: string): string =>
  `${organization === undefined ? 'Your organization' : `${organization}'s`} policy does not allow the ${label} provider.`;

/** Whether `path` is locked itself, sits inside a locked setting, or contains one. */
export function isManagedSettingPath(summary: Pick<ManagedPolicySummary, 'lockedSettings'> | undefined, path: string): boolean {
  return summary !== undefined && summary.lockedSettings.some(locked => locked === path || locked.startsWith(`${path}.`) || path.startsWith(`${locked}.`));
}

/** Why `mode` is unavailable on an environment with `summary`, or undefined when it is allowed. */
export function managedRuntimeModeDenial(summary: ManagedPolicySummary | undefined, mode: RuntimeMode): string | undefined {
  const ceiling = summary?.limits?.maxRuntimeMode;
  return summary !== undefined && ceiling !== undefined && isRuntimeModeAbove(mode, ceiling) ? managedRuntimeModeCeilingMessage(summary.organization, ceiling) : undefined;
}

/** `mode` when the policy allows it, else the broadest of `modes` (narrowest first) it does; undefined when none is. */
export function clampManagedRuntimeMode(summary: ManagedPolicySummary | undefined, mode: RuntimeMode, modes: readonly RuntimeMode[] = RUNTIME_MODES): RuntimeMode | undefined {
  const ceiling = summary?.limits?.maxRuntimeMode;
  if (!isRuntimeModeAbove(mode, ceiling)) return mode;
  return modes.findLast(candidate => !isRuntimeModeAbove(candidate, ceiling));
}

const CAPABILITY_LABELS = {
  remoteClients: 'connecting from other devices',
  connect: 'T3 Connect',
  webhookTasks: 'webhook tasks',
  browserCookieImport: 'importing browser cookies',
} as const;

/** Why a capability is unavailable on an environment with `summary`, or undefined when allowed. */
export function managedCapabilityDenial(summary: ManagedPolicySummary | undefined, capability: keyof typeof CAPABILITY_LABELS): string | undefined {
  const limits = summary?.limits;
  if (summary === undefined || limits === undefined) return undefined;
  const denied = capability === 'remoteClients' ? limits.clients !== undefined && !limits.clients.includes('remote') : limits[capability] === false;
  return denied ? managedPolicyDeniedMessage(summary.organization, CAPABILITY_LABELS[capability]) : undefined;
}

/** Whether the policy lets `driver` run. */
export function isManagedDriverAllowed(summary: ManagedPolicySummary | undefined, driver: string): boolean {
  const allowed = summary?.limits?.providers?.allowedDrivers;
  return allowed === undefined || allowed.includes(driver);
}

/** Why updates are unavailable on an environment with `summary`, or undefined when allowed. */
export const managedUpdatesDenial = (summary: ManagedPolicySummary | undefined): string | undefined =>
  summary?.limits?.updates?.mode === 'off' ? managedUpdatesMessage(summary.organization) : undefined;
