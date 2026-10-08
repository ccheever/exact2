// @ref llp/1109.002-design-system-parity.spec.md#semantic-colors
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
  const variant = `${palette === "t3-code" ? "" : `${palette}-`}${dark ? "dark" : "light"}`;
  const tokens = themes[variant] ?? themes[dark ? "dark" : "light"];
  const token = (name: string) => tokens[`--color-${name}`];
  return {
      menuBackground: token('sheet'), card: token('card'), foreground: token('foreground'), muted: token('foreground-muted'), border: token('border'),
      screen: token('screen'), drawer: token('drawer'),
      primary: token('primary'), primaryText: token('primary-text'), primaryForeground: token('primary-foreground'), iconSubtle: token('icon-subtle'),
      dangerForeground: token('danger-foreground'), done: token('adaptive-emerald-700-300'), draft: token('adaptive-amber-700-300'),
      tertiary: token('foreground-tertiary'), secondary: token('foreground-secondary'),
      secondaryFill: token('secondary'), secondaryForeground: token('secondary-foreground'),
      borderSubtle: token('border-subtle'), warning: token('warning-foreground'),
      inputStatus: token('adaptive-indigo-600-300'), workingStatus: token('adaptive-sky-600-400'),
      };
}

export function mobileThreadColors(scheme: string, palette = 't3-code') {
  const dark = scheme === 'dark';
  const variant = `${palette === 't3-code' ? '' : `${palette}-`}${dark ? 'dark' : 'light'}`;
  const tokens = themes[variant] ?? themes[dark ? 'dark' : 'light'];
  const token = (name: string) => tokens[`--color-${name}`];
  return {
    screen: token('screen'), glassFallback: token('glass-fallback'), foreground: token('foreground'), secondary: token('foreground-secondary'),
    muted: token('foreground-muted'), iconSubtle: token('icon-subtle'), iconMuted: token('icon-muted'), border: token('border'),
    borderSubtle: token('border-subtle'), subtle: token('subtle'), userBubble: token('user-bubble'),
    userForeground: token('user-bubble-foreground'), primary: token('primary'), primaryForeground: token('primary-foreground'),
    danger: token('danger'), dangerForeground: token('danger-foreground'), warningForeground: token('warning-foreground'), composerSurface: token('composer-surface'),
    composerBorder: token('composer-border'), placeholder: token('placeholder'),
    codeBackground: token('md-code-bg'), codeForeground: token('md-code-text'),
    userCodeBackground: token('md-user-code-bg'), userCodeForeground: token('md-user-code-text'),
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
