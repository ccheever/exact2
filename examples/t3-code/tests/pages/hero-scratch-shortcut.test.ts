// DraftHeroHeadline at 7c3b3b3605ef (MIT, see LICENSE-T3): "or start without a project" shows chat.newWithoutProject's
// shortcut in its tip (noProjectShortcut), and no tip without one.
import { expect, test } from 'bun:test';
import { pagesHome } from '../../src/pages/pages-home';
import { opened } from '../support/composer-controls-fixture';

test('the hero names the no-project shortcut, and nothing without one', async () => {
  const { client } = await opened();
  client.config = { ...client.config, keybindings: [{ command: 'chat.newWithoutProject', shortcut: { key: 'n', modKey: true, altKey: true } }] };
  expect((await pagesHome(client, null, false)).scratchShortcut).toBe('⌥⌘N');
  client.config = { ...client.config, keybindings: [] };
  expect((await pagesHome(client, null, false)).scratchShortcut).toBe('');
});
