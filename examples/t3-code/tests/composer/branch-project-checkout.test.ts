// BranchToolbar.logic.test.ts at T3 Code v0.1.7 (#29908), by its cases: a ref checked out at the project's own checkout
// keeps the thread in the project folder, a subfolder project's included; another checkout is reused as a worktree.
import { expect, test } from 'bun:test';
import { isProjectCheckout } from '../../src/composer/composer-controls-branch';

test('picking a branch in a subfolder project keeps the thread in the project folder', () => {
  expect(isProjectCheckout('/repo/nested-checkout/packages/app', '/repo/nested-checkout', '/repo')).toBe(false);
  // A subdirectory project's own checkout is reported at the repository root; the root is undefined until its status loads.
  for (const [cwd, root] of [['/repo', '/repo'], ['/repo/packages/app', '/repo'], ['/repo/packages/app', undefined]] as const) {
    expect(isProjectCheckout(cwd, root, '/repo')).toBe(true);
  }
  expect(isProjectCheckout('/repo', undefined, '/repo/.t3/worktrees/feature-a')).toBe(false);
});
