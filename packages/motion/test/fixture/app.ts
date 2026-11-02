import type { Answer, Sources } from './app.contract.d.ts';
import { accelerometer } from '@exact/motion';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'device.motion purpose.motion';

const sources: Sources = {
  // A data-code reader of a 64-sample batch: the samples since its last answer.
  counted: async () => {
    try {
      const a = await accelerometer({ frequency: 60, interval: 100, history: 64 });
      return { text: `${a.samples.length} ${a.dropped}` };
    } catch (e) {
      return { text: `error ${(e as { code?: string }).code}` };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
