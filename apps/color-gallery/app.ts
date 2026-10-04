// The color gallery's data and its canvas surface (LLP 1100 §10).
import type { Answer, Ctx2D, Frame } from './app.contract.d.ts';

export const appId = 'com.exact.colorgallery';
export const grants = '';

type Swatch = { label: string; color: string };
const rgb = (space: string, mid = '0.5 0.5 0.5') => [
  { label: 'red', color: `color(${space} 1 0 0)` },
  { label: 'green', color: `color(${space} 0 1 0)` },
  { label: 'blue', color: `color(${space} 0 0 1)` },
  { label: 'mid', color: `color(${space} ${mid})` },
  { label: 'white', color: `color(${space} 1 1 1)` },
];
const row = (name: string, note: string, swatches: Swatch[]) => ({ name, note, swatches });
// PQ signal for 203 (SDR white), 406, 812 and 1000 cd/m².
const pq = (space: string) => [
  { label: 'red', color: `color(${space} 0.5807 0 0)` },
  { label: 'white', color: `color(${space} 0.5807 0.5807 0.5807)` },
  { label: '2×', color: `color(${space} 0.6542 0.6542 0.6542)` },
  { label: '4×', color: `color(${space} 0.7291 0.7291 0.7291)` },
  { label: '1000', color: `color(${space} 0.7518 0.7518 0.7518)` },
];
// HLG: 0.75 is reference white, 1 the peak.
const hlg = (space: string) => [
  { label: 'red', color: `color(${space} 0.75 0 0)` },
  { label: 'mid', color: `color(${space} 0.5 0.5 0.5)` },
  { label: 'white', color: `color(${space} 0.75 0.75 0.75)` },
  { label: '0.9', color: `color(${space} 0.9 0.9 0.9)` },
  { label: 'peak', color: `color(${space} 1 1 1)` },
];
const linear = (space: string) => [
  { label: 'red', color: `color(${space} 1 0 0)` },
  { label: 'mid', color: `color(${space} 0.18 0.18 0.18)` },
  { label: 'white', color: `color(${space} 1 1 1)` },
  { label: '2×', color: `color(${space} 2 2 2)` },
  { label: '4×', color: `color(${space} 4 4 4)` },
];
const fn = (labels: string[], colors: string[]) => labels.map((label, i) => ({ label, color: colors[i] }));
const PRIMARY = ['red', 'green', 'blue', 'mid', 'white'];

const CSS = [
  row('srgb', 'the reference', rgb('srgb')),
  row('srgb-linear', 'linear: mid differs', rgb('srgb-linear')),
  row('display-p3', 'wider red, green', rgb('display-p3')),
  row('display-p3-linear', 'P3, linear', rgb('display-p3-linear')),
  row('a98-rgb', 'Adobe RGB', rgb('a98-rgb')),
  row('prophoto-rgb', 'clipped to display', rgb('prophoto-rgb')),
  row('rec2020', 'BT.2020, clipped', rgb('rec2020')),
  row('xyz-d65', '= srgb row', fn(PRIMARY, [
    'color(xyz-d65 0.4124 0.2126 0.0193)', 'color(xyz-d65 0.3576 0.7152 0.1192)', 'color(xyz-d65 0.1805 0.0722 0.9505)',
    'color(xyz-d65 0.2034 0.214 0.233)', 'color(xyz-d65 0.9505 1 1.089)'])),
  row('xyz-d50', '= srgb row', fn(PRIMARY, [
    'color(xyz-d50 0.4361 0.2225 0.0139)', 'color(xyz-d50 0.3851 0.7169 0.0971)', 'color(xyz-d50 0.1431 0.0606 0.7141)',
    'color(xyz-d50 0.2068 0.214 0.1765)', 'color(xyz-d50 0.9642 1 0.8251)'])),
  row('lab', 'past sRGB', fn(PRIMARY, ['lab(54 95 75)', 'lab(85 -120 90)', 'lab(30 70 -120)', 'lab(50 0 0)', 'lab(100 0 0)'])),
  row('lch', 'past sRGB', fn(PRIMARY, ['lch(54 120 38)', 'lch(85 150 143)', 'lch(30 139 300)', 'lch(50 0 0)', 'lch(100 0 0)'])),
  row('oklab', 'past sRGB', fn(PRIMARY, ['oklab(0.65 0.25 0.14)', 'oklab(0.87 -0.28 0.2)', 'oklab(0.45 -0.03 -0.32)', 'oklab(0.6 0 0)', 'oklab(1 0 0)'])),
  row('oklch', 'past sRGB', fn(PRIMARY, ['oklch(0.65 0.29 29)', 'oklch(0.87 0.34 144)', 'oklch(0.45 0.32 265)', 'oklch(0.6 0 0)', 'oklch(1 0 0)'])),
  row('rec2100-pq', 'red · white · 2× · 4× · 1000 nits', pq('rec2100-pq')),
  row('rec2100-hlg', 'red · mid · white · 0.9 · peak', hlg('rec2100-hlg')),
  row('rec2100-linear', 'red · mid · white · 2× · 4×', linear('rec2100-linear')),
];

const PLATFORM = [
  row('--dci-p3', 'cinema P3', rgb('--dci-p3')),
  row('--rec709', 'HDTV', rgb('--rec709')),
  row('--rec2020-srgb-transfer', 'sRGB transfer', rgb('--rec2020-srgb-transfer')),
  row('--rec2020-linear', 'linear', rgb('--rec2020-linear')),
  row('--display-p3-pq', 'red · white · 2× · 4× · 1000', pq('--display-p3-pq')),
  row('--display-p3-hlg', 'red · mid · white · 0.9 · peak', hlg('--display-p3-hlg')),
  row('--rec709-pq', 'red · white · 2× · 4× · 1000', pq('--rec709-pq')),
  row('--rec709-hlg', 'red · mid · white · 0.9 · peak', hlg('--rec709-hlg')),
  row('--aces-cg', 'linear, very wide', rgb('--aces-cg', '0.18 0.18 0.18')),
  row('--romm-rgb', 'ProPhoto', rgb('--romm-rgb')),
  row('--gray-gamma-2.2', '0 · ¼ · ½ · ¾ · 1', fn(['black', 'quarter', 'mid', 'three', 'white'], [0, 0.25, 0.5, 0.75, 1].map((v) => `color(--gray-gamma-2.2 ${v})`))),
  row('--gray-linear', 'linear 0 · ¼ · ½ · ¾ · 1', fn(['black', 'quarter', 'mid', 'three', 'white'], [0, 0.25, 0.5, 0.75, 1].map((v) => `color(--gray-linear ${v})`))),
];

const PICTURES = [
  ['srgb-ramp.png', 'sRGB · PNG · 8-bit'],
  ['untagged.png', 'untagged · PNG · 8-bit (shown as sRGB)'],
  ['srgb-16bit.png', 'sRGB · PNG · 16-bit'],
  ['p3-primaries.png', 'Display P3 · PNG · 8-bit'],
  ['p3-camera.jpg', 'Display P3 · JPEG · 8-bit'],
  ['p3.heic', 'Display P3 · HEIC · 8-bit (Safari only on the web)'],
  ['adobe-rgb.jpg', 'Adobe RGB (1998) · JPEG · 8-bit'],
  ['prophoto-16.tif', 'ProPhoto RGB · TIFF · 16-bit (not in Chrome)'],
  ['cmyk.jpg', 'CMYK · JPEG · 8-bit'],
  ['gray-gamma22.png', 'Gray gamma 2.2 · PNG · 8-bit'],
  ['p3-16bit-ramp.png', 'Display P3 · PNG · 16-bit ramp (no banding)', 12],
  ['linear.exr', 'linear sRGB · OpenEXR · 16-bit float (above 1 clips to white; not on the web)'],
  ['pq-cicp.png', 'BT.2100 PQ · PNG (cICP) · 16-bit · HDR'],
  ['pq.heic', 'BT.2100 PQ · HEIC · 10-bit · HDR (Safari only on the web)'],
  ['pq.avif', 'BT.2100 PQ · AVIF · 10-bit · HDR'],
  ['hlg.heic', 'BT.2100 HLG · HEIC · 10-bit · HDR (Safari only on the web)'],
  ['gainmap-iso.jpg', 'Display P3 + ISO gain map · JPEG · HDR'],
  ['gainmap-iso.heic', 'Display P3 + ISO gain map · HEIC · HDR (Safari only on the web)'],
].map(([file, label, height]) => ({ src: `assets/${file}`, label, height: height ?? 44 }));

// Only a browser has a user agent. Only Safari decodes HEIC and TIFF; no
// browser decodes OpenEXR.
const ua: string = (globalThis as any).navigator?.userAgent ?? '';
const web = ua !== '';
const safari = /Safari\//.test(ua) && !/Chrome|Chromium|CriOS|Edg|Firefox|FxiOS/.test(ua);
const shows = (src: string) => !web || (!src.endsWith('.exr') && (safari || !/\.(heic|tif)$/.test(src)));
const SAFARI_CONSTRAINED = "Safari doesn't support constrained yet and drops it: this column takes the page's limit, and choosing constrained above means no-limit.";
const HDR = [
  ['gainmap-iso.jpg', 'gain-map JPEG'],
  ['gainmap-iso.heic', 'gain-map HEIC'],
  ['pq-cicp.png', 'PQ PNG'],
  ['pq.avif', 'PQ AVIF'],
  ['pq.heic', 'PQ HEIC'],
  ['hlg.heic', 'HLG HEIC'],
].map(([file, label]) => ({ src: `assets/${file}`, label }));

export const answer: Answer = ((source: string, args: unknown[]) => {
  if (source === 'colorSpaces') {
    if (args[0] === 'platform') return web ? [] : PLATFORM;
    return web ? CSS.map((r) => (r.name.startsWith('rec2100') ? { ...r, note: 'not in browsers yet', swatches: [] } : r)) : CSS;
  }
  if (source === 'picturePairs') {
    const shown = PICTURES.filter((p) => shows(p.src)), pairs = [];
    for (let i = 0; i < shown.length; i += 2) pairs.push({ a: shown[i], b: shown[i + 1] ?? shown[i], two: i + 1 < shown.length });
    return pairs;
  }
  if (source === 'hostKind') return web ? 'web' : 'native';
  if (source === 'hdrPictures') return HDR.filter((p) => shows(p.src));
  // As of Mobile Safari 27.0 (LLP 1100 D8).
  if (source === 'caveat') return safari ? SAFARI_CONSTRAINED : '';
  if (source === 'hidden') {
    const gone = (args[0] === 'hdr' ? HDR : PICTURES).filter((p) => !shows(p.src)).map((p) => p.src.slice(7));
    return gone.length ? `Not shown in this browser (it can't decode them): ${gone.join(', ')}.` : '';
  }
  throw new Error(`no source ${source}`);
}) as unknown as Answer;
export const surfaces: Record<string, number> = { reds: 0 };
export function draw(surface: string, _args: any[], ctx: Ctx2D, frame: Frame): boolean {
  if (surface !== 'reds') throw new Error(`no surface ${surface}`);
  ctx.fillStyle = 'red';
  ctx.fillRect(0, 0, frame.width / 2, frame.height);
  ctx.fillStyle = 'color(display-p3 1 0 0)';
  ctx.fillRect(frame.width / 2, 0, frame.width / 2, frame.height);
  return false;
}
