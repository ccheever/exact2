import type { Answer, Sources } from './app.contract.d.ts';
import { pedometer } from '@exact/pedometer';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'device.motion purpose.motion';

const sources: Sources = {
  // A data-code reader of the count since its first ask.
  counted: async () => {
    try {
      const p = await pedometer({});
      return { text: `${p.steps}` };
    } catch (e) {
      return { text: `error ${(e as { code?: string }).code}` };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
