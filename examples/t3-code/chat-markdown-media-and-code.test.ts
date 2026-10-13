// chat-markdown-media-and-code (task record .exact/implementation/20261005-t3code-macos-parity/tasks/20261014-chat-markdown-media-and-code.md).
import { describe, expect, test } from 'bun:test';
import { LINE_SCAN_LIMIT, scanWork, tokenizeLine } from './r12-render-textmate';
import { highlightSlice, resetHighlightSlicing, shikiTokens, startHighlightTurn } from './r12-render-highlight';
import { highlight } from './timeline-highlight';

const UNIT = 'template <typename T> std::vector<T> twice(const T& x) { return {x, x}; } ';
const templateLine = (length: number) => UNIT.repeat(Math.ceil(length / UNIT.length)).slice(0, length);

describe('KD-code-cpp-perf: Shiki\'s 500 ms line limit, counted as regex work', () => {
  test('a 2,960-character template-dense cpp line finishes well inside the limit, every character in its scopes', () => {
    tokenizeLine('int x;', 'cpp', null);
    const before = scanWork();
    const result = tokenizeLine(templateLine(2960), 'cpp', null);
    // Before the memo kept its searches across a capture's retokenizing: 1.2e10 units (6.2 s in Hermes); now 4.0e8 (0.25 s).
    expect(scanWork() - before).toBeLessThan(LINE_SCAN_LIMIT * 0.6);
    expect(result.stopped).toBe(false);
    expect(result.pieces.every(piece => piece.scopes.length > 0)).toBe(true);
  });
  test('sliced, the line spreads over turns and paints as it does unsliced', () => {
    const text = `${templateLine(2960)}\nint main() { return 0; }`;
    resetHighlightSlicing();
    const whole = shikiTokens(text, 'cpp');
    resetHighlightSlicing();
    startHighlightTurn();
    let tokens = shikiTokens(text, 'cpp'), turns = 1;
    while (!tokens && turns < 500) { startHighlightTurn(); highlightSlice(); tokens = shikiTokens(text, 'cpp'); turns++; }
    resetHighlightSlicing();
    expect(turns).toBeGreaterThan(5);
    expect(turns).toBeLessThan(500);
    expect(tokens).toEqual(whole);
  });
  test('a 5,920-character line stops at the limit: what came before keeps its scopes, the rest is plain, the next line starts where it stopped', () => {
    const stopped = tokenizeLine(templateLine(5920), 'cpp', null);
    expect(stopped.stopped).toBe(true);
    const rest = stopped.pieces[stopped.pieces.length - 1]!;
    expect(rest.scopes).toEqual([]);
    expect(rest.text.length).toBeGreaterThan(1000);
    expect(rest.text.length).toBeLessThan(5920);
    expect(stopped.pieces.slice(0, -1).every(piece => piece.scopes.length > 0)).toBe(true);
    const next = tokenizeLine('int main() { return 0; }', 'cpp', stopped.state);
    expect(next.stopped).toBe(false);
    expect(next.pieces.some(piece => piece.scopes.some(scope => scope.startsWith('keyword') || scope.startsWith('storage')))).toBe(true);
    // The app path paints the rest plain (synColor class '').
    resetHighlightSlicing();
    const tokens = highlight(`${templateLine(5920)}\nint main() { return 0; }`, 'cpp');
    const firstLine = tokens.map(token => token.text).join('').indexOf('\n');
    let at = 0, plainTail = 0;
    for (const token of tokens) { if (at >= firstLine - rest.text.length && at < firstLine) plainTail += token.cls === '' ? token.text.length : 0; at += token.text.length; }
    expect(plainTail).toBeGreaterThanOrEqual(rest.text.length - UNIT.length);
  });
});
