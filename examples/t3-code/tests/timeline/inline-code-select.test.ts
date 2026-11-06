// ChatMarkdown's selectInlineCode at T3 Code v0.1.5 (#26639): a double-click on an inline code span selects all of it, in
// answers, user messages, plans, question cards and table cells alike.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';

describe('a double-click on inline code', () => {
  const run = (component: string, source: string) => {
    const flow = contractComponent('markdown.contract', component);
    const calls: unknown[][] = [];
    flow.call('selectCode', flow.state(), { source, setSelectionRange: (...args: unknown[]) => calls.push(['select', ...args]), citeHold: () => calls.push(['hold']) },
      ['code-m1-0-3', 'examples/t3-code'.length]);
    return calls;
  };
  test('in an answer selects the whole span and starts Cite\'s wait, as the answer\'s own double-click does', () => {
    expect(run('FlowRuns', 'm1')).toEqual([['select', 'code-m1-0-3', 0, 16], ['hold']]);
    expect(run('CellRuns', 'm1')).toEqual([['select', 'code-m1-0-3', 0, 16], ['hold']]);
  });
  test('outside an answer selects the whole span with no Cite', () => {
    expect(run('FlowRuns', '')).toEqual([['select', 'code-m1-0-3', 0, 16]]);
    expect(run('CellRuns', '')).toEqual([['select', 'code-m1-0-3', 0, 16]]);
  });
  test('each code span takes the double-click with an id that names its document', async () => {
    const source = await Bun.file(appPath('markdown.contract')).text();
    expect(source).toContain('fn codeNode(source: string, scope: string, blockId: string, runId: string): string = source != "" ? `cite-${source}-${blockId}-${runId}` : `code-${scope}-${blockId}-${runId}`');
    expect(source).toContain('dblclick=selectCode(codeNode(source, scope, blockId, run.id), run.text.length)');
    expect(source).toContain('dblclick=selectCode(`code-${scope}-${nodePrefix}-${run.id}`, run.text.length)');
    expect(source).toMatch(/FlowRuns\(runs=block\.runs,[^\n]*scope=scope,/);
    expect(source).toContain('MarkdownTable(block=block, scope=scope, source=source,');
  });
});
