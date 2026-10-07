// The font size clamps of Settings › Appearance (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// apps/web/src/appearanceFonts.ts clampFontSize and its three callers, ranges and defaults from
// packages/contracts/src/settings.ts). The interface size is the root font size every `rem` in the
// Contract follows (app.contract `rootFont`, `setRootFontSize`); the prompt and code sizes stay in px
// so they do not scale twice. Change from the reference: the input is `unknown` (a stored value
// may be any JSON); a non-number takes the default, as NaN does there.
export const MIN_INTERFACE_FONT_SIZE = 12;
export const MAX_INTERFACE_FONT_SIZE = 20;
export const DEFAULT_INTERFACE_FONT_SIZE = 16;
export const MIN_PROMPT_FONT_SIZE = 12;
export const MAX_PROMPT_FONT_SIZE = 20;
export const DEFAULT_PROMPT_FONT_SIZE = 14;
export const MIN_CODE_FONT_SIZE = 10;
export const MAX_CODE_FONT_SIZE = 18;
export const DEFAULT_CODE_FONT_SIZE = 13;

function clampFontSize(value: unknown, minimum: number, maximum: number, fallback: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return fallback;
  return Math.min(maximum, Math.max(minimum, Math.round(value)));
}

export function clampInterfaceFontSize(value: unknown): number {
  return clampFontSize(value, MIN_INTERFACE_FONT_SIZE, MAX_INTERFACE_FONT_SIZE, DEFAULT_INTERFACE_FONT_SIZE);
}

export function clampPromptFontSize(value: unknown): number {
  return clampFontSize(value, MIN_PROMPT_FONT_SIZE, MAX_PROMPT_FONT_SIZE, DEFAULT_PROMPT_FONT_SIZE);
}

export function clampCodeFontSize(value: unknown): number {
  return clampFontSize(value, MIN_CODE_FONT_SIZE, MAX_CODE_FONT_SIZE, DEFAULT_CODE_FONT_SIZE);
}
