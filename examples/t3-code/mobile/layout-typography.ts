// @ref llp/1109.011-responsive-workspace.decision.md#source-helpers
// Source: T3 Code 365aa87982a4d81cc8e0c085e8d1a40ca7daecdc apps/mobile/src/lib/typography.ts
// Original SHA256: abd581c51c343c0132d1ab7a33a431e0dc6e76a8385f579d6aff30d3cc82a6e1
export const MOBILE_TYPOGRAPHY = {
  micro: { fontSize: 11, lineHeight: 14 },
  caption: { fontSize: 12, lineHeight: 16 },
  label: { fontSize: 13, lineHeight: 17 },
  footnote: { fontSize: 14, lineHeight: 19 },
  body: { fontSize: 16, lineHeight: 23 },
  headline: { fontSize: 18, lineHeight: 23 },
  title: { fontSize: 21, lineHeight: 28 },
  largeTitle: { fontSize: 26, lineHeight: 32 },
  display: { fontSize: 30, lineHeight: 36 },
} as const;

/** Shared geometry for dense, horizontally scrolling code surfaces. */
export const MOBILE_CODE_SURFACE = {
  rowHeight: 22,
  gutterWidth: 46,
  codePadding: 7,
  textVerticalInset: 2,
  fontSize: MOBILE_TYPOGRAPHY.caption.fontSize,
  lineNumberFontSize: MOBILE_TYPOGRAPHY.micro.fontSize,
} as const;

// @ref llp/1109-t3-code-ios.rfc.md#architecture
// Source: T3 Code 365aa87982a4d81cc8e0c085e8d1a40ca7daecdc apps/mobile/src/lib/appearancePreferences.ts
// Original SHA256: 9141d3b999b5122063c13f3eeed41dfebcc2d0104b2f9aa45721b9c08238450c
// Exact constant/function slices needed by layout; unrelated appearance/terminal helpers omitted.
export const DEFAULT_BASE_FONT_SIZE = MOBILE_TYPOGRAPHY.body.fontSize;
export const MIN_BASE_FONT_SIZE = 11;
export const MAX_BASE_FONT_SIZE = 22;
export function normalizeBaseFontSize(value: number | null | undefined): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return DEFAULT_BASE_FONT_SIZE;
  }

  return Math.min(MAX_BASE_FONT_SIZE, Math.max(MIN_BASE_FONT_SIZE, Math.round(value)));
}

export function scaledTypographyLineHeight(
  role: { readonly lineHeight: number },
  baseFontSize: number,
): number {
  const scale = normalizeBaseFontSize(baseFontSize) / DEFAULT_BASE_FONT_SIZE;
  return Math.max(10, Math.round(role.lineHeight * scale));
}
