// The hosted thread link Copy thread writes (T3 Code v0.1.7 #26290): threadLinks.test.ts "hosted thread links",
// composerContextReferences.test.ts threadLinkContextReferences and composerInlineTokenPaste.test.ts, by name.
import { describe, expect, test } from 'bun:test';
import { formatHostedThreadLink, hostedThreadLink, parseThreadLinkHref, pastedThreadLinkChips, relabelThreadLinks, threadLinksIn } from '../../src/timeline/thread-links';
import { threadChipCopyTarget } from '../../src/sidebar/thread-reference';
import { contextId, contextLink } from '../../src/composer/composer-editor-menu';
import { promptLinkAppPath, promptLinkDeepLink } from '../../src/prompt-links/prompt-links';

const ref = (environmentId: string, threadId: string) => ({ environmentId, threadId });
const chip = (threadId: string, title: string) => contextLink('thread', contextId('thread', threadId), title);

describe('hosted thread links', () => {
  test('escapes every id character a sentence or path could claim, and reads it back', () => {
    const thread = ref('env.1', 'thread:delegated/mcp%3A1.');
    const link = hostedThreadLink(thread);
    expect(link).toBe('https://app.t3.codes/threads/env%2E1/thread%3Adelegated%2Fmcp%253A1%2E');
    expect(parseThreadLinkHref(link)).toEqual(thread);
    expect(hostedThreadLink(thread, 'https://preview.t3.codes/')).toMatch(/^https:\/\/preview\.t3\.codes\/threads\//);
  });
  test('reads only a bare thread path on the hosted app', () => {
    expect(parseThreadLinkHref('https://app.t3.codes/threads/env-1/thread-1')).toEqual(ref('env-1', 'thread-1'));
    for (const href of ['https://app.t3.codes/threads/env-1/thread-1?prompt=go', 'https://app.t3.codes/threads/env-1/thread-1/extra',
      'https://app.t3.codes/threads/env-1', 'https://app.t3.codes/env-1/thread-1', 'https://evil.test/threads/env-1/thread-1']) expect(parseThreadLinkHref(href)).toBeNull();
  });
  test('writes a copied link a reader can follow, labeled when it has a title', () => {
    expect(formatHostedThreadLink(ref('e', 't1'), 'Fix [ci]')).toBe('[Fix ci](https://app.t3.codes/threads/e/t1)');
    expect(formatHostedThreadLink(ref('e', 't1'), ' ')).toBe('https://app.t3.codes/threads/e/t1');
  });
  test('titles hosted links in every Markdown form and writes them as v2 links', () => {
    const markdown = ['See https://app.t3.codes/threads/here/t1.', '[old](https://app.t3.codes/threads/here/t1)', '<https://app.t3.codes/threads/here/t1>',
      '`https://app.t3.codes/threads/here/t1`', 'https://app.t3.codes/threads/here/t1?prompt=go'].join('\n');
    expect(relabelThreadLinks(markdown, [{ id: 't1', title: 'Here' }], 'here')).toBe(['See [Here](t3-thread://v2/here/t1).', '[Here](t3-thread://v2/here/t1)',
      '[Here](t3-thread://v2/here/t1)', '`https://app.t3.codes/threads/here/t1`', 'https://app.t3.codes/threads/here/t1?prompt=go'].join('\n'));
    // Another machine's link stays a link, as the v2 link it names, for its chip to resolve or explain.
    expect(relabelThreadLinks('https://app.t3.codes/threads/there/t1', [], 'here')).toBe('[t3-thread://v2/there/t1](t3-thread://v2/there/t1)');
    expect(threadLinksIn('Open https://app.t3.codes/threads/there/t%3A1').map(link => parseThreadLinkHref(link.href))).toEqual([ref('there', 't:1')]);
  });
});

describe('opening a copied thread link', () => {
  test('opens a copied thread link in the app at that thread', () => {
    const path = promptLinkAppPath(hostedThreadLink(ref('env-1', 'mcp:1')));
    expect(path).toBe('/env-1/mcp%3A1');
    const deepLink = promptLinkDeepLink(path!);
    expect(deepLink).toBe('t3code://threads/env-1/mcp%3A1');
    expect(promptLinkAppPath(deepLink)).toBe(path);
    expect(promptLinkAppPath('https://app.t3.codes/threads/env-1/thread-1?prompt=go')).toBe('/env-1/thread-1?prompt=go');
    // The web app's own thread route, reloaded, is no link.
    expect(promptLinkAppPath('https://app.t3.codes/env-1/thread-1')).toBeNull();
    expect(promptLinkAppPath('https://app.t3.codes/threads/env-1')).toBeNull();
  });
});

describe('pasting thread links into the composer', () => {
  const threads = [{ id: 'thread-1', title: 'Fix the login flow' }];
  test('pastes a copied thread link back as the chip it was copied from', () => {
    const target = threadChipCopyTarget(ref('env-1', 'thread-1'), 'Fix the login flow');
    expect(target.value).toBe('https://app.t3.codes/threads/env-1/thread-1');
    expect(pastedThreadLinkChips(`See ${target.value}`, threads, 'env-1')).toBe(`See ${chip('thread-1', 'Fix the login flow')}`);
  });
  test('makes a chip of a v2 link, titled here, else by its label', () => {
    expect(pastedThreadLinkChips('[Login](t3-thread://v2/env-1/thread-1)', threads, 'env-1')).toBe(chip('thread-1', 'Fix the login flow'));
    expect(pastedThreadLinkChips('[Login](t3-thread://v2/env-1/gone) <t3-thread://v2/env-1/gone2>', threads, 'env-1'))
      .toBe(`${chip('gone', 'Login')} ${chip('gone2', 'Thread')}`);
  });
  test('keeps links it may not chip, v1 links, and links in code as written', () => {
    const text = ['https://app.t3.codes/threads/elsewhere/thread-1', '[Old](t3-thread://v1/thread-1)', '`https://app.t3.codes/threads/env-1/thread-1`'].join(' ');
    expect(pastedThreadLinkChips(text, threads, 'env-1')).toBe(text);
  });
});
