// Settings › Appearance's editable prompt sample (settings-prompt-preview.contract), against
// T3 Code 1e2ecbd975 (MIT, see LICENSE-T3) SettingsFontPreviews.tsx PromptFontPreview: the
// sample is the composer's serialized prompt, and an unset Prompt font is the interface font
// (`font-(family-name:--font-composer,var(--font-sans))`).
import { describe, expect, test } from 'bun:test';
import { decodeClientPrefs } from './settings-core';
import { appearanceSections } from './settings-appearance';
import manifest from './app.json';

const rows = (prefs: Record<string, unknown>) => appearanceSections(decodeClientPrefs(prefs), true).flatMap(section => section.rows);
const row = (prefs: Record<string, unknown>, id: string) => rows(prefs).find(entry => entry.id === id)!;

describe('prompt sample family', () => {
  test('an unset Prompt font draws in the interface family', () => {
    const prefs = { typographyAdvanced: true, fontFamilySans: 'New York', fontFamilyComposer: '' };
    expect(row(prefs, 'interface-font').driver).toBe('ui-serif');
    expect(row(prefs, 'prompt-font').driver).toBe('ui-serif');
    expect(row(prefs, 'prompt-font').label).toBe('New York');
  });
  test('a chosen Prompt font is its own', () => {
    expect(row({ typographyAdvanced: true, fontFamilySans: 'New York', fontFamilyComposer: 'SF Mono' }, 'prompt-font').driver).toBe('ui-monospace');
    expect(row({ typographyAdvanced: true, fontFamilySans: '', fontFamilyComposer: '' }, 'prompt-font').driver).toBe('system-ui');
  });
  test('simple mode samples the prompt under Interface font, advanced under Prompt font', () => {
    expect(rows({ typographyAdvanced: false }).filter(entry => entry.info === 'preview-prompt').map(entry => entry.id)).toEqual(['interface-font']);
    expect(rows({ typographyAdvanced: true }).filter(entry => entry.info === 'preview-prompt').map(entry => entry.id)).toEqual(['prompt-font']);
  });
});

describe('the editable sample', () => {
  test('is a composer-kind textarea with its own hatch and the reference sample', async () => {
    const source = await Bun.file(new URL('settings-prompt-preview.contract', import.meta.url)).text();
    expect(manifest.hatches).toContain('t3-prompt-preview');
    expect(source).toMatch(/^\s*textarea value=prompt input=write hatch="t3-prompt-preview" aria-label="Prompt font preview"/m);
    expect(source).toContain('appearance="none"'); // the editor is focus:outline-none: no field look or ring
    expect(source).toContain('state prompt = "Use $frontend-design to fix the flaky test in [surface.test.ts](apps/web/src/terminal/ghostty/surface.test.ts) and align the header with [SettingsPanels.tsx](apps/web/src/components/settings/SettingsPanels.tsx) before shipping."');
  });
});
