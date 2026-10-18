import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.starter';
// The server's address, its one place: server.ts listens on it (a phone reaches the Mac by its LAN address or HTTPS).
export const API = 'http://127.0.0.1:8787';
export const grants = `net.fetch ${API}`;
// Who may sign in: the server's token and the name the app shows for it. A real sign-in keeps its token
// with `secret.keep token` and store.set, and asks the server for the name.
const PEOPLE = [{ token: 'ada', name: 'Ada Lovelace' }, { token: 'grace', name: 'Grace Hopper' }];

type Answered = { ok: true; body: unknown } | { ok: false; error: string };
async function call(persona: string, path: string, init: RequestInit = {}): Promise<Answered> {
  try {
    const res = await request(persona, path, init);
    const body = (await res.json().catch(() => ({}))) as { error?: unknown };
    if (res.ok) return { ok: true, body };
    return { ok: false, error: typeof body.error === 'string' ? body.error : 'The server had a problem. Try again.' };
  } catch (e) {
    if ((e as { code?: string }).code === 'bake') throw e; // no server at build
    return { ok: false, error: 'Can’t reach the server. Check that it’s running, then try again.' }; // a sheet keeps what was typed
  }
}

function request(persona: string, path: string, init: RequestInit = {}): Promise<Response> {
  return fetch(`${API}${path}`, { ...init, exactTimeout: 10000,
    headers: { authorization: `Bearer ${persona}`, 'content-type': 'application/json' } });
}

const reply = (r: Answered) => (r.ok ? { ok: true, error: '' } : { ok: false, error: r.error });
type ItemBody = { id: number; title: string; notes: string; locked: number };
function isItemBody(value: unknown): value is ItemBody {
  if (value === null || typeof value !== 'object') return false;
  const item = value as Record<string, unknown>;
  return typeof item.id === 'number' && Number.isSafeInteger(item.id) &&
    typeof item.title === 'string' && typeof item.notes === 'string' &&
    (item.locked === 0 || item.locked === 1);
}

const sources: Sources = {
  loadPeople: async () => PEOPLE,
  loadItems: async ([persona]): Promise<Result<'loadItems'>> => {
    const response = await request(persona, '/items');
    if (!response.ok) throw new Error('The server rejected the list request.');
    const body: unknown = await response.json();
    if (!Array.isArray(body) || !body.every(isItemBody)) throw new Error('The server returned an invalid item list.');
    return { owner: persona, items: body.map((i) => ({ id: String(i.id), title: i.title, notes: i.notes, locked: i.locked === 1 })) };
  },
  addItem: async ([persona, title, notes]) => reply(await call(persona, '/items', { method: 'POST', body: JSON.stringify({ title, notes }) })),
  lockItem: async ([persona, id, locked]) => reply(await call(persona, `/items/${encodeURIComponent(id)}`, { method: 'PATCH', body: JSON.stringify({ locked }) })),
  removeItem: async ([persona, id]) => reply(await call(persona, `/items/${encodeURIComponent(id)}`, { method: 'DELETE' })),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
