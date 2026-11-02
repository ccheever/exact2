import type { Answer, Sources } from './app.contract.d.ts';
import { quiet } from './src/quiet.ts';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = '';

const sources: Sources = {
  quiet: () => quiet(),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
