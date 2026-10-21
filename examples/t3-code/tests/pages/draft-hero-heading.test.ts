// DraftHeroProjectHeading's fit check, the hero's lane, and the "No project" names (T3 Code 74541bab17
// DraftHeroProjectHeading.tsx, DraftHeroHeadline.tsx, ChatHeader.tsx; MIT, see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { heroInlineWidth } from '../../src/shared/laid-out-widths';
import { snapshot } from '../../src/client/presentation';
import { opened } from '../support/composer-controls-fixture';
import { readFileSync } from 'node:fs';

describe('the project heading', () => {
  test('reads the one-line probe\'s laid-out width, 0 before it is laid out', () => {
    expect(heroInlineWidth({ anchors: {} })).toBe(0);
    expect(heroInlineWidth({ anchors: { 'hero-inline': [0, 512.5] } })).toBe(512.5);
  });
  // app-main.contract's EmptyThread arguments, evaluated as written (Contract's `and` is JavaScript's &&).
  const source = readFileSync(new URL('../../src/shell/app-main.contract', import.meta.url), 'utf8');
  const call = source.split('\n').find(line => line.trim().startsWith('EmptyThread('))!;
  const arg = (name: string) => /inline=\((.+?)\), cap=(.+?), command=/.exec(call)![name === 'inline' ? 1 : 2]!.replaceAll(' and ', ' && ');
  const derived = (width: number, chatMax: number, measured: number) => {
    const scope = { rem: 16, viewport: { width }, conversationWidth: width - 256, laneInset: 0, min: Math.min, data: { look: { chatMax }, heroInlineWidth: measured } };
    const run = (name: string) => new Function(...Object.keys(scope), `return (${arg(name)})`)(...Object.values(scope));
    return { heroCap: run('cap') as number, heroInline: run('inline') as boolean };
  };
  test('the lane is the chat width setting\'s, at most 64rem (max-w-(--chat-content-max-width) inside max-w-5xl)', () => {
    expect(derived(1280, 736, 0).heroCap).toBe(736);
    expect(derived(1280, 1152, 0).heroCap).toBe(1024);
    expect(derived(1280, 100000, 0).heroCap).toBe(1024);
  });
  test('one line on an lg window when the probe fits the lane, and before the probe is laid out', () => {
    expect(derived(1280, 736, 0).heroInline).toBe(true);
    expect(derived(1280, 736, 700).heroInline).toBe(true);
    expect(derived(1280, 736, 740).heroInline).toBe(false);
    expect(derived(1000, 736, 300).heroInline).toBe(false); // below lg it always stacks
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
