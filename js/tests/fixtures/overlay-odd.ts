// A proxied `exact` whose `get` supplies `overlay` (no such property): the load-time read finds it. `reads`
// counts the trap's reads of `overlay`.
function overlay(_source: string, _args: unknown[], answer: { text: string }[]): unknown {
  return answer.concat([{ text: "overlaid" }]);
}

let reads = 0;
const target = {
  abi: 1,
  appId: "test.overlay-none",
  grants: "",
  answer: (source: string) => {
    if (source === "items") return [];
    if (source === "reads") return String(reads);
    throw new Error("unknown source " + source);
  },
};

(globalThis as any).exact = new Proxy(target, {
  get(t, key) {
    if (key !== "overlay") return Reflect.get(t, key);
    reads++;
    return overlay;
  },
});
