// A CSV or TSV cell cut at 2000 characters keeps its last surrogate pair whole (T3 Code 5d3d78ab93, #27497:
// packages/shared/src/delimitedPreview.test.ts, "Unicode cells").
import { describe, expect, it } from 'bun:test';
import { parseDelimitedPreview } from '../../file-preview-host/src/shared/delimited-preview';

describe.each([',', '\t'] as const)('Unicode cells with delimiter %j', delimiter => {
  it.each([false, true])('does not split surrogate pairs in quoted=%s cells', quoted => {
    const prefix = 'x'.repeat(1999);
    const value = `${prefix}😀tail`;
    const input = `${quoted ? `"${value}"` : value}${delimiter}next\nlast`;
    expect(parseDelimitedPreview(input, delimiter)).toEqual({ rows: [[prefix, 'next'], ['last']], truncated: true });
  });
  it('keeps pairs that fit and truncates later characters', () => {
    const value = `${'x'.repeat(1998)}😀`;
    expect(parseDelimitedPreview(value, delimiter)).toEqual({ rows: [[value]], truncated: false });
    expect(parseDelimitedPreview(`${value}tail`, delimiter)).toEqual({ rows: [[value]], truncated: true });
  });
  it('keeps escaped quotes and BMP text at the same limit', () => {
    const prefix = 'x'.repeat(1999);
    expect(parseDelimitedPreview(`"${prefix}""tail"`, delimiter)).toEqual({ rows: [[`${prefix}"`]], truncated: true });
    expect(parseDelimitedPreview(`${prefix}éextra`, delimiter)).toEqual({ rows: [[`${prefix}é`]], truncated: true });
  });
});
