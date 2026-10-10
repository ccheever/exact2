// T3 Code (MIT, LICENSE-T3)365aa87982 apps/mobile/src/lib/attachmentDocumentPresentation.ts.
// Source SHA256 bb3c933f43ed11b858b396c8c0847191f192570e9fc7a43c3aac0ba3a0b5c995; only import paths adapted.
// @ref llp/1109.005-composer-and-transcript.decision.md#media-presentation
import type { FilePreviewKind } from "./attachment-document-kind";

/** The available preview and selected body must agree, including source-only draft files. */
export function attachmentDocumentPresentation(input: {
  kind: FilePreviewKind;
  hasTable: boolean;
  hasEnvironment: boolean;
  rendered: boolean;
}) {
  const renderedMode = input.hasTable
    ? "table"
    : input.kind === "markdown" && input.hasEnvironment
      ? "markdown"
      : input.kind === "html"
        ? "html"
        : null;
  return {
    renderedMode,
    activeMode: input.rendered && renderedMode !== null ? renderedMode : "source",
  } as const;
}
