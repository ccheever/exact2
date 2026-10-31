// A file's bytes, read whole under a viewer's limit (MIT reference, see LICENSE-T3: apps/web/src/components/files/
// readBoundedDocument.ts, readBinaryPreview.ts, and OfficeViewer.tsx's and OpenXmlOfficeViewer.tsx's readOfficeFile;
// T3 Code 7c3b3b3605ef). Each says what it refuses in its viewer's words.

async function readBounded(response: Response, limit: number, tooLarge: string, unavailable: string) {
  const reader = response.body?.getReader();
  if (!reader) throw new Error(unavailable);
  const chunks: Uint8Array<ArrayBuffer>[] = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.length;
      if (size > limit) throw new Error(tooLarge);
      chunks.push(value);
    }
  } finally {
    await reader.cancel();
  }
  return new Blob(chunks).arrayBuffer();
}

/** readBoundedDocument: the PDF viewer's read. */
export async function readBoundedDocument(src: string, signal: AbortSignal, limit: number, cache: RequestCache = "default") {
  const response = await fetch(src, { signal, cache });
  if (!response.ok || Number(response.headers.get("content-length")) > limit)
    throw new Error("This file is unavailable or too large to preview.");
  return readBounded(response, limit, "This file is too large to preview.", "This file is unavailable.");
}

/** readBinaryPreview and readOfficeFile: an archive's, a database's or an Office file's read, under 50 MB. */
export async function readPreviewFile(src: string, signal: AbortSignal, limit: number) {
  const response = await fetch(src, { signal });
  if (!response.ok) throw new Error("The file could not be loaded. Close it and try again.");
  if (Number(response.headers.get("content-length")) > limit) throw new Error("Preview limited to files under 50 MB.");
  return readBounded(response, limit, "Preview limited to files under 50 MB.", "The file is unavailable.");
}
