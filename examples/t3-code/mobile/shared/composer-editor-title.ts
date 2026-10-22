// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/composer-editor-title.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// ChatView onSend's titleSeed for a new thread: the prompt without its context
// chips, assistant quotes as their text; with no prose, the first image, then
// the first attached file, names it. truncate() keeps 50 characters.
import { citationsToPlainText, stripContextReferences } from './composer-editor-menu';

export function launchTitle(text: string, imageName = '', fileName = ''): string {
  let seed = citationsToPlainText(stripContextReferences(text.trim())).trim();
  if (!seed) seed = imageName ? `Image: ${imageName}` : fileName ? `File: ${fileName}` : 'New thread';
  return seed.length <= 50 ? seed : `${seed.slice(0, 50)}...`;
}
