// D8's JSON Schema subset, the one every platform's constrained decoding
// carries: `object` with `properties` and `required`, `string` with `enum`
// or `pattern`, `number` and `integer` with `minimum` and `maximum`,
// `boolean`, `array` with `items`, `minItems` and `maxItems`,
// `description`, and `$ref` into `$defs`.

const KEYS = {
  object: ['properties', 'required'],
  string: ['enum', 'pattern'],
  number: ['minimum', 'maximum'],
  integer: ['minimum', 'maximum'],
  boolean: [],
  array: ['items', 'minItems', 'maxItems'],
};
const ANY = ['type', 'description', '$defs', '$schema', 'title'];

/** Why `schema` is outside the subset, or null. */
export function refusal(schema, root = schema, at = '') {
  if (!schema || typeof schema !== 'object' || Array.isArray(schema)) return `${at || 'the schema'} is not an object`;
  if (schema.$ref !== undefined) return resolve(schema.$ref, root) ? null : `${at}: \`${schema.$ref}\` names nothing in \`$defs\``;
  const keys = KEYS[schema.type];
  if (!keys) return `${at || 'the schema'}: type \`${schema.type}\` is outside D8's subset`;
  for (const k of Object.keys(schema)) if (!keys.includes(k) && !ANY.includes(k)) return `${at || 'the schema'}: \`${k}\` is outside D8's subset`;
  for (const [name, d] of Object.entries(schema.$defs ?? {})) { const why = refusal(d, root, `$defs.${name}`); if (why) return why; }
  if (schema.type === 'object') for (const [name, p] of Object.entries(schema.properties ?? {})) { const why = refusal(p, root, `${at}${at ? '.' : ''}${name}`); if (why) return why; }
  if (schema.type === 'array' && schema.items) return refusal(schema.items, root, `${at}[]`);
  return null;
}

const resolve = (ref, root) => typeof ref === 'string' && ref.startsWith('#/$defs/') ? root.$defs?.[ref.slice(8)] : undefined;

/** Why `v` does not satisfy `schema`, or null. */
export function violation(schema, v, root = schema, at = '') {
  if (schema.$ref !== undefined) return violation(resolve(schema.$ref, root) ?? {}, v, root, at);
  const where = at || 'the value';
  switch (schema.type) {
    case 'object': {
      if (!v || typeof v !== 'object' || Array.isArray(v)) return `${where} is not an object`;
      for (const r of schema.required ?? []) if (!(r in v)) return `${where} has no \`${r}\``;
      for (const [k, x] of Object.entries(v)) {
        const p = schema.properties?.[k];
        if (!p) return `${where} has \`${k}\`, which the schema does not`;
        const why = violation(p, x, root, at ? `${at}.${k}` : k);
        if (why) return why;
      }
      return null;
    }
    case 'string':
      if (typeof v !== 'string') return `${where} is not a string`;
      if (schema.enum && !schema.enum.includes(v)) return `${where} is not one of ${schema.enum.join(', ')}`;
      if (schema.pattern && !new RegExp(schema.pattern, 'u').test(v)) return `${where} does not match ${schema.pattern}`;
      return null;
    case 'number': case 'integer':
      if (typeof v !== 'number' || !isFinite(v) || (schema.type === 'integer' && !Number.isInteger(v))) return `${where} is not ${schema.type === 'integer' ? 'an integer' : 'a number'}`;
      if (schema.minimum != null && v < schema.minimum) return `${where} is under ${schema.minimum}`;
      if (schema.maximum != null && v > schema.maximum) return `${where} is over ${schema.maximum}`;
      return null;
    case 'boolean':
      return typeof v === 'boolean' ? null : `${where} is not a bool`;
    case 'array':
      if (!Array.isArray(v)) return `${where} is not an array`;
      if (schema.minItems != null && v.length < schema.minItems) return `${where} has fewer than ${schema.minItems} items`;
      if (schema.maxItems != null && v.length > schema.maxItems) return `${where} has more than ${schema.maxItems} items`;
      for (const [i, x] of v.entries()) { const why = schema.items && violation(schema.items, x, root, `${where}[${i}]`); if (why) return why; }
      return null;
    default:
      return null;
  }
}

/** §9's first instance: each required property in order, a string's first `enum` value or `""`, a number's
 * `minimum` or 0, `false`, and an array of `minItems` first instances. */
export function firstInstance(schema, root = schema) {
  if (schema.$ref !== undefined) return firstInstance(resolve(schema.$ref, root) ?? {}, root);
  switch (schema.type) {
    case 'object': {
      const out = {}, required = schema.required ?? [];
      for (const [k, p] of Object.entries(schema.properties ?? {})) if (required.includes(k)) out[k] = firstInstance(p, root);
      return out;
    }
    case 'string': return schema.enum?.[0] ?? '';
    case 'number': case 'integer': return schema.minimum ?? 0;
    case 'boolean': return false;
    case 'array': return Array.from({ length: schema.minItems ?? 0 }, () => firstInstance(schema.items ?? {}, root));
    default: return null;
  }
}
