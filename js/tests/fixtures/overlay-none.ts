// A module loaded with no overlay: `arm` gives it one after load (which adds a row) and `disarm` takes it away
// again. The host decides at load, so neither is followed, as on the JS web target.
function overlay(_source: string, _args: unknown[], answer: { text: string }[]): unknown {
  return answer.concat([{ text: "overlaid" }]);
}

function answer(source: string): unknown {
  const exact = (globalThis as any).exact;
  if (source === "items") return [];
  if (source === "arm") { exact.overlay = overlay; return "armed"; }
  if (source === "disarm") { delete exact.overlay; return "disarmed"; }
  throw new Error("unknown source " + source);
}

(globalThis as any).exact = {
  abi: 1,
  appId: "test.overlay-none",
  grants: "",
  answer,
};
