// Excord's data module (llp/0001 §4.2), answering from the stand-in world in `world.ts`.
// A real store answers on a later turn, so these do too: every answer resolves
// after the call returns. (The fixture's slow history is a delay in the view's
// request, `askAnchor`: a data module has no clock.)
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
const LIMIT = 200, HALF = 100;
const later = <T>(answer: () => T): Promise<T> => Promise.resolve().then(answer);

function ack(write: () => unknown): Promise<Result<'sendMessage'>> {
  return later(() => {
    try { write(); return { ok: true, error: '' }; }
    catch (error) { return { ok: false, error: error instanceof Error ? error.message : String(error) }; }
  });
}

// `since` is the read position the reader opened the channel at: the first row
// after it is the New divider's.
function window(channel: string, anchor: string, since: string): Result<'loadWindow'> {
  if (!channel) return { channel: '', anchor, rows: [], hasBefore: false, hasAfter: false, firstId: '', lastId: '', pin: '' };
  const page = anchor ? world.around(channel, anchor, LIMIT) : world.latest(channel, LIMIT);
  const rows = page.messages.map((m, i) => {
    const row = i ? render(m, page.messages[i - 1]) : render(m, page.previous, page.hasBefore && !page.previous);
    const previousId = i ? page.messages[i - 1].id : page.previous?.id ?? '';
    const isNew = since !== '' && m.id > since && previousId <= since;
    return isNew ? { ...row, isNew } : row;
  });
  return { channel, anchor, rows, hasBefore: page.hasBefore, hasAfter: page.hasAfter,
    firstId: rows[0]?.id ?? '', lastId: rows[rows.length - 1]?.id ?? '',
    pin: anchor || rows[Math.max(0, rows.length - HALF)]?.id || '' };
}

const sources: Sources = {
  gatewayState: () => later(() => ({ state: 'ready', me: world.me().id, pending: world.pendingSends(), channels: world.channelList() })),
  loadWindow: ([channel, anchor, , since]) => later(() => window(channel, anchor, since)),
  sendMessage: ([channel, body, replyTo, at]) => ack(() => world.send(channel, body, replyTo, at)),
  settleSends: () => ack(() => world.settle()),
  retrySend: ([channel, key]) => ack(() => world.retry(channel, key)),
  deleteMessage: ([channel, key]) => ack(() => world.remove(channel, key)),
  editMessage: ([channel, id, body]) => ack(() => world.edit(channel, id, body)),
  react: ([channel, id, emoji]) => ack(() => world.react(channel, id, emoji)),
  markRead: ([channel]) => ack(() => world.markRead(channel)),
  inject: ([channel, what, near, at]) => ack(() => world.inject(channel, what, near, at)),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
