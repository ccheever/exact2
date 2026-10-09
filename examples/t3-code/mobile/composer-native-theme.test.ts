// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { expect, test } from 'bun:test';
import { createNativeComposerTheme } from './composer-native-theme';
import { mobileNativeComposerTheme } from './design';
import light from './themes/light.json';

test('all mobile palettes provide opaque native chip roles in both appearances', () => {
  for (const palette of ['t3-code', 'material-you', 't3-chat', 'grove', 'ocean', 'ember', 'iris']) {
    for (const scheme of ['light', 'dark']) {
      const theme = mobileNativeComposerTheme(scheme, palette);
      for (const color of Object.values(theme)) expect(color).toMatch(/^#[\da-f]{6}$/i);
      expect(theme.skillBorder).not.toBe(theme.skillBackground);
    }
  }
});
test('pinned native alpha fixtures blend each role over its actual composer or chip surface', () => {
  const theme = createNativeComposerTheme({ ...light,
    '--color-screen': '#ffffffff', '--color-composer-surface': '#ffffffff',
    '--color-subtle': '#0000000d', '--color-placeholder': '#0000009e',
    '--color-inline-skill-background': '#ffffffff', '--color-inline-skill-border': '#00000080',
  });
  expect(theme.chipBackground).toBe('#f2f2f2');
  expect(theme.placeholder).toBe('#616161');
  expect(theme.skillBorder).toBe('#7f7f7f');
});
test('unknown palette uses the current appearance rather than native hardcoded defaults', () => {
  expect(mobileNativeComposerTheme('dark', 'unknown')).toEqual(mobileNativeComposerTheme('dark'));
  expect(mobileNativeComposerTheme('light').text).toBe(light['--color-foreground']);
});
