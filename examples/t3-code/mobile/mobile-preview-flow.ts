// Root preparation and menu projection for the pinned mobile browser/device routes.
// @ref llp/1109-t3-code-ios.rfc.md#architecture
import { mobileClient, mobileNative } from './client';
import { mobileStreamOwner } from './browser-mobile-owner';
import { mobileBrowserRead, mobileBrowserSnapshot, mobileBrowserStatus, mobileBrowserAction, mobileBrowserRelease, type MobilePreviewMenu, type MobileBrowserSnapshot } from './browser-mobile-data';
import { mobileDevicesRead, mobileDevicesSnapshot, mobileDevicesStatus, mobileDevicesAction, type MobileDevicesSnapshot } from './devices-mobile-data';
import { type Native } from './shared/protocol';
import { arr, obj, str } from './shared/domain';
let observedOwner = '';
export function mobilePreviewOwner(active: boolean) { return { owner: active ? mobileStreamOwner(mobileClient) : '' }; }
export async function mobilePreviewPrepare(owner: string, native?: Native | null) {
  if (!native?.available) return { revision: mobileClient.revision };
  const handle = mobileNative(native), previous = observedOwner; observedOwner = owner;
  if (previous && previous !== owner) await mobileBrowserRelease(previous, handle);
  if (owner) await Promise.all([mobileBrowserRead(owner, handle), mobileDevicesRead(owner, handle)]);
  return { revision: mobileClient.revision };
}
export function mobileBrowserPreviewStatus(owner: string, active: boolean, native?: Native | null): MobileBrowserSnapshot | Promise<MobileBrowserSnapshot> {
  return active && owner && native?.available ? mobileBrowserStatus(owner, mobileNative(native)) : mobileBrowserSnapshot(owner);
}
export function mobileDevicesPreviewStatus(owner: string, active: boolean, native?: Native | null): MobileDevicesSnapshot | Promise<MobileDevicesSnapshot> {
  return active && owner && native?.available ? mobileDevicesStatus(owner, mobileNative(native)) : mobileDevicesSnapshot(owner);
}
export async function mobilePreviewAction(kind: string, owner: string, operation: string, value: string, native?: Native | null) {
  if (!owner || !native?.available) return { message: 'This preview requires a connected native app.', revision: mobileClient.revision };
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
    options: JSON.stringify({ ...base, title: kind === 'browser' ? 'Browser tab options' : 'Device options', symbol: 'ellipsis', items: kind === 'browser' ? choices : [...items.map(item => ({ ...item, group: 'Devices' })), ...choices] }) };
}
export async function mobilePreviewMenuAction(kind: string, raw: string, owner: string, native?: Native | null) {
  let event; try { event = obj(JSON.parse(raw)); } catch { return { message: '', revision: mobileClient.revision }; }
  const snapshot = kind === 'browser' ? mobileBrowserSnapshot(owner) : mobileDevicesSnapshot(owner);
  if (!owner || event.owner !== owner || event.target !== snapshot.source) return { message: '', revision: mobileClient.revision };
  return mobilePreviewAction(kind, owner, str(event.operation), str(event.value), native);
}
