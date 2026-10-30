// useEnvironmentIdentificationMode (hooks/useSettings.ts at 6d450a4284, MIT, see LICENSE-T3) and SidebarChromeHeader's
// stage: the sidebar's Nightly artwork, the pill, or nothing, as `look` resolves them (settings-appearance-look.ts).
import { describe, expect, test } from 'bun:test';
import { appStageLabel, environmentIdentification, identificationPillLabel, stageBackdropVariant } from '../../src/appearance/settings-appearance-look';
import { decodeClientPrefs, type ClientPrefs } from '../../src/settings/settings-core';

const prefs = (overrides: Partial<ClientPrefs>): ClientPrefs => ({ ...decodeClientPrefs({}), environmentIdentificationMode: 'artwork', ...overrides });
const nightly = (loaded: boolean, overrides: Partial<ClientPrefs>, mode: string) => environmentIdentification(loaded, prefs(overrides), mode, 'Nightly');

describe('the stage', () => {
  test('a nightly or preview version is the Nightly stage; this stable client is Beta', () => {
    expect([appStageLabel('0.1.4-nightly.20261027.3770'), appStageLabel('0.0.46-preview.20261004.1'), appStageLabel('0.1.5'), appStageLabel()]).toEqual(['Nightly', 'Nightly', 'Beta', 'Beta']);
    expect(['Nightly', 'Dev', 'Beta', 'Latest'].map(stageBackdropVariant)).toEqual(['nightly', 'dev', null, null]);
    expect(['Nightly', 'Dev', 'Beta'].map(identificationPillLabel)).toEqual(['Nightly', 'Dev', '']);
  });
  test('on this stable client nothing shows, whatever the mode or theme', () => {
    for (const environmentIdentificationMode of ['artwork', 'pill', 'none']) {
      for (const themeDark of ['t3-code', 'night-owl']) expect(environmentIdentification(true, prefs({ environmentIdentificationMode, themeDark }), 'dark')).toEqual({ artwork: false, pill: '' });
    }
  });
});

describe('environment identification on a Nightly', () => {
  test('shows nothing until the preferences load', () => {
    expect(nightly(false, {}, 'dark')).toEqual({ artwork: false, pill: '' });
  });
  test('keeps the artwork on the stock theme and the built-in palettes that have it', () => {
    expect(nightly(true, {}, 'system')).toEqual({ artwork: true, pill: '' });
    for (const theme of ['t3-chat', 'grove', 'ocean', 'ember', 'iris']) {
      expect(nightly(true, { themeLight: theme, themeDark: theme }, 'light')).toEqual({ artwork: true, pill: '' });
    }
  });
  test('shows the Nightly pill instead while a custom theme, or the theme editor\'s draft, is the active half', () => {
    expect(nightly(true, { themeDark: 'night-owl' }, 'dark')).toEqual({ artwork: false, pill: 'Nightly' });
    expect(nightly(true, { themeDark: 'night-owl' }, 'light')).toEqual({ artwork: true, pill: '' });
    expect(nightly(true, { themeLight: '__theme-editor-draft' }, 'light')).toEqual({ artwork: false, pill: 'Nightly' });
    // Under System either half without artwork shows the pill.
    expect(nightly(true, { themeDark: 'night-owl' }, 'system')).toEqual({ artwork: false, pill: 'Nightly' });
  });
  test('keeps an explicit pill or none whatever the theme', () => {
    expect(nightly(true, { environmentIdentificationMode: 'pill' }, 'dark')).toEqual({ artwork: false, pill: 'Nightly' });
    expect(nightly(true, { environmentIdentificationMode: 'none', themeDark: 'night-owl' }, 'dark')).toEqual({ artwork: false, pill: '' });
  });
});
