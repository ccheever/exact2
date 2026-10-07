// @ref llp/1106.000-mobile-app-layout.decision.md#shared-typescript
// GAP 001: pinned shared-source copy; see the layout decision.
import { pairingFields } from './shared/r10-connect-pairing';

export const appId = 'com.exact.t3code.ios';
export const grants = '';

export function answer(source: string, args: unknown[]) {
  if (source === 'pairingFields') return pairingFields(String(args[0] ?? ''));
  throw new Error(`Unknown mobile source: ${source}`);
}
