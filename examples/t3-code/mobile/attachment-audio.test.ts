import { expect, test } from 'bun:test';
import { mobileAudioStatus, mobileAudioAction } from './attachment-audio';
import { obj } from './shared/domain';
import type { Native } from './shared/protocol';

test('late playback status cannot enable controls for another attachment or retry', () => {
  const raw = JSON.stringify({ identifier: 'previous', loaded: true, playing: true, currentTime: 91.9, duration: 3600.8, error: 'playback' });
  expect(mobileAudioStatus(raw, 'new')).toMatchObject({ loaded: false, playing: false, position: '0:00 / 0:00', error: '' });
  expect(mobileAudioStatus(raw, 'previous')).toMatchObject({ loaded: true, playing: true, position: '1:31 / 60:00', error: 'playback' });
  expect(mobileAudioStatus('{', '')).toMatchObject({ loaded: false, playing: false });
  expect(mobileAudioStatus(JSON.stringify({ identifier: 'new', currentTime: -2, duration: null }), 'new').position).toBe('0:00 / 0:00');
});
test('playback commands keep exact presentation identity; invalid actions do not cross native seam', async () => {
  const calls: unknown[] = [];
  const native: Native = { available: true, watch() {}, async later(request) { calls.push(request); return { ok: true, generation: 0, value: {} }; } };
  await mobileAudioAction('route:retry2', 'toggle', native);
  expect(obj(calls[0])).toMatchObject({ op: 'mobileAudioControl', identifier: 'route:retry2', operation: 'toggle' });
  await mobileAudioAction('', 'back', native); await mobileAudioAction('route:retry2', 'delete', native);
  expect(calls).toHaveLength(1);
});
