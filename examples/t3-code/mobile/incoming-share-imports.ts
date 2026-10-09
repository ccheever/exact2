// @ref llp/1109.005-composer-and-transcript.decision.md#incoming-share-inbox
import type { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { incomingShareImport, incomingShareObject, type IncomingShareImport } from './incoming-share-model';
import { ClientError } from './shared/protocol';
const field = 'mobileIncomingShareImports';
const clone = <T>(value: T): T => value === undefined ? value : JSON.parse(JSON.stringify(value));
export function mobileIncomingShareImportsHydrate(client: T3Client, saved: Obj): void {
  if (Object.hasOwn(saved, field)) Object.assign(client.local, { [field]: clone(saved[field]) });
}
export function mobileIncomingShareImportsPersisted(client: T3Client): unknown { return clone(obj(client.local)[field]); }
/** Preserve malformed saved ownership rather than silently permitting another import. */
export function mobileIncomingShareImports(client: T3Client): Record<string, Record<string, IncomingShareImport>> {
  const raw = obj(client.local)[field];
  if (raw === undefined) return {};
  const invalid = () => { throw new ClientError('The saved share import receipts are invalid. Keep the inbox and draft.', 'protocol'); };
  if (!incomingShareObject(raw)) return invalid();
  const output: Record<string, Record<string, IncomingShareImport>> = {};
  for (const [key, records] of Object.entries(raw)) {
    if (!incomingShareObject(records)) return invalid();
    output[key] = {};
    for (const [id, value] of Object.entries(records)) {
      const receipt = incomingShareImport(value);
      if (receipt.destination.draftKey !== key || receipt.adoptionId !== id) return invalid();
      output[key][id] = receipt;
    }
  }
  return output;
}
export function mobileIncomingShareImportRemember(client: T3Client, input: IncomingShareImport): void {
  const receipt = incomingShareImport(input), saved = mobileIncomingShareImports(client), key = receipt.destination.draftKey;
  const previous = saved[key]?.[receipt.adoptionId];
  if (previous && JSON.stringify(previous) !== JSON.stringify(receipt)) throw new ClientError('The share import identity changed.', 'protocol');
  saved[key] = { ...saved[key], [receipt.adoptionId]: receipt };
  Object.assign(client.local, { [field]: saved }); client.revision++;
}
