import type { Answer, Sources } from './app.contract.d.ts';
import { watchPosition } from '@exact/geolocation';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'device.geolocation purpose.geolocation';

const sources: Sources = {
  spot: async ([watching]) => {
    if (!watching) return { text: 'off' };
    try {
      const p = await watchPosition({});
      return { text: `${p.coords.latitude},${p.coords.longitude}` };
    } catch (e) {
      return { text: `error ${(e as { code?: string }).code}` };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
