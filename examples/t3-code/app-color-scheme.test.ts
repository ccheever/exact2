// app-color-scheme: every surface draws in the app's appearance. Settings › Appearance decides it as T3 Code's
// useTheme resolvedTheme does (System follows macOS; Light and Dark override it), and the window shows the same
// (T3WindowChrome.setAppearance). On macOS `viewport.prefersColorScheme` is the system's appearance beneath the
// window's own, so a view that picked its palette from it drew the other palette whenever the two differed (the
// 2026-10-09 audit's PG-1, PA-11, TH-7). The root resolves one `scheme` and hands it down; these checks read the
// Contract sources. The palettes themselves are proven by the macOS drive in tasks/20261009-app-color-scheme.md.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';

const dir = new URL('./', import.meta.url);
const source = (file: string) => Bun.file(new URL(file, dir)).text();
const contracts = readdirSync(dir).filter(file => file.endsWith('.contract')).sort();
const DERIVE = '  derive scheme = ';

/** The resolved scheme's expression as a JavaScript function of the mode and the system's scheme. */
async function resolved(): Promise<(mode: string, system: string) => string> {
  const line = (await source('app.contract')).split('\n').find(text => text.startsWith(DERIVE));
  if (!line) throw new Error('app.contract: no derive scheme');
  const body = line.slice(DERIVE.length).replaceAll(' or ', ' || ').replaceAll(' and ', ' && ')
    .replaceAll('data.look.mode', 'mode').replaceAll('viewport.prefersColorScheme', 'system');
  return new Function('mode', 'system', `return ${body};`) as (mode: string, system: string) => string;
}

describe('one resolved colour scheme', () => {
  test('viewport.prefersColorScheme is read once, by the root\'s resolved scheme; no view picks a palette from it', async () => {
    const sites: string[] = [];
    for (const file of contracts) {
      (await source(file)).split('\n').forEach((line, index) => {
        if (line.includes('viewport.prefersColorScheme') && !line.trimStart().startsWith('//')) sites.push(`${file}:${index + 1}`);
      });
    }
    const app = (await source('app.contract')).split('\n');
    expect(sites).toEqual([`app.contract:${app.findIndex(line => line.startsWith(DERIVE)) + 1}`]);
    // The views that resolved the mode themselves (the terminal drawer, Settings › Providers, the provider wizard) take it too.
    for (const file of contracts) expect(`${file}: ${(await source(file)).includes('data.look.mode == "system"')}`).toBe(`${file}: false`);
  });

  test('System follows the system appearance; Light and Dark override it', async () => {
    const scheme = await resolved();
    expect(scheme('system', 'dark')).toBe('dark');
    expect(scheme('system', 'light')).toBe('light');
    expect(scheme('light', 'dark')).toBe('light');
    expect(scheme('dark', 'light')).toBe('dark');
    expect(scheme('light', 'light')).toBe('light');
    expect(scheme('dark', 'dark')).toBe('dark');
    // Before the device settings are read the window follows the system (setAppearance ignores another mode).
    expect(scheme('', 'dark')).toBe('dark');
    expect(scheme('', 'light')).toBe('light');
  });

  test('the window hands the scheme to every view that draws a palette by it', async () => {
    expect(await source('app.contract')).toContain('    T3Window(data=data, viewport=viewport, scheme=scheme, ');
    expect(await source('app.contract')).toContain('paletteCursor >= 0, wallTime.epochAtZero + elapsed, scheme, data.revision) as shape PaletteView');
    const window = await source('app-window.contract');
    for (const child of ['ChatColumn', 'RightPanels', 'PagesCover', 'RightSheets', 'WelcomeLayer', 'SettingsWindow', 'ProjectDialogs', 'SettingsModals', 'WindowOverlays']) {
      const call = window.split('\n').find(line => line.trimStart().startsWith(`${child}(`)) ?? '';
      expect(`${child}: ${call.includes('viewport=viewport, scheme=scheme')}`).toBe(`${child}: true`);
    }
    // PA-11: the drawer's terminal and its chrome.
    expect(window).toContain('TerminalDrawer(view=terminalDrawer, look=data.look, dark=(scheme == "dark"), ');
    const main = await source('app-main.contract');
    // PA-11: the right panel's terminal surface (inline and as a sheet).
    expect(main.match(/SurfacePanel\([^\n]*scheme=scheme, /g)?.length).toBe(2);
    // PG-1: the Usage page and its model dialog.
    expect(main).toContain('          when scheme == "dark"\n            UsagePage(controlsLeft=data.controlsLeft, page=usage, keys=usageShortcuts, scheme="dark", ');
    expect(main).toContain('      when scheme == "dark"\n        UsageModelLayer(page=usage, scheme="dark", ');
    expect(main).toContain('UsagePricesLayer(page=usage, scheme=scheme, ');
    // TH-7: the expanded Mermaid diagram's card.
    expect(await source('app-overlays.contract')).toContain('DiagramPreviewDialog(diagram=diagram, windowWidth=viewport.width, windowHeight=viewport.height, scheme=scheme, local=chatLocal)');
  });
});
