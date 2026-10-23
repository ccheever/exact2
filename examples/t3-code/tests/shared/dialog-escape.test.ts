// Escape over stacked popups. The Mac host gives a key to the first `aria-keyshortcuts` button it finds by node id
// (host/apple ShortcutsMac.swift `nodes()`), so a popup mounted earlier takes Escape from a dialog mounted over it unless
// it gives up its shortcut while that dialog shows. Base UI closes only the topmost popup. Read from the Contract
// sources; the keys are driven on macOS.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';

// OpenInPicker's Menu (panel form, MenuPopup align="end" anchored at the row, w-(--anchor-width)): a press anywhere
// outside it or Escape closes it, as Base UI's Menu does; the host light-dismisses a `popover="auto"` (exact2 PR #676).
describe('the "Open in" menu is a popover', () => {
  test("the chevron's tint and expanded state follow the popover's toggle", () => {
    const row = contractComponent('shell-details.contract', 'OpenInRow');
    const state = row.state();
    expect(state.open).toBe(false);
    row.call('toggled', state, {}, [{ newState: 'open', oldState: 'closed' }]);
    expect(state.open).toBe(true);
    row.call('toggled', state, {}, [{ newState: 'closed', oldState: 'open' }]);
    expect(state.open).toBe(false);
  });
});
