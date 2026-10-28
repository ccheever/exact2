// A Mermaid file's diagram (MIT reference, see LICENSE-T3: components/files/MermaidViewer.tsx and
// MermaidFilePreview.tsx; T3 Code 7c3b3b3605ef): a `.mmd` / `.mermaid` file in Files or a sent one is the chat's
// MermaidDiagram with its expand button, in the Mermaid page Files' rendered Markdown draws in (timeline-mermaid.ts,
// view "file"). Source the reference will not lay out is named instead (mermaid-file.contract).
import type { T3Client } from '../client/client';
import { filesMermaid, type MermaidDiagramView } from '../timeline/timeline-mermaid';

/** Mermaid lays out DOM before it sanitizes its SVG, so resource and config syntax is refused before that pass. */
const UNSAFE = /%%\s*\{|^\s*---|<\/?[A-Za-z]|<[!?]|@\s*\{|\b(?:img|image)\s*:|url\s*\(|@import|(?:https?|data|blob|file):/im;
export const MERMAID_REFUSED = 'This diagram contains resource URLs, markup, node metadata, or custom configuration. Show Mermaid source to inspect it safely.';

export type MermaidFile = { diagrams: MermaidDiagramView[]; note: string };

/**
 * The file as one diagram, or the refusal. The file goes to filesMermaid as one fence of Files' document, so its
 * expand button opens it as a fence's does; the fence is longer than any backtick run in the file, so no line of it
 * closes the fence.
 */
export function mermaidFile(client: T3Client, text: string): MermaidFile {
  if (UNSAFE.test(text)) return { diagrams: [], note: MERMAID_REFUSED };
  let longest = 2;
  for (const run of text.match(/`+/g) ?? []) longest = Math.max(longest, run.length);
  const fence = '`'.repeat(longest + 1);
  return { diagrams: filesMermaid(client, `${fence}mermaid\n${text}\n${fence}\n`), note: '' };
}
