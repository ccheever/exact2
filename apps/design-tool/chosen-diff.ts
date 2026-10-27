// A held import's diff follows its card's choice at once (stage 15 D1.1, the third pass's #10 item 1). A card that
// shows a diff (a recomputed or edited import) shows the server's plan of every operation it holds; once the card has
// chosen fewer, it shows the plan of the ones chosen instead, asked as a plain `propose` (planned, never held or
// written: no wire change), once for each choice (kept by proposal and items until the card goes). All chosen is the
// held diff, asked of no one; none chosen is no diff.
import type { Sources } from './app.contract.d.ts';

type Call = (path: string, body: unknown, timeout?: number, bound?: string) => Promise<{ error: string; reply: { ok?: any; error?: any }; used: string }>;
type Card = { proposal: string; diff: string };

export function chosenDiffSources(call: Call, cards: () => Card[], chosen: (proposal: string, picked: string) => number[], opsOf: (proposal: string) => unknown[], bound: () => string | undefined): Pick<Sources, 'loadChosenDiffs'> {
  const planned = new Map<string, { diff: string; note: string }>();
  let answers = 0;
  return {
    loadChosenDiffs: async ([client, picked]) => {
      const shown = cards();
      // (a card gone, its plans with it)
      for (const key of planned.keys()) if (!shown.some(h => key.startsWith(`${h.proposal} `))) planned.delete(key);
      const diffs = await Promise.all(shown.map(async h => {
        const ops = opsOf(h.proposal), items = chosen(h.proposal, picked), key = `${h.proposal} ${items.join(',')}`;
        if (items.length === ops.length) return { proposal: h.proposal, diff: h.diff, note: '' };
        if (!items.length) return { proposal: h.proposal, diff: '', note: 'Nothing is chosen, so accepting it would write nothing.' };
        let got = planned.get(key);
        if (!got) {
          const { error, reply } = await call('/__tamber', { method: 'propose', client, ops: items.map(i => ops[i]) }, 15000, bound());
          got = error ? { diff: h.diff, note: `The diff of only what is chosen could not be planned (${error}): this is the diff of all of it.` } : { diff: String(reply.ok?.diff ?? ''), note: '' };
          if (!error) planned.set(key, got);
        }
        return { proposal: h.proposal, ...got };
      }));
      return { ok: true, answer: ++answers, diffs };
    },
  };
}
