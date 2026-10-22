// A refused inspect, said in the Inspector (the hands-on pass on the clone): a build whose page answered
// `unsupported inspect` is said there while this tool inspects it, until that build inspects after all (a view
// selected in it), inspect is asked again, the build registers again, or another tool or none owns inspect there.
// Owning inspect says nothing of whether its page inspects: the server names its owner on sending, refused or not.

/** What a build carries of its refusal: its text (null for none) and a count of the events that moved it. */
export type Refusing = { inspectRefused?: string | null; refusalSeq?: number; id: string; host: string; page: string; status: any };

/** A build's refusal said or let go by an event, counted: a failed re-send puts back only what nothing has moved since. */
export const refuse = (b: Refusing | undefined, r: string | null) => { if (b) { b.inspectRefused = r; b.refusalSeq = (b.refusalSeq ?? 0) + 1; } };

/** A listing's word on a refusal: a build first seen takes the one the server still records; one registered again (its
 * status reset, and no refusal of inspect recorded) has none; any other keeps the tool's. Counted only when it moves. */
export const refusalOf = (b: any, was?: Refusing) => {
  const r = !was ? (b.unsupported?.what === 'inspect' ? String(b.unsupported.detail ?? '') : null)
    : b.status == null && (was.status != null || b.unsupported?.what !== 'inspect') ? null : was.inspectRefused ?? null;
  return { inspectRefused: r, refusalSeq: (was?.refusalSeq ?? 0) + (r === (was?.inspectRefused ?? null) ? 0 : 1) };
};

/** The refusals the Inspector says (at most three, then a count): each as the Builds list names its build (the
 * Inspector names a grid copy by its tile, which it reads as it draws). */
export const refusalRows = (builds: Iterable<Refusing>) => {
  const all = [...builds], refused = all.filter(b => b.inspectRefused != null);
  return {
    refusals: refused.slice(0, 3).map(b => ({ id: b.id, host: b.host, name: `${b.host} · ${b.id}${b.page === 'gallery' ? ' · the gallery' : ''}`,
      tail: ` could not turn Inspect on${b.inspectRefused ? `: ${b.inspectRefused}` : ''}. Stop inspecting and Inspect again to retry.` })),
    refusedMore: Math.max(0, refused.length - 3),
    // Each build's refusal events counted (each moves it, the same words again included): what a move up the chain on
    // that build waits to see change (tamber/1.16 `select`).
    refusalCounts: all.map(b => ({ id: b.id, count: b.refusalSeq ?? 0 })),
    // The builds whose page answered a move up the chain that it can't step.
    noStep: [...cantStep],
  };
};

/** Inspect asked again: the refusals said before go, kept with their counts for `restoreRefusals`. */
export const takeRefusals = (builds: Map<string, Refusing>) => {
  const kept = new Map([...builds].map(([id, b]) => [id, { r: b.inspectRefused ?? null, at: b.refusalSeq ?? 0 }]));
  for (const b of builds.values()) b.inspectRefused = null;
  return kept;
};
/** Not asked after all (the request failed): what was refused is said again, where nothing has moved it since. */
export const restoreRefusals = (builds: Map<string, Refusing>, kept: ReturnType<typeof takeRefusals>) => {
  for (const [id, { r, at }] of kept) { const b = builds.get(id); if (b && (b.refusalSeq ?? 0) === at) b.inspectRefused = r; }
};

/** A move up the chain sent to a build (tamber/1.16 `select`), until its page answers: an `unsupported inspect` in
 * that while is the page saying it can't step (Apple's for now), which greys out the tool's up control for that build
 * alone, never inspect (theme2 15:02). A selection from it ends the while. */
export const stepping = new Set<string>(), cantStep = new Set<string>();
export const stepRefused = (id: string) => { if (!stepping.delete(id)) return false; cantStep.add(id); return true; };
