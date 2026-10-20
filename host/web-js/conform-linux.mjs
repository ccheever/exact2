// The Linux comparison's narrow apart form (theme2's 18:18 ruling): a
// target's `LINUX_APART` entry `{ slots: [...], why }` leaves only those
// state slots uncompared on Linux, every other slot and the tree compared
// through the whole drive. Any other entry (a `[step, why]` stop) skips none.
export function withoutSlots(apart, slots) {
  const named = !Array.isArray(apart) && Array.isArray(apart?.slots) ? apart.slots : [];
  if (!named.length || slots == null || typeof slots !== 'object') return slots;
  return Object.fromEntries(Object.entries(slots).filter(([name]) => !named.includes(name)));
}
