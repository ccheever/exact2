// pr-handoffs-and-quick-actions: a folded pull request header does not carry over to another pull request.
// The fold (chromeStateByTab) is PrdBody's own state, so it lasts as long as the PrdBody instance does. The
// reference keys PullRequestDetailPanel per pull request (`key={renderedPullRequestSurface.id}` on the page,
// `key={host:repository#number}` beside a thread), so every PrdBody call site must sit in a block keyed by
// `detail.ref`. Read from the Contract sources as context-menu-hookup.test.ts and dialog-focus.test.ts do; the
// behavior was seen in the real-input session (#115 folded, then #144 opened folded) before 115fb3a5c.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';

const dir = new URL('./', import.meta.url);
const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const indent = (line: string) => line.length - line.trimStart().length;

/** The lines of `component name` in `file`, up to the next top-level declaration. */
async function component(file: string, name: string): Promise<string[]> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end);
}

describe('the header fold belongs to one pull request', () => {
  test('PrdBody keeps each tab its own fold as component state', async () => {
    const body = await component('pages-pr-detail.contract', 'PrdBody');
    expect(body.some(line => /^\s*state foldSummary = false$/.test(line))).toBe(true);
    expect(body.some(line => /^\s*state foldTimeline = false$/.test(line))).toBe(true);
  });

  test('every PrdBody is keyed by the pull request, so another one starts with its header open', async () => {
    const files = readdirSync(dir).filter(name => name.endsWith('.contract'));
    const sites: string[] = [];
    const unkeyed: string[] = [];
    for (const file of files) {
      const lines = (await source(file)).split('\n');
      lines.forEach((line, index) => {
        if (!/^\s*PrdBody\(/.test(line)) return;
        sites.push(`${file}:${index + 1}`);
        // The nearest enclosing line (less indented, not a comment) must be `each <k> in [detail.ref] key=<k>`.
        let parent = index - 1;
        while (parent >= 0 && (lines[parent].trim() === '' || lines[parent].trim().startsWith('//') || indent(lines[parent]) >= indent(line))) parent--;
        const keyed = parent >= 0 && /^\s*each (\w+) in \[detail\.ref\] key=(\w+)$/.exec(lines[parent]);
        if (!keyed || keyed[1] !== keyed[2]) unkeyed.push(`${file}:${index + 1}`);
      });
    }
    // The pull request page (pages-pr-detail.contract) and the thread's pull request surface (r5-panels.contract).
    expect(sites.length).toBe(2);
    expect(unkeyed).toEqual([]);
  });
});
