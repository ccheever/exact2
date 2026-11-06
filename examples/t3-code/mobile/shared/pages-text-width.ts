// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/pages-text-width.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Advance widths of the system UI font (SF Pro) at the two sizes the pages
// lane's menus size themselves by, measured in the served reference's canvas
// (printable ASCII; anything else counts as an average glyph). A popover here
// cannot shrink-wrap wider than its invoker, so menus that the reference
// sizes to their content take this width instead (lane "pages").
const R14 = [3.79, 4.2, 6.54, 8.67, 8.67, 12.8, 9.81, 4.01, 5.2, 5.2, 6.45, 8.67, 4.01, 6.45, 4.01, 4.12, 8.67, 6.34, 8.3, 8.63, 8.86, 8.5, 8.76, 7.82, 8.79, 8.76, 4.01, 4.01, 8.67, 8.67, 8.67, 7.03, 12.7, 9.28, 9.05, 9.87, 10.02, 8.19, 7.86, 10.3, 10.24, 3.6, 7.38, 9.07, 7.8, 12.09, 10.24, 10.65, 8.74, 10.65, 9, 8.77, 8.72, 10.17, 9.28, 13.4, 9.35, 9.02, 9.11, 5.2, 4.12, 5.2, 8.67, 8.02, 6.85, 7.57, 8.45, 7.68, 8.45, 7.85, 4.92, 8.38, 8.09, 3.31, 3.3, 7.45, 3.39, 12.03, 8.02, 8.12, 8.39, 8.38, 5.18, 7.18, 4.94, 8.02, 7.44, 10.69, 7.19, 7.45, 7.4, 5.2, 3.47, 5.2, 8.67];
const R12 = [3.38, 3.73, 5.73, 7.56, 7.56, 11.1, 8.54, 3.56, 4.58, 4.58, 5.66, 7.56, 3.56, 5.66, 3.56, 3.66, 7.56, 5.57, 7.24, 7.52, 7.72, 7.42, 7.64, 6.83, 7.66, 7.64, 3.56, 3.56, 7.56, 7.56, 7.56, 6.15, 11.02, 8.09, 7.89, 8.59, 8.72, 7.15, 6.87, 8.96, 8.91, 3.21, 6.46, 7.9, 6.81, 10.49, 8.91, 9.26, 7.62, 9.26, 7.84, 7.65, 7.61, 8.85, 8.09, 11.61, 8.14, 7.86, 7.94, 4.58, 3.66, 4.58, 7.56, 7, 6, 6.62, 7.37, 6.71, 7.37, 6.86, 4.34, 7.31, 7.06, 2.96, 2.96, 6.52, 3.04, 10.44, 7, 7.09, 7.32, 7.31, 4.57, 6.28, 4.36, 7, 6.5, 9.29, 6.29, 6.52, 6.47, 4.58, 3.11, 4.58, 7.56];
const QUOTE = { 14: 4.01, 12: 3.56 };

/** The rendered width of one line of regular-weight system text at 12 or 14pt. */
export function textWidth(text: string, size: 12 | 14): number {
  const table = size === 14 ? R14 : R12, average = size === 14 ? 8 : 6.9;
  let width = 0;
  for (const char of text) {
    const code = char.charCodeAt(0);
    width += code >= 32 && code < 127 ? table[code - 32]! : char === '\u2019' ? QUOTE[size] : average;
  }
  // Pair kerning tightens a run by about 1% against its glyphs' summed advances.
  return Math.round(width * 0.99 * 10) / 10;
}

/** MenuPopup (min-w-40, p-1, 1pt border) around MenuCheckboxItems: 8 + 16pt check + 8, label, 12, status, 16. */
export function checkMenuWidth(rows: { label: string; status: string }[]): number {
  const widest = rows.reduce((max, row) => Math.max(max, 32 + textWidth(row.label, 14) + (row.status ? 12 + textWidth(row.status, 12) : 0) + 16), 0);
  return Math.max(160, Math.ceil(widest + 10));
}
