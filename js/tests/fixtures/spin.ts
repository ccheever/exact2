// A source that never returns on its own: a synchronous loop only an
// interrupt from another thread ends (LLP 1048.000 D10). `words` answers at
// once, except for the name "spin", for which it spins.

type Words = string[];

function words(args: unknown[]): Words {
  const names = Array.isArray(args[0]) ? args[0] : [];
  const name = typeof names[0] === "string" ? names[0] : "";
  if (name === "spin") spin();
  return ["hello", name];
}

function spin(): void {
  let turns = 0;
  for (;;) turns++;
}

function answer(source: string, args: unknown[]): unknown {
  if (source === "words") return words(args);
  throw { kind: "UnknownSource", message: source };
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.spin",
  grants: "",
  answer,
};
