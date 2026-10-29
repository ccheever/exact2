// A proxied `exact` whose `overlay` getter tries `store.set` with the store an answer captured (`arm`):
// at commit, both of `__exact_overlay`'s reads run guarded, so each try is refused. `reads` answers how each
// read's try went, one line per read after `arm`.
function overlay(_source: string, _args: unknown[], answer: { text: string }[]): unknown {
  return answer.concat([{ text: "overlaid" }]);
}

let store: any = null;
let tries: string[] = [];
const target = {
  abi: 1,
  appId: "test.overlay-none",
  grants: "",
  answer: (source: string, _args: unknown[], given: unknown) => {
    if (source === "items") return [];
    if (source === "arm") { store = given; tries = []; return "captured"; }
    if (source === "reads") return tries.join("\n");
    throw new Error("unknown source " + source);
  },
};

(globalThis as any).exact = new Proxy(target, {
  get(t, key) {
    if (key !== "overlay") return Reflect.get(t, key);
    if (store) {
      try { store.set("x", "y"); tries.push("ran"); } catch (e) { tries.push("throws " + (e as Error).message); }
    }
    return overlay;
  },
});
