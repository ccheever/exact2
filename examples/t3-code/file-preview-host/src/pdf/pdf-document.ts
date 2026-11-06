// PDF.js as the viewer opens it (MIT reference, see LICENSE-T3: apps/web/src/components/files/pdfDocument.ts; T3 Code
// 7c3b3b3605ef). The worker, standard fonts, CMaps and wasm decoders are pdfjs-dist's own files, copied under
// ../pdfjs/ (tools/generate/gen-file-preview.mjs) instead of the reference's Vite asset globs.
import { getDocument, GlobalWorkerOptions } from "pdfjs-dist";

const assets = new URL("../pdfjs/", import.meta.url);
GlobalWorkerOptions.workerSrc = new URL("pdf.worker.min.mjs", assets).href;

export class BinaryDataFactory {
  async fetch({ kind, filename }: { kind: string; filename: string }) {
    const directory = kind === "cMapUrl" ? "cmaps" : kind === "wasmUrl" ? "wasm" : "standard_fonts";
    // The globs named only the files the package ships; a document's own name never leaves the directory.
    if (!/^[\w.+-]+$/.test(filename) || filename.startsWith(".")) throw new Error(`Unsupported PDF ${directory} asset.`);
    const response = await fetch(new URL(`${directory}/${filename}`, assets));
    if (!response.ok || response.headers.get("content-type")?.startsWith("text/html"))
      throw new Error(`PDF ${directory} asset unavailable.`);
    return new Uint8Array(await response.arrayBuffer());
  }
}

export function openPdf(bytes: ArrayBuffer) {
  return getDocument({
    data: new Uint8Array(bytes),
    useSystemFonts: true,
    useWorkerFetch: false,
    BinaryDataFactory,
    // PDF.js imports JavaScript decoders directly, bypassing BinaryDataFactory.
    wasmUrl: new URL("wasm/", assets).href,
    canvasMaxAreaInBytes: 32 * 1024 * 1024,
    useWasm: true,
  });
}
