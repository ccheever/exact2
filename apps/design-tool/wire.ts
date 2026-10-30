// The tool's one `tamber/1` client (LLP 1122 D2, D12): its token and every
// request. Local, the server is this machine's at the tool's own origin and
// the token is the one its loopback page gives (`/__tamber/token`), asked
// again when the server restarts. Paired with another machine's server
// (T6), the server is the door the person typed, https or a tunnel they
// trust to this machine's loopback (anything else is refused before the
// code is sent), and the token is the one pairing gave: kept in memory only,
// never in storage, a URL, a log or an error. Revoked or expired (401), it
// is dropped and nothing more is sent.

import type { StreamEvent } from './app.contract.d.ts';

type Diagnostic = { file?: string; line?: number; id?: string; message?: string } | string;
export type Reply = { ok?: any; error?: { message?: string; diagnostics?: Diagnostic[]; hint?: string } };
/** A refusal's words: its message, and each diagnostic's file, line, id
 * and explanation, which say what to correct. */
const refusal = (error: Reply['error'], status: number) => {
  const said = (error?.diagnostics ?? []).map(d => (typeof d === 'string' ? d : `${d.file ?? ''}${d.line ? `:${d.line}` : ''} ${d.id ?? ''}: ${d.message ?? ''}`.trim()));
  return [error?.message ?? `The design server refused the request (${status}).`, ...said, ...(error?.hint ? [error.hint] : [])].join('\n');
};
const RESTARTED = 'The design server restarted (or another took its port) since this session began: nothing was written, and undo and redo start again.';
export const GONE = 'This client was revoked, or its pairing expired: nothing more is sent to the design server. Pair again with a new code.';
const BEFORE = 'This was made before the tool paired with another design server, so it is not sent there: review it again.';

/** What each method needs (scripts/tamber-pairing.mjs's table, which
 * scripts/design-tool-scope.test.mjs holds this to): a paired client never
 * sends one above its scope (the server refuses it too). `local` is the
 * local token's alone. */
export const NEEDS: Record<string, string> = {
  hello: 'read', describe: 'read', resolve: 'read', explain: 'read', uses: 'read', history: 'read', export: 'read', inspect: 'read', subscribe: 'read', events: 'read',
  propose: 'propose', import: 'propose', revert: 'propose', drop: 'propose', 'edit-held': 'propose', choose: 'propose',
  transaction: 'write', undo: 'write', commit: 'write', accept: 'write', pin: 'write', preview: 'write', outline: 'write', hold: 'write', navigate: 'write', scrub: 'write', 'scrub-release': 'write', 'scrub-cancel': 'write',
  clients: 'local', revoke: 'local', 'pair-code': 'local',
  canvas: 'read',
};
const SCOPES = ['read', 'propose', 'write'];
// The long poll's reply ceiling: 16 MiB, room for a held import's diffs (a native host counts twice that running).
const LONG_POLL_MAX = 16 * 1024 * 1024;
// A batched resolve's reply (`tokens`, tamber/1.17): the server's bound on its answers (16 MiB) and its `files`, said
// rather than left to a host's default (a fetch's is 64 MiB, a stream's 1 MiB).
const BATCH_MAX = 17 * 1024 * 1024;
/** What a request needs, as the server reads it: inspect other than a
 * refresh is `write` (it moves the builds), and so is a drop, an edit or a choice of another
 * client's held proposal (`ownerOf` its proposer, null when not known); a
 * method not in the table, `local`. */
export const needOf = (path: string, body: any, ownerOf: (id: string) => string | null = () => null) => {
  if (path !== '/__tamber') return 'read';
  if (body?.method === 'inspect' && body.refresh !== true) return 'write';
  if (body?.method === 'drop' || body?.method === 'edit-held' || body?.method === 'choose') { const by = ownerOf(String(body.proposal)); return by !== null && by !== body.client ? 'write' : 'propose'; }
  return NEEDS[String(body?.method)] ?? 'local';
};
/** Why a paired client at `scope` does not send `need`'s request; "" when it may. */
export const above = (scope: string, need: string, method: string) => need === 'local' ? `${method} is the local tool's alone: this client is paired.` : SCOPES.indexOf(scope) < SCOPES.indexOf(need) ? `This client is paired at ${scope}: ${method} needs ${need}. Nothing was sent.` : '';

/** A door a pairing code and a token may go to: an https origin, or plain
 * http to this machine's loopback (a tunnel the person trusts); the origin,
 * or why not. */
export function doorOf(text: string): { origin: string; error: string } {
  let u: URL;
  try { u = new URL(text.trim()); } catch { return { origin: '', error: 'Type the design server\'s address as the other machine shows it: https://<host>:<port>.' }; }
  const loopback = /^(127\.\d{1,3}\.\d{1,3}\.\d{1,3}|localhost|\[::1\])$/.test(u.hostname);
  if (u.username || u.password) return { origin: '', error: 'The address carries a name or password: type the server\'s address alone. Nothing was sent.' };
  if (u.protocol !== 'https:' && !(u.protocol === 'http:' && loopback)) return { origin: '', error: 'Pairing goes only over an encrypted connection: an https address, or a tunnel to this machine (http://127.0.0.1:<port>). Nothing was sent.' };
  return { origin: u.origin, error: '' };
}

/** The client, local at `api` until it pairs; `ownerOf` a held proposal's proposer. */
export function wire(api: string, ownerOf: (id: string) => string | null = () => null) {
  let base = api, paired = false, gone = false, scope = 'write', pairedToken = '';
  // Each pairing starts a new session: an answer that comes from an older one changes nothing.
  let session = 0;
  let token: string | null = null;
  // What the rest of the tool holds of a token (a review's, an undo's, a held list's server run): a handle
  // for it, never the token, so no state, log or reply shown carries one.
  const tokens = new Map<string, string>(), handles = new Map<string, string>();
  const handleOf = (t: string) => { let h = handles.get(t); if (!h) { h = `run-${handles.size + 1}`; handles.set(t, h); tokens.set(h, t); } return h; };
  // One ask at a time: the first reads start together.
  let asking: Promise<{ token: string | null; error: string }> | null = null;
  const noServer = () => paired
    ? `No design server answers at ${base}. Check the other machine's dev loop or tamber serve still runs with --remote.`
    : `No design server answers at ${api}. Start one for your app with \`bun exact.mjs tamber tool <app>\`, then try again.`;

  /** The token, asked once; null with the words to show when there is none. */
  function tokenOf(): Promise<{ token: string | null; error: string }> {
    if (token) return Promise.resolve({ token, error: '' });
    // (Paired, a token gone is not asked for again: only a new code gives one.)
    if (paired) return Promise.resolve({ token: null, error: GONE });
    asking ??= askToken().finally(() => { asking = null; });
    return asking;
  }

  async function askToken(): Promise<{ token: string | null; error: string }> {
    try {
      const res = await fetch(`${api}/__tamber/token`, { exactTimeout: 5000, redirect: 'error' });
      if (!res.ok) return paired ? tokenOf() : { token: null, error: `${api} gave no design server token: it answers only this machine's loopback, and only while \`bun exact.mjs tamber tool\` runs.` };
      const given = String(((await res.json()) as { token?: unknown }).token ?? '') || null;
      // (Paired meanwhile, while it was asked or read: this machine's token is never the one used.)
      if (paired) return tokenOf();
      token = given;
      return { token, error: token ? '' : noServer() };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e; // no server at build
      return { token: null, error: noServer() };
    }
  }

  /** One `tamber/1` request: its reply (and a handle for the token it went with), or the
   * words to show. A token the server no longer knows (it restarted: a bare
   * not-found, which an operation's own refusal never is) is asked for again,
   * once, for a read. A write goes with the token of the server run it
   * belongs to (`bound`: the one its review, or its inverse, came from), and
   * never with another, so it cannot reach whatever server holds the port
   * since: that is a refusal, the server restarted. A paired client revoked
   * or expired (401) is told so, a write in flight then too, and sends
   * nothing more. `held`: the server holds the reply until something changes (the changes poll), so no
   * agent clock waits on it (`exactHeld`, LLP 1016.000 D9). */
  async function call(path: string, body: unknown, timeout = 15000, bound?: string, held = false): Promise<{ error: string; reply: Reply; used: string }> {
    for (let attempt = 0; attempt < 2; attempt++) {
      const got = bound ? { token: tokens.get(bound) ?? null, error: RESTARTED } : await tokenOf();
      if (!got.token) return { error: got.error, reply: {}, used: '' };
      const used = handleOf(got.token), at = session;
      if (paired && gone) return { error: GONE, reply: {}, used: '' };
      // Paired, a write bound to another token (this machine's server's, a review made before pairing)
      // never goes to the door: a token goes only to the server it came from.
      if (paired && got.token !== pairedToken) return { error: BEFORE, reply: {}, used: '' };
      const over = paired ? above(scope, needOf(path, body, ownerOf), String((body as { method?: unknown })?.method ?? path)) : '';
      if (over) return { error: over, reply: {}, used: '' };
      try {
        const batch = Array.isArray((body as { tokens?: unknown })?.tokens) ? { exactIndependentHttp: { maxResponseBytes: BATCH_MAX } } : {};
        const res = await fetch(`${base}${path}`, { method: 'POST', exactTimeout: timeout, exactHeld: held, redirect: 'error', body: JSON.stringify(body), ...batch,
          headers: { 'content-type': 'application/json', 'x-exact-tamber-token': got.token } });
        const reply = (await res.json().catch(() => ({}))) as Reply;
        // (A 401 to an older pairing's token ends that one, not the session since.)
        if (res.status === 401 && paired) { if (at === session && got.token === pairedToken) { gone = true; token = null; } return { error: GONE, reply: {}, used }; }
        const unknown = res.status === 404 && !reply.error;
        if (unknown && bound) { if (token === got.token) token = null; return { error: RESTARTED, reply: {}, used }; }
        if (unknown && attempt === 0 && !paired) { token = null; continue; }
        if (res.ok) return { error: '', reply, used };
        return { error: refusal(reply.error, res.status), reply, used };
      } catch (e) {
        if ((e as { code?: string }).code === 'bake') throw e;
        return { error: noServer(), reply: {}, used: '' };
      }
    }
    return { error: noServer(), reply: {}, used: '' };
  }

  /** Pairs with the server at `address` with a code another machine's tool
   * showed: from then on every request goes there with the token it gives,
   * at its scope. The code goes in the request's body to that door alone. */
  async function pairWith(address: string, code: string, name: string): Promise<{ ok: boolean; error: string; client: string; scope: string }> {
    const door = doorOf(address);
    if (door.error) return { ok: false, error: door.error, client: '', scope: '' };
    if (!code.trim()) return { ok: false, error: 'Type the code the other machine shows.', client: '', scope: '' };
    try {
      const res = await fetch(`${door.origin}/__tamber/pair`, { method: 'POST', exactTimeout: 15000, redirect: 'error', body: JSON.stringify({ code: code.trim(), ...(name ? { name } : {}) }), headers: { 'content-type': 'application/json' } });
      const reply = (await res.json().catch(() => ({}))) as Reply;
      if (res.status === 429) return { ok: false, error: 'Too many tries from here: wait a minute, then type the code again.', client: '', scope: '' };
      if (!res.ok || typeof reply.ok?.token !== 'string') return { ok: false, error: res.ok ? 'The design server gave no token.' : refusal(reply.error, res.status), client: '', scope: '' };
      base = door.origin; token = pairedToken = reply.ok.token; paired = true; gone = false; asking = null; scope = String(reply.ok.scope ?? 'read'); session += 1;
      return { ok: true, error: '', client: String(reply.ok.client ?? ''), scope: String(reply.ok.scope ?? '') };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e;
      return { ok: false, error: `No design server answers at ${door.origin}: check the address, and that its certificate was accepted in this browser (open ${door.origin}/__tamber/tool/ once).`, client: '', scope: '' };
    }
  }

  // (The server checks every scope; the tool's own reading of it only words and greys.)
  /** The changes' long poll (tamber/1.17's GET, `exactLongPoll`): its stream, made at once with the token in hand
   * (the web build takes a stream's fetch only before an answer's first await), each event mapped with that token's
   * handle; null when no token is in hand, or this client was revoked: the caller reads once instead, which asks.
   * Its replies may be as large as a read's (a held import's diffs): `LONG_POLL_MAX`, not a stream's 1 MiB. */
  function longPoll<T>(query: string, map: (event: StreamEvent, used: string) => T): Promise<T> | null {
    if (!token || (paired && (gone || token !== pairedToken))) return null;
    const used = handleOf(token), at = session, sent = token;
    // (Its end on a token the server no longer knows, it restarted, drops that token, so the next ask, a read, asks
    // again; a paired client's 401 ends it, as a request's does.)
    const ended = (event: StreamEvent) => {
      const status = (event as { status?: number }).status;
      if (event.type !== 'error' || at !== session || token !== sent) return;
      if (status === 401 && paired) { gone = true; token = null; } else if (status === 404 && !paired) token = null;
    };
    return fetch(`${base}/__tamber/events?${query}`, { exactStream: (event: StreamEvent) => (ended(event), map(event, used)), exactLongPoll: { cursor: 'after', from: 'cursor' }, exactIndependentHttp: { maxResponseBytes: LONG_POLL_MAX }, redirect: 'error', headers: { 'x-exact-tamber-token': token } });
  }

  return { call, longPoll, RESTARTED, pairWith, remote: () => (paired ? base : ''), scope: () => scope };
}
