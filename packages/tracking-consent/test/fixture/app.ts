import type { Answer, Sources } from './app.contract.d.ts';
import { advertisingIdentifier } from '@exact/tracking-consent';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'device.tracking purpose.ads';

const ZEROS = '00000000-0000-0000-0000-000000000000';

const sources: Sources = {
  // What the identifier is, never the identifier itself: "zeros", "set", or the error.
  identifier: async () => {
    try {
      const { identifier } = await advertisingIdentifier();
      return { text: identifier === ZEROS ? 'zeros' : 'set' };
    } catch (e) {
      return { text: `error ${(e as { name?: string }).name}` };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
