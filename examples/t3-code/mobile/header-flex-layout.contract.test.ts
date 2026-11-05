import { expect, test } from 'bun:test';
import { appText } from '../tests/support/app-files';

test('mobile headers declare the flex layout their alignment uses', () => {
  const route = appText('mobile/route-content.contract');
  const root = appText('mobile/root-header.contract');
  const newTaskHeader = route.slice(route.indexOf('if routeName == "newTask"'), route.indexOf('else if routeName == "threadModel"'));
  expect(newTaskHeader).toContain('header height=44 flex-shrink=0 display="flex" align-items="center" justify-content="space-between"');
  expect(root).toContain('header height=44 flex-shrink=0 display="flex" align-items="center" justify-content="space-between"');
  expect(root).toContain('view display="flex" flex-direction="column" align-items="center" min-width=0');
});
