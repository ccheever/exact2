// Rendered previews (lane r4-surfaces; MIT reference, see LICENSE-T3: DelimitedTablePreview.tsx and
// packages/shared/src/delimitedPreview.ts): CSV/TSV as rows. Markdown, a file in Files or a sent attachment, is
// parsed by the chat renderer itself (app.contract filesMarkdown over macos/src/markdown.rs: r4-surfaces-files.ts
// renderedMarkdown, r5-panels-attach.ts renderedAttachment).

/** parseDelimitedPreview: quoted delimiters, escaped quotes and multiline cells, 100 rows × 30 columns. */
export function parseDelimited(text: string, delimiter: ',' | '\t'): { rows: string[][]; truncated: boolean } {
  const rows: string[][] = [];
  let row: string[] = [], cell = '', quoted = false, truncated = false;
  let rowStart = text.charCodeAt(0) === 0xfeff ? 1 : 0;
  const endCell = () => { if (row.length < 30) row.push(cell); else truncated = true; cell = ''; };
  for (let index = rowStart; index < text.length; index++) {
    const char = text[index];
    if (char === '"') {
      if (quoted && text[index + 1] === '"') { if (cell.length < 2000) cell += '"'; else truncated = true; index++; continue; }
      if (quoted || cell === '') { quoted = !quoted; continue; }
    }
    if (!quoted && (char === delimiter || char === '\n' || char === '\r')) {
      endCell();
      if (char !== delimiter) {
        rows.push(row); row = [];
        if (char === '\r' && text[index + 1] === '\n') index++;
        rowStart = index + 1;
        if (rows.length === 100) return { rows, truncated: truncated || index < text.length - 1 };
      }
    } else if (cell.length < 2000) cell += char;
    else truncated = true;
  }
  if (rowStart < text.length) { endCell(); rows.push(row); }
  return { rows, truncated: truncated || quoted };
}
export function tableRows(path: string, text: string) {
  const { rows, truncated } = parseDelimited(text, /\.tsv$/i.test(path) ? '\t' : ',');
  return { truncated, rows: rows.map((cells, index) => ({ id: String(index), header: index === 0, cells: cells.map((cell, at) => ({ id: String(at), text: cell, syntax: '' })) })) };
}
