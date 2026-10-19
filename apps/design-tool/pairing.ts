// Pairing in the tool (LLP 1122 D12, T6; LLP 1121 stage 10 A2, tamber/1.13):
// the local tool mints a code for a client on another machine, at a scope,
// and shows it with each remote door's certificate fingerprint, which the
// other machine checks before it trusts the door; it lists the paired
// clients and revokes one. A code is shown, never logged, put in a URL or
// kept: it is good once, for two minutes. These are the local token's alone.
import type { Sources } from './app.contract.d.ts';

type Call = (path: string, body: unknown, timeout?: number, bound?: string) => Promise<{ error: string; reply: { ok?: any }; used: string }>;

/** A time the server gave (ISO 8601) as the clock on the wall shows it; "" for none. */
const clock = (at: unknown) => {
  const t = typeof at === 'string' ? Date.parse(at) : NaN;
  return Number.isFinite(t) ? new Date(t).toLocaleTimeString() : '';
};

/** The pairing sources, over the tool's one `tamber/1` client. */
export function pairingSources(call: Call): Pick<Sources, 'mintCode' | 'loadClients' | 'revokeClient'> {
  return {
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
      const { error, reply } = await call('/__tamber', { method: 'clients' });
      if (error) return { ok: false, error, clients: [] };
      const list = Array.isArray(reply.ok?.clients) ? reply.ok.clients : [];
      return { ok: true, error: '', clients: list.filter((c: any) => !c?.local).map((c: any) => ({ client: String(c.client ?? ''), name: String(c.name ?? ''), scope: String(c.scope ?? ''), peer: String(c.peer ?? ''), seen: clock(c.lastSeen), until: clock(c.expires) })) };
    },
    revokeClient: async ([client]) => {
      const { error } = await call('/__tamber', { method: 'revoke', client });
      return error ? { ok: false, error, note: '' } : { ok: true, error: '', note: `${client} was revoked: its token is refused from now on, and its stream and long poll end.` };
    },
  };
}
