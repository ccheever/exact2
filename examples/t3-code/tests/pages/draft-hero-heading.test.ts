// DraftHeroProjectHeading's fit check and the "No project" names (T3 Code 74541bab17 DraftHeroProjectHeading.tsx,
// ChatHeader.tsx; MIT, see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { heroInline } from '../../src/shared/laid-out-widths';
import { snapshot } from '../../src/client/presentation';
import { opened } from '../support/composer-controls-fixture';

describe('the project heading', () => {
  test('sits on one line only when its laid-out probe fits the heading', () => {
    expect(heroInline({ anchors: {} })).toBe(false);
    expect(heroInline({ anchors: { 'hero-inline': [0, 512] } })).toBe(false);
    expect(heroInline({ anchors: { 'hero-inline': [0, 512], 'hero-heading': [0, 896] } })).toBe(true);
    expect(heroInline({ anchors: { 'hero-inline': [0, 896], 'hero-heading': [0, 896] } })).toBe(true);
    expect(heroInline({ anchors: { 'hero-inline': [0, 1210.5], 'hero-heading': [0, 896] } })).toBe(false);
  });
});

describe('a scratch thread', () => {
  test('names "No project" in its header crumb, and its project otherwise', async () => {
    const { client } = await opened();
    expect(snapshot(client).projectName).toBe('Example');
    client.config.scratchWorkspaceRoot = '/repo';
    expect(snapshot(client).projectName).toBe('No project');
  });
});
