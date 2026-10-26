import type { Answer, Sources } from './app.contract.d.ts';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
// Gamepad asks no grant (LLP 1133.022 D9).
export const grants = '';

const sources: Sources = {};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
