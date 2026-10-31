// A review guide's notification target (`guide:` and its reference as JSON), which the guide's notification
// posts and the shell opens (pages-pr-guide.ts guideOpenTarget, shell-notify.ts notificationOps, shell.ts). Its own
// file so the notification code reads it without importing the guide's.
import { obj } from '../client/domain';

export const GUIDE_TARGET_PREFIX = 'guide:';
/** The guide reference a target names, as JSON, or null when it names none. */
export function decodeGuideOpenTarget(target: string): string | null {
  if (!target.startsWith(GUIDE_TARGET_PREFIX)) return null;
  try { const reference = obj(JSON.parse(decodeURIComponent(target.slice(GUIDE_TARGET_PREFIX.length)))); return typeof reference.projectId === 'string' && typeof reference.repository === 'string' && typeof reference.number === 'number' && reference.number > 0 && typeof reference.environmentId === 'string' ? JSON.stringify(reference) : null; } catch { return null; }
}
