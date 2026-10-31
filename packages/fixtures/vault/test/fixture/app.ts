import type { Answer, Sources } from './app.contract.d.ts';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'sqlite.open app:/data/vault.sqlite';

// The app's own connection to the vault, kept across answers until `own-close`.
let held: { close(): Promise<void> } | null = null;

const sources: Sources = {
  vault: async ([op, path], _store, storage, native) => {
    try {
      if (op === 'own-open') { held = await storage.sqlite.open(path); return { text: 'own opened' }; }
      if (op === 'own-close') { await held?.close(); held = null; return { text: 'own closed' }; }
      if (!native?.available) return { text: 'no native module' };
      return { text: String(native.call({ op, path }).text) };
    } catch (e: any) {
      return { text: `refused: ${e.message}` };
    }
  },
};

export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
