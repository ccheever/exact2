import type { Answer, Sources } from './app.contract.d.ts';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = '';

const sources: Sources = {};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
