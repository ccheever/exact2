// Pinned T3 Code 365aa87982 AddProjectScreen and AppTextInput semantic colors.
// @ref llp/1109.002-design-system-parity.spec.md#semantic-colors
import { settingsTokens } from './settings-appearance';

export function mobileAddProjectColors(scheme: string, themeId = 't3-code') {
  const tokens = settingsTokens(scheme, themeId);
  const token = (name: string) => tokens[`--color-${name}`]!;
  return {
    sheet: token('sheet-solid'), groupedCard: token('grouped-card'), foreground: token('foreground'),
    muted: token('foreground-muted'), icon: token('icon'), iconMuted: token('icon-muted'),
    chevron: token('chevron'), borderSubtle: token('border-subtle'), primary: token('primary'),
    primaryForeground: token('primary-foreground'), danger: token('danger'), dangerBorder: token('danger-border'),
    dangerForeground: token('danger-foreground'), input: token('input'), inputBorder: token('input-border'),
    switchOn: token('switch-active-track'), switchOff: token('switch-inactive-track'),
  };
}

// Pinned ProjectCloneBanner semantic classes.
export function mobileNewTaskCloneColors(scheme: string, themeId = 't3-code') {
  const tokens = settingsTokens(scheme, themeId);
  const token = (name: string) => tokens[`--color-${name}`]!;
  return { foreground: token('foreground'), muted: token('foreground-muted'), border: token('border'),
    // Source bg-background has no declared token and emits no utility.
    card: token('card'), background: 'transparent', iconMuted: token('icon-muted'),
    danger: token('danger'), dangerBorder: token('danger-border'), dangerForeground: token('danger-foreground'),
    warning: token('warning'), warningBorder: token('warning-border'), warningForeground: token('warning-foreground') };
}
