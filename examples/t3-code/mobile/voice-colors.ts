// @ref llp/1107.008-mobile-voice.decision.md#presentation
import { mobileTheme } from './design';
import { mobileReviewColors } from './review-colors';
export function mobileVoiceColors(scheme: string, palette = 't3-code') {
  const colors = mobileReviewColors(scheme, palette), theme = mobileTheme(scheme, palette);
  return { foreground: colors.foreground, muted: colors.muted, icon: colors.icon, iconMuted: colors.iconMuted,
    primary: colors.primary, primaryForeground: colors.primaryForeground, subtle: colors.subtle,
    danger: theme.colors.dangerForeground, card: colors.card, border: colors.border };
}
