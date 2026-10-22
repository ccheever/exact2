// The Quit shortcut mode (settings.ts QuitConfirmationMode, default "hold"), which the window's ⌘Q hold
// reads from the settingsCore resource (quit-hold.contract).
import { obj, str } from '../client/domain';

export const QUIT_MODES = ['direct', 'hold', 'double-click'] as const;
export function quitMode(local: { clientSettings?: unknown }): string {
  const value = str(obj(local.clientSettings).confirmQuit);
  return (QUIT_MODES as readonly string[]).includes(value) ? value : 'hold';
}
