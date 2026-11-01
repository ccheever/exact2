// Tests copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3), with their original names:
// apps/web/src/appearanceFonts.test.ts `describe("font size clamping")`. Changes from the reference:
// `vite-plus/test` is `bun:test`. The last case is new: a stored non-number takes the default.
import { describe, expect, it } from "bun:test";

import { clampCodeFontSize, clampInterfaceFontSize, clampPromptFontSize } from "../../src/appearance/appearance-fonts";
import { chatLaneMetrics } from "../../src/shell/chat-canvas-layout";
// The look first: importing composer-menus.ts on its own enters an import cycle at laid-out-widths.ts.
import { chatMaxWidth, look } from "../../src/appearance/settings-appearance-look";
import { atRootFontSize } from "../../src/composer/composer-menus";

describe("font size clamping", () => {
  it("keeps sizes inside the ranges the UI can absorb", () => {
    expect(clampInterfaceFontSize(16)).toBe(16);
    expect(clampInterfaceFontSize(2)).toBe(12);
    expect(clampInterfaceFontSize(96)).toBe(20);
    expect(clampPromptFontSize(40)).toBe(20);
    expect(clampCodeFontSize(1)).toBe(10);
  });

  it("rounds fractional values and falls back for unusable input", () => {
    expect(clampCodeFontSize(13.4)).toBe(13);
    expect(clampInterfaceFontSize(Number.NaN)).toBe(16);
    expect(clampPromptFontSize(Number.POSITIVE_INFINITY)).toBe(14);
  });

  it("takes the default for a stored value that is not a number", () => {
    expect(clampInterfaceFontSize("20")).toBe(16);
    expect(clampInterfaceFontSize(undefined)).toBe(16);
    expect(clampCodeFontSize(null)).toBe(13);
  });
});

// The clone's own: the JS layout values that follow the root font size (not in the reference's tests).
describe("root font size in the JS layout", () => {
  it("scales the chat width setting and the lane's rem constants", () => {
    expect(chatMaxWidth("comfortable", 16)).toBe(736);
    expect(chatMaxWidth("comfortable", 20)).toBe(920);
    expect(chatMaxWidth("wide", 12)).toBe(864);
    expect(chatMaxWidth("full", 20)).toBe(100000);
    expect(chatLaneMetrics(0, 20)).toEqual({ padding: 60, maxChatWidth: 920, minChatWidth: 800 });
  });

  it("scales the composer's measured menus and leaves them alone at 16", () => {
    const row = { effortMenuWidth: 200, runtimeMenuHeight: 102, sendTipWidth: 80, other: 7 };
    expect(atRootFontSize(row, 16)).toBe(row);
    expect(atRootFontSize(row, 20)).toEqual({ effortMenuWidth: 250, runtimeMenuHeight: 127.5, sendTipWidth: 100, other: 7 });
  });
});

// The native catalog is the source for every picker; monospace restrictions apply on commit.
import { installedFonts } from '../../src/appearance/appearance-fonts';
import { fontFamilies, fontStack } from '../../src/appearance/settings-appearance';
import { applyCoreSetting, applyDeviceSetting } from '../../src/settings/settings-core';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { toasts } from '../../src/shell/toast';

describe('installed font selection', () => {
  const catalog = [{ family: 'Arial', monospace: false }, { family: 'Menlo', monospace: true }, { family: 'Avenir Next', monospace: false }];
  function nativeCatalog(onRead = () => {}): Native {
    return { available: true, watch() {}, later: async () => { onRead(); return { ok: true, generation: 0, value: { fonts: catalog } }; } };
  }
  it('loads the actual native list once per client and exposes proportional families in the monospace picker', async () => {
    const client = new T3Client(); let reads = 0;
    const native = nativeCatalog(() => reads++);
    const first = await installedFonts(client, native);
    expect(await installedFonts(client, native)).toEqual(catalog);
    expect(reads).toBe(1);
    expect(fontFamilies('', true, 'SF Mono', first).map(font => font.label)).toEqual(['SF Mono', 'Arial', 'Menlo', 'Avenir Next']);
    expect(fontFamilies('', false, 'Arial', first)[0]!.detail).toBe('"Arial", system-ui');
    expect(await installedFonts(new T3Client(), { ...native, available: false })).toEqual([]);
  });
  it('keeps installed family names in the runtime CSS stack', () => {
    expect(fontStack('Arial', false)).toBe('"Arial", system-ui');
    expect(fontStack('Menlo', true)).toBe('"Menlo", ui-monospace');
    expect(fontStack('Avenir Next', false)).toBe('"Avenir Next", system-ui');
    expect(fontStack('SF Mono', true)).toBe('ui-monospace');
  });
  it('retains code and terminal fonts and posts the reference toast after a proportional choice', async () => {
    const client = new T3Client(), native = nativeCatalog();
    await applyCoreSetting(client, native, 'code-font:fontFamilyCode|||', 'Menlo');
    await applyCoreSetting(client, native, 'terminal-font:fontFamilyTerminal|||', 'Menlo');
    await applyCoreSetting(client, native, 'code-font:fontFamilyCode|||', 'Arial');
    await applyCoreSetting(client, native, 'terminal-font:fontFamilyTerminal|||', 'Avenir Next');
    const prefs = client.local as unknown as { clientSettings: { fontFamilyCode: string; fontFamilyTerminal: string; fontFamilySans: string } };
    expect(prefs.clientSettings.fontFamilyCode).toBe('Menlo');
    expect(prefs.clientSettings.fontFamilyTerminal).toBe('Menlo');
    expect(toasts(client).map(toast => [toast.title, toast.description, toast.stacked])).toEqual([
      ["\"Arial\" isn't monospace", 'Code and terminal need a fixed-width font, so the current font was kept.', true],
      ["\"Avenir Next\" isn't monospace", 'Code and terminal need a fixed-width font, so the current font was kept.', true],
    ]);
    await applyCoreSetting(client, native, 'interface-font:fontFamilySans|||', 'Arial');
    expect(prefs.clientSettings.fontFamilySans).toBe('Arial');
  });
  it('preserves an explicit Prompt family while Terminal switches between shared and advanced families', async () => {
    const client = new T3Client(), native = nativeCatalog();
    await applyCoreSetting(client, native, 'interface-font:fontFamilySans|||', 'Arial');
    await applyCoreSetting(client, native, 'code-font:fontFamilyCode|||', 'Menlo');
    expect(look(client).promptFont).toBe('"Arial", system-ui');
    await applyCoreSetting(client, native, 'prompt-font:fontFamilyComposer|||', 'Avenir Next');
    // With separate fonts off the prompt box is the interface font (appearanceFonts.ts at v0.1.6); the family is kept.
    expect(look(client).promptFont).toBe('"Arial", system-ui');
    expect(look(client).codeFont).toBe('\"Menlo\", ui-monospace');
    expect(look(client).terminalFont).toBe('Menlo');
    await applyCoreSetting(client, native, 'separate-fonts:typographyAdvanced|||', 'true'); // Typography's Separate fonts row
    expect(look(client).promptFont).toBe('\"Avenir Next\", system-ui');
    expect(look(client).terminalFont).toBe('');
  });
  it('the prompt box follows Interface size until separate fonts are on (#26559)', () => {
    const client = new T3Client();
    const prefs = (client.local as unknown as { clientSettings: Record<string, unknown> }).clientSettings;
    Object.assign(prefs, { fontSizeInterface: 18, fontSizePrompt: 20 });
    expect([look(client).fontSize, look(client).promptSize]).toEqual([18, 15.75]);
    prefs.typographyAdvanced = true;
    expect(look(client).promptSize).toBe(20);
  });
  it('the terminal page\'s face carries Font smoothing, which the page\'s root follows (terminal-host applySmoothing)', () => {
    const client = new T3Client();
    expect(JSON.parse(look(client).terminalFace)).toMatchObject({ smoothing: true });
    applyDeviceSetting(client.local as never, 'fontSmoothing', 'false');
    expect(JSON.parse(look(client).terminalFace)).toMatchObject({ smoothing: false });
  });
  it('rejects a malformed native catalog without caching it', async () => {
    const client = new T3Client(), malformed: Native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: { fonts: [{ family: 42 }] } }) };
    await expect(installedFonts(client, malformed)).rejects.toThrow('The installed font catalog could not be read.');
    expect(await installedFonts(client, nativeCatalog())).toEqual(catalog);
  });
});
