import type { FileHoldTarget } from './composer-file-holds-io';

/** Decodes scope only; native registration and receipts still establish file authority. */
export function mobileComposerFileScope(owner: string, target: unknown): { target: FileHoldTarget; generation: number } | null {
  if (target === null || typeof target !== 'object' || Array.isArray(target)
    || ![Object.prototype, null].includes(Object.getPrototypeOf(target))) return null;
  const data = (key: string) => {
      const property = Object.getOwnPropertyDescriptor(target, key);
      return property?.enumerable && 'value' in property
        && typeof property.value === 'string' && property.value.length > 0;
  };
  const ordinary = ['origin', 'environmentId', 'threadId', 'draftKey'];
  const independent = ['kind', 'origin', 'environmentId', 'projectId', 'draftKey', 'createdAt'];
  const fields = Reflect.ownKeys(target).length === ordinary.length ? ordinary : independent;
  if (Reflect.ownKeys(target).length !== fields.length || !fields.every(data)) return null;
  const t = target as Record<string, unknown>;
  try {
    const decoded: unknown = JSON.parse(owner);
    if (!Array.isArray(decoded) || ![5, 6].includes(decoded.length) || decoded[0] !== 'ordinary'
      || decoded[1] !== t.origin || decoded[2] !== t.environmentId || decoded[4] !== t.draftKey
      || typeof decoded[3] !== 'number' || !Number.isSafeInteger(decoded[3]) || decoded[3] < 0) return null;
    if (fields === ordinary) {
      if (String(t.threadId).startsWith('new:') || String(t.draftKey).includes('~queued-edit~')
        || t.draftKey !== `${t.environmentId}:${t.threadId}`) return null;
    } else if (t.kind !== 'new-task' || decoded.length !== 6 || decoded[5] !== t.createdAt
      || String(t.createdAt).length > 128 || !/^new-task:[A-Za-z0-9_-]{1,128}$/.test(String(t.draftKey))
      || String(t.draftKey).startsWith('new-task:pending-')) return null;
    return { target: t as unknown as FileHoldTarget, generation: decoded[3] };
  } catch { return null; }
}
