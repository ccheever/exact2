// @ref llp/1109.010-mobile-browser-devices.decision.md#connection-and-command-ownership
import { mobileTheme } from './design';
import { mobileReviewColors } from './review-colors';
export function mobilePreviewColors(scheme: string, palette = 't3-code') {
  const theme = mobileTheme(scheme, palette), look = mobileReviewColors(scheme, palette);
  const colors = { background: theme.sheet, foreground: look.foreground, muted: look.muted, icon: look.icon,
    secondary: theme.secondary, secondaryForeground: theme.secondaryForeground, border: look.border, input: theme.colors.input, inputBorder: theme.colors.inputBorder,
    primary: look.primary, primaryForeground: look.primaryForeground, subtle: look.subtle };
  return { ...colors, stream: JSON.stringify({ background: colors.background, foreground: colors.foreground, muted: colors.muted,
    buttonBackground: colors.secondary, buttonForeground: colors.secondaryForeground, buttonBorder: colors.border }) };
}
