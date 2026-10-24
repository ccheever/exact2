import type { Answer, Sources } from './app.contract.d.ts';

// The suite writes the id `exact new` gave the app, and its server's origin.
export const appId = '__APP_ID__';
const origin = '__PULL_ORIGIN__';
export const grants = `net.fetch ${origin}`;

// A wake's owned work: one request, which the suite's server answers at
// once or never.
const sources: Sources = {
  pull: async ([tag]) => {
    const r = await fetch(`${origin}/pull?tag=${encodeURIComponent(String(tag))}`);
    return { n: Number(await r.text()) };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
