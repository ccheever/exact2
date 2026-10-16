// A Git options menu item gives the focus to the menu's trigger as it closes (Base UI Menu's returnFocus), so the Commit or
// Publish dialog it opens records the chevron as the focus before it and gives the focus back there on Escape. Left to the
// popover's own restore, the focus went to what held it before the menu opened: the prompt, or nothing after a pointer press.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const contract = readFileSync(new URL('./r4-git.contract', import.meta.url), 'utf8');
const row = contract.slice(contract.indexOf('component R4GitMenuRow'), contract.indexOf('\n// ', contract.indexOf('component R4GitMenuRow')));

describe('git-menu-final-focus', () => {
  test('a chosen item focuses the chevron, then runs its action', () => {
    expect(row).toContain('  action choose()\n    focus("details-git-menu")\n    act("shell:git-menu", cwd, item.id)');
    expect(row).toContain('press=choose popovertarget="details-git-options" popovertargetaction="hide"');
    expect(contract).toContain('button id="details-git-menu" popovertarget="details-git-options"');
  });
});
