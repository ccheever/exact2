// Pinned365aa87982 Git sheets and SheetActionButton semantic tokens (MIT).
// @ref llp/1107.002-design-system-parity.spec.md#semantic-colors
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

export function mobileGitColors(scheme: string, themeId = 't3-code') {
  const dark = scheme === 'dark', variant = `${themeId === 't3-code' ? '' : `${themeId}-`}${dark ? 'dark' : 'light'}`;
  const values = themes[variant] ?? themes[dark ? 'dark' : 'light']!;
  const token = (name: string) => values[`--color-${name}`] ?? '';
  return { foreground: token('foreground'), secondary: token('foreground-secondary'), muted: token('foreground-muted'),
    sheet: token('sheet-solid'), card: token('card'), border: token('border'), subtle: token('subtle'), subtleStrong: token('subtle-strong'),
    icon: token('icon'), iconSubtle: token('icon-subtle'), primary: token('primary'), primaryForeground: token('primary-foreground'),
    warningForeground: token('warning-foreground'), addition: token('adaptive-emerald-700-300'), deletion: token('adaptive-rose-700-300'),
    secondaryBackground: token('secondary'), secondaryBorder: token('secondary-border'), secondaryForeground: token('secondary-foreground') };
}
