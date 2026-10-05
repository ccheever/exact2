// The Device surface (lane r4-surfaces; MIT reference, see LICENSE-T3:
// components/device/DeviceSetup.tsx, DevicePanel.tsx, DeviceHostUpdates.tsx,
// state/device.ts): the environment's device hub state from `subscribeDeviceState`,
// the "Set up devices" wizard (device.configure / device.list) and the panel's
// simulator list. Opening a device stream is not part of this client.
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { subscriptionSerial } from './shell-vcs';
import { emptyR6Device, type R6DeviceView } from './r6-media-device'; // lane r6-media: opening a device and its workspace

export const DEVICE_STATE_KEY = 'r4-device-state';
export const HUB_DESCRIPTION = 'Enable this environment to open simulators and emulators, whether they run here or on a remote device host.';
export const AGENT_DESCRIPTION = 'Allow new agent sessions in this environment to start and control local and remote devices, with required tools set up automatically.';

export type PlatformStatus = { id: string; platform: string; ready: boolean; message: string };
export type HostNotice = { id: string; label: string; detail: string; failed: boolean };
export type DeviceRow = { id: string; name: string; description: string; action: string; platform: string };
export type DeviceGroup = { id: string; label: string; devices: DeviceRow[] };
export type DeviceView = {
  loaded: boolean; step: number; enabled: boolean; busy: boolean; pending: string; hostStatus: string;
  hubStatus: string; hubReady: boolean; platforms: PlatformStatus[]; agentEnabled: boolean; agentStatus: string; agentReady: boolean;
  failure: string; canContinue: boolean; doneLabel: string; notices: HostNotice[]; groups: DeviceGroup[]; empty: string; hint: string;
  panelStatus: string; refreshable: boolean; r6: R6DeviceView;
};
type DeviceStore = { generation: number; id: string; floor: number; maxSeen: number; state: Obj | null; step: number; pending: string; tried: boolean };
const stores = new WeakMap<T3Client, DeviceStore>();
function store(client: T3Client): DeviceStore {
  let value = stores.get(client);
  if (!value || value.generation !== client.generation) {
    value = { generation: client.generation, id: '', floor: value?.maxSeen ?? 0, maxSeen: value?.maxSeen ?? 0, state: null, step: 0, pending: '', tried: false };
    stores.set(client, value);
  }
  return value;
}

/** One `r4-device-state` inbox entry (client.ts drain): the newest stream's snapshot wins. */
export function deviceStateEvent(client: T3Client, entry: Obj): void {
  const value = stores.get(client);
  if (!value) return;
  const id = str(entry.subscriptionId), serial = subscriptionSerial(id);
  value.maxSeen = Math.max(value.maxSeen, serial);
  if (serial <= value.floor || (value.id && serial < subscriptionSerial(value.id))) return;
  value.id = id;
  const item = obj(entry.value);
  if (item._retryDue || item._streamEnded || item._transportError) return;
  if (Array.isArray(item.hosts)) value.state = item;
}
/** useDeviceState: one stream per connection while the device surface or its wizard is in use. */
export async function watchDevice(client: T3Client, native: Native): Promise<void> {
  const value = store(client);
  if (value.id || value.tried) return;
  value.tried = true; value.floor = value.maxSeen;
  try {
    const reply = await client.restAccess(native).call({ op: 'subscribe', key: DEVICE_STATE_KEY, method: 'subscribeDeviceState', payload: {} });
    const serial = subscriptionSerial(str(reply.id));
    value.maxSeen = Math.max(value.maxSeen, serial);
    if (serial > value.floor && (!value.id || serial > subscriptionSerial(value.id))) value.id = str(reply.id);
  } catch { value.tried = false; }
}
/** The newest device state this connection has read (lane r6-media). */
export const deviceStateOf = (client: T3Client): Obj | null => store(client).state;
export function deviceReady(client: T3Client): boolean {
  const state = store(client).state;
  return !!state && state.onboardingCompleted === true && str(state.hostStatus) !== 'disabled';
}

const platformName = (platform: string) => platform === 'ios' ? 'iOS' : 'Android';
/** platformSetupStatus. */
export function platformSetupStatus(state: Obj, platform: string): { ready: boolean; message: string } {
  const availability = arr(state.hosts).flatMap(host => arr(host.platforms)).find(candidate => candidate.platform === platform);
  if (!availability || availability.available !== true) return { ready: false, message: str(availability?.reason, `${platformName(platform)} support was not detected.`) };
  if (state.hostStatus === 'ready' && !arr(state.devices).some(device => device.platform === platform)) {
    return { ready: false, message: platform === 'ios' ? 'Xcode is installed, but no iOS Simulator is available. Install a runtime in Xcode Settings → Components.'
      : 'The Android SDK is installed, but no virtual device exists. Create one in Android Studio → Device Manager.' };
  }
  return { ready: true, message: platform === 'ios' ? 'Xcode and iOS Simulator are available.' : 'The Android SDK and Emulator are available.' };
}
/** DeviceHubSetupStatus / AgentDeviceSetupStatus labels. */
export function hubStatusLabel(hostStatus: string, pending: boolean): string {
  if (!pending && hostStatus !== 'ready') return '';
  if (!pending) return 'Device hub is ready.';
  return hostStatus === 'installing' ? 'Installing device hub…' : hostStatus === 'starting' ? 'Starting device hub…' : 'Updating device hub…';
}
export function agentStatusLabel(state: Obj, pending: boolean): string {
  const hostStatus = str(state.hostStatus);
  if (pending) return hostStatus === 'installing' ? 'Installing agent tools…' : hostStatus === 'starting' ? 'Starting agent tools…' : 'Updating agent access…';
  if (state.agentAccessEnabled === true && hostStatus === 'ready' && arr(state.hosts).some(host => host.agentDeviceInstalled === true)) return 'Agent tools are ready.';
  return '';
}
const EMPTY_STATE: Obj = { hosts: [], hostStatus: 'disabled', hostStatuses: {}, devices: [], sessions: [], onboardingCompleted: false, agentAccessEnabled: false };

export async function deviceView(client: T3Client, native: Native): Promise<DeviceView> {
  await watchDevice(client, native);
  const value = store(client), state = value.state ?? EMPTY_STATE, hostStatus = str(state.hostStatus, 'disabled');
  const enabled = hostStatus !== 'disabled', busy = hostStatus === 'installing' || hostStatus === 'starting';
  const statuses = obj(state.hostStatuses);
  const notices = enabled ? arr(state.hosts).flatMap((host): HostNotice[] => {
    const status = obj(statuses[str(host.id)]), kind = str(status.status);
    if (!['installing', 'starting', 'failed'].includes(kind)) return [];
    const failed = kind === 'failed';
    return [{ id: str(host.id), label: str(host.label), failed,
      detail: str(status.detail) || (failed ? 'Device support could not start.' : kind === 'installing' ? 'Installing device tools…' : 'Starting device tools…') }];
  }) : [];
  const groups = (['ios', 'android'] as const).flatMap((platform): DeviceGroup[] => {
    const devices = arr(state.devices).filter(device => device.platform === platform)
      .sort((a, b) => Number(b.booted === true) - Number(a.booted === true) || str(a.name).localeCompare(str(b.name)));
    return devices.length ? [{ id: platform, label: platform === 'ios' ? 'iOS Simulators' : 'Android Emulators', devices: devices.map(device => ({
      id: `${str(device.hostId)}\u0000${str(device.id)}`, name: str(device.name), platform,
      description: `${str(arr(state.hosts).find(host => host.id === device.hostId)?.label)} · ${str(device.version)} · ${device.booted ? 'Running' : 'Stopped'}`,
      action: device.booted ? 'Open' : 'Start' })) }] : [];
  });
  const hostReady = Object.values(statuses).some(status => obj(status).status === 'ready');
  const unavailable = arr(state.hosts).flatMap(host => arr(host.platforms).filter(platform => platform.available !== true));
  return {
    loaded: value.state !== null, step: value.step, enabled, busy, pending: value.pending, hostStatus,
    hubStatus: hubStatusLabel(hostStatus, value.pending === 'hub' || (busy && value.pending !== 'agent')), hubReady: hostStatus === 'ready' && value.pending !== 'hub',
    platforms: ['ios', 'android'].map(platform => ({ id: platform, platform: platformName(platform), ...platformSetupStatus(state, platform) })),
    agentEnabled: state.agentAccessEnabled === true, agentStatus: agentStatusLabel(state, value.pending === 'agent'), agentReady: value.pending !== 'agent' && agentStatusLabel(state, false) !== '',
    failure: hostStatus === 'failed' ? str(state.hostStatusDetail) : '', canContinue: hostStatus === 'ready' && value.pending === '',
    doneLabel: value.pending === 'complete' ? 'Saving…' : 'Done', notices, groups,
    empty: groups.length ? '' : hostStatus === 'failed' ? str(state.hostStatusDetail, 'The device hub failed to start.') : 'No simulators or emulators were found on this environment.',
    hint: hostReady && !arr(state.devices).some(device => device.platform === 'android') && !unavailable.some(platform => platform.platform === 'android')
      ? "No Android virtual devices found. Create one in Android Studio's Device Manager, then refresh." : '',
    panelStatus: value.state === null ? 'Finding devices…' : hostStatus === 'installing' ? str(state.hostStatusDetail, 'Installing device support…') : busy ? 'Finding devices…' : '',
    refreshable: value.state !== null && !busy, r6: emptyR6Device(),
  };
}
export const emptyDevice = (): DeviceView => ({ loaded: false, step: 0, enabled: false, busy: false, pending: '', hostStatus: 'disabled', hubStatus: '', hubReady: false, platforms: [],
  agentEnabled: false, agentStatus: '', agentReady: false, failure: '', canContinue: false, doneLabel: 'Done', notices: [], groups: [], empty: '', hint: '', panelStatus: '', refreshable: false, r6: emptyR6Device() });

/** `shelllocal:surface-device-*`: wizard navigation (earlier steps only, never while busy). */
export async function deviceLocal(client: T3Client, _native: Native, op: string, _id: string, value: string): Promise<string> {
  const current = store(client), state = current.state ?? EMPTY_STATE;
  const busy = ['installing', 'starting'].includes(str(state.hostStatus)) || current.pending !== '';
  if (op === 'step') { const step = Number(value); if (!busy && step >= 0 && step <= current.step) current.step = step; return ''; }
  if (op === 'back') { if (!busy && current.step > 0) current.step--; return ''; }
  if (op === 'continue') { if (state.hostStatus === 'ready' && current.pending === '' && current.step < 2) current.step++; return ''; }
  if (op === 'reset') { current.step = 0; return ''; }
  return '';
}
/** The wizard's switches and buttons as the reference's device.configure / device.list calls. */
export function configureInput(op: string, value: string): Obj {
  if (op === 'hub') return value === 'true' ? { enabled: true } : { enabled: false, agentAccessEnabled: false };
  if (op === 'agent') return { agentAccessEnabled: value === 'true' };
  if (op === 'complete') return { onboardingCompleted: true };
  throw new ClientError(`Unknown device action: ${op}`);
}
export async function deviceCommand(client: T3Client, native: Native, op: string, _id: string, value: string): Promise<string> {
  const current = store(client), access = client.restAccess(native);
  if (current.pending) return '';
  current.pending = op === 'list' ? 'check' : op;
  try {
    const result = op === 'list' ? await access.request('device.list', {}, true) : await access.request('device.configure', configureInput(op, value), true);
    if (Array.isArray(result.hosts)) current.state = result;
    if (op === 'complete') current.step = 0;
  } catch { /* useAtomCommand reports failures to the console only; the wizard keeps its state. */ }
  finally { current.pending = ''; }
  return '';
}
