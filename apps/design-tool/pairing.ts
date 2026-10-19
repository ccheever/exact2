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

/** The frame URL for a gateway's grant (stage 10 C), or why not: the canvas
 * port is the paired door's host over https, another port than the door
 * (another origin, so a framed page cannot reach the tool), at
 * `/canvas/<client>/` and nothing more; the frame is built from those
 * parts, the grant only in its fragment. Never this machine's own pages,
 * nor any other host the server might name. */
export function canvasFrame(url: string, grant: string, door: string, client: string): { frame: string; error: string } {
  let u: URL, d: URL;
  try { u = new URL(url); d = new URL(door); } catch { return { frame: '', error: 'the design server named no gateway this tool can frame' }; }
  if (u.protocol !== 'https:' || u.hostname !== d.hostname || u.port === d.port || u.username || u.password || u.search || u.hash || u.pathname !== `/canvas/${encodeURIComponent(client)}/` || !/^[\w-]{16,}$/.test(grant))
    return { frame: '', error: 'the design server named no gateway this tool can frame (its canvas is https, on its own host and another port)' };
  return { frame: `${u.origin}/canvas/${encodeURIComponent(client)}/#g=${grant}`, error: '' };
}

/** The pairing sources, over the tool's one `tamber/1` client; `door` the paired server's origin ("" while local). */
export function pairingSources(call: Call, pairWith: PairWith, door: () => string): Pick<Sources, 'mintCode' | 'loadClients' | 'revokeClient' | 'pairRemote' | 'loadCanvas'> {
  return {
    // A paired tool's canvas (stage 10 C): the app through the server's
    // gateway, on its own origin, entered with a single-use grant (30 s)
    // in the frame URL's fragment, which no server, log or Referer sees and
    // the gateway's entry page (C2) strips before any request (theme2's
    // 09:09Z ruling). Asked once a server run, and again when the canvas
    // is opened again (`tick`: a spent grant never enters twice).
    loadCanvas: async ([local, client, epoch]) => {
      if (local || !client || !epoch || !door()) return { ok: true, error: '', frame: '' };
      const { error, reply } = await call('/__tamber', { method: 'canvas' });
      if (error) return { ok: false, error: `No canvas: ${error}`, frame: '' };
      const made = canvasFrame(String(reply.ok?.url ?? ''), String(reply.ok?.grant ?? ''), door(), client);
      return made.error ? { ok: false, error: `No canvas: ${made.error}.`, frame: '' } : { ok: true, error: '', frame: made.frame };
    },
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
