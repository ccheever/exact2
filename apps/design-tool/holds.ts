// Held states in the inspector (LLP 1122 D8, T4): a host-known state held
// on the selected node so its look shows without input (`hold`, LLP 1121
// stage 8 E), one hold a build, a new one replacing the last; what the
// server and the build said of it; and the rows the held state shows.

/** A build's hold as its listing and the events give it: whether it can
 * hold states, the hold the server keeps for it, and the build's report on
 * that hold (null until it reports: the server forgets a report when it
 * sends a hold, so a listing's is its hold's). */
export type Holding = { can: boolean; hold: { site: number; instance: unknown[]; state: string } | null; held: { state: string | null; reason?: string } | null };

const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);

/** A listing's entry, over what the tool knew. */
export function holdingOf(b: any, was?: Holding): Holding {
  return {
    can: Array.isArray(b?.capabilities) ? b.capabilities.includes('hold') : !!was?.can,
    hold: 'hold' in b ? b.hold ?? null : was?.hold ?? null,
    held: 'held' in b ? b.held ?? null : was?.held ?? null,
  };
}

/** The last word on a build's hold, as a listing moves it from `was`: a new
 * hold (anyone's) ends what was said of the last; one the build let go of
 * itself says why, though its event was missed (a resync). */
export function holdNote(h: Holding, was: Holding | undefined, note: string) {
  if (h.hold && !same(h.hold, was?.hold)) return '';
  // (A report the server forgets at each hold frame: one with a reason not seen before is a let-go since.)
  if (!h.hold && was && h.held?.reason && (was.hold || !same(h.held, was.held))) return letGo(h.held.reason);
  return note;
}

/** A `held` report or the server's `hold-cleared`, over the build's hold. */
export const heldEvent = (h: Holding, e: any): Holding =>
  e.event === 'hold-cleared' ? { ...h, hold: null, held: { state: null, reason: e.reason } }
  : { ...h, held: { state: e.state ?? null, reason: e.reason }, ...(e.state ? {} : { hold: null }) };

// A let-go's reason in words (the build channel's `held` reasons, the
// server's `hold-cleared` ones).
const REASONS: Record<string, string> = {
  input: 'real input on the node let it go',
  disabled: 'the node is disabled, which refuses a hold',
  gone: 'no single view there (for a press, no press there), so it was let go',
  replaced: 'another hold replaced it',
  client: 'its client went',
};
export const reasonText = (reason: string) => REASONS[reason] ?? reason;
export const letGo = (reason: string, detail?: string) => `The hold was let go: ${reasonText(reason)}${detail ? ` (${detail})` : ''}.`;

const STATES: Record<string, string> = { active: 'pressed', hover: 'hovered', 'focus-visible': 'focused' };

/** What the inspector says of holds on the selected copy's build: whether
 * it can hold, what is held here and whether the build says so, and why
 * the last one went. A press holds on the node the press resolves to: the
 * build names it (stage 9 E's `held.site`) when it is another node. */
export function holdView(e: any, h: Holding, note: string, stale: boolean) {
  const state = h.hold?.state ?? '';
  // The selection's own report of this hold (its state), naming the node held.
  const report = h.hold && e?.held?.state === state ? e.held : null;
  // Here: held on this copy, or on this node as the press it resolves to.
  const here = !!h.hold && ((h.hold.site === Number(e?.site) && same(h.hold.instance ?? [], e?.instance ?? [])) || (Number.isInteger(report?.site) && report.site === Number(e?.site)));
  const held = !!h.hold && h.held?.state === state;
  const word = STATES[state] ?? state;
  const owner = Number.isInteger(report?.site) && report.site !== Number(e?.site) ? ` on the node its press resolves to, #${report.node ?? report.site}, above this one` : '';
  return {
    can: h.can && !stale,
    state: here ? state : '',
    held: here && held,
    status: !h.can ? (stale ? note : 'This build cannot hold states.')
      : h.hold && !here ? `${held ? 'Held' : 'Holding'} ${word} on another node of this build: a hold here replaces it.`
      : h.hold ? `${held ? 'Held' : 'Holding'} ${word}${owner}.`
      : note,
  };
}
