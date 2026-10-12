import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.starter';
// The server's address, its one place: server.ts listens on it (a phone reaches the Mac by its LAN address or HTTPS).
export const API = 'http://127.0.0.1:8787';
export const grants = `net.fetch ${API}`;
// The persona's name is its token; a real sign-in keeps its token with `secret.keep token` and store.set.

type Answered = { ok: true; body: unknown } | { ok: false; error: string };
async function call(persona: string, path: string, init: RequestInit = {}): Promise<Answered> {
  try {
    const res = await fetch(`${API}${path}`, { ...init, exactTimeout: 10000,
      headers: { authorization: `Bearer ${persona.toLowerCase()}`, 'content-type': 'application/json' } });
    const body = (await res.json().catch(() => ({}))) as { error?: unknown };
    if (res.ok) return { ok: true, body };
    return { ok: false, error: typeof body.error === 'string' ? body.error : 'The server had a problem. Try again.' };
  } catch (e) {
    if ((e as { code?: string }).code === 'bake') throw e; // no server at build
    return { ok: false, error: 'Can’t reach the server. Check that it’s running, then try again.' }; // a sheet keeps what was typed
  }
}

let last: Result<'loadItems'>['items'] = []; // a failed poll keeps the list on screen
const reply = (r: Answered) => (r.ok ? { ok: true, error: '' } : { ok: false, error: r.error });
const sources: Sources = {
  loadItems: async ([persona]) => {
    const r = await call(persona, '/items');
    if (!r.ok) return { ok: false, error: r.error, items: last };
    last = (r.body as Array<{ id: number; title: string; notes: string; locked: number }>).map((i) => ({ id: String(i.id), title: i.title, notes: i.notes, locked: i.locked === 1 }));
    return { ok: true, error: '', items: last };
  },
  addItem: async ([persona, title, notes]) => reply(await call(persona, '/items', { method: 'POST', body: JSON.stringify({ title, notes }) })),
  lockItem: async ([persona, id, locked]) => reply(await call(persona, `/items/${encodeURIComponent(id)}`, { method: 'PATCH', body: JSON.stringify({ locked }) })),
  removeItem: async ([persona, id]) => reply(await call(persona, `/items/${encodeURIComponent(id)}`, { method: 'DELETE' })),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
