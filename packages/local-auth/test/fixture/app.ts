import type { Answer, Sources } from './app.contract.d.ts';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = ['secret.keep token user-presence optional', 'secret.keep key user-presence', 'device.local-auth purpose.lock'].join('\n');

// What a call answered, or the error's name and code: never a secret's value beyond the fixture's own.
const failed = (e: unknown) => ({ text: `${(e as Error).name} ${(e as { code?: string }).code ?? ''}`.trim() });

const sources: Sources = {
  facts: (_args, store) => ({ text: [store.has('token'), store.locked('token'), store.has('key'), store.locked('key')].join(' ') }),
  storeOp: async ([op], store) => {
    try {
      if (op === 'set-token') { store.set('token', 'open'); return { text: 'kept' }; }
      if (op === 'set-key') { store.set('key', 'locked-value'); return { text: 'kept' }; }
      if (op === 'protect') { await store.protect('token'); return { text: 'protected' }; }
      if (op === 'unprotect') { await store.unprotect('token', { reason: 'reason.unlock' }); return { text: 'unprotected' }; }
      if (op === 'unlock') return { text: `unlocked ${await store.unlock('token', { reason: 'reason.unlock' })}` };
      if (op === 'read-key') return { text: `read ${store.get('key')}` };
      return { text: `no op ${op}` };
    } catch (e) {
      return failed(e);
    }
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
