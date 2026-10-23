// Excord's data module (llp/0001 §4.2), answering from the stand-in world in `world.ts`.
// A real store answers on a later turn, so these do too: every answer resolves
// after the call returns. (The fixture's slow history is a delay in the view's
// request, `askAnchor`: a data module has no clock.)
import type { Answer, Result, Sources } from './app.contract.d.ts';
import * as world from './world';
import { createRenderer, windowOf } from './render';

export const appId = 'com.exact.excord';
// Where Discord is: the fixture (`bun apps/excord/fixture/server.ts`) unless the bake says otherwise.
// `EXACT_BACKEND_ORIGIN=https://discord.com` (or `.exact/backend-origin`) replaces this literal at
// bake; only Charlie makes that build, by hand (llp/0001 §2). Every other build talks to the fixture.
export const origin = 'http://127.0.0.1:4790';
export const grants = '';

const render = createRenderer(world);
const LIMIT = 200;
const later = <T>(answer: () => T): Promise<T> => Promise.resolve().then(answer);

function ack(write: () => unknown): Promise<Result<'sendMessage'>> {
  return later(() => {
    try { write(); return { ok: true, error: '' }; }
    catch (error) { return { ok: false, error: error instanceof Error ? error.message : String(error) }; }
  });
}

function window(channel: string, anchor: string, since: string): Result<'loadWindow'> {
  if (!channel) return { channel: '', anchor, rows: [], hasBefore: false, hasAfter: false, firstId: '', lastId: '', pin: '' };
  return windowOf(anchor ? world.around(channel, anchor, LIMIT) : world.latest(channel, LIMIT), channel, anchor, since, render);
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
