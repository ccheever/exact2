// A held import's diff follows its card's choice at once (stage 15 D1.1, the third pass's #10 item 1). A card that
// shows a diff (a recomputed or edited import) shows the server's plan of every operation it holds; once the card has
// chosen fewer, it shows the plan of the ones chosen instead, asked as a plain `propose` (planned, never held or
// written: no wire change), once for each choice on the files as they are (kept by proposal, items and the files'
// digests until the card goes, and kept only when the files planned are exactly the ones the changes say). All chosen
// is the held diff, asked of no one; none chosen is no diff. Each row names the card as it was shown (`shown`: its
// choiceGen and its flips), so a card ticked since shows no older row's diff; a card out of step with its base (built
// before another client's choice moved it, or still holding flips that choice made obsolete, their boxes drawn until
// dropped: choice.ts `inStep`) has no row until it is in step.
import type { Sources } from './app.contract.d.ts';

type Call = (path: string, body: unknown, timeout?: number, bound?: string) => Promise<{ error: string; reply: { ok?: any; error?: any }; used: string }>;
type Card = { proposal: string; diff: string; choiceGen: number };

export function chosenDiffSources(call: Call, cards: () => Card[], chosen: (proposal: string, picked: string) => number[], opsOf: (proposal: string) => unknown[], bound: () => string | undefined, files: () => Record<string, string>, inStep: (proposal: string, picked: string, shownGen: number) => boolean): Pick<Sources, 'loadChosenDiffs'> {
  const planned = new Map<string, { diff: string; note: string }>();
  let answers = 0;
  return {
    loadChosenDiffs: async ([client, picked]) => {
      const shown = cards();
      // (a card gone, its plans with it)
      for (const key of planned.keys()) if (!shown.some(h => key.startsWith(`${h.proposal} `))) planned.delete(key);
      const digests = (f: Record<string, string>) => JSON.stringify(Object.keys(f).sort().map(k => [k, f[k]]));
      const rows = await Promise.all(shown.map(async h => {
        const flips = picked.split(';').filter(f => f.startsWith(`${h.proposal}:`)), now = files();
        if (!inStep(h.proposal, picked, h.choiceGen)) return null;
        const ops = opsOf(h.proposal), items = chosen(h.proposal, picked), key = `${h.proposal} ${items.join(',')} ${digests(now)}`;
        const shownAs = { proposal: h.proposal, shown: `${h.choiceGen}|${flips.join(';')}` };
        if (items.length === ops.length) return { ...shownAs, diff: h.diff, note: '' };
        if (!items.length) return { ...shownAs, diff: '', note: 'Nothing is chosen, so accepting it would write nothing.' };
        let got = planned.get(key);
        if (!got) {
          const { error, reply } = await call('/__tamber', { method: 'propose', client, ops: items.map(i => ops[i]) }, 15000, bound());
          got = error ? { diff: h.diff, note: `The diff of only what is chosen could not be planned (${error}): this is the diff of all of it.` } : { diff: String(reply.ok?.diff ?? ''), note: '' };
          // (kept only when it was planned on exactly the files the changes know: a write in between plans another)
          if (!error && digests(reply.ok?.files ?? {}) === digests(now)) planned.set(key, got);
        }
        return { ...shownAs, ...got };
      }));
      return { ok: true, answer: ++answers, diffs: rows.filter(r => r !== null) };
    },
  };
}
