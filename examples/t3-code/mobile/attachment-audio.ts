// AudioFilePreview at365aa87982 over an app-owned AVPlayer.
// @ref llp/1107.005-composer-and-transcript.decision.md#media-presentation
import { mobileNative } from './client';
import { obj, str } from './shared/domain';
import { bridgeReply, type Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
export function mobileAudioStatus(raw: string, identifier: string) {
  let value; try { value = obj(JSON.parse(raw)); } catch { value = {}; }
  const current = !!identifier && value.identifier === identifier;
  const seconds = (input: unknown) => typeof input === 'number' && Number.isFinite(input) ? Math.max(0, Math.floor(input)) : 0;
  const stamp = (input: unknown) => { const n = seconds(input); return `${Math.floor(n / 60)}:${String(n % 60).padStart(2, '0')}`; };
  return { identifier, loaded: current && value.loaded === true, playing: current && value.playing === true,
    position: `${stamp(current ? value.currentTime : 0)} / ${stamp(current ? value.duration : 0)}`,
    error: current ? str(value.error) : '' };
}
export async function mobileAudioAction(identifier: string, operation: string, native?: Native | null) {
  if (!identifier || !native?.available || !['toggle', 'back', 'forward'].includes(operation)) return { revision: 0, message: '' };
  const result = await bridgeReply(letGoAware(mobileNative(native)), { op: 'mobileAudioControl', identifier, operation });
  return { revision: 0, message: result.ok ? '' : result.error?.message ?? 'Audio playback is unavailable.' };
}
