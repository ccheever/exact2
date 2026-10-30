// Lists whose elements an answer repeats (js/src/binary.js `keptReply`): deep-frozen plain rows the prelude may keep
// the JSON of, beside rows it must not keep (mutated, frozen around a mutable list, a toJSON hook, a getter, a
// large string), a string and a key that begin with U+0000, and values JSON drops. `rows(kind)` answers a kind's
// list; `rows("plain " + kind)` answers a fresh, unfrozen copy of the same list as JSON reads it now, the oracle;
// `rows("tick")` moves the counter the hooks and mutations read; `lengths()` counts a proxied list's length reads.
type Inner = { k: string };
type Row = { id: string; text: string; n: number; inner: Inner[] };

let count = 0;
const deep = <T>(v: T): T => {
  if (v && typeof v === "object" && !Object.isFrozen(v)) {
    Object.freeze(v);
    for (const name of Object.getOwnPropertyNames(v)) deep((v as any)[name]);
  }
  return v;
};
const make = (i: number, text = `row ${i}`): Row => ({ id: `r${i}`, text, n: i / 3, inner: [{ k: `k${i}` }, { k: "  é \"q\" \\" }] });
const list = (build: (i: number) => Row): Row[] => Array.from({ length: 40 }, (_, i) => build(i));

const frozen = deep(list((i) => make(i)));
const mutable = list((i) => make(i));
const shallow = list((i) => Object.freeze({ ...make(i), inner: [{ k: `k${i}` }] }) as Row);
// A row's own hook runs before the replacer sees it; one inside a frozen row would be stale if the row were kept.
const hooked = deep(list((i) => (i === 2 ? ({ toJSON: () => make(2, `hook ${count}`) } as unknown as Row)
  : i === 3 ? { ...make(3), inner: [{ toJSON: () => ({ k: `hook ${count}` }) } as unknown as Inner] } : make(i))));
const getter = deep(list((i) => (i === 4 ? (Object.defineProperty({ ...make(4) }, "text", { get: () => `get ${count}`, enumerable: true }) as Row) : make(i))));
const large = deep(list((i) => make(i, i === 6 ? "x".repeat(70_000) : `row ${i}`)));
// Strings in the reply's own text that hold U+0000: at the start, and after a quote (JSON's `\"\u0000`).
const nul = Object.freeze(list((i) => (i === 1 ? make(i, "\u0000" + "0") : deep(make(i)))));
const nulQuote = Object.freeze(list((i) => (i === 2 ? make(i, 'a"' + "\u0000" + '1"') : deep(make(i)))));
// Unfrozen, so its key is written into the reply's own text beside kept rows.
const nulKey = Object.freeze(list((i) => (i === 1 ? ({ ...make(i), ["\u0000" + "0"]: "k" } as Row) : deep(make(i)))));
const dropped = deep(list((i) => ({ ...make(i), extra: undefined, fn: () => i, [Symbol("s")]: i } as unknown as Row)));
const short = deep(Array.from({ length: 3 }, (_, i) => make(i)));
// A boxed string JSON unboxes after the replacer, and a key whose value JSON drops (so the key isn't written).
const boxed = Object.freeze(list((i) => (i === 1 ? ({ ...make(i), text: new String("\u0000" + "1") } as unknown as Row) : deep(make(i)))));
const keyDropped = Object.freeze(list((i) => (i === 1 ? ({ ...make(i), ["\u0000" + "2"]: undefined } as Row) : deep(make(i)))));
// Hooks that would make a frozen row's JSON change: a function's own toJSON, a number wrapper's toPrimitive.
const hook = () => `fn ${count}`;
(hook as any).toJSON = hook;
const wrapper = Object(5);
Object.setPrototypeOf(wrapper, null);
wrapper[Symbol.toPrimitive] = () => count;
const fnHook = deep(list((i) => (i === 2 ? ({ ...make(i), text: hook } as unknown as Row) : make(i))));
const wrapped = deep(list((i) => (i === 2 ? ({ ...make(i), n: wrapper } as unknown as Row) : make(i))));
// A string that ends in U+0000 and digits after a quote (JSON's `\"\u00001"`), as a value and as a key.
const nulTail = Object.freeze(list((i) => (i === 2 ? make(i, 'a"' + "\u0000" + "1") : deep(make(i)))));
const nulTailKey = Object.freeze(list((i) => (i === 2 ? ({ ...make(i), ['a"' + "\u0000" + "1"]: "k" } as Row) : deep(make(i)))));
// Boxed strings whose own toString JSON honors (defined, since the hardened String.prototype's can't be assigned over): spelled like a token only once converted, and the reverse.
const spelled = new String("ordinary");
Object.defineProperty(spelled, "toString", { value: () => "\u0000" + "1" });
const unspelled = new String("\u0000" + "1");
Object.defineProperty(unspelled, "toString", { value: () => "ordinary" });
const boxedHook = Object.freeze(list((i) => (i === 1 ? ({ ...make(i), text: spelled } as unknown as Row) : deep(make(i)))));
const boxedUnspelled = Object.freeze(list((i) => (i === 1 ? ({ ...make(i), text: unspelled } as unknown as Row) : deep(make(i)))));
// A frozen row holding a proxy whose `toJSON` reads the counter: kept, the row would show an old count.
const nested = new Proxy(Object.freeze({ k: "proxied" }), { get: (t, k, r) => (k === "toJSON" ? () => ({ k: `proxy ${count}` }) : Reflect.get(t, k, r)) });
const nestedProxy = deep(list((i) => (i === 3 ? { ...make(i), inner: [nested as unknown as Inner] } : make(i))));
// The same through Proxy.revocable: a row holding one is no more kept than one holding `new Proxy`.
const revocableNested = Proxy.revocable(Object.freeze({ k: "proxied" }), { get: (t, k, r) => (k === "toJSON" ? () => ({ k: `revocable ${count}` }) : Reflect.get(t, k, r)) }).proxy;
const revocableProxy = deep(list((i) => (i === 3 ? { ...make(i), inner: [revocableNested as unknown as Inner] } : make(i))));
// A frozen row holding a proxy over a frozen sparse list whose trap answers the hole with the counter.
const holed = new Proxy(Object.freeze([, Object.freeze({ k: "x" })]), { get: (t, k, r) => (k === "0" ? { k: `hole ${count}` } : Reflect.get(t, k, r)) });
const proxyHoles = deep(list((i) => (i === 3 ? { ...make(i), inner: holed as unknown as Inner[] } : make(i))));
// A boxed string whose own toString throws, counting its calls: converted once a reply, as JSON would.
let conversions = 0;
const throwing = new String("never");
Object.defineProperty(throwing, "toString", { value: () => { conversions++; throw new Error("no text"); } });
const boxedThrows = Object.freeze(list((i) => (i === 1 ? ({ ...make(i), text: throwing } as unknown as Row) : deep(make(i)))));
// A frozen row whose nested object's toJSON is an accessor counting its reads: refused unread, and read by
// JSON once a reply, as a plain stringify reads it.
let hookReads = 0;
const counting = Object.defineProperty({ k: "x" }, "toJSON", { get: () => { hookReads++; return () => ({ k: `hook ${count}` }); } });
const accessorHook = deep(list((i) => (i === 2 ? { ...make(i), inner: [counting as Inner] } : make(i))));
// A list read through a proxy that counts how often its length is read: as often as JSON reads it, no more.
let lengths = 0;
const proxiedList = new Proxy(Object.freeze(list((i) => deep(make(i)))), { get: (t, k, r) => { if (k === "length") lengths++; return Reflect.get(t, k, r); } });
// A string spelled like a token inside a row that is kept: its text is spliced in after the scan.
const nulKept = deep(list((i) => make(i, i === 1 ? "\u0000" + "3" : `row ${i}`)));
const kinds: Record<string, Row[]> = { frozen, mutable, shallow, hooked, getter, large, nul, nulQuote, nulKey, dropped, short, boxed, keyDropped, fnHook, wrapped,
  nulTail, nulTailKey, boxedHook, boxedUnspelled, nestedProxy, nulKept, proxyHoles, boxedThrows, accessorHook, revocableProxy, proxiedList: proxiedList as Row[] };

function answer(source: string, args: unknown[]): unknown {
  if (source === "lengths") return String(lengths);
  if (source === "conversions") return String(conversions);
  if (source === "hookReads") return String(hookReads);
  if (source === "newTargetReads") {
    // A new target whose `prototype` read is counted (a Proxy over a function): the wrapper's one divergence.
    let reads = 0;
    const target = new Proxy(function () {}, { get: (t, k, r) => { if (k === "prototype") reads++; return Reflect.get(t, k, r); } });
    Reflect.construct(Proxy, [{}, {}], target);
    return String(reads);
  }
  if (source === "proxy") {
    // The engine's Proxy, as an app sees it after the prelude tracks the ones it makes.
    const made = new Proxy({ a: 1 }, {}), revocable = Proxy.revocable({ b: 2 }, {});
    let called = "";
    try { (Proxy as any)({}, {}); } catch (e) { called = (e as Error).constructor.name; }
    revocable.revoke();
    let revoked = "";
    try { void (revocable.proxy as any).b; } catch (e) { revoked = (e as Error).constructor.name; }
    let revocableNew = "";
    try { new (Proxy.revocable as any)({}, {}); } catch (e) { revocableNew = (e as Error).constructor.name; }
    const viaReflect = Reflect.construct(Proxy, [{ c: 3 }, {}], Object) as any;
    return [typeof Proxy, (made as any).a, called, revoked, String(Proxy.prototype), revocableNew, viaReflect.c,
      Proxy.name, Proxy.length, Object.getOwnPropertyDescriptor(Proxy, "revocable")!.enumerable].join(" ");
  }
  if (source !== "rows") throw new Error("unknown source " + source);
  const kind = String(args[0]);
  if (kind === "tick") {
    count++;
    mutable[3].text = `mutated ${count}`;
    shallow[5].inner[0].k = `mutated ${count}`;
    return [];
  }
  if (kind.startsWith("plain ")) return JSON.parse(JSON.stringify(kinds[kind.slice(6)]));
  return kinds[kind];
}

(globalThis as any).exact = { abi: 1, appId: "test.kept-json", grants: "", answer };
