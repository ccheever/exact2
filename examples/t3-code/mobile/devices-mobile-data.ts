// @ref llp/1107.010-mobile-browser-devices.decision.md#connection-and-command-ownership
// Pinned365aa87982 DevicePreviewRouteScreen over existing device stream state and RPC ownership.
import { mobileClient } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, num, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { watchDevice, deviceStateOf, DEVICE_STATE_KEY } from './shared/r4-surfaces-device';
import { mobileStreamOwner, assertMobileStreamOwner, mobileStreamNative, mobileStreamPermission, mobileStreamDescriptor } from './browser-mobile-owner';
import { threadDevicePreviews, selectedThreadDevicePreview } from './devices-mobile-model';
import { previewMenuItem, type MobilePreviewMenu } from './browser-mobile-data';

export interface MobileDevicesSnapshot { owner: string; revision: number; title: string; source: string; count: number; loaded: boolean;
  loading: boolean; error: string; close: boolean; selected: string; devices: MobilePreviewMenu[]; menu: MobilePreviewMenu[];
  inputConnected: boolean; controlsVisible: boolean; status: string; detail: string; toolVersions: string; operating: boolean; needsRefresh: boolean }
interface State { owner: string; allowed: boolean; operate: boolean; selected: string; loading: boolean; error: string; native: Obj; operating: boolean; retry: number; lastSeq: number; serial: number }
const states = new WeakMap<T3Client, State>();
function stateOf(client: T3Client): State {
  const owner = mobileStreamOwner(client); let state = states.get(client);
  if (!state || state.owner !== owner) { state = { owner, allowed: false, operate: false, selected: '', loading: false, error: '', native: {}, operating: false, retry: 0, lastSeq: 0, serial: 0 }; states.set(client, state); }
  return state;
}
const previewsOf = (client: T3Client) => { const state = deviceStateOf(client); return threadDevicePreviews(state ? { hosts: arr(state.hosts), devices: arr(state.devices), sessions: arr(state.sessions) } : null, client.threadId); };
function toolVersions(host: Obj, status: Obj) {
  const tools = obj(host.tools), values = [obj(tools.hub), obj(tools.agent)];
  const ownership = "This environment's T3 server chooses device tool versions for itself and its SSH hosts. Update that server to receive newer tool versions; updating only your browser or mobile app does not update a remote server.";
  const outdated = values.some(tool => Array.isArray(tool.installedVersions) && tool.installedVersions.length > 0 && !tool.installedVersions.includes(tool.requiredVersion ?? ''));
  const policy = !host.tools ? 'Versions have not been checked. Reconnect the host and check versions.' : outdated
    ? 'Update pending. Required tools will install automatically when next used. The host needs network access; an older install is not used as a fallback.'
    : 'Required tools are installed automatically when needed. Checking versions does not install or start anything.';
  const labels = !host.tools ? ['Device tool versions have not been checked.'] : values.map((tool, index) => `${index === 0 ? 'Device hub' : 'Agent tools'}: installed ${Array.isArray(tool.installedVersions) && tool.installedVersions.length ? tool.installedVersions.join(', ') : 'none'}; required ${str(tool.requiredVersion)}${tool.runningVersion ? `; running ${tool.runningVersion}` : ''}.`);
  return ownership + '\n\n' + policy + '\n\n' + labels.join('\n') + '\n' + (str(host.toolInspectionError) || str(status.detail));
}
export function mobileDevicesSnapshot(owner = mobileStreamOwner(mobileClient), client: T3Client = mobileClient): MobileDevicesSnapshot {
  const state = stateOf(client), valid = !!owner && owner === state.owner, service = valid && state.allowed ? deviceStateOf(client) : null;
  const previews = service ? previewsOf(client) : [], preview = selectedThreadDevicePreview(previews, state.selected);
  if (preview && preview.key !== state.selected) { state.selected = preview.key; state.native = {}; }
  const native = valid ? state.native : {};
  const inputConnected = native.inputConnected === true && state.operate;
  const hostId = str(preview?.session.hostId), host = arr(service?.hosts).find(host => host.id === hostId) ?? {}, hostStatus = obj(obj(service?.hostStatuses)[hostId]);
  return { owner, revision: client.revision, title: preview ? str(preview.name) : 'Devices', count: previews.length,
    source: preview ? JSON.stringify({ ...mobileStreamDescriptor(client, owner), hostId, deviceId: preview.session.deviceId, platform: preview.session.platform, operate: state.operate }) : '',
    loaded: service !== null, loading: valid && state.loading, error: valid ? state.error : '', close: service !== null && previews.length === 0,
    selected: preview?.key ?? '', devices: previews.map(device => ({ ...previewMenuItem(device.key, str(device.name), '', device.key === preview?.key), subtitle: device.description })),
    menu: [...arr(service?.hosts).filter(host => service?.supportsHostRetry === true && obj(obj(service?.hostStatuses)[str(host.id)]).status === 'failed')
      .map(host => previewMenuItem(`retry:${host.id}`, `Retry ${host.label}`, 'arrow.clockwise', false, state.operating)),
      ...(service?.supportsToolInspection === true ? [previewMenuItem('inspect', 'Check device tool versions', 'arrow.clockwise', false, state.operating)] : []),
      previewMenuItem('tools', 'Device tool versions', 'info.circle'), previewMenuItem('reload', 'Reload stream', 'arrow.clockwise', false, !preview || state.operating),
      ...(preview?.session.platform === 'android' ? [previewMenuItem('back', 'Back', 'arrow.left', false, !inputConnected)] : []),
      previewMenuItem('appSwitcher', 'App switcher', 'square.on.square', false, !inputConnected),
      ...(preview?.session.platform === 'ios' ? [previewMenuItem('rotate', 'Rotate device', 'arrow.clockwise', false, !inputConnected)] : []),
      previewMenuItem('shutdown', state.operating ? 'Shutting down…' : 'Shut down device', 'power', false, !preview || state.operating || !state.operate)],
    inputConnected, controlsVisible: native.controlsVisible !== false, status: str(native.status, 'connecting'), detail: str(native.detail),
    toolVersions: toolVersions(host, hostStatus), operating: state.operating, needsRefresh: state.retry > 0 };
}
/** Observe retry markers before shared drain; device snapshots still use the shared reducer. */
export function mobileDevicesEvents(entries: unknown, client: T3Client = mobileClient) {
  const state = states.get(client); if (!state || state.owner !== mobileStreamOwner(client)) return;
  for (const entry of arr(entries)) {
    if (entry.key !== DEVICE_STATE_KEY || num(entry.generation, -1) !== client.generation) continue;
    const seq = num(entry.seq); if (seq > 0 && seq <= state.lastSeq) continue; state.lastSeq = Math.max(state.lastSeq, seq);
    const event = obj(entry.value);
    if (event._retryDue) state.retry++;
    else if (event._transportError) state.error = str(obj(event._transportError).message);
  }
}
export async function mobileDevicesRead(owner: string, nativeInput: Native, client: T3Client = mobileClient) {
  const state = stateOf(client), serial = ++state.serial, retry = state.retry; state.loading = true;
  try { const native = mobileStreamNative(client, owner, nativeInput, () => state.serial === serial); state.operate = await mobileStreamPermission(client, native, 'orchestration:operate');
    state.allowed = true;
    if (retry > 0) { await client.restAccess(native).call({ op: 'subscribe', key: DEVICE_STATE_KEY, method: 'subscribeDeviceState', payload: {} }); if (state.retry === retry) state.retry = 0; }
    else await watchDevice(client, native);
    assertMobileStreamOwner(client, owner);
    if (states.get(client) !== state || serial !== state.serial) throw new ClientError('A newer preview read replaced this one.', 'superseded');
    state.error = ''; }
  catch (error) { if (letGo(error)) throw error; if (error instanceof ClientError && error.kind === 'permission') state.allowed = false; state.error = error instanceof Error ? error.message : 'Devices unavailable'; }
  finally { if (serial === state.serial) { state.loading = false; if (state.retry === retry) state.retry = 0; } client.revision++; }
  return mobileDevicesSnapshot(owner, client);
}
export async function mobileDevicesStatus(owner: string, nativeInput: Native, client: T3Client = mobileClient) {
  const state = stateOf(client), selected = mobileDevicesSnapshot(owner, client).selected;
  const native = mobileStreamNative(client, owner, nativeInput); native.watch('t3.mobile-devices');
  const result = await bridgeReply(native, { op: 'mobileDevices', action: 'status', owner, generation: client.generation });
  const value = obj(result.value);
  if (result.ok && states.get(client) === state && mobileDevicesSnapshot(owner, client).selected === selected && JSON.stringify([str(value.hostId), str(value.deviceId)]) === selected) state.native = value;
  return mobileDevicesSnapshot(owner, client);
}
export async function mobileDevicesAction(owner: string, op: string, arg: string, nativeInput: Native, client: T3Client = mobileClient) {
  let message = '', ownsOperation = false; const state = stateOf(client);
  try {
    const native = mobileStreamNative(client, owner, nativeInput), snapshot = mobileDevicesSnapshot(owner, client);
    if (op === 'select') { if (!snapshot.devices.some(device => device.id === arg)) throw new ClientError('That device is no longer open.'); state.selected = arg; state.native = {}; }
    else if (op === 'refresh') return { message: '', data: await mobileDevicesRead(owner, nativeInput, client) };
    else if (op === 'home' || op === 'controls' || op === 'reload' || ['back', 'appSwitcher', 'rotate'].includes(op)) {
      if (op !== 'controls' && op !== 'reload' && !snapshot.inputConnected) throw new ClientError('Device input is not connected.');
      const result = await bridgeReply(native, { op: 'mobileDevices', action: op === 'reload' ? 'reconnect' : op === 'controls' ? 'controls' : 'command', owner, input: op, generation: client.generation });
      if (!result.ok) throw new ClientError(result.error!.message);
    } else {
      const option = snapshot.menu.find(row => row.id === op); if (!option || option.disabled) throw new ClientError('This device option is unavailable.');
      if (op === 'tools') return { message: snapshot.toolVersions, data: snapshot };
      if (state.operating) throw new ClientError('A device action is already running.'); state.operating = true; ownsOperation = true;
      if (op === 'shutdown') {
        const preview = selectedThreadDevicePreview(previewsOf(client), snapshot.selected); if (!preview) throw new ClientError('That device is no longer open.');
        if (!await mobileStreamPermission(client, native, 'orchestration:operate')) throw new ClientError('This connection cannot operate devices.');
        await client.restAccess(native).request('device.shutdown', { hostId: preview.session.hostId, deviceId: preview.session.deviceId, platform: preview.session.platform }, true);
      } else await client.restAccess(native).request('device.list', op === 'inspect' ? { inspectOnly: true } : { retryHostId: op.slice(6) }, true);
    }
  } catch (error) { if (letGo(error)) throw error; message = error instanceof Error ? error.message : 'Could not change this device'; }
  finally { if (ownsOperation) state.operating = false; client.revision++; }
  return { message, data: mobileDevicesSnapshot(owner, client) };
}
