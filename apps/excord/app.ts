// Excord's data module (llp/0001 §4). Stage 1 answers from an in-memory
// channel; the Discord client in `discord/` and the fixture server replace it.
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.exact.excord';
export const grants = '';

type Message = Result<'loadChannel'>[number];
const channels = new Map<string, Message[]>([['general', Array.from({ length: 200 }, (_, i) => ({
  id: String(i + 1), author: i % 3 === 0 ? 'ada' : 'grace', body: `Message ${i + 1}`, pending: false,
}))]]);

const sources: Sources = {
  loadChannel: ([channel]) => channels.get(channel) ?? [],
  sendMessage: ([channel, body]) => {
    const list = channels.get(channel) ?? [];
    list.push({ id: String(list.length + 1), author: 'me', body, pending: false });
    channels.set(channel, list);
    return { error: '' };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
