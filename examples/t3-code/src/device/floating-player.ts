// A device an agent opens floats over its thread's chat (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3:
// components/preview/agentOpenedPreviews.ts applyDeviceSnapshot, startAgentOpenedPreviews; ChatView.logic.ts
// shouldRenderPreviewMiniPlayer; previewMiniPlayerStore.ts). Every connected environment's device state is followed for
// the life of the app, not only the open thread's, since an agent in a background thread can open a device too. The
// first snapshot of an environment is a baseline, so persisted tabs restore themselves and existing sessions never
// resurrect a player the person closed; the baseline outlives a disconnect, so a device opened during the gap still
// shows. A session that is new since the last snapshot (by thread, host, device and opening time) floats over its
// thread, or opens as that thread's panel tab with floating previews off, unless the person closed that device's tab
// (rightPanelStore.ts dismissedDeviceSurfaceIds); a session whose device summary has not
// arrived stays out of the baseline, so a later snapshot opens it. Sheet layouts (window ≤ 980 pt) get neither. A
// floating device follows its session, and the player hides while the panel shows the same device. The focused
// environment's state is this connection's (device-surface.ts); a background environment's comes from the fleet, which
// hands each snapshot to `applyDeviceSnapshot`.
import type { T3Client } from '../client/client';
import { arr, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { fleet, fleetDeviceStates, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { browserAutoShowFloatingPreview } from '../browser/browser-capture';
import { surfaceStore } from '../panel/panel-surfaces';
import { deviceSurfaceId, openDeviceSurface } from '../panel/right-panel-tabs';
import { adoptSavedDismissals } from '../panel/saved-panel';
import { deviceStateOf, watchDevice } from './device-surface';
import { closeMiniDevice, emptyMini, floatMiniDevice, miniStoreOf, type DeviceTarget, type DeviceMini } from './device-session';

/** `panels`: each thread panel's device tabs and its person's-choice revision at the last pass. */
type Track = { baselines: Map<string, Set<string>>; tried: number; panels: Map<string, { devices: Set<string>; revision: number }> };
let tracks = new WeakMap<T3Client, Track>();
const trackOf = (client: T3Client): Track => {
  let track = tracks.get(client);
  if (!track) { track = { baselines: new Map(), tried: -1, panels: new Map() }; tracks.set(client, track); }
  return track;
};
/**
 * rightPanelStore's userAction: a device tab that a person's panel change removed is dismissed; one that shows again
 * (the person opened it) is not. A person's change is one that moved the panel's userRevision (panel-surfaces.ts). The
 * dismissals are the panel's own (`dismissedDevices`), so they are saved with it (saved-panel.ts).
 */
function noteDismissedDevices(client: T3Client): void {
  const track = trackOf(client);
  for (const [key, state] of surfaceStore(client).panels) {
    const devices = new Set(state.surfaces.flatMap(surface => surface.kind === 'device' && surface.device ? [deviceSurfaceId(surface.device)] : []));
    const seen = track.panels.get(key), dismissed = new Set(state.dismissedDevices ?? []);
    for (const id of devices) dismissed.delete(id);
    if (seen && seen.revision !== state.userRevision) for (const id of seen.devices) if (!devices.has(id)) dismissed.add(id);
    if (dismissed.size || state.dismissedDevices?.length) state.dismissedDevices = [...dismissed];
    track.panels.set(key, { devices, revision: state.userRevision });
  }
}
/** Test seam: a fresh launch (no environment has a baseline). */
export function resetLaunch(): void { tracks = new WeakMap(); }

// `openedAt` tells a session apart from one that ended and reopened on the same device while this client was away.
const sessionKey = (session: Obj) => `${str(session.threadId)}:${str(session.hostId)}:${str(session.deviceId)}:${str(session.openedAt)}`;
const liveSession = (state: Obj, threadId: string, target: { hostId: string; deviceId: string }) =>
  arr(state.sessions).some(session => str(session.threadId) === threadId && str(session.hostId) === target.hostId && str(session.deviceId) === target.deviceId);
/** The environment a thread of the mini player store is in: the shell that lists it, else the focus (a draft's own id). */
function environmentOf(client: T3Client, threadId: string, source: EnvironmentFleet): string {
  if (client.shell?.threads?.some(thread => thread.id === threadId)) return client.environmentId;
  return [...source.entries.values()].find(entry => entry.shell.threads.some(thread => thread.id === threadId))?.environmentId ?? client.environmentId;
}
/** useRightPanelStore.openDevice(ref, target, true): the thread's panel gets the device's tab unless it has one or the
 *  person closed it, in this launch or (a panel not restored yet) an earlier one, whose dismissals the panel takes. */
function openDeviceTab(client: T3Client, environmentId: string, threadId: string, target: DeviceTarget): void {
  const panels = surfaceStore(client).panels, key = `${environmentId}:${threadId}`;
  const state = panels.get(key) ?? { surfaces: [], active: '', visible: false, userRevision: 0 };
  adoptSavedDismissals(client, key, state);
  if (state.dismissedDevices?.includes(deviceSurfaceId(target))) return;
  panels.set(key, state);
  if (state.surfaces.some(surface => surface.kind === 'device' && surface.device?.hostId === target.hostId && surface.device.deviceId === target.deviceId)) return;
  openDeviceSurface(state, target);
}

/** applyDeviceSnapshot: one environment's device state, as it arrives. */
export function applyDeviceSnapshot(client: T3Client, environmentId: string, state: Obj, options: { sheet: boolean; autoShowFloatingPreview: boolean }, source: EnvironmentFleet = fleet): void {
  noteDismissedDevices(client);
  const deviceFor = (session: Obj) => arr(state.devices).find(device => str(device.hostId) === str(session.hostId) && str(device.id) === str(session.deviceId));
  // A floating device of this environment whose session ended has nothing to stream.
  for (const [threadId, entry] of Object.entries(miniStoreOf(client).byThreadKey)) {
    if (entry.source.kind !== 'device' || environmentOf(client, threadId, source) !== environmentId) continue;
    if (!liveSession(state, threadId, entry.source)) closeMiniDevice(client, threadId);
  }
  const track = trackOf(client), previous = track.baselines.get(environmentId);
  track.baselines.set(environmentId, new Set(arr(state.sessions).filter(session => deviceFor(session) !== undefined).map(sessionKey)));
  if (!previous || options.sheet) return;
  for (const session of arr(state.sessions)) {
    if (previous.has(sessionKey(session))) continue;
    const device = deviceFor(session);
    if (!device) continue;
    const threadId = str(session.threadId);
    const target: DeviceTarget = { hostId: str(session.hostId), deviceId: str(session.deviceId), platform: str(device.platform) === 'android' ? 'android' : 'ios', name: str(device.name) };
    if (options.autoShowFloatingPreview) floatMiniDevice(client, threadId, target);
    else openDeviceTab(client, environmentId, threadId, target);
  }
  noteDismissedDevices(client); // the tabs just opened are the panels' own, not a later person's change
}

/** useDeviceState for the focused environment (drafts included): one subscription attempt per connection. */
export async function watchThreadDevices(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available || !client.ready) return;
  const track = trackOf(client);
  if (track.tried === client.generation) return;
  track.tried = client.generation;
  await watchDevice(client, native);
}

/**
 * The focused environment's device state, followed on each answer of the shell once its first snapshot arrived, and
 * every connected background environment's (startAgentOpenedPreviews' watchEnvironmentDevices): a device an agent opens
 * in one of their threads floats too. The fleet subscribes each one's device hub (settings-b-fleet.ts).
 */
export async function threadDevices(client: T3Client, native: Native | null | undefined, sheet: boolean, source: EnvironmentFleet = fleet): Promise<void> {
  await watchThreadDevices(client, native);
  noteDismissedDevices(client);
  const options = { sheet, autoShowFloatingPreview: browserAutoShowFloatingPreview(client) };
  const state = deviceStateOf(client);
  if (state && client.environmentId) applyDeviceSnapshot(client, client.environmentId, state, options, source);
  for (const background of fleetDeviceStates(source)) {
    if (background.environmentId !== client.environmentId) applyDeviceSnapshot(client, background.environmentId, background.state, options, source);
  }
}

/** shouldRenderPreviewMiniPlayer: the player hides while the rendered panel surface is the same device. Its frame
 *  is the chat canvas's (chat-canvas-view.ts), which applies the same rule. */
export function visibleMini(mini: DeviceMini, shown: DeviceTarget | undefined): DeviceMini {
  if (!mini.show) return mini;
  return shown && shown.hostId === mini.hostId && shown.deviceId === mini.deviceId ? emptyMini() : mini;
}
