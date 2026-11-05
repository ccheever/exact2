import { expect, test } from 'bun:test';
import { appText } from '../tests/support/app-files';

test('standalone Files search uses the native search field contract', () => {
  const route = appText('mobile/route-content.contract');
  const start = route.indexOf('if not fileBrowserHeader');
  expect(start).toBeGreaterThanOrEqual(0);
  const end = route.indexOf('\n', start);
  expect(route.slice(start, end + 220)).toContain('input type="search" value=filesQuery input=editFilesQuery');
  expect(route.slice(start, end + 320)).toContain('autocapitalize="none" autocomplete="off" spellcheck=false');
});
