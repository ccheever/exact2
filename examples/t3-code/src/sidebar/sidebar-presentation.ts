// The snooze menu's options (T3 Code 6d450a4284, MIT; see LICENSE-T3): apps/web/src/components/Sidebar.snooze.ts over
// client-runtime state/threadSettled.ts (resolveSnoozePresets, pickSnoozePreset). The options are the environment's
// `snoozePresets` setting, resolved by settings-snooze.ts as Settings › Threads › Snooze options previews them; times
// use the selected timestamp format.
import { obj } from '../client/domain';
import { DEFAULT_SNOOZE_PRESETS, resolveSnoozePreset, resolveSnoozePresets, snoozePresetId, snoozePresetsOf, snoozeTimeFormat,
  type SnoozePresetChoice, type SnoozePresetSetting } from '../settings/settings-snooze';

export interface SnoozePreset { id: string; label: string; wakeLabel: string; until: string }
const menuRow = (choice: SnoozePresetChoice): SnoozePreset => ({ id: choice.id, label: choice.label, wakeLabel: choice.whenLabel, until: choice.snoozedUntil });

/** An environment's options (server settings `snoozePresets`); a server that sends none, or a list it would refuse, has the defaults. */
export function snoozePresetSettings(config: unknown): readonly SnoozePresetSetting[] {
  return snoozePresetsOf(obj(obj(config).settings));
}

/** The snooze menu: soonest first, one row per wake minute (the first in settings order keeps a tie). */
export function snoozePresets(now: number, format: string, settings: readonly SnoozePresetSetting[] = DEFAULT_SNOOZE_PRESETS): SnoozePreset[] {
  if (!Number.isFinite(now) || now <= 0) return [];
  return resolveSnoozePresets(new Date(now), settings, snoozeTimeFormat(format)).map(menuRow);
}

/**
 * A menu choice as picked at `now`, from the menu as it was shown (`shown`). A duration re-resolves from its setting
 * (not the deduped menu) so it starts at selection. A set time keeps the shown wake, which a day change would
 * otherwise shift. Undefined when that wake has passed.
 */
export function pickSnoozePreset(shown: readonly SnoozePreset[], id: string, now: number, format: string, settings: readonly SnoozePresetSetting[] = DEFAULT_SNOOZE_PRESETS): SnoozePreset | undefined {
  if (!Number.isFinite(now) || now <= 0) return undefined;
  const original = shown.find(preset => preset.id === id);
  const ahead = original && Date.parse(original.until) > now ? original : undefined;
  const setting = settings.find(candidate => candidate.kind === 'after' && snoozePresetId(candidate) === id);
  const resolved = setting && resolveSnoozePreset(setting, new Date(now), snoozeTimeFormat(format));
  return (resolved && menuRow(resolved)) ?? ahead;
}
