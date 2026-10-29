// A source that never returns on its own: a synchronous loop only an
// interrupt from another thread ends (LLP 1048.000 D10). `words` answers at
// once, except for the name "spin", for which it spins, telling the native
// module, where there is one, from inside the loop once it is running.

type Words = string[];

type Native = { available: boolean; call(request: Record<string, unknown>): unknown } | null;

function words(args: unknown[], native: Native): Words {
  const names = Array.isArray(args[0]) ? args[0] : [];
  const name = typeof names[0] === "string" ? names[0] : "";
  if (name === "spin") spin(native);
  return ["hello", name];
}

function spin(native: Native): void {
  let turns = 0;
  for (;;) {
    if (turns === 1 && native?.available) native.call({ spinning: true });
    turns++;
  }
}

function answer(source: string, args: unknown[], _store: unknown, _storage: unknown, native: Native): unknown {
  if (source === "words") return words(args, native);
  throw { kind: "UnknownSource", message: source };
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.spin",
  grants: "",
  answer,
};
