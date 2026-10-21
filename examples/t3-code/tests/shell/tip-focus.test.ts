// ui/tooltip.tsx (Base UI Tooltip): a tooltip opens on its trigger's keyboard focus as on its hover, so a control that
// cannot act (the pull request's reviewer and label pickers, aria-disabled) says why to a keyboard user; focus a pointer
// press gave does not open it. shell-tip.contract Tip.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractComponent, value } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';

const tip = contractComponent('shell-tip.contract', 'Tip');
/** The bubble's `shown` as the Tip's view computes it, over the Tip's state. */
const shownExpr = /component Tip\n[\s\S]*?TipBubble\([^\n]*shown=(\(.*\))\)\n/.exec(readFileSync(appPath('shell-tip.contract'), 'utf8'))![1]!;
const shown = (state: Record<string, unknown>) => value(shownExpr, state) as boolean;

describe('a tooltip and its trigger\'s focus', () => {
  test('keyboard focus shows it and leaving hides it; a press\'s focus does not, until focus leaves', () => {
    const state = tip.state();
    expect(shown(state)).toBe(false);
    tip.call('focusEntered', state, {}, [{}]);
    expect(shown(state)).toBe(true);
    tip.call('focusLeft', state, {}, [{}]);
    expect(shown(state)).toBe(false);
    tip.call('pressDown', state, {}, [{}]);
    tip.call('focusEntered', state, {}, [{}]);
    expect(shown(state)).toBe(false);
    tip.call('focusLeft', state, {}, [{}]);
    tip.call('focusEntered', state, {}, [{}]);
    expect(shown(state)).toBe(true);
    tip.call('setOver', state, {}, [true]);
    tip.call('focusLeft', state, {}, [{}]);
    expect(shown(state)).toBe(true);
  });
  test('an unavailable picker keeps its keyboard stop (aria-disabled), so its focus can show the reason', () => {
    const picker = contractComponent('pages-pr-meta.contract', 'PrdCandidatePicker');
    expect(picker.drawn('aria-disabled=true aria-label=label', {}, { picker: { allowed: false, rows: [], selected: [] }, highlight: 0, query: '' })).toBe(true);
    expect(picker.drawn('aria-disabled=true aria-label=label', {}, { picker: { allowed: true, rows: [], selected: [] }, highlight: 0, query: '' })).toBe(false);
  });
});
