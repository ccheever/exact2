// Chips without pills stay readable on every theme (T3 Code v0.1.5 themePalette.test.ts contextChipVariables, #25066).
import { describe, expect, test } from 'bun:test';
import { CONTEXT_CHIP_ACCENTS, chipInk, contextChipInks } from '../../src/shared/chip-accents';
import { themeRoles } from '../../src/appearance/settings-appearance';
import { documentKind } from '../../src/composer/composer-chip-presentation';

const rgb = (hex: string) => [1, 3, 5].map(at => parseInt(hex.slice(at, at + 2), 16));
const lum = (hex: string) => { const [r, g, b] = rgb(hex).map(v => { const c = v / 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!; };
const contrast = (a: string, b: string) => { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };

describe('contextChipInks', () => {
  test.each([['t3-code', 'light'], ['t3-code', 'dark'], ['ocean', 'light'], ['ember', 'dark']] as const)('every kind reads at 5:1 on %s %s', (id, mode) => {
    const roles = { errorForeground: mode === 'light' ? '#c10007' : '#ff6467', ...themeRoles(id, mode) };
    const inks = contextChipInks(roles);
    expect(Object.keys(inks)).toEqual(expect.arrayContaining([...Object.keys(CONTEXT_CHIP_ACCENTS), 'invalid']));
    for (const surface of [roles.canvas, roles.surface]) for (const kind of Object.keys(CONTEXT_CHIP_ACCENTS)) {
      expect([kind, contrast(inks[kind]!, surface.slice(0, 7)) >= 4.99]).toEqual([kind, true]);
    }
  });
  test('a light theme darkens the accents and a dark one lightens them, keeping the image chip blue', () => {
    const light = contextChipInks(themeRoles('t3-code', 'light')), dark = contextChipInks(themeRoles('t3-code', 'dark'));
    expect(lum(light.image!)).toBeLessThan(lum(dark.image!));
    const [r, g, b] = rgb(light.image!);
    expect(b!).toBeGreaterThan(r!); expect(b!).toBeGreaterThan(g!);
    expect(chipInk('image', light, dark)).toBe(`light-dark(${light.image}, ${dark.image})`);
    expect(chipInk('neutral', light, dark)).toBe('');
  });
  test('a theme where 5:1 is out of reach takes the best black or white reaches', () => {
    const inks = contextChipInks({ canvas: '#808080', surface: '#808080', surfaceRaised: '#808080', messageSurface: '#808080' });
    expect(contrast(inks.skill!, '#808080')).toBeGreaterThan(3.9);
  });
  test('a bubble on the other side of the canvas keeps its own text for chips', () => {
    expect(contextChipInks({ canvas: '#ffffff', surface: '#ffffff', surfaceRaised: '#ffffff', messageSurface: '#111111', messageForeground: '#fafafa' }).message).toBe('#fafafa');
  });
  test('documents take their own accents by extension', () => {
    expect(['a.pdf', 'b.DOCX', 'c.csv', 'd.pptx', 'e.epub', 'f.ts'].map(documentKind)).toEqual(['doc-pdf', 'doc-word', 'doc-spreadsheet', 'doc-slides', 'doc-epub', '']);
  });
});
