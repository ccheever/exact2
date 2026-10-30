// ChatMarkdown's selectInlineCode at T3 Code v0.1.5 (#26639): a double-click on an inline code span selects all of it.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';

describe('a double-click on inline code', () => {
  const flow = contractComponent('markdown.contract', 'FlowRuns');
  const run = (source: string) => {
    const calls: unknown[][] = [];
    flow.call('selectCode', flow.state(), { source, setSelectionRange: (...args: unknown[]) => calls.push(['select', ...args]), citeHold: () => calls.push(['hold']) },
      ['cite-m1-0-3', 'examples/t3-code'.length]);
    return calls;
  };
  test('in an answer selects the whole span and starts Cite\'s wait, as the answer\'s own double-click does', () => {
    expect(run('m1')).toEqual([['select', 'cite-m1-0-3', 0, 16], ['hold']]);
  });
  test('outside an answer, where spans have no ids of their own, leaves the word selection', () => {
    expect(run('')).toEqual([]);
  });
  test('each prose code span takes the double-click with its own id and length', async () => {
    const source = await Bun.file(appPath('markdown.contract')).text();
    expect(source).toContain('dblclick=selectCode(`cite-${source}-${blockId}-${run.id}`, run.text.length)');
  });
});
