// The design tool's protocol client (LLP 1122 D2): `tamber/1` over HTTP to the
// app's Tamber server, which serves this page from the same local origin
// (`bun exact.mjs tamber tool`). The token comes once from `GET
// /__tamber/token`, which answers only this machine's loopback, and rides
// every POST as `x-exact-tamber-token`, never in a URL. Changes are followed
// by the long poll, `POST /__tamber/events`: a data module's `fetch` reads a
// body whole, so it cannot follow the server-sent stream. A failure is
// words in the answer, never a throw.
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.exact.design-tool';
// The tool's own local URL: `tamber tool` serves it here, and a grant names
// one origin and port.
export const API = 'http://127.0.0.1:7740';
export const grants = `net.fetch ${API}`;

const NO_SERVER = `No design server answers at ${API}. Start one for your app with \`bun exact.mjs tamber tool <app>\`, then try again.`;

type Reply = { ok?: any; error?: { message?: string } };
let token: string | null = null;

/** The token, asked once; null with the words to show when there is none. */
async function tokenOf(): Promise<{ token: string | null; error: string }> {
  if (token) return { token, error: '' };
  try {
    const res = await fetch(`${API}/__tamber/token`, { exactTimeout: 5000 });
    if (!res.ok) return { token: null, error: `${API} gave no design server token: it answers only this machine's loopback, and only while \`bun exact.mjs tamber tool\` runs.` };
    token = String(((await res.json()) as { token?: unknown }).token ?? '') || null;
    return { token, error: token ? '' : NO_SERVER };
  } catch (e) {
    if ((e as { code?: string }).code === 'bake') throw e; // no server at build
    return { token: null, error: NO_SERVER };
  }
}

/** One `tamber/1` request: its reply, or the words to show. A token the
 * server no longer knows (it restarted) is asked for again, once. */
async function call(path: string, body: unknown, timeout = 15000): Promise<{ error: string; reply: Reply }> {
  for (let attempt = 0; attempt < 2; attempt++) {
    const got = await tokenOf();
    if (!got.token) return { error: got.error, reply: {} };
    try {
      const res = await fetch(`${API}${path}`, { method: 'POST', exactTimeout: timeout, body: JSON.stringify(body),
        headers: { 'content-type': 'application/json', 'x-exact-tamber-token': got.token } });
      if (res.status === 404 && attempt === 0) { token = null; continue; }
      const reply = (await res.json().catch(() => ({}))) as Reply;
      if (res.ok) return { error: '', reply };
      return { error: reply.error?.message ?? `The design server refused the request (${res.status}).`, reply };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e;
      return { error: NO_SERVER, reply: {} };
    }
  }
  return { error: NO_SERVER, reply: {} };
}

const files = (digests: Record<string, string> = {}) => Object.entries(digests).map(([path, digest]) => ({ path, digest }));

type Change = Result<'waitChanges'>['recent'][number];
// The session's changes, newest first, as the long poll brought them.
let recent: Change[] = [];
let answers = 0;
let place = { seq: 0, epoch: '' };

/** An event from the long poll as the view lists it. */
function change(e: any): Change | null {
  const who = typeof e.client === 'string' ? e.client : 'changed on disk';
  switch (e.event) {
    case 'changed': return { seq: e.seq, kind: 'changed', who, files: e.edited ?? [], tokens: (e.changed ?? []).map((c: { token: string }) => c.token) };
    case 'committed': return { seq: e.seq, kind: 'committed', who, files: e.files ?? [], tokens: [] };
    case 'proposal': return { seq: e.seq, kind: 'proposal', who, files: [], tokens: (e.changed ?? []).map((c: { token: string }) => c.token) };
    case 'proposal-dropped': return { seq: e.seq, kind: `proposal ${e.reason ?? 'dropped'}`, who: typeof e.client === 'string' ? e.client : 'the server', files: [], tokens: [] };
    case 'down': return { seq: e.seq, kind: 'down', who: 'the server', files: [], tokens: [] };
    default: return null;
  }
}

const sources: Sources = {
  loadHello: async () => {
    const { error, reply } = await call('/__tamber', { method: 'hello' });
    if (error) return { ok: false, error, protocol: '', format: 0, app: '', files: [] };
    const ok = reply.ok ?? {};
    return { ok: true, error: '', protocol: String(ok.protocol ?? ''), format: Number(ok.format ?? 0), app: String(ok.app ?? ''), files: files(ok.files) };
  },
  waitChanges: async ([since, epoch]) => {
    // Up to 25 s with nothing new, and the server answers once there is.
    const { error, reply } = await call('/__tamber/events', epoch ? { after: since, epoch } : { after: 0, epoch: '' }, 40000);
    answers += 1;
    if (error) return { ok: false, error, answer: answers, seq: place.seq, epoch: place.epoch, missed: false, recent };
    const r = reply as { events?: unknown[]; seq?: number; epoch?: string; missed?: boolean };
    const fresh = (r.events ?? []).map(change).filter((c): c is Change => c !== null).reverse();
    recent = [...fresh, ...recent].slice(0, 50);
    place = { seq: Number(r.seq ?? 0), epoch: String(r.epoch ?? '') };
    return { ok: true, error: '', answer: answers, seq: place.seq, epoch: place.epoch, missed: r.missed === true, recent };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
