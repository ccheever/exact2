import type { Answer, Sources, Result } from './app.contract.d.ts';

export const appId = 'com.exact.greybox';
export const grants = '';

function hud(message: string): Result<'hud'> {
  if (message === '') return { beacons: 0 };
  const value: unknown = JSON.parse(message);
  if (!value || typeof value !== 'object' || Array.isArray(value)
      || !('beacons' in value) || typeof value.beacons !== 'number'
      || !Number.isInteger(value.beacons) || value.beacons < 0 || value.beacons > 1) {
    throw new Error('Expected a grey box HUD message: {beacons: 0 | 1}');
  }
  return { beacons: value.beacons };
}

const sources: Sources = { hud: ([message]) => hud(message) };
export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
