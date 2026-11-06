import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url));
export async function generate() {
  const built = await Bun.build({ entrypoints: [join(here, 'codex-inline-directives.mjs')], target: 'browser', conditions: ['react-native'], minify: true,
    define: { 'process.env.NODE_ENV': '"production"' } });
  if (!built.success) throw new AggregateError(built.logs, 'Codex inline directive bundle failed');
  return [
    { file: join(here, '../../src/highlight/codex-directives.js'), text: await built.outputs[0].text() },
    { file: join(here, '../../src/highlight/codex-directives.d.ts'), text: 'export function renderCodexInlineDirectivesAsMarkdown(markdown: string): string;\n' },
  ];
}
