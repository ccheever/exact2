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
    { file: join(here, '../../mobile/shared/file-markdown-parser.d.ts'), text: `export interface FileTableRun { id: string; kind: 'text' | 'code' | 'strong'; text: string }
export interface FileTableCell { id: string; numeric: boolean; runs: FileTableRun[] }
export interface FileTableRow { id: string; header: boolean; cells: FileTableCell[] }
export interface FileTableSlice { start: number; end: number; columns: number; rows: FileTableRow[] }
export type FileMarkdownPlanSegment =
  | { kind: 'code'; start: number; end: number; value: string; language: string }
  | { kind: 'prose'; start: number; end: number; tables: FileTableSlice[] };
export function fileMarkdownPlan(source: string): FileMarkdownPlanSegment[] | null;
` },
  ];
}
