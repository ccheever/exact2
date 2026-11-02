import type { Answer, Sources } from './app.contract.d.ts';
import { healthStatistics } from '@exact/health';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'device.health-read purpose.health\ndevice.motion purpose.motion';

const sources: Sources = {
  // 11 to 14 October 2026's daily steps, a day with no samples as "-".
  days: async () => {
    try {
      const s = await healthStatistics({ type: 'steps', start: 1791676800000, end: 1791936000000, interval: 'P1D' });
      return { text: s.buckets.map((b) => (b.value == null ? '-' : `${b.value}`)).join(',') };
    } catch (e) {
      return { text: `error ${(e as { code?: string }).code}` };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
