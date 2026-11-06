// Removing an image that is also an inline chip asks first (removeComposerImage); a Remove that held the focus gets it
// back on Cancel, and Confirm moves it to where the removal leaves it (the next tile or the prompt). The confirm moves
// the focus before its dialog goes, as Base UI's finalFocus does while the dialog's exit transition keeps it mounted
// (reference 1e2ecbd975 dialog-styles.ts, duration-200), so an assistive press on Cancel or Confirm (AXPress, VO-Space)
// never leaves the focus on a destroyed button. Run with the Contract action semantics of contract-action-fixture.ts.
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';


describe('a removal asked from a focused Remove gives the focus away before its dialog goes', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const scopeFor = (calls: string[]) => ({
    focus: (id: string) => calls.push(`focus:${id}`),
    confirmClose: (accept: boolean) => calls.push(`close:${accept}`),
  });
  const asked = (): Env => ({ confirmOp: 'remove-snapshot', confirmTarget: 'img1', confirmValue: 'preview-image-img2', confirmHanding: '' });

  test('Cancel focuses that Remove, Confirm where the removal leaves the focus; the dialog closes 200 ms later', async () => {
    for (const [accept, target] of [[false, 'remove-snapshot-img1'], [true, 'preview-image-img2']] as const) {
      const calls: string[] = [], state = asked();
      root.call('confirmUi', state, scopeFor(calls), [accept]);
      expect(calls).toEqual([`focus:${target}`]);
      expect(state.confirmHanding).toBe(accept ? 'accept' : 'cancel');
      // A second press while the dialog leaves does nothing.
      root.call('confirmUi', state, scopeFor(calls), [!accept]);
      expect(calls).toHaveLength(1);
      root.call('confirmHanded', state, scopeFor(calls), []);
      expect(calls).toEqual([`focus:${target}`, `close:${accept}`]);
      expect(state.confirmHanding).toBe('');
    }
    const app = await appText('app.contract');
    expect(app).toContain('  task confirmHand when confirmHanding != "" key=confirmHanding\n    after(200, confirmHanded)\n');
  });

  test('a Remove pressed without the focus, and every other confirm, closes at once', () => {
    for (const state of [{ ...asked(), confirmValue: '' }, { ...asked(), confirmOp: 'shell:delete' }]) {
      const calls: string[] = [];
      root.call('confirmUi', state, scopeFor(calls), [false]);
      expect(calls).toEqual(['close:false']);
      expect(state.confirmHanding).toBe('');
    }
  });
});
