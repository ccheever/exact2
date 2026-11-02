// ChatView onSend's title seed, sent with every message (client-runtime operations/threadTitle.ts
// deriveThreadTitleSeed at T3 Code 74541bab17, MIT; see LICENSE-T3): the prompt with each context chip
// read as its label and each image chip left out, assistant quotes as their text and whitespace
// collapsed; with no prose, "Attached image(s)" for image chips or image attachments, then the first
// attachment's name ("File: …"), then the first terminal, review comment ("Review: …") or preview
// annotation context. truncate() keeps 50 characters.
import { citationsToPlainText, contextReferences, replaceContextReferences } from './composer-editor-menu';

const normalize = (value: string) => citationsToPlainText(value).trim().replace(/\s+/gu, ' ');
const truncate = (value: string) => { const trimmed = value.trim(); return trimmed.length <= 50 ? trimmed : `${trimmed.slice(0, 50)}...`; };
const imagesLabel = (count: number) => count === 1 ? 'Attached image' : 'Attached images';

/** threadTitleTextFromPrompt. */
export function titleTextFromPrompt(text: string): string {
  const images = new Set<string>();
  const prose = replaceContextReferences(text, reference => {
    if (reference.kind === 'image') { images.add(reference.id); return ''; }
    return reference.label;
  });
  return normalize(prose) || (images.size > 0 ? imagesLabel(images.size) : '');
}

export function launchTitle(text: string, attachments: ReadonlyArray<{ type?: unknown; name?: unknown }> = []): string {
  const prose = titleTextFromPrompt(text);
  if (prose) return truncate(prose);
  const images = attachments.filter(attachment => attachment.type === 'image');
  if (images.length > 0) return imagesLabel(images.length);
  const name = normalize(typeof attachments[0]?.name === 'string' ? attachments[0].name : '');
  if (name) return truncate(`File: ${name}`);
  const references = contextReferences(text), first = (kind: string) => references.find(reference => reference.kind === kind)?.label;
  const review = first('review-comment');
  for (const label of [first('terminal'), review === undefined ? undefined : `Review: ${review}`, first('preview-annotation')]) {
    const normalized = normalize(label ?? '');
    if (normalized) return truncate(normalized);
  }
  return 'New thread';
}
