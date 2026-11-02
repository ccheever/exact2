import { oauth } from "../../../packages/oauth/index.ts";
// Answers that await a promise another answer made, then fetch their own
// rows (LLP 1133.002 D14): each fetch after the await must be its own
// answer's, through `Promise.all` and an imported helper too. The test's
// transport sees which answer each request arrives under.

const API = "https://context.test";

let shared: Promise<string> | null = null;

// One load, made by whichever answer asks first.
function load(): Promise<string> {
  shared ??= fetch(`${API}/shared`).then((r) => r.text());
  return shared;
}

async function helper(name: string): Promise<string> {
  return (await fetch(`${API}/${name}/helper`)).text();
}

async function work(name: string): Promise<{ text: string }> {
  const loaded = await load();
  const [one, two] = await Promise.all([
    fetch(`${API}/${name}/one`).then((r) => r.text()),
    fetch(`${API}/${name}/two`).then((r) => r.text()),
  ]);
  const nested = await helper(name);
  return { text: [loaded, one, two, nested].join(" ") };
}

const auth = oauth({ key: "token", clientId: "native-test", scopes: [], apis: [API],
  endpoints: { authorization: `${API}/authorize`, token: `${API}/token` } });
function answer(source: string, args: unknown[], store: any): unknown {
  if (source === "secret" && String(args[0]).startsWith("read-")) return store.exclusive("token", () => ({ text: store.get("token") ?? "empty" }));
  if (source === "secret") return auth.fetch(store, `${API}/data`).then(r => r.text()).then(text => ({ text }), e => ({ text: `failed:${e.message}` }));
  if (source === "work" && typeof args[0] === "string") return work(args[0]);
  throw { kind: "UnknownSource", message: source };
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.context",
  grants: "net.fetch https://context.test\nsecret.keep token\n",
  answer,
};
