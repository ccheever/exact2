// Exercise the real bytecode loader, including aliases captured at module
// initialization. These fixtures deliberately attempt unavailable APIs.
const { now } = Date;
const { random } = Math;
const DateAlias = globalThis.Date;
const constructorAlias = new Date(0).constructor as DateConstructor;
const calls: Record<string, () => unknown> = {
  now: () => Date.now(),
  new: () => new Date(),
  call: () => Date(),
  "call-with-arg": () => (Date as any)(0),
  random: () => Math.random(),
  "alias-now": () => now(),
  "alias-random": () => random(),
  "alias-date": () => new DateAlias(),
  "prototype-constructor": () => new constructorAlias(),
  "computed-now": () => globalThis["Da" + "te"]["n" + "ow"](),
  "computed-random": () => globalThis["Ma" + "th"]["ran" + "dom"](),
  "bound-now": () => Date.now.bind(Date)(),
  "bound-new": () => new (Date.bind(null) as any)(),
  reflect: () => Reflect.construct(Date, []),
};

const atInit: Record<string, string> = {};
for (const name of Object.keys(calls)) {
  try { atInit[name] = "UNGUARDED: " + String(calls[name]()); }
  catch (e) { atInit[name] = String(e.message); }
}

function explicit(epochMs: number, seed: number): string {
  // App-owned pure seed calculation; no executor RNG state to carry.
  const choice = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  const date = new Date(epochMs);
  if (!(date instanceof Date) || date.constructor !== Date) throw new Error("Date identity");
  return date.toISOString() + "/" + choice;
}

function answer(source: string, args: any[]): unknown {
  switch (source) {
    case "atInit": return atInit[args[0]];
    case "ambient": return calls[args[0]]();
    case "ambientLater": return fetch("https://fixture.exact.test/value").then(() => calls[args[0]]());
    case "explicit":
      console.log("explicit", args[0], args[1]);
      return explicit(args[0], args[1]);
    case "explicitLater":
      return fetch("https://fixture.exact.test/value").then(() => explicit(args[0], args[1]));
    case "utc":
      return [
        new Date(Date.UTC(2024, 1, 29, 12, 34, 56, 789)).toISOString(),
        new Date("2024-02-29T12:34:56.789Z").getUTCHours(),
        Date.parse("2024-02-29T12:34:56.789Z"),
        Number.isNaN(new Date(undefined).getTime()),
      ].join("/");
    default: throw new Error("unknown source " + source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.explicit-inputs",
  grants: "net.fetch https://fixture.exact.test\n",
  answer,
};
