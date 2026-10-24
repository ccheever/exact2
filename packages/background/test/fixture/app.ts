import type { Answer, Sources } from './app.contract.d.ts';
import { registerSync, registerPeriodicSync } from '@exact/background';

// The suite writes the id `exact new` gave the app, and its server's origin.
export const appId = '__APP_ID__';
const origin = '__PULL_ORIGIN__';
export const grants = `net.fetch ${origin}\ndevice.background-sync\ndevice.periodic-background-sync`;

const nameOf = (e: unknown) => (e as { name?: string; message?: string }).name ?? String(e);

const sources: Sources = {
  queueSync: async ([tag]) => {
    await registerSync({ tag: String(tag) });
    return { text: 'queued' };
  },
  // A wake's owned work: one request; a refusal registers the tag again, which waits out the backoff.
  pull: async ([tag, lastChance]) => {
    const r = await fetch(`${origin}/pull?tag=${encodeURIComponent(String(tag))}`).catch(() => null);
    const sent = r?.ok === true;
    if (!sent) await registerSync({ tag: String(tag) });
    return { ok: true, tag: String(tag), sent, lastChance: lastChance === true };
  },
  periodicSync: async () => {
    try {
      await registerPeriodicSync({ tag: 'p', minInterval: 900000 });
      return { text: 'registered' };
    } catch (e) {
      return { text: nameOf(e) };
    }
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
