// Project favicons are asked of each project's own server and cached by server, so another environment's projects
// (the sidebar's background rows, thread chips) draw that server's favicon, never the focused one's.
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { fleet } from '../../src/connections/settings-b-fleet';
import { faviconSrc, projectGlyph, syncProjectFavicons } from '../../src/sidebar/sidebar-glyph';
import { markdownThreadGlyphs } from '../../src/timeline/markdown-thread-glyphs';

const identity = (name: string) => ({ projectMark: name.slice(0, 2).toUpperCase(), projectInk: '#000000', projectSurface: '#ffffff' });
const shared = { id: 'p1', title: 'shared', workspaceRoot: '/srv/shared' };
const client = { environmentId: 'env-a', generation: 3, shell: { projects: [shared], threads: [] } } as unknown as T3Client;
const entry = { key: 'b', origin: 'http://b.example', environmentId: 'env-b', phase: 'connected' as const, message: '', traceId: '', generation: 7, synchronized: 7, lastEvent: 0,
  subscriptions: {}, config: {}, shell: { projects: [{ ...shared, title: 'remote' }], threads: [{ id: 'remote-thread', title: 'Remote', projectId: 'p1' }] } as never, scopes: [], error: '', requested: false };
const answer = (origin: string) => async () => ({ relativeUrl: `/assets/favicon-${origin}.png?sig=1` });
afterEach(() => fleet.entries.delete('b'));

describe('favicons by server', () => {
  test('two servers with the same checkout path keep their own favicons', async () => {
    fleet.entries.set('b', entry);
    await syncProjectFavicons({ environmentId: 'env-a', generation: 3 }, 'http://a.example', client.shell.projects, answer('a'));
    await syncProjectFavicons({ environmentId: 'env-b', generation: 7 }, 'http://b.example', entry.shell.projects, answer('b'));
    expect(faviconSrc(client, shared)).toBe('http://a.example/assets/favicon-a.png?sig=1');
    expect(faviconSrc(client, shared, 'env-b')).toBe('http://b.example/assets/favicon-b.png?sig=1');
    // A sidebar scope member and a background row's qualified id name their server too.
    expect(faviconSrc(client, { ...shared, environmentId: 'env-b' })).toBe('http://b.example/assets/favicon-b.png?sig=1');
    expect(faviconSrc(client, { ...shared, id: 'fleet:env-b:p1' })).toBe('http://b.example/assets/favicon-b.png?sig=1');
    expect(projectGlyph(client, shared, identity, 'env-b')).toMatchObject({ kind: 'image', src: 'http://b.example/assets/favicon-b.png?sig=1' });
    // An environment that is not connected has no favicon to show.
    expect(faviconSrc(client, shared, 'env-gone')).toBe('');
  });
  test("a thread link to another environment's thread leads with that server's favicon", async () => {
    fleet.entries.set('b', entry);
    await syncProjectFavicons({ environmentId: 'env-b', generation: 7 }, 'http://b.example', entry.shell.projects, answer('b'));
    const [glyph] = markdownThreadGlyphs(client, 'See [x](t3-thread://v1/remote-thread)');
    expect(glyph).toMatchObject({ title: 'remote', glyph: { kind: 'image', src: 'http://b.example/assets/favicon-b.png?sig=1' } });
  });
});
