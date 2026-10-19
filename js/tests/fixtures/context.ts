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

function answer(source: string, args: unknown[]): unknown {
  if (source === "work" && typeof args[0] === "string") return work(args[0]);
  throw { kind: "UnknownSource", message: source };
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.context",
  grants: "net.fetch https://context.test\n",
  answer,
};
