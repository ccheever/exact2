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
 * moved (belt and braces: the app's design is planned folded with its
 * packages', so an event names their derived tokens too). An event that named none, a
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

// The most names one batch asks: about 370 KB of the clone's answers (the server takes 4,096, 16 MiB of answers).
const BATCH_NAMES = 1024;

/** Each of `names`' `resolve`, as one per name answers it (a refused one
 * `{error, reply: {error}}`): one `resolve` with `tokens` (tamber/1.17, the
 * third pass's #5 B), one snapshot and one fold at the server; else, from a
 * server before it, one per name, four at a time (a browser keeps
 * connections for the long poll and the other reads), until `still` is
 * false: none once it is. */
export async function resolveAll<R extends { error: string; reply: any }>(call: (path: string, body: unknown, timeout: number) => Promise<R>, names: string[], which: { file?: string }, timeout: number, still: () => boolean): Promise<R[]> {
  if (!names.length || !still()) return [];
  // At most 1,024 names an ask (the server takes 4,096), and half as many again when a batch's answers pass its bytes.
  if (names.length > BATCH_NAMES) {
    const out: R[] = [];
    for (let at = 0; at < names.length && still(); at += BATCH_NAMES) out.push(...await resolveAll(call, names.slice(at, at + BATCH_NAMES), which, timeout, still));
    return out;
  }
  const batch = await call('/__tamber', { method: 'resolve', tokens: names, ...which }, timeout), answers = batch.reply?.ok?.answers;
  if (batch.reply?.error?.id === 'tamber-resolve-batch' && names.length > 1) {
    const half = Math.ceil(names.length / 2), first = await resolveAll(call, names.slice(0, half), which, timeout, still);
    return first.length < half ? first : [...first, ...await resolveAll(call, names.slice(half), which, timeout, still)];
  }
  if (!batch.error && Array.isArray(answers) && answers.length === names.length) {
    return answers.map((a: any) => (a?.error ? { ...batch, error: String(a.error.message ?? 'The design server refused the token.'), reply: { error: a.error } } : { ...batch, reply: { ok: { ...a, files: batch.reply.ok.files } } }));
  }
  // Asked again by name only from a server before the batch, which refuses it for its missing `token` (in the words
  // contract/cli/tests/it/tamber_resolve_batch.rs pins: a refusal carries no id to match on); any other
  // failure (no server, a timeout, the gateway's 504 while the batch still runs) is each name's answer. A request by
  // name that reached none stops the rest, each name not asked answering with it.
  if (!/^`token` is a string/.test(String(batch.reply?.error?.message ?? ''))) return names.map(() => batch);
  const out: R[] = [];
  let next = 0, stalled: R | null = null;
  await Promise.all(Array.from({ length: Math.min(4, names.length) }, async () => {
    while (next < names.length && still() && !stalled) {
      const i = next++;
      out[i] = await call('/__tamber', { method: 'resolve', token: names[i], ...which }, timeout);
      if (out[i].error && !out[i].reply?.error) stalled = out[i];
    }
  }));
  return stalled ? names.map((_, i) => out[i] ?? stalled!) : out;
}
