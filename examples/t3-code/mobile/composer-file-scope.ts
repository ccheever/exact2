import type { FileHoldTarget } from './composer-file-holds-io';

/** Decodes scope only; native registration and receipts still establish file authority. */
export function mobileComposerFileScope(owner: string, target: unknown): { target: FileHoldTarget; generation: number } | null {
  if (target === null || typeof target !== 'object' || Array.isArray(target)
    || ![Object.prototype, null].includes(Object.getPrototypeOf(target))) return null;
  const fields = ['origin', 'environmentId', 'threadId', 'draftKey'];
  if (Reflect.ownKeys(target).length !== fields.length
    || !fields.every(key => {
      const property = Object.getOwnPropertyDescriptor(target, key);
      return property?.enumerable && 'value' in property
        && typeof property.value === 'string' && property.value.length > 0;
    })) return null;
  const t = target as Record<string, unknown>;
  if (String(t.threadId).startsWith('new:') || String(t.draftKey).includes('~queued-edit~')
    || t.draftKey !== `${t.environmentId}:${t.threadId}`) return null;
  try {
    const decoded: unknown = JSON.parse(owner);
    if (!Array.isArray(decoded) || ![5, 6].includes(decoded.length) || decoded[0] !== 'ordinary'
      || decoded[1] !== t.origin || decoded[2] !== t.environmentId || decoded[4] !== t.draftKey
      || typeof decoded[3] !== 'number' || !Number.isSafeInteger(decoded[3]) || decoded[3] < 0) return null;
    return { target: t as unknown as FileHoldTarget, generation: decoded[3] };
  } catch { return null; }
}
