import { expect, test } from "bun:test";
import { packedUrl } from "./packed";

const worker = "exact-app://app/assets/file-preview/js/document.worker.js";

test("the page's own wasm, fonts and CMaps are read packed; any other request goes as it was asked", () => {
  expect(packedUrl("./generated/parse/docx_parse_bg.wasm", worker)?.href).toBe("exact-app://app/assets/file-preview/js/generated/parse/docx_parse_bg.wasm");
  expect(packedUrl(new URL("../assets/Carlito-Regular.ttf?v=2", worker), worker)?.pathname).toBe("/assets/file-preview/assets/Carlito-Regular.ttf");
  expect(packedUrl("../pdfjs/cmaps/UniJIS-UCS2-H.bcmap", worker)).not.toBeNull();
  // The file being previewed comes from the environment's server, or from a draft's Blob.
  expect(packedUrl("http://127.0.0.1:3773/api/assets/fonts/brand.ttf", worker)).toBeNull();
  expect(packedUrl("blob:exact-app://app/6f1c9a2e", worker)).toBeNull();
  expect(packedUrl("../pdfjs/pdf.worker.min.mjs", worker)).toBeNull();
  expect(packedUrl("not a url", "about:blank")).toBeNull();
});
