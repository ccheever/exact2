// Settings → Connections › "This machine" (20261005-local-primary-environment items 4-5), after T3
// Code (MIT, see LICENSE-T3; reference 1e2ecbd975) ConnectionsSettings.tsx primarySettings and
// settings/LocalEnvironmentSetting.tsx, and ipc/methods/localEnvironment.ts for the switch.
//  - The section: the machine icon, the primary's label (else "This machine"), "More actions for this
//    machine" with the environment icon menu, the Local environment row, and while this machine can
//    be managed the Version row (`serverVersion · displayUrl`, "Loading…", "Up to date", the update's
//    progress; a desktop-managed server that cannot update from here gets the sentence instead).
//    Network access, its endpoints, Tailscale HTTPS and Authorized clients are connections-network.ts
//    (20261005-this-machine-network-access).
//  - The switch persists desktop-settings.json (decision U7), then returns a relaunch request for
//    Contract to dispatch (decision U4). A write failure stays under the dialog's description.
// A refused development build (no T3_LOCAL_HOME / T3_LOCAL_PORT) draws no primary and shows the
// refusal here only; a missing or failed runtime shows its reason here too.
import { obj, str } from './domain';
import type { Native } from './protocol';
import { writeDesktopSettings } from './local-backend';
import { canManageLocalBackend, localEnvironmentEnabled, primary, type LocalPrimary } from './local-primary';
import { versionMismatch } from './version-skew';
import { desktopManagedOnly, DESKTOP_MANAGED_NOTE } from './server-installation';
import { serverUpdateStageLabel, serverUpdateStateFor } from './server-update';
import { placeholderRow, type ConnectionResult, type SavedRowView, type Source } from './connections';
import type { T3Client } from './client';
import { letGo } from './let-go';

export const LOCAL_ON_DESCRIPTION = 'Run agents on this computer. Turn off to use T3 Code only with remote environments.';
export const LOCAL_OFF_DESCRIPTION = 'Turned off. Agents only run in remote environments.';
export const TURN_OFF = { title: 'Turn off local environment?', confirm: 'Restart and turn off',
  body: 'T3 Code will restart without running a server on this computer. Any agents and terminals running here will stop, and other devices will no longer be able to connect to this computer. Your projects, history, and remote environments are unaffected.' };
export const TURN_ON = { title: 'Turn on local environment?', confirm: 'Restart and turn on', body: 'T3 Code will restart and start running a server on this computer again.' };

/** The switch as applied: while a change runs, the value it started from (the reference's switch shows the value this process started with). */
let applyingAny: boolean | null = null;
/** The section as it stood when a change began: it holds still until the change is done (the reference relaunches then). */
let held: ReturnType<typeof project> | null = null;
/** The last change's failure, shown under the dialog's description (LocalEnvironmentSetting `error`); a new change clears it. */
let failure = '';

/** The section's projection: the primary's source and its row shape (for the icon menu), when there is a primary. */
export function thisMachine(source: Source | undefined, row: SavedRowView | null, local: LocalPrimary = primary) {
  const view = project(source, row, local);
  if (applyingAny === null) { last = view; return view; }
  return held ?? view;
}
let last: ReturnType<typeof project> | null = null;
function project(source: Source | undefined, row: SavedRowView | null, local: LocalPrimary) {
  const enabled = applyingAny ?? !local.disabled;
  const config = source?.config ?? {};
  const environment = obj(config.environment);
  const serverVersion = str(environment.serverVersion);
  const manage = canManageLocalBackend(local) && enabled;
  // ServerUpdateProgress and ServerUpdateAction, as the saved rows (server-update.ts).
  const update = source ? serverUpdateStateFor(source.environmentId, serverVersion || null) : { status: 'idle' as const };
  const mismatch = serverVersion ? versionMismatch(serverVersion) : null;
  const capabilities = obj(environment.capabilities);
  const displayUrl = local.target?.httpBaseUrl ?? '';
  const status = local.status.state === 'refused' ? local.status.refused
    : local.status.state === 'runtime-missing' || local.status.state === 'failed' ? `Connection failed: ${local.status.failure || local.status.install?.reason || 'The local server could not start.'}` : '';
  return {
    title: source?.label || local.target?.label || 'This machine',
    kind: Object.keys(config).length ? str(row?.kind, 'desktop') : 'desktop',
    menu: !!source && !!row,
    environment: row ?? placeholderRow(),
    enabled,
    description: enabled ? LOCAL_ON_DESCRIPTION : LOCAL_OFF_DESCRIPTION,
    canManage: manage,
    version: update.status !== 'idle' ? '' : [serverVersion, displayUrl].filter(Boolean).join(' · ') || 'Loading…',
    progress: update.status === 'running' ? serverUpdateStageLabel(update.stage) : '',
    progressFailed: update.status === 'failed' ? update.message : '',
    update: mismatch && source && update.status !== 'running' && !desktopManagedOnly(capabilities) ? (update.status === 'failed' ? 'Retry update' : `Update to ${mismatch.clientVersion}`) : '',
    updateNote: mismatch && source && update.status !== 'running' && desktopManagedOnly(capabilities) ? DESKTOP_MANAGED_NOTE : '',
    updateKey: source?.key ?? '', updateVersion: mismatch?.clientVersion ?? '',
    upToDate: !mismatch && update.status === 'idle' && Object.keys(config).length > 0,
    status, statusError: status !== '' && local.status.state !== 'refused',
    dialogTitle: enabled ? TURN_OFF.title : TURN_ON.title,
    dialogBody: enabled ? TURN_OFF.body : TURN_ON.body,
    dialogConfirm: enabled ? TURN_OFF.confirm : TURN_ON.confirm,
    error: failure,
  };
}
export type ThisMachineView = ReturnType<typeof project>;

/** Persist the Local environment setting before Contract can request the process relaunch. */
export async function applyLocalSetting(client: T3Client, native: Native, change: { localEnvironmentEnabled: boolean }): Promise<ConnectionResult> {
  applyingAny = localEnvironmentEnabled(client); held = last; failure = '';
  try {
    const result = await writeDesktopSettings(native, change, client);
    return { status: null, generation: -1, ...(result.changed ? { relaunch: true } : {}) };
  } catch (error) {
    if (letGo(error)) throw error;
    // The dialog shows the reason and stays open; nothing else on the page names it.
    failure = error instanceof Error && error.message ? error.message : "Couldn't change this setting.";
    return { status: null, generation: -1 };
  } finally { applyingAny = null; held = null; }
}
