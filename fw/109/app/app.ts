import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.x14-native-reply-reask';
export const grants = '';

let asks = 0;
const sources: Sources = {
  snapshot: async (_args, _store, _storage, native): Promise<Result<'snapshot'>> => {
    if (!native || !native.available) return { text: 'placeholder', n: 0 };
    asks += 1;
    const ask = asks;
    native.watch('x');
    const r = (await native.later({ op: 'slow', ask })) as { ask: number };
    return { text: `reply to ask ${r.ask} (asks so far: ${asks})`, n: r.ask };
  },
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
