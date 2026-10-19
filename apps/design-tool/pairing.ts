// Pairing in the tool (LLP 1122 D12, T6; LLP 1121 stage 10 A2, tamber/1.13):
// the local tool mints a code for a client on another machine, at a scope,
// and shows it with each remote door's certificate fingerprint, which the
// other machine checks before it trusts the door; it lists the paired
// clients and revokes one (the local token's alone). On the other machine
// the tool pairs with that server (remote mode) by its address and the code.
// A code is shown or typed, never logged, put in a URL or kept: it is good
// once, for two minutes.
import type { Sources } from './app.contract.d.ts';

type Call = (path: string, body: unknown, timeout?: number, bound?: string) => Promise<{ error: string; reply: { ok?: any }; used: string }>;

/** A time the server gave (ISO 8601) as the clock on the wall shows it; "" for none. */
const clock = (at: unknown) => {
  const t = typeof at === 'string' ? Date.parse(at) : NaN;
  return Number.isFinite(t) ? new Date(t).toLocaleTimeString() : '';
};

type PairWith = (address: string, code: string, name: string) => Promise<{ ok: boolean; error: string; client: string; scope: string }>;

// The server run (its token) the clients shown were listed by.
let listedBy = '';

/** The frame URL for a gateway's grant (stage 10 C), or why not: only
 * through B's TLS door on another machine (a tunnel has no canvas in stage
 * 10, and a loopback host, over https or not, is this machine's, where any
 * port may be another service), the canvas on that door's host over https,
 * another port than the door (another origin, so a framed page cannot
 * reach the tool, which the door serves), at `/canvas/<client>/` and
 * nothing more, the grant as the gateway mints it (43 base64url
 * characters); the frame is built from those parts, the grant only in its
 * fragment. Never another host the server might name. */
export function canvasFrame(url: string, grant: string, door: string, client: string): { frame: string; root: string; error: string } {
  let u: URL, d: URL;
  try { u = new URL(url); d = new URL(door); } catch { return { frame: '', root: '', error: 'the design server named no gateway this tool can frame' }; }
  const loopback = /^(127\.\d{1,3}\.\d{1,3}\.\d{1,3}|localhost|\[::1\])$/;
  if (d.protocol !== 'https:' || u.protocol !== 'https:' || loopback.test(u.hostname) || u.hostname !== d.hostname || u.port === d.port || u.username || u.password || u.search || u.hash || u.pathname !== `/canvas/${encodeURIComponent(client)}/` || !/^[A-Za-z0-9_-]{43}$/.test(grant))
    return { frame: '', root: '', error: 'the design server named no gateway this tool can frame (its canvas is https, on its TLS door\'s host, another machine, and another port)' };
  return { frame: `${u.origin}/canvas/${encodeURIComponent(client)}/#g=${grant}`, root: `${u.origin}/`, error: '' };
}

// The canvas the tool last minted (its key: client, server run, tick; its
// frame URL, grant and all, until it has entered), the newest mint asked,
// and the key whose entry page has said it entered (C2: `{type:
// 'exact-canvas-entered'}`, posted to this page's origin, from that canvas's
// origin and from the frame the tool shows alone); once it has, the grant is
// dropped from the frame's URL, the tool's state and this memory.
let minted = { key: '', origin: '', frame: '', root: '' }, entered = '';
// The mint in flight (one a key, shared), and the newest ask's number.
let minting: { key: string; answer: Promise<{ frame: string; root: string; error: string }> } | null = null, asks = 0;
type Peer = { parent?: unknown; top?: unknown };
type Message = { origin?: string; source?: Peer | null; data?: { type?: unknown } };
const page = globalThis as { addEventListener?: (type: string, listener: (e: Message) => void) => void };
/** The page's `message` listener (exported for its test). */
export function onCanvasWord(e: Message) {
  if (e?.data?.type !== 'exact-canvas-entered' || !minted.frame || e.origin !== minted.origin) return;
  // (From a frame of this page, the canvas being its one: never a window the canvas opened, which is
  // its own top, nor a frame inside the canvas, whose parent is the canvas. A cross-origin window's
  // parent and top can be read.)
  const from = e.source;
  if (!from || from === from.top || from.parent !== from.top) return;
  entered = minted.key;
  minted = { ...minted, frame: '' };
}
page.addEventListener?.('message', onCanvasWord);

const canvasKey = (client: string, epoch: string, tick: number) => JSON.stringify([client, epoch, tick]);

/** The pairing sources, over the tool's one `tamber/1` client; `door` the paired server's origin ("" while local). */
export function pairingSources(call: Call, pairWith: PairWith, door: () => string): Pick<Sources, 'mintCode' | 'loadClients' | 'revokeClient' | 'pairRemote' | 'loadCanvas' | 'canvasEntered'> {
  return {
    // A paired tool's canvas (stage 10 C): the app through the server's
    // gateway, on its own origin, entered with a single-use grant (30 s)
    // in the frame URL's fragment, which no server, log or Referer sees and
    // the gateway's entry page (C2) strips before any request (theme2's
    // 09:09Z ruling). Asked once a server run, and again when the canvas
    // is opened again (`tick`: a spent grant never enters twice).
    // One grant a key (client, server run, tick): asked again (its page's
    // word `done` moving), the same canvas comes back, as its gateway page
    // once it has entered, the grant gone: no new grant, one load of that page.
    loadCanvas: async ([local, client, epoch, tick]) => {
      if (local || !client || !epoch || !door()) return { ok: true, error: '', frame: '', root: '' };
      const key = canvasKey(client, epoch, tick);
      if (minted.key === key) return { ok: true, error: '', frame: entered === key ? minted.root : minted.frame, root: minted.root };
      // One mint a key: a second ask for it (its `done` moving meanwhile) waits on the first.
      const mine = ++asks;
      if (minting?.key !== key) {
        const answer = (async () => {
          const { error, reply } = await call('/__tamber', { method: 'canvas' });
          if (error) return { frame: '', root: '', error: `No canvas: ${error}` };
          const made = canvasFrame(String(reply.ok?.url ?? ''), String(reply.ok?.grant ?? ''), door(), client);
          return made.error ? { frame: '', root: '', error: `No canvas: ${made.error}.` } : made;
        })();
        minting = { key, answer };
      }
      const made = await minting.answer;
      if (minting?.key === key) minting = null;
      if (made.error) return { ok: false, error: made.error, frame: '', root: '' };
      // (A mint a newer ask has overtaken is not the one shown: kept nowhere, and answered without its grant.)
      if (mine !== asks && minted.key !== key) return { ok: false, error: '', frame: '', root: '' };
      if (minted.key !== key) minted = { key, origin: new URL(made.root).origin, frame: made.frame, root: made.root };
      return { ok: true, error: '', frame: entered === key ? minted.root : minted.frame, root: minted.root };
    },
    // Whether the canvas of this key has said it entered (asked each second
    // while the frame's URL holds its grant).
    canvasEntered: async ([client, epoch, tick]) => ({ entered: entered === canvasKey(client, epoch, tick) }),
    // This tool paired with another machine's server (remote mode): the
    // address it typed and the code that machine showed, its name the
    // person's; the code is sent once, to that door, and kept nowhere.
    pairRemote: async ([address, code, name]) => {
      const r = await pairWith(address, code, name);
      return { ok: r.ok, error: r.error, note: r.ok ? `Paired as ${r.client}, at ${r.scope}.` : '' };
    },
    mintCode: async ([scope]) => {
      const { error, reply } = await call('/__tamber', { method: 'pair-code', scope });
      if (error) return { ok: false, error, code: '', scope: '', until: '', doors: [] };
      const ok = reply.ok ?? {};
      // Each remote door open (stage 10 B): its origin, and the fingerprint
      // of its certificate, made for this run; a trusted tunnel has none.
      const doors = (Array.isArray(ok.remote) ? ok.remote : []).map((d: any) => ({ origin: String(d?.origin ?? ''), fingerprint: String(d?.fingerprint ?? ''), tunnel: d?.tunnel === true }));
      return { ok: true, error: '', code: String(ok.code ?? ''), scope: String(ok.scope ?? scope), until: clock(ok.expires), doors };
    },
    // The paired clients (the local one, this tool's own, left out), each
    // with its scope, where it pairs from and when it was last seen.
    loadClients: async ([local]) => {
      if (!local) return { ok: true, error: '', clients: [] };
      const { error, reply, used } = await call('/__tamber', { method: 'clients' });
      if (error) return { ok: false, error, clients: [] };
      listedBy = used;
      const list = Array.isArray(reply.ok?.clients) ? reply.ok.clients : [];
      return { ok: true, error: '', clients: list.filter((c: any) => !c?.local).map((c: any) => ({ client: String(c.client ?? ''), name: String(c.name ?? ''), scope: String(c.scope ?? ''), peer: String(c.peer ?? ''), seen: clock(c.lastSeen), until: clock(c.expires) })) };
    },
    // A revoke goes to the server run whose list it was pressed in, never a
    // later one's (a restart names its clients from #1 again).
    revokeClient: async ([client]) => {
      const { error } = await call('/__tamber', { method: 'revoke', client }, 15000, listedBy || undefined);
      return error ? { ok: false, error, note: '' } : { ok: true, error: '', note: `${client} was revoked: its token is refused from now on, and its stream and long poll end.` };
    },
  };
}
