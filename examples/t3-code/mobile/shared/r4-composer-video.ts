// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r4-composer-video.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// packages/shared video.ts videoMimeType (T3 Code, MIT; see LICENSE-T3): which attached files are videos
// (their shelf tile, their file chip's film glyph and tint). No imports, so the composer's files module can use it.
const VIDEO_BY_EXTENSION: Record<string, string> = { avi: 'video/x-msvideo', m4v: 'video/mp4', mkv: 'video/x-matroska', mov: 'video/quicktime', mp4: 'video/mp4', ogv: 'video/ogg', webm: 'video/webm' };
const GENERIC = new Set(['application/octet-stream', 'binary/octet-stream', 'application/unknown']);
/** videoMimeType: a definite type answers; the name is evidence only when nothing recorded what this is. */
export function videoMimeType(attachment: { name: string; mimeType: string }): string | null {
  const mimeType = (attachment.mimeType.split(';', 1)[0] ?? '').trim().toLowerCase();
  if (mimeType.startsWith('video/')) return mimeType;
  if (mimeType !== '' && !GENERIC.has(mimeType)) return null;
  const dot = attachment.name.lastIndexOf('.');
  return dot < 0 ? null : VIDEO_BY_EXTENSION[attachment.name.slice(dot + 1).toLowerCase()] ?? null;
}
