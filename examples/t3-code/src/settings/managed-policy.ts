// What the web client derives from an environment's managed policy (client/managed-policy.ts reads the
// summary and has the contract's messages and checks). Reference: T3 Code 5d3d78ab93 (MIT, LICENSE-T3)
// apps/web/src/state/managedPolicy.ts and settingsLayout.tsx SettingsRow's managedBy.
import type { Obj } from '../client/domain';
import { isManagedSettingPath, managedPolicySummary, type ManagedPolicySummary } from '../client/managed-policy';
import { fleet, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { primaryEnvironmentId } from '../connections/local-primary';

/**
 * useManagedPolicy: the policy of the environment with this id, read from its live config (the focused
 * connection's, else its fleet entry's), or undefined when it is unmanaged or unknown.
 */
export function managedPolicyFor(client: { environmentId: string; config: Obj }, environmentId: string | null | undefined, source: EnvironmentFleet = fleet): ManagedPolicySummary | undefined {
  if (!environmentId) return undefined;
  if (environmentId === client.environmentId) return managedPolicySummary(client.config);
  for (const entry of source.entries.values()) if (entry.environmentId === environmentId) return managedPolicySummary(entry.config);
  return undefined;
}
/** usePrimaryManagedPolicy: this machine's policy, which the primary environment reports. */
export const primaryManagedPolicy = (client: { environmentId: string; config: Obj }, source: EnvironmentFleet = fleet) =>
  managedPolicyFor(client, primaryEnvironmentId() ?? client.environmentId, source);

/**
 * Why a control acting on several environments is unavailable: the first denial any target's
 * policy gives, since one refusing target refuses the whole action.
 */
export function firstManagedDenial(policies: Iterable<ManagedPolicySummary | undefined>, deny: (policy: ManagedPolicySummary | undefined) => string | undefined): string | undefined {
  for (const policy of policies) {
    const denial = deny(policy);
    if (denial !== undefined) return denial;
  }
  return undefined;
}

/** What usage-data notices say when the policy turns product analytics off. */
export const managedTelemetryOffMessage = (policy: ManagedPolicySummary | undefined) =>
  policy?.limits?.telemetry === false ? `${policy.organization} turned off anonymous usage data.` : undefined;

export interface ManagedInstanceLocks {
  /** Keys inside the instance's `config` users cannot change. */
  readonly configKeys: ReadonlySet<string>;
  /** Environment variables users cannot edit, rename, or remove. */
  readonly environmentNames: ReadonlySet<string>;
  /** Under a runtime-mode ceiling every variable is the administrator's, not only named ones. */
  readonly wholeEnvironment: boolean;
  readonly reason: string;
}
/** What the policy pins on instances of `driver`, or undefined when nothing is. */
export function managedInstanceLocks(policy: ManagedPolicySummary | undefined, driver: string): ManagedInstanceLocks | undefined {
  const paths = policy?.limits?.providers?.lockedInstanceFields?.[driver];
  if (policy === undefined || paths === undefined || paths.length === 0) return undefined;
  const fieldsUnder = (prefix: string) => new Set(paths.flatMap(path => path.startsWith(prefix) ? [path.slice(prefix.length)] : []));
  return { configKeys: fieldsUnder('config.'), environmentNames: fieldsUnder('environment.'), wholeEnvironment: paths.includes('environment'), reason: `Managed by ${policy.organization}.` };
}

/**
 * SettingsRow's managedBy: "Managed by <organization>." when any target's policy locks one of the
 * row's setting keys or dot paths, an administrator's lock winning over every other reason a control is inert.
 */
export function managedSettingReason(policies: Iterable<ManagedPolicySummary | undefined>, paths: readonly string[]): string | undefined {
  if (paths.length === 0) return undefined;
  for (const policy of policies) if (policy !== undefined && paths.some(path => isManagedSettingPath(policy, path))) return `Managed by ${policy.organization}.`;
  return undefined;
}

/** ManagedByOrganization: the notice's line and its Get help link ('' without one). */
export const managedByOrganization = (policy: ManagedPolicySummary) => ({ text: `This environment is managed by ${policy.organization}.`, helpUrl: policy.supportUrl ?? '' });
