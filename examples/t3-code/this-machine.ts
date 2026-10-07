// Settings → Connections › "This machine" (20261005-local-primary-environment items 4-5), after T3
// Code (MIT, see LICENSE-T3; reference 1e2ecbd975) ConnectionsSettings.tsx primarySettings and
// settings/LocalEnvironmentSetting.tsx, and ipc/methods/localEnvironment.ts for the switch.
//  - The section: the machine icon, the primary's label (else "This machine"), "More actions for this
//    machine" with the environment icon menu, the Local environment row, and while this machine can
//    be managed the Version row (`serverVersion · displayUrl`, "Loading…", "Up to date", the update's
//    progress; a desktop-managed server that cannot update from here gets the sentence instead).
//    Network access, endpoints and clients come with 20261005-this-machine-network-access.
//  - The switch (`applyLocalSetting`, one seam for every local setting): the reference persists the
//    setting and relaunches the app (decision U4). exact2 has no process relaunch (issue X45, exact2
//    #122 closed without one), so this is the stopgap: persist, then stop or start the embedded server
//    in place and reconnect, keeping the window. A failure puts the setting back and shows its reason
//    under the dialog's description.
// A refused development build (no T3_LOCAL_HOME / T3_LOCAL_PORT) draws no primary and shows the
// refusal here only; a missing or failed runtime shows its reason here too.
import { obj, str, type Obj } from './domain';
import { ClientError, bridgeReply, type Native } from './protocol';
import { parseLocalBackendStatus } from './local-backend';
import { canManageLocalBackend, focusedOnPrimary, localEnvironmentEnabled, primary, primaryEntry, setLocalEnvironmentEnabled, type LocalPrimary } from './local-primary';
import { fleet, environmentKey } from './settings-b-fleet';
import { versionMismatch } from './version-skew';
import { desktopManagedOnly, DESKTOP_MANAGED_NOTE } from './server-installation';
import { serverUpdateStageLabel, serverUpdateStateFor } from './server-update';
import { placeholderRow, type SavedRowView, type Source } from './connections';
import type { T3Client } from './client';

export const LOCAL_ON_DESCRIPTION = 'Run agents on this computer. Turn off to use T3 Code only with remote environments.';
export const LOCAL_OFF_DESCRIPTION = 'Turned off. Agents only run in remote environments.';
export const TURN_OFF = { title: 'Turn off local environment?', confirm: 'Restart and turn off',
  body: 'T3 Code will restart without running a server on this computer. Any agents and terminals running here will stop, and other devices will no longer be able to connect to this computer. Your projects, history, and remote environments are unaffected.' };
export const TURN_ON = { title: 'Turn on local environment?', confirm: 'Restart and turn on', body: 'T3 Code will restart and start running a server on this computer again.' };

/** The switch as applied: while a change runs, the value it started from (the reference's switch shows the value this process started with). */
let applyingAny: boolean | null = null;

/** The section's projection: the primary's source and its row shape (for the icon menu), when there is a primary. */
export function thisMachine(source: Source | undefined, row: SavedRowView | null, local: LocalPrimary = primary) {
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
  };
}
export type ThisMachineView = ReturnType<typeof thisMachine>;

type Result = { status: Obj | null; generation: number };
async function call(native: Native, request: Obj) {
  const reply = await bridgeReply(native, request);
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  return reply;
}

/**
 * applyLocalSetting (U4 stopgap, see the header): one local setting change. Turning off hands the
 * focus away from the primary first (to the first switched-on saved environment, else none), then
 * stops the server; turning on starts it and, with nothing else focused, connects to it. Returns
 * the focused connection's new status for T3Client to adopt.
 */
export async function applyLocalSetting(client: T3Client, native: Native, change: { localEnvironmentEnabled: boolean }): Promise<Result> {
  const enabled = change.localEnvironmentEnabled, before = localEnvironmentEnabled(client);
  if (enabled === before && primary.status.enabled === enabled) return { status: null, generation: -1 };
  applyingAny = before;
  setLocalEnvironmentEnabled(client, enabled); // T3Client saves the preference file after the command
  let result: Result = { status: null, generation: -1 };
  const local = primaryEntry();
  try {
    if (!enabled) {
      if (focusedOnPrimary(client)) {
        let reply = await call(native, { op: 'disconnect', forget: false });
        const next = fleet.saved.find(entry => entry.enabled !== false && str(entry.environmentId) && str(entry.origin));
        if (next) {
          fleet.forget(environmentKey(str(next.origin), str(next.environmentId)));
          await native.later({ op: 'fleetStop', fleet: environmentKey(str(next.origin), str(next.environmentId)) }).catch(() => undefined);
          reply = await call(native, { op: 'connect', origin: str(next.origin), credential: '' }).catch(() => reply);
        }
        result = { status: obj(reply.value), generation: reply.generation };
      }
      if (local) {
        const key = environmentKey(str(local.origin), str(local.environmentId));
        fleet.forget(key);
        await native.later({ op: 'fleetStop', fleet: key }).catch(() => undefined);
      }
    }
    const reply = await call(native, { op: 'localBackendSetEnabled', enabled });
    client.localBackend = parseLocalBackendStatus(reply.value);
    primary.update(client.localBackend, enabled);
    if (enabled && primary.connectable && primary.target && (!client.environmentId || ['disconnected', 'error'].includes(client.connection))) {
      const opened = await call(native, { op: 'connect', origin: primary.target.httpBaseUrl, primary: true });
      result = { status: obj(opened.value), generation: opened.generation };
    }
    return result;
  } catch (error) {
    setLocalEnvironmentEnabled(client, before);
    primary.update(primary.status, before);
    throw error;
  } finally { applyingAny = null; }
}
