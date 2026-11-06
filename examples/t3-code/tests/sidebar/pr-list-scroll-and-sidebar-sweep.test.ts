// T3 Code 1e2ecbd975 (MIT; see LICENSE-T3).
// The sidebar's row-action sweep as Sidebar.tsx startActionSweep and Sidebar.pointer.ts
// SidebarPointerSensor run it: past 6 px the list takes `**:pointer-events-none`, so the row's card or a control's
// tooltip goes as the sweep takes over; Escape cancels it; a cancelled release applies nothing (row-keys-and-width.test.ts holds
// the client's half); the rows then take the pointer again, so the row under it is the hovered one. The host keeps the
// pressed row hovered through a pan and hovers nothing new at the release until the pointer moves, so the section keeps
// that row as its rows' hover until the window's hover count moves on (`sweepRowHover`), and closes the card through the
// root's half of the hover when the sweep arms. An agent drive of the app shows them.
// The Pull Requests list keeps its place across a window refocus; what moved it was the agent's first
// `prefer` re-telling the system appearance (a one-file repro showed it), which remounts the per-scheme subtrees. The
// guard below keeps the list's mount free of the window's focus and visibility.
import { describe, expect, test } from 'bun:test';
import { contractFn } from '../support/contract-fn-fixture';

describe('The rows a sweep passes over, and after it (sweepRowHover)', () => {
  test('none while live; a cancelled sweep\'s resting row until the window\'s hover moves on; else the window\'s', async () => {
    const hover = await contractFn('sidebar.contract', 'sweepRowHover', ['live', 'rest', 'restSeq', 'seq', 'hoverId']);
    // Live: the list takes no pointer, so no row is hovered, the pressed one included.
    expect(hover(true, '', 0, 7, 'sweep-4~active#settle')).toBe('');
    // Cancelled with Escape over Sweep three: Sweep three, through the release, while the host's stale hover is the pressed row.
    expect(hover(false, 'sweep-3~active', 7, 7, '')).toBe('sweep-3~active');
    expect(hover(false, 'sweep-3~active', 7, 7, 'sweep-4~active#settle')).toBe('sweep-3~active');
    // The pointer moved and the window hovered something: its hover again.
    expect(hover(false, 'sweep-3~active', 7, 8, 'sweep-3~active')).toBe('sweep-3~active');
    expect(hover(false, 'sweep-3~active', 7, 9, '')).toBe('');
    // No sweep: the window's hover.
    expect(hover(false, '', 0, 3, 'sweep-2~active')).toBe('sweep-2~active');
  });
});
