// A design read again after writes (the third pass's #5): describe, then a
// `resolve` only for the tokens the writes since the last read moved, the
// rest that read's answers, then hello. A full read is a resolve per token,
// at the one design server behind each other (the clone's 267 took 3.6 s).

/** A changeset entry as a read again follows it: the files' digests either
 * side, the tokens its event named (with the dependents in its file, as
 * the server plans them), and whether its event named none (a hand edit) or
 * stands for events missed. */
export type Step = { before: Record<string, string>; after: Record<string, string>; changed: { token: string }[]; hand?: boolean; gap?: boolean };
/** A whole read of one file's design, as the next read starts from. */
export type Kept = { epoch: string; digests: string; outline: string; entries: Map<string, string>; answers: Map<string, any> };

/** What a read again leaves to the server: the tokens and the axes, the
 * files and the packages. A change to any is a full read. */
const outlineOf = (ok: any) => JSON.stringify([ok.ir.axes, ok.ir.tuples, ok.ir.tokens.map((t: { name: string; kind: string; file?: string; package?: string }) => [t.name, t.kind, t.file ?? null, t.package ?? null]), Object.keys(ok.files ?? {}).sort(), ok.packages ?? null]);

/** The read `described` (describe's answer) and its `answers` (a resolve's
 * per token, in the IR's order), kept for the next. */
export function keep(epoch: string, described: any, answers: any[], digestKey: (f?: Record<string, string>) => string): Kept {
  const tokens = described.ir.tokens as { name: string }[];
  return { epoch, digests: digestKey(described.files), outline: outlineOf(described), entries: new Map(tokens.map(t => [t.name, JSON.stringify(t)])), answers: new Map(tokens.map((t, i) => [t.name, answers[i]])) };
}

/** The tokens to resolve again for `described`, `kept` the last whole read
 * (on the server run `epoch`), `steps` the changeset newest first; null for
 * a full read. From describe's revision back to the kept one, each step is
 * a write's that named its tokens; it then resolves those and every token
 * whose IR entry moved (a package's token derived from one the app's file
 * set, which the event, of the app's file, does not name). A hand edit, a
 * gap, another server run, another outline or a token named that the
 * design has not is a full read. */
export function again(kept: Kept | undefined, epoch: string, described: any, steps: Step[], digestKey: (f?: Record<string, string>) => string): Set<string> | null {
  if (!kept || !epoch || kept.epoch !== epoch || kept.outline !== outlineOf(described)) return null;
  const names = new Set<string>();
  let at = digestKey(described.files), i = steps.findIndex(n => !n.gap && digestKey(n.after) === at);
  while (at !== kept.digests) {
    const n = steps[i++];
    if (!n || n.gap || n.hand || digestKey(n.after) !== at) return null;
    for (const c of n.changed) names.add(c.token);
    at = digestKey(n.before);
  }
  const tokens = described.ir.tokens as { name: string }[];
  if ([...names].some(name => !kept.entries.has(name))) return null;
  for (const t of tokens) if (kept.entries.get(t.name) !== JSON.stringify(t)) names.add(t.name);
  return names;
}
