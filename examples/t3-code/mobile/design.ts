import { createNativeComposerTheme, flattenThemeColor } from './composer-native-theme';
import { mobileTextRoles } from './settings-preferences';
// @ref examples/t3-code/mobile/docs/1130.002-design-system-parity.spec.md#semantic-colors
// Literal palettes from upstream 365aa87982 generated-uniwind-themes.css.
import t0 from './themes/light.json';
import t1 from './themes/dark.json';
import t2 from './themes/t3-chat-light.json';
import t3 from './themes/t3-chat-dark.json';
import t4 from './themes/grove-light.json';
import t5 from './themes/grove-dark.json';
import t6 from './themes/ocean-light.json';
import t7 from './themes/ocean-dark.json';
import t8 from './themes/ember-light.json';
import t9 from './themes/ember-dark.json';
import t10 from './themes/iris-light.json';
import t11 from './themes/iris-dark.json';

const themes: Record<string, Record<string, string>> = {
  'light': t0,
  'dark': t1,
  't3-chat-light': t2,
  't3-chat-dark': t3,
  'grove-light': t4,
  'grove-dark': t5,
  'ocean-light': t6,
  'ocean-dark': t7,
  'ember-light': t8,
  'ember-dark': t9,
  'iris-light': t10,
  'iris-dark': t11,
};

export function mobileTheme(scheme: string, palette = 't3-code') {
  const dark = scheme === 'dark';
  const variant = `${palette === 't3-code' ? '' : `${palette}-`}${dark ? 'dark' : 'light'}`;
  const tokens = themes[variant] ?? themes[dark ? 'dark' : 'light'];
  const token = (name: string) => tokens[`--color-${name}`];
  return {
    screen: token('screen'), sheet: token('sheet-solid'), foreground: token('foreground'),
    secondary: token('secondary'), secondaryForeground: token('secondary-foreground'),
    colors: {
      groupedCard: token('grouped-card'), input: token('input'), inputBorder: token('input-border'),
      foreground: token('foreground'), muted: token('foreground-muted'), placeholder: token('placeholder'),
      primary: token('primary'), primaryForeground: token('primary-foreground'), border: token('border'),
      iconSubtle: token('icon-subtle'), danger: token('danger'), dangerBorder: token('danger-border'),
      dangerForeground: token('danger-foreground'), subtle: token('subtle'), iconMuted: token('icon-muted'),
      statusConnected: dark ? '#34d399' : '#059669', statusRetrying: token('warning-foreground'),
      switchActiveTrack: token('switch-active-track'),
    },
  };
}

// Same hex/rgba alpha replacement as mobileTheme.ts; preserve source channels.
export function withAlpha(color: string, alpha: number): string {
  if (/^#[0-9a-f]{6}$/i.test(color)) {
    const value = parseInt(color.slice(1), 16);
    return `rgba(${value >> 16}, ${(value >> 8) & 255}, ${value & 255}, ${alpha})`;
  }
  const rgba = color.match(/^rgba?\(([^)]+)\)$/);
  if (rgba) return `rgba(${rgba[1].split(',').slice(0, 3).map(value => value.trim()).join(', ')}, ${alpha})`;
  throw new Error(`Unsupported mobile status color: ${color}`);
}

// Home roles are a separate source shape from connection screens.
export function mobileHomeColors(scheme: string, palette = "t3-code") {
  const dark = scheme === "dark";
  // Upstream mobileThemeVariables.ts: iOS default Light sidebar runtime roles.
  const lightDefaultDrawer = !dark && (palette === "t3-code" || palette === "material-you");
  const variant = `${palette === "t3-code" ? "" : `${palette}-`}${dark ? "dark" : "light"}`;
  const tokens = themes[variant] ?? themes[dark ? "dark" : "light"];
  const token = (name: string) => tokens[`--color-${name}`];
  return {
      menuBackground: token('sheet'), card: token('card'), foreground: token('foreground'), muted: token('foreground-muted'), border: token('border'),
      screen: token('screen'), drawer: lightDefaultDrawer ? withAlpha(token('row-hover'), 1) : token('drawer'),
      drawerForeground: token('drawer-foreground'), drawerMuted: token(lightDefaultDrawer ? 'foreground-muted' : 'drawer-foreground-muted'),
      selectedBackground: token('thread-selected'), selectedForeground: token('thread-selected-foreground'),
      selectedMuted: token('thread-selected-foreground-muted'), selectedQueued: token('user-bubble-foreground-muted'),
      primary: token('primary'), primaryText: token('primary-text'), primaryForeground: token('primary-foreground'), iconSubtle: token('icon-subtle'),
      dangerForeground: token('danger-foreground'), done: token('adaptive-emerald-700-300'), draft: token('adaptive-amber-700-300'),
      tertiary: token('foreground-tertiary'), secondary: token('foreground-secondary'),
      secondaryFill: token('secondary'), secondaryForeground: token('secondary-foreground'),
      borderSubtle: token('border-subtle'), warning: token('warning-foreground'),
      inputStatus: token('adaptive-indigo-600-300'), workingStatus: token('adaptive-sky-600-400'),
      };
}

export function mobilePendingInputColors(scheme: string, palette = 't3-code') {
  const dark = scheme === 'dark', variant = `${palette === 't3-code' ? '' : `${palette}-`}${dark ? 'dark' : 'light'}`;
  const tokens = themes[variant] ?? themes[dark ? 'dark' : 'light'];
  const token = (name: string) => tokens[`--color-${name}`]!;
  return { foreground: token('foreground'), secondary: token('foreground-secondary'), muted: token('foreground-muted'), tertiary: token('foreground-tertiary'),
    iconSubtle: token('icon-subtle'), card: token('card-alt'), border: token('border'), input: token('input'), inputBorder: token('input-border'),
    primary: token('primary'), primaryForeground: token('primary-foreground'), selected: withAlpha(token('primary'), 0.1),
    subtleStrong: token('subtle-strong'), danger: token('danger'), dangerForeground: token('danger-foreground') };
}

export function mobileThreadColors(scheme: string, palette = 't3-code', baseFontSize = 16) {
  const dark = scheme === 'dark';
  const variant = `${palette === 't3-code' ? '' : `${palette}-`}${dark ? 'dark' : 'light'}`;
  const tokens = themes[variant] ?? themes[dark ? 'dark' : 'light'];
  const token = (name: string) => tokens[`--color-${name}`];
  const caption = mobileTextRoles(baseFontSize).caption;
  return {
    screen: token('screen'), card: token('card'), glassFallback: token('glass-fallback'), foreground: token('foreground'), secondary: token('foreground-secondary'),
    muted: token('foreground-muted'), iconSubtle: token('icon-subtle'), iconMuted: token('icon-muted'), border: token('border'),
    borderSubtle: token('border-subtle'), subtle: token('subtle'), userBubble: token('user-bubble'),
    userForeground: token('user-bubble-foreground'), primary: token('primary'), primaryForeground: token('primary-foreground'),
    danger: token('danger'), dangerForeground: token('danger-foreground'), warningForeground: token('warning-foreground'), composerSurface: token('composer-surface'),
    composerBorder: token('composer-border'), placeholder: token('placeholder'),
    codeBackground: token('md-code-bg'), codeForeground: token('md-code-text'),
    userCodeBackground: token('md-user-code-bg'), userCodeForeground: token('md-user-code-text'),
    queuedIntentBorder: token('adaptive-amber-500-a25-400-a25'), queuedIntentBackground: token('adaptive-amber-500-a10-400-a10'),
    queuedIntentForeground: token('adaptive-amber-700-300'), steerIntentBorder: token('adaptive-sky-500-a25-400-a25'),
    steerIntentBackground: token('adaptive-sky-500-a10-400-a10'), steerIntentForeground: token('adaptive-sky-700-300'),
    intentFontSize: caption.size, intentLineHeight: caption.line, intentLetterSpacing: caption.size * 0.025,
    composerBackdrop: `linear-gradient(to bottom, ${withAlpha(token('screen'), 0)} 0%, ${withAlpha(token('screen'), 0.6)} 50%, ${withAlpha(token('screen'), 0.9)} 100%)`,
  };
}

export function mobileComposerColors(scheme: string, palette = 't3-code') {
  const dark = scheme === 'dark';
  const variant = `${palette === 't3-code' ? '' : `${palette}-`}${dark ? 'dark' : 'light'}`;
  const tokens = themes[variant] ?? themes[dark ? 'dark' : 'light'];
  const token = (name: string) => tokens[`--color-${name}`];
  return {
    sheet: token('sheet-solid'), groupedCard: token('grouped-card'), foreground: token('foreground'),
    secondary: token('foreground-secondary'), muted: token('foreground-muted'), icon: token('icon'),
    iconSubtle: token('icon-subtle'), borderSubtle: token('border-subtle'), subtle: token('subtle'),
    primary: token('primary'), danger: token('danger-foreground'),
  };
}

export function mobileArchiveColors(scheme: string, palette = 't3-code') {
  const tokens = themes[`${palette === 't3-code' ? '' : `${palette}-`}${scheme === 'dark' ? 'dark' : 'light'}`] ?? themes.light;
  const token = (name: string) => tokens[`--color-${name}`];
  return { foreground: token('foreground'), muted: token('foreground-muted'), tertiary: token('foreground-tertiary'),
    card: token('card'), border: token('border'), groupedCard: token('grouped-card'), subtle: token('subtle'),
    separator: token('separator'), icon: token('icon'), iconSubtle: token('icon-subtle'), iconMuted: token('icon-muted'),
    danger: token('danger'), dangerBorder: token('danger-border'), dangerForeground: token('danger-foreground'),
    primary: token('primary'), primaryForeground: token('primary-foreground') };
}
export function mobileAgentColors(scheme: string, palette = 't3-code') {
  const tokens = themes[`${palette === 't3-code' ? '' : `${palette}-`}${scheme === 'dark' ? 'dark' : 'light'}`] ?? themes.light;
  const token = (name: string) => tokens[`--color-${name}`];
  return { foreground: token('foreground'), muted: token('foreground-muted'), iconSubtle: token('icon-subtle'),
    iconMuted: token('icon-muted'), border: token('border'), working: token('adaptive-sky-600-400'),
    completed: token('adaptive-emerald-600-400'), failed: token('adaptive-rose-600-400') };
}

// ComposerCommandPopover uses GlassSurface's surface tint, not ComposerSurface's tint.
// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#composer-command-foundation
export function mobileCommandColors(scheme: string, palette = 't3-code') {
  const variant = `${palette === 't3-code' ? '' : `${palette}-`}${scheme === 'dark' ? 'dark' : 'light'}`;
  const tokens = themes[variant] ?? themes[scheme === 'dark' ? 'dark' : 'light'];
  const token = (name: string) => tokens[`--color-${name}`];
  return { foreground: token('foreground'), muted: token('foreground-muted'), tertiary: token('foreground-tertiary'),
    icon: token('icon-subtle'), border: token('border'), surface: token('glass-surface'), fallback: token('glass-fallback') };
}

// Pinned nativeComposerTheme uses opaque chip roles composited over the composer.
// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#composer-command-foundation
export function mobileNativeComposerTheme(scheme: string, palette = 't3-code') {
  const variant = `${palette === 't3-code' ? '' : `${palette}-`}${scheme === 'dark' ? 'dark' : 'light'}`;
  return createNativeComposerTheme(themes[variant] ?? themes[scheme === 'dark' ? 'dark' : 'light']);
}

// Local feedback and usage cards use the source card and tertiary tokens.
// @ref examples/t3-code/mobile/docs/1130.002-design-system-parity.spec.md#semantic-colors
export function mobileThreadLocalColors(scheme: string, palette = 't3-code') {
  const variant = `${palette === 't3-code' ? '' : `${palette}-`}${scheme === 'dark' ? 'dark' : 'light'}`;
  const tokens = themes[variant] ?? themes[scheme === 'dark' ? 'dark' : 'light'];
  return { card: tokens['--color-card'], tertiary: tokens['--color-foreground-tertiary'], subtleStrong: tokens['--color-subtle-strong'] };
}

// T3 Code (MIT), pinned365aa87982 htmlRender.ts and mobile/lib/htmlRenderTheme.ts.
// iOS uses raw native palette roles, not the contrast-adjusted text tokens.
// mobileTheme.ts retains raw RGB in these alpha tokens; discard alpha, never blend it.
export function mobileHtmlRenderTheme(scheme: string, palette = 't3-code'): {
  appearance: 'light' | 'dark'; variables: Readonly<Record<string, string>>;
} {
  const appearance = scheme === 'dark' ? 'dark' : 'light';
  const variant = `${palette === 't3-code' ? '' : `${palette}-`}${appearance}`;
  const tokens = themes[variant] ?? themes[appearance];
  const token = (name: string) => tokens[`--color-${name}`];
  const opaque = (name: string) => flattenThemeColor(withAlpha(token(name), 1), token('screen'));
  const accent = opaque('inline-skill-border');
  const fixed = appearance === 'dark'
    ? { successForeground: '#34d399', infoForeground: '#60a5fa', chart: ['#2dd4bf', '#fbbf24', '#c084fc', '#fb7185', '#a3e635'] }
    : { successForeground: '#047857', infoForeground: '#1d4ed8', chart: ['#0d9488', '#d97706', '#9333ea', '#e11d48', '#65a30d'] };
  return { appearance, variables: {
    '--background': token('screen'),
    '--foreground': token('foreground'),
    '--muted': token('subtle'),
    '--muted-foreground': token('switch-inactive-thumb'),
    '--card': token('card'),
    '--card-foreground': token('foreground'),
    '--popover': opaque('glass-surface'),
    '--popover-foreground': token('foreground'),
    '--secondary': token('secondary'),
    '--secondary-foreground': token('secondary-foreground'),
    '--border': token('border'),
    '--input': token('input-border'),
    '--ring': token('focus'),
    '--primary': token('primary'),
    '--primary-foreground': token('primary-foreground'),
    '--accent': accent,
    // All 12 pinned variants give accent and messageAction the same foreground.
    '--accent-foreground': token('primary-foreground'),
    '--accent-surface': token('inline-skill-background'),
    '--accent-surface-foreground': token('inline-skill-foreground'),
    '--destructive': opaque('danger-border'),
    '--destructive-foreground': token('danger-foreground'),
    '--destructive-surface': token('danger'),
    '--warning': opaque('warning-border'),
    '--warning-foreground': token('warning-foreground'),
    '--warning-surface': token('warning'),
    '--success': '#10b981',
    '--success-foreground': fixed.successForeground,
    '--info': '#3b82f6',
    '--info-foreground': fixed.infoForeground,
    '--code-background': token('md-code-bg'),
    '--code-foreground': token('md-code-text'),
    '--chart-1': accent,
    ...Object.fromEntries(fixed.chart.map((color, index) => [`--chart-${index + 2}`, color])),
    '--radius': '0.625rem',
    '--font-sans': '-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif',
    '--font-mono': '"SF Mono", "SFMono-Regular", Menlo, Consolas, "Liberation Mono", monospace',
  } };
}
