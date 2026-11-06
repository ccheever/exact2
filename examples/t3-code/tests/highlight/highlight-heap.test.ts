// The highlight caches in Hermes's heap. The TypeScript module runs in Hermes with a 64 MiB heap (js/src/lib.rs
// DEFAULT_MAX_HEAP), and an allocation past it aborts the app. Each scenario runs the real cache modules there,
// bundled, in a heap of half that size (the rest is the app's), with the lexers' answers given from records lexed
// here. It needs the Hermes command line, at ../ibex/tools/hermes-vanilla/hermes beside the checkout or named by
// EXACT_HERMES_CLI, and is skipped without it.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { lineRecords } from '../support/highlights-fixture';
import { APP } from '../support/app-files';

const HERMES = process.env.EXACT_HERMES_CLI ?? join(APP, '..', '..', '..', 'ibex', 'tools', 'hermes-vanilla', 'hermes');
const HEAP = '32M';

/** The scenario's output under Hermes, after the cache modules bundled and `data` defined. */
async function underHermes(data: Record<string, unknown>, scenario: string): Promise<{ status: number | null; out: string }> {
  const dir = mkdtempSync(join(tmpdir(), 'highlight-heap-'));
  writeFileSync(join(dir, 'entry.ts'), `import { highlightAsk, highlightStore, resetHighlights } from ${JSON.stringify(join(APP, 'src/highlight/highlights'))};
import { messageCodeBlocks } from ${JSON.stringify(join(APP, 'src/highlight/timeline-highlight'))};
(globalThis as any).H = { highlightAsk, highlightStore, resetHighlights, messageCodeBlocks };`);
  const built = await Bun.build({ entrypoints: [join(dir, 'entry.ts')], target: 'browser', format: 'iife' });
  const bundle = await built.outputs[0]!.text();
  const script = `${bundle}
${Object.entries(data).map(([name, value]) => `var ${name} = ${JSON.stringify(value)};`).join('\n')}
${DRIVER}
${scenario}`;
  writeFileSync(join(dir, 'run.js'), script);
  const run = spawnSync(HERMES, [`-gc-max-heap=${HEAP}`, join(dir, 'run.js')], { encoding: 'utf8', timeout: 240_000 });
  return { status: run.status, out: `${run.stdout}${run.stderr}` };
}

/** highlights.rs answering from a text's whole records (`recordsOf`): a whole text in full, a document from its first
 *  line not yet final; then the pump's rounds until nothing waits, and the heap's peak. */
const DRIVER = `
var docs = {}, peak = 0;
function answer(requests, recordsOf) {
  return requests.map(function (request) {
    if (!request.doc) return { id: request.id, line: 0, records: recordsOf(request.text) };
    var doc = docs[request.doc];
    if (request.from === 0) docs[request.doc] = doc = { text: request.text, delivered: 0 };
    else if (!doc || doc.text.length !== request.from) return { id: request.id, line: -2, records: [] };
    else doc.text += request.text;
    var all = recordsOf(doc.text), out = [], breaks = 0, base = 0;
    for (var at = 0; at + 1 < all.length; at += 2) {
      if (breaks >= doc.delivered) out.push(all[at] - base, all[at + 1]);
      if (all[at + 1] === 0xffffffff && ++breaks === doc.delivered) base = all[at];
    }
    var line = doc.delivered;
    doc.delivered = breaks;
    return { id: request.id, line: line, records: out };
  });
}
function settle(recordsOf) {
  for (var round = 0; round < 10000; round++) {
    var ask = H.highlightAsk();
    if (!ask.requests.length) return;
    H.highlightStore(answer(ask.requests, recordsOf));
  }
}
function measure() { gc(); peak = Math.max(peak, HermesInternal.getInstrumentedStats().js_allocatedBytes); }
/** The records of a prefix of the text whose records are \`all\`: those within it, the token it cuts ending at its end. */
function prefixRecords(all, length) {
  var out = [];
  for (var at = 0; at + 1 < all.length; at += 2) {
    if (all[at] > length) { if (all[at + 1] !== 0xffffffff) out.push(length, all[at + 1]); break; }
    out.push(all[at], all[at + 1]);
  }
  return out;
}
`;

test.skipIf(!existsSync(HERMES))('a reply streaming a 1,500-line block keeps one version of its blocks in the heap', async () => {
  // Cached for every message text, 201 entries of up to 28,000 tokens each, the blocks filled a 64 MiB heap at about
  // 140 versions.
  const code = Array.from({ length: 1_500 }, (_, i) => `export function stream${i}(value: number): number { return value * ${i} + base; } // ${i}`).join('\n');
  const { status, out } = await underHermes({ CODE: code, RECORDS: lineRecords(72, code) }, `
var head = 'Here is the block.\\n\\n\\u0060\\u0060\\u0060ts\\n', versions = 0;
function recordsOf(text) { return prefixRecords(RECORDS, text.length); }
for (var end = 500; end <= CODE.length; end += 500) {
  var reply = head + CODE.slice(0, end);
  H.messageCodeBlocks(reply, null, 'reply');
  settle(recordsOf);
  H.messageCodeBlocks(reply, null, 'reply');
  if (++versions % 20 === 0) measure();
}
measure();
print('done: ' + versions + ' versions, peak ' + Math.round(peak / 1048576) + ' MiB');
`);
  console.log(out.trim().split('\n').slice(-2).join('\n'));
  expect(status).toBe(0);
  expect(out).toContain(`done: ${Math.floor(code.length / 500)} versions`);
}, 300_000);

test.skipIf(!existsSync(HERMES))('160 lexed texts no view paints stay within the heap', async () => {
  // Kept up to 4,000,000 characters, 149 texts of 26,814 characters, they filled a 64 MiB heap at about 80.
  const body = 'export const value = compute(input, { retries: 3, label: "x" });\n'.repeat(400);
  const { status, out } = await underHermes({ BODY: body, RECORDS: lineRecords(72, `// text 000\n${body}`) }, `
function recordsOf() { return RECORDS; }
for (var i = 0; i < 160; i++) {
  var block = '\\u0060\\u0060\\u0060ts\\n// text ' + ('00' + i).slice(-3) + '\\n' + BODY + '\\u0060\\u0060\\u0060\\n';
  H.messageCodeBlocks(block, null, 'message ' + i);
  settle(recordsOf);
  H.messageCodeBlocks(block, null, 'message ' + i);
  if ((i + 1) % 10 === 0) measure();
}
print('done: 160 texts, peak ' + Math.round(peak / 1048576) + ' MiB');
`);
  console.log(out.trim().split('\n').slice(-2).join('\n'));
  expect(status).toBe(0);
  expect(out).toContain('done: 160 texts');
}, 300_000);
