// Lane r12-threads: a device session floats over the chat (T3 Code, MIT, see LICENSE-T3:
// ChatView.tsx's device-session effects with autoShowFloatingPreview, which the clone keeps on
// as the reference's default; ChatView.logic.ts shouldRenderPreviewMiniPlayer;
// previewMiniPlayerStore.ts). ChatView reads the environment's device state for every open
// thread. A session that is new for the thread floats as the player; the first device state a
// thread sees is its baseline, so opening a thread never resurrects a closed player, and a session
// whose device summary has not arrived stays out of the baseline. Sheet layouts (window ≤ 980 pt)
// float nothing. A floating device follows its session, and the player hides while the panel shows
// the same device.
// Lane r13-panels: the reference's own rule for a load, not a launch flag. useDeviceState's
// `loaded` is `query.data !== undefined`, but useEnvironmentQuery's data is `A | null`
// (Option.getOrNull), so `loaded` already holds before the subscription's first chunk and the
// effects read EMPTY_DEVICE_STATE then. A thread shown before its environment's device state
// arrives therefore takes an empty baseline and floats its open session when the state lands (a
// page loaded straight onto the thread, as the f870c41 oracle shows); a thread first shown after
// the state arrived takes that state as its baseline and floats nothing (navigating to it). The
// subscription atom follows its stream across reconnects (followStreamInEnvironment), so a reconnect
// keeps the environment's last state until the next chunk instead of reading empty again.
import type { T3Client } from './client';
import { arr, str, type Obj } from './domain';
import type { Native } from './protocol';
import { deviceStateOf, watchDevice } from './r4-surfaces-device';
import { closeMiniDevice, deviceThreadId, emptyMini, floatMiniDevice, miniDeviceOf, type DeviceTarget, type R6DeviceMini } from './r6-media-device';

type Track = { previous: Map<string, Set<string>>; tried: number };
let tracks = new WeakMap<T3Client, Track>();
let received = new WeakMap<T3Client, { environmentId: string; state: Obj }>();
/** Test seam: a fresh launch (no thread has a baseline, no device state has arrived). */
export function resetLaunch(): void { tracks = new WeakMap(); received = new WeakMap(); }

/** state/device.ts EMPTY_DEVICE_STATE: what the effects read before the first chunk. */
export const EMPTY_DEVICE_STATE: Obj = { hosts: [], hostStatus: 'disabled', hostStatuses: {}, devices: [], sessions: [],
  onboardingCompleted: false, agentAccessEnabled: false, hubBasePath: '/api/device-hub', revision: 0 };

const sessionKey = (session: Obj) => `${str(session.hostId)}:${str(session.deviceId)}`;

/** The device effects of ChatView for the open thread. `sheet`: shouldUsePlanSidebarSheet. `latest`: this
 *  connection's newest device state, null until its first chunk (then the environment's last one, or
 *  EMPTY_DEVICE_STATE before any arrived). */
export function autoShowDevices(client: T3Client, latest: Obj | null, sheet: boolean): void {
  const kept = received.get(client);
  if (latest) received.set(client, { environmentId: client.environmentId, state: latest });
  const state = latest ?? (kept && kept.environmentId === client.environmentId ? kept.state : EMPTY_DEVICE_STATE);
  const threadId = deviceThreadId(client); // activeThreadRef: a draft's own id too
  if (!threadId || !client.environmentId) return;
  let track = tracks.get(client);
  if (!track) { track = { previous: new Map(), tried: -1 }; tracks.set(client, track); }
  const threadKey = `${client.environmentId}:${threadId}`;
  const sessions = arr(state.sessions).filter(session => str(session.threadId) === threadId);
  const deviceFor = (session: Obj) => arr(state.devices).find(device => str(device.hostId) === str(session.hostId) && str(device.id) === str(session.deviceId));
  const previous = track.previous.get(threadKey);
  track.previous.set(threadKey, new Set(sessions.filter(session => deviceFor(session) !== undefined).map(sessionKey)));
  if (previous && !sheet) {
    for (const session of sessions) {
      if (previous.has(sessionKey(session))) continue;
      const device = deviceFor(session);
      if (!device) continue;
      floatMiniDevice(client, threadId, { hostId: str(session.hostId), deviceId: str(session.deviceId),
        platform: str(device.platform) === 'android' ? 'android' : 'ios', name: str(device.name) });
    }
  }
  // A floating device follows its session: once the agent or another client closes the device there is nothing left to stream.
  const source = miniDeviceOf(client, threadId);
  if (source && !arr(state.sessions).some(session => str(session.threadId) === threadId && str(session.hostId) === source.hostId && str(session.deviceId) === source.deviceId)) {
    closeMiniDevice(client, threadId);
  }
}

/** useDeviceState for the focused environment (drafts included): one subscription attempt per connection. */
export async function watchThreadDevices(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available || !client.ready) return;
  let track = tracks.get(client);
  if (!track) { track = { previous: new Map(), tried: -1 }; tracks.set(client, track); }
  if (track.tried === client.generation) return;
  track.tried = client.generation;
  await watchDevice(client, native);
}

/** ChatView's device effects for one answer of the shell, then the state they read. */
export async function threadDevices(client: T3Client, native: Native | null | undefined, sheet: boolean): Promise<void> {
  await watchThreadDevices(client, native);
  autoShowDevices(client, deviceStateOf(client), sheet);
}

/** shouldRenderPreviewMiniPlayer: the player hides while the rendered panel surface is the same device. Its frame
 *  is the chat canvas's (chat-canvas-view.ts), which applies the same rule. */
export function visibleMini(mini: R6DeviceMini, shown: DeviceTarget | undefined): R6DeviceMini {
  if (!mini.show) return mini;
  return shown && shown.hostId === mini.hostId && shown.deviceId === mini.deviceId ? emptyMini() : mini;
}
