import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url));
export async function generate() {
  const built = await Bun.build({ entrypoints: [join(here, 'minimap-preview-text.mjs')], target: 'browser', conditions: ['react-native'], minify: true,
    define: { 'process.env.NODE_ENV': '"production"' } });
  if (!built.success) throw new AggregateError(built.logs, 'Minimap preview text bundle failed');
  const text = await built.outputs[0].text();
  return [
    { file: join(here, '../../src/timeline/minimap-text.js'), text },
    { file: join(here, '../../src/timeline/minimap-text.d.ts'), text: 'export function minimapPreviewText(markdown: string): string;\n' },
    { file: join(here, '../../mobile/shared/file-markdown-parser.js'), text },
    { file: join(here, '../../mobile/shared/file-markdown-parser.d.ts'), text: `export interface FileTableRun { id: string; kind: 'text' | 'code' | 'strong' | 'emphasis'; text: string }
export interface FileTableCell { id: string; numeric: boolean; runs: FileTableRun[] }
export interface FileTableRow { id: string; header: boolean; cells: FileTableCell[] }
export interface FileMarkdownInput { readonly kind: 'markdown' | 'code'; readonly text: string; readonly language: string }
export type FileMarkdownPart =
  | { readonly kind: 'code'; readonly blockIndex: number }
  | { readonly kind: 'markdown'; readonly blockIndex: number; readonly idSuffix: string; readonly text: string }
  | { readonly kind: 'heading'; readonly blockIndex: number; readonly idSuffix: string; readonly text: string; readonly level: 1 | 2 | 3 | 4 | 5 | 6 }
  | { readonly kind: 'table'; readonly blockIndex: number; readonly idSuffix: string; readonly columns: number; readonly rows: FileTableRow[] };
export function fileMarkdownPlan(source: string, blocks: readonly FileMarkdownInput[]): readonly FileMarkdownPart[];
` },
  ];
}
