// A module loaded with an overlay from its top level (the module the no-overlay one becomes when loaded again
// with one): its overlay adds a row to the answer. `disarm` deletes it after load, which is not followed.
function overlay(_source: string, _args: unknown[], answer: { text: string }[]): unknown {
  return answer.concat([{ text: "overlaid" }]);
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.overlay-none",
  grants: "",
  answer: (source: string) => {
    if (source === "items") return [];
    if (source === "disarm") { delete (globalThis as any).exact.overlay; return "disarmed"; }
    throw new Error("unknown source " + source);
  },
  overlay,
};
