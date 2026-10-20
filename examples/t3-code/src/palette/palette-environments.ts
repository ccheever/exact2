// Adapted from T3 Code 9c161462c9 (MIT); see LICENSE-T3.
// The environments the palette names: Add project's Environments step
// (CommandPalette.tsx addProjectEnvironmentOptions, buildEnvironmentItem and
// resolveEnvironmentOptionLabel in BranchToolbar.logic.ts) and the location a
// project row shows (useEnvironmentLookups' projectEnvironmentLocationById).
// The focused environment is T3Client's own connection; every other connected one
// is a fleet entry (settings-b-fleet.ts) with its own transport.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { fleet, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { machineKind } from '../connections/connections';
import { isPrimaryEnvironment } from '../connections/local-primary';
import { environmentRequest } from '../pull-requests/pages-pr-environments';

export type AddEnvironment = {
  environmentId: string; label: string; isPrimary: boolean; machine: string;
  /** T3Client's own connection; else a fleet entry's. */
  focused: boolean; config: Obj; generation: number; projects: Obj[];
};

const GENERIC_LOCAL_LABELS = new Set(['local', 'local environment']);
/** resolveEnvironmentOptionLabel: the primary keeps a non-generic name, else "This device"; others their label, else their id. */
export function environmentOptionLabel(isPrimary: boolean, environmentId: string, runtimeLabel: string, savedLabel = ''): string {
  const runtime = runtimeLabel.trim(), saved = savedLabel.trim();
  if (isPrimary) return [runtime, saved].find(label => label && !GENERIC_LOCAL_LABELS.has(label.toLowerCase())) ?? 'This device';
  return runtime || saved || environmentId;
}

/**
 * addProjectEnvironmentOptions: every connected environment (canCreateProjectInEnvironment), the primary
 * first, then by label. A background environment counts once its shell is synchronized.
 */
export function addProjectEnvironments(client: T3Client, source: EnvironmentFleet = fleet): AddEnvironment[] {
  const options: AddEnvironment[] = [];
  const option = (environmentId: string, config: Obj, savedLabel: string, focused: boolean, generation: number, projects: Obj[]): AddEnvironment => {
    const isPrimary = isPrimaryEnvironment(environmentId);
    return { environmentId, isPrimary, focused, config, generation, projects, machine: machineKind(config),
      label: environmentOptionLabel(isPrimary, environmentId, str(obj(config.environment).label), savedLabel) };
  };
  if (client.environmentId && client.ready) options.push(option(client.environmentId, client.config, '', true, client.generation, client.shell.projects));
  for (const entry of source.entries.values()) {
    if (!entry.environmentId || options.some(known => known.environmentId === entry.environmentId)) continue;
    if (entry.phase !== 'connected' || entry.synchronized !== entry.generation) continue;
    const saved = source.saved.find(item => str(item.environmentId) === entry.environmentId);
    options.push(option(entry.environmentId, entry.config, str(saved?.label), false, entry.generation, entry.shell.projects));
  }
  return options.sort((left, right) => left.isPrimary !== right.isPrimary ? (left.isPrimary ? -1 : 1) : left.label.localeCompare(right.label));
}

/** The environment an add-project page names (`…/env:<id>/…`); '' when it names none. */
export function pageEnvironmentId(page: string): string {
  const segment = page.split('/').find(part => part.startsWith('env:'));
  return segment ? segment.slice(4) : '';
}
/**
 * browseEnvironmentId: the environment the page chose, else the default (the first connected option); the
 * root's New project page takes the first that can make one (openNewProjectFlow). Null when the chosen one
 * is no longer connected, or there is none.
 */
export function addTarget(client: T3Client, page: string, source: EnvironmentFleet = fleet): AddEnvironment | null {
  const options = addProjectEnvironments(client, source), chosen = pageEnvironmentId(page);
  if (page === 'new-project') return options.find(option => str(option.config.newProjectsRoot)) ?? null;
  return (chosen ? options.find(option => option.environmentId === chosen) : options[0]) ?? null;
}
/** One request to the target's server: the focused connection's own, or its background transport's. */
export function targetRequest(client: T3Client, native: Native, target: AddEnvironment, method: string, payload: Obj, write = false, source: EnvironmentFleet = fleet): Promise<Obj> {
  return target.focused ? client.restAccess(native).request(method, payload, write) : environmentRequest(client, native, target.environmentId, method, payload, { write }, source);
}

export type ProjectLocation = { label: string; machine: string; remote: boolean };
/**
 * projectEnvironmentLocationById: the primary is "Local"; another environment is remote, named by its
 * label, with its machine glyph. An environment this window does not know is "Remote".
 */
export function projectLocation(client: T3Client, environmentId: string, source: EnvironmentFleet = fleet): ProjectLocation {
  const id = environmentId || client.environmentId;
  if (isPrimaryEnvironment(id)) return { label: 'Local', machine: '', remote: false };
  if (id === client.environmentId) return { label: str(obj(client.config.environment).label) || id, machine: machineKind(client.config), remote: true };
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === id);
  if (!entry) return { label: 'Remote', machine: 'server', remote: true };
  const saved = source.saved.find(item => str(item.environmentId) === id);
  return { label: str(obj(entry.config.environment).label) || str(saved?.label) || id, machine: machineKind(entry.config, str(saved?.machine)), remote: true };
}
