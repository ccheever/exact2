// A design read again after writes (the third pass's #5): describe, then a
// `resolve` only for the tokens the writes since the last whole read moved,
// the rest that read's answers, then hello. A full read is a resolve per
// token, at the one design server behind each other (the clone's 267 took
// 3.6 s).

/** A whole read of one file's design, as the next read starts from: its
 * server run, the last event seq the tool had folded when its describe was
 * sent (every event after it is a change it may not have seen), and its
 * outline, IR entries and resolve answers by token. */
export type Kept = { epoch: string; seq: number; outline: string; entries: Map<string, string>; answers: Map<string, any> };

// Every `changed` event folded: its run, seq, and the tokens it named (with
// the dependents in its file, as the server plans them), null when it named
// none (a hand edit, a package's design moved). The newest 1,000; a read
// kept from before the oldest is read whole.
const heardLog: { epoch: string; seq: number; tokens: string[] | null }[] = [];
let dropped = { epoch: '', seq: -1 };
export function heard(e: any) {
  if (e.event !== 'changed') return;
  heardLog.push({ epoch: String(e.epoch ?? ''), seq: Number(e.seq ?? 0), tokens: Array.isArray(e.changed) ? e.changed.map((c: { token: string }) => String(c.token)) : null });
  if (heardLog.length > 1000) dropped = heardLog.shift()!;
}

/** What a read again leaves to the server: the tokens and the axes, the
 * files and the packages, and the components' params (a param's name
 * resolves before a design token's: tamber_serve.rs `lookup`). A change to
 * any is a full read. */
const outlineOf = (ok: any) => JSON.stringify([ok.ir.axes, ok.ir.tuples, ok.ir.tokens.map((t: { name: string; kind: string; file?: string; package?: string }) => [t.name, t.kind, t.file ?? null, t.package ?? null]),
  (ok.ir.components ?? []).map((c: { identity: string; params?: { name: string; short?: string; token?: string }[] }) => [c.identity, (c.params ?? []).map(p => [p.name, p.short ?? null, p.token ?? null])]), Object.keys(ok.files ?? {}).sort(), ok.packages ?? null]);

/** The read `described` (describe's answer, sent after event `seq` of run
 * `epoch`) and its `answers` (a resolve's per token, in the IR's order),
 * kept for the next. An answer kept is one of describe's revision: a tuple
 * per column, each the IR's value for it (D5's `values` and `entries`). One
 * that is not (a package's design moved and moved back between describe and
 * its resolve, which no `files` digest shows) keeps no entry, so the next
 * read resolves it again. */
export function keep(epoch: string, seq: number, described: any, answers: any[]): Kept {
  const tokens = described.ir.tokens as { name: string; values: string[]; entries: number[] | null }[];
  const agrees = (t: (typeof tokens)[number], tuples: { value?: unknown }[] = []) => tuples.length === described.ir.tuples && tuples.every((u, k) => u.value === t.values[t.entries ? t.entries[k] : 0]);
  return { epoch, seq, outline: outlineOf(described), entries: new Map(tokens.map((t, i) => [t.name, agrees(t, answers[i]?.tuples) ? JSON.stringify(t) : ''])), answers: new Map(tokens.map((t, i) => [t.name, answers[i]])) };
}

/** The tokens to resolve again for `described`, `kept` the last whole read
 * (on the server run `epoch`), `changeset` the session's (its `gap`s); null
 * for a full read. Each event since the kept read's describe must have
 * named its tokens: it then resolves those and every token whose IR entry
 * moved (a package's token derived from one the app's file set, which the
 * event, of the app's file, does not name). An event that named none, a
 * gap, another server run, another outline or a token named that the
 * design has not is a full read. */
export function again(kept: Kept | undefined, epoch: string, described: any, changeset: { seq: number; gap?: boolean }[]): Set<string> | null {
  if (!kept || !epoch || kept.epoch !== epoch || kept.outline !== outlineOf(described)) return null;
  if ((dropped.epoch === epoch && dropped.seq > kept.seq) || changeset.some(n => n.gap && n.seq > kept.seq)) return null;
  const names = new Set<string>();
  for (const e of heardLog) {
    if (e.epoch !== epoch || e.seq <= kept.seq) continue;
    if (!e.tokens) return null;
    for (const t of e.tokens) names.add(t);
  }
  if ([...names].some(name => !kept.entries.has(name))) return null;
  // A token named as a param is (`lookup` reads the param first), whose default a package's source moves with no
  // event and no entry moving, is resolved every time.
  const params = new Set((described.ir.components ?? []).flatMap((c: { params?: { name: string; short?: string }[] }) => (c.params ?? []).flatMap(p => [p.name, p.short])));
  for (const t of described.ir.tokens as { name: string }[]) if (params.has(t.name) || kept.entries.get(t.name) !== JSON.stringify(t)) names.add(t.name);
  return names;
}
