import { describe, expect, test } from 'bun:test';
import { mobileHomeColors, mobileHtmlRenderTheme, mobileTheme, mobileThreadColors } from './design';

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

describe('iOS HTML render theme', () => {
  test('keeps raw default text and status colors apart from adjusted UI text and fills', () => {
    const light = mobileHtmlRenderTheme('light');
    expect(light.variables['--muted-foreground']).toBe('#71717b');
    expect(light.variables['--muted-foreground']).not.toBe(mobileThreadColors('light').muted);
    expect(light.variables['--destructive']).toBe('#fb2c36');
    expect(light.variables['--destructive-surface']).toBe('#fcebec');
    expect(light.variables['--warning']).toBe('#fe9a00');
    expect(light.variables['--warning-surface']).toBe('#fcf4e8');
    const dark = mobileHtmlRenderTheme('dark');
    expect(dark.variables['--muted-foreground']).toBe('#818181');
    expect(dark.variables['--muted-foreground']).not.toBe(mobileThreadColors('dark').muted);
    expect(dark.variables['--destructive']).toBe('#fb414a');
    expect(dark.variables['--destructive-surface']).toBe('#301214');
    expect(light.appearance).toBe('light');
    expect(dark.appearance).toBe('dark');
    expect(light.variables['--muted-foreground']).toBe('#71717b');
  });

  test('uses Grove brand and overlay roles without substituting action, focus, or card colors', () => {
    const dark = mobileHtmlRenderTheme('dark', 'grove').variables;
    expect(dark['--accent']).toBe('#69d69a');
    expect(dark['--primary']).toBe('#e3b34e');
    expect(dark['--chart-1']).toBe(dark['--accent']);
    expect(dark['--popover']).toBe('#444d49');
    expect(dark['--card']).toBe('#1b2821');
    expect(dark['--input']).toBe('#4f725f');
    expect(dark['--muted-foreground']).toBe('#9da5a2');
    expect(dark['--muted-foreground']).not.toBe(mobileThreadColors('dark', 'grove').muted);
    const chat = mobileHtmlRenderTheme('dark', 't3-chat').variables;
    expect(chat['--accent']).toBe('#a3004c');
    expect(chat['--ring']).toBe('#db2777');
  });

  test('every built-in appearance stays on its feed canvas and sends opaque role colors', () => {
    for (const palette of ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris']) {
      for (const appearance of ['light', 'dark']) {
        const theme = mobileHtmlRenderTheme(appearance, palette);
        expect(theme.appearance).toBe(appearance);
        expect(theme.variables['--background']).toBe(mobileThreadColors(appearance, palette).screen);
        for (const role of ['--popover', '--accent', '--destructive', '--warning']) {
          expect(theme.variables[role]).toMatch(/^#[0-9a-f]{6}$/);
        }
        expect(theme.variables['--font-sans']).toBe('-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif');
        expect(theme.variables['--font-mono']).toBe('"SF Mono", "SFMono-Regular", Menlo, Consolas, "Liberation Mono", monospace');
      }
    }
  });

  test('unsupported and Material You ids retain the existing iOS default fallback', () => {
    for (const scheme of ['light', 'dark']) {
      expect(mobileHtmlRenderTheme(scheme, 'material-you')).toEqual(mobileHtmlRenderTheme(scheme));
      expect(mobileHtmlRenderTheme(scheme, 'missing')).toEqual(mobileHtmlRenderTheme(scheme));
    }
  });
});
