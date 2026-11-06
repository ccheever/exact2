// The context chips' ink (T3 Code v0.1.5, MIT; see LICENSE-T3): lib/contextChipAccents.ts and themePalette.ts
// contextChipVariables. A chip has no pill: it is its kind's accent, a text colour, raised to 5:1 against every surface
// a chip sits on in the theme (the canvas, its surfaces and, when it is on the canvas's side, the message bubble), so it
// stays readable on every theme. Only the accent's lightness moves (readableThemeColorOn), toward black on a light
// theme and white on a dark one; a theme where neither reaches 5:1 takes the best that black or white reaches.
// Every lane's chips read this table: the composer's, the timeline's sent chips, file links and citations.
import { readableThemeColorOn, toHex } from '../appearance/settings-themes';

export const CONTEXT_CHIP_ACCENTS = {
  image: 'oklch(0.62 0.136 237)',
  video: 'oklch(0.62 0.16 48)',
  file: 'oklch(0.62 0.136 237)',
  mention: 'oklch(0.62 0.11 215)',
  terminal: 'oklch(0.62 0.134 163)',
  element: 'oklch(0.62 0.134 70)',
  'preview-annotation': 'oklch(0.62 0.134 70)',
  'review-comment': 'oklch(0.62 0.16 292)',
  'pull-request': 'oklch(0.62 0.16 277)',
  'pr-open': 'oklch(0.62 0.134 163)',
  'pr-draft': 'oklch(0.62 0.02 259)',
  'pr-merged': 'oklch(0.62 0.16 292)',
  'pr-closed': 'oklch(0.62 0.16 16)',
  skill: 'oklch(0.62 0.16 322)',
  thread: 'oklch(0.62 0.12 190)',
  environment: 'oklch(0.62 0.11 145)',
  citation: 'oklch(0.62 0.16 259)',
  'doc-pdf': 'oklch(0.62 0.16 24)',
  'doc-word': 'oklch(0.62 0.136 248)',
  'doc-spreadsheet': 'oklch(0.62 0.134 148)',
  'doc-slides': 'oklch(0.62 0.134 56)',
  'doc-epub': 'oklch(0.62 0.16 291)',
} as const;
export type ContextChipAccentKind = keyof typeof CONTEXT_CHIP_ACCENTS;

const CONTEXT_CHIP_CONTRAST = 5;

type Rgb = [number, number, number];
const rgbOf = (hex: string): Rgb => [1, 3, 5].map(at => parseInt(hex.slice(at, at + 2), 16)) as Rgb;
const alphaOf = (hex: string) => hex.length === 9 ? parseInt(hex.slice(7, 9), 16) / 255 : 1;
const luminance = ([r, g, b]: Rgb) => {
  const channel = (value: number) => { const c = value / 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
};
const contrast = (a: Rgb, b: Rgb) => { const x = luminance(a), y = luminance(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
const hexOf = (rgb: Rgb) => '#' + rgb.map(channel => Math.round(channel).toString(16).padStart(2, '0')).join('');

/**
 * contextChipVariables for one theme half: each kind's ink, `invalid` (the theme's error foreground at the same
 * contrast) and `message` (the bubble's own text, when the bubble is on the other side of the canvas's lightness).
 * `colors` are the half's roles (canvas, surface, surfaceRaised, messageSurface, messageForeground, errorForeground).
 */
export function contextChipInks(colors: Record<string, string | undefined>): Record<string, string> {
  const canvasHex = toHex(colors.canvas) ?? '#ffffff', canvas = rgbOf(canvasHex);
  // A translucent role paints over the canvas.
  const onCanvas = (value: string | undefined): Rgb => {
    const hex = toHex(value);
    if (!hex) return canvas;
    const rgb = rgbOf(hex), alpha = alphaOf(hex);
    return rgb.map((channel, index) => canvas[index]! + (channel - canvas[index]!) * alpha) as Rgb;
  };
  const isDark = (color: Rgb) => luminance(color) < 0.179;
  const messageMatchesCanvas = isDark(onCanvas(colors.messageSurface)) === isDark(canvas);
  const surfaces = [canvas, onCanvas(colors.surface), onCanvas(colors.surfaceRaised), ...(messageMatchesCanvas ? [onCanvas(colors.messageSurface)] : [])];
  const onSurfaces = (color: Rgb) => Math.min(...surfaces.map(surface => contrast(color, surface)));
  const black = onSurfaces([0, 0, 0]), white = onSurfaces([255, 255, 255]);
  const darker = black >= white, minContrast = Math.min(CONTEXT_CHIP_CONTRAST, Math.max(black, white));
  // The surface nearest the ink's side is the one that limits its contrast, so solving against it reads on them all.
  const limiting = hexOf(surfaces.reduce((worst, surface) => (darker ? luminance(surface) < luminance(worst) : luminance(surface) > luminance(worst)) ? surface : worst));
  const ink = (accent: string) => toHex(readableThemeColorOn(accent, limiting, minContrast)) ?? accent;
  const inks: Record<string, string> = Object.fromEntries((Object.entries(CONTEXT_CHIP_ACCENTS) as Array<[string, string]>).map(([kind, accent]) => [kind, ink(accent)]));
  if (colors.errorForeground) inks.invalid = ink(colors.errorForeground);
  if (!messageMatchesCanvas && colors.messageForeground) inks.message = colors.messageForeground;
  return inks;
}

/** A chip's ink in both halves of the appearance, as `light-dark()`; '' for a kind with no accent (neutral, currentColor). */
export function chipInk(kind: string, light: Record<string, string>, dark: Record<string, string>): string {
  const a = light[kind], b = dark[kind];
  return a && b ? `light-dark(${a}, ${b})` : '';
}
