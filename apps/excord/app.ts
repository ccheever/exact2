import type { Answer, Result, Sources } from './app.contract.d.ts';
import * as world from './world';
import { createRenderer } from './render';

export const appId = 'com.exact.excord';
// Where Discord is: the fixture (`bun apps/excord/fixture/server.ts`) unless the bake says otherwise.
// `EXACT_BACKEND_ORIGIN=https://discord.com` (or `.exact/backend-origin`) replaces this literal at
// bake; only Charlie makes that build, by hand (llp/0001 §2). Every other build talks to the fixture.
export const origin = 'http://127.0.0.1:4790';
export const grants = '';

const render = createRenderer(world);
function ack(write: () => unknown): Result<'sendMessage'> {
  try { write(); return { ok: true, error: '' }; }
  catch (error) { return { ok: false, error: error instanceof Error ? error.message : String(error) }; }
}

const sources: Sources = {
  gatewayState: () => ({ state: 'ready', me: world.me().id, channels: world.channelList() }),
  loadWindow: ([channel, anchor]) => {
    if (!channel) return { channel: '', anchor, rows: [], hasBefore: false, hasAfter: false, firstId: '', lastId: '', pin: '' };
    const page = anchor ? world.around(channel, anchor, 200) : world.latest(channel, 200);
    const rows = page.messages.map((m, i) => render(m, i ? page.messages[i - 1] : world.previous(channel, m.id)));
    return { channel, anchor, rows, hasBefore: page.hasBefore, hasAfter: page.hasAfter,
      firstId: rows[0]?.id ?? '', lastId: rows[rows.length - 1]?.id ?? '',
      pin: anchor || rows[Math.max(0, rows.length - 100)]?.id || '' };
  },
  sendMessage: ([channel, body, replyTo, at]) => ack(() => world.send(channel, body, replyTo, at)),
  settleSends: ([channel]) => ack(() => world.settle(channel)),
  retrySend: ([channel, key]) => ack(() => world.retry(channel, key)),
  deleteMessage: ([channel, key]) => ack(() => world.remove(channel, key)),
  editMessage: ([channel, id, body]) => ack(() => world.edit(channel, id, body)),
  react: ([channel, id, emoji]) => ack(() => world.react(channel, id, emoji)),
  markRead: ([channel]) => ack(() => world.markRead(channel)),
  inject: ([channel, what, near, at]) => ack(() => world.inject(channel, what, near, at)),
  noteJump: () => ({ ok: true, error: '' }),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
