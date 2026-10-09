// Exercise the real bytecode loader, including aliases captured at module
// initialization. These fixtures deliberately attempt unavailable APIs.
const { now } = Date;
const { random } = Math;
const DateAlias = globalThis.Date;
const constructorAlias = new Date(0).constructor as DateConstructor;
const dateFormatter = new Intl.DateTimeFormat("en-US", { year: "numeric", timeZone: "UTC" });
const { format } = dateFormatter;
const parts = dateFormatter.formatToParts.bind(dateFormatter);
const formatGetter = Object.getOwnPropertyDescriptor(Intl.DateTimeFormat.prototype, "format")!.get!;
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
  "intl-format": () => dateFormatter.format(),
  "intl-format-undefined": () => dateFormatter.format(undefined),
  "intl-parts": () => dateFormatter.formatToParts(),
  "intl-parts-undefined": () => dateFormatter.formatToParts(undefined),
  "intl-format-alias": () => format(),
  "intl-format-alias-undefined": () => format(undefined),
  "intl-parts-alias": () => parts(),
  "intl-parts-alias-undefined": () => parts(undefined),
  "intl-format-getter": () => formatGetter.call(dateFormatter)(),
  "intl-format-computed": () => new globalThis["In" + "tl"]["DateTime" + "Format"]()["for" + "mat"](),
  "intl-parts-prototype": () => Intl.DateTimeFormat.prototype.formatToParts.call(dateFormatter),
  timeout: () => setTimeout(() => {}, 0),
  interval: () => setInterval(() => {}, 1000),
  "computed-timeout": () => (globalThis as any)["set" + "Timeout"](() => {}, 0),
  frame: () => requestAnimationFrame(() => {}),
  performance: () => performance.now(),
};
// The browser's own I/O, refused by name as on the web (LLP 1016.000 D3).
const io: Record<string, () => unknown> = {
  kinds: () => [typeof WebSocket, typeof XMLHttpRequest, typeof EventSource].join(" "),
  socket: () => new WebSocket("wss://fixture.exact.test/s"),
  request: () => new XMLHttpRequest(),
  "event-source": () => new EventSource("https://fixture.exact.test/e"),
  "computed-socket": () => new (globalThis as any)["Web" + "Socket"]("wss://fixture.exact.test/s"),
  "call-socket": () => (WebSocket as any)("wss://fixture.exact.test/s"),
  "reflect-socket": () => Reflect.construct(WebSocket, ["wss://fixture.exact.test/s"]),
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
    case "io": try { return String(io[args[0]]()); } catch (e) { return (e as Error).name + ": " + (e as Error).message; }
    case "ambientLater": return fetch("https://fixture.exact.test/value").then(() => calls[args[0]]());
    case "explicit":
      console.log("explicit", args[0], args[1]);
      return explicit(args[0], args[1]);
    case "explicitLater":
      return fetch("https://fixture.exact.test/value").then(() => { console.log("explicitLater", args[0], args[1]); return explicit(args[0], args[1]); });
    case "utc":
      return [
        new Date(Date.UTC(2024, 1, 29, 12, 34, 56, 789)).toISOString(),
        new Date("2024-02-29T12:34:56.789Z").getUTCHours(),
        Date.parse("2024-02-29T12:34:56.789Z"),
        Number.isNaN(new Date(undefined).getTime()),
      ].join("/");
    case "intl": {
      const epochMs = args[0];
      if (format !== dateFormatter.format || format !== formatGetter.call(dateFormatter)) {
        throw new Error("DateTimeFormat format identity");
      }
      return [
        dateFormatter.format(epochMs), format(epochMs),
        parts(epochMs).map((part) => part.value).join(""),
        Intl.DateTimeFormat.prototype.formatToParts.call(dateFormatter, epochMs).map((part) => part.value).join(""),
      ].join("/");
    }
    default: throw new Error("unknown source " + source);
  }
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.explicit-inputs",
  grants: "net.fetch https://fixture.exact.test\n",
  answer,
};
