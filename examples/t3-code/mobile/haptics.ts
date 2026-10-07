// Mobile adaptation of ThreadDetailScreen.tsx useStreamingHaptics and
// threadActivity.ts buildThreadFeed at T3 Code 365aa87982a4d81cc8e0c085e8d1a40ca7daecdc.
// T3 Code is MIT licensed; see assets/LICENSE-T3.
// @ref llp/1077-css-visual-properties-native-draws-cheaply.rfc.md#4-deferred — D14 separates press and event haptics.
import { arr, obj } from './shared/domain';

export interface MobileStreamingAssistant { readonly id: string; readonly textLength: number }
export interface MobileStreamingHapticState {
  readonly owner: string;
  readonly latest: MobileStreamingAssistant | null;
  readonly lastAt: number;
}
export interface MobileStreamingHapticResult {
  readonly state: MobileStreamingHapticState;
  readonly haptic: '' | 'selection';
}

/** Read the shared projection, before rendered rows discard empty assistant text.
 * buildThreadFeed preserves message order/id/text and groups only activities. */
export function mobileStreamingAssistant(visibleTurnItems: unknown): MobileStreamingAssistant | null {
  const rows = arr(visibleTurnItems);
  for (let index = rows.length - 1; index >= 0; index--) {
    const item = obj(rows[index]?.item);
    if (item.type === 'assistant_message' && item.streaming === true &&
        typeof item.messageId === 'string' && typeof item.text === 'string') {
      return { id: item.messageId, textLength: item.text.length };
    }
  }
  return null;
}

/** One state per mounted thread screen. Pass null on mount, retain across feed
 * updates, and discard on unmount. owner is the environment/thread identity.
 * The caller supplies wall-clock milliseconds and dispatches haptic(result.haptic)
 * only when nonempty. A clock tick alone never produces delayed feedback. */
export function mobileStreamingHaptic(
  previous: MobileStreamingHapticState | null,
  owner: string,
  latest: MobileStreamingAssistant | null,
  now: number,
): MobileStreamingHapticResult {
  const state = { owner, latest, lastAt: previous?.lastAt ?? 0 };
  if (!previous || previous.owner !== owner || !latest) return { state, haptic: '' };
  const isNewStream = previous.latest?.id !== latest.id;
  const textGrew = previous.latest?.id === latest.id && latest.textLength > previous.latest.textLength;
  if (!isNewStream && (!textGrew || now - state.lastAt < 320)) return { state, haptic: '' };
  return { state: { ...state, lastAt: now }, haptic: 'selection' };
}
