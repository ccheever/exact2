// A request-answering data source in TypeScript — Weird Castle's login in
// miniature, the executor's fixture for LLP 1027 D1a: `fetch` is the web's
// and goes through the host's ticket path; the store is the seam's; an
// answer may await two fetches in a row; a refusal is a thrown `DataError`.

type Session = { ok: boolean; username: string; error: string };

interface Store {
  get(name: string): string | null;
  set(name: string, value: string): void;
  forget(name: string): void;
}

class DataError extends Error {
  constructor(public kind: "UnknownSource" | "BadArguments" | "Unavailable", message: string) {
    super(message);
  }
}

const SECRET = "castle.session";
const API = "https://api.castle.xyz/graphql";
const LOGIN = "mutation Login($who: String!, $password: String!) { loginV2(who: $who, password: $password) { token username } }";

const idle: Session = { ok: false, username: "", error: "" };

function text(args: unknown[], i: number): string {
  const v = args[i];
  if (typeof v === "string") return v;
  throw new DataError("BadArguments", `argument ${i}`);
}

function remember(store: Store): Session {
  const kept = store.get(SECRET);
  if (kept === null) return idle;
  const k = JSON.parse(kept) as { token: string; username: string };
  return { ok: true, username: k.username, error: "" };
}

async function login(who: string, password: string, store: Store): Promise<Session> {
  who = who.trim();
  if (who === "" || password === "") return { ...idle, error: "Enter a username and a password" };
  let r: Response;
  try {
    r = await fetch(API, {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json" },
      body: JSON.stringify({ query: LOGIN, variables: { who, password } }),
    });
  } catch (e: any) {
    const error =
      e.kind === "Refused" ? "Castle is not a host this app may reach"
      : e.kind === "Unsupported" ? "This host cannot reach Castle yet"
      : `Couldn't reach Castle (${e.message})`;
    return { ...idle, error };
  }
  let j: any;
  try { j = await r.json(); } catch { return { ...idle, error: `Castle answered HTTP ${r.status} without JSON` }; }
  const user = j?.data?.loginV2;
  if (user && typeof user.token === "string" && user.token !== "") {
    store.set(SECRET, JSON.stringify({ token: user.token, username: String(user.username ?? "") }));
    return { ok: true, username: String(user.username ?? ""), error: "" };
  }
  return { ...idle, error: String(j?.errors?.[0]?.message ?? `Login failed (HTTP ${r.status})`) };
}

// Two fetches in a row: who am I, then that user's profile — the second
// request depends on the first reply (LLP 1027 D1a, `parse` → `Later`).
async function profile(store: Store): Promise<Session> {
  const kept = store.get(SECRET);
  if (kept === null) return { ...idle, error: "Not signed in" };
  const token = (JSON.parse(kept) as { token: string }).token;
  await Promise.resolve(); // A microtask before the first fetch still owns this call.
  const me = await fetch("https://api.castle.xyz/me", { headers: { "x-auth-token": token } });
  const username = String(((await me.json()) as any).username ?? "");
  const p = await fetch(`https://api.castle.xyz/profile/${encodeURIComponent(username)}`);
  return { ok: true, username, error: await p.text() };
}

function logout(store: Store): Session {
  store.forget(SECRET);
  return idle;
}

// An answer that awaits something no fetch will ever resolve.
async function stuck(): Promise<Session> {
  await new Promise<void>(() => {});
  return idle;
}

// A refusal, thrown, and one thrown after a fetch.
function refused(): Session {
  throw new DataError("Unavailable", "refused on purpose");
}
async function refusedLater(): Promise<Session> {
  await fetch("https://api.castle.xyz/ping");
  throw new Error("after the fetch");
}

// One fetch two answers share, as a page's code memoizes one (hn-reader
// F7): the second answer awaits the first's request, not one of its own.
const items = new Map<string, Promise<string>>();
function item(id: string): Session | Promise<Session> {
  if (!id) return idle;
  if (!items.has(id)) items.set(id, fetch(`https://api.castle.xyz/item/${id}`).then(r => r.text()));
  return items.get(id)!.then(error => ({ ok: true, username: id, error }));
}

// It awaits the shared fetch, then fetches again: that continuation runs as
// the answer whose reply settled the shared one, so its request lands among
// that answer's tickets (review r4a 1).
async function followup(id: string): Promise<Session> {
  if (!id) return idle;
  const first = await item(id);
  const more = await fetch(`https://api.castle.xyz/comments/${id}`);
  return { ok: true, username: id, error: first.error + " + " + await more.text() };
}

// A save that does not await its POST: the answer is given at once, the
// request still goes (a browser runs it), even while another answer is in
// flight (Grok's batch 2 review, runtime).
async function saveQuietly(id: string): Promise<Session> {
  void fetch(`https://api.castle.xyz/save/${id}`, { method: "POST", body: id });
  return { ok: true, username: id, error: "saved" };
}

// One controller for every fetch (review r4a 7): a browser realm's fetch
// watches its signal with a listener it removes when the fetch settles;
// Hermes's watches through Ibex's own abort hooks and adds none.
const reusedController = new AbortController();
const listening = { added: 0, removed: 0 };
{
  const signal = reusedController.signal as any, add = signal.addEventListener, remove = signal.removeEventListener;
  signal.addEventListener = function (...args: unknown[]) { listening.added++; return add.apply(this, args); };
  signal.removeEventListener = function (...args: unknown[]) { listening.removed++; return remove.apply(this, args); };
}
async function reused(): Promise<Session> {
  await fetch("https://api.castle.xyz/reused", { signal: reusedController.signal });
  return { ok: true, username: "", error: `${listening.added}/${listening.removed}` };
}

async function parallel(): Promise<Session> {
  const [a, b] = await Promise.all([fetch("https://api.castle.xyz/a"), fetch("https://api.castle.xyz/b")]);
  return {ok:true,username:"parallel",error:Array.from(new Uint8Array(await a.arrayBuffer())).join(",") + "/" + await b.text()};
}

function answer(source: string, args: unknown[], store: Store): unknown {
  switch (source) {
    case "remember": return remember(store);
    case "login": return login(text(args, 0), text(args, 1), store);
    case "profile": return profile(store);
    case "logout": return logout(store);
    case "stuck": return stuck();
    case "refused": return refused();
    case "refusedLater": return refusedLater();
    case "parallel": return parallel();
    case "item": case "thread": return item(text(args, 0));
    case "followup": return followup(text(args, 0));
    case "reused": return reused();
    case "saveQuietly": return saveQuietly(text(args, 0));
    // An answer that keeps coming (LLP 1016.000): each event, and the end.
    case "events": return fetch("https://api.castle.xyz/events?since=" + args[0], {
      exactStream: (e: { type: string; data: string; lastEventId: string; coalesced: number; message?: string }) =>
        e.type === "error" ? `ended: ${e.message}` : `${e.type} ${e.lastEventId}:${e.data}:${e.coalesced}:${store.get("castle.session") ?? "-"}`,
    } as RequestInit);
    // A socket is the same answer with a `wss:` URL (LLP 1069.004 slice 3):
    // Jetstream's shape, the cursor a query parameter.
    case "socket": return fetch("wss://jetstream.castle.xyz/subscribe?cursor=" + args[0], {
      exactStream: (e: { type: string; data: string; message?: string }) =>
        e.type === "error" ? `closed: ${e.message}` : `${e.type}:${e.data}`,
    } as RequestInit);
    case "scheduled": return fetch("https://api.castle.xyz/search/" + args[0], {
      method: "POST",
      ...(args[2] ? { exactIndependentHttp: { maxResponseBytes: args[1] } } : {}),
    }).then(r => r.text());
    // A deadline for the whole exchange: a timeout rejects with its kind.
    case "timed": return fetch("https://api.castle.xyz/slow", { exactTimeout: args[0] } as RequestInit)
      .then(r => r.text(), (e: { kind: string; message: string }) => `failed: ${e.kind}: ${e.message}`);
    default: throw new DataError("UnknownSource", source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "xyz.castle.test",
  grants: "net.fetch https://api.castle.xyz\nsecret.keep castle.session\n",
  answer,
};
