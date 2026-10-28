// useEnvironmentIdentificationMode (hooks/useSettings.ts at 7c3b3b3605ef, MIT, see LICENSE-T3): the sidebar's Nightly
// artwork, the pill, or nothing, as `look` resolves them (settings-appearance-look.ts).
import { describe, expect, test } from 'bun:test';
import { environmentIdentification } from '../../src/appearance/settings-appearance-look';
import { decodeClientPrefs, type ClientPrefs } from '../../src/settings/settings-core';

const prefs = (overrides: Partial<ClientPrefs>): ClientPrefs => ({ ...decodeClientPrefs({}), environmentIdentificationMode: 'artwork', ...overrides });

describe('environment identification', () => {
  test('shows nothing until the preferences load', () => {
    expect(environmentIdentification(false, prefs({}), 'dark')).toEqual({ artwork: false, pill: '' });
  });
  test('keeps the artwork on the stock theme and the built-in palettes that have it', () => {
    expect(environmentIdentification(true, prefs({}), 'system')).toEqual({ artwork: true, pill: '' });
    for (const theme of ['t3-chat', 'grove', 'ocean', 'ember', 'iris']) {
      expect(environmentIdentification(true, prefs({ themeLight: theme, themeDark: theme }), 'light')).toEqual({ artwork: true, pill: '' });
    }
  });
  test('shows the Nightly pill instead while a custom theme, or the theme editor\'s draft, is the active half', () => {
    expect(environmentIdentification(true, prefs({ themeDark: 'night-owl' }), 'dark')).toEqual({ artwork: false, pill: 'Nightly' });
    expect(environmentIdentification(true, prefs({ themeDark: 'night-owl' }), 'light')).toEqual({ artwork: true, pill: '' });
    expect(environmentIdentification(true, prefs({ themeLight: '__theme-editor-draft' }), 'light')).toEqual({ artwork: false, pill: 'Nightly' });
    // Under System either half without artwork shows the pill.
    expect(environmentIdentification(true, prefs({ themeDark: 'night-owl' }), 'system')).toEqual({ artwork: false, pill: 'Nightly' });
  });
  test('keeps an explicit pill or none whatever the theme', () => {
    expect(environmentIdentification(true, prefs({ environmentIdentificationMode: 'pill' }), 'dark')).toEqual({ artwork: false, pill: 'Nightly' });
    expect(environmentIdentification(true, prefs({ environmentIdentificationMode: 'none', themeDark: 'night-owl' }), 'dark')).toEqual({ artwork: false, pill: '' });
  });
});
