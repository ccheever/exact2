// The Canvas 2D gallery's data module (LLP 1056 §4): no sources, only
// surfaces. Each `canvas surface=<name>(…)` in app.contract draws with the
// fixture of that name in fixtures.js, which direct.html also runs against
// Chrome's own context.
import type { Answer, Ctx2D, Frame } from './app.contract.d.ts';
import { fixtures, surfaces as roster } from './fixtures';

export const appId = 'com.exact.canvasgallery';
export const grants = '';
export const answer: Answer = ((source: string) => {
  throw { kind: 'UnknownSource', message: source };
}) as unknown as Answer;
export const surfaces: Record<string, number> = roster;
export function draw(surface: string, args: any[], ctx: Ctx2D, frame: Frame): boolean {
  const fixture = fixtures[surface];
  if (!fixture) throw new Error(`no fixture ${surface}`);
  return fixture(ctx, frame, args) === true;
}
