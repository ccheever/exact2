// ChatView onSend's titleSeed, sent with every message: the prompt without its
// context chips, assistant quotes as their text; with no prose, the first
// image, then the first attached file, then the first terminal, review comment
// or preview annotation context names it. truncate() keeps 50 characters.
import { citationsToPlainText, contextReferences, stripContextReferences } from './composer-editor-menu';

export function launchTitle(text: string, imageName = '', fileName = ''): string {
  let seed = citationsToPlainText(stripContextReferences(text.trim())).trim();
  if (!seed) {
    const references = contextReferences(text), first = (kind: string) => references.find(reference => reference.kind === kind)?.label;
    const terminal = first('terminal'), review = first('review-comment'), annotation = first('preview-annotation');
    seed = imageName ? `Image: ${imageName}` : fileName ? `File: ${fileName}` : terminal ?? (review !== undefined ? `Review: ${review}` : annotation ?? 'New thread');
  }
  return seed.length <= 50 ? seed : `${seed.slice(0, 50)}...`;
}
