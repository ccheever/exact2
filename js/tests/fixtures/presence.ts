// LLP 1133.010 D5 through the Hermes executor: a presence name's occupancy,
// `get`'s refusal of a locked one, a locked write refused where no locked
// secret can be kept, and `unlock`'s request of its own.
type Store = any;
const grants = "secret.keep token user-presence optional\nsecret.keep key user-presence\ndevice.local-auth purpose.lock\n";
const failed = (e: any) => ({text: `${e.name} ${e.code ?? ""}: ${e.message}`});
function answer(source: string, args: unknown[], store: Store) {
  const op = String(args[0]);
  if (op === "read") { try { return {text: String(store.get("token"))}; } catch (e: any) { return failed(e); } }
  if (op === "occupancy") return {text: [store.has("token"), store.locked("token"), store.has("key"), store.locked("key")].join(" ")};
  if (op === "set-key") { try { store.set("key", "secret-2"); return {text: "kept"}; } catch (e: any) { return failed(e); } }
  if (op === "unlock") return store.unlock("token", {reason: "reason.unlock"}).then((v: string | null) => ({text: `unlocked ${v}`}), failed);
  if (op === "protect") return store.protect("token").then(() => ({text: "protected"}), failed);
  return {text: "?"};
}
(globalThis as any).exact = {abi: 1, appId: "dev.exact.presence-test", grants, answer};
