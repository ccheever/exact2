// Adapted from T3 Code365aa87982 shared/mobile composerContextClipboard (MIT, LICENSE-T3).
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { decodeMobileComposerClipboardValue, type MobileComposerClipboardFragment } from './composer-context-schema';
import { formatComposerContextReference, replaceComposerContextReferences } from './composer-editor-document';
import { mobileReferencedComposerContext, type MobileMessageContext } from './mobile-new-task-context';
import type { Obj } from './shared/domain';
import { letGo } from './shared/let-go';

export const MOBILE_COMPOSER_CLIPBOARD_MIME = 'web application/x-t3-context-fragment+json';
const MAX_FRAGMENT_CHARS = 16000000;
export function encodeMobileComposerContextFragment(fragment: MobileComposerClipboardFragment): string | null {
  const encoded = JSON.stringify(fragment);
  return encoded.length <= MAX_FRAGMENT_CHARS ? encoded : null;
}
export function encodeMobileComposerContextHtml(text: string, fragment: string, html?: string): string {
  if (html !== undefined) return `<div data-t3-context-fragment="${encodeURIComponent(fragment)}">${html}</div>`;
  const escaped = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  return `<pre data-t3-context-fragment="${encodeURIComponent(fragment)}">${escaped}</pre>`;
}
export function decodeMobileComposerContextFragment(raw: string | null | undefined): MobileComposerClipboardFragment | null {
  if (!raw || raw.length > MAX_FRAGMENT_CHARS) return null;
  try { return decodeMobileComposerClipboardValue(JSON.parse(raw)); } catch { return null; }
}
export function decodeMobileComposerContextHtml(html: string | null | undefined): MobileComposerClipboardFragment | null {
  if (!html || html.length > MAX_FRAGMENT_CHARS * 9 + 4096) return null;
  const encoded = /data-t3-context-fragment=["']([^"']+)["']/.exec(html)?.[1];
  if (!encoded) return null;
  try { return decodeMobileComposerContextFragment(decodeURIComponent(encoded)); } catch { return null; }
}
export function reidentifyMobileComposerContext(text: string, records: readonly Obj[], createId: () => string) {
  const ids = new Map(records.map(record => {
    const id = createId();
    if (typeof id !== 'string' || id.length > 128 || !/^[a-z0-9_-]+$/i.test(id)) throw new Error('Invalid composer context identity');
    return [String(record.contextId), id];
  }));
  return {
    text: replaceComposerContextReferences(text, reference => formatComposerContextReference({
      ...reference, contextId: ids.get(reference.contextId) ?? reference.contextId,
    })),
    context: { version: 1 as const, records: records.map((record): Obj => ({ ...record,
      contextId: ids.get(String(record.contextId))!,
      ...(record.kind === 'preview-annotation' && record.screenshotContextId
        ? { screenshotContextId: ids.get(String(record.screenshotContextId)) ?? record.screenshotContextId } : {}),
    })) },
  };
}
export interface MobileComposerClipboardInput { text: string; fragment: string; html: string }
export interface MobileComposerClipboardAttachment extends Obj { id: string }
/** All callbacks belong to this invocation. No handle is retained in a document or a module map.
 * importAttachment must journal ownership before producing bytes and check its own awaits.
 * assertCurrent throws the owner's superseded error; it is distinct from a per-file failure.
 * After letGo, the journal (not a finally callback through the lost handle) owns cleanup. */
export interface MobileComposerClipboardImportPorts<A extends MobileComposerClipboardAttachment> {
  createId(): string;
  assertCurrent(): void;
  importAttachment(record: Obj, environmentId: string): Promise<A>;
  discardAttachment(attachment: A): Promise<void>;
}
export interface MobileComposerClipboardImport<A extends MobileComposerClipboardAttachment> {
  text: string; context: MobileMessageContext; attachments: A[]; failures: string[];
}
/** Import coordination only. A successful result transfers file ownership to the caller,
 * which must publish atomically or discard through its live invocation/journal. */
export async function importMobileComposerContextClipboard<A extends MobileComposerClipboardAttachment>(
  input: MobileComposerClipboardInput, existingCount: number, ports: MobileComposerClipboardImportPorts<A>,
  existingContextCount = 0,
): Promise<MobileComposerClipboardImport<A> | null> {
  ports.assertCurrent();
  const fragment = decodeMobileComposerContextFragment(input.fragment) ?? decodeMobileComposerContextHtml(input.html);
  if (!fragment) return null;
  const selected = mobileReferencedComposerContext(input.text, { version: 1, records: fragment.records });
  if (existingContextCount + (selected?.records.length ?? 0) > 200)
    throw new Error('Remove some context items from the draft before pasting more.');
  const imported = reidentifyMobileComposerContext(input.text, selected?.records ?? [], ports.createId);
  const attachments: A[] = [], records: Obj[] = [], failures: string[] = [];
  try {
    for (const record of imported.context.records) {
      ports.assertCurrent();
      if (!('attachmentId' in record)) { records.push(record); continue; }
      let file: A;
      try {
        if (existingCount + attachments.length >= 100) throw new Error('Attachment limit reached');
        file = await ports.importAttachment(record, fragment.source.environmentId);
      } catch (error) {
        if (letGo(error)) throw error;
        ports.assertCurrent();
        failures.push(String(record.name)); continue;
      }
      // Claim the result before checking its owner: cleanup must still know what this await made.
      attachments.push(file);
      ports.assertCurrent();
      records.push({ ...record, attachmentId: file.id });
    }
    ports.assertCurrent();
    return { text: imported.text, context: { version: 1, records }, attachments, failures };
  } catch (error) {
    if (letGo(error)) throw error;
    // An ordinary cancellation can discard already-owned files. Losing this cleanup answer
    // stops immediately; unresolved entries remain in the native ownership journal.
    for (const attachment of attachments) {
      try { await ports.discardAttachment(attachment); }
      catch (cleanupError) { if (letGo(cleanupError)) throw cleanupError; }
    }
    throw error;
  }
}
