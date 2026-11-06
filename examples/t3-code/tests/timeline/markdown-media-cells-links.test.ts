// Images in a table cell and inside a link's text (ChatMarkdown.tsx, T3 Code 1e2ecbd975): ChatMarkdownImage draws
// both; markdownImageGallery collects a cell's image; an image inside an `<a>` is the link's to press.
import { describe, expect, test } from 'bun:test';
import { contractFns } from '../support/contract-fn-fixture';

const fns = contractFns('markdown.contract');

const run = (id: string, kind: string, href: string, link = '') => ({ id, text: id, kind, href, link, label: '', icon: '', weight: 400, slant: 'normal', mono: false });
const chip = (href: string, icon = 'environment') => ({ id: href, href, kind: 'media', size: 'image', icon, label: '', tip: '', detail: '', target: href, owner: 'm' });

describe('the gallery\'s occurrences (markdownGalleryIds)', () => {
  test('a table cell\'s images join in row and cell order; an image inside a link and a blocked one do not', () => {
    const chips = [chip('t3-image:a'), chip('t3-image:b'), chip('t3-image:c'), chip('t3-image:linked'), chip('t3-image:bad', 'blocked'), chip('t3-file:block.png')];
    const blocks = [
      { id: 'b0', kind: 'image', href: 't3-file:block.png', flow: false, runs: [], rows: [] },
      { id: 'b1', kind: 'paragraph', flow: true, rows: [], runs: [run('r0', '', ''), run('r1', 'media', 't3-image:a'), run('r2', 'media', 't3-image:linked', 'https://a.com')] },
      { id: 'b2', kind: 'table', flow: false, runs: [], rows: [
        { id: '0', header: true, cells: [{ id: '0', runs: [run('r0', '', '')] }, { id: '1', runs: [run('r0', 'media', 't3-image:bad')] }] },
        { id: '1', header: false, cells: [{ id: '0', runs: [run('r0', 'media', 't3-image:b'), run('r1', 'media', 't3-image:linked', 'https://a.com')] }, { id: '1', runs: [run('r0', 'media', 't3-image:c')] }] },
      ] },
    ];
    expect(fns.markdownGalleryIds!(blocks as never, chips as never)).toEqual(['b0', 'b1-r1', 'b2-1-0-r0', 'b2-1-1-r0']);
  });
  test('the favicon leads an image only where it starts its link', () => {
    const runs = [run('r0', 'link-start', 'https://a.com'), run('r1', 'media', 't3-image:x', 'https://a.com'), run('r2', 'media', 't3-image:y', 'https://b.com')];
    expect(fns.linkLeads!(runs as never, 'r1' as never, 'https://a.com' as never)).toBe(false);
    expect(fns.linkLeads!(runs as never, 'r2' as never, 'https://b.com' as never)).toBe(true);
    expect(fns.linkLeads!([runs[1]] as never, 'r1' as never, 'https://a.com' as never)).toBe(true);
  });
});
