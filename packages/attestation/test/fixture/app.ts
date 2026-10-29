import type { Answer, Sources } from './app.contract.d.ts';
import { attest } from '@exact/attestation';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'device.attestation';

const sources: Sources = {
  // What the call answered, never the token: its format, or the error's name.
  prove: async () => {
    try {
      const proof = await attest({ clientData: 'fixture-challenge', android: { cloudProjectNumber: '000000000000' } });
      return { text: `format ${proof.format}` };
    } catch (e) {
      return { text: `error ${(e as { name?: string }).name}` };
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
