// The UTI each `file_handlers` MIME type names on Apple platforms.
// `inode/directory` is the one deviation from IANA's registry —
// freedesktop's spelling for a folder, because the web has no MIME type for
// one and an app that opens a directory (the LLP reader) must be able to say
// so. An unmapped type is refused rather than guessed (fail
// closed, loudly), when any build reads the manifest: the web's too, so a
// manifest the web accepts is one every host builds. The
// common document and image types are here (CSV included), and IANA's
// generic binary is Apple's generic data (`.bin`, `.dat`).
export const DOCUMENT_UTIS = {
  'text/markdown': 'net.daringfireball.markdown',
  'text/plain': 'public.plain-text',
  'text/html': 'public.html',
  'text/csv': 'public.comma-separated-values-text',
  'text/tab-separated-values': 'public.tab-separated-values-text',
  'application/json': 'public.json',
  'application/pdf': 'com.adobe.pdf',
  'application/zip': 'public.zip-archive',
  'application/octet-stream': 'public.data',
  'image/png': 'public.png',
  'image/jpeg': 'public.jpeg',
  'image/gif': 'com.compuserve.gif',
  'image/webp': 'org.webmproject.webp',
  'image/svg+xml': 'public.svg-image',
  'inode/directory': 'public.folder',
};
