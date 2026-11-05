import { expect, test } from 'bun:test';
import { appText } from '../tests/support/app-files';

test('Git branch and worktree fields preserve machine-name input traits', () => {
  const source = appText('mobile/git-branches.contract');
  for (const placeholder of ['feature/mobile-polish', 'main', 'feature/mobile-thread']) {
    const start = source.indexOf(`placeholder="${placeholder}"`);
    expect(start).toBeGreaterThanOrEqual(0);
    const lineStart = source.lastIndexOf('\n', start) + 1;
    const lineEnd = source.indexOf('\n', start);
    const line = source.slice(lineStart, lineEnd < 0 ? source.length : lineEnd);
    expect(line).toContain('autocapitalize="none" autocomplete="off" spellcheck=false');
  }
});
