// Accept source clipboard kinds at the saved-context boundary without normalizing saved bytes.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { decodeMobileComposerContextRecord } from './composer-context-schema';
import type { Obj } from './shared/domain';

/** Struct decoding drops excess fields. They may stay in saved records, but every declared
 * value must already be canonical. A future record's entire JSON payload remains significant. */
function canonicalFields(original: unknown, decoded: unknown): boolean {
  if (original === decoded) return true;
  if (Array.isArray(decoded)) return Array.isArray(original) && original.length === decoded.length &&
    decoded.every((value, index) => canonicalFields(original[index], value));
  if (decoded === null || typeof decoded !== 'object' || original === null || typeof original !== 'object' || Array.isArray(original)) return false;
  return Object.entries(decoded).every(([key, value]) => Object.hasOwn(original, key) && canonicalFields((original as Obj)[key], value));
}
/** Only the newly imported kinds use this decoder-backed gate; existing producer checks stay intact. */
export function mobileImportedContextRecordValid(record: Obj): boolean {
  const decoded = decodeMobileComposerContextRecord(record);
  return decoded !== null && canonicalFields(record, decoded);
}
