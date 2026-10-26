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

/**
 * A video's oriented display size from its ISO base media header (MP4, MOV, M4V; ISO/IEC 14496-12 §8.3.2 `tkhd`): the
 * first track with a picture, its width and height turned by a quarter-turn matrix, as AVFoundation's naturalSize under
 * the preferred transform gives them. Null for another container or a header it cannot read; the preview is then 16:9.
 */
export function videoFrameSize(bytes: Uint8Array): { width: number; height: number } | null {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const type = (at: number) => String.fromCharCode(bytes[at]!, bytes[at + 1]!, bytes[at + 2]!, bytes[at + 3]!);
  /** The boxes in [start, end): each one's type and its body's range. */
  const boxes = function* (start: number, end: number): Generator<{ type: string; body: number; end: number }> {
    for (let at = start; at + 8 <= end;) {
      let size = view.getUint32(at), body = at + 8;
      if (size === 1) {
        if (body + 8 > end) return;
        size = view.getUint32(body) * 2 ** 32 + view.getUint32(body + 4);
        body += 8;
      } else if (size === 0) size = end - at;
      if (size < body - at || at + size > end) return;
      yield { type: type(at + 4), body, end: at + size };
      at += size;
    }
  };
  for (const moov of boxes(0, bytes.length)) {
    if (moov.type !== 'moov') continue;
    for (const trak of boxes(moov.body, moov.end)) {
      if (trak.type !== 'trak') continue;
      for (const tkhd of boxes(trak.body, trak.end)) {
        if (tkhd.type !== 'tkhd') continue;
        const matrix = tkhd.body + (bytes[tkhd.body] === 1 ? 52 : 40), size = matrix + 36;
        if (size + 8 > tkhd.end) break;
        const width = view.getUint32(size) / 65536, height = view.getUint32(size + 4) / 65536;
        if (!(width > 0 && height > 0)) break;
        // a, b of the 16.16 matrix: a quarter turn has a = 0 and |b| = 1.
        const quarter = view.getInt32(matrix) === 0 && Math.abs(view.getInt32(matrix + 4)) === 65536;
        return { width: Math.round(quarter ? height : width), height: Math.round(quarter ? width : height) };
      }
    }
  }
  return null;
}
