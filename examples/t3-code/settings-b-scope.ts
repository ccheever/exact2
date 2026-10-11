// Lane settings-b: the Providers route's single-environment scope
// (routes/settings.tsx singleEnvironment, SettingsScopeContext
// selectSingleEnvironmentScope, SettingsScopeSentence EnvironmentScopeMenu).
// The machine axis never offers "All environments" there: it defaults to the
// primary environment, so the sentence reads "… on <machine>" with its icon.
import type { T3Client } from './client';
import type { ScopeChoice } from './settings-core';
import { machineKind } from './connections';
import { isPrimaryEnvironment } from './local-primary';

export const singleEnvironmentRoute = (route: string) => route === 'providers';

/**
 * The machine a single-environment route resolves when none is chosen. Given the scope's
 * environments (an unavailable scope has none), it is selectSingleEnvironmentScope's choice:
 * the primary, else a connected one, else the first; without them, the focused environment.
 */
export function scopeMachine(client: Pick<T3Client, 'environmentId'>, route: string, machine: string,
  candidates?: readonly { environmentId: string; connection: { phase: string } }[]): string {
  if (!singleEnvironmentRoute(route) || machine) return machine;
  if (!candidates) return client.environmentId;
  const chosen = candidates.find(candidate => isPrimaryEnvironment(candidate.environmentId))
    ?? candidates.find(candidate => candidate.connection.phase === 'connected') ?? candidates[0];
  return chosen?.environmentId ?? '';
}

/**
 * Environment choices for the scope menu: a single-environment route drops
 * "All environments"; every environment row carries its machine kind in
 * `mark` so the menu draws EnvironmentMachineIcon beside it.
 */
export function environmentScopeChoices(client: Pick<T3Client, 'config'>, route: string, choices: ScopeChoice[]): ScopeChoice[] {
  const kind = machineKind(client.config);
  return (singleEnvironmentRoute(route) ? choices.filter(choice => choice.id) : choices).map(choice => choice.id ? { ...choice, mark: kind } : choice);
}

/** The trigger's icon: the selected environment's kind, none for "All environments". */
export function environmentScopeIcon(client: Pick<T3Client, 'config' | 'environmentId'>, machine: string): string {
  return machine && machine === client.environmentId ? machineKind(client.config) : '';
}
