// A reply's changed-files card is on the Tab path, as the reference's buttons are with Keyboard Navigation off: ⇧Tab from
// the composer stops on its rows, Open diff and Expand all folders (the reference's order, measured with CDP).
import { expect, test } from 'bun:test';
import { appText } from '../support/app-files';

const source = appText('src/timeline/timeline.contract').split('\n');
const button = (marker: string) => source.find(line => line.includes('button ') && line.includes(marker)) ?? '';

test("the card's buttons are Tab stops", () => {
  for (const marker of ['testId=`tree-dir-', 'testId=`tree-file-', 'aria-label="Open diff"', 'aria-label=label tabindex', 'disabled=disabled']) {
    expect(button(marker)).toContain('tabindex=0');
  }
});
