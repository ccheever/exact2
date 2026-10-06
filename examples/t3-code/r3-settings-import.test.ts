// Lane r3-settings: VS Code theme import accent/input derivation (upstream 2b8bb0c188,
// vscodeThemeImport.test.ts "imported themes keep switches and focus rings visible").
import { describe, expect, test } from 'bun:test';
import { convertVsCodeTheme } from './settings-appearance-import';

const rgb = (hex: string) => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16));
const lum = (hex: string) => { const [r, g, b] = rgb(hex).map(v => { const c = v / 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!; };
const contrast = (a: string, b: string) => { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
const convert = (name: string, type: string, colors: Record<string, string>) => {
  const theme = convertVsCodeTheme({ name, type, colors }, []);
  return (theme[theme.appearance] ?? {}) as Record<string, string>;
};

describe('VS Code theme import keeps switches and focus rings visible', () => {
  test('prefers a visible input background over a transparent border', () => {
    expect(convert('Catppuccin Mocha', 'dark', { 'editor.background': '#1e1e2e', 'input.border': '#00000000', 'input.background': '#313244' }).input).toBe('#313244');
  });
  test('keeps unchecked inputs distinct from the checked action color', () => {
    const c = convert('Gruvbox Light Soft', 'light', { 'editor.background': '#f2e5bc', 'button.background': '#45858880', 'input.background': '#45858880', 'input.border': '#928374' });
    expect(contrast(c.input!, c.messageAction!)).toBeGreaterThanOrEqual(1.1);
  });
  test('keeps the derived input distinct when a button reuses it', () => {
    const c = convert('Derived input collision', 'dark', { 'editor.background': '#1e1e2e', focusBorder: '#89b4fa', 'button.background': '#525661' });
    expect(c.messageAction).toBe('#525661');
    expect(c.input).not.toBe('#525661');
    expect(contrast(c.input!, c.canvas!)).toBeGreaterThanOrEqual(1.1);
    expect(contrast(c.input!, c.messageAction!)).toBeGreaterThanOrEqual(1.1);
  });
  test('skips a transparent focus border for a visible accent key', () => {
    const c = convert('Vitesse Dark', 'dark', { 'editor.background': '#121212', focusBorder: '#00000000', 'button.background': '#4d9375' });
    expect(c.accent).toBe('#4d9375');
    expect(c.focus).toBe('#4d9375');
  });
  test('keeps focus visible against an explicit raised surface', () => {
    const c = convert('Raised focus', 'dark', { 'editor.background': '#000000', focusBorder: '#111111', 'editorWidget.background': '#111111', 'button.background': '#4d9375' });
    expect(c.focus).toBe('#4d9375');
    expect(contrast(c.focus!, c.surfaceRaised!)).toBeGreaterThanOrEqual(1.1);
  });
  test('keeps the fallback focus visible against an explicit raised surface', () => {
    const c = convert('Raised fallback focus', 'dark', { 'editor.background': '#121212', 'editorWidget.background': '#346bf1' });
    expect(c.focus).toBe('#ffffff');
    expect(contrast(c.focus!, c.canvas!)).toBeGreaterThanOrEqual(1.1);
  });
  test('skips a transparent or raised-surface button background for the action color', () => {
    expect(convert('Transparent button', 'dark', { 'editor.background': '#121212', focusBorder: '#4d9375', 'button.background': '#00000000' }).messageAction).toBe('#4d9375');
    expect(convert('Raised button', 'dark', { 'editor.background': '#121212', focusBorder: '#4d9375', 'editorWidget.background': '#2a2d3a', 'button.background': '#2a2d3a' }).messageAction).toBe('#4d9375');
  });
  test('uses a visible default accent when the file has no usable accent key', () => {
    const c = convert('No accent', 'dark', { 'editor.background': '#121212', focusBorder: '#121212' });
    expect(c.focus).toBe('#346bf1');
    expect(contrast(c.focus!, c.canvas!)).toBeGreaterThanOrEqual(1.1);
  });
  test('validates placeholders against the resolved raised surface', () => {
    const c = convert('Light Plus Shape', 'light', { 'editor.background': '#eaeff3', 'editor.foreground': '#1f1f1f', 'editorWidget.background': '#ffffff', 'input.placeholderForeground': '#767676' });
    expect(contrast(c.placeholder!, c.surfaceRaised!)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(c.placeholder!, c.surfaceRaised!)).toBeLessThan(contrast(c.text!, c.surfaceRaised!));
  });
});
