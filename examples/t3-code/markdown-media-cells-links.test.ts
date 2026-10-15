// Images in a table cell and inside a link's text (ChatMarkdown.tsx, T3 Code 1e2ecbd975): ChatMarkdownImage draws
// both; markdownImageGallery collects a cell's image; an image inside an `<a>` is the link's to press.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractFns } from './contract-fn-fixture';

const fns = contractFns('markdown.contract');
const source = (file: string) => readFileSync(new URL(`./${file}`, import.meta.url), 'utf8');
function component(file: string, name: string): string {
  const text = source(file), start = text.indexOf(`component ${name}\n`);
  if (start < 0) throw new Error(`no component ${name} in ${file}`);
  const rest = text.slice(start + 1), end = rest.search(/\n(component|shape|fn|keyframes|use) /);
  return text.slice(start, end < 0 ? undefined : start + 1 + end);
}

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

describe('the views', () => {
  test('a table cell with an image draws its runs one by one, the image through its media chip with the gallery\'s node id', () => {
    expect(source('markdown.contract')).toContain('fn cellBoxes(runs: ChatRun[]): bool = runs.filter((part) => part.mono or part.kind == "file" or part.kind == "media" or');
    const cell = component('markdown.contract', 'TableCell');
    expect(cell).toContain('CellRuns(runs=cell.runs');
    expect(cell).toContain('chips=chips, urls=urls, local=local, nodePrefix=nodePrefix)');
    expect(cell).toContain('CellSizer(runs=column.sizer');
    const runs = component('markdown.contract', 'CellRuns');
    expect(runs).toContain('standalone=false, link=run.link');
    expect(runs).toContain('nodeId=`media-${chip.owner}-${nodePrefix}-${run.id}`');
    expect(component('markdown.contract', 'MarkdownTable')).toContain('nodePrefix=`${block.id}-${tableRow.id}-${cell.id}`');
    // The sizer holds the picture itself, so the column is as wide as the image (up to the cell's 24rem).
    expect(component('markdown.contract', 'CellSizer')).toContain('image entry.url aria-hidden=true flex-shrink=0 max-width="22.5rem"');
  });
  test('an image inside a link sits in the link, whose press opens it; its right click is the media menu; no preview button', () => {
    const body = component('media-markdown.contract', 'MarkdownMediaBody');
    const linked = body.slice(body.indexOf('and link != ""'), body.indexOf('and link == ""'));
    expect(linked).toContain('link href=link press=linkOpen("link", link) contextmenu=linkedMenu aria-label=linkLabel');
    expect(linked).toContain('image url aria-label=alt');
    expect(linked).not.toContain('MediaPreviewActions');
    expect(body.slice(body.indexOf('and link == ""'))).toContain('MediaPreviewActions(');
    const flow = component('markdown.contract', 'FlowRuns');
    expect(flow).toContain('link=run.link, linkLabel=(run.label != "" ? run.label : run.text), favicon=(webLink(run.link) and linkLeads(runs, run.id, run.link))');
  });
});
