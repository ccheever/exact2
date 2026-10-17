// Root preparation and menu projection for the pinned mobile browser/device routes.
// @ref llp/1109-t3-code-ios.rfc.md#architecture
import { mobileClient, mobileNative } from './client';
import { mobileStreamOwner, assertMobileStreamOwner } from './browser-mobile-owner';
import { mobileBrowserRead, mobileBrowserSnapshot, mobileBrowserStatus, mobileBrowserAction, mobileBrowserRelease, type MobilePreviewMenu, type MobileBrowserSnapshot } from './browser-mobile-data';
import { mobileDevicesRead, mobileDevicesSnapshot, mobileDevicesStatus, mobileDevicesAction, type MobileDevicesSnapshot } from './devices-mobile-data';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { arr, obj, str } from './shared/domain';
let observedOwner = '';
let preparationSerial = 0;
const admissionKey = (owner: string, visit: string, kind: string) => owner ? JSON.stringify([owner, visit, kind]) : '';
export function mobilePreviewOwner(active: boolean, visit: string, kind: string) {
  const owner = active ? mobileStreamOwner(mobileClient) : '';
  return { owner, visit: owner ? visit : '', kind: owner ? kind : '', admissionKey: admissionKey(owner, visit, kind) };
}
export async function mobilePreviewPrepare(owner: string, visit: string, kind: string, readsAllowed: boolean, native?: Native | null) {
  const serial = ++preparationSerial, key = admissionKey(owner, visit, kind);
  const result = (retryable = false) => ({ revision: mobileClient.revision, admissionKey: key, retryable });
  if (!native?.available) return result();
  // A session-authority supersession can ask for a fresh answer. A raw abort
  // means this answer already lost ownership and cannot schedule more work.
  let rawAborted = false, sessionSuperseded = false;
  const raw: Native = { available: native.available, watch: topic => native.watch(topic), later: async request => {
    try { return await native.later(request); }
    catch (error) { if (letGo(error)) rawAborted = true; throw error; }
  } };
  const handle = mobileNative(raw), previous = observedOwner; observedOwner = owner;
  if (previous && previous !== owner) await mobileBrowserRelease(previous, handle);
  if (!owner || !readsAllowed) return result();
  const current = () => serial === preparationSerial && owner === mobileStreamOwner(mobileClient);
  const guard = () => {
    assertMobileStreamOwner(mobileClient, owner);
    if (serial !== preparationSerial) throw new ClientError('A newer preview preparation replaced this one.', 'superseded');
  };
  const read: Native = { available: handle.available, watch: topic => { guard(); handle.watch(topic); }, later: async request => {
    guard(); let answer: unknown;
    try { answer = await handle.later(request); }
    catch (error) {
      const input = obj(request);
      if (input.op === 'http' && input.path === '/api/auth/session' && letGo(error) && !rawAborted && current()) sessionSuperseded = true;
      throw error;
    }
    guard(); return answer;
  } };
  try {
    await mobileBrowserRead(owner, read); guard();
    await mobileDevicesRead(owner, read); guard();
    return result();
  } catch (error) {
    if (letGo(error) && sessionSuperseded && !rawAborted && current()) return result(true);
    throw error;
  }
}
export function mobileBrowserPreviewStatus(owner: string, active: boolean, native?: Native | null): MobileBrowserSnapshot | Promise<MobileBrowserSnapshot> {
  return active && owner && native?.available ? mobileBrowserStatus(owner, mobileNative(native)) : mobileBrowserSnapshot(owner);
}
export function mobileDevicesPreviewStatus(owner: string, active: boolean, native?: Native | null): MobileDevicesSnapshot | Promise<MobileDevicesSnapshot> {
  return active && owner && native?.available ? mobileDevicesStatus(owner, mobileNative(native)) : mobileDevicesSnapshot(owner);
}
export async function mobilePreviewAction(kind: string, owner: string, operation: string, value: string, native?: Native | null) {
  if (!owner || !native?.available) return { message: 'This preview requires a connected native app.', revision: mobileClient.revision };
  if (operation === 'refresh') preparationSerial++;
  const result = kind === 'browser' ? await mobileBrowserAction(owner, operation, value, mobileNative(native))
    : await mobileDevicesAction(owner, operation, value, mobileNative(native));
  return { message: result.message, revision: mobileClient.revision };
}
function menuRows(input: unknown): MobilePreviewMenu[] {
  return arr(input).map(item => ({ id: str(item.id), title: str(item.title), subtitle: str(item.subtitle), symbol: str(item.symbol),
    selected: item.selected === true, disabled: item.disabled === true, destructive: item.destructive === true, group: str(item.group) }));
}
export function mobilePreviewMenus(kind: string, input: unknown) {
  const data = obj(input), source = str(data.source);
  const base = { owner: str(data.owner), target: source };
  const row = (item: MobilePreviewMenu, operation = item.id, value = '') => ({ ...item, operation, value });
  const items = menuRows(kind === 'browser' ? data.tabs : data.devices).map(item => row(item, 'select', item.id));
  const choices = menuRows(data.menu).map(item => row(item));
  if (kind === 'browser' && data.pipSupported === true) choices.push(row({ id: 'pip', title: data.pipActive === true ? 'Stop picture in picture' : 'Picture in picture', subtitle: '', symbol: 'pip.enter', selected: false, disabled: false, destructive: false, group: '' }));
  return { selection: JSON.stringify({ ...base, title: kind === 'browser' ? 'Browser tabs' : 'Devices', symbol: kind === 'browser' ? 'square.on.square' : 'iphone', items }),
    options: JSON.stringify({ ...base, title: kind === 'browser' ? 'Browser tab options' : 'Device options', symbol: 'ellipsis', items: kind === 'browser' ? choices : [...(items.length > 1 ? items.map(item => ({ ...item, group: 'Devices' })) : []), ...choices] }) };
}
export async function mobilePreviewMenuAction(kind: string, raw: string, owner: string, native?: Native | null) {
  let event; try { event = obj(JSON.parse(raw)); } catch { return { message: '', revision: mobileClient.revision }; }
  const snapshot = kind === 'browser' ? mobileBrowserSnapshot(owner) : mobileDevicesSnapshot(owner);
  if (!owner || event.owner !== owner || event.target !== snapshot.source) return { message: '', revision: mobileClient.revision };
  return mobilePreviewAction(kind, owner, str(event.operation), str(event.value), native);
}
