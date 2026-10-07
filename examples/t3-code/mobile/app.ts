// @ref llp/1106.003-pairing-and-transport.decision.md#decision
import { mobileSnapshot, mobileCommand, mobilePairingFields } from './client';
import { mobileEnvironmentDetail, mobileEnvironmentDetailCommand } from './environment-detail';
import { obj } from './shared/domain';
import { mobileShelves, mobileToggleShelf, mobileHomeView } from './home-state';
import { mobileTheme, mobileHomeColors } from './design';
import { connectionView } from './presentation';
import { bridgeReply, type Files, type Native } from './shared/protocol';

export const appId = 'com.exact.t3code.ios';
export const grants = 'device.camera purpose.camera';

export function answer(source: string, args: unknown[], _store?: unknown, storage?: Files, native?: Native | null) {
  if (source === 'pairingFields') return mobilePairingFields(String(args[0] ?? ''));
  if (source === 'theme') return mobileTheme(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  if (source === 'environmentDetail') return mobileEnvironmentDetail(String(args[0] ?? ''), native, storage);
  if (source === 'environmentDetailCommand') return mobileEnvironmentDetailCommand(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), native, storage);
  if (source === 'homeColors') return mobileHomeColors(String(args[0] ?? 'light'));
  if (source === 'homePreferences') return mobileShelves(native);
  if (source === 'toggleShelf') return mobileToggleShelf(String(args[0] ?? ''), native);
  if (source === 'homeView') return mobileHomeView(args);
  if (source === 'snapshot') return mobileSnapshot(native, storage!).then(snapshot => connectionView(snapshot, String(args[0] ?? 'light')));
  if (source === 'cameraPermission') {
    if (!native?.available) return { status: 'unavailable' };
    return bridgeReply(native, { op: 'mobileCameraPermission' }).then(reply =>
      ({ status: reply.ok ? String(obj(reply.value).status ?? 'unavailable') : 'error:Camera access could not be checked.' }));
  }
  if (source === 'alert') {
    if (!native?.available) return { choice: 'cancel' };
    return bridgeReply(native, { op: 'mobileAlert', kind: String(args[0] ?? 'info'),
      title: String(args[1] ?? ''), message: String(args[2] ?? '') }).then(reply =>
      ({ choice: reply.ok ? String(obj(reply.value).choice ?? 'cancel') : 'cancel' }));
  }
  if (source === 'command') {
    const operation = String(args[0] ?? ''), key = String(args[1] ?? '');
    const separator = key.indexOf('\n');
    const origin = separator < 0 ? key : key.slice(0, separator);
    const environmentId = separator < 0 ? '' : key.slice(separator + 1);
    if (operation === 'forget') return mobileCommand(['forget', origin, environmentId, 0], native, storage!);
    if (operation === 'save-environment') return mobileCommand([operation, environmentId,
      JSON.stringify({ label: String(args[2] ?? ''), url: String(args[3] ?? '') }), 0], native, storage!);
    return mobileCommand(args, native, storage!);
  }
  throw new Error(`Unknown mobile source: ${source}`);
}
