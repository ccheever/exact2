import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url));
export async function generate() {
  const built = await Bun.build({ entrypoints: [join(here, 'minimap-preview-text.mjs')], target: 'browser', conditions: ['react-native'], minify: true,
    define: { 'process.env.NODE_ENV': '"production"' } });
  if (!built.success) throw new AggregateError(built.logs, 'Minimap preview text bundle failed');
  return [
    { file: join(here, '../../src/timeline/minimap-text.js'), text: await built.outputs[0].text() },
    { file: join(here, '../../src/timeline/minimap-text.d.ts'), text: 'export function minimapPreviewText(markdown: string): string;\n' },
  ];
}
