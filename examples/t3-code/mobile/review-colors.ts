// Pinned nativeReviewDiffAdapter.ts and FileTreeBrowser; theme values remain literal imports.
// @ref llp/1107.002-design-system-parity.spec.md#semantic-colors
import light from './themes/light.json';
import dark from './themes/dark.json';
import t3Light from './themes/t3-chat-light.json';
import t3Dark from './themes/t3-chat-dark.json';
import groveLight from './themes/grove-light.json';
import groveDark from './themes/grove-dark.json';
import oceanLight from './themes/ocean-light.json';
import oceanDark from './themes/ocean-dark.json';
import emberLight from './themes/ember-light.json';
import emberDark from './themes/ember-dark.json';
import irisLight from './themes/iris-light.json';
import irisDark from './themes/iris-dark.json';
import { withAlpha } from './design';
import { mobileTextRoles } from './settings-preferences';
const palettes: Record<string, Record<string, string>[]> = {
  't3-code': [light, dark], 't3-chat': [t3Light, t3Dark], grove: [groveLight, groveDark],
  ocean: [oceanLight, oceanDark], ember: [emberLight, emberDark], iris: [irisLight, irisDark],
};
export function mobileReviewColors(scheme: string, palette = 't3-code') {
  const isDark = scheme === 'dark', values = (palettes[palette] ?? palettes['t3-code']!)[isDark ? 1 : 0]!;
  const token = (name: string) => values[`--color-${name}`] ?? '';
  return { foreground: token('foreground'), secondary: token('foreground-secondary'), muted: token('foreground-muted'), tertiary: token('foreground-tertiary'),
    sheet: token('sheet-solid'), card: token('card'), border: token('border'), subtle: token('subtle'), subtleStrong: token('subtle-strong'),
    icon: token('icon'), iconSubtle: token('icon-subtle'), iconMuted: token('icon-muted'), primary: token('primary'), primaryForeground: token('primary-foreground'),
    warning: token('warning'), warningForeground: token('warning-foreground'), addition: isDark ? '#5ECC71' : '#199F43', deletion: isDark ? '#FF6762' : '#D52C36',
    additionBackground: isDark ? '#0d2f28' : '#e5f8f5', deletionBackground: isDark ? '#391415' : '#ffe6e7',
    additionBar: '#00cab1', deletionBar: '#ff2e3f', codeBackground: token('md-code-bg'), codeForeground: token('md-code-text'),
    selection: withAlpha(token('primary'), .1) };
}

// ReviewFileNavigator uses semantic count colors, not the native diff's colors.
export function mobileReviewNavigatorAppearance(scheme: string, palette: string, baseFontSize: number) {
  const values = (palettes[palette] ?? palettes['t3-code']!)[scheme === 'dark' ? 1 : 0]!;
  const roles = mobileTextRoles(baseFontSize);
  return { addition: values['--color-adaptive-emerald-700-300']!, deletion: values['--color-adaptive-rose-700-300']!,
    labelSize: roles.label.size, labelLine: roles.label.line, countSize: roles.caption.size, countLine: roles.caption.line };
}
