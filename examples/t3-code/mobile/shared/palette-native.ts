// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/palette-native.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// What the app's native module needs from the device settings beyond deviceSettings:
// the Quit shortcut mode (settings.ts QuitConfirmationMode, default "hold").
import { obj, str } from './domain';

export const QUIT_MODES = ['direct', 'hold', 'double-click'] as const;
export function quitMode(local: { clientSettings?: unknown }): string {
  const value = str(obj(local.clientSettings).confirmQuit);
  return (QUIT_MODES as readonly string[]).includes(value) ? value : 'hold';
}
