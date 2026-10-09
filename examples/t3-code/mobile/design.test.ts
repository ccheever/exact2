import { describe, expect, test } from 'bun:test';
import { mobileHomeColors, mobileTheme, mobileThreadColors } from './design';

describe('iOS sidebar runtime palette', () => {
  // Pinned upstream mobileThemeVariables.test.ts: default iPad Light uses the
  // opaque row-hover frame; Dark keeps its black drawer below the chat canvas.
  for (const palette of ['t3-code', 'material-you']) {
    test(`${palette} adapts only the default Light drawer roles`, () => {
      const light = mobileHomeColors('light', palette);
      expect(light.drawer).toBe('rgba(244, 244, 245, 1)');
      expect(light.drawerMuted).toBe('#6f6f79');
      expect(light.selectedBackground).toBe('#ffffff');
      expect(light.selectedMuted).toBe('#71717b');
      expect(light.screen).toBe('#fcfcfc');
      expect(mobileTheme('light', palette).sheet).toBe('#fcfcfc');
      expect(mobileThreadColors('light', palette).screen).toBe('#fcfcfc');
      const dark = mobileHomeColors('dark', palette);
      expect(dark.drawer).toBe('#000000');
      expect(dark.screen).toBe('#0a0a0a');
      expect(dark.drawerMuted).toBe('#a3a3a3');
    });
  }

  test('custom palettes keep their shared drawer colors in both appearances', async () => {
    for (const palette of ['t3-chat', 'grove', 'ocean', 'ember', 'iris']) {
      for (const scheme of ['light', 'dark']) {
        const tokens = await Bun.file(new URL(`./themes/${palette}-${scheme}.json`, import.meta.url)).json();
        const colors = mobileHomeColors(scheme, palette);
        expect(colors.drawer).toBe(tokens['--color-drawer']);
        expect(colors.drawerMuted).toBe(tokens['--color-drawer-foreground-muted']);
      }
    }
  });
});
