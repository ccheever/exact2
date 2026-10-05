// What the app's native module needs from the device settings beyond deviceSettings:
// the Quit shortcut mode (settings.ts QuitConfirmationMode, default "hold").
import { obj, str } from './domain';

export const QUIT_MODES = ['direct', 'hold', 'double-click'] as const;
export function quitMode(local: { clientSettings?: unknown }): string {
  const value = str(obj(local.clientSettings).confirmQuit);
  return (QUIT_MODES as readonly string[]).includes(value) ? value : 'hold';
}
