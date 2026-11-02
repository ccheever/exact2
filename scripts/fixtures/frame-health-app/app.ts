import type { Answer, Ctx2D, Frame } from './app.contract.d.ts';

export const appId = 'com.exact.framehealth';
export const grants = '';
export const answer: Answer = ((source: string): never => {
  throw { kind: 'UnknownSource', message: source };
}) as unknown as Answer;
export const surfaces: Record<string, number> = { motion: 1 };

export function draw(_surface: string, args: unknown[], ctx: Ctx2D, frame: Frame): boolean {
  const running = args[0] === true;
  const position = running ? 1 - Math.abs((frame.time % 2000) / 1000 - 1) : 0;
  ctx.clearRect(0, 0, frame.width, frame.height);
  ctx.fillStyle = '#007aff';
  ctx.fillRect(Math.max(0, frame.width - 24) * position, 12, 24, 24);
  return running;
}
