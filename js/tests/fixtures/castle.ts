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
    // An answer that keeps coming (LLP 1016.000): each event, and the end.
    case "events": return fetch("https://api.castle.xyz/events?since=" + args[0], {
      exactStream: (e: { type: string; data: string; lastEventId: string; coalesced: number; message?: string }) =>
        e.type === "error" ? `ended: ${e.message}` : `${e.type} ${e.lastEventId}:${e.data}:${e.coalesced}:${store.get("castle.session") ?? "-"}`,
    } as RequestInit);
    case "scheduled": return fetch("https://api.castle.xyz/search/" + args[0], {
      method: "POST",
      ...(args[2] ? { exactIndependentHttp: { maxResponseBytes: args[1] } } : {}),
    }).then(r => r.text());
    default: throw new DataError("UnknownSource", source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "xyz.castle.test",
  grants: "net.fetch https://api.castle.xyz\nsecret.keep castle.session\n",
  answer,
};
