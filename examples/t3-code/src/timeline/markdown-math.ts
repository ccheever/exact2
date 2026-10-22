// Display math, adapted from T3 Code 74541bab17 (MIT, see LICENSE-T3): apps/web/src/components/chat/MarkdownMath.tsx
// and MathTypeset.tsx. With Settings › Appearance › Render math on, each display formula is a ```math fence
// (data/src/markdown_math.rs) that the math page (assets/math/, an `iframe` per formula, markdown.contract
// MathBlock) typesets with the reference's KaTeX build and options. The page reports the formula's size, or that
// KaTeX could not parse it (`math-frame`), and this keeps the reports, as MathTypeset's cache keeps its markup, so a
// formula that mounts again (the transcript is virtualized) takes its size at once.
import { obj, str } from '../client/domain';

/** markdown.contract MathFrame: one formula's report in one theme at one text size. */
export interface MathFrame { theme: string; size: number; tex: string; status: string; width: number; height: number }
/** Reports kept, oldest dropped first: every MathBlock filters this list. */
const MAX_FRAMES = 256;
const frames = new Map<string, MathFrame>();

/** `math-frame`: a math page's report, the JSON it posts to its parent (assets/math/render.js). */
export function noteMathFrame(payload: string): string {
  let report;
  try { report = obj(JSON.parse(payload)); } catch { return ''; }
  const tex = str(report.tex), theme = report.theme === 'dark' ? 'dark' : 'light', size = Number(report.size) || 0;
  if (!tex.trim() || !(size > 0)) return '';
  const width = Number(report.width) || 0, height = Number(report.height) || 0;
  const frame: MathFrame = report.status === 'rendered' ? { theme, size, tex, status: 'rendered', width, height } : { theme, size, tex, status: 'error', width: 0, height: 0 };
  if (frame.status === 'rendered' && (!(width > 0) || !(height > 0))) return '';
  const key = `${theme}\n${size}\n${tex}`;
  frames.delete(key);
  frames.set(key, frame);
  while (frames.size > MAX_FRAMES) frames.delete(frames.keys().next().value!);
  return '';
}

export function mathFrames(): MathFrame[] {
  return [...frames.values()];
}
